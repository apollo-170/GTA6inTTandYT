//! Native TikTok Desktop Login Kit and Content Posting API.
//! Personal Windows installation: credentials kept in Windows Credential Manager.
//! Official API only: no cookie scraping, CAPTCHA bypass or web automation.
use crate::social::AccountView;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD,Engine};
use rand::{rngs::OsRng,RngCore};
use reqwest::header::{CONTENT_LENGTH,CONTENT_RANGE,CONTENT_TYPE};
use serde::{Serialize,Deserialize};
use serde_json::{json,Value};
use sha2::{Sha256,Digest};
use std::{sync::atomic::{AtomicBool,Ordering},time::Duration};
use tauri::{AppHandle,Emitter};
use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::TcpListener};

const CRED_APP:&str="ClipForge.TikTok.LocalDeveloper";
const CRED_ACCOUNT:&str="ClipForge.TikTok.Account";
const CLIENT_INDEX:&str="ClipForge.TikTok.AccountList";
const CALLBACK:&str="http://127.0.0.1:34865/callback/";
const LIMIT:u64=500*1024*1024;
static LOGIN:AtomicBool=AtomicBool::new(false);

#[derive(Clone,Serialize,Deserialize)]
struct TikTokAccount {
 id:String,name:String,access:String,refresh:String,expires:i64,scopes:String
}
#[derive(Clone,Serialize)]
pub struct TikTokStatus{pub configured:bool,pub connected:usize,pub redirect_uri:String}
#[derive(Clone,Serialize)]
pub struct CreatorInfo{pub name:String,pub privacy_levels:Vec<String>,pub max_duration:i64,pub comments_disabled:bool,pub duet_disabled:bool,pub stitch_disabled:bool}
#[derive(Clone)]
pub struct PostResult{pub state:String,pub detail:String}

