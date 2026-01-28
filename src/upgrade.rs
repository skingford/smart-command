//! Version upgrade functionality
//!
//! Provides automatic version checking and self-upgrade capabilities:
//! - Query GitHub Releases API for new versions
//! - Cache version check results to avoid excessive API calls
//! - Download and install new versions
//! - Cross-platform support (macOS, Linux, Windows)

use crate::config::UpgradeConfig;
use crate::error::UpgradeError;
use crate::output::Output;

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// GitHub release information
#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    #[allow(dead_code)]
    pub name: String,
    pub prerelease: bool,
    #[allow(dead_code)]
    pub published_at: String,
    pub assets: Vec<ReleaseAsset>,
    pub body: String,
}

/// GitHub release asset
#[derive(Debug, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    #[allow(dead_code)]
    pub size: u64,
}

/// Cached version information
#[derive(Debug, Serialize, Deserialize)]
pub struct VersionCache {
    pub latest_version: String,
    pub current_version: String,
    /// Unix timestamp (seconds since epoch)
    pub checked_at: i64,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
}

/// Version update information
#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub version: String,
    pub download_url: String,
    pub release_notes: Option<String>,
    pub asset_name: String,
}

/// Version checker and upgrader
pub struct Upgrader {
    config: UpgradeConfig,
    cache_path: PathBuf,
}

impl Upgrader {
    /// Create new upgrader with configuration
    pub fn new(config: UpgradeConfig) -> Self {
        let cache_path = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("smart-command")
            .join("version-cache.json");

        Self { config, cache_path }
    }

    /// Get current version from Cargo.toml
    pub fn current_version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// Check for available updates
    pub async fn check_for_update(&self) -> Result<Option<VersionInfo>, UpgradeError> {
        // Check cache first
        if let Some(cache) = self.load_cache() {
            if self.is_cache_valid(&cache) {
                // Parse versions and compare
                let current = Version::parse(Self::current_version())
                    .map_err(|e| UpgradeError::Parse(e.to_string()))?;
                let latest = Version::parse(&cache.latest_version)
                    .map_err(|e| UpgradeError::Parse(e.to_string()))?;

                if latest > current {
                    return Ok(Some(VersionInfo {
                        version: cache.latest_version,
                        download_url: cache.download_url.unwrap_or_default(),
                        release_notes: cache.release_notes,
                        asset_name: String::new(),
                    }));
                }
                return Ok(None);
            }
        }

        // Fetch from GitHub
        self.fetch_latest_release().await
    }

    /// Fetch latest release from GitHub API
    async fn fetch_latest_release(&self) -> Result<Option<VersionInfo>, UpgradeError> {
        let client = reqwest::Client::builder()
            .user_agent("smart-command")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| UpgradeError::CheckFailed(e.to_string()))?;

