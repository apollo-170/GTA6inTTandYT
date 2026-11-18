use crate::{AppState,PublishingGuard};
use serde::Serialize;
use serde_json::Value;
use std::{path::{Path,PathBuf},process::Command,sync::atomic::Ordering};
use tauri::{AppHandle,Manager};

#[derive(Serialize)]
pub struct VodInfo { pub path:String, pub duration:f64, pub size:u64, pub name:String }
fn validate(path:&str)->Result<PathBuf,String>{
 let p=PathBuf::from(path);
 if !p.is_file(){return Err("Выбери существующий видеофайл".into())}
 if !["mp4","mkv","mov","webm"].contains(&p.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase().as_str()){return Err("Поддерживаются MP4, MKV, MOV и WEBM".into())}
 Ok(p)
}
fn tools(app:&AppHandle)->Result<(PathBuf,PathBuf),String>{
 let settings=app.state::<AppState>().database.try_lock().map_err(|_|"Настройки заняты; повтори действие".to_owned())?.settings()?;
 let ffmpeg=crate::ffmpeg::executable(&settings.ffmpeg_path);
 let ffprobe=ffmpeg.clone();
 Ok((ffmpeg,ffprobe))
}
fn probe_duration(ffprobe:&Path,path:&Path)->Result<f64,String>{
 let out=Command::new(ffprobe).arg("-hide_banner").arg("-i").arg(path).output().map_err(|e|format!("Не удалось запустить FFmpeg: {e}"))?;
 let detail=String::from_utf8_lossy(&out.stderr);
 let duration=detail.split("Duration: ").nth(1).and_then(|s|s.split(',').next()).ok_or("FFmpeg не обнаружил длительность VOD")?;
 let parts:Vec<f64>=duration.trim().split(':').map(|s|s.parse::<f64>().unwrap_or(f64::NAN)).collect();
 if parts.len()!=3{return Err("Не удалось прочитать длительность файла".into())}
 let v=parts[0]*3600.0+parts[1]*60.0+parts[2];
 if !v.is_finite()||v<=0.0{return Err("Некорректная длительность файла".into())}
 Ok(v)
}
#[tauri::command]
pub async fn vod_info(app:AppHandle,path:String)->Result<VodInfo,String>{
 let p=validate(&path)?;
 let size=std::fs::metadata(&p).map_err(|e|e.to_string())?.len();
 let (_ffmpeg,ffprobe)=tools(&app)?;
 let duration=tokio::task::spawn_blocking({let p=p.clone();move||probe_duration(&ffprobe,&p)}).await.map_err(|e|e.to_string())??;
 Ok(VodInfo{path,name:p.file_name().unwrap_or_default().to_string_lossy().to_string(),duration,size})
}
#[tauri::command]
pub async fn vod_frame(app:AppHandle,path:String,at:f64)->Result<String,String>{
 let p=validate(&path)?;
 let (ffmpeg,ffprobe)=tools(&app)?;
 let duration=tokio::task::spawn_blocking({let p=p.clone();move||probe_duration(&ffprobe,&p)}).await.map_err(|e|e.to_string())??;
 if !at.is_finite()||at<0.0||at>=duration{return Err("Время кадра за пределами видео".into())}
 let output=tokio::task::spawn_blocking(move||Command::new(ffmpeg).args(["-hide_banner","-loglevel","error","-ss",&format!("{at:.3}"),"-i"]).arg(p).args(["-frames:v","1","-vf","scale=640:-2","-f","image2pipe","-vcodec","mjpeg","pipe:1"]).output().map_err(|e|e.to_string())).await.map_err(|e|e.to_string())??;
 if !output.status.success()||output.stdout.len()>5_000_000{return Err("Не удалось получить кадр предпросмотра".into())}
 use base64::Engine;
 Ok(format!("data:image/jpeg;base64,{}",base64::engine::general_purpose::STANDARD.encode(output.stdout)))
}
#[tauri::command]
pub async fn vod_cut(app:AppHandle,path:String,start:f64,end:f64,vertical:bool)->Result<String,String>{
 let p=validate(&path)?;
 let (ffmpeg,ffprobe)=tools(&app)?;
 let duration=tokio::task::spawn_blocking({let p=p.clone();move||probe_duration(&ffprobe,&p)}).await.map_err(|e|e.to_string())??;
 if !start.is_finite()||!end.is_finite()||start<0.0||end<=start||end>duration+0.1{return Err("Неверные границы фрагмента".into())}
 if vertical&&end-start>180.0{return Err("Для вертикального клипа выбери максимум 180 секунд".into())}
 let dir=dirs::video_dir().unwrap_or_else(std::env::temp_dir).join("ClipForge").join("VOD");
 std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
 let stamp=chrono::Utc::now().timestamp_millis();
 let dest=dir.join(format!("clip_{stamp}_{}.mp4",if vertical{"vertical"}else{"wide"}));
 let temp=dir.join(format!("clip_{stamp}.tmp.mp4"));
 let duration_s=format!("{:.3}",end-start);
 let output=tokio::task::spawn_blocking(move||{
  let mut c=Command::new(ffmpeg);
  c.args(["-hide_banner","-loglevel","error","-y","-ss",&format!("{start:.3}"),"-i"]).arg(&p).args(["-t",&duration_s]);
  if vertical{
   c.args(["-filter_complex","[0:v]split=2[bg][fg];[bg]scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920,boxblur=15:3[blur];[fg]scale=1080:1440:force_original_aspect_ratio=decrease,setsar=1[front];[blur][front]overlay=(W-w)/2:(H-h)/2[v]","-map","[v]","-map","0:a?"]);
  }else{c.args(["-map","0:v:0","-map","0:a?"]);}
  let o=c.args(["-c:v","libx264","-preset","veryfast","-crf","23","-pix_fmt","yuv420p","-c:a","aac","-b:a","160k","-movflags","+faststart"]).arg(&temp).output().map_err(|e|e.to_string())?;
  if !o.status.success(){let _=std::fs::remove_file(&temp);return Err(String::from_utf8_lossy(&o.stderr).chars().take(600).collect())}
  std::fs::rename(&temp,&dest).map_err(|e|e.to_string())?;
  Ok(dest.to_string_lossy().to_string())
 }).await.map_err(|e|e.to_string())??;
 Ok(output)
}
#[tauri::command]
pub async fn vod_youtube_upload(app:AppHandle,path:String,account_ids:Vec<String>,title:String,privacy:String,confirmed:bool)->Result<Vec<String>,String>{
 if !confirmed{return Err("Подтверди права на видео и публикацию".into())}
 if title.trim().is_empty()||title.chars().count()>100{return Err("Название YouTube должно содержать 1–100 символов".into())}
 if !["private","unlisted","public"].contains(&privacy.as_str()){return Err("Некорректная приватность YouTube".into())}
 if account_ids.is_empty()||account_ids.len()>20{return Err("Выбери 1–20 YouTube каналов".into())}
 let unique:std::collections::HashSet<&String>=account_ids.iter().collect();
 if unique.len()!=account_ids.len(){return Err("YouTube аккаунты должны быть уникальны".into())}
 let p=validate(&path)?;
 if p.extension().and_then(|s|s.to_str()).unwrap_or("").to_lowercase()!="mp4"{return Err("Для публикации целого стрима нужен MP4. Для нарезок доступны другие форматы.".into())}
 let st=app.state::<AppState>();
 if st.publishing.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){return Err("Уже выполняется публикация".into())}
 let _guard=PublishingGuard(&st.publishing);
 let mut results=Vec::new();
 for id in account_ids{
  if !id.starts_with("youtube:"){results.push(format!("{id}: не YouTube аккаунт"));continue}
  let result=async{
   let (platform,token)=crate::social::token_for(&st.accounts,&id).await?;
   if platform!="youtube"{return Err("Не YouTube аккаунт".into())}
   crate::publisher::youtube_upload_streaming(p.to_str().ok_or("Некорректный путь файла")?,&token,&title,&privacy).await
  }.await;
  results.push(format!("{}: {}",id,match result{Ok(link)=>link,Err(e)=>format!("Ошибка: {e}")}));
 }
 Ok(results)
}
