use serde::Serialize;
use std::{sync::atomic::{AtomicBool,Ordering}, time::Duration};
use tauri::{AppHandle,Emitter,Manager};
use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::TcpListener,sync::Mutex};
use rand::{distributions::Alphanumeric,Rng};
use sha2::{Digest,Sha256};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD,Engine};
use serde_json::Value;

#[derive(Clone,Serialize)]
pub struct AccountView { pub id:String,pub name:String,pub platform:String }
#[derive(Clone)]
pub struct AccountToken { pub id:String,pub name:String,pub platform:String,pub access_token:String,pub refresh_token:String,pub expiry:i64 }
pub struct Accounts { pub items:Mutex<Vec<AccountToken>>, pub youtube_pending:AtomicBool }
impl Accounts {
 pub fn new()->Self{Self{items:Mutex::new(Vec::new()),youtube_pending:AtomicBool::new(false)}}
 pub async fn list(&self)->Vec<AccountView>{self.items.lock().await.iter().map(|a|AccountView{id:a.id.clone(),name:a.name.clone(),platform:a.platform.clone()}).collect()}
 pub async fn disconnect(&self,id:&str)->bool{
  let mut accounts=self.items.lock().await;
  let count=accounts.len();accounts.retain(|a|a.id!=id);count!=accounts.len()
 }
}
fn random(len:usize)->String{rand::thread_rng().sample_iter(&Alphanumeric).take(len).map(char::from).collect()}
fn youtube_id()->Result<&'static str,String>{
 option_env!("CLIPFORGE_YOUTUBE_CLIENT_ID").filter(|s|!s.trim().is_empty())
 .ok_or_else(||"Вход YouTube ещё не настроен владельцем ClipForge. Нужен Google OAuth Client ID типа Desktop app в настройках сборки (не пароль и не токен пользователя).".to_string())
}

