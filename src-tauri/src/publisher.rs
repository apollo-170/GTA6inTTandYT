use reqwest::header::{CONTENT_LENGTH,CONTENT_RANGE,CONTENT_TYPE,LOCATION};
use serde_json::{json,Value};
use tokio::io::AsyncReadExt;

pub async fn youtube_upload(file:&str,token:&str,title:&str,privacy:&str)->Result<String,String>{
 if !["private","unlisted","public"].contains(&privacy){return Err("Некорректный YouTube privacyStatus".into())}
 if token.trim().is_empty(){return Err("Нужен OAuth access token с youtube.upload".into())}
 let data=tokio::fs::read(file).await.map_err(|e|e.to_string())?;
 let client=reqwest::Client::new();
 let meta=json!({"snippet":{"title":title.chars().take(100).collect::<String>(),"description":"#shorts #twitch","categoryId":"20"},"status":{"privacyStatus":privacy}});
 let init=client.post("https://www.googleapis.com/upload/youtube/v3/videos?uploadType=resumable&part=snippet,status")
  .bearer_auth(token).header("X-Upload-Content-Type","video/mp4").header("X-Upload-Content-Length",data.len().to_string()).json(&meta).send().await.map_err(|e|e.to_string())?;
 if !init.status().is_success(){return Err(format!("YouTube init HTTP {}: {}",init.status(),init.text().await.unwrap_or_default()))}
 let upload_url=init.headers().get(LOCATION).ok_or("YouTube не вернул Location")?.to_str().map_err(|e|e.to_string())?.to_string();
 if !upload_url.starts_with("https://www.googleapis.com/"){return Err("Неподдерживаемый upload URL".into())}
 let end=client.put(&upload_url).header(CONTENT_TYPE,"video/mp4").header(CONTENT_LENGTH,data.len().to_string()).body(data).send().await.map_err(|e|e.to_string())?;
 let status=end.status();let body=end.text().await.map_err(|e|e.to_string())?;
 if !status.is_success(){return Err(format!("YouTube upload {}: {}",status,body.chars().take(600).collect::<String>()))}
 let v:Value=serde_json::from_str(&body).map_err(|e|e.to_string())?;
 let id=v["id"].as_str().ok_or("YouTube не вернул ID ролика")?;
 Ok(format!("https://www.youtube.com/watch?v={}",id))
}

// Official TikTok Inbox Upload: user completes publishing inside TikTok.
pub async fn tiktok_draft(file:&str,token:&str)->Result<String,String>{
 if token.trim().is_empty(){return Err("Нужен TikTok OAuth token с video.upload".into())}
 let size=tokio::fs::metadata(file).await.map_err(|e|e.to_string())?.len();
 if size==0{return Err("Пустой файл".into())}
 let chunk_size=10_000_000_u64;
 let count=size.div_ceil(chunk_size);
 let client=reqwest::Client::new();
 let init=client.post("https://open.tiktokapis.com/v2/post/publish/inbox/video/init/").bearer_auth(token)
  .json(&json!({"source_info":{"source":"FILE_UPLOAD","video_size":size,"chunk_size":chunk_size,"total_chunk_count":count}}))
  .send().await.map_err(|e|e.to_string())?;
 let status=init.status();let v:Value=init.json().await.map_err(|e|e.to_string())?;
 if !status.is_success()||v["error"]["code"].as_str()!=Some("ok") {return Err(format!("TikTok init {}: {}",status,v["error"]["message"].as_str().unwrap_or("error")))}
 let upload=v["data"]["upload_url"].as_str().ok_or("TikTok не вернул URL")?;
 if !upload.starts_with("https://open-upload.tiktokapis.com/"){return Err("Недопустимый TikTok upload URL".into())}
 let mut f=tokio::fs::File::open(file).await.map_err(|e|e.to_string())?;
 for i in 0..count {
  let start=i*chunk_size;let length=(size-start).min(chunk_size);
  let mut bytes=vec![0u8;length as usize];f.read_exact(&mut bytes).await.map_err(|e|e.to_string())?;
  let res=client.put(upload).header(CONTENT_TYPE,"video/mp4").header(CONTENT_RANGE,format!("bytes {}-{}/{}",start,start+length-1,size))
   .header(CONTENT_LENGTH,length.to_string()).body(bytes).send().await.map_err(|e|e.to_string())?;
  if !res.status().is_success(){return Err(format!("TikTok upload HTTP {}: {}",res.status(),res.text().await.unwrap_or_default()))}
 }
 Ok(v["data"]["publish_id"].as_str().unwrap_or("draft queued").to_string())
}
