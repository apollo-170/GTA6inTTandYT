use serde::Serialize;
use std::{sync::atomic::{AtomicBool,Ordering}, time::Duration, collections::HashSet};
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
pub struct Accounts { pub items:Mutex<Vec<AccountToken>>, pub youtube_pending:AtomicBool, pub tiktok_pending:AtomicBool, pub tiktok_pair:Mutex<Option<String>> }
impl Accounts {
 pub fn new()->Self{Self{items:Mutex::new(Vec::new()),youtube_pending:AtomicBool::new(false),tiktok_pending:AtomicBool::new(false),tiktok_pair:Mutex::new(None)}}
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

fn tiktok_bridge()->Result<String,String>{
 let value=option_env!("CLIPFORGE_TIKTOK_BRIDGE_URL").filter(|s|!s.trim().is_empty())
  .ok_or("TikTok OAuth пока не настроен разработчиком: нужен публичный HTTPS TikTok OAuth Bridge с одобренным приложением TikTok. Используй пока публикацию через Twitch.")?;
 let url=url::Url::parse(value).map_err(|e|format!("Неправильный URL TikTok Bridge: {e}"))?;
 if url.scheme()!="https" || url.host_str().is_none() || url.query().is_some() || url.fragment().is_some(){
  return Err("TikTok Bridge должен быть HTTPS адресом без параметров".into())
 }
 Ok(value.trim_end_matches('/').to_owned())
}
fn bridge_http()->Result<reqwest::Client,String>{
 reqwest::Client::builder().timeout(Duration::from_secs(20))
  .redirect(reqwest::redirect::Policy::none())
  .build().map_err(|e|e.to_string())
}
pub async fn begin_tiktok(app:AppHandle)->Result<String,String>{
 let endpoint=tiktok_bridge()?;
 let st=app.state::<crate::AppState>();
 if st.accounts.tiktok_pending.swap(true,Ordering::SeqCst){
  return Err("Вход TikTok уже запущен. Заверши авторизацию или повтори позже.".into())
 }
 let pair=st.accounts.tiktok_pair.lock().await.clone();
 let result=async{
  let http=bridge_http()?;
  let reply=http.post(format!("{endpoint}/v1/session"))
   .json(&serde_json::json!({"pair_token":pair}))
   .send().await.map_err(|e|format!("TikTok Bridge недоступен: {e}"))?;
  if !reply.status().is_success(){return Err(format!("TikTok Bridge: HTTP {}",reply.status()))}
  let info:Value=reply.json().await.map_err(|e|e.to_string())?;
  let token=info["pair_token"].as_str().filter(|s|s.len()>=30).ok_or("TikTok Bridge не вернул pairing token")?.to_string();
  let authorize=info["authorize_url"].as_str().ok_or("TikTok Bridge не вернул ссылку входа")?;
  let target=url::Url::parse(authorize).map_err(|e|e.to_string())?;
  let trusted=url::Url::parse(&endpoint).map_err(|e|e.to_string())?;
  if target.origin()!=trusted.origin() || target.scheme()!="https"{
   return Err("TikTok Bridge вернул адрес другой платформы".into())
  }
  *st.accounts.tiktok_pair.lock().await=Some(token);
  Ok(authorize.to_owned())
 }.await;
 match result{
  Ok(authorize)=>{
   tokio::spawn(async move {
    let state=app.state::<crate::AppState>();
    for _ in 0..90{
     tokio::time::sleep(Duration::from_secs(2)).await;
     if let Ok(names)=sync_tiktok(&state.accounts).await{
      if !names.is_empty(){
       let _=app.emit("clipforge:refresh",());
       for name in names{let _=app.emit("clipforge:connected",format!("TikTok: {name} подключён"));}
       break;
      }
     }
    }
    state.accounts.tiktok_pending.store(false,Ordering::SeqCst);
   });
   Ok(authorize)
  },
  Err(error)=>{st.accounts.tiktok_pending.store(false,Ordering::SeqCst);Err(error)}
 }
}
async fn sync_tiktok(store:&Accounts)->Result<Vec<String>,String>{
 let endpoint=tiktok_bridge()?;
 let Some(pair)=store.tiktok_pair.lock().await.clone() else {return Ok(Vec::new())};
 let response=bridge_http()?.post(format!("{endpoint}/v1/accounts"))
  .json(&serde_json::json!({"pair_token":pair})).send().await.map_err(|e|e.to_string())?;
 if !response.status().is_success(){return Err(format!("TikTok Bridge HTTP {}",response.status()))}
 let payload:Value=response.json().await.map_err(|e|e.to_string())?;
 let list=payload["accounts"].as_array().ok_or("Некорректный список аккаунтов TikTok")?;
 let mut items=store.items.lock().await;
 let old:HashSet<String>=items.iter().filter(|a|a.platform=="tiktok").map(|a|a.id.clone()).collect();
 items.retain(|a|a.platform!="tiktok");
 let mut new_names=Vec::new();
 for item in list.iter().take(20){
  let Some(id)=item["id"].as_str().filter(|x|x.starts_with("tiktok:") && x.len()<180) else {continue};
  let name=item["name"].as_str().unwrap_or("TikTok").chars().take(80).collect::<String>();
  if !old.contains(id){new_names.push(name.clone())}
  items.push(AccountToken{id:id.to_owned(),name,platform:"tiktok".into(),
   access_token:String::new(),refresh_token:String::new(),expiry:0});
 }
 Ok(new_names)
}
pub async fn disconnect_account(store:&Accounts,id:&str)->Result<bool,String>{
 if !id.starts_with("tiktok:"){return Ok(store.disconnect(id).await)}
 let Some(pair)=store.tiktok_pair.lock().await.clone() else {return Err("Переподключи TikTok".into())};
 let endpoint=tiktok_bridge()?;
 let reply=bridge_http()?.post(format!("{endpoint}/v1/remove"))
  .json(&serde_json::json!({"pair_token":pair,"account_id":id}))
  .send().await.map_err(|e|e.to_string())?;
 if !reply.status().is_success(){return Err(format!("TikTok не отключён: HTTP {}",reply.status()))}
 Ok(store.disconnect(id).await)
}
async fn tiktok_access(store:&Accounts,id:&str)->Result<String,String>{
 let exists=store.items.lock().await.iter().any(|a|a.id==id&&a.platform=="tiktok");
 if !exists{return Err("TikTok аккаунт отключён".into())}
 let Some(pair)=store.tiktok_pair.lock().await.clone() else {return Err("Нужно заново подключить TikTok".into())};
 let endpoint=tiktok_bridge()?;
 let response=bridge_http()?.post(format!("{endpoint}/v1/access"))
  .json(&serde_json::json!({"pair_token":pair,"account_id":id}))
  .send().await.map_err(|e|format!("TikTok Bridge: {e}"))?;
 if !response.status().is_success(){return Err(format!("TikTok токен недоступен: HTTP {}",response.status()))}
 let body:Value=response.json().await.map_err(|e|e.to_string())?;
 Ok(body["access_token"].as_str().filter(|s|!s.is_empty()).ok_or("TikTok Bridge не вернул access token")?.to_owned())
}

pub async fn token_for(store:&Accounts,id:&str)->Result<(String,String),String>{
 if id.starts_with("tiktok:"){return Ok(("tiktok".into(),tiktok_access(store,id).await?))}
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
