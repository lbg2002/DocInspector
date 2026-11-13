#[cfg(not(standalone_test))]
mod history;
mod matcher;
use matcher::{Matcher, ScanOptions};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use rayon::prelude::*;
#[cfg(not(standalone_test))]
use tauri::Emitter;
use walkdir::WalkDir;
use calamine::{open_workbook_auto, Reader};
use quick_xml::{events::Event, Reader as XmlReader};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub word: String,
    pub kind: String,
    pub enabled: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub word: String,
    pub kind: String,
    pub location: String,
    pub context: String,
    #[serde(default)]
    pub matched: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub path: String,
    pub status: String,
    pub hits: Vec<Hit>,
    pub missing: Vec<String>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub scanned: usize,
    pub total: usize,
    pub current: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub files: Vec<FileResult>,
    pub total: usize,
    pub scanned: usize,
    pub cancelled: bool,
    #[serde(default)]
    pub duration_ms: u64,
}

struct ScanState(Arc<AtomicBool>);

#[derive(Clone)]
pub struct Segment {
    location: String,
    text: String,
    line_starts: Option<Vec<usize>>,
    unit: &'static str,
}

impl Segment {
    pub fn new(location: String, text: String) -> Self {
        Self {
            location,
            text,
            line_starts: None,
            unit: "行",
        }
    }

    pub fn new_lines(text: String) -> Self {
        let mut starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self {
            location: "全文".into(),
            text,
            line_starts: Some(starts),
            unit: "行",
        }
    }

    pub fn new_paragraphs(section: String, text: String) -> Self {
        let mut starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self {
            location: section,
            text,
            line_starts: Some(starts),
            unit: "段",
        }
    }

    pub fn new_page_lines(page: usize, text: String) -> Self {
        let mut starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        let multi = starts.len() > 1;
        Self {
            location: format!("第 {page} 页"),
            text,
            line_starts: if multi { Some(starts) } else { None },
            unit: "行",
        }
    }

    pub fn location_at(&self, byte: usize) -> String {
        if let Some(starts) = &self.line_starts {
            let index = starts.partition_point(|v| *v <= byte);
            if self.location == "全文" {
                format!("第 {} {}", index, self.unit)
            } else {
                format!("{} · 第 {} {}", self.location, index, self.unit)
            }
        } else {
            self.location.clone()
        }
    }
}

fn extract_trailing_number(stem: &str) -> usize {
    let digits: String = stem
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    digits.parse::<usize>().unwrap_or(0)
}

fn office_section_label(name: &str) -> Option<(u8, usize, String)> {
    if name == "word/document.xml" {
        return Some((1, 0, "正文".into()));
    }
    if name.starts_with("word/header") && name.ends_with(".xml") {
        let stem = name.trim_end_matches(".xml");
        let n = extract_trailing_number(stem);
        return Some((2, n, "页眉".into()));
    }
    if name.starts_with("word/footer") && name.ends_with(".xml") {
        let stem = name.trim_end_matches(".xml");
        let n = extract_trailing_number(stem);
        return Some((3, n, "页脚".into()));
    }
    if name == "word/footnotes.xml" {
        return Some((4, 0, "脚注".into()));
    }
    if name == "word/endnotes.xml" {
        return Some((5, 0, "尾注".into()));
    }
    if name == "word/comments.xml" {
        return Some((6, 0, "批注".into()));
    }
    if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
        let stem = name.trim_end_matches(".xml");
        let n = extract_trailing_number(stem);
        let label = if n > 0 {
            format!("第 {n} 页幻灯片")
        } else {
            "幻灯片".into()
        };
        return Some((10, n, label));
    }
    if name.starts_with("ppt/notesSlides/notesSlide") && name.ends_with(".xml") {
        let stem = name.trim_end_matches(".xml");
        let n = extract_trailing_number(stem);
        let label = if n > 0 {
            format!("第 {n} 页备注")
        } else {
            "演讲者备注".into()
        };
        return Some((11, n, label));
    }
    None
}