fn cred(service:&str,name:&str)->Result<keyring::Entry,String>{
 keyring::Entry::new(service,name).map_err(|e|format!("Windows Credential Manager: {e}"))
}
fn app_credentials()->Result<(String,String),String>{
 let key=cred(CRED_APP,"client_key")?.get_password().map_err(|_|"В разделе TikTok укажи Client Key из TikTok Developer".to_string())?;
 let secret=cred(CRED_APP,"client_secret")?.get_password().map_err(|_|"В разделе TikTok укажи Client Secret из TikTok Developer".to_string())?;
 if key.trim().is_empty()||secret.trim().is_empty(){return Err("TikTok Client Key и Client Secret не настроены".into())}
 Ok((key,secret))
}
pub fn status()->TikTokStatus{
 TikTokStatus{configured:app_credentials().is_ok(),connected:accounts().len(),redirect_uri:CALLBACK.into()}
}
pub fn configure(key:&str,secret:&str)->Result<(),String>{
 let key=key.trim();let secret=secret.trim();
 if key.len()<8||key.len()>160||secret.len()<12||secret.len()>250||key.chars().any(char::is_whitespace)||secret.chars().any(char::is_whitespace){
  return Err("Проверь Client Key и Client Secret TikTok Developer".into())
 }
 cred(CRED_APP,"client_key")?.set_password(key).map_err(|e|e.to_string())?;
 cred(CRED_APP,"client_secret")?.set_password(secret).map_err(|e|e.to_string())
}
fn ids()->Vec<String>{
 let Ok(raw)=cred(CLIENT_INDEX,"ids").and_then(|entry|entry.get_password().map_err(|e|e.to_string()))else{return Vec::new()};
 serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default().into_iter()
  .filter(|id|id.starts_with("tt:")&&id.len()<150).take(50).collect()
}
fn store_index(index:&[String])->Result<(),String>{
 cred(CLIENT_INDEX,"ids")?.set_password(&serde_json::to_string(index).map_err(|e|e.to_string())?)
  .map_err(|e|e.to_string())
}
fn load(id:&str)->Result<TikTokAccount,String>{
 if !id.starts_with("tt:"){return Err("Неверный TikTok Account ID".into())}
 let text=cred(CRED_ACCOUNT,id)?.get_password().map_err(|_|"Аккаунт TikTok отключён; войди заново".to_string())?;
 let account:TikTokAccount=serde_json::from_str(&text).map_err(|_|"Ошибка сохранённого аккаунта TikTok".to_string())?;
 if account.id!=id{return Err("Несовпадение TikTok аккаунта".into())}
 Ok(account)
}
fn store(account:&TikTokAccount)->Result<(),String>{
 if !account.id.starts_with("tt:")||account.id.len()>150{return Err("Некорректный TikTok open_id".into())}
 cred(CRED_ACCOUNT,&account.id)?.set_password(&serde_json::to_string(account).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 let mut all=ids();
 if !all.contains(&account.id){all.push(account.id.clone())}
 store_index(&all)
}
pub fn accounts()->Vec<AccountView>{
 ids().into_iter().filter_map(|id|load(&id).ok().map(|a|AccountView{id:a.id,name:a.name,platform:"tiktok".into()})).collect()
}
pub fn remove(id:&str)->Result<(),String>{
 if !ids().iter().any(|a|a==id){return Err("TikTok-аккаунт не найден".into())}
 let _=cred(CRED_ACCOUNT,id)?.delete_credential();
 let mut all=ids();all.retain(|a|a!=id);store_index(&all)
}
fn http()->Result<reqwest::Client,String>{
 reqwest::Client::builder().user_agent("ClipForge TikTok Desktop/0.8")
  .timeout(Duration::from_secs(90)).redirect(reqwest::redirect::Policy::none())
  .build().map_err(|e|e.to_string())
}
fn random(len:usize)->String{
 let mut buf=vec![0u8;len];OsRng.fill_bytes(&mut buf);URL_SAFE_NO_PAD.encode(buf)
}
fn checked(value:Value)->Result<Value,String>{
 let err=value["error"]["code"].as_str().unwrap_or("ok");
 if err!="ok"{return Err(format!("TikTok API: {} — {}",err,value["error"]["message"].as_str().unwrap_or("проверь права доступа")))}
 Ok(value)
}
async fn tokens(form:Vec<(String,String)>)->Result<Value,String>{
 let params:Vec<(&str,&str)>=form.iter().map(|(k,v)|(k.as_str(),v.as_str())).collect();
 let reply=http()?.post("https://open.tiktokapis.com/v2/oauth/token/").form(&params)
  .send().await.map_err(|e|format!("TikTok OAuth: {e}"))?;
 let status=reply.status();let payload:Value=reply.json().await.map_err(|e|e.to_string())?;
 if !status.is_success()||payload["access_token"].as_str().is_none(){
  return Err(format!("TikTok OAuth HTTP {status}: {}",payload["error_description"].as_str().or(payload["error"].as_str()).unwrap_or("проверь Client Key, Secret, Redirect и разрешения")))
 }
 Ok(payload)
}
async fn user_name(access:&str)->String{
 let Ok(client)=http()else{return "TikTok-аккаунт".into()};
 let Ok(resp)=client.get("https://open.tiktokapis.com/v2/user/info/?fields=open_id,display_name")
  .bearer_auth(access).send().await else{return "TikTok-аккаунт".into()};
 let Ok(json)=resp.json::<Value>().await else{return "TikTok-аккаунт".into()};
 json["data"]["user"]["display_name"].as_str().filter(|s|!s.trim().is_empty()).unwrap_or("TikTok-аккаунт").chars().take(80).collect()
}
async fn finish_oauth(code:&str,verifier:&str)->Result<TikTokAccount,String>{
 let (key,secret)=app_credentials()?;
 let token=tokens(vec![
  ("client_key".into(),key),("client_secret".into(),secret),
  ("code".into(),code.into()),("grant_type".into(),"authorization_code".into()),
  ("code_verifier".into(),verifier.into()),("redirect_uri".into(),CALLBACK.into())
 ]).await?;
 let id=token["open_id"].as_str().filter(|id|!id.is_empty()&&id.len()<110).ok_or("TikTok не вернул open_id")?;
 let access=token["access_token"].as_str().ok_or("TikTok не вернул access_token")?;
 let refresh=token["refresh_token"].as_str().ok_or("TikTok не вернул refresh_token")?;
 let scopes=token["scope"].as_str().unwrap_or("");
 if !scopes.split(',').any(|p|p.trim()=="video.publish"||p.trim()=="video.upload"){
  return Err("TikTok не разрешил video.publish или video.upload. Проверь Content Posting API".into())
 }
 let account=TikTokAccount{
  id:format!("tt:{id}"),name:user_name(access).await,access:access.into(),refresh:refresh.into(),
  expires:chrono::Utc::now().timestamp()+token["expires_in"].as_i64().unwrap_or(7200),
  scopes:scopes.into()
 };
 store(&account)?;
 Ok(account)
}
struct LoginGuard;
impl Drop for LoginGuard{fn drop(&mut self){LOGIN.store(false,Ordering::SeqCst);}}
pub async fn begin_login(app:AppHandle,direct:bool)->Result<String,String>{
 let (key,_)=app_credentials()?;
 if LOGIN.swap(true,Ordering::SeqCst){return Err("Авторизация TikTok уже открыта".into())}
 let listener=match TcpListener::bind("127.0.0.1:34865").await{
  Ok(l)=>l,
  Err(e)=>{LOGIN.store(false,Ordering::SeqCst);return Err(format!("Порт 34865 занят: {e}"))}
 };
 let verifier=random(48);let state=random(24);
 let challenge=format!("{:x}",Sha256::digest(verifier.as_bytes()));
 let mut url=url::Url::parse("https://www.tiktok.com/v2/auth/authorize/").map_err(|e|e.to_string())?;
 url.query_pairs_mut().append_pair("client_key",&key)
  .append_pair("scope",if direct{"user.info.basic,video.publish"}else{"user.info.basic,video.upload"})
  .append_pair("response_type","code")
  .append_pair("redirect_uri",CALLBACK)
  .append_pair("state",&state)
  .append_pair("code_challenge",&challenge)
  .append_pair("code_challenge_method","S256");
 tokio::spawn(async move{
  let _guard=LoginGuard;
  let result=tokio::time::timeout(Duration::from_secs(300),async{
   loop {
    let (mut socket,_)=listener.accept().await.map_err(|e|e.to_string())?;
    let mut bytes=[0u8;4096];
    let size=tokio::time::timeout(Duration::from_secs(8),socket.read(&mut bytes))
      .await.map_err(|_|"TikTok callback timeout".to_string())?.map_err(|e|e.to_string())?;
    let raw=String::from_utf8_lossy(&bytes[..size]);
    let first=raw.lines().next().unwrap_or("");
    let Some(target)=first.strip_prefix("GET ").and_then(|v|v.split_whitespace().next()) else {continue};
    if !target.starts_with("/callback/?"){continue}
    let Ok(callback)=url::Url::parse(&format!("http://127.0.0.1:34865{target}"))else{continue};
    let query:std::collections::HashMap<String,String>=callback.query_pairs().map(|(a,b)|(a.into_owned(),b.into_owned())).collect();
    if query.get("state")!=Some(&state){
      let _=socket.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
      continue;
    }
    let code=query.get("code").cloned();
    let html=if code.is_some(){"<h2>ClipForge: TikTok authorization received.</h2><p>Return to ClipForge.</p>"}
      else{"<h2>TikTok authorization denied or cancelled.</h2><p>Return to ClipForge.</p>"};
    let response=format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",html.len(),html);
    let _=socket.write_all(response.as_bytes()).await;
    let Some(code)=code else{return Err("TikTok вход отменён".into())};
    return finish_oauth(&code,&verifier).await;
   }
  }).await;
  match result {
   Ok(Ok(account))=>{
    let _=app.emit("clipforge:connected",format!("TikTok: {} подключён",account.name));
    let _=app.emit("clipforge:refresh",());
   }
   Ok(Err(e))=>{let _=app.emit("clipforge:notice",format!("TikTok: {e}"));}
   Err(_)=>{let _=app.emit("clipforge:notice","TikTok: время авторизации истекло");}
  }
 });
 Ok(url.to_string())
}
async fn access(id:&str,required:&str)->Result<String,String>{
 let mut account=load(id)?;
 if !account.scopes.split(',').any(|v|v.trim()==required){
  return Err(format!("Аккаунт не разрешил {required}. Переподключи аккаунт с нужными правами."))
 }
 if account.expires<=chrono::Utc::now().timestamp()+300{
  let (key,secret)=app_credentials()?;
  let token=tokens(vec![
   ("client_key".into(),key),("client_secret".into(),secret),
   ("refresh_token".into(),account.refresh),("grant_type".into(),"refresh_token".into())
  ]).await?;
  account.access=token["access_token"].as_str().ok_or("TikTok не обновил access_token")?.into();
  account.refresh=token["refresh_token"].as_str().ok_or("TikTok не обновил refresh_token")?.into();
  account.expires=chrono::Utc::now().timestamp()+token["expires_in"].as_i64().unwrap_or(7200);
  account.scopes=token["scope"].as_str().unwrap_or(&account.scopes).into();
  store(&account)?;
 }
 Ok(account.access)
}
pub async fn creator(id:&str)->Result<CreatorInfo,String>{
 let token=access(id,"video.publish").await?;
 let reply=http()?.post("https://open.tiktokapis.com/v2/post/publish/creator_info/query/")
  .bearer_auth(&token).json(&json!({})).send().await.map_err(|e|e.to_string())?;
 let status=reply.status();let body:Value=reply.json().await.map_err(|e|e.to_string())?;
 if !status.is_success(){return Err(format!("TikTok Creator Info HTTP {status}"))}
 let payload=checked(body)?;
 let data=&payload["data"];
 let privacy_levels=data["privacy_level_options"].as_array().map(|list|list.iter().filter_map(|p|p.as_str().map(str::to_owned)).collect()).unwrap_or_default();
 Ok(CreatorInfo{
  name:data["creator_nickname"].as_str().unwrap_or("TikTok").to_owned(),
  privacy_levels,max_duration:data["max_video_post_duration_sec"].as_i64().unwrap_or(60),
  comments_disabled:data["comment_disabled"].as_bool().unwrap_or(true),
  duet_disabled:data["duet_disabled"].as_bool().unwrap_or(true),
  stitch_disabled:data["stitch_disabled"].as_bool().unwrap_or(true)
 })
}
async fn upload_chunks(path:&str,url:&str)->Result<(),String>{
 let uri=url::Url::parse(url).map_err(|_|"TikTok вернул некорректный upload URL".to_string())?;
 if uri.scheme()!="https"||uri.host_str()!=Some("open-upload.tiktokapis.com"){
  return Err("TikTok вернул недоверенный upload endpoint".into())
 }
 let size=tokio::fs::metadata(path).await.map_err(|e|e.to_string())?.len();
 if size==0||size>LIMIT{return Err("TikTok MP4 должен быть меньше 500 МБ".into())}
 let chunk=if size<10_000_000{size}else{10_000_000u64};
 let count=size.div_ceil(chunk);
 let mut file=tokio::fs::File::open(path).await.map_err(|e|e.to_string())?;
 let client=http()?;
 for index in 0..count{
  let start=index*chunk;let length=(size-start).min(chunk);
  let mut data=vec![0u8;length as usize];
  file.read_exact(&mut data).await.map_err(|e|e.to_string())?;
  let response=client.put(url).header(CONTENT_TYPE,"video/mp4")
   .header(CONTENT_RANGE,format!("bytes {}-{}/{}",start,start+length-1,size))
   .header(CONTENT_LENGTH,length.to_string()).body(data)
   .send().await.map_err(|e|format!("TikTok загрузка: {e}"))?;
  if !response.status().is_success(){return Err(format!("TikTok отклонил фрагмент: HTTP {}",response.status()))}
 }
 Ok(())
}
pub async fn publish(path:&str,id:&str,title:&str,privacy:&str,mode:&str)->Result<PostResult,String>{
 if !["DIRECT_POST","MEDIA_UPLOAD"].contains(&mode){return Err("Режим TikTok не поддерживается".into())}
 if title.trim().is_empty(){return Err("Укажи название TikTok ролика".into())}
 let file=tokio::fs::metadata(path).await.map_err(|e|e.to_string())?;
 if !file.is_file()||file.len()==0||file.len()>LIMIT{return Err("Неверный размер MP4 (до 500 МБ)".into())}
 let account=load(id)?;
 let direct=mode=="DIRECT_POST";
 let required=if direct{"video.publish"}else{"video.upload"};
 if !account.scopes.split(',').any(|v|v.trim()==required){
  return Err(format!("Аккаунт не разрешил {required}. Переподключи его в соответствующем режиме."))
 }
 let token=access(id,required).await?;
 let info=if direct{Some(creator(id).await?)}else{None};
 if let Some(ref creator)=info{
  if !creator.privacy_levels.iter().any(|v|v==privacy){
   return Err(format!("У этого TikTok аккаунта недоступна видимость {privacy}. Выбери разрешённую в ClipForge."))
  }
 }else if privacy!="SELF_ONLY"{
  return Err("У TikTok есть только разрешение на черновики. Для прямой публикации нужен video.publish".into())
 }
 let chunk_size=file.len().min(10_000_000u64);
 let chunks=file.len().div_ceil(chunk_size);
 let source=json!({"source":"FILE_UPLOAD","video_size":file.len(),"chunk_size":chunk_size,"total_chunk_count":chunks});
 let init=if direct{"https://open.tiktokapis.com/v2/post/publish/video/init/"}
  else{"https://open.tiktokapis.com/v2/post/publish/inbox/video/init/"};
 let request=if direct{
  json!({"source_info":source,"post_info":{
   "title":title.chars().take(150).collect::<String>(),
   "privacy_level":privacy,
   "disable_comment":info.as_ref().unwrap().comments_disabled,
   "disable_duet":info.as_ref().unwrap().duet_disabled,
   "disable_stitch":info.as_ref().unwrap().stitch_disabled,
   "video_cover_timestamp_ms":1000
  }})
 }else{json!({"source_info":source})};
 let response=http()?.post(init).bearer_auth(&token).json(&request)
  .send().await.map_err(|e|e.to_string())?;
 let status=response.status();let body:Value=response.json().await.map_err(|e|e.to_string())?;
 if !status.is_success(){return Err(format!("TikTok Init HTTP {status}: {}",body["error"]["message"].as_str().unwrap_or("ошибка")))}
 let response=checked(body)?;
 let upload=response["data"]["upload_url"].as_str().ok_or("TikTok не прислал upload_url")?;
 let id=response["data"]["publish_id"].as_str().ok_or("TikTok не прислал publish_id")?;
 upload_chunks(path,upload).await?;
 let mut state=if direct{"queued"}else{"draft"}.to_string();
 let mut message=if direct{"TikTok принял видео на обработку"}else{"Отправлено в TikTok Inbox; заверши публикацию в приложении TikTok"}.to_owned();
 for _ in 0..6{
  tokio::time::sleep(Duration::from_secs(5)).await;
  let response=http()?.post("https://open.tiktokapis.com/v2/post/publish/status/fetch/")
   .bearer_auth(&token).json(&json!({"publish_id":id})).send().await;
  let Ok(reply)=response else{break};
  let Ok(payload)=reply.json::<Value>().await else{break};
  if checked(payload.clone()).is_err(){break}
  match payload["data"]["status"].as_str().unwrap_or(""){
   "PUBLISH_COMPLETE"=>{state="success".into();message="TikTok подтвердил публикацию".into();break}
   "FAILED"=>{return Err(format!("TikTok не опубликовал: {}",payload["data"]["fail_reason"].as_str().unwrap_or("unknown")))}
   "SEND_TO_USER_INBOX"=>{state="draft".into();message="Черновик доступен в TikTok Inbox".into();break}
   _=>{}
  }
 }
 Ok(PostResult{state,detail:format!("{message}. ID: {id}")})
}
