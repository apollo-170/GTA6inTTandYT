use crate::models::{Channel,Highlight,Settings};
use rusqlite::{params,Connection};

pub struct Database(pub Connection);
impl Database {
 pub fn open() -> Result<Self,String> {
  let base=dirs::data_local_dir().unwrap_or_else(||std::env::temp_dir()).join("ClipForge");
  std::fs::create_dir_all(&base).map_err(|e|e.to_string())?;
  let db=Connection::open(base.join("clipforge.sqlite3")).map_err(|e|e.to_string())?;
  db.execute_batch("PRAGMA journal_mode=WAL;
    CREATE TABLE IF NOT EXISTS config (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS channels (id TEXT PRIMARY KEY, json TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS highlights (id INTEGER PRIMARY KEY AUTOINCREMENT, channel_id TEXT NOT NULL, json TEXT NOT NULL);
    CREATE INDEX IF NOT EXISTS idx_highlights_channel ON highlights(channel_id);").map_err(|e|e.to_string())?;
  Ok(Self(db))
 }
 pub fn settings(&self)->Result<Settings,String> {
  let res=self.0.query_row("SELECT json FROM config WHERE id=1",[],|r|r.get::<_,String>(0));
  match res {Ok(s)=>serde_json::from_str(&s).map_err(|e|e.to_string()),Err(_)=>Ok(Settings::default())}
 }
 pub fn save_settings(&self,s:&Settings)->Result<(),String>{
  self.0.execute("INSERT INTO config(id,json) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",params![serde_json::to_string(s).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;Ok(())
 }
 pub fn channels(&self)->Result<Vec<Channel>,String>{
  let mut stmt=self.0.prepare("SELECT json FROM channels ORDER BY rowid DESC").map_err(|e|e.to_string())?;
  let rows=stmt.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;
  rows.map(|r| serde_json::from_str::<Channel>(&r.map_err(|e|e.to_string())?).map_err(|e|e.to_string())).collect()
 }
 pub fn save_channel(&self,c:&Channel)->Result<(),String>{
  self.0.execute("INSERT INTO channels(id,json) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json",params![c.id,serde_json::to_string(c).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;Ok(())
 }
 pub fn remove_channel(&self,id:&str)->Result<(),String>{self.0.execute("DELETE FROM channels WHERE id=?1",[id]).map_err(|e|e.to_string())?;Ok(())}
 pub fn highlights(&self)->Result<Vec<Highlight>,String>{
  let mut stmt=self.0.prepare("SELECT id,json FROM highlights ORDER BY id DESC LIMIT 250").map_err(|e|e.to_string())?;
  let rows=stmt.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;
  rows.map(|r| {let (id,s)=r.map_err(|e|e.to_string())?;let mut h:Highlight=serde_json::from_str(&s).map_err(|e|e.to_string())?;h.id=id;Ok(h)}).collect()
 }
 pub fn add_highlight(&self,h:&mut Highlight)->Result<(),String>{
  self.0.execute("INSERT INTO highlights(channel_id,json) VALUES(?1,?2)",params![h.channel_id,serde_json::to_string(h).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;h.id=self.0.last_insert_rowid();self.save_highlight(h)
 }
 pub fn save_highlight(&self,h:&Highlight)->Result<(),String>{
  self.0.execute("UPDATE highlights SET json=?1 WHERE id=?2",params![serde_json::to_string(h).map_err(|e|e.to_string())?,h.id]).map_err(|e|e.to_string())?;Ok(())
 }
 pub fn highlight(&self,id:i64)->Result<Highlight,String>{
  let s:String=self.0.query_row("SELECT json FROM highlights WHERE id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
  serde_json::from_str(&s).map_err(|e|e.to_string())
 }
}
