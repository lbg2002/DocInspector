# DocInspector v0.3.1

轻量离线文档与代码关键词检索工具。支持 Ubuntu、Windows，Android 为预览版。Rust 搜索核心、Tauri 2 + Svelte 界面。

## 操作

左侧选择检查方案，右侧选择文件夹或文件，扫描并在历史记录中重新查看结果。右上角可编辑、复制、删除方案，原文件始终只读。

“扫描选项”默认折叠，可按方案设置：
- 区分大小写：默认开启。关闭时忽略 ASCII 英文大小写，尚未实现完整 Unicode 大小写折叠。
- 完整单词匹配：默认关闭。Unicode 字母、数字和下划线用于判断单词边界。
- 跳过代码缓存目录：默认开启，自动排除 .git、node_modules、target、.venv、venv、__pycache__、dist、build、.cache 等文件夹。
- 自定义排除目录：按目录名填写，逗号分隔。

## v0.3.1 更新

- 使用 Rust Aho–Corasick 单次匹配多关键词，包括重叠词。
- 最多 4 个工作线程并行扫描，支持取消。
- 支持文本和代码跨行关键词匹配，保留起始行号。
- 结果搜索文件名、路径、关键词、上下文和缺失关键词。
- 结果分栏可拖动并保存宽度，文件列表支持双击打开、右键操作。
- 扫描历史显示用时。
- 新增 Rust 关键词准确性、跨行、命中数量上限与性能冒烟测试，并加入 GitHub Actions。
- Ubuntu 继续仅发布小体积 deb，不发布大型 AppImage。

## 文件格式

支持 PDF、DOCX、PPTX、XLS/XLSX/ODS、TXT、MD、CSV、LOG、JSON、TOML、YAML，以及 Python、Rust、JS/TS、C/C++、Go、Java、SQL、Shell、TeX 等代码文件。复杂 Office 嵌入对象、扫描版 PDF 不保证完全读取，暂不支持 OCR。

## 下载

在 https://github.com/lbg2002/DocInspector/actions 找最新成功的 Desktop Build：
- Ubuntu：DocInspector-Ubuntu-DEB，可解压 deb 并安装。
- Windows：DocInspector-Windows-Installer，解压 NSIS exe 安装器。
Android：Android APK (Preview) 调试预览，暂不支持目录递归，部分文件提供方访问需要真机验证。

## 开发

需要 Node.js 22+、Rust stable、Tauri 2 对应系统依赖。
- npm install
- npm run tauri dev
- cargo test --manifest-path src-tauri/Cargo.toml --lib
- npm run tauri build -- --bundles deb

配置保存在本地，历史证据有数量限制，完整归档请在当次扫描后导出 CSV。由 GPT-6 协助开发。
