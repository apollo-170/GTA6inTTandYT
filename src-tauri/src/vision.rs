use std::path::Path;
use std::process::Command;
use base64::Engine;
use serde_json::{json,Value};
use crate::models::Settings;

// Optional post-clip semantic vision; requires an authorized video file and a user-supplied key.
pub async fn analyze(input:&str,settings:&Settings,key:&str)->Result<(u8,String),String>{
 if key.is_empty()||settings.vision_model.trim().is_empty(){return Err("Укажи модель Vision и API key".into())}
 let ep=&settings.vision_endpoint;
 if !(ep.starts_with("https://")||ep.starts_with("http://127.0.0.1:")||ep.starts_with("http://localhost:")){return Err("Vision endpoint должен использовать HTTPS или localhost".into())}
 if !Path::new(input).is_file(){return Err("Видео не найдено".into())}
 let tmp=std::env::temp_dir().join(format!("clipforge_frame_{}.jpg",std::process::id()));
 let ffmpeg=crate::ffmpeg::executable(&settings.ffmpeg_path);
 let result=Command::new(&ffmpeg).args(["-hide_banner","-loglevel","error","-y","-ss","00:00:02","-i",input,"-frames:v","1","-vf","scale=640:-2"]).arg(&tmp).output().map_err(|e|e.to_string())?;
 if !result.status.success(){return Err(format!("Ошибка извлечения кадра: {}",String::from_utf8_lossy(&result.stderr)))}
 let bytes=std::fs::read(&tmp).map_err(|e|e.to_string())?;let _=std::fs::remove_file(&tmp);
 let b64=base64::engine::general_purpose::STANDARD.encode(bytes);
 let payload=json!({"model":settings.vision_model,"max_tokens":180,"temperature":0.2,"messages":[{"role":"user","content":[
   {"type":"text","text":"Assess this screenshot from a short gaming stream highlight. Return strictly JSON with integer score (0-100) for visual intensity and a short Russian explanation under 120 characters. Do not infer unseen actions."},
   {"type":"image_url","image_url":{"url":format!("data:image/jpeg;base64,{}",b64)}}
 ]}]});
 let res=reqwest::Client::builder().timeout(std::time::Duration::from_secs(50)).build().map_err(|e|e.to_string())?
 .post(ep).bearer_auth(key).json(&payload).send().await.map_err(|e|e.to_string())?;
 let status=res.status();let v:Value=res.json().await.map_err(|e|e.to_string())?;
 if !status.is_success(){return Err(format!("Vision API {}: {}",status,v["error"]["message"].as_str().unwrap_or("request failed")))}
 let s=v["choices"][0]["message"]["content"].as_str().ok_or("Vision API вернул пустой ответ")?;
 let clean=s.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
 let j:Value=serde_json::from_str(clean).map_err(|_|format!("Модель вернула не JSON: {}",s.chars().take(160).collect::<String>()))?;
 Ok((j["score"].as_u64().unwrap_or(50).min(100) as u8,j["explanation"].as_str().unwrap_or("Кадр обработан").into()))
}
