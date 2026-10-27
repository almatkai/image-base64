pub mod codec;
pub mod files;
use codec::Image;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    future::Future,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    task::{Context, Poll, Waker},
};
use tezbar_extension_sdk::{
    BoxFuture, CommandConcurrency, CoreResult, Extension, ExtensionContext,
};

pub const ID: &str = "com.tezbar.native.image-base64";
const COMMANDS: &[&str] = &["base64-to-image", "image-to-base64"];
const MAX_SESSIONS: usize = 16;
#[derive(Default)]
pub struct Adapter {
    sessions: Mutex<HashMap<String, Session>>,
    sequence: AtomicU64,
}
struct Session {
    opened: ExtensionContext,
    command: String,
    form_token: u64,
    view_token: u64,
    field_revision: u64,
    draft: Value,
    image: Option<Image>,
    notice: String,
    copy_unknown: bool,
    downloaded: Option<String>,
}
fn cancelled(ctx: &ExtensionContext) -> bool {
    let mut task = Context::from_waker(Waker::noop());
    matches!(
        Box::pin(ctx.cancelled()).as_mut().poll(&mut task),
        Poll::Ready(())
    )
}
fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    mutex.lock().map_err(|_| "Session lock failed".into())
}
fn node(kind: &str, props: Value) -> Value {
    json!({"type":kind,"props":props})
}
fn action(id: String, title: &str, kind: &str, key: &str) -> Value {
    json!({"id":id,"title":title,"kind":kind,"shortcut":{"modifiers":["cmd"],"key":key}})
}
fn render(s: &Session) -> Value {
    let title = if s.command == COMMANDS[0] {
        "Base64 to Image"
    } else {
        "Image to Base64"
    };
    let (root, actions) = if let Some(image) = &s.image {
        let prefix = format!("d:{}:", s.view_token);
        let mut actions = vec![action(
            format!("{prefix}another"),
            "Convert Another",
            "action",
            "r",
        )];
        if s.command == COMMANDS[0] && s.downloaded.is_none() {
            actions.push(action(
                format!("{prefix}download"),
                "Download Image",
                "action",
                "d",
            ));
        }
        let copy_title = if s.copy_unknown {
            "Retry Copy Exact Base64"
        } else if s.command == COMMANDS[0] {
            "Copy Image Base64"
        } else {
            "Copy Base64 to Clipboard"
        };
        actions.push(action(format!("{prefix}copy"), copy_title, "action", "c"));
        let mut labels = vec![
            node(
                "Detail.Metadata.Label",
                json!({"title":if s.command==COMMANDS[0]{"Format"}else{"File Name"},"text":if s.command==COMMANDS[0]{image.extension.to_uppercase()}else{image.label.clone()}}),
            ),
            node("Detail.Metadata.Separator", json!({})),
            node(
                "Detail.Metadata.Label",
                json!({"title":"Dimensions","text":format!("{}x{}",image.dimensions.0,image.dimensions.1)}),
            ),
            node("Detail.Metadata.Separator", json!({})),
            node(
                "Detail.Metadata.Label",
                json!({"title":"Size","text":codec::format_size(image.size)}),
            ),
        ];
        labels.push(json!({"type":"Detail.Metadata.TagList","props":{"title":"Actions"},"children":if s.command==COMMANDS[0]{vec![node("Detail.Metadata.TagList.Item",json!({"text":"Download"})),node("Detail.Metadata.TagList.Item",json!({"text":"Copy Base64"}))]}else{vec![node("Detail.Metadata.TagList.Item",json!({"text":"Copy Base64"}))]}}));
        if !s.notice.is_empty() {
            labels.push(node(
                "Detail.Metadata.Label",
                json!({"title":"Status","text":s.notice}),
            ));
        }
        if let Some(path) = &s.downloaded {
            labels.push(node(
                "Detail.Metadata.Label",
                json!({"title":"Downloaded To","text":path}),
            ));
        }
        (
            json!({"type":"Detail","props":{"markdown":codec::preview(image).unwrap_or_else(||"Preview unavailable for this payload. Copy and Download preserve the original bytes.".into())},"metadata":{"type":"Detail.Metadata","children":labels}}),
            actions,
        )
    } else {
        let change = format!("f:{}:change", s.form_token);
        let mut fields = if s.command == COMMANDS[0] {
            vec![
                node(
                    "Form.TextArea",
                    json!({"id":"base64","title":"Base64","placeholder":"Paste your base64 image here","value":s.draft["base64"],"actionId":change}),
                ),
                json!({"type":"Form.Dropdown","props":{"id":"outputFormat","title":"Output Format","value":s.draft["outputFormat"],"actionId":change},"children":[node("Form.Dropdown.Item",json!({"value":"png","title":"PNG"})),node("Form.Dropdown.Item",json!({"value":"jpeg","title":"JPEG"})),node("Form.Dropdown.Item",json!({"value":"webp","title":"WebP"}))]}),
            ]
        } else {
            vec![node(
                "Form.FilePicker",
                json!({"id":"filePath","title":"Select Image","value":s.draft["filePath"],"allowMultipleSelection":false,"canChooseDirectories":false,"actionId":change,"nativeAbsolutePath":true}),
            )]
        };
        if !s.notice.is_empty() {
            fields.push(node(
                "Form.Description",
                json!({"title":"Status","text":s.notice}),
            ));
        }
        (
            json!({"type":"Form","props":{"formToken":s.form_token.to_string(),"formRevision":s.field_revision},"children":fields}),
            vec![action(
                format!("f:{}:submit", s.form_token),
                if s.command == COMMANDS[0] {
                    "Convert to Image"
                } else {
                    "Convert to Base64"
                },
                "submit-form",
                "enter",
            )],
        )
    };
    let mut out = tezbar_extension_sdk::view(title, root, actions);
    out["refreshMs"] = Value::Null;
    if !s.notice.is_empty() {
        // Explicit outcome text also overrides the host's title-based "Copied"
        // fallback; an ambiguous ACK must never acquire success authority.
        out["message"] = json!(if s.image.is_none() {
            format!("Error: {}", s.notice)
        } else {
            s.notice.clone()
        });
    }
    out
}
fn fields(s: &mut Session, payload: &Value, change: bool) -> Result<bool, String> {
    let v = payload["formValues"]
        .as_object()
        .ok_or("Host must provide all current form fields")?;
    if v.get("__formToken").and_then(Value::as_str) != Some(&s.form_token.to_string()) {
        return Err("Stale form token; reopen the current form".into());
    }
    let rev = v
        .get("__formRevision")
        .and_then(Value::as_u64)
        .filter(|r| *r <= 9_007_199_254_740_991)
        .ok_or("Host all-field revision prerequisite is required")?;
    if rev < s.field_revision || (change && rev == s.field_revision) {
        return Ok(false);
    }
    let draft = if s.command == COMMANDS[0] {
        if v.keys().any(|k| {
            !["base64", "outputFormat", "__formRevision", "__formToken"].contains(&k.as_str())
        }) {
            return Err("Unexpected form field".into());
        }
        let text = v
            .get("base64")
            .and_then(Value::as_str)
            .ok_or("Base64 field must be text")?;
        let fmt = v
            .get("outputFormat")
            .and_then(Value::as_str)
            .ok_or("Output format missing")?;
        if text.len() > codec::MAX_INPUT || fmt.len() > 4 {
            return Err("Form budget exceeded".into());
        }
        json!({"base64":text,"outputFormat":fmt})
    } else {
        if v.keys()
            .any(|k| !["filePath", "__formRevision", "__formToken"].contains(&k.as_str()))
        {
            return Err("Unexpected form field".into());
        }
        let paths = v
            .get("filePath")
            .and_then(Value::as_array)
            .ok_or("Select one file")?;
        if paths.len() > 1
            || paths
                .iter()
                .any(|p| p.as_str().is_none_or(|s| s.len() > 4096))
        {
            return Err("Select one file, with a path of at most 4096 bytes".into());
        }
        json!({"filePath":paths})
    };
    if rev == s.field_revision && draft != s.draft {
        return Err("Changed fields require a fresh form revision".into());
    }
    s.field_revision = rev;
    s.draft = draft;
    Ok(true)
}
pub fn strict_ack(value: &Value) -> bool {
    value.as_object().is_some_and(|v| {
        v.get("ok") == Some(&Value::Bool(true))
            && v.keys().all(|k| k == "ok" || k == "message")
            && v.get("message").is_none_or(Value::is_string)
    })
}
impl Adapter {
    fn next(&self) -> u64 {
        self.sequence.fetch_add(1, Ordering::Relaxed) + 1
    }
}
impl Extension for Adapter {
    fn id(&self) -> &'static str {
        ID
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    fn commands(&self) -> &'static [&'static str] {
        COMMANDS
    }
    fn concurrency(&self, _: &str) -> CommandConcurrency {
        CommandConcurrency::RejectWhileBusy
    }
    fn shutdown<'a>(&'a self, _: &'a ExtensionContext) -> BoxFuture<'a, CoreResult> {
        Box::pin(async move {
            lock(&self.sessions)?.clear();
            Ok(json!({}))
        })
    }
    fn handle<'a>(
        &'a self,
        ctx: &'a ExtensionContext,
        command: &'a str,
        operation: &'a str,
        payload: &'a Value,
        _state: &'a mut Value,
    ) -> BoxFuture<'a, CoreResult> {
        Box::pin(async move {
            let sid = payload["sessionId"]
                .as_str()
                .filter(|s| s.len() <= 256)
                .ok_or("Invalid session ID")?
                .to_owned();
            if operation == "extension:dispose-session" {
                lock(&self.sessions)?.remove(&sid);
                return Ok(json!({}));
            }
            if cancelled(ctx) {
                return Err("Operation cancelled".into());
            }
            let prepared = {
                let mut sessions = lock(&self.sessions)?;
                // The SDK may cancel an open before its transactional state commits.
                // Prune its captured scope/token before counting adapter slots.
                sessions.retain(|_, s| !cancelled(&s.opened));
                if operation == "extension:run-command" {
                    if !COMMANDS.contains(&command) {
                        return Err("Unknown command".into());
                    }
                    if sessions.len() >= MAX_SESSIONS {
                        return Err("At most 16 converter sessions may be open".into());
                    }
                    let token = self.next();
                    sessions.insert(
                        sid.clone(),
                        Session {
                            opened: ctx.clone(),
                            command: command.into(),
                            form_token: token,
                            view_token: token,
                            field_revision: 0,
                            draft: if command == COMMANDS[0] {
                                json!({"base64":"","outputFormat":"png"})
                            } else {
                                json!({"filePath":[]})
                            },
                            image: None,
                            notice: String::new(),
                            copy_unknown: false,
                            downloaded: None,
                        },
                    );
                    return Ok(render(&sessions[&sid]));
                }
                let s = sessions
                    .get_mut(&sid)
                    .ok_or("Session expired; reopen the converter")?;
                if s.command != command {
                    return Err("Command/session mismatch".into());
                }
                if operation == "extension:refresh-session" {
                    return Ok(render(s));
                }
                if operation != "extension:invoke-action" {
                    return Err("Unsupported operation".into());
                }
                let id = payload["actionId"]
                    .as_str()
                    .filter(|s| s.len() <= 80)
                    .ok_or("Invalid action ID")?;
                if s.image.is_none() {
                    let change = id == format!("f:{}:change", s.form_token);
                    if !change && id != format!("f:{}:submit", s.form_token) {
                        return Ok(tezbar_extension_sdk::unchanged());
                    }
                    match fields(s, payload, change) {
                        Ok(false) => return Ok(tezbar_extension_sdk::unchanged()),
                        Err(error) => {
                            s.notice = error;
                            return Ok(render(s));
                        }
                        Ok(true) => {}
                    }
                    if change {
                        s.notice.clear();
                        return Ok(render(s));
                    }
                    let converted = if command == COMMANDS[0] {
                        codec::from_base64(
                            s.draft["base64"].as_str().unwrap(),
                            s.draft["outputFormat"].as_str().unwrap(),
                        )
                    } else {
                        s.draft["filePath"][0]
                            .as_str()
                            .ok_or_else(|| "Please select an image file".to_string())
                            .and_then(|p| files::read_selected(Path::new(p)))
                    };
                    match converted {
                        Ok(image) => {
                            s.image = Some(image);
                            s.view_token = self.next();
                            s.notice.clear();
                        }
                        Err(error) => {
                            s.notice = error;
                            // A submitted action is consumed even when conversion fails.
                            // Preserve its draft, but require a new explicit submit token.
                            s.form_token = self.next();
                            s.field_revision = 0;
                        }
                    }
                    return Ok(render(s));
                }
                let prefix = format!("d:{}:", s.view_token);
                let Some(kind) = id.strip_prefix(&prefix) else {
                    return Ok(tezbar_extension_sdk::unchanged());
                };
                if payload
                    .get("formValues")
                    .is_some_and(|v| !v.is_null() && v.as_object().is_none_or(|o| !o.is_empty()))
                {
                    return Ok(tezbar_extension_sdk::unchanged());
                }
                match kind {
                    "another" => {
                        s.image = None;
                        s.form_token = self.next();
                        s.view_token = s.form_token;
                        s.field_revision = 0;
                        s.notice.clear();
                        s.copy_unknown = false;
                        s.downloaded = None;
                        return Ok(render(s));
                    }
                    "download" if command == COMMANDS[0] && s.downloaded.is_none() => {
                        s.view_token = self.next();
                        // Local checkpoint, before any await; refresh/stale/copy cannot replay.
                        let result = files::downloads().and_then(|p| {
                            files::download(s.image.as_ref().unwrap(), &p, files::now_millis()?)
                        });
                        match result {
                            Ok(path) => {
                                s.downloaded = Some(path.to_string_lossy().into());
                                s.notice = "Image downloaded".into();
                            }
                            Err(error) => {
                                s.notice = format!("Download failed. Retry explicitly: {error}")
                            }
                        }
                        return Ok(render(s));
                    }
                    "copy" => {
                        s.view_token = self.next();
                        s.copy_unknown = true;
                        s.notice =
                            "Could not confirm copy. Outcome unknown; retry sends exactly the prepared text.".into();
                        (s.image.as_ref().unwrap().uri.clone(), s.view_token)
                    }
                    _ => return Ok(tezbar_extension_sdk::unchanged()),
                }
            };
            let reply = ctx
                .host("host.clipboard.write", json!({"text":prepared.0}))
                .await;
            if cancelled(ctx) {
                return Err(
                    "Copy cancelled; outcome unknown. Refresh to retry the exact prepared text."
                        .into(),
                );
            }
            let mut sessions = lock(&self.sessions)?;
            let s = sessions.get_mut(&sid).ok_or("Session disposed")?;
            if s.view_token != prepared.1 {
                return Ok(tezbar_extension_sdk::unchanged());
            }
            if reply.as_ref().is_ok_and(strict_ack) {
                s.copy_unknown = false;
                s.notice = "Base64 string copied to clipboard".into();
            }
            // Invalid/error/ambiguous acknowledgements deliberately retain unknown state.
            Ok(render(s))
        })
    }
}
