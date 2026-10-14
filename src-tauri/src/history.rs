use std::{fs, path::{Path,PathBuf}, sync::atomic::{AtomicUsize,Ordering}, time::{SystemTime,UNIX_EPOCH}};
use serde::{Serialize,Deserialize};
use tauri::Manager;
use crate::Report;

static SEQUENCE:AtomicUsize=AtomicUsize::new(0);
const MAX_RECORDS:usize=30;
const MAX_HISTORY_BYTES:usize=16*1024*1024;

#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct HistoryMeta {
 pub id:String,
 pub profile_id:String,
 pub profile_name:String,
 pub timestamp:u64,
 pub sources:Vec<String>,
 pub total:usize,
 pub scanned:usize,
 pub issues:usize,
 pub unreadable:usize,
 pub cancelled:bool,
 pub truncated:bool,
}

#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
struct StoredReport {
 meta:HistoryMeta,
 report:Report,
}

fn folder(app:&tauri::AppHandle)->Result<PathBuf,String> {
 app.path().app_data_dir().map(|p|p.join("history")).map_err(|e|e.to_string())
}
fn validate_id(value:&str)->Result<(),String> {
 if value.len()>96 || value.is_empty() || !value.bytes().all(|b|b.is_ascii_alphanumeric() || b==b'-' || b==b'_') {
  return Err("无效的记录编号".into())
 }
 Ok(())
}
fn index_path(dir:&Path)->PathBuf {dir.join("index.json")}
fn record_path(dir:&Path,id:&str)->PathBuf {dir.join(format!("{id}.json"))}
fn load_index(dir:&Path)->Result<Vec<HistoryMeta>,String> {
 match fs::read(index_path(dir)) {
  Ok(data)=>serde_json::from_slice(&data).map_err(|e|format!("历史索引损坏: {e}")),
  Err(e) if e.kind()==std::io::ErrorKind::NotFound =>Ok(vec![]),
  Err(e)=>Err(e.to_string()),
 }
}
fn atomic_write(path:&Path, bytes:&[u8])->Result<(),String>{
 let filename=path.file_name().and_then(|n|n.to_str()).unwrap_or("record");
 let tmp=path.with_file_name(format!("{filename}.tmp-{}",SEQUENCE.fetch_add(1,Ordering::Relaxed)));
 fs::write(&tmp,bytes).map_err(|e|e.to_string())?;
 match fs::rename(&tmp,path){
  Ok(())=>Ok(()),
  Err(_) if path.exists()=>{
   fs::remove_file(path).map_err(|e|e.to_string())?;
   fs::rename(&tmp,path).map_err(|e|e.to_string())
  },
  Err(e)=>Err(e.to_string())
 }
}
fn save_index(dir:&Path,list:&[HistoryMeta])->Result<(),String> {
 atomic_write(&index_path(dir),&serde_json::to_vec_pretty(list).map_err(|e|e.to_string())?)
}

#[tauri::command]
pub fn list_scan_history(app:tauri::AppHandle)->Result<Vec<HistoryMeta>,String>{
 let dir=folder(&app)?;
 load_index(&dir)
}
#[tauri::command]
pub fn load_scan_history(app:tauri::AppHandle,id:String)->Result<Report,String>{
 validate_id(&id)?;
 let dir=folder(&app)?;
 let record:StoredReport=serde_json::from_slice(&fs::read(record_path(&dir,&id)).map_err(|e|e.to_string())?)
  .map_err(|e|format!("历史记录无法读取: {e}"))?;
 Ok(record.report)
}
#[tauri::command]
pub fn save_scan_history(
 app:tauri::AppHandle,profile_id:String,profile_name:String,sources:Vec<String>,report:Report,
)->Result<HistoryMeta,String>{
 let dir=folder(&app)?;
 fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
 let now=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|e.to_string())?.as_millis();
 let id=format!("{now}-{}-{}",std::process::id(),SEQUENCE.fetch_add(1,Ordering::Relaxed));
 let issues=report.files.iter().filter(|r|r.status=="issue").count();
 let unreadable=report.files.iter().filter(|r|r.status=="unreadable").count();
 let mut stored=report;
 let truncated=stored.files.iter().any(|r|r.hits.len()>60);
 for r in &mut stored.files {
  r.hits.truncate(60);
  for h in &mut r.hits {
   if h.context.chars().count()>500 {h.context=h.context.chars().take(500).collect();}
  }
 }
 let mut meta=HistoryMeta{id,profile_id,profile_name,timestamp:(now/1000) as u64,
   sources,total:stored.total,scanned:stored.scanned,issues,unreadable,
   cancelled:stored.cancelled,truncated};
 let mut bytes=serde_json::to_vec(&StoredReport{meta:meta.clone(),report:stored.clone()}).map_err(|e|e.to_string())?;
 if bytes.len()>MAX_HISTORY_BYTES {
  meta.truncated=true;
  // Keep every file's status and missing requirements; omit evidence when too large.
  for f in &mut stored.files {f.hits.clear();}
  bytes=serde_json::to_vec(&StoredReport{meta:meta.clone(),report:stored}).map_err(|e|e.to_string())?;
  if bytes.len()>MAX_HISTORY_BYTES {return Err("本次扫描记录过大，请导出 CSV 留存完整结果".into());}
 }
 atomic_write(&record_path(&dir,&meta.id),&bytes)?;
 let mut list=load_index(&dir)?;
 list.insert(0,meta.clone());
 if list.len()>MAX_RECORDS {
  let removed=list.split_off(MAX_RECORDS);
  for item in removed {let _=fs::remove_file(record_path(&dir,&item.id));}
 }
 save_index(&dir,&list)?;
 Ok(meta)
}
#[tauri::command]
pub fn delete_scan_history(app:tauri::AppHandle,id:String)->Result<(),String>{
 validate_id(&id)?;
 let dir=folder(&app)?;
 let mut list=load_index(&dir)?;
 list.retain(|entry|entry.id!=id);
 save_index(&dir,&list)?;
 match fs::remove_file(record_path(&dir,&id)){
  Ok(())=>Ok(()),
  Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),
  Err(e)=>Err(e.to_string())
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn rejects_path_traversal(){assert!(validate_id("../oops").is_err());assert!(validate_id("a-b_234").is_ok());}
}
