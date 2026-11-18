use futures_util::StreamExt;
use semver::Version;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::Duration};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

pub const RELEASES_URL: &str = "https://github.com/apollo-170/GTA6inTTandYT/releases";
const API_URL: &str = "https://api.github.com/repos/apollo-170/GTA6inTTandYT/releases?per_page=30";
const DOWNLOAD_PREFIX: &str =
    "https://github.com/apollo-170/GTA6inTTandYT/releases/download/";
const MAX_INSTALLER_BYTES: u64 = 250 * 1024 * 1024;

#[derive(Clone, Serialize)]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub available: bool,
    pub can_install: bool,
    pub release_url: String,
    pub download_url: Option<String>,
    pub message: String,
}
#[derive(Clone, Serialize)]
struct UpdateProgress {
    stage: String,
    percent: u8,
}
struct NewRelease {
    version: Version,
    release_url: String,
    download_url: String,
    sha256: Option<String>,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("ClipForge-Updater/0.5 (+https://github.com/apollo-170/GTA6inTTandYT)")
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())
}
fn installer_name(version: &Version) -> String {
    format!("ClipForge_{version}_x64-setup.exe")
}
fn valid_sha256(digest: &str) -> Option<String> {
    let hex = digest.strip_prefix("sha256:")?;
    if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(hex.to_ascii_lowercase())
    } else {
        None
    }
}
async fn releases(client: &reqwest::Client) -> Result<Option<Vec<Value>>, String> {
    let response = client
        .get(API_URL)
        .send()
        .await
        .map_err(|e| format!("Нет соединения с GitHub: {e}"))?;
    match response.status() {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::NOT_FOUND => Ok(None),
        status if !status.is_success() => {
            Err(format!("GitHub Releases API: HTTP {status}"))
        }
        _ => {
            let json: Value = response.json().await.map_err(|e| e.to_string())?;
            let list = json.as_array().ok_or("GitHub вернул некорректные данные")?;
            Ok(Some(list.to_owned()))
        }
    }
}
fn latest_release(items: &[Value]) -> Option<NewRelease> {
    let mut best: Option<NewRelease> = None;
    for item in items {
        if item["draft"] == true {
            continue;
        }
        let Some(tag) = item["tag_name"].as_str() else { continue };
        let Ok(version) = Version::parse(tag.trim_start_matches('v')) else { continue };
        if best.as_ref().is_some_and(|old| old.version >= version) {
            continue;
        }
        let name = installer_name(&version);
        let Some(asset) = item["assets"]
            .as_array()
            .and_then(|assets| assets.iter().find(|asset| asset["name"].as_str() == Some(name.as_str())))
        else {
            continue;
        };
        let link = format!("{DOWNLOAD_PREFIX}{tag}/{name}");
        let sha256 = asset["digest"].as_str().and_then(valid_sha256);
        best = Some(NewRelease {
            version,
            release_url: item["html_url"].as_str().unwrap_or(RELEASES_URL).to_owned(),
            download_url: link,
            sha256,
        });
    }
    best
}
pub async fn check() -> Result<UpdateStatus, String> {
    let current = env!("CARGO_PKG_VERSION").to_owned();
    let mut result = UpdateStatus {
        current_version: current.clone(),
        latest_version: None,
        available: false,
        can_install: false,
        release_url: RELEASES_URL.into(),
        download_url: None,
        message: "Проверка обновлений ещё не выполнялась".into(),
    };
    let Some(items) = releases(&client()?).await? else {
        result.message = "Репозиторий GitHub приватный. Проверка новых версий без публичного канала обновлений недоступна. Открой Releases в браузере, войдя в GitHub.".into();
        return Ok(result);
    };
    let Some(latest) = latest_release(&items) else {
        result.message = "В GitHub Releases пока нет Windows-установщика.".into();
        return Ok(result);
    };
    let current_version = Version::parse(&current).map_err(|e| e.to_string())?;
    result.available = latest.version > current_version;
    result.can_install = result.available && latest.sha256.is_some();
    result.latest_version = Some(latest.version.to_string());
    result.release_url = latest.release_url;
    result.download_url = Some(latest.download_url);
    result.message = if !result.available {
        "ClipForge обновлён до актуальной версии.".into()
    } else if result.can_install {
        "Новая версия готова. Нажми «Обновить» — ClipForge скачает и проверит установщик.".into()
    } else {
        "Есть новая версия, но отсутствует проверочная сумма SHA-256. Открой официальный релиз.".into()
    };
    Ok(result)
}
fn progress(app: &AppHandle, stage: &str, percent: u8) {
    let _ = app.emit("clipforge:update-progress", UpdateProgress {
        stage: stage.into(), percent,
    });
}

