use crate::models::{Channel,LoggedIn,TwitchDevice};
use serde_json::{json,Value};
use reqwest::Client;
use tokio::io::AsyncWriteExt;
use std::path::Path;

pub fn http()->Client {Client::builder().timeout(std::time::Duration::from_secs(25)).user_agent("ClipForge/0.1").build().expect("reqwest client")}
fn api(client:&Client,client_id:&str,token:&str,url:&str)->reqwest::RequestBuilder{client.get(url).bearer_auth(token).header("Client-Id",client_id)}
async fn parsed(res:reqwest::Response)->Result<Value,String>{
 let status=res.status();let body=res.text().await.map_err(|e|e.to_string())?;
 if !status.is_success(){return Err(format!("Twitch API HTTP {}: {}",status,body.chars().take(360).collect::<String>()))}
 serde_json::from_str(&body).map_err(|e|e.to_string())
}

pub async fn device_start(client_id:&str)->Result<TwitchDevice,String>{
 if client_id.trim().len()<6{return Err("Twitch OAuth ещё не настроен разработчиком. В сборке должен быть CLIPFORGE_TWITCH_CLIENT_ID.".into())}
 let v=parsed(http().post("https://id.twitch.tv/oauth2/device").form(&[
  ("client_id",client_id),
  ("scopes","user:read:chat clips:edit editor:manage:clips channel:manage:clips")
 ]).send().await.map_err(|e|e.to_string())?).await?;
 Ok(TwitchDevice{device_code:v["device_code"].as_str().unwrap_or_default().into(),user_code:v["user_code"].as_str().unwrap_or_default().into(),verification_uri:v["verification_uri"].as_str().unwrap_or_default().into(),interval:v["interval"].as_u64().unwrap_or(5)})
}
pub async fn device_poll(client_id:&str,device_code:&str)->Result<Option<(String,LoggedIn)>,String>{
 let res=http().post("https://id.twitch.tv/oauth2/token").form(&[
  ("client_id",client_id),
  ("device_code",device_code),
  ("grant_type","urn:ietf:params:oauth:grant-type:device_code")
 ]).send().await.map_err(|e|e.to_string())?;
 let status=res.status();let v:Value=res.json().await.map_err(|e|e.to_string())?;
 if !status.is_success(){
  let error=v["error"].as_str().or(v["message"].as_str()).unwrap_or("OAuth error");
  let normalized=error.to_ascii_lowercase().replace(' ',"_");
  if normalized.contains("authorization_pending")||normalized.contains("slow_down"){return Ok(None)}
  return Err(error.to_string());
 }
 let token=v["access_token"].as_str().ok_or("Не найден access_token")?.to_string();
 let login=validate(&token).await?;
 if login.0!=client_id{return Err("Client ID в токене не совпадает".into())}
 Ok(Some((token,login.1)))
}
async fn validate(token:&str)->Result<(String,LoggedIn),String>{
 let v=parsed(http().get("https://id.twitch.tv/oauth2/validate").header("Authorization",format!("OAuth {}",token)).send().await.map_err(|e|e.to_string())?).await?;
 Ok((v["client_id"].as_str().unwrap_or_default().into(),LoggedIn{display_name:v["login"].as_str().unwrap_or("unknown").into(),user_id:v["user_id"].as_str().unwrap_or_default().into()}))
}
pub async fn resolve_channel(client_id:&str,token:&str,login:&str)->Result<Channel,String>{
 let clean=login.trim().trim_start_matches('@').to_lowercase();
 if clean.is_empty()||!clean.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'){return Err("Некорректное имя канала".into())}
 let v=parsed(api(&http(),client_id,token,"https://api.twitch.tv/helix/users").query(&[("login",clean.as_str())]).send().await.map_err(|e|e.to_string())?).await?;
 let u=v["data"].as_array().and_then(|a|a.first()).ok_or("Канал не найден")?;
 Ok(Channel{id:u["id"].as_str().unwrap_or_default().into(),login:u["login"].as_str().unwrap_or(&clean).into(),display_name:u["display_name"].as_str().unwrap_or(&clean).into(),enabled:true,rights_confirmed:false,is_live:false,messages:0,last_score:0})
}
pub async fn stream_live(client_id:&str,token:&str,user_id:&str)->Result<bool,String>{
 let v=parsed(api(&http(),client_id,token,"https://api.twitch.tv/helix/streams").query(&[("user_id",user_id)]).send().await.map_err(|e|e.to_string())?).await?;
 Ok(v["data"].as_array().is_some_and(|a|!a.is_empty()))
}
pub async fn subscribe_chat(client_id:&str,token:&str,session_id:&str,reader_id:&str,broadcaster_id:&str)->Result<(),String>{
 let json=json!({"type":"channel.chat.message","version":"1","condition":{"broadcaster_user_id":broadcaster_id,"user_id":reader_id},"transport":{"method":"websocket","session_id":session_id}});
 parsed(http().post("https://api.twitch.tv/helix/eventsub/subscriptions").bearer_auth(token).header("Client-Id",client_id).json(&json).send().await.map_err(|e|e.to_string())?).await?;
 Ok(())
}
pub async fn create_clip(client_id:&str,token:&str,broadcaster:&str,title:&str,duration:u32)->Result<String,String>{
 let dur=duration.clamp(5,60).to_string();
 let v=parsed(http().post("https://api.twitch.tv/helix/clips").bearer_auth(token).header("Client-Id",client_id)
  .query(&[("broadcaster_id",broadcaster),("duration",dur.as_str()),("title",title)])
  .send().await.map_err(|e|e.to_string())?).await?;
 v["data"].as_array().and_then(|a|a.first()).and_then(|x|x["id"].as_str()).map(str::to_string).ok_or("Twitch не вернул clip ID".into())
}