pub async fn begin_youtube(app:AppHandle)->Result<String,String>{
 let client_id=youtube_id()?.to_owned();
 let st=app.state::<crate::AppState>();
 if st.accounts.youtube_pending.swap(true,Ordering::SeqCst){return Err("Окно входа YouTube уже открыто".into())}
 let listener=match TcpListener::bind(("127.0.0.1",0)).await{
  Ok(v)=>v,Err(e)=>{st.accounts.youtube_pending.store(false,Ordering::SeqCst);return Err(format!("Не удалось открыть локальный OAuth порт: {e}"))}
 };
 let port=listener.local_addr().map_err(|e|e.to_string())?.port();
 let redirect=format!("http://127.0.0.1:{port}/oauth/youtube");
 let verifier=random(64);
 let challenge=URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
 let state=random(48);
 let mut url=url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").map_err(|e|e.to_string())?;
 url.query_pairs_mut()
 .append_pair("client_id",&client_id).append_pair("redirect_uri",&redirect)
 .append_pair("response_type","code").append_pair("scope","https://www.googleapis.com/auth/youtube.upload https://www.googleapis.com/auth/youtube.readonly")
 .append_pair("access_type","offline").append_pair("prompt","select_account consent")
 .append_pair("code_challenge",&challenge).append_pair("code_challenge_method","S256").append_pair("state",&state);
 tokio::spawn(async move {
  let result=finish_youtube(listener,&client_id,&redirect,&verifier,&state).await;
  let st=app.state::<crate::AppState>();
  st.accounts.youtube_pending.store(false,Ordering::SeqCst);
  match result{
   Ok(account)=>{
    let name=account.name.clone();
    let mut items=st.accounts.items.lock().await;
    items.retain(|a|a.id!=account.id);
    items.push(account);
    drop(items);
    let _=app.emit("clipforge:refresh",());
    let _=app.emit("clipforge:connected",format!("YouTube: {name} подключён"));
   }
   Err(err)=>{let _=app.emit("clipforge:notice",format!("YouTube OAuth: {err}"));}
  }
 });
 Ok(url.to_string())
}
async fn finish_youtube(listener:TcpListener,client_id:&str,redirect:&str,verifier:&str,state:&str)->Result<AccountToken,String>{
 let deadline=tokio::time::Instant::now()+Duration::from_secs(180);
 let code=loop{
  let time=deadline.saturating_duration_since(tokio::time::Instant::now());
  if time.is_zero(){return Err("Время авторизации истекло".into())}
  let (mut socket,_)=tokio::time::timeout(time,listener.accept()).await.map_err(|_|"Истекло время ожидания входа YouTube".to_string())?.map_err(|e|e.to_string())?;
  let mut buf=[0u8;8192];
  let n=tokio::time::timeout(Duration::from_secs(5),socket.read(&mut buf)).await.map_err(|_|"Таймаут локального callback".to_string())?.map_err(|e|e.to_string())?;
  let request=String::from_utf8_lossy(&buf[..n]);
  let first=request.lines().next().unwrap_or("");
  let mut parts=first.split_whitespace();
  if parts.next()!=Some("GET"){continue}
  let path=parts.next().unwrap_or("");
  let callback=match url::Url::parse(&format!("http://127.0.0.1{path}")){Ok(u)=>u,Err(_)=>continue};
  if callback.path()!="/oauth/youtube"{let _=socket.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await;continue}
  let incoming_state=callback.query_pairs().find(|(k,_)|k=="state").map(|(_,v)|v.to_string()).unwrap_or_default();
  if incoming_state!=state {
   let _=socket.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n").await;
   continue;
  }
  let code=callback.query_pairs().find(|(k,_)|k=="code").map(|(_,v)|v.to_string());
  let error=callback.query_pairs().find(|(k,_)|k=="error").map(|(_,v)|v.to_string());
  let html=if code.is_some(){"<html><meta charset=\"utf-8\"><body style=\"font:18px sans-serif;background:#161622;color:white;padding:40px\"><h2>YouTube подключается к ClipForge</h2><p>Можешь закрыть эту вкладку и вернуться в программу.</p></body></html>"}else{"<html><meta charset=\"utf-8\"><body><h2>Вход отменён</h2>Вернись в ClipForge.</body></html>"};
  let response=format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",html.len());
  let _=socket.write_all(response.as_bytes()).await;
  if let Some(e)=error{return Err(format!("Авторизация отменена: {e}"))}
  break code.ok_or("Google не вернул код авторизации")?;
 };
 let client=reqwest::Client::builder().timeout(Duration::from_secs(30)).build().map_err(|e|e.to_string())?;
 let token_res=client.post("https://oauth2.googleapis.com/token").form(&[
  ("client_id",client_id),("code",code.as_str()),("code_verifier",verifier),
  ("redirect_uri",redirect),("grant_type","authorization_code")
 ]).send().await.map_err(|e|e.to_string())?;
 let status=token_res.status();
 let token:Value=token_res.json().await.map_err(|e|e.to_string())?;
 if !status.is_success(){return Err(format!("Google token HTTP {status}: {}",token["error_description"].as_str().or(token["error"].as_str()).unwrap_or("OAuth error")))}
 let access=token["access_token"].as_str().ok_or("Google не вернул access_token")?.to_owned();
 let info_res=client.get("https://www.googleapis.com/youtube/v3/channels")
 .bearer_auth(&access).query(&[("part","snippet"),("mine","true")]).send().await.map_err(|e|e.to_string())?;
 let status=info_res.status();
 let info:Value=info_res.json().await.map_err(|e|e.to_string())?;
 if !status.is_success(){return Err(format!("Не удалось получить канал YouTube (HTTP {status}); проверь доступ к YouTube Data API"))}
 let channel=info["items"].as_array().and_then(|a|a.first()).ok_or("В аккаунте не найден YouTube-канал")?;
 let id=channel["id"].as_str().ok_or("YouTube вернул канал без ID")?;
 let name=channel["snippet"]["title"].as_str().unwrap_or("YouTube-канал");
 Ok(AccountToken{id:format!("youtube:{id}"),name:name.to_owned(),platform:"youtube".into(),access_token:access,refresh_token:token["refresh_token"].as_str().unwrap_or("").to_owned(),expiry:chrono::Utc::now().timestamp()+token["expires_in"].as_i64().unwrap_or(3600)})
}
pub async fn token_for(store:&Accounts,id:&str)->Result<(String,String),String>{
 let mut items=store.items.lock().await;
 let account=items.iter_mut().find(|a|a.id==id).ok_or("Аккаунт отключён. Войди заново")?;
 if account.platform=="youtube"&&account.expiry < chrono::Utc::now().timestamp()+90 {
  if account.refresh_token.is_empty(){return Err("Сессия YouTube истекла. Нажми «Войти через YouTube» снова.".into())}
  let client_id=youtube_id()?.to_owned();
  let reply=reqwest::Client::new().post("https://oauth2.googleapis.com/token")
   .form(&[("client_id",client_id.as_str()),("refresh_token",account.refresh_token.as_str()),("grant_type","refresh_token")])
   .send().await.map_err(|e|e.to_string())?;
  let status=reply.status();let data:Value=reply.json().await.map_err(|e|e.to_string())?;
  if !status.is_success(){return Err(format!("YouTube: нужно повторно войти (HTTP {status})"))}
  account.access_token=data["access_token"].as_str().ok_or("Google не вернул новый токен")?.to_owned();
  account.expiry=chrono::Utc::now().timestamp()+data["expires_in"].as_i64().unwrap_or(3600);
 }
 Ok((account.platform.clone(),account.access_token.clone()))
}
