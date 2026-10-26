#!/usr/bin/env python3
"""Compile actual narrow host producers with inert OS/dialog implementations."""
from pathlib import Path
import subprocess
r=Path(__file__).resolve().parents[1]
picker=(r/'.qa/host-patched/src-tauri/src/extension_file_picker.rs').read_text().replace('#[tauri::command]','')
store=(r/'qa/fixtures/host/src-tauri/src/native_core/launcher/stores.rs').read_text()
textarg=store[store.index('pub fn text_arg('):store.index('pub fn load_snippets(')]
clip=(r/'qa/fixtures/host/src-tauri/src/native_core/launcher/clipboard.rs').read_text()
clip=clip[clip.index('            "clipboard:write" => {'):clip.index('            "clipboard:write-png" => {')]
content='''// GENERATED qa/producers.py. Only tauri command attribute removed; actual
// native picker body and clipboard:write producer run against inert OS mocks.
use serde_json::{json,Value};
use std::path::{Path,PathBuf};
mod tauri { #[derive(Clone)] pub struct AppHandle {pub path:Option<std::path::PathBuf>,pub calls:std::sync::Arc<std::sync::atomic::AtomicUsize>} }
mod tauri_plugin_dialog {
 use super::tauri::AppHandle;
 pub trait DialogExt {fn dialog(&self)->AppHandle;}
 impl DialogExt for AppHandle {fn dialog(&self)->AppHandle{self.clone()}}
 pub struct FilePath(pub std::path::PathBuf);
 impl FilePath {pub fn into_path(self)->Result<std::path::PathBuf,String>{Ok(self.0)}}
 impl AppHandle {pub fn file(self)->Self{self} pub fn pick_file(self,callback:impl FnOnce(Option<FilePath>)){self.calls.fetch_add(1,std::sync::atomic::Ordering::SeqCst);callback(self.path.map(FilePath))}}
}
'''+picker+'\n'+textarg+'''
#[derive(Default)]struct Clipboard {writes:Vec<String>,fail:bool}
impl Clipboard {
 fn write_text(&mut self,text:String)->Result<(),String>{if self.fail {return Err("inert permission error".into())}self.writes.push(text);Ok(())}
 #[allow(clippy::needless_return)] // Frozen host branch copied verbatim.
 fn handle(&mut self,c:&str,a:&[Value])->Result<Value,String>{match c {
'''+clip+'''_=>panic!("unexpected clipboard producer")}}
}
fn owned()->tempfile::TempDir {let p=Path::new(env!("CARGO_MANIFEST_DIR")).join(".qa/producers");std::fs::create_dir_all(&p).unwrap();tempfile::tempdir_in(p).unwrap()}
#[tokio::test] async fn native_picker_explicit_cancel_relative_directory_symlink() {
 let d=owned();let p=d.path().join("Selected.JPG");std::fs::write(&p,b"inert").unwrap();
 let calls=std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
 for path in [None,Some(p.clone())] {let got=pick_extension_file(tauri::AppHandle{path:path.clone(),calls:calls.clone()}).await.unwrap();assert_eq!(got,path.map(|p|p.to_str().unwrap().to_owned()));}
 for path in [PathBuf::from("relative"),d.path().into(),d.path().join("missing")] {assert!(pick_extension_file(tauri::AppHandle{path:Some(path),calls:calls.clone()}).await.is_err());}
 #[cfg(unix)] {let link=d.path().join("link");std::os::unix::fs::symlink(&p,&link).unwrap();assert!(pick_extension_file(tauri::AppHandle{path:Some(link),calls:calls.clone()}).await.is_err());}
 assert!(calls.load(std::sync::atomic::Ordering::SeqCst)>=5);
}
#[test]fn actual_clipboard_producer_closed_ack_and_error(){
 let mut c=Clipboard::default();let text="data:image/JPEG;base64,SGVsbG8=";
 let ack=c.handle("clipboard:write",&[json!(text)]).unwrap();assert!(image_base64_native::strict_ack(&ack));assert_eq!(c.writes,[text]);
 assert!(c.handle("clipboard:write",&[json!(null)]).is_err());c.fail=true;assert!(c.handle("clipboard:write",&[json!(text)]).is_err());assert_eq!(c.writes.len(),1);
}
'''
(r/'tests/producers.rs').write_text(content)
subprocess.run(['rustfmt','--edition','2021',str(r/'tests/producers.rs')],check=True)
print('Generated actual native picker and clipboard producer bodies; OS inert')
