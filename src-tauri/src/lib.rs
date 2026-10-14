mod history;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use tauri::Emitter;
use walkdir::WalkDir;
use calamine::{open_workbook_auto, Reader};
use quick_xml::{Reader as XmlReader, events::Event};
use std::io::Read;

#[derive(Clone, Deserialize)]
#[serde(rename_all="camelCase")]
struct Rule { word:String, kind:String, enabled:bool }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
struct Hit { word:String, kind:String, location:String, context:String }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
struct FileResult { path:String, status:String, hits:Vec<Hit>, missing:Vec<String>, error:Option<String> }
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
struct Progress { scanned:usize, total:usize, current:String }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
struct Report { files:Vec<FileResult>, total:usize, scanned:usize, cancelled:bool }

struct ScanState(Arc<AtomicBool>);
#[derive(Clone)]
struct Segment { location:String, text:String }

fn xml_text<R: Read>(input:R)->Result<String,String> {
 let mut reader=XmlReader::from_reader(std::io::BufReader::new(input));
 let mut buf=Vec::new(); let mut out=String::new();
 loop {
  match reader.read_event_into(&mut buf) {
   Ok(Event::Text(t)) => { out.push_str(&t.unescape().map_err(|e|e.to_string())?); }
   Ok(Event::End(e)) if matches!(e.name().as_ref(), b"w:p"|b"a:p") => out.push('\n'),
   Ok(Event::Eof)=>break,
   Err(e)=>return Err(e.to_string()), _=>{}
  }
  buf.clear();
 }
 Ok(out)
}
fn unzip_xml(path:&Path)->Result<Vec<Segment>,String> {
 let file=fs::File::open(path).map_err(|e|e.to_string())?;
 let mut archive=zip::ZipArchive::new(file).map_err(|e|e.to_string())?;
 let mut segments=Vec::new();
 for index in 0..archive.len() {
  let mut entry=archive.by_index(index).map_err(|e|e.to_string())?;
  let name=entry.name().to_string();
  if !(name=="word/document.xml" || name.starts_with("word/header") || name.starts_with("word/footer") || (name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))) {continue}
  let mut data=Vec::new(); entry.read_to_end(&mut data).map_err(|e|e.to_string())?;
  let text=xml_text(data.as_slice())?;
  segments.push(Segment{location:name,text});
 }
 if segments.is_empty() {return Err("文档中没有可解析的正文".into())}
 Ok(segments)
}
fn extract(path:&Path)->Result<Vec<Segment>,String> {
 let ext=path.extension().and_then(|s|s.to_str()).unwrap_or("").to_lowercase();
 match ext.as_str() {
  "pdf" => {
   let pages=pdf_extract::extract_text_by_pages(path).map_err(|e|e.to_string())?;
   Ok(pages.into_iter().enumerate().map(|(i,text)| Segment{location:format!("第 {} 页",i+1),text}).collect())
  }
  "docx"|"pptx" => unzip_xml(path),
  "xlsx"|"xls"|"ods" => {
   let mut wb=open_workbook_auto(path).map_err(|e|e.to_string())?;
   let mut out=Vec::new();
   for name in wb.sheet_names().to_vec() {
    if let Ok(range)=wb.worksheet_range(&name) {
     for (r,row) in range.rows().enumerate() {
      for (c,cell) in row.iter().enumerate() {
       let text=cell.to_string(); if text.trim().is_empty(){continue}
       out.push(Segment{location:format!("{} · R{}C{}",name,r+1,c+1),text});
      }
     }
    }
   }
   Ok(out)
  }
  "txt"|"md"|"csv"|"log"|"json"|"xml"|"html"|"htm"|"rs"|"py"|"js"|"ts"|"tsx"|"jsx"|"c"|"cpp"|"h"|"java"|"toml"|"yaml"|"yml"|"ini"|"sh"|"cc"|"hpp"|"go"|"sql"|"css"|"scss"|"bash"|"zsh"|"kt"|"vue"|"svelte"|"php"|"rb"|"tex"|"bib"|"ipynb"|"mjs"|"cjs"|"jsonl" => {
   let bytes=fs::read(path).map_err(|e|e.to_string())?;
   let text=String::from_utf8(bytes).map_err(|_|"文本不是 UTF-8 编码".to_string())?;
   Ok(text.lines().enumerate().map(|(i,s)| Segment{location:format!("第 {} 行",i+1),text:s.to_owned()}).collect())
  }
  _=>Err("不支持的文件类型".into())
 }
}
fn supported(path:&Path)->bool {
 let ext=path.extension().and_then(|x|x.to_str()).unwrap_or("").to_lowercase();
 matches!(ext.as_str(),"pdf"|"docx"|"pptx"|"xlsx"|"xls"|"ods"|"txt"|"md"|"csv"|"log"|"json"|"xml"|"html"|"htm"|"rs"|"py"|"js"|"ts"|"tsx"|"jsx"|"c"|"cpp"|"h"|"java"|"toml"|"yaml"|"yml"|"ini"|"sh"|"cc"|"hpp"|"go"|"sql"|"css"|"scss"|"bash"|"zsh"|"kt"|"vue"|"svelte"|"php"|"rb"|"tex"|"bib"|"ipynb"|"mjs"|"cjs"|"jsonl")
}
fn collect(paths:&[String])->Vec<PathBuf> {
 let mut all=std::collections::BTreeSet::new();
 for input in paths {
  let p=Path::new(input);
  if p.is_file() { all.insert(p.to_path_buf()); }
  else if p.is_dir() {
   for e in WalkDir::new(p).follow_links(false).into_iter().filter_map(Result::ok) {
    if e.file_type().is_file() && supported(e.path()) {all.insert(e.path().to_path_buf());}
   }
  }
 }
 all.into_iter().collect()
}
fn inspect(path:&Path,rules:&[Rule])->FileResult {
 let name=path.to_string_lossy().to_string();
 match extract(path) {
  Err(error)=> FileResult{path:name,status:"unreadable".into(),hits:vec![],missing:vec![],error:Some(error)},
  Ok(segments)=>{
   let mut hits=Vec::new();
   let mut missing=Vec::new();
   let mut has_forbidden=false;
   for r in rules.iter().filter(|r|r.enabled && !r.word.trim().is_empty()) {
    let mut found=false;
    for s in &segments {
     for (idx,_) in s.text.match_indices(&r.word) {
      found=true;
      if r.kind=="forbidden" {has_forbidden=true;}
      if hits.len()>=3000 {continue}
      let mut from=idx.saturating_sub(64);
      while !s.text.is_char_boundary(from) {from+=1}
      let mut to=(idx+r.word.len()+64).min(s.text.len());
      while !s.text.is_char_boundary(to){to-=1}
      hits.push(Hit{word:r.word.clone(),kind:r.kind.clone(),location:s.location.clone(),context:s.text[from..to].to_owned()});
     }
    }
    if r.kind=="required" && !found {missing.push(r.word.clone());}
   }
   let issue=!missing.is_empty() || has_forbidden;
   FileResult{path:name,status:if issue{"issue"}else{"passed"}.into(),hits,missing,error:None}
  }
 }
}
fn classify_bytes(name: &str, bytes: &[u8]) -> Result<String,String> {
 let ext = std::path::Path::new(name).extension().and_then(|x|x.to_str()).unwrap_or("").to_lowercase();
 if supported(std::path::Path::new(name)) { return Ok(ext) }
 if bytes.starts_with(b"%PDF-") { return Ok("pdf".into()) }
 if bytes.starts_with(b"PK\x03\x04") {
  let reader=std::io::Cursor::new(bytes);
  if let Ok(mut archive)=zip::ZipArchive::new(reader) {
   if archive.by_name("word/document.xml").is_ok() { return Ok("docx".into()) }
   if archive.by_name("ppt/presentation.xml").is_ok() { return Ok("pptx".into()) }
   if archive.by_name("xl/workbook.xml").is_ok() { return Ok("xlsx".into()) }
  }
  return Err("不支持的 ZIP 文档格式".into())
 }
 if std::str::from_utf8(bytes).is_ok() { return Ok("txt".into()) }
 Err("无法识别文档格式".into())
}

