use image_base64_native::{
    codec::{self},
    files, strict_ack, Adapter,
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
use tezbar_extension_sdk::{Extension, ExtensionPaths, TestContext};
fn qa() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".qa/unit");
    fs::create_dir_all(&p).unwrap();
    p
}
fn temp() -> tempfile::TempDir {
    tempfile::tempdir_in(qa()).unwrap()
}
fn png() -> Vec<u8> {
    let mut b = vec![0; 24];
    b[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    b[19] = 7;
    b[23] = 9;
    b
}
#[test]
fn actual_locked_source_oracle() {
    let oracle: Value = serde_json::from_str(include_str!("../qa/oracle-cases.json")).unwrap();
    for row in oracle["rows"].as_array().unwrap() {
        let input = row["input"].as_str().unwrap();
        assert_eq!(
            codec::valid(input),
            row["valid"].as_bool().unwrap(),
            "validator {input:?}"
        );
        assert_eq!(
            codec::size(input),
            row["size"].as_f64().unwrap(),
            "size {input:?}"
        );
        assert_eq!(
            codec::format_size(codec::size(input)),
            row["formatted"].as_str().unwrap(),
            "format {input:?}"
        );
        assert_eq!(
            hex(&codec::decode(input)),
            row["decoded"].as_str().unwrap(),
            "decode {input:?}"
        );
        assert_eq!(
            serde_json::to_value(codec::dimensions(&codec::decode(codec::strip_prefix(
                input
            ))))
            .unwrap(),
            row["dimensions"],
            "dimensions {input:?}"
        );
        let converted = codec::from_base64(input, "webp").ok();
        match (converted, row.get("converted").filter(|v| !v.is_null())) {
            (Some(i), Some(v)) => {
                assert_eq!(i.uri, v["uri"].as_str().unwrap());
                assert_eq!(i.extension, v["extension"].as_str().unwrap());
                assert_eq!(i.size, v["size"].as_f64().unwrap());
                assert_eq!(serde_json::to_value(i.dimensions).unwrap(), v["dimensions"]);
                assert_eq!(hex(&i.bytes), v["bytes"].as_str().unwrap());
            }
            (None, None) => {
                assert_eq!(
                    codec::from_base64(input, "webp").unwrap_err(),
                    row["error"].as_str().unwrap()
                );
            }
            _ => panic!("conversion mismatch {input:?}"),
        }
    }
    for row in oracle["formats"].as_array().unwrap() {
        let converted = codec::from_base64(
            row["input"].as_str().unwrap(),
            row["outputFormat"].as_str().unwrap(),
        )
        .ok();
        assert_eq!(
            converted.as_ref().map(|i| i.uri.as_str()),
            row["uri"].as_str()
        );
        assert_eq!(
            converted.as_ref().map(|i| i.extension.as_str()),
            row["extension"].as_str()
        );
    }
    for row in oracle["sizes"].as_array().unwrap() {
        assert_eq!(
            codec::format_size(row["bytes"].as_f64().unwrap()),
            row["formatted"].as_str().unwrap()
        );
    }
}
fn hex(b: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    b.iter()
        .flat_map(|b| {
            [
                HEX[(b >> 4) as usize] as char,
                HEX[(b & 15) as usize] as char,
            ]
        })
        .collect()
}
#[test]
fn strict_closed_ack() {
    for good in [json!({"ok":true}), json!({"ok":true,"message":"done"})] {
        assert!(strict_ack(&good));
    }
    for bad in [
        Value::Null,
        json!(true),
        json!(false),
        json!({}),
        json!({"ok":false}),
        json!({"ok":true,"error":"contradiction"}),
        json!({"ok":true,"message":null}),
        json!({"ok":true,"extra":1}),
        json!({"ok":"true"}),
    ] {
        assert!(!strict_ack(&bad));
    }
}
#[test]
fn safe_preview_separates_export_uri() {
    let mut i = codec::from_file(
        png(),
        "](<img src=https://invalid onerror=bad>)".into(),
        "svg+xml\");<script>".into(),
    );
    let preview = codec::preview(&i).unwrap();
    assert!(preview.starts_with("![Converted Image](data:image/png;base64,"));
    assert!(!preview.contains("script"));
    let before = i.uri.clone();
    i.bytes = b"<svg onload=evil><image href='https://invalid'/></svg>".to_vec();
    assert_eq!(codec::preview(&i), None);
    assert_eq!(i.uri, before);
}
#[test]
fn preview_pixel_budget() {
    let mut bytes = png();
    bytes[16..20].copy_from_slice(&100_000u32.to_be_bytes());
    assert!(codec::preview(&codec::from_file(bytes, "bomb.png".into(), "png".into())).is_none());
}
#[test]
fn input_budgets_no_truncation() {
    assert!(codec::from_base64(&"A".repeat(codec::MAX_INPUT + 4), "png").is_err());
    assert!(codec::from_base64("AAAA", "../../other").is_err());
    assert_eq!(
        codec::from_base64("SGVsbG8=", "jpeg").unwrap().bytes,
        b"Hello"
    );
}
#[test]
fn explicit_regular_file_literal_extension() {
    let d = temp();
    let p = d.path().join("weird ]<script>.JPG");
    fs::write(&p, png()).unwrap();
    let i = files::read_selected(&p).unwrap();
    assert_eq!(i.extension, "JPG");
    assert!(i.uri.starts_with("data:image/JPG;base64,"));
    assert_eq!(i.bytes, png());
    assert_eq!(i.dimensions, (7, 9));
    assert!(files::read_selected(PathBuf::from("relative.png").as_path()).is_err());
    assert!(files::read_selected(d.path()).is_err());
    assert!(files::read_selected(&d.path().join("missing")).is_err());
}
#[test]
fn file_size_limit() {
    let d = temp();
    let p = d.path().join("big");
    fs::File::create(&p)
        .unwrap()
        .set_len(codec::MAX_FILE as u64 + 1)
        .unwrap();
    assert!(files::read_selected(&p).is_err());
}
#[test]
fn dotfile_empty_and_nonimage() {
    let d = temp();
    let p = d.path().join(".hidden");
    fs::write(&p, []).unwrap();
    let i = files::read_selected(&p).unwrap();
    assert_eq!(i.uri, "data:image/;base64,");
    assert_eq!(i.dimensions, (0, 0));
    assert_eq!(i.extension, "");
}
#[cfg(unix)]
#[test]
fn symlink_and_fifo_rejected() {
    use std::os::unix::{ffi::OsStrExt, fs::symlink};
    let d = temp();
    let p = d.path().join("a");
    fs::write(&p, png()).unwrap();
    let l = d.path().join("link");
    symlink(&p, &l).unwrap();
    assert!(files::read_selected(&l).is_err());
    let linked = d.path().join("dirlink");
    symlink(d.path(), &linked).unwrap();
    assert!(files::read_selected(&linked.join("a")).is_err());
    let fifo = d.path().join("fifo");
    let c = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    assert!(files::read_selected(&fifo).is_err());
}
#[test]
fn atomic_download_collision_failure_and_recovery() {
    let d = temp();
    let downloads = d.path().join("Downloads");
    let t = d.path().join("temp");
    fs::create_dir(&t).unwrap();
    fs::create_dir(&downloads).unwrap();
    let i = codec::from_base64("SGVsbG8=", "jpeg").unwrap();
    fs::write(downloads.join("image_7.jpeg"), b"preserve").unwrap();
    let p = files::download(&i, &downloads, 7).unwrap();
    assert_eq!(p.file_name().unwrap(), "image_7_1.jpeg");
    assert_eq!(fs::read(p).unwrap(), b"Hello");
    assert_eq!(
        fs::read(downloads.join("image_7.jpeg")).unwrap(),
        b"preserve"
    );
    assert_eq!(fs::read_dir(&t).unwrap().count(), 0);
    for n in 2..16 {
        fs::write(downloads.join(format!("image_7_{n}.jpeg")), []).unwrap();
    }
    let before: Vec<_> = (0..16)
        .map(|n| {
            let name = if n == 0 {
                "image_7.jpeg".into()
            } else {
                format!("image_7_{n}.jpeg")
            };
            let path = downloads.join(name);
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    assert!(files::download(&i, &downloads, 7).is_err());
    for (path, bytes) in before {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    assert_eq!(fs::read_dir(&t).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&downloads).unwrap().count(), 16);
    assert!(files::download(&i, &d.path().join("absent"), 8).is_err());
    assert!(files::download(&i, &downloads, 8).is_ok());
}
#[cfg(unix)]
#[test]
fn download_permission_failure_and_symlinks() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let d = temp();
    let t = d.path().join("temp");
    let out = d.path().join("out");
    fs::create_dir(&t).unwrap();
    fs::create_dir(&out).unwrap();
    let i = codec::from_base64("AAAA", "png").unwrap();
    fs::set_permissions(&out, fs::Permissions::from_mode(0o500)).unwrap();
    assert!(files::download(&i, &out, 1).is_err());
    fs::set_permissions(&out, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(files::download(&i, &out, 1).is_ok());
    let l = d.path().join("linked");
    symlink(&out, &l).unwrap();
    assert!(files::download(&i, &l, 1).is_err());
}
fn context() -> (tempfile::TempDir, TestContext) {
    let d = temp();
    let p = ExtensionPaths {
        package_dir: d.path().into(),
        data_dir: d.path().into(),
        temp_dir: d.path().into(),
    };
    let ctx = TestContext::new(p, &["clipboard.write"]);
    (d, ctx)
}
async fn call(a: &Adapter, c: &TestContext, op: &str, p: Value) -> Value {
    a.handle(&c.context, "base64-to-image", op, &p, &mut json!({}))
        .await
        .unwrap()
}
fn field(v: &Value, text: &str, rev: u64) -> Value {
    json!({"sessionId":"s","actionId":format!("f:{}:change",v["root"]["props"]["formToken"].as_str().unwrap()),"formValues":{"base64":text,"outputFormat":"png","__formToken":v["root"]["props"]["formToken"],"__formRevision":rev}})
}
fn submit(v: &Value, text: &str, rev: u64) -> Value {
    let mut p = field(v, text, rev);
    p["actionId"] = v["actions"][0]["id"].clone();
    p
}
#[tokio::test]
async fn drafts_failure_another_and_a_b_a() {
    let (_d, c) = context();
    let a = Adapter::default();
    let v = call(&a, &c, "extension:run-command", json!({"sessionId":"s"})).await;
    let a1 = call(&a, &c, "extension:invoke-action", field(&v, "AAAA", 1)).await;
    let b = call(&a, &c, "extension:invoke-action", field(&v, "BBBB", 2)).await;
    let a3 = call(&a, &c, "extension:invoke-action", field(&v, "AAAA", 3)).await;
    assert_eq!(b["root"]["children"][0]["props"]["value"], "BBBB");
    assert_eq!(a3["root"]["children"][0]["props"]["value"], "AAAA");
    let stale = call(&a, &c, "extension:invoke-action", submit(&a1, "AAAA", 1)).await;
    assert_eq!(stale["mode"], "unchanged");
    let failed = call(&a, &c, "extension:invoke-action", submit(&a3, "bad", 4)).await;
    assert_eq!(failed["root"]["children"][0]["props"]["value"], "bad");
    let detail = call(
        &a,
        &c,
        "extension:invoke-action",
        submit(&failed, "AAAA", 5),
    )
    .await;
    let old = detail["actions"][2]["id"].clone();
    let form = call(
        &a,
        &c,
        "extension:invoke-action",
        json!({"sessionId":"s","actionId":detail["actions"][0]["id"]}),
    )
    .await;
    assert_eq!(form["root"]["children"][0]["props"]["value"], "AAAA");
    assert_eq!(
        call(
            &a,
            &c,
            "extension:invoke-action",
            json!({"sessionId":"s","actionId":old})
        )
        .await["mode"],
        "unchanged"
    );
    assert!(c.calls().is_empty());
}
#[tokio::test]
async fn ack_failure_retry_exact_without_reconversion() {
    let (_d, c) = context();
    let a = Adapter::default();
    let v = call(&a, &c, "extension:run-command", json!({"sessionId":"s"})).await;
    let detail = call(&a, &c, "extension:invoke-action", submit(&v, "SGVsbG8=", 1)).await;
    let old = detail["actions"][2]["id"].clone();
    c.reply(Ok(json!({"ok":true,"error":"contradiction"})));
    let unknown = c
        .run(call(
            &a,
            &c,
            "extension:invoke-action",
            json!({"sessionId":"s","actionId":old}),
        ))
        .await;
    assert_eq!(unknown["actions"][2]["title"], "Retry Copy Exact Base64");
    assert_eq!(
        call(
            &a,
            &c,
            "extension:invoke-action",
            json!({"sessionId":"s","actionId":old})
        )
        .await["mode"],
        "unchanged"
    );
    let refreshed = call(
        &a,
        &c,
        "extension:refresh-session",
        json!({"sessionId":"s"}),
    )
    .await;
    assert_eq!(refreshed, unknown);
    c.reply(Ok(json!({"ok":true})));
    let done = c
        .run(call(
            &a,
            &c,
            "extension:invoke-action",
            json!({"sessionId":"s","actionId":unknown["actions"][2]["id"]}),
        ))
        .await;
    assert_eq!(done["actions"][2]["title"], "Copy Image Base64");
    assert_ne!(done["actions"][2]["id"], unknown["actions"][2]["id"]);
    assert_eq!(c.calls().len(), 2);
    assert_eq!(c.calls()[0]["params"], c.calls()[1]["params"]);
}
#[test]
fn arbitrary_data_uri_kept_and_no_transcoding() {
    let i = codec::from_base64("data:image/svg+xml;base64,PHN2Zy8+", "jpeg").unwrap();
    assert_eq!(i.uri, "data:image/svg+xml;base64,PHN2Zy8+");
    assert_eq!(i.bytes, codec::decode(&i.uri));
    assert!(codec::preview(&i).is_none());
}
#[test]
fn header_truncations_do_not_fail_conversion() {
    for n in 0..24 {
        let bytes = png()[..n].to_vec();
        let i = codec::from_file(bytes, "x.png".into(), "png".into());
        if n < 24 {
            assert_eq!(i.dimensions, (0, 0));
        }
    }
}

#[tokio::test]
async fn both_commands_allow_fresh_deliberate_copies_without_replaying_old_ids() {
    for command in ["base64-to-image", "image-to-base64"] {
        let (d, c) = context();
        let a = Adapter::default();
        let selected = d.path().join("selected.PNG");
        fs::write(&selected, png()).unwrap();
        let mut state = json!({});
        let form = a
            .handle(
                &c.context,
                command,
                "extension:run-command",
                &json!({"sessionId":"s"}),
                &mut state,
            )
            .await
            .unwrap();
        let mut values = if command == "base64-to-image" {
            json!({"base64":"SGVsbG8=", "outputFormat":"jpeg"})
        } else {
            json!({"filePath":[selected]})
        };
        values["__formToken"] = form["root"]["props"]["formToken"].clone();
        values["__formRevision"] = json!(1);
        let mut view = a
            .handle(
                &c.context,
                command,
                "extension:invoke-action",
                &json!({"sessionId":"s","actionId":form["actions"][0]["id"],"formValues":values}),
                &mut state,
            )
            .await
            .unwrap();
        fs::remove_file(selected).unwrap();
        let copy_title = if command == "base64-to-image" {
            "Copy Image Base64"
        } else {
            "Copy Base64 to Clipboard"
        };
        let mut consumed = vec![];
        for count in 1..=2 {
            let old = view["actions"].as_array().unwrap().last().unwrap()["id"].clone();
            c.reply(Ok(json!({"ok":true})));
            view = c
                .run(a.handle(
                    &c.context,
                    command,
                    "extension:invoke-action",
                    &json!({"sessionId":"s","actionId":old}),
                    &mut state,
                ))
                .await
                .unwrap();
            assert_eq!(
                view["actions"].as_array().unwrap().last().unwrap()["title"],
                copy_title
            );
            assert_ne!(
                view["actions"].as_array().unwrap().last().unwrap()["id"],
                old
            );
            consumed.push(old);
            for stale in &consumed {
                let unchanged = a
                    .handle(
                        &c.context,
                        command,
                        "extension:invoke-action",
                        &json!({"sessionId":"s","actionId":stale}),
                        &mut state,
                    )
                    .await
                    .unwrap();
                assert_eq!(unchanged["mode"], "unchanged");
            }
            let refreshed = a
                .handle(
                    &c.context,
                    command,
                    "extension:refresh-session",
                    &json!({"sessionId":"s"}),
                    &mut state,
                )
                .await
                .unwrap();
            assert_eq!(refreshed, view);
            assert_eq!(c.calls().len(), count);
        }
        assert_eq!(c.calls()[0]["params"], c.calls()[1]["params"]);
        let form = a
            .handle(
                &c.context,
                command,
                "extension:invoke-action",
                &json!({"sessionId":"s","actionId":view["actions"][0]["id"]}),
                &mut state,
            )
            .await
            .unwrap();
        assert_eq!(form["root"]["type"], "Form");
        assert_eq!(c.calls().len(), 2);
    }
}