// Exact time window via authorized broadcaster/editor VOD when currently available.
pub async fn create_vod_clip(client_id:&str,token:&str,broadcaster:&str,editor:&str,title:&str,detected_at:&str,duration:u32,post_seconds:u64)->Result<String,String>{
 let streams=parsed(api(&http(),client_id,token,"https://api.twitch.tv/helix/streams").query(&[("user_id",broadcaster)]).send().await.map_err(|e|e.to_string())?).await?;
 let live=streams["data"].as_array().and_then(|x|x.first()).ok_or("Стрим уже завершился или недоступен")?;
 let stream_id=live["id"].as_str().ok_or("Нет Stream ID")?;
 let started=live["started_at"].as_str().ok_or("Нет started_at")?;
 let start_time=chrono::DateTime::parse_from_rfc3339(started).map_err(|e|e.to_string())?;
 let event_time=chrono::DateTime::parse_from_rfc3339(detected_at).map_err(|e|e.to_string())?;
 let end_offset=(event_time-start_time).num_seconds()+post_seconds as i64;
 if end_offset < duration as i64{return Err("Стрим слишком короткий для выбранного окна".into())}
 let videos=parsed(api(&http(),client_id,token,"https://api.twitch.tv/helix/videos").query(&[("user_id",broadcaster),("type","archive"),("first","20")]).send().await.map_err(|e|e.to_string())?).await?;
 let vod=videos["data"].as_array().and_then(|arr|arr.iter().find(|v|v["stream_id"].as_str()==Some(stream_id))).and_then(|v|v["id"].as_str()).ok_or("Текущий VOD ещё недоступен")?;
 let dur=duration.clamp(5,60).to_string();let offset=end_offset.to_string();
 let v=parsed(http().post("https://api.twitch.tv/helix/videos/clips").bearer_auth(token).header("Client-Id",client_id)
  .query(&[("editor_id",editor),("broadcaster_id",broadcaster),("vod_id",vod),("vod_offset",offset.as_str()),("duration",dur.as_str()),("title",title)])
  .send().await.map_err(|e|e.to_string())?).await?;
 v["data"].as_array().and_then(|a|a.first()).and_then(|x|x["id"].as_str()).map(str::to_string).ok_or("Twitch не вернул VOD clip ID".into())
}

pub async fn clip_url(client_id:&str,token:&str,id:&str)->Result<Option<String>,String>{
 let v=parsed(api(&http(),client_id,token,"https://api.twitch.tv/helix/clips").query(&[("id",id)]).send().await.map_err(|e|e.to_string())?).await?;
 Ok(v["data"].as_array().and_then(|a|a.first()).and_then(|x|x["url"].as_str()).map(str::to_string))
}
pub async fn download_clip(client_id:&str,token:&str,broadcaster:&str,editor:&str,clip_id:&str,path:&Path)->Result<(),String>{
 let v=parsed(api(&http(),client_id,token,"https://api.twitch.tv/helix/clips/downloads").query(&[("broadcaster_id",broadcaster),("editor_id",editor),("clip_id",clip_id)]).send().await.map_err(|e|e.to_string())?).await?;
 let item=v["data"].as_array().and_then(|a|a.first()).ok_or("Нет прав владельца/редактора на скачивание клипа")?;
 let url=item["landscape_download_url"].as_str().or_else(||item["portrait_download_url"].as_str()).ok_or("Twitch не предоставил MP4")?;
 if !url.starts_with("https://"){return Err("Небезопасный URL загрузки".into())}
 if let Some(parent)=path.parent(){tokio::fs::create_dir_all(parent).await.map_err(|e|e.to_string())?}
 let mut res=Client::builder().timeout(std::time::Duration::from_secs(180)).build().map_err(|e|e.to_string())?.get(url).send().await.map_err(|e|e.to_string())?.error_for_status().map_err(|e|e.to_string())?;
 let mut f=tokio::fs::File::create(path).await.map_err(|e|e.to_string())?;
 while let Some(chunk)=res.chunk().await.map_err(|e|e.to_string())?{f.write_all(&chunk).await.map_err(|e|e.to_string())?;}
 f.flush().await.map_err(|e|e.to_string())?;
 Ok(())
}
