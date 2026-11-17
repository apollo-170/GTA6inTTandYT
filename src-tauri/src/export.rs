use crate::models::Settings;
use std::path::{Path,PathBuf};
use std::process::{Command,Stdio};

pub fn render(input:&str,settings:&Settings,name:&str)->Result<String,String>{
 let src=Path::new(input);
 if !src.is_file(){return Err("Исходный видеофайл не найден".into())}
 let out_dir=if settings.output_dir.trim().is_empty(){dirs::video_dir().unwrap_or_else(std::env::temp_dir).join("ClipForge") }else{PathBuf::from(&settings.output_dir)};
 std::fs::create_dir_all(&out_dir).map_err(|e|e.to_string())?;
 let safe:String=name.chars().map(|c|if c.is_ascii_alphanumeric()||c=='-'||c=='_' {c}else{'_'}).take(80).collect();
 let dest=out_dir.join(format!("{}_shorts.mp4",safe));
 let temp=out_dir.join(format!("{}_shorts.tmp.mp4",safe));
 let banner=if settings.banner_path.trim().is_empty(){None}else{let p=Path::new(&settings.banner_path);if !p.is_file(){return Err("Баннер не найден".into())}Some(p)};
 let vf_background="[0:v]split=2[bg][fg];[bg]scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920,boxblur=15:3[blur];[fg]scale=1080:1440:force_original_aspect_ratio=decrease,setsar=1[front];[blur][front]overlay=(W-w)/2:(H-h)/2[base]";
 let filter=if banner.is_some(){format!("{};[1:v]scale=1080:-1[bn];[base][bn]overlay=0:0:format=auto[vout]",vf_background)}else{format!("{};[base]null[vout]",vf_background)};
 let ffmpeg=crate::ffmpeg::executable(&settings.ffmpeg_path);
 let mut cmd=Command::new(&ffmpeg);
 cmd.args(["-hide_banner","-loglevel","error","-y","-i"]).arg(src);
 if let Some(b)=banner {cmd.args(["-loop","1","-i"]).arg(b);}
 cmd.args(["-filter_complex",&filter,"-map","[vout]","-map","0:a?","-c:v","libx264","-preset","veryfast","-crf","22","-pix_fmt","yuv420p","-r","30","-c:a","aac","-b:a","160k","-movflags","+faststart","-t","60","-shortest"]);
 cmd.arg(&temp).stdout(Stdio::null()).stderr(Stdio::piped());
 let out=cmd.output().map_err(|e|format!("Не найден FFmpeg (или ошибка запуска): {}",e))?;
 if !out.status.success(){let _=std::fs::remove_file(&temp);return Err(format!("FFmpeg: {}",String::from_utf8_lossy(&out.stderr).chars().take(800).collect::<String>()))}
 if dest.exists(){std::fs::remove_file(&dest).map_err(|e|e.to_string())?}
 std::fs::rename(&temp,&dest).map_err(|e|e.to_string())?;
 Ok(dest.to_string_lossy().to_string())
}
