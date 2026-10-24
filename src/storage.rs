use crate::{
    error::{Error, ErrorCode, Result},
    json,
    verification::{hash, VerificationKey},
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroize;

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct State {
    pub token: Option<String>,
    pub envelope: Option<String>,
    pub verification_key: Option<VerificationKey>,
    pub checkpoint: Option<String>,
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some(t) = &mut self.token {
            t.zeroize();
        }
    }
}
pub(crate) struct Store {
    directory: PathBuf,
    pub installation: String,
}
pub(crate) struct Guard(File);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}
fn storage<T>(result: std::io::Result<T>) -> Result<T> {
    result.map_err(|_| Error(ErrorCode::Storage))
}
fn private_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        storage(builder.create(path))?;
    }
    let meta = storage(fs::symlink_metadata(path))?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(Error(ErrorCode::Storage));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if meta.uid() != unsafe { libc::geteuid() } {
            return Err(Error(ErrorCode::Storage));
        }
        storage(fs::set_permissions(path, fs::Permissions::from_mode(0o700)))?;
    }
    Ok(())
}
fn open_private(path: &Path, create: bool) -> Result<File> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(Error(ErrorCode::Storage));
        }
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(create)
        .create(create)
        .truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = storage(options.open(path))?;
    let meta = storage(file.metadata())?;
    if !meta.is_file() || meta.len() > json::MAX_BYTES as u64 {
        return Err(Error(ErrorCode::Storage));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.uid() != unsafe { libc::geteuid() } || meta.nlink() != 1 || meta.mode() & 0o077 != 0
        {
            return Err(Error(ErrorCode::Storage));
        }
    }
    Ok(file)
}
impl Store {
    pub fn new(root: &Path, namespace: &str) -> Result<Self> {
        if !root.is_absolute() {
            return Err(Error(ErrorCode::Configuration));
        }
        // Caller supplies a trusted private state root; never use a public web directory.
        private_dir(root)?;
        let root = storage(root.canonicalize())?;
        let directory = root.join(&hash(namespace.as_bytes())[..32]);
        private_dir(&directory)?;
        let mut store = Self {
            directory,
            installation: String::new(),
        };
        let _guard = store.lock()?;
        let identity = store.directory.join("installation.json");
        let id = match fs::symlink_metadata(&identity) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let id = format!("kdp-install-{}", uuid::Uuid::new_v4());
                store.write(
                    "installation.json",
                    &serde_json::to_vec(&serde_json::json!({"installation_id":id}))
                        .map_err(|_| Error(ErrorCode::Storage))?,
                )?;
                id
            }
            Err(_) => return Err(Error(ErrorCode::Storage)),
            Ok(_) => {
                let raw = store.read("installation.json")?;
                json::value(&raw).map_err(|_| Error(ErrorCode::Storage))?["installation_id"]
                    .as_str()
                    .ok_or(Error(ErrorCode::Storage))?
                    .to_string()
            }
        };
        if !id.starts_with("kdp-install-") || uuid::Uuid::parse_str(&id[12..]).is_err() {
            return Err(Error(ErrorCode::Storage));
        }
        store.installation = id;
        Ok(store)
    }
    pub fn lock(&self) -> Result<Guard> {
        let file = open_private(&self.directory.join(".lock"), true)?;
        let start = Instant::now();
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(Guard(file)),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if start.elapsed() > Duration::from_secs(3) {
                        return Err(Error(ErrorCode::Busy));
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(_) => return Err(Error(ErrorCode::Storage)),
            }
        }
    }
    fn read(&self, name: &str) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        storage(
            open_private(&self.directory.join(name), false)?
                .take((json::MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes),
        )?;
        if bytes.len() > json::MAX_BYTES {
            return Err(Error(ErrorCode::Storage));
        }
        Ok(bytes)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> Result<()> {
        if bytes.len() > json::MAX_BYTES {
            return Err(Error(ErrorCode::Storage));
        }
        let target = self.directory.join(name);
        if let Ok(meta) = fs::symlink_metadata(&target) {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(Error(ErrorCode::Storage));
            }
        }
        let mut temp = storage(tempfile::NamedTempFile::new_in(&self.directory))?;
        storage(temp.write_all(bytes))?;
        storage(temp.as_file().sync_all())?;
        temp.persist(&target)
            .map_err(|_| Error(ErrorCode::Storage))?;
        #[cfg(unix)]
        storage(File::open(&self.directory).and_then(|f| f.sync_all()))?;
        Ok(())
    }
    pub fn verify_identity(&self) -> Result<()> {
        let identity =
            json::value(&self.read("installation.json")?).map_err(|_| Error(ErrorCode::Storage))?;
        if identity["installation_id"] != self.installation {
            return Err(Error(ErrorCode::Storage));
        }
        Ok(())
    }
    pub fn load(&self) -> Result<State> {
        self.verify_identity()?;
        let path = self.directory.join("state.json");
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
            Err(_) => Err(Error(ErrorCode::Storage)),
            Ok(_) => {
                let raw = self.read("state.json")?;
                json::parse(&raw).map_err(|_| Error(ErrorCode::Storage))?;
                let state: State =
                    serde_json::from_slice(&raw).map_err(|_| Error(ErrorCode::Storage))?;
                if state.token.as_ref().is_some_and(|t| !valid_token(t)) {
                    return Err(Error(ErrorCode::Storage));
                }
                Ok(state)
            }
        }
    }
    pub fn save(&self, state: &State) -> Result<()> {
        let mut raw = serde_json::to_vec(state).map_err(|_| Error(ErrorCode::Storage))?;
        let result = self.write("state.json", &raw);
        raw.zeroize();
        result
    }
    pub fn clear(&self) -> Result<()> {
        match fs::remove_file(self.directory.join("state.json")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(Error(ErrorCode::Storage)),
        }
    }
}
pub(crate) fn valid_token(token: &str) -> bool {
    token.starts_with("act_")
        && (8..=1024).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
pub(crate) fn default_root() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    base.map(|b| b.join("KDPika Licensing Rust"))
        .ok_or(Error(ErrorCode::Storage))
}
