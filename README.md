# DocInspector v0.3

面向 Ubuntu / Windows / Android（预览）的本地文档与源代码关键词检索及规则检查器，采用 Rust + Tauri 2 + Svelte。

## 操作

1. 左侧只显示检查方案列表，点击选择当前方案。
2. 右侧顶部可选文件夹、多个文件，临时补充检索关键词并开始扫描。
3. 右侧下部显示当前方案扫描历史，点击历史记录可重新查看文件状态、命中位置和上下文。
4. 右上角提供编辑、复制、删除方案；编辑页可直接修改关键词、规则类型、启用状态，支持批量添加。
5. 结果按全部、问题、通过、未检查筛选，可导出 CSV。

方案保存在本机应用 WebView 存储中，扫描历史保存在系统应用数据目录。历史记录最多 30 条，每文件历史证据最多 60 条，超大记录可能进一步裁剪证据，但仍保留文件状态。完整留档应导出 CSV。

旧版最近任务只保存名称和数量，不包含实际扫描结果，无法恢复为可点击的详细记录。新版产生的扫描会记录可查看的结果。

删除方案不会自动删除历史文件，但对应历史不再显示于其他方案下；扫描历史可以单独删除。

## 安装

GitHub Actions 的 Desktop Build 中提供 Ubuntu amd64 的 DocInspector-Ubuntu-DEB 和 Windows x64 的 DocInspector-Windows-Installer。下载、解压后安装。Linux 不再发布体积较大的 AppImage。

Ubuntu 使用系统 WebKitGTK，Windows 使用 WebView2。各系统安装包大小以实际构建产物为准。

## 文件格式

支持 PDF、DOCX、PPTX、XLS、XLSX、ODS、TXT、MD、CSV、LOG、JSON、YAML、TOML，以及 .py / .rs / .js / .ts / .cpp / .h / .java / .go / .sql / .sh 等代码文件。代码按行、文字型 PDF 按页、Excel 按工作表单元格提供位置。

OCR、复杂 Word/PPT 嵌入对象及语义检索尚未支持，无法读取或解析的文件显示“未检查”。

## Android 预览

Android 使用单列布局与系统多文件选择，不支持标准文件对话框的目录递归。部分 content:// 提供方访问仍需要真机验证。预览 APK 位于 Android APK (Preview) 工作流。

## 开发

依赖 Node.js 22+、Rust stable、Tauri 2 对应平台工具链。

本地启动：npm install；npm run tauri dev

Ubuntu 打包：npm run tauri build -- --bundles deb

Windows 打包：npm run tauri build -- --bundles nsis

GitHub Actions: https://github.com/lbg2002/DocInspector/actions

由 GPT-6 协助开发。
