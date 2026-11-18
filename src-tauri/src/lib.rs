mod models;mod db;mod scoring;mod twitch;mod export;mod publisher;mod vision;mod ffmpeg;mod social;mod updates;mod upload_post;
use chrono::Utc;
use futures_util::StreamExt;
use models::{Channel,Highlight,Settings,Snapshot,TwitchDevice,LoggedIn};
use scoring::Detector;
use std::{collections::HashMap,sync::{Arc,atomic::{AtomicBool,Ordering}},path::PathBuf};
use tauri::{Manager,Emitter,State,AppHandle};
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use serde_json::Value;

struct Auth {token:String,user:Option<LoggedIn>,device:Option<TwitchDevice>}
struct AppState {
 database:Mutex<db::Database>,auth:Mutex<Auth>,detector:Mutex<Detector>,
 counts:Mutex<HashMap<String,u64>>,monitor:Arc<AtomicBool>,worker:Mutex<Option<tokio::task::JoinHandle<()>>>,
 accounts:social::Accounts,
 publishing:AtomicBool,
}
fn twitch_id(settings:&Settings)->String{option_env!("CLIPFORGE_TWITCH_CLIENT_ID").filter(|s|!s.trim().is_empty()).unwrap_or(&settings.client_id).to_owned()}
fn notify(app:&AppHandle){let _=app.emit("clipforge:refresh",());}
fn alert(app:&AppHandle,message:&str){let _=app.emit("clipforge:notice",message.to_owned());}

