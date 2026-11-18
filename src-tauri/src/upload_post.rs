//! Upload-Post hosted publishing: no ClipForge OAuth server needed.
//! Credentials stay in the Windows Credential Manager, never in app preferences or release files.
use reqwest::{multipart::{Form,Part},Client};
use serde::Serialize;
use serde_json::{json,Value};
use std::{path::Path,time::Duration};
use crate::social::AccountView;

const SERVICE:&str="ClipForge.UploadPost";
const USER:&str="desktop-api-key";
const BASE:&str="https://api.upload-post.com";
const MAX_UPLOAD:u64=500*1024*1024;

#[derive(Clone,Serialize)]
pub struct HostedState{pub connected:bool,pub provider:String,pub error:Option<String>}
#[derive(Clone)]
pub struct UploadResult{pub state:String,pub detail:String}
fn entry()->Result<keyring::Entry,String>{
 keyring::Entry::new(SERVICE,USER).map_err(|_|"Не удалось открыть хранилище Windows Credential Manager".into())
}
fn get_key()->Result<String,String>{
 let key=entry()?.get_password().map_err(|_|"В настройках публикации подключи API-ключ Upload-Post. Он сохраняется в Windows Credential Manager.".to_string())?;
 if key.trim().len()<10{return Err("Не настроен API-ключ Upload-Post".into())}
 Ok(key)
}
pub async fn configured()->bool{get_key().is_ok()}
pub async fn save_key(value:String)->Result<(),String>{
 let value=value.trim().to_owned();
 if value.len()<12||value.len()>300||value.chars().any(char::is_whitespace){return Err("Проверь API-ключ Upload-Post".into())}
 tokio::task::spawn_blocking(move||entry()?.set_password(&value).map_err(|_|"Windows не сохранил ключ".to_string()))
  .await.map_err(|e|e.to_string())?
}
pub async fn clear_key()->Result<(),String>{
 tokio::task::spawn_blocking(||entry()?.delete_credential().map_err(|e|format!("Не удалось удалить ключ из Windows: {e}")))
  .await.map_err(|e|e.to_string())?
}
fn http()->Result<Client,String>{
 Client::builder().timeout(Duration::from_secs(240)).user_agent("ClipForge/0.7 hosted publishing").build().map_err(|e|e.to_string())
}
async fn value(r:reqwest::Response)->Result<Value,String>{
 let status=r.status();
 let text=r.text().await.map_err(|e|e.to_string())?;
 let data:Value=serde_json::from_str(&text).map_err(|_|format!("Upload-Post: некорректный ответ HTTP {status}"))?;
 if !status.is_success(){return Err(format!("Upload-Post HTTP {status}: {}",data["message"].as_str().or(data["error"].as_str()).unwrap_or("проверь тариф, API-ключ и подключённые аккаунты")))}
 Ok(data)
}
fn profile_valid(s:&str)->bool{s.len()>0&&s.len()<=100&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'||b==b'-')}
pub fn target_parts(id:&str)->Result<(&str,&str),String>{
 let mut pieces=id.split(':');
 let (Some("up"),Some(profile),Some(platform),None)=(pieces.next(),pieces.next(),pieces.next(),pieces.next())else{return Err("Неизвестный профиль публикации".into())};
 if !profile_valid(profile)||!["youtube","tiktok"].contains(&platform){return Err("Некорректный аккаунт Upload-Post".into())}
 Ok((profile,platform))
}
pub async fn accounts()->Result<Vec<AccountView>,String>{
 let key=get_key()?;
 let client=http()?;
 let payload=value(client.get(format!("{BASE}/api/uploadposts/users"))
  .header("Authorization",format!("Apikey {key}")).send().await.map_err(|e|e.to_string())?).await?;
 let mut output=Vec::new();
 if let Some(profiles)=payload["profiles"].as_array(){
  for profile in profiles{
   let Some(user)=profile["username"].as_str().filter(|s|profile_valid(s))else{continue};
   let social=&profile["social_accounts"];
   for platform in ["tiktok","youtube"]{
    let linked=&social[platform];
    if !linked.is_object(){continue}
    let name=linked["display_name"].as_str().or(linked["username"].as_str())
     .filter(|s|!s.is_empty()).unwrap_or(user);
    output.push(AccountView{
      id:format!("up:{user}:{platform}"),
      name:format!("{name} · Upload-Post ({user})"),
      platform:platform.to_string()
    });
   }
  }
 }
 Ok(output)
}
pub async fn connect_link(profile:&str)->Result<String,String>{
 if !profile_valid(profile){return Err("Имя профиля: латинские буквы, цифры, _ и -, до 100 знаков".into())}
 let key=get_key()?;
 let client=http()?;
 let check=value(client.get(format!("{BASE}/api/uploadposts/users"))
 .header("Authorization",format!("Apikey {key}")).send().await.map_err(|e|e.to_string())?).await?;
 let exists=check["profiles"].as_array().is_some_and(|p|p.iter().any(|u|u["username"].as_str()==Some(profile)));
 if !exists{
  let response=client.post(format!("{BASE}/api/uploadposts/users"))
   .header("Authorization",format!("Apikey {key}")).json(&json!({"username":profile}))
   .send().await.map_err(|e|e.to_string())?;
  if response.status()!=reqwest::StatusCode::CONFLICT {value(response).await?;}
 }
 let info=value(client.post(format!("{BASE}/api/uploadposts/users/generate-jwt"))
   .header("Authorization",format!("Apikey {key}"))
   .json(&json!({"username":profile,"show_calendar":false,"language":"en"}))
   .send().await.map_err(|e|e.to_string())?).await?;
 let link=info["access_url"].as_str().ok_or("Upload-Post не прислал ссылку подключения")?;
 let url=url::Url::parse(link).map_err(|e|e.to_string())?;
 if url.scheme()!="https"||url.host_str()!=Some("app.upload-post.com"){return Err("Недоверенная ссылка входа Upload-Post".into())}
 Ok(link.to_owned())
}
pub async fn send_video(file:&str,id:&str,title:&str,youtube_privacy:&str,tiktok_mode:&str)->Result<UploadResult,String>{
 let (profile,platform)=target_parts(id)?;
 if !["private","unlisted","public"].contains(&youtube_privacy){return Err("Недопустимая видимость YouTube".into())}
 if !["MEDIA_UPLOAD","DIRECT_POST"].contains(&tiktok_mode){return Err("Недопустимый режим TikTok".into())}
 let path=Path::new(file);
 let size=tokio::fs::metadata(path).await.map_err(|e|format!("Видео не найдено: {e}"))?.len();
 if size==0||size>MAX_UPLOAD{return Err("Размер видео должен быть от 1 байта до 500 МБ".into())}
 let key=get_key()?;
 let bytes=tokio::fs::read(path).await.map_err(|e|e.to_string())?;
 let part=Part::bytes(bytes).file_name("clipforge.mp4").mime_str("video/mp4").map_err(|e|e.to_string())?;
 let mut form=Form::new().part("video",part)
   .text("user",profile.to_owned()).text("platform[]",platform.to_owned())
   .text("title",title.chars().take(100).collect::<String>())
   .text("async_upload","false");
 if platform=="youtube"{form=form.text("privacyStatus",youtube_privacy.to_owned())}
 if platform=="tiktok"{
  form=form.text("post_mode",tiktok_mode.to_owned());
  if tiktok_mode=="DIRECT_POST"{form=form.text("privacy_level","PUBLIC_TO_EVERYONE")}
 }
 let http=http()?;
 let response=value(http.post(format!("{BASE}/api/upload"))
  .header("Authorization",format!("Apikey {key}"))
  .multipart(form).send().await.map_err(|e|format!("Загрузка Upload-Post: {e}"))?).await?;
 let mut data=response;
 if data["results"].get(platform).is_none(){
  if let Some(request_id)=data["request_id"].as_str().map(str::to_string){
   for _ in 0..12{
    tokio::time::sleep(Duration::from_secs(10)).await;
    let check=value(http.get(format!("{BASE}/api/uploadposts/status"))
     .header("Authorization",format!("Apikey {key}"))
     .query(&[("request_id",request_id.as_str())])
     .send().await.map_err(|e|e.to_string())?).await?;
    if check["results"].get(platform).is_some(){data=check;break}
    if check["status"].as_str().is_some_and(|s|s=="failed"||s=="error"){
     return Err(format!("Upload-Post отклонил загрузку: {}",check["message"].as_str().unwrap_or("неизвестная ошибка")))
    }
   }
  }
 }
 let target=&data["results"][platform];
 if target.is_object(){
  if target["success"]==false{return Err(format!("{}: {}",platform,target["error"].as_str().unwrap_or("загрузка не удалась")))}
  if target["success"]==true{
   let link=target["url"].as_str().or(target["post_id"].as_str()).unwrap_or("принято платформой");
   return Ok(UploadResult{state:if platform=="tiktok"&&tiktok_mode=="MEDIA_UPLOAD"{"draft".into()}else{"success".into()},detail:format!("Upload-Post: {link}")});
  }
 }
 if data["success"]==false{return Err(format!("Upload-Post: {}",data["message"].as_str().unwrap_or("отклонено")))}
 if let Some(request_id)=data["request_id"].as_str(){
  return Ok(UploadResult{state:"queued".into(),detail:format!("Upload-Post принял видео в обработку (request_id: {request_id}). Не подтверждено как опубликованное.")});
 }
 Err("Нет подтверждённого результата от Upload-Post".into())
}
