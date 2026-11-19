# DocInspector v0.3.3 · 应用内更新

此版本新增桌面应用的手动更新能力，安装后可在 **设置与迁移 → 关于 DocInspector** 检查新版本。

- **Ubuntu amd64**：检查 GitHub Releases，下载并校验 .deb 后，由系统授权界面执行 dpkg 安装。
- **Windows x64**：检查 GitHub Releases，下载并校验 NSIS 安装器，然后启动安装向导。
- 校验包括固定项目来源、受支持的安装包文件名、GitHub 提供的 SHA-256 摘要、下载大小与 SemVer 版本比较。
- 设置页展示新版本、更新说明、下载进度与失败原因。
- 新版继续由 GitHub Actions 构建 Linux DEB 和 Windows 安装器，并发布 SHA256SUMS.txt。
- Android 仍为独立预览渠道，不启用桌面更新命令。

首次使用：请安装本次 v0.3.3 的完整安装包。以后有新版本时再从「关于」中直接检查更新。

**注意**：更新过程需要联网；Ubuntu 需要 pkexec/polkit 与 dpkg，Windows 需要确认安装向导；尚未配置代码签名，下载校验并不替代发布者身份签名验证。
