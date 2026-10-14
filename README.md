# DocInspector v0.2

基于 Tauri 2 + Rust 的本地文档和代码关键词检索检查器：Ubuntu、Windows、Android 共用 Rust 核心。

## 工作流程

选择文件或文件夹 → 选择自定义方案 → 扫描 → 查看结果 → CSV 导出。支持禁止词、必需词、检索词，自定义方案可通过 JSON 剪贴板导出和导入。

文件覆盖 PDF、DOCX、PPTX、XLS/XLSX/ODS、TXT、MD、CSV、LOG，以及 Python、Rust、JavaScript、TypeScript、C/C++、Java、Go、SQL、Shell、JSON、TOML、YAML 等常见代码文件。普通代码与文本按行定位。

## GitHub Actions 构建产物

- Desktop Build → linux → DocInspector-Ubuntu-DEB（小体积 .deb，Ubuntu 24.04+）
- Desktop Build → windows → DocInspector-Windows-Installer（Windows 10/11 NSIS .exe）
- Android APK (Preview) → DocInspector-Android-APK-Preview（调试签名 .apk）

Linux 默认仅发布 .deb，避免将约 80MB 的 AppImage 误认为轻量版。Windows 使用系统 WebView2。Android 使用系统 WebView。各平台安装包大小以构建产物为准。

## Android 特别说明

Android 标准对话框不支持文件夹递归选择，首版仅支持系统多文件选择。

Android 系统选中文档通常返回 content:// URI。程序尝试通过 Tauri 文件系统插件读取选中文件，把内容送至 Rust 引擎分析。某些第三方文档提供方无法正确读取时，结果明确标为“未检查”。单文件限制 24MB，避免 JS-Rust 桥接内存压力。

如 URI 不携带扩展名，尝试从 PDF 或 Office ZIP 内部结构识别格式。Android CSV 保存依赖提供方兼容性，失败时尝试复制 CSV 文本到剪贴板。

Android APK 属于预览构建，不是商店签名正式发行版。即使 CI 编译成功，也需在真实手机上验证 Android 文件访问与保存功能。

## 本地构建

需要 Node.js 22+、Rust stable，Ubuntu 还需 WebKitGTK 4.1 与 GTK 相关开发依赖。

- 开发：npm install；npm run tauri dev
- Ubuntu：npm run tauri build -- --bundles deb
- Windows：npm run tauri build -- --bundles nsis
- Android：npm run tauri android init -- --ci；npm run tauri android build -- --apk --debug --ci

## 当前限制

关键词检查仍是区分大小写的文字匹配；OCR、复杂正则、语义检索、索引缓存未实现。复杂 PDF、Office 隐藏对象及扫描件可能无法完整提取。检查失败、提取失败应显示为“未检查”而非“通过”。

源码采用 Tauri 2 + Svelte + Rust，项目由 GPT-6 辅助开发。
