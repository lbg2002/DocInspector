# DocInspector

Rust-powered local document search and inspection for Ubuntu.

**桌面文档与代码全文检索 + 自定义检查方案**。支持递归扫描文件夹，也可以选取单个或多个文件。默认本地处理，不修改输入文件。

## 功能

- 首页管理自定义方案：命名、复制、编辑和删除。
- 搜索词（定位文本）、禁止词（命中即报问题）、必需词（缺失则报问题）。
- 支持选择文件夹递归或多个独立文件；显示扫描进度，允许取消。
- 结果按通过、问题、未检查筛选，展示文本上下文与定位信息。
- 导出 CSV，记录任务历史（不自动储存文档正文）。
- 支持 **Python (.py)、Rust (.rs)、JavaScript/TypeScript、Shell、C/C++、Java、JSON、TOML、YAML、HTML、Markdown、TXT、LOG、CSV** 等 UTF-8 文本格式，给出**行号**。
- 支持 **PDF**（页码）、**DOCX/PPTX**（XML 部件）、**XLSX/XLS/ODS**（工作表与行列）。
- 不支持扫描图片文字 OCR。对于无法解析的文件显示“未检查”，不会冒充检查通过。

## Linux 开发环境

需要 Node.js 22+、Rust stable，以及 Tauri 2 的 Linux WebKitGTK 4.1 依赖。

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev librsvg2-dev libayatana-appindicator3-dev patchelf
npm install
npm run tauri dev
```

构建 .deb 和 AppImage：

```bash
npm run tauri build -- --bundles deb,appimage
```

也可以在 GitHub Actions 的 **Desktop Build** 工作流下载编译产物。

## 当前边界

- 当前按原始字符串、区分大小写精确匹配；逐文件进行规则检查。
- 命中结果每文件最多记录 3000 项；超出部分目前截断。扫描状态不代表文档所有排版元素都可完整解析。
- DOCX 只提取主文档和常见页眉页脚；PPTX 从幻灯片 XML 提取文本；复杂嵌入对象、批注、隐藏内容、图片、加密文档不保证覆盖。
- UTF-8 以外的文本编码暂未自动检测。PDF 复杂排版可能影响提取顺序。
- 预览为提取后的文本上下文，不是原生版式预览。
- 历史任务只记录摘要，不保存完整扫描结果。取消操作在文件边界响应。
- 不需要网络 API，也不引入 AI 服务；在线字体仅是可选的显示字体，会回退到系统字体。

## 版本规划

- v0.1: 可运行桌面基础版，规则方案、文档/代码搜索、结果导出。
- v0.2: 高效索引与增量扫描、更完善的报告与精确高亮、代码范围规则。
- v0.3: 可选 OCR、压缩包扫描、扩展格式支持。

由 GPT-6 辅助开发。欢迎通过 Issues 报告问题或提出改进。