#[tauri::command]
async fn snapshot(state:State<'_,AppState>)->Result<Snapshot,String>{
 let db=state.database.lock().await;
 let mut channels=db.channels()?;
 let counts=state.counts.lock().await;
 for c in &mut channels{c.messages=*counts.get(&c.id).unwrap_or(&0);}
 let auth=state.auth.lock().await;
 Ok(Snapshot{settings:db.settings()?,channels,highlights:db.highlights()?,monitoring:state.monitor.load(Ordering::Relaxed),authenticated_as:auth.user.as_ref().map(|u|u.display_name.clone())})
}
#[tauri::command]
async fn save_settings(state:State<'_,AppState>,settings:Settings)->Result<(),String>{
 if settings.threshold>100||settings.cooldown_seconds<15||settings.cooldown_seconds>3600{return Err("Некорректные настройки детектора".into())}
 if settings.auto_daily_limit==0||settings.auto_daily_limit>20{return Err("Дневной лимит: от 1 до 20 клипов".into())}
 if !["private","unlisted","public"].contains(&settings.auto_youtube_privacy.as_str())||!["MEDIA_UPLOAD","DIRECT_POST"].contains(&settings.auto_tiktok_mode.as_str()){return Err("Некорректный режим публикации".into())}
 if settings.auto_publish{
  if settings.demo_mode||!settings.auto_clip||!settings.auto_export||!settings.auto_publish_confirmed{return Err("Включи реальный Twitch, авто-клип, экспорт и подтверди права".into())}
  if settings.auto_targets.is_empty()||settings.auto_targets.len()>20{return Err("Для автопубликации выбери 1–20 профилей Upload-Post".into())}
  for target in &settings.auto_targets{upload_post::target_parts(target)?;}
  if !upload_post::configured().await{return Err("Сначала подключи API-ключ Upload-Post в Windows Credential Manager".into())}
 }
 state.database.lock().await.save_settings(&settings)
}
#[tauri::command]
async fn add_channel(state:State<'_,AppState>,login:String)->Result<Channel,String>{
 let settings=state.database.lock().await.settings()?;
 let channel=if settings.demo_mode {
   let clean=login.trim().trim_start_matches('@').to_lowercase();
   if clean.is_empty()||!clean.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'){return Err("Недопустимый ник стримера".into())}
   Channel{id:format!("demo:{}",clean),login:clean.clone(),display_name:clean,enabled:true,rights_confirmed:false,is_live:true,messages:0,last_score:0}
 }else{
   let auth=state.auth.lock().await;
   if auth.token.is_empty(){return Err("Сначала подключи Twitch в Настройках".into())}
   twitch::resolve_channel(&twitch_id(&settings),&auth.token,&login).await?
 };
 state.database.lock().await.save_channel(&channel)?;
 Ok(channel)
}
#[tauri::command]
async fn remove_channel(state:State<'_,AppState>,id:String)->Result<(),String>{state.database.lock().await.remove_channel(&id)}
#[tauri::command]
async fn update_channel(state:State<'_,AppState>,id:String,enabled:bool,rights_confirmed:bool)->Result<(),String>{
 let db=state.database.lock().await;
 let mut c=db.channels()?.into_iter().find(|c|c.id==id).ok_or("Канал не найден")?;
 c.enabled=enabled;c.rights_confirmed=rights_confirmed;db.save_channel(&c)
}
#[tauri::command]
async fn twitch_device_start(state:State<'_,AppState>)->Result<TwitchDevice,String>{
 let settings=state.database.lock().await.settings()?;
 let result=twitch::device_start(&twitch_id(&settings)).await?;
 state.auth.lock().await.device=Some(result.clone());Ok(result)
}
#[tauri::command]
async fn twitch_device_poll(state:State<'_,AppState>)->Result<Option<LoggedIn>,String>{
 let settings=state.database.lock().await.settings()?;
 let device=state.auth.lock().await.device.clone().ok_or("Сначала начни авторизацию")?;
 let result=twitch::device_poll(&twitch_id(&settings),&device.device_code).await?;
 if let Some((token,user))=result{
  {
    let mut auth=state.auth.lock().await;
    auth.token=token;auth.user=Some(user.clone());auth.device=None;
   }
   let mut live_settings=state.database.lock().await.settings()?;
   if live_settings.demo_mode {
    live_settings.demo_mode=false;
    state.database.lock().await.save_settings(&live_settings)?;
   }
  Ok(Some(user))
 }else{Ok(None)}
}
#[tauri::command]
async fn disconnect_twitch(state:State<'_,AppState>)->Result<(),String>{let mut a=state.auth.lock().await;a.token.clear();a.user=None;a.device=None;Ok(())}

async fn incoming(app:&AppHandle,channel_id:&str,user:&str,message:&str){
 let st=app.state::<AppState>();
 let settings=match st.database.lock().await.settings(){Ok(s)=>s,Err(_)=>return};
 {let mut counts=st.counts.lock().await;*counts.entry(channel_id.to_string()).or_insert(0)+=1;}
 let maybe=st.detector.lock().await.ingest(channel_id,user,message,Utc::now(),settings.cooldown_seconds,settings.threshold);
 if let Some(score)=maybe{
   let mut db=st.database.lock().await;
   let Some(ch)=db.channels().ok().and_then(|list|list.into_iter().find(|c|c.id==channel_id&&c.enabled))else{return};
   let length=scoring::duration_from_score(score.score);
   let title=format!("{} — яркий момент #shorts",ch.display_name);
   let mut h=Highlight{id:0,channel_id:ch.id.clone(),channel_login:ch.login.clone(),title,score:score.score,length,detected_at:Utc::now().to_rfc3339(),status:"detected".into(),clip_id:None,clip_url:None,source_path:None,output_path:None,explanation:Some(score.description)};
   if db.add_highlight(&mut h).is_err(){return}
   drop(db);notify(app);
   if settings.auto_clip&&ch.rights_confirmed&&!settings.demo_mode{
     let app=app.clone();let wait=scoring::post_seconds(score.score);
     tokio::spawn(async move{
       tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
       if let Err(e)=create_for_id(&app,h.id).await{alert(&app,&format!("Не удалось создать Twitch Clip: {}",e));}
     });
   }
 }
}
#[tauri::command]
async fn demo_burst(app:AppHandle)->Result<(),String>{
 let state=app.state::<AppState>();
 if !state.database.lock().await.settings()?.demo_mode{return Err("Тестовый всплеск доступен только в demo".into())}
 if !state.monitor.load(Ordering::Relaxed){return Err("Сначала включи мониторинг".into())}
 let channels=state.database.lock().await.channels()?;
 let ch=channels.iter().find(|c|c.enabled).ok_or("Добавь хотя бы одного стримера")?;
 for i in 0..24{incoming(&app,&ch.id,&format!("viewer{}",i),if i%3==0{"АХАХ WOW!!!"}else{"CLIP OMG!!!"}).await;}
 notify(&app);Ok(())
}

async fn monitor_task(app:AppHandle,channels:Vec<Channel>,client_id:String,token:String,reader_id:String,running:Arc<AtomicBool>){
 let subscriptions:Vec<Channel>=channels.into_iter().filter(|c|c.enabled&&!c.id.starts_with("demo:")).collect();
 while running.load(Ordering::Relaxed){
  match connect_async("wss://eventsub.wss.twitch.tv/ws").await{
   Ok((mut socket,_))=>{
    alert(&app,"Twitch EventSub подключён");
    while running.load(Ordering::Relaxed){
     let Some(Ok(msg))=socket.next().await else{break};
     if let tokio_tungstenite::tungstenite::Message::Text(text)=msg{
      let Ok(v)=serde_json::from_str::<Value>(&text)else{continue};
      let typ=v["metadata"]["message_type"].as_str().unwrap_or("");
      if typ=="session_welcome"{
       let sid=v["payload"]["session"]["id"].as_str().unwrap_or("");
       // Subscription must be created promptly after welcome; create in parallel.
       for c in &subscriptions{
        let cc=c.clone();let sid=sid.to_string();let ci=client_id.clone();let tk=token.clone();let rd=reader_id.clone();let ap=app.clone();
        tokio::spawn(async move{if let Err(e)=twitch::subscribe_chat(&ci,&tk,&sid,&rd,&cc.id).await{alert(&ap,&format!("Чат {}: {}",cc.login,e));}});
       }
      }else if typ=="notification" && v["payload"]["subscription"]["type"]=="channel.chat.message"{
       let event=&v["payload"]["event"];
       let id=event["broadcaster_user_id"].as_str().unwrap_or("");
       let user=event["chatter_user_id"].as_str().unwrap_or("viewer");
       let body=event["message"]["text"].as_str().unwrap_or("");
       incoming(&app,id,user,body).await;
      }else if typ=="session_reconnect"{
       // Reconnect by fresh socket; old subscriptions expire with the old session.
       break;
      }else if typ=="revocation"{alert(&app,"Twitch отозвал доступ к одному из чатов");}
     }
    }
   },Err(e)=>alert(&app,&format!("EventSub: {}",e))
  }
  if running.load(Ordering::Relaxed){tokio::time::sleep(std::time::Duration::from_secs(6)).await;}
 }
 running.store(false,Ordering::Relaxed);notify(&app);
}
#[tauri::command]
async fn start_monitor(app:AppHandle)->Result<(),String>{
 let state=app.state::<AppState>();
 if state.monitor.swap(true,Ordering::SeqCst){return Ok(())}
 let settings=state.database.lock().await.settings()?;
 let channels=state.database.lock().await.channels()?;
 if !channels.iter().any(|c|c.enabled){state.monitor.store(false,Ordering::Relaxed);return Err("Добавь и включи Twitch-канал".into())}
 if settings.demo_mode{notify(&app);return Ok(())}
 if channels.iter().any(|c|c.enabled && c.id.starts_with("demo:")){
  state.monitor.store(false,Ordering::Relaxed);
  return Err("После входа в Twitch удали демонстрационные каналы и добавь настоящие Twitch-каналы заново.".into())
 }
 let auth=state.auth.lock().await;
 let user=match auth.user.as_ref(){Some(u)=>u.user_id.clone(),None=>{state.monitor.store(false,Ordering::Relaxed);return Err("Подключи Twitch-аккаунт".into())}};
 let token=auth.token.clone();drop(auth);
 let running=state.monitor.clone();
 let task=tokio::spawn(monitor_task(app.clone(),channels,twitch_id(&settings),token,user,running));
 *state.worker.lock().await=Some(task);
 notify(&app);Ok(())
}
#[tauri::command]
async fn stop_monitor(app:AppHandle)->Result<(),String>{
 let state=app.state::<AppState>();
 state.monitor.store(false,Ordering::SeqCst);
 if let Some(task)=state.worker.lock().await.take(){task.abort();}
 state.detector.lock().await.reset();notify(&app);Ok(())
}
async fn create_for_id(app:&AppHandle,id:i64)->Result<(),String>{
 let state=app.state::<AppState>();
 let h=state.database.lock().await.highlight(id)?;
 if h.channel_id.starts_with("demo:"){return Err("Demo не создаёт настоящие клипы".into())}
 let settings=state.database.lock().await.settings()?;
 // A manually requested clip also waits for the post-event portion, up to 30 seconds.
 if let Ok(t)=chrono::DateTime::parse_from_rfc3339(&h.detected_at){
   let age=(Utc::now().fixed_offset()-t).num_seconds().max(0) as u64;
   let need=scoring::post_seconds(h.score);
   if age<need {tokio::time::sleep(std::time::Duration::from_secs(need-age)).await;}
 }
 let auth=state.auth.lock().await;let token=auth.token.clone();let user=auth.user.clone();drop(auth);
 if token.is_empty(){return Err("Twitch не авторизован".into())}
 let clip_id=if let Some(ref user)=user {
   match twitch::create_vod_clip(&twitch_id(&settings),&token,&h.channel_id,&user.user_id,&h.title,&h.detected_at,h.length,scoring::post_seconds(h.score)).await {
     Ok(id)=>id,
     Err(e)=>{
       let age=chrono::DateTime::parse_from_rfc3339(&h.detected_at).map(|t|(Utc::now().fixed_offset()-t).num_seconds()).unwrap_or(i64::MAX);
       if age>50 {return Err(format!("Точное VOD-окно недоступно ({}); поздно создавать live-клип",e))}
       twitch::create_clip(&twitch_id(&settings),&token,&h.channel_id,&h.title,h.length).await?
     }
   }
 }else{twitch::create_clip(&twitch_id(&settings),&token,&h.channel_id,&h.title,h.length).await?};
 let mut h=h;h.clip_id=Some(clip_id.clone());h.status="creating".into();state.database.lock().await.save_highlight(&h)?;notify(app);
 for _ in 0..12{
  tokio::time::sleep(std::time::Duration::from_secs(5)).await;
  if let Ok(Some(url))=twitch::clip_url(&twitch_id(&settings),&token,&clip_id).await{
    h.clip_url=Some(url);h.status="clip_ready".into();state.database.lock().await.save_highlight(&h)?;notify(app);
    // Download is possible only if this user is broadcaster/editor with the required scopes.
    if let Some(user)=user{
     let dest=dirs::video_dir().unwrap_or_else(std::env::temp_dir).join("ClipForge").join(format!("twitch_{}.mp4",clip_id));
     if twitch::download_clip(&twitch_id(&settings),&token,&h.channel_id,&user.user_id,&clip_id,&dest).await.is_ok(){
      h.source_path=Some(dest.to_string_lossy().to_string());h.status="downloaded".into();state.database.lock().await.save_highlight(&h)?;notify(app);
      if settings.auto_export{
       let input=h.source_path.clone().unwrap_or_default();let ss=settings.clone();let name=format!("{}_{}",h.channel_login,h.id);
       if let Ok(Ok(path))=tokio::task::spawn_blocking(move||export::render(&input,&ss,&name)).await{
        h.output_path=Some(path);h.status="exported".into();state.database.lock().await.save_highlight(&h)?;notify(app);
       }
      }
     }
    }
    return Ok(())
  }
 }
 h.status="clip_timeout".into();state.database.lock().await.save_highlight(&h)?;notify(app);
 Err("Клип не был готов за 60 секунд".into())
}
#[tauri::command]
async fn create_twitch_clip(app:AppHandle,id:i64)->Result<(),String>{create_for_id(&app,id).await}
#[tauri::command]
async fn attach_video(app:AppHandle,id:i64,path:String)->Result<(),String>{
 let p=PathBuf::from(&path);
 if !p.is_file(){return Err("Файл не найден".into())}
 if !["mp4","mov","mkv","webm"].contains(&p.extension().and_then(|x|x.to_str()).unwrap_or("").to_lowercase().as_str()){return Err("Разрешены MP4/MOV/MKV/WEBM".into())}
 let state=app.state::<AppState>();let mut h=state.database.lock().await.highlight(id)?;h.source_path=Some(path);h.status="source_attached".into();state.database.lock().await.save_highlight(&h)?;notify(&app);Ok(())
}
#[tauri::command]
async fn render_highlight(app:AppHandle,id:i64)->Result<String,String>{
 let state=app.state::<AppState>();let mut h=state.database.lock().await.highlight(id)?;
 let input=h.source_path.clone().ok_or("Сначала прикрепи исходное видео")?;
 let settings=state.database.lock().await.settings()?;
 let name=format!("{}_{}",h.channel_login,h.id);
 let result=tokio::task::spawn_blocking(move||export::render(&input,&settings,&name)).await.map_err(|e|e.to_string())??;
 h.output_path=Some(result.clone());h.status="exported".into();state.database.lock().await.save_highlight(&h)?;notify(&app);Ok(result)
}
#[tauri::command]
async fn analyze_highlight(app:AppHandle,id:i64,api_key:String)->Result<String,String>{
 let state=app.state::<AppState>();let mut h=state.database.lock().await.highlight(id)?;
 let path=h.source_path.clone().ok_or("Нет доступного видео")?;
 let settings=state.database.lock().await.settings()?;
 let (score,desc)=vision::analyze(&path,&settings,&api_key).await?;
 h.score=((h.score as u16*2+score as u16)/3) as u8;
 h.explanation=Some(format!("Vision: {} ({}); чат + кадр",desc,score));
 state.database.lock().await.save_highlight(&h)?;notify(&app);Ok(desc)
}

#[tauri::command]
async fn connected_accounts(state:State<'_,AppState>)->Result<Vec<social::AccountView>,String>{
 let mut all=state.accounts.list().await;
 if upload_post::configured().await{if let Ok(hosted)=upload_post::accounts().await{all.extend(hosted)}}
 Ok(all)
}
#[tauri::command]
async fn upload_post_status()->Result<upload_post::HostedState,String>{
 Ok(upload_post::HostedState{connected:upload_post::configured().await,provider:"Upload-Post".into(),error:None})
}
#[tauri::command]
async fn upload_post_save_key(api_key:String)->Result<(),String>{upload_post::save_key(api_key).await}
#[tauri::command]
async fn upload_post_forget_key()->Result<(),String>{upload_post::clear_key().await}
#[tauri::command]
async fn upload_post_connect(profile:String)->Result<String,String>{upload_post::connect_link(&profile).await}
#[tauri::command]
async fn upload_post_accounts()->Result<Vec<social::AccountView>,String>{upload_post::accounts().await}
#[tauri::command]
async fn youtube_oauth_start(app:AppHandle)->Result<String,String>{social::begin_youtube(app).await}
#[tauri::command]
async fn tiktok_oauth_start(app:AppHandle)->Result<String,String>{social::begin_tiktok(app).await}
#[tauri::command]
async fn disconnect_social_account(app:AppHandle,id:String)->Result<(),String>{
 let st=app.state::<AppState>();
 if id.starts_with("up:"){return Err("Отключай соцсети через панель Upload-Post".into())}
 if !social::disconnect_account(&st.accounts,&id).await?{return Err("Аккаунт не найден".into())}
 notify(&app);Ok(())
}
#[tauri::command]
async fn update_check()->Result<updates::UpdateStatus,String>{updates::check().await}
#[tauri::command]
async fn update_install(app:AppHandle)->Result<String,String>{updates::install(app).await}
#[derive(serde::Serialize)]
struct IntegrationDiagnostics {
 twitch_configured:bool,twitch_connected:bool,
 youtube_configured:bool,youtube_accounts:usize,
 tiktok_configured:bool,ffmpeg_ready:bool,update_feed_public:bool
}
#[tauri::command]
async fn integration_diagnostics(state:State<'_,AppState>)->Result<IntegrationDiagnostics,String> {
 let settings=state.database.lock().await.settings()?;
 let path=ffmpeg::executable(&settings.ffmpeg_path);
 let ffmpeg_ready=tokio::task::spawn_blocking(move|| {
  std::process::Command::new(&path).arg("-version").output()
   .map(|x|x.status.success()).unwrap_or(false)
 }).await.map_err(|e|e.to_string())?;
 let twitch_configured=!twitch_id(&settings).trim().is_empty();
 let twitch_connected=state.auth.lock().await.user.is_some();
 let youtube_configured=option_env!("CLIPFORGE_YOUTUBE_CLIENT_ID").is_some_and(|s|!s.trim().is_empty());
 let youtube_accounts=state.accounts.list().await.iter().filter(|a|a.platform=="youtube").count();
 Ok(IntegrationDiagnostics{
  twitch_configured,twitch_connected,youtube_configured,youtube_accounts,
  tiktok_configured:option_env!("CLIPFORGE_TIKTOK_BRIDGE_URL").is_some_and(|url|url.starts_with("https://")),ffmpeg_ready,update_feed_public:false
 })
}


#[derive(Clone,serde::Serialize)]
struct PublishOutcome {
 account_id:String,
 account_name:String,
 platform:String,
 state:String,
 detail:String,
}
struct PublishingGuard<'a>(&'a AtomicBool);
impl Drop for PublishingGuard<'_>{
 fn drop(&mut self){self.0.store(false,Ordering::Release);}
}
#[tauri::command]
async fn publish_batch(app:AppHandle,id:i64,account_ids:Vec<String>,title:String,privacy:String,confirmed:bool)->Result<Vec<PublishOutcome>,String>{
 if !confirmed{return Err("Перед публикацией подтверди права на ролик".into())}
 if title.trim().is_empty(){return Err("Введи название ролика".into())}
 if account_ids.is_empty()||account_ids.len()>20{return Err("Выбери от 1 до 20 аккаунтов".into())}
 let unique:std::collections::HashSet<&String>=account_ids.iter().collect();
 if unique.len()!=account_ids.len(){return Err("Один аккаунт нельзя указать дважды".into())}
 let st=app.state::<AppState>();
 if st.publishing.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){
  return Err("Публикация уже выполняется".into())
 }
 let _guard=PublishingGuard(&st.publishing);
 let h=st.database.lock().await.highlight(id)?;
 let path=h.output_path.as_deref().ok_or("Сначала экспортируй вертикальный MP4")?.to_owned();
 let file=tokio::fs::metadata(&path).await.map_err(|e|format!("Видео не найдено: {e}"))?;
 if !file.is_file()||file.len()==0{return Err("Экспортированный файл пуст или недоступен".into())}
 let mut available=st.accounts.list().await;
 if let Ok(hosted)=upload_post::accounts().await{available.extend(hosted)}
 let settings=st.database.lock().await.settings()?;
 let mut results=Vec::with_capacity(account_ids.len());
 let mut youtube_uploaded=false;
 let mut tiktok_sent=false;
 for account_id in &account_ids {
  let info=available.iter().find(|a|&a.id==account_id);
  let name=info.map(|a|a.name.clone()).unwrap_or_else(||"Отключённый аккаунт".into());
  let platform=info.map(|a|a.platform.clone()).unwrap_or_else(||"unknown".into());
  let progress=PublishOutcome{account_id:account_id.clone(),account_name:name.clone(),platform:platform.clone(),state:"uploading".into(),detail:"Загружаем видео".into()};
  let _=app.emit("clipforge:publish-progress",progress);
  let response:Result<(String,String),String>=async {
   if account_id.starts_with("up:"){
    let posted=upload_post::send_video(&path,account_id,&title,&privacy,&settings.auto_tiktok_mode).await?;
    return Ok((posted.state,posted.detail))
   }
   let (kind,token)=social::token_for(&st.accounts,account_id).await?;
   let link=match kind.as_str(){
    "youtube"=>publisher::youtube_upload(&path,&token,&title,&privacy).await?,
    "tiktok"=>publisher::tiktok_draft(&path,&token).await?,
    _=>return Err("Неподдерживаемая платформа".into()),
   };
   Ok((if kind=="tiktok"{"draft"}else{"success"}.into(),format!("Отправка: {link}")))
  }.await;
  let result=match response{
   Ok((state,detail))=>{
    if platform=="youtube"{youtube_uploaded=true}
    if platform=="tiktok"{tiktok_sent=true}
    PublishOutcome{account_id:account_id.clone(),account_name:name,platform,state,detail}
   }
   Err(message)=>PublishOutcome{account_id:account_id.clone(),account_name:name,platform,
     state:"error".into(),detail:message.chars().take(400).collect()},
  };
  let _=app.emit("clipforge:publish-progress",result.clone());
  results.push(result);
 }
 if youtube_uploaded||tiktok_sent{
  let mut record=h;
  record.status=if youtube_uploaded{"published_youtube"}else{"tiktok_draft"}.into();
  st.database.lock().await.save_highlight(&record)?;
  notify(&app);
 }
 Ok(results)
}

