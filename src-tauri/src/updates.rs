use serde::Serialize;
use serde_json::Value;

pub const RELEASES_URL:&str="https://github.com/apollo-170/GTA6inTTandYT/releases";
const API_URL:&str="https://api.github.com/repos/apollo-170/GTA6inTTandYT/releases?per_page=20";

#[derive(Serialize)]
pub struct UpdateStatus {
 pub current_version:String,
 pub latest_version:Option<String>,
 pub available:bool,
 pub release_url:String,
 pub download_url:Option<String>,
 pub message:String
}
pub async fn check()->Result<UpdateStatus,String>{
 let current=env!("CARGO_PKG_VERSION").to_string();
 let mut result=UpdateStatus{
  current_version:current.clone(),latest_version:None,available:false,
  release_url:RELEASES_URL.into(),download_url:None,message:"Последняя версия не проверена".into()
 };
 let response=reqwest::Client::builder().user_agent("ClipForge Updater").timeout(std::time::Duration::from_secs(12))
 .build().map_err(|e|e.to_string())?.get(API_URL).send().await.map_err(|e|format!("Нет подключения к GitHub: {e}"))?;
 if response.status()==reqwest::StatusCode::NOT_FOUND||response.status()==reqwest::StatusCode::UNAUTHORIZED{
  result.message="Репозиторий GitHub приватный: автоматическая проверка недоступна без публичного канала обновлений. Открой Releases в браузере, где ты вошёл в GitHub.".into();
  return Ok(result)
 }
 if !response.status().is_success(){return Err(format!("GitHub Releases: HTTP {}",response.status()))}
 let list:Value=response.json().await.map_err(|e|e.to_string())?;
 let releases=list.as_array().ok_or("GitHub вернул некорректный ответ")?;
 let current_v=semver::Version::parse(&current).map_err(|e|e.to_string())?;
 for item in releases {
  if item["draft"].as_bool()==Some(true){continue}
  let tag=item["tag_name"].as_str().unwrap_or("").trim_start_matches('v');
  let Ok(v)=semver::Version::parse(tag)else{continue};
  let is_newer=v>current_v;
  if result.latest_version.as_ref().and_then(|s|semver::Version::parse(s).ok()).is_some_and(|existing|existing>=v.clone()){continue}
  let installer=item["assets"].as_array().and_then(|files|files.iter().find(|a|
   a["name"].as_str().is_some_and(|s|s.to_ascii_lowercase().ends_with("x64-setup.exe"))
  ));
  result.latest_version=Some(v.to_string());
  result.available=is_newer&&installer.is_some();
  result.release_url=item["html_url"].as_str().unwrap_or(RELEASES_URL).to_string();
  result.download_url=installer.and_then(|a|a["browser_download_url"].as_str()).map(str::to_string);
  result.message=if is_newer{
   if installer.is_some(){"Доступен новый установщик Windows — открой релиз и подтверди установку.".to_owned()}
   else{"Новая версия опубликована, но Windows-установщик ещё не прикреплён.".to_owned()}
  }else{"Установлена актуальная версия.".to_owned()};
 }
 if result.latest_version.is_none(){result.message="В GitHub Releases ещё нет доступных версий.".into()}
 Ok(result)
}
