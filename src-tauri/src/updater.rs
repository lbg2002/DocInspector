//! Desktop updater backed by this repository's GitHub Releases.
//! Only the fixed project and expected platform installer names are accepted.
//! GitHub's asset SHA-256 digest is verified before execution.
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub notes: String,
    pub release_url: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    total: u64,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    desktop::check_for_update(&app).await
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
#[tauri::command]
pub async fn check_for_update() -> Result<UpdateInfo, String> {
    Err("应用内更新仅支持 Ubuntu amd64 和 Windows x64 正式安装版".into())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<String, String> {
    desktop::install_update(&app).await
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
#[tauri::command]
pub async fn install_update() -> Result<String, String> {
    Err("当前平台不支持应用内安装更新".into())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
mod desktop {
    use super::{UpdateInfo, UpdateProgress};
    use reqwest::{Client, Url};
    use semver::Version;
    use serde::Deserialize;
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self},
        io::Write,
        path::PathBuf,
        time::Duration,
    };
    use tauri::{Emitter, Manager};

    const RELEASE_API: &str =
        "https://api.github.com/repos/lbg2002/DocInspector/releases/latest";
    const MAX_INSTALLER_SIZE: u64 = 100 * 1024 * 1024;

    #[derive(Deserialize)]
    struct GithubRelease {
        tag_name: String,
        #[serde(default)]
        body: Option<String>,
        draft: bool,
        prerelease: bool,
        assets: Vec<GithubAsset>,
    }

    #[derive(Deserialize)]
    struct GithubAsset {
        name: String,
        browser_download_url: String,
        size: u64,
        digest: Option<String>,
    }

    struct SelectedUpdate {
        version: String,
        notes: String,
        release_url: String,
        download_url: String,
        expected_digest: String,
        size: u64,
        filename: String,
    }

    fn valid_host(url: &Url) -> bool {
        if url.scheme() != "https" {
            return false;
        }
        match url.host_str() {
            Some("api.github.com" | "github.com") => true,
            Some(host) if host.ends_with(".githubusercontent.com") => true,
            _ => false,
        }
    }

    fn client() -> Result<Client, String> {
        Client::builder()
            .user_agent("DocInspector-Desktop-Updater")
            .timeout(Duration::from_secs(300))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 || !valid_host(attempt.url()) {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .map_err(|e| format!("无法初始化安全下载连接：{e}"))
    }

    fn digest_hex(value: Option<&str>) -> Result<String, String> {
        let value = value
            .and_then(|s| s.strip_prefix("sha256:"))
            .ok_or("发行包未提供 GitHub SHA-256 校验摘要，已拒绝安装")?;
        if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("发行包 SHA-256 摘要格式不正确".into());
        }
        Ok(value.to_ascii_lowercase())
    }

    fn expected_filename(version: &Version) -> Result<String, String> {
        if !cfg!(target_arch = "x86_64") {
            return Err("当前 CPU 架构尚未提供可更新的桌面安装包".into());
        }
        if cfg!(target_os = "windows") {
            Ok(format!("DocInspector_{version}_x64-setup.exe"))
        } else {
            Ok(format!("DocInspector_{version}_amd64.deb"))
        }
    }

    fn select(release: GithubRelease, current: &str) -> Result<Option<SelectedUpdate>, String> {
        if release.draft || release.prerelease {
            return Err("最新 GitHub Release 不是正式稳定版本".into());
        }
        let version = Version::parse(release.tag_name.trim_start_matches('v'))
            .map_err(|_| "最新发行版版本号不符合 SemVer 格式")?;
        let installed = Version::parse(current)
            .map_err(|_| "当前安装的应用版本号不符合 SemVer 格式")?;
        if version <= installed {
            return Ok(None);
        }
        let filename = expected_filename(&version)?;
        let expected_url = format!(
            "https://github.com/lbg2002/DocInspector/releases/download/{}/{}",
            release.tag_name, filename
        );
        let asset = release
            .assets
            .into_iter()
            .find(|asset| asset.name == filename)
            .ok_or("新版已发布，但尚未提供适用于当前系统的安装包")?;
        if asset.browser_download_url != expected_url {
            return Err("发行包下载地址未通过项目来源校验".into());
        }
        if asset.size == 0 || asset.size > MAX_INSTALLER_SIZE {
            return Err("发行包大小不在允许范围内".into());
        }
        let expected_digest = digest_hex(asset.digest.as_deref())?;
        Ok(Some(SelectedUpdate {
            version: version.to_string(),
            notes: release.body.unwrap_or_default().chars().take(2500).collect(),
            release_url: format!(
                "https://github.com/lbg2002/DocInspector/releases/tag/{}",
                release.tag_name
            ),
            download_url: expected_url,
            expected_digest,
            size: asset.size,
            filename,
        }))
    }

    async fn fetch_release(client: &Client) -> Result<GithubRelease, String> {
        let response = client
            .get(RELEASE_API)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| format!("无法连接 GitHub Releases：{e}"))?;
        let response = response
            .error_for_status()
            .map_err(|e| format!("GitHub Releases 请求失败：{e}"))?;
        response
            .json::<GithubRelease>()
            .await
            .map_err(|e| format!("解析最新发行版信息失败：{e}"))
    }

    async fn latest(app: &tauri::AppHandle, client: &Client) -> Result<Option<SelectedUpdate>, String> {
        let current = app.package_info().version.to_string();
        select(fetch_release(client).await?, &current)
    }

    pub(super) async fn check_for_update(
        app: &tauri::AppHandle,
    ) -> Result<UpdateInfo, String> {
        let client = client()?;
        let current = app.package_info().version.to_string();
        let selected = latest(app, &client).await?;
        Ok(match selected {
            Some(update) => UpdateInfo {
                available: true,
                current_version: current,
                latest_version: update.version,
                notes: update.notes,
                release_url: update.release_url,
            },
            None => UpdateInfo {
                available: false,
                current_version: current.clone(),
                latest_version: current,
                notes: String::new(),
                release_url: "https://github.com/lbg2002/DocInspector/releases/latest".into(),
            },
        })
    }

    async fn download(
        app: &tauri::AppHandle,
        client: &Client,
        update: &SelectedUpdate,
    ) -> Result<PathBuf, String> {
        let target_dir = app
            .path()
            .app_cache_dir()
            .map_err(|e| format!("无法获取应用缓存目录：{e}"))?
            .join("updates");
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("无法创建更新缓存目录：{e}"))?;

        let response = client
            .get(&update.download_url)
            .send()
            .await
            .map_err(|e| format!("下载安装包失败：{e}"))?
            .error_for_status()
            .map_err(|e| format!("安装包下载请求失败：{e}"))?;
        if !valid_host(response.url()) {
            return Err("下载发生了不可信的网络跳转".into());
        }
        if response.content_length().is_some_and(|size| size > MAX_INSTALLER_SIZE) {
            return Err("下载安装包超过允许大小".into());
        }

        let mut temporary = tempfile::Builder::new()
            .prefix(".docinspector-update-")
            .tempfile_in(&target_dir)
            .map_err(|e| format!("无法创建临时安装包：{e}"))?;
        let mut response = response;
        let mut downloaded = 0_u64;
        let mut hasher = Sha256::new();
        let _ = app.emit(
            "updater-progress",
            UpdateProgress { downloaded: 0, total: update.size },
        );

        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("下载安装包中断：{e}"))?
        {
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            if downloaded > update.size || downloaded > MAX_INSTALLER_SIZE {
                return Err("下载字节数超出 GitHub 声明大小，已取消更新".into());
            }
            temporary
                .write_all(&chunk)
                .map_err(|e| format!("写入安装包失败：{e}"))?;
            hasher.update(&chunk);
            let _ = app.emit(
                "updater-progress",
                UpdateProgress { downloaded, total: update.size },
            );
        }
        if downloaded != update.size {
            return Err("安装包下载不完整，已拒绝安装".into());
        }
        let actual = format!("{:x}", hasher.finalize());
        if actual != update.expected_digest {
            return Err("安装包 SHA-256 校验失败，已拒绝安装".into());
        }
        temporary
            .as_file_mut()
            .sync_all()
            .map_err(|e| format!("无法完成安装包落盘：{e}"))?;

        let path = target_dir.join(&update.filename);
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("无法清理旧的更新缓存：{e}"))?;
        }
        temporary
            .persist(&path)
            .map_err(|e| format!("保存已校验安装包失败：{e}"))?;
        Ok(path)
    }

    #[cfg(target_os = "linux")]
    async fn launch_installer(path: PathBuf) -> Result<String, String> {
        if !std::path::Path::new("/usr/bin/pkexec").is_file() || !std::path::Path::new("/usr/bin/dpkg").is_file() {
            return Err(format!(
                "系统缺少 pkexec 或 dpkg。已校验安装包保存在 {}，可手动安装",
                path.display()
            ));
        }
        let saved_path = path.clone();
        let status = tauri::async_runtime::spawn_blocking(move || {
            std::process::Command::new("/usr/bin/pkexec")
                .arg("/usr/bin/dpkg")
                .arg("-i")
                .arg(path)
                .status()
        })
        .await
        .map_err(|e| format!("无法获取安装程序状态：{e}"))?
        .map_err(|e| format!("无法启动系统授权安装：{e}"))?;
        if !status.success() {
            return Err(format!(
                "系统安装未完成（状态 {status}）。已下载的安装包：{}",
                saved_path.display()
            ));
        }
        Ok("新版本已安装，正在准备重启".into())
    }

    #[cfg(target_os = "windows")]
    async fn launch_installer(path: PathBuf) -> Result<String, String> {
        std::process::Command::new(&path)
            .spawn()
            .map_err(|e| format!("无法启动 Windows 安装程序（{}）：{e}", path.display()))?;
        Ok("已启动安装程序，请按向导完成更新".into())
    }

    pub(super) async fn install_update(app: &tauri::AppHandle) -> Result<String, String> {
        let client = client()?;
        // Re-fetch the release; never trust a URL, version or filesystem path supplied by JS.
        let update = latest(app, &client)
            .await?
            .ok_or("当前已是最新版本，无需安装")?;
        let path = download(app, &client, &update).await?;
        let result = launch_installer(path).await?;
        #[cfg(target_os = "windows")]
        app.exit(0);
        Ok(result)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn release(tag: &str, digest: Option<&str>, url: &str) -> GithubRelease {
            GithubRelease {
                tag_name: tag.into(),
                body: Some("更新说明".into()),
                draft: false,
                prerelease: false,
                assets: vec![GithubAsset {
                    name: expected_filename(&Version::parse("0.3.3").unwrap()).unwrap(),
                    browser_download_url: url.into(),
                    size: 6_000_000,
                    digest: digest.map(str::to_owned),
                }],
            }
        }

        #[test]
        fn version_order_only_allows_newer_release() {
            let url = format!(
                "https://github.com/lbg2002/DocInspector/releases/download/v0.3.3/{}",
                expected_filename(&Version::parse("0.3.3").unwrap()).unwrap()
            );
            assert!(select(release("v0.3.3", Some(&format!("sha256:{}", "a".repeat(64))), &url), "0.3.2").unwrap().is_some());
            assert!(select(release("v0.3.3", Some(&format!("sha256:{}", "a".repeat(64))), &url), "0.3.3").unwrap().is_none());
            assert!(select(release("v0.3.3", Some(&format!("sha256:{}", "a".repeat(64))), &url), "0.4.0").unwrap().is_none());
        }

        #[test]
        fn rejects_untrusted_release_assets() {
            let hash = format!("sha256:{}", "f".repeat(64));
            let wrong_url = "https://evil.example/download/DocInspector_0.3.3_amd64.deb";
            assert!(select(release("v0.3.3", Some(&hash), wrong_url), "0.3.2").is_err());
            let url = format!(
                "https://github.com/lbg2002/DocInspector/releases/download/v0.3.3/{}",
                expected_filename(&Version::parse("0.3.3").unwrap()).unwrap()
            );
            assert!(select(release("v0.3.3", None, &url), "0.3.2").is_err());
            assert!(select(release("v0.3.3", Some("sha256:bad"), &url), "0.3.2").is_err());
            assert!(!valid_host(&Url::parse("http://github.com/example").unwrap()));
            assert!(!valid_host(&Url::parse("https://github.com.evil.example").unwrap()));
        }
    }
}
