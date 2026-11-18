use serde::{Serialize,Deserialize};

#[derive(Clone,Serialize,Deserialize)]
pub struct Channel {
  pub id: String,
  pub login: String,
  pub display_name: String,
  pub enabled: bool,
  pub rights_confirmed: bool,
  pub is_live: bool,
  pub messages: u64,
  pub last_score: u8,
}
#[derive(Clone,Serialize,Deserialize)]
pub struct Highlight {
  pub id: i64,
  pub channel_id: String,
  pub channel_login: String,
  pub title: String,
  pub score: u8,
  pub length: u32,
  pub detected_at: String,
  pub status: String,
  pub clip_id: Option<String>,
  pub clip_url: Option<String>,
  pub source_path: Option<String>,
  pub output_path: Option<String>,
  pub explanation: Option<String>,
}
#[derive(Clone,Serialize,Deserialize)]
pub struct Settings {
 #[serde(default)] pub auto_publish:bool,
 #[serde(default)] pub auto_publish_confirmed:bool,
 #[serde(default)] pub auto_targets:Vec<String>,
 #[serde(default="default_youtube_privacy")] pub auto_youtube_privacy:String,
 #[serde(default="default_tiktok_mode")] pub auto_tiktok_mode:String,
 #[serde(default="default_daily_limit")] pub auto_daily_limit:u8,
  pub client_id: String,
  pub demo_mode: bool,
  pub auto_clip: bool,
  pub auto_export: bool,
  pub threshold: u8,
  pub cooldown_seconds: u64,
  pub output_dir: String,
  pub banner_path: String,
  pub ffmpeg_path: String,
  pub vision_endpoint: String,
  pub vision_model: String,
}
fn default_youtube_privacy()->String{"private".into()}
fn default_tiktok_mode()->String{"MEDIA_UPLOAD".into()}
fn default_daily_limit()->u8{5}
impl Default for Settings {
  fn default() -> Self { Self{auto_publish:false,auto_publish_confirmed:false,auto_targets:Vec::new(),auto_youtube_privacy:default_youtube_privacy(),auto_tiktok_mode:default_tiktok_mode(),auto_daily_limit:default_daily_limit(),client_id:String::new(),demo_mode:true,auto_clip:false,auto_export:false,threshold:68,cooldown_seconds:120,output_dir:String::new(),banner_path:String::new(),ffmpeg_path:"ffmpeg".into(),vision_endpoint:"https://api.openai.com/v1/chat/completions".into(),vision_model:String::new()} }
}
#[derive(Serialize)]
pub struct Snapshot {pub settings:Settings,pub channels:Vec<Channel>,pub highlights:Vec<Highlight>,pub monitoring:bool,pub authenticated_as:Option<String>}
#[derive(Clone,Serialize)]
pub struct TwitchDevice {pub device_code:String,pub user_code:String,pub verification_uri:String,pub interval:u64}
#[derive(Clone,Serialize)]
pub struct LoggedIn {pub display_name:String,pub user_id:String}