        let url = format!(
            "https://api.github.com/repos/{}/releases/latest",
            self.config.repository
        );

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() || e.is_connect() {
                    UpgradeError::NetworkUnavailable
                } else {
                    UpgradeError::CheckFailed(e.to_string())
                }
            })?;

        if response.status() == 403 {
            return Err(UpgradeError::RateLimited);
        }

        if response.status() == 404 {
            // No releases yet
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(UpgradeError::CheckFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let release: GitHubRelease = response
            .json()
            .await
            .map_err(|e| UpgradeError::Parse(e.to_string()))?;

        // Skip prereleases unless configured
        if release.prerelease && !self.config.include_prerelease {
            return Ok(None);
        }

        // Parse version (remove 'v' prefix if present)
        let version_str = release.tag_name.trim_start_matches('v');
        let latest = Version::parse(version_str)
            .map_err(|e| UpgradeError::Parse(e.to_string()))?;
        let current = Version::parse(Self::current_version())
            .map_err(|e| UpgradeError::Parse(e.to_string()))?;

        // Find matching asset for current platform
        let platform = Self::detect_platform()?;
        let asset = release
            .assets
            .iter()
            .find(|a| a.name.contains(&platform) || Self::asset_matches_platform(&a.name, &platform));

        // Save to cache
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let cache = VersionCache {
            latest_version: version_str.to_string(),
            current_version: Self::current_version().to_string(),
            checked_at: now,
            download_url: asset.map(|a| a.browser_download_url.clone()),
            release_notes: Some(release.body.clone()),
        };
        let _ = self.save_cache(&cache);

        // Return update info if newer
        if latest > current {
            if let Some(asset) = asset {
                return Ok(Some(VersionInfo {
                    version: version_str.to_string(),
                    download_url: asset.browser_download_url.clone(),
                    release_notes: Some(release.body),
                    asset_name: asset.name.clone(),
                }));
            }
        }

        Ok(None)
    }

    /// Check if asset name matches current platform
    fn asset_matches_platform(asset_name: &str, platform: &str) -> bool {
        let name_lower = asset_name.to_lowercase();
        let (arch, os) = platform.split_once('-').unwrap_or(("", ""));

        // Check architecture
        let arch_match = match arch {
            "x86_64" => name_lower.contains("x86_64") || name_lower.contains("amd64") || name_lower.contains("x64"),
            "aarch64" => name_lower.contains("aarch64") || name_lower.contains("arm64"),
            _ => false,
        };

        // Check OS
        let os_match = match os {
            "apple-darwin" => name_lower.contains("darwin") || name_lower.contains("macos") || name_lower.contains("apple"),
            "unknown-linux-gnu" => name_lower.contains("linux"),
            "pc-windows-msvc" => name_lower.contains("windows") || name_lower.contains(".exe"),
            _ => false,
        };

        arch_match && os_match
    }

    /// Detect current platform
    pub fn detect_platform() -> Result<String, UpgradeError> {
        let os = match std::env::consts::OS {
            "macos" => "apple-darwin",
            "linux" => "unknown-linux-gnu",
            "windows" => "pc-windows-msvc",
            other => return Err(UpgradeError::UnsupportedPlatform(other.to_string())),
        };

        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64",
            "aarch64" => "aarch64",
            other => return Err(UpgradeError::UnsupportedPlatform(other.to_string())),
        };

        Ok(format!("{}-{}", arch, os))
    }

    /// Check if cache is still valid
    fn is_cache_valid(&self, cache: &VersionCache) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let elapsed_secs = now - cache.checked_at;
        let interval_secs = (self.config.check_interval_hours * 3600) as i64;
        elapsed_secs < interval_secs
    }

    /// Load cached version info
    fn load_cache(&self) -> Option<VersionCache> {
        let content = fs::read_to_string(&self.cache_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save version info to cache
    fn save_cache(&self, cache: &VersionCache) -> Result<(), UpgradeError> {
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(cache)
            .map_err(|e| UpgradeError::Parse(e.to_string()))?;
        fs::write(&self.cache_path, content)?;
        Ok(())
    }

    /// Download and install update
    pub async fn upgrade(&self, version_info: &VersionInfo) -> Result<(), UpgradeError> {
        Output::info(&format!("正在下载 {}...", version_info.asset_name));

        let client = reqwest::Client::builder()
            .user_agent("smart-command")
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| UpgradeError::DownloadFailed(e.to_string()))?;

        // Download to temp file
        let response = client
            .get(&version_info.download_url)
            .send()
            .await
            .map_err(|e| UpgradeError::DownloadFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(UpgradeError::DownloadFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| UpgradeError::DownloadFailed(e.to_string()))?;

        // Create temp directory
        let tmp_dir = tempfile::tempdir()?;
        let archive_path = tmp_dir.path().join(&version_info.asset_name);

        // Write downloaded file
        let mut file = fs::File::create(&archive_path)?;
        file.write_all(&bytes)?;

        Output::info("正在解压...");

        // Extract binary
        let binary_path = self.extract_binary(&archive_path, tmp_dir.path())?;

        Output::info("正在安装...");

        // Replace current binary
        self.replace_binary(&binary_path)?;

        // Clear cache to force fresh check next time
        let _ = fs::remove_file(&self.cache_path);

        Ok(())
    }

    /// Extract binary from archive
    fn extract_binary(&self, archive_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<PathBuf, UpgradeError> {
        let archive_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if archive_name.ends_with(".tar.gz") || archive_name.ends_with(".tgz") {
            self.extract_tar_gz(archive_path, dest_dir)
        } else {
            // Assume it's a raw binary
            Ok(archive_path.to_path_buf())
        }
    }

    /// Extract .tar.gz archive
    fn extract_tar_gz(&self, archive_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<PathBuf, UpgradeError> {
        use std::io::BufReader;

        let file = fs::File::open(archive_path)?;
        let buf_reader = BufReader::new(file);
        let gz = flate2::read::GzDecoder::new(buf_reader);
        let mut archive = tar::Archive::new(gz);

        archive
            .unpack(dest_dir)
            .map_err(|e| UpgradeError::InstallFailed(e.to_string()))?;

        // Find the binary
        self.find_binary_in_dir(dest_dir)
    }

    /// Find binary file in extracted directory
    fn find_binary_in_dir(&self, dir: &std::path::Path) -> Result<PathBuf, UpgradeError> {
        let binary_name = if cfg!(windows) { "sc.exe" } else { "sc" };

        // Search directory recursively
        fn find_file(dir: &std::path::Path, name: &str) -> Option<PathBuf> {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(found) = find_file(&path, name) {
                            return Some(found);
                        }
                    } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
                        return Some(path);
                    }
                }
            }
            None
        }

        find_file(dir, binary_name).ok_or_else(|| {
            UpgradeError::InstallFailed(format!(
                "在下载的文件中找不到 {} 二进制文件",
                binary_name
            ))
        })
    }

    /// Replace current binary with new one
    fn replace_binary(&self, new_binary: &std::path::Path) -> Result<(), UpgradeError> {
        let current_exe = std::env::current_exe()
            .map_err(|e| UpgradeError::InstallFailed(e.to_string()))?;

        // On Windows, rename current to .old first
        #[cfg(windows)]
        {
            let backup_path = current_exe.with_extension("exe.old");
            let _ = fs::remove_file(&backup_path);
            fs::rename(&current_exe, &backup_path)
                .map_err(|e| UpgradeError::InstallFailed(format!("无法备份当前程序: {}", e)))?;
        }

        // Copy new binary
        fs::copy(new_binary, &current_exe)
            .map_err(|e| UpgradeError::InstallFailed(format!("无法复制新程序: {}", e)))?;

        // Set executable permission (Unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&current_exe, fs::Permissions::from_mode(0o755))
                .map_err(|e| UpgradeError::InstallFailed(format!("无法设置执行权限: {}", e)))?;
        }

        Ok(())
    }

    /// Verify file checksum
    #[allow(dead_code)]
    pub fn verify_checksum(path: &std::path::Path, expected: &str) -> Result<(), UpgradeError> {
        let mut file = fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        let result = hasher.finalize();
        let hash = format!("{:x}", result);

        if hash.eq_ignore_ascii_case(expected) {
            Ok(())
        } else {
            Err(UpgradeError::ChecksumMismatch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = Upgrader::detect_platform();
        assert!(platform.is_ok());
        let platform = platform.unwrap();
        assert!(platform.contains("-"));
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version::parse("0.1.0").unwrap();
        let v2 = Version::parse("0.2.0").unwrap();
        let v3 = Version::parse("0.1.1").unwrap();
        assert!(v2 > v1);
        assert!(v3 > v1);
        assert!(v2 > v3);
    }

    #[test]
    fn test_current_version() {
        let version = Upgrader::current_version();
        assert!(!version.is_empty());
        assert!(Version::parse(version).is_ok());
    }

    #[test]
    fn test_asset_matches_platform() {
        // macOS Intel
        assert!(Upgrader::asset_matches_platform("sc-x86_64-apple-darwin.tar.gz", "x86_64-apple-darwin"));
        assert!(Upgrader::asset_matches_platform("sc-macos-amd64.tar.gz", "x86_64-apple-darwin"));

        // macOS ARM
        assert!(Upgrader::asset_matches_platform("sc-aarch64-apple-darwin.tar.gz", "aarch64-apple-darwin"));
        assert!(Upgrader::asset_matches_platform("sc-macos-arm64.tar.gz", "aarch64-apple-darwin"));

        // Linux
        assert!(Upgrader::asset_matches_platform("sc-x86_64-unknown-linux-gnu.tar.gz", "x86_64-unknown-linux-gnu"));
        assert!(Upgrader::asset_matches_platform("sc-linux-amd64.tar.gz", "x86_64-unknown-linux-gnu"));

        // Windows
        assert!(Upgrader::asset_matches_platform("sc-x86_64-pc-windows-msvc.zip", "x86_64-pc-windows-msvc"));
        assert!(Upgrader::asset_matches_platform("sc-windows-x64.exe", "x86_64-pc-windows-msvc"));
    }
}
