use std::path::PathBuf;

/// Prefer the FFmpeg executable bundled with the Windows installer.
/// A non-default user-specified path always takes priority.
pub fn executable(configured: &str) -> PathBuf {
    let custom = configured.trim();
    if !custom.is_empty() && custom != "ffmpeg" && custom != "ffmpeg.exe" {
        return PathBuf::from(custom);
    }
    #[cfg(windows)]
    if let Ok(current) = std::env::current_exe() {
        if let Some(dir) = current.parent() {
            for candidate in [
                dir.join("resources").join("ffmpeg.exe"),
                dir.join("ffmpeg.exe"),
            ] {
                if candidate.is_file() { return candidate; }
            }
        }
    }
    PathBuf::from(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" })
}
