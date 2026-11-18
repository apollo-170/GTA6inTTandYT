use crate::{AppState,PublishOutcome,PublishingGuard};
use chrono::{Duration,Utc};
use std::sync::atomic::Ordering;
use tauri::{AppHandle,Emitter,Manager};

/// Runs only for a real Twitch clip with channel publishing rights and the owner's
/// explicit automatic-publishing consent. Uses the hosted Upload-Post service.
pub async fn process(app:AppHandle,id:i64)->Result<(),String>{
 let st=app.state::<AppState>();
 let config=st.database.lock().await.settings()?;
 if !config.auto_publish{return Ok(())}
 if config.demo_mode||!config.auto_clip||!config.auto_export||!config.auto_publish_confirmed{
  return Err("Автопубликация запрещена настройками: подтверди права и включи экспорт".into())
 }
 let h=st.database.lock().await.highlight(id)?;
 let permitted=st.database.lock().await.channels()?.iter()
  .any(|c|c.id==h.channel_id&&c.enabled&&c.rights_confirmed);
 if !permitted{return Err("Нет подтверждённого разрешения на публикацию этого Twitch-канала".into())}
 let path=h.output_path.clone().ok_or("Не найден экспортированный MP4")?;
 if config.auto_targets.is_empty(){return Err("Не выбраны аккаунты для автозагрузки".into())}
 let mut acquired=false;
 for _ in 0..24{
  if st.publishing.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_ok(){acquired=true;break}
  tokio::time::sleep(std::time::Duration::from_secs(5)).await;
 }
 if !acquired{return Err("Очередь публикации занята. Видео сохранено для ручной отправки".into())}
 let _guard=PublishingGuard(&st.publishing);
 let since=Utc::now()-Duration::hours(24);
 let today=st.database.lock().await.highlights()?.iter().filter(|item|
  ["auto_sending","auto_sent","auto_partial"].contains(&item.status.as_str())
  && chrono::DateTime::parse_from_rfc3339(&item.detected_at)
    .is_ok_and(|t|t.with_timezone(&Utc)>since)
 ).count();
 if today>=config.auto_daily_limit as usize{
  return Err(format!("Дневной лимит {} автоклипов достигнут",config.auto_daily_limit))
 }
 let mut clip=h;
 clip.status="auto_sending".into();
 st.database.lock().await.save_highlight(&clip)?;
 crate::notify(&app);
 let mut success=0usize;
 let mut failed=0usize;
 for target in &config.auto_targets{
  let (profile,platform)=crate::upload_post::target_parts(target)?;
  let label=profile.to_string();
  let _=app.emit("clipforge:publish-progress",PublishOutcome{
   account_id:target.clone(),account_name:label.clone(),
   platform:platform.into(),state:"uploading".into(),detail:"Автоматическая загрузка".into()
  });
  let uploaded=crate::upload_post::send_video(&path,target,&clip.title,
   &config.auto_youtube_privacy,&config.auto_tiktok_mode).await;
  let result=match uploaded{
   Ok(res)=>{success+=1;PublishOutcome{
    account_id:target.clone(),account_name:label,platform:platform.into(),
    state:res.state,detail:res.detail
   }},
   Err(err)=>{failed+=1;PublishOutcome{
    account_id:target.clone(),account_name:label,platform:platform.into(),
    state:"error".into(),detail:err.chars().take(400).collect()
   }},
  };
  let _=app.emit("clipforge:publish-progress",result);
 }
 clip.status=if success==0{"auto_failed"}else if failed>0{"auto_partial"}else{"auto_sent"}.into();
 st.database.lock().await.save_highlight(&clip)?;
 crate::notify(&app);
 let _=app.emit("clipforge:connected",format!(
  "Автозагрузка: {success} из {} профилей приняли видео (в том числе возможны задачи в очереди). Проверь статусы в Upload-Post.",config.auto_targets.len()));
 Ok(())
}
