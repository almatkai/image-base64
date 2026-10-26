use crate::codec::{self, Image, MAX_FILE};
#[cfg(unix)]
use std::fs::File;
#[cfg(not(unix))]
use std::fs::OpenOptions;
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn ordinary_path(path: &Path, directory: bool) -> Result<(), String> {
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err("Choose an absolute path of at most 4096 bytes".into());
    }
    let mut walk = std::path::PathBuf::new();
    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) {
            return Err("Non-normal path".into());
        }
        walk.push(component);
        let metadata = fs::symlink_metadata(&walk).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(
                "Symbolic links are not supported; select the ordinary file directly".into(),
            );
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("Reparse paths are not supported".into());
            }
        }
    }
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if (directory && !m.is_dir()) || (!directory && !m.is_file()) {
        return Err("Path must be an ordinary file/directory".into());
    }
    Ok(())
}
fn same(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev()
            || a.ino() != b.ino()
            || a.ctime() != b.ctime()
            || a.ctime_nsec() != b.ctime_nsec()
        {
            return false;
        }
    }
    a.is_file() == b.is_file() && a.len() == b.len() && a.modified().ok() == b.modified().ok()
}
#[cfg(unix)]
fn open_confined(path: &Path, directory: bool) -> Result<File, String> {
    use std::os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::OsStrExt,
    };
    if !path.is_absolute() {
        return Err("Expected absolute path".into());
    }
    let mut current = File::open("/").map_err(|e| e.to_string())?;
    let parts: Vec<_> = path
        .components()
        .filter(|c| !matches!(c, Component::RootDir))
        .collect();
    for (n, part) in parts.iter().enumerate() {
        let Component::Normal(name) = part else {
            return Err("Non-normal path".into());
        };
        let name = std::ffi::CString::new(name.as_bytes()).map_err(|e| e.to_string())?;
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if directory || n + 1 < parts.len() {
                libc::O_DIRECTORY
            } else {
                0
            };
        // Each component is opened relative to its already validated directory.
        let fd = unsafe { libc::openat(current.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        current = unsafe { File::from_raw_fd(fd) };
    }
    Ok(current)
}
pub fn read_selected(path: &Path) -> Result<Image, String> {
    ordinary_path(path, false)?;
    let before = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if before.len() > MAX_FILE as u64 {
        return Err("Selected file exceeds 1,000,000 bytes".into());
    }
    #[cfg(unix)]
    let mut file = open_confined(path, false)?;
    #[cfg(not(unix))]
    let mut file = {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x00200000);
        }
        options.open(path).map_err(|e| e.to_string())?
    };
    let opened = file.metadata().map_err(|e| e.to_string())?;
    if !opened.is_file() || !same(&before, &opened) {
        return Err("File changed while opening".into());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_FILE as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    ordinary_path(path, false)?;
    if bytes.len() > MAX_FILE
        || bytes.len() as u64 != before.len()
        || !same(&before, &file.metadata().map_err(|e| e.to_string())?)
        || !same(
            &before,
            &fs::symlink_metadata(path).map_err(|e| e.to_string())?,
        )
    {
        return Err("File changed during conversion".into());
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Filename must be UTF-8")?;
    if name.len() > 512 {
        return Err("Filename exceeds 512 UTF-8 bytes".into());
    }
    // Node path.extname: a lone dotfile has no extension; trailing dot has empty extension.
    let extension = name
        .rfind('.')
        .filter(|n| *n > 0)
        .map(|n| &name[n + 1..])
        .unwrap_or("");
    Ok(codec::from_file(bytes, name.into(), extension.into()))
}
pub fn downloads() -> Result<std::path::PathBuf, String> {
    #[cfg(windows)]
    let key = "USERPROFILE";
    #[cfg(not(windows))]
    let key = "HOME";
    let home = std::env::var_os(key).ok_or("Home directory unavailable")?;
    Ok(Path::new(&home).join("Downloads"))
}
// The staging name and final link are relative to the same validated directory
// handle on Unix. Drop removes only our staging entry, even if its directory moves.
#[cfg(unix)]
struct StagedDownload {
    file: File,
    directory: File,
    name: std::ffi::CString,
}
#[cfg(unix)]
impl StagedDownload {
    fn new(downloads: &Path) -> Result<Self, String> {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let directory = open_confined(downloads, true)?;
        for _ in 0..16 {
            let name = std::ffi::CString::new(format!(
                ".image-base64-{}-{}-{}.tmp",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed),
            ))
            .map_err(|e| e.to_string())?;
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd >= 0 {
                return Ok(Self {
                    file: unsafe { File::from_raw_fd(fd) },
                    directory,
                    name,
                });
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(error.to_string());
            }
        }
        Err("Could not reserve an owned download staging name".into())
    }
    fn publish(&self, dest: &Path) -> std::io::Result<()> {
        use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
        let name = std::ffi::CString::new(dest.file_name().unwrap().as_bytes())?;
        let fd = self.directory.as_raw_fd();
        if unsafe { libc::linkat(fd, self.name.as_ptr(), fd, name.as_ptr(), 0) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}
#[cfg(unix)]
impl Drop for StagedDownload {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        // Never traverse a pathname or remove anything except our own entry.
        unsafe { libc::unlinkat(self.directory.as_raw_fd(), self.name.as_ptr(), 0) };
    }
}
#[cfg(not(unix))]
struct StagedDownload {
    file: tempfile::NamedTempFile,
}
#[cfg(not(unix))]
impl StagedDownload {
    fn new(downloads: &Path) -> Result<Self, String> {
        Ok(Self {
            file: tempfile::Builder::new()
                .prefix(".image-base64-")
                .suffix(".tmp")
                .tempfile_in(downloads)
                .map_err(|e| e.to_string())?,
        })
    }
    fn publish(&self, dest: &Path) -> std::io::Result<()> {
        fs::hard_link(self.file.path(), dest)
    }
}
pub fn download(
    image: &Image,
    downloads: &Path,
    millis: u128,
) -> Result<std::path::PathBuf, String> {
    download_with(image, downloads, millis, StagedDownload::publish)
}
fn download_with(
    image: &Image,
    downloads: &Path,
    millis: u128,
    mut publish: impl FnMut(&StagedDownload, &Path) -> std::io::Result<()>,
) -> Result<std::path::PathBuf, String> {
    if !["png", "jpeg", "webp"].contains(&image.extension.as_str()) {
        return Err("Unsafe download extension".into());
    }
    ordinary_path(downloads, true)?;
    let mut stage = StagedDownload::new(downloads)?;
    stage
        .file
        .write_all(&image.bytes)
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    stage.file.sync_all().map_err(|e| e.to_string())?;
    #[cfg(not(unix))]
    stage.file.as_file().sync_all().map_err(|e| e.to_string())?;
    for suffix in 0..16 {
        let name = if suffix == 0 {
            format!("image_{millis}.{}", image.extension)
        } else {
            format!("image_{millis}_{suffix}.{}", image.extension)
        };
        let dest = downloads.join(name);
        ordinary_path(downloads, true)?;
        match publish(&stage, &dest) {
            Ok(()) => return Ok(dest),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Download not published: {e}. Downloads must support atomic non-replacing hard links.")),
        }
    }
    Err("All 16 download names already exist; retry explicitly".into())
}
pub fn now_millis() -> Result<u128, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis())
}
#[cfg(test)]
mod download_tests {
    use super::*;
    #[test]
    fn injected_publication_failure_cleans_destination_stage() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".qa/unit");
        fs::create_dir_all(&root).unwrap();
        let dir = tempfile::tempdir_in(root).unwrap();
        let image = codec::from_base64("SGVsbG8=", "png").unwrap();
        let existing = dir.path().join("image_1.png");
        fs::write(&existing, b"preserve").unwrap();
        let mut calls = 0;
        let error = download_with(&image, dir.path(), 1, |stage, dest| {
            calls += 1;
            let entries: Vec<_> = fs::read_dir(dir.path())
                .unwrap()
                .map(|p| p.unwrap().path())
                .collect();
            assert_eq!(entries.len(), 2);
            let staging = entries
                .iter()
                .find(|p| {
                    p.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with(".image-base64-")
                })
                .unwrap();
            assert_eq!(fs::read(staging).unwrap(), b"Hello");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    fs::metadata(staging).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
            if calls == 1 {
                stage.publish(dest)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "injected publication failure",
                ))
            }
        })
        .unwrap_err();
        assert!(error.contains("injected publication failure"));
        assert_eq!(calls, 2);
        assert_eq!(fs::read(existing).unwrap(), b"preserve");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(
            fs::read(download(&image, dir.path(), 2).unwrap()).unwrap(),
            b"Hello"
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    #[cfg(unix)]
    #[test]
    fn staging_publication_and_cleanup_use_open_directory() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".qa/unit");
        fs::create_dir_all(&root).unwrap();
        let dir = tempfile::tempdir_in(root).unwrap();
        let downloads = dir.path().join("Downloads");
        let moved = dir.path().join("moved");
        fs::create_dir(&downloads).unwrap();
        let mut stage = StagedDownload::new(&downloads).unwrap();
        stage.file.write_all(b"exact").unwrap();
        fs::rename(&downloads, &moved).unwrap();
        fs::create_dir(&downloads).unwrap();
        fs::write(downloads.join("unrelated"), b"preserve").unwrap();
        stage.publish(&downloads.join("image_1.png")).unwrap();
        drop(stage);
        assert_eq!(fs::read(moved.join("image_1.png")).unwrap(), b"exact");
        assert_eq!(fs::read_dir(&moved).unwrap().count(), 1);
        assert_eq!(fs::read_dir(&downloads).unwrap().count(), 1);
        assert_eq!(fs::read(downloads.join("unrelated")).unwrap(), b"preserve");
    }
}