fn xml_text<R: Read>(input: R) -> Result<String, String> {
    let mut reader = XmlReader::from_reader(std::io::BufReader::new(input));
    let mut buf = Vec::new();
    let mut out = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(t)) => {
                out.push_str(&t.unescape().map_err(|e| e.to_string())?);
            }
            Ok(Event::CData(t)) => {
                if let Ok(s) = std::str::from_utf8(t.as_ref()) {
                    out.push_str(s);
                }
            }
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => match e.name().as_ref() {
                b"w:tab" | b"a:tab" => out.push('\t'),
                b"w:br" | b"a:br" | b"w:cr" => {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) if matches!(e.name().as_ref(), b"w:p" | b"a:p" | b"w:tr" | b"a:tr") => {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn unzip_xml(path: &Path) -> Result<Vec<Segment>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut raw_segments: Vec<(u8, usize, Segment)> = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        if entry.size() > 64 * 1024 * 1024 {
            return Err("Office 文档的解压内容过大".into());
        }
        let name = entry.name().to_string();
        let Some((group, order, label)) = office_section_label(&name) else {
            continue;
        };
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        let text = xml_text(data.as_slice())?;
        if !text.trim().is_empty() {
            raw_segments.push((group, order, Segment::new_paragraphs(label, text)));
        }
    }
    if raw_segments.is_empty() {
        return Err("文档中没有可解析的正文".into());
    }
    raw_segments.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    Ok(raw_segments.into_iter().map(|(_, _, seg)| seg).collect())
}

fn decode_text(mut bytes: Vec<u8>) -> Result<String, String> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes.drain(..3);
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok(text),
        Err(err) => {
            let bytes = err.into_bytes();
            if bytes.starts_with(&[0xFF, 0xFE]) {
                let (cow, had_errors) = encoding_rs::UTF_16LE.decode_without_bom_handling(&bytes[2..]);
                if !had_errors {
                    return Ok(cow.into_owned());
                }
            } else if bytes.starts_with(&[0xFE, 0xFF]) {
                let (cow, had_errors) = encoding_rs::UTF_16BE.decode_without_bom_handling(&bytes[2..]);
                if !had_errors {
                    return Ok(cow.into_owned());
                }
            }
            if bytes.contains(&0) {
                return Err("文件包含二进制数据，不是纯文本".into());
            }
            let (cow, had_errors) = encoding_rs::GB18030.decode_without_bom_handling(&bytes);
            if !had_errors {
                return Ok(cow.into_owned());
            }
            Err("文本不是受支持的 UTF-8 / GB18030 编码".into())
        }
    }
}

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "csv", "log", "json", "xml", "html", "htm", "rs", "py", "js", "ts", "tsx", "jsx",
    "c", "cpp", "h", "java", "toml", "yaml", "yml", "ini", "sh", "cc", "hpp", "go", "sql", "css",
    "scss", "bash", "zsh", "kt", "vue", "svelte", "php", "rb", "tex", "bib", "ipynb", "mjs", "cjs",
    "jsonl", "rst", "conf", "cfg", "env", "properties", "cmake", "less", "sass", "swift", "dart",
    "lua", "r", "scala", "cs", "ps1", "bat", "cmd",
];

fn is_text_ext(ext: &str) -> bool {
    TEXT_EXTENSIONS.contains(&ext)
}

fn extract(path: &Path) -> Result<Vec<Segment>, String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
        return Err("文件超过 64 MB 扫描上限".into());
    }
    match ext.as_str() {
        "pdf" => {
            let pages = pdf_extract::extract_text_by_pages(path).map_err(|e| e.to_string())?;
            if pages.iter().all(|s| s.trim().is_empty()) {
                return Err("没有可提取文字，可能需要 OCR".into());
            }
            Ok(pages
                .into_iter()
                .enumerate()
                .map(|(i, text)| Segment::new_page_lines(i + 1, text))
                .collect())
        }
        "docx" | "pptx" => unzip_xml(path),
        "xlsx" | "xls" | "ods" => {
            let mut wb = open_workbook_auto(path).map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            for name in wb.sheet_names().to_vec() {
                if let Ok(range) = wb.worksheet_range(&name) {
                    for (r, row) in range.rows().enumerate() {
                        for (c, cell) in row.iter().enumerate() {
                            let text = cell.to_string();
                            if text.trim().is_empty() {
                                continue;
                            }
                            out.push(Segment::new(format!("{} · R{}C{}", name, r + 1, c + 1), text));
                        }
                    }
                }
            }
            Ok(out)
        }
        other if is_text_ext(other) => {
            let bytes = fs::read(path).map_err(|e| e.to_string())?;
            let text = decode_text(bytes)?;
            Ok(vec![Segment::new_lines(text)])
        }
        _ => Err("不支持的文件类型".into()),
    }
}

fn supported(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(ext.as_str(), "pdf" | "docx" | "pptx" | "xlsx" | "xls" | "ods")
        || is_text_ext(ext.as_str())
}