/// User-initiated installer launch. Never install a URL passed by the frontend.
/// The release and hash are retrieved independently over HTTPS from the pinned GitHub repository.
pub async fn install(app: AppHandle) -> Result<String, String> {
    #[cfg(not(windows))]
    {
        let _ = app;
        return Err("Установка обновлений поддерживается только в Windows".into());
    }
    #[cfg(windows)]
    {
        let http = client()?;
        let items = releases(&http).await?
            .ok_or("Приватный GitHub не разрешает автоматическую загрузку. Открой Releases в браузере.")?;
        let latest = latest_release(&items).ok_or("Не найден установщик ClipForge")?;
        let current = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?;
        if latest.version <= current {
            return Err("Уже установлена актуальная версия ClipForge".into());
        }
        let expected = latest.sha256.ok_or(
            "GitHub не предоставил SHA-256. Автоматическая установка отменена для безопасности."
        )?;
        if !latest.download_url.starts_with(DOWNLOAD_PREFIX)
            || !latest.download_url.ends_with(&installer_name(&latest.version))
        {
            return Err("Недоверенная ссылка на обновление".into());
        }
        progress(&app, "Подключение к GitHub", 2);
        let response = http.get(&latest.download_url).send().await.map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(format!("Не удалось скачать установщик: HTTP {}. Проверь доступ к релизу.", response.status()));
        }
        let size = response.content_length().unwrap_or(0);
        if size > MAX_INSTALLER_BYTES {
            return Err("Размер обновления превышает допустимый предел".into());
        }
        let directory: PathBuf = std::env::temp_dir().join("ClipForge-Updates");
        tokio::fs::create_dir_all(&directory).await.map_err(|e| e.to_string())?;
        let destination = directory.join(installer_name(&latest.version));
        let tmp = directory.join(format!("{}.download", installer_name(&latest.version)));
        let mut file = tokio::fs::File::create(&tmp).await.map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut stream = response.bytes_stream();
        let mut received: u64 = 0;
        while let Some(block) = stream.next().await {
            let chunk = block.map_err(|e| format!("Ошибка загрузки: {e}"))?;
            received = received.saturating_add(chunk.len() as u64);
            if received > MAX_INSTALLER_BYTES {
                drop(file);
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err("Размер обновления превышает лимит".into());
            }
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
            hash.update(&chunk);
            if size > 0 {
                progress(&app, "Скачивание обновления", 5 + ((received * 80 / size).min(80) as u8));
            }
        }
        file.flush().await.map_err(|e| e.to_string())?;
        drop(file);
        progress(&app, "Проверка SHA-256", 91);
        let downloaded_hash = format!("{:x}", hash.finalize());
        if downloaded_hash != expected {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err("Контрольная сумма не совпадает. Установка отменена.".into());
        }
        if tokio::fs::try_exists(&destination).await.unwrap_or(false) {
            tokio::fs::remove_file(&destination).await.map_err(|e| e.to_string())?;
        }
        tokio::fs::rename(&tmp, &destination).await.map_err(|e| e.to_string())?;
        progress(&app, "Запуск проверенного установщика", 100);
        std::process::Command::new(&destination).spawn()
            .map_err(|e| format!("Windows не удалось запустить установщик: {e}"))?;
        let exit_app = app.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(900)).await;
            exit_app.exit(0);
        });
        Ok(format!("Запущен установщик ClipForge {}", latest.version))
    }
}
