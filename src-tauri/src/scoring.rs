use chrono::{DateTime,Utc};
use std::collections::{HashMap,HashSet,VecDeque};

#[derive(Clone)]
struct Message {at:DateTime<Utc>,user:String,excited:bool}
#[derive(Default)]
pub struct Detector {history:HashMap<String,VecDeque<Message>>, last_trigger:HashMap<String,DateTime<Utc>>}
#[derive(Debug,Clone)]
pub struct Score {pub score:u8,pub count:usize,pub description:String}
impl Detector {
 pub fn ingest(&mut self,channel:&str,user:&str,text:&str,now:DateTime<Utc>,cooldown:u64,threshold:u8)->Option<Score>{
  let h=self.history.entry(channel.to_string()).or_default();
  let excited=is_excited(text);
  h.push_back(Message{at:now,user:user.to_string(),excited});
  while h.front().is_some_and(|m| (now-m.at).num_seconds()>70){h.pop_front();}
  let recent:Vec<&Message>=h.iter().filter(|m|(now-m.at).num_seconds()<=10).collect();
  let n=recent.len();
  let older=h.iter().filter(|m|{let age=(now-m.at).num_seconds();age>10&&age<=70}).count();
  let unique=recent.iter().map(|m|m.user.as_str()).collect::<HashSet<_>>().len();
  let excited_count=recent.iter().filter(|m|m.excited).count();
  // Estimate recent messages/minute and compare to prior 60 sec. Bound growth to avoid division by zero.
  let rate_recent=n as f64*6.0;
  let rate_older=older as f64;
  let growth=(rate_recent/(rate_older+6.0)).clamp(0.0,8.0);
  let activity=(rate_recent/3.0).min(36.0);
  let growth_score=((growth-1.0).max(0.0)*7.0).min(25.0);
  let excitement=(excited_count as f64*3.5).min(24.0);
  let diversity=(unique as f64*1.6).min(15.0);
  let score=(activity+growth_score+excitement+diversity).round().min(100.0) as u8;
  if n<7 || unique<3 || score<threshold {return None;}
  if self.last_trigger.get(channel).is_some_and(|last|(now-*last).num_seconds() < cooldown as i64){return None;}
  self.last_trigger.insert(channel.into(),now);
  Some(Score{score,count:n,description:format!("{} сообщений за 10 с, {} участников, {} эмоциональных реакций",n,unique,excited_count)})
 }
 pub fn reset(&mut self){self.history.clear();self.last_trigger.clear()}
}
fn is_excited(s:&str)->bool{
 let t=s.to_lowercase();
 ["ахах","хаха","клип","clip","omg","wtf","wow","чего","чтооо","ооо","пиздец","no way","insane","gg","lol","crazy","имба","легенда","???","!!!","🔥","😂","💀"].iter().any(|word|t.contains(word)) || s.chars().filter(|&c|c=='!').count()>=2 || (s.chars().count()>3 && s.chars().filter(|c|c.is_uppercase()).count()>=5)
}
pub fn duration_from_score(score:u8)->u32 {if score>=90{60}else if score>=80{55}else if score>=70{45}else{35}}
pub fn post_seconds(score:u8)->u64 {if score>=90{30}else if score>=80{25}else{18}}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]fn clamp(){assert_eq!(duration_from_score(100),60);assert_eq!(duration_from_score(0),35)}
 #[test]fn spike_detection(){let mut d=Detector::default();let now=Utc::now();let mut got=0;for i in 0..25 {if let Some(s)=d.ingest("x",&format!("user{}",i),"WOW!!!",now,120,68){if s.score>=68{got+=1}}} assert!(got<=1);}
 #[test]fn no_single_user_trigger(){let mut d=Detector::default();let now=Utc::now();for _ in 0..50 {assert!(d.ingest("x","bot","!!!",now,120,68).is_none());}}
}