const DEFAULT_IGNORED: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    "dist",
    "build",
    ".cache",
    ".pytest_cache",
    ".mypy_cache",
    ".next",
    ".nuxt",
    "coverage",
];

fn skip_dir(path: &Path, options: &ScanOptions) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    (options.skip_common_dirs && DEFAULT_IGNORED.iter().any(|x| name.eq_ignore_ascii_case(x)))
        || options
            .exclude_dirs
            .iter()
            .any(|x| !x.trim().is_empty() && name.eq_ignore_ascii_case(x.trim()))
}

fn collect(paths: &[String], options: &ScanOptions) -> Vec<PathBuf> {
    let mut all = std::collections::BTreeSet::new();
    for input in paths {
        let p = Path::new(input);
        if p.is_file() {
            all.insert(p.to_path_buf());
        } else if p.is_dir() {
            let entries = WalkDir::new(p)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    !(e.depth() > 0 && e.file_type().is_dir() && skip_dir(e.path(), options))
                });
            for entry in entries.filter_map(Result::ok) {
                if entry.file_type().is_file() && supported(entry.path()) {
                    all.insert(entry.path().to_path_buf());
                }
            }
        }
    }
    all.into_iter().collect()
}

fn inspect(path: &Path, matcher: &Matcher) -> FileResult {
    let name = path.to_string_lossy().into_owned();
    match extract(path) {
        Ok(segments) => matcher.check(name, &segments),
        Err(error) => FileResult {
            path: name,
            status: "unreadable".into(),
            hits: vec![],
            missing: vec![],
            error: Some(error),
        },
    }
}

fn classify_bytes(name: &str, bytes: &[u8]) -> Result<String, String> {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_lowercase();
    if supported(std::path::Path::new(name)) {
        return Ok(ext);
    }
    if bytes.starts_with(b"%PDF-") {
        return Ok("pdf".into());
    }
    if bytes.starts_with(b"PK\x03\x04") {
        let reader = std::io::Cursor::new(bytes);
        if let Ok(mut archive) = zip::ZipArchive::new(reader) {
            if archive.by_name("word/document.xml").is_ok() {
                return Ok("docx".into());
            }
            if archive.by_name("ppt/presentation.xml").is_ok() {
                return Ok("pptx".into());
            }
            if archive.by_name("xl/workbook.xml").is_ok() {
                return Ok("xlsx".into());
            }
        }
        return Err("不支持的 ZIP 文档格式".into());
    }
    if decode_text(bytes.to_vec()).is_ok() {
        return Ok("txt".into());
    }
    Err("无法识别文档格式".into())
}

fn format_csv_string(report: &Report) -> Result<String, String> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record(["文件", "状态", "规则", "关键词", "位置", "原文", "错误"])
        .map_err(|e| e.to_string())?;
    for f in &report.files {
        if f.hits.is_empty() && f.missing.is_empty() {
            writer
                .write_record([
                    f.path.as_str(),
                    f.status.as_str(),
                    "",
                    "",
                    "",
                    "",
                    f.error.as_deref().unwrap_or(""),
                ])
                .map_err(|e| e.to_string())?;
        }
        for h in &f.hits {
            writer
                .write_record([
                    f.path.as_str(),
                    f.status.as_str(),
                    h.kind.as_str(),
                    h.word.as_str(),
                    h.location.as_str(),
                    h.context.as_str(),
                    "",
                ])
                .map_err(|e| e.to_string())?;
        }
        for missing in &f.missing {
            writer
                .write_record([
                    f.path.as_str(),
                    "issue",
                    "required",
                    missing.as_str(),
                    "",
                    "缺失",
                    "",
                ])
                .map_err(|e| e.to_string())?;
        }
    }
    let data = writer.into_inner().map_err(|e| e.to_string())?;
    let body = String::from_utf8(data).map_err(|e| e.to_string())?;
    Ok(format!("\u{FEFF}{body}"))
}