#[tauri::command]
async fn scan_bytes(name: String, bytes: Vec<u8>, rules: Vec<Rule>) -> Result<FileResult,String> {
 if bytes.len()>24*1024*1024 {return Ok(FileResult{path:name,status:"unreadable".into(),hits:vec![],missing:vec![],error:Some("文件超过 24MB 移动端限制".into())})}
 tauri::async_runtime::spawn_blocking(move || {
  let suffix=match classify_bytes(&name,&bytes) {
   Ok(ext)=>format!(".{ext}"),
   Err(e)=>return FileResult{path:name,status:"unreadable".into(),hits:vec![],missing:vec![],error:Some(e)}
  };
  let mut temp=match tempfile::Builder::new().prefix("docinspector-").suffix(&suffix).tempfile() {
   Ok(t)=>t,
   Err(e)=>return FileResult{path:name,status:"unreadable".into(),hits:vec![],missing:vec![],error:Some(e.to_string())}
  };
  if let Err(e)=temp.write_all(&bytes) {
   return FileResult{path:name,status:"unreadable".into(),hits:vec![],missing:vec![],error:Some(e.to_string())}
  }
  let mut report=inspect(temp.path(),&rules);
  report.path=name;
  report
 }).await.map_err(|e|e.to_string())
}

#[tauri::command]
fn cancel_scan(state:tauri::State<'_,ScanState>){state.0.store(true,Ordering::SeqCst);}
#[tauri::command]
async fn scan_paths(app:tauri::AppHandle, state:tauri::State<'_,ScanState>, paths:Vec<String>, rules:Vec<Rule>)->Result<Report,String>{
 state.0.store(false,Ordering::SeqCst);
 let stop=state.0.clone();
 tauri::async_runtime::spawn_blocking(move ||{
  let paths=collect(&paths); let total=paths.len();
  let mut files=Vec::new();
  for path in paths {
   if stop.load(Ordering::SeqCst) {break}
   let r=inspect(&path,&rules); files.push(r);
   let _=app.emit("scan-progress",Progress{scanned:files.len(),total,current:path.to_string_lossy().into_owned()});
  }
  let scanned=files.len();
  Report{files,total,scanned,cancelled:stop.load(Ordering::SeqCst)}
 }).await.map_err(|e|e.to_string())
}
#[tauri::command]
fn export_csv_text(report:Report)->Result<String,String> {
 let mut writer=csv::Writer::from_writer(Vec::new());
 writer.write_record(["文件","状态","规则","关键词","位置","原文","错误"]).map_err(|e|e.to_string())?;
 for f in report.files {
  if f.hits.is_empty() && f.missing.is_empty() {writer.write_record([f.path.as_str(),f.status.as_str(),"","","","",f.error.as_deref().unwrap_or("")]).map_err(|e|e.to_string())?;}
  for h in f.hits { writer.write_record([f.path.as_str(),f.status.as_str(),h.kind.as_str(),h.word.as_str(),h.location.as_str(),h.context.as_str(),""]).map_err(|e|e.to_string())?; }
  for missing in f.missing {writer.write_record([f.path.as_str(),"issue","required",missing.as_str(),"","缺失",""]).map_err(|e|e.to_string())?;}
 }
 let data=writer.into_inner().map_err(|e|e.to_string())?;
 String::from_utf8(data).map_err(|e|e.to_string())
}

#[tauri::command]
fn export_csv(path:String, report:Report)->Result<(),String> {
 let mut w=csv::Writer::from_path(path).map_err(|e|e.to_string())?;
 w.write_record(["文件","状态","规则","关键词","位置","原文","错误"]).map_err(|e|e.to_string())?;
 for f in report.files {
  if f.hits.is_empty() && f.missing.is_empty() {w.write_record([f.path.as_str(),f.status.as_str(),"","","","",f.error.as_deref().unwrap_or("")]).map_err(|e|e.to_string())?;}
  for h in f.hits { w.write_record([f.path.as_str(),f.status.as_str(),h.kind.as_str(),h.word.as_str(),h.location.as_str(),h.context.as_str(),""]).map_err(|e|e.to_string())?; }
  for missing in f.missing {w.write_record([f.path.as_str(),"issue","required",missing.as_str(),"","缺失",""]).map_err(|e|e.to_string())?;}
 }
 w.flush().map_err(|e|e.to_string())
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(){
 tauri::Builder::default()
 .plugin(tauri_plugin_dialog::init())
 .plugin(tauri_plugin_fs::init())
 .manage(ScanState(Arc::new(AtomicBool::new(false))))
 .invoke_handler(tauri::generate_handler![scan_paths,scan_bytes,cancel_scan,export_csv,export_csv_text,
  history::list_scan_history,history::load_scan_history,history::save_scan_history,history::delete_scan_history])
 .run(tauri::generate_context!()).expect("DocInspector startup failed");
}