#[tauri::command]
async fn publish_highlight(app:AppHandle,id:i64,account_id:String,title:String,privacy:String,confirmed:bool)->Result<String,String>{
 if !confirmed{return Err("Публикация требует подтверждения пользователя".into())}
 let state=app.state::<AppState>();let mut h=state.database.lock().await.highlight(id)?;
 let path=h.output_path.clone().ok_or("Сначала сделай вертикальный экспорт")?;
 let (platform,access_token)=social::token_for(&state.accounts,&account_id).await?;
 let response=match platform.as_str(){
 "youtube"=>publisher::youtube_upload(&path,&access_token,&title,&privacy).await?,
 "tiktok"=>publisher::tiktok_draft(&path,&access_token).await?,
 _=>return Err("Неизвестная платформа".into())
 };
 h.status=if platform=="youtube"{"published_youtube"}else{"tiktok_draft"}.into();
 state.database.lock().await.save_highlight(&h)?;notify(&app);Ok(response)
}

pub fn run(){
 let database=db::Database::open().expect("Cannot open ClipForge SQLite database");
 tauri::Builder::default().plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_opener::init())
 .manage(AppState{database:Mutex::new(database),auth:Mutex::new(Auth{token:String::new(),user:None,device:None}),detector:Mutex::new(Detector::default()),counts:Mutex::new(HashMap::new()),monitor:Arc::new(AtomicBool::new(false)),worker:Mutex::new(None),accounts:social::Accounts::new(),publishing:AtomicBool::new(false)})
 .invoke_handler(tauri::generate_handler![snapshot,save_settings,add_channel,remove_channel,update_channel,twitch_device_start,twitch_device_poll,disconnect_twitch,start_monitor,stop_monitor,demo_burst,create_twitch_clip,attach_video,render_highlight,analyze_highlight,publish_highlight,publish_batch,connected_accounts,upload_post_status,upload_post_save_key,upload_post_forget_key,upload_post_connect,upload_post_accounts,youtube_oauth_start,tiktok_oauth_start,disconnect_social_account,update_check,update_install,integration_diagnostics])
 .run(tauri::generate_context!()).expect("Tauri startup failed");
}