#[cfg(not(standalone_test))]
#[tauri::command]
async fn scan_bytes(
    name: String,
    bytes: Vec<u8>,
    rules: Vec<Rule>,
    options: ScanOptions,
) -> Result<FileResult, String> {
    if bytes.len() > 24 * 1024 * 1024 {
        return Ok(FileResult {
            path: name,
            status: "unreadable".into(),
            hits: vec![],
            missing: vec![],
            error: Some("文件超过 24MB 移动端限制".into()),
        });
    }
    tauri::async_runtime::spawn_blocking(move || {
        let suffix = match classify_bytes(&name, &bytes) {
            Ok(ext) => format!(".{ext}"),
            Err(e) => {
                return FileResult {
                    path: name,
                    status: "unreadable".into(),
                    hits: vec![],
                    missing: vec![],
                    error: Some(e),
                }
            }
        };
        let mut temp = match tempfile::Builder::new()
            .prefix("docinspector-")
            .suffix(&suffix)
            .tempfile()
        {
            Ok(t) => t,
            Err(e) => {
                return FileResult {
                    path: name,
                    status: "unreadable".into(),
                    hits: vec![],
                    missing: vec![],
                    error: Some(e.to_string()),
                }
            }
        };
        if let Err(e) = temp.write_all(&bytes) {
            return FileResult {
                path: name,
                status: "unreadable".into(),
                hits: vec![],
                missing: vec![],
                error: Some(e.to_string()),
            };
        }
        let matcher = match Matcher::new(rules, options) {
            Ok(m) => m,
            Err(e) => {
                return FileResult {
                    path: name,
                    status: "unreadable".into(),
                    hits: vec![],
                    missing: vec![],
                    error: Some(e),
                }
            }
        };
        let mut report = inspect(temp.path(), &matcher);
        report.path = name;
        report
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(not(standalone_test))]
#[tauri::command]
fn cancel_scan(state: tauri::State<'_, ScanState>) {
    state.0.store(true, Ordering::SeqCst);
}

#[cfg(not(standalone_test))]
#[tauri::command]
async fn scan_paths(
    app: tauri::AppHandle,
    state: tauri::State<'_, ScanState>,
    paths: Vec<String>,
    rules: Vec<Rule>,
    options: ScanOptions,
) -> Result<Report, String> {
    state.0.store(false, Ordering::SeqCst);
    let stop = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let started = std::time::Instant::now();
        let matcher = Matcher::new(rules, options.clone())?;
        let files = collect(&paths, &options);
        let total = files.len();
        let completed = AtomicUsize::new(0);
        let threads = std::thread::available_parallelism()
            .map(|v| v.get())
            .unwrap_or(2)
            .clamp(1, 4);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|e| e.to_string())?;
        let reports: Vec<FileResult> = pool.install(|| {
            files
                .par_iter()
                .filter_map(|path| {
                    if stop.load(Ordering::Relaxed) {
                        return None;
                    }
                    let r = inspect(path, &matcher);
                    let count = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    let _ = app.emit(
                        "scan-progress",
                        Progress {
                            scanned: count,
                            total,
                            current: path.to_string_lossy().into_owned(),
                        },
                    );
                    Some(r)
                })
                .collect()
        });
        let scanned = reports.len();
        Ok(Report {
            files: reports,
            total,
            scanned,
            cancelled: stop.load(Ordering::SeqCst),
            duration_ms: started.elapsed().as_millis() as u64,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(not(standalone_test))]
#[tauri::command]
fn open_document(path: String, reveal: bool) -> Result<(), String> {
    let original = Path::new(&path);
    if !original.is_file() {
        return Err("文件不存在或已经移动".into());
    }
    let target = if reveal {
        original.parent().ok_or("无法定位文件夹")?
    } else {
        original
    };
    open::that(target).map_err(|e| e.to_string())
}

#[cfg(not(standalone_test))]
#[tauri::command]
fn export_csv_text(report: Report) -> Result<String, String> {
    format_csv_string(&report)
}

#[cfg(not(standalone_test))]
#[tauri::command]
fn export_csv(path: String, report: Report) -> Result<(), String> {
    let content = format_csv_string(&report)?;
    fs::write(path, content.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(not(standalone_test))]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(ScanState(Arc::new(AtomicBool::new(false))))
        .invoke_handler(tauri::generate_handler![
            scan_paths,
            scan_bytes,
            cancel_scan,
            export_csv,
            export_csv_text,
            open_document,
            history::list_scan_history,
            history::load_scan_history,
            history::save_scan_history,
            history::delete_scan_history
        ])
        .run(tauri::generate_context!())
        .expect("DocInspector startup failed");
}

#[cfg(test)]
mod scan_directory_tests {
    use super::*;

    #[test]
    fn exclusions_skip_cache_and_custom_dirs_but_not_regular_code() {
        let root = tempfile::tempdir().unwrap();
        for name in [".git", "node_modules", "private", "src"] {
            std::fs::create_dir_all(root.path().join(name)).unwrap();
            std::fs::write(root.path().join(name).join("test.rs"), "fn main() {}").unwrap();
        }
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let mut opts = ScanOptions::default();
        let initial = collect(&roots, &opts);
        assert_eq!(initial.len(), 2);
        opts.exclude_dirs.push("private".into());
        let excluded = collect(&roots, &opts);
        assert_eq!(excluded.len(), 1);
        assert!(excluded[0].ends_with(std::path::Path::new("src/test.rs")));
        opts.skip_common_dirs = false;
        let unignored = collect(&roots, &opts);
        assert_eq!(unignored.len(), 3);
    }

    #[test]
    fn office_xml_extracts_paragraphs_and_friendly_labels() {
        let xml = r#"<w:document><w:body><w:p><w:r><w:t>第一段摘要</w:t></w:r></w:p><w:p></w:p><w:p><w:r><w:t>第二段包含 TODO 标记</w:t></w:r></w:p></w:body></w:document>"#.as_bytes();
        let text = xml_text(xml).unwrap();
        let seg = Segment::new_paragraphs("正文".into(), text);
        let matcher = Matcher::new(
            vec![Rule {
                word: "TODO".into(),
                kind: "forbidden".into(),
                enabled: true,
            }],
            ScanOptions::default(),
        )
        .unwrap();
        let res = matcher.check("paper.docx".into(), &[seg]);
        assert_eq!(res.status, "issue");
        assert_eq!(res.hits.len(), 1);
        assert_eq!(res.hits[0].location, "正文 · 第 2 段");
        assert_eq!(
            office_section_label("ppt/slides/slide12.xml"),
            Some((10, 12, "第 12 页幻灯片".into()))
        );
        assert_eq!(
            office_section_label("word/comments.xml"),
            Some((6, 0, "批注".into()))
        );
    }

    #[test]
    fn decodes_utf8_bom_utf16_and_gb18030_text() {
        let utf8_bom = b"\xEF\xBB\xBF\xE4\xB8\xAD\xE6\x96\x87".to_vec();
        assert_eq!(decode_text(utf8_bom).unwrap(), "中文");

        let (gbk_bytes, _, _) = encoding_rs::GB18030.encode("轨控发动机测试");
        assert_eq!(decode_text(gbk_bytes.into_owned()).unwrap(), "轨控发动机测试");
    }

    #[test]
    fn csv_export_includes_bom_and_rows() {
        let report = Report {
            files: vec![FileResult {
                path: "demo.docx".into(),
                status: "issue".into(),
                hits: vec![Hit {
                    word: "TODO".into(),
                    kind: "forbidden".into(),
                    location: "正文 · 第 1 段".into(),
                    context: "此处 TODO 待修改".into(),
                    matched: "TODO".into(),
                }],
                missing: vec!["参考文献".into()],
                error: None,
            }],
            total: 1,
            scanned: 1,
            cancelled: false,
            duration_ms: 15,
        };
        let csv = format_csv_string(&report).unwrap();
        assert!(csv.starts_with('\u{FEFF}'));
        assert!(csv.contains("正文 · 第 1 段"));
        assert!(csv.contains("参考文献"));
    }

    #[test]
    fn inspect_docx_archive_and_classify_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let docx_path = dir.path().join("sample.docx");
        {
            let file = fs::File::create(&docx_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("word/document.xml", opts).unwrap();
            zip.write_all(
                r#"<w:document><w:body><w:p><w:r><w:t>摘要：本文研究轨道机动。</w:t></w:r></w:p><w:p><w:r><w:t>第二段：待补充实验数据。</w:t></w:r></w:p></w:body></w:document>"#
                    .as_bytes(),
            )
            .unwrap();
            zip.finish().unwrap();
        }
        let raw_bytes = fs::read(&docx_path).unwrap();
        assert_eq!(classify_bytes("unknown_file", &raw_bytes).unwrap(), "docx");

        let matcher = Matcher::new(
            vec![
                Rule {
                    word: "待补充".into(),
                    kind: "forbidden".into(),
                    enabled: true,
                },
                Rule {
                    word: "摘要".into(),
                    kind: "required".into(),
                    enabled: true,
                },
            ],
            ScanOptions::default(),
        )
        .unwrap();
        let result = inspect(&docx_path, &matcher);
        assert_eq!(result.status, "issue");
        assert!(result.missing.is_empty());
        assert_eq!(result.hits.len(), 2);
        assert_eq!(result.hits[0].location, "正文 · 第 1 段");
        assert_eq!(result.hits[1].location, "正文 · 第 2 段");
    }
}
