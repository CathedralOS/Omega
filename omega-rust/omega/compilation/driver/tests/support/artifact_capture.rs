//! Opt-in immutable corpus artifacts for pinned qualification. Capture and
//! verification observe existing products; neither changes compilation or
//! supplies runtime authority. Verification never replaces retained bytes.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

pub(super) struct Capture {
    directory: PathBuf,
    verify: bool,
}

impl Capture {
    pub(super) fn from_environment(
        tier: &str,
        fixture: &str,
        target: &str,
    ) -> Result<Option<Self>, String> {
        let capture = std::env::var_os("OMEGA_CORPUS_CAPTURE_ARTIFACTS");
        let verify = std::env::var_os("OMEGA_CORPUS_VERIFY_ARTIFACTS");
        let (root, verify) = match (capture, verify) {
            (None, None) => return Ok(None),
            (Some(root), None) => (root, false),
            (None, Some(root)) => (root, true),
            (Some(_), Some(_)) => {
                return Err(
                    "corpus artifact capture and verification are mutually exclusive".to_owned(),
                );
            }
        };
        let relative = Path::new(tier).join(fixture).join(target);
        if !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(
                "corpus artifact identity is not a relative fixture/target path".to_owned(),
            );
        }
        Ok(Some(Self {
            directory: PathBuf::from(root).join(relative),
            verify,
        }))
    }

    pub(super) fn observe(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.directory.join(name);
        if self.verify {
            return self.compare(name, bytes);
        }
        fs::create_dir_all(&self.directory)
            .map_err(|error| format!("create corpus artifact directory: {error}"))?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                format!(
                    "create immutable corpus artifact {}: {error}",
                    path.display()
                )
            })?;
        output
            .write_all(bytes)
            .map_err(|error| format!("retain corpus artifact {}: {error}", path.display()))?;
        let mut permissions = output
            .metadata()
            .map_err(|error| format!("read corpus artifact permissions: {error}"))?
            .permissions();
        permissions.set_readonly(true);
        output
            .set_permissions(permissions)
            .map_err(|error| format!("seal corpus artifact permissions: {error}"))
    }

    fn compare(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.directory.join(name);
        let retained = fs::read(&path).map_err(|error| {
            format!("read retained corpus artifact {}: {error}", path.display())
        })?;
        if retained != bytes {
            return Err(format!(
                "corpus artifact bytes differ from retained {}",
                path.display()
            ));
        }
        Ok(())
    }

    pub(super) fn observe_executable(&self, executable: &Path) -> Result<(), String> {
        let bytes = fs::read(executable)
            .map_err(|error| format!("read published corpus executable: {error}"))?;
        self.observe("native-program.exe", &bytes)?;
        if !self.verify {
            let mut permissions = fs::metadata(executable)
                .map_err(|error| format!("read published executable permissions: {error}"))?
                .permissions();
            permissions.set_readonly(true);
            fs::set_permissions(self.directory.join("native-program.exe"), permissions)
                .map_err(|error| format!("retain executable permissions: {error}"))?;
        }
        Ok(())
    }

    /// Check the actual pathname's bytes and filesystem identity. Unix change
    /// time detects writes restored to identical contents and modification time.
    pub(super) fn verify_executable(&self, executable: &Path) -> Result<ExecutableStamp, String> {
        let before = ExecutableStamp::read(executable)?;
        let bytes = fs::read(executable)
            .map_err(|error| format!("read executing corpus image: {error}"))?;
        self.compare("native-program.exe", &bytes)?;
        if before != ExecutableStamp::read(executable)? {
            return Err("executing corpus image changed while reading".to_owned());
        }
        Ok(before)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ExecutableStamp {
    identity: (u64, u64),
    length: u64,
    modified: std::time::SystemTime,
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    changed: (i64, i64),
}

impl ExecutableStamp {
    fn read(path: &Path) -> Result<Self, String> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| format!("read executing corpus image metadata: {error}"))?;
        if !metadata.is_file() {
            return Err("executing corpus image is not an ordinary file".to_owned());
        }
        Ok(Self {
            identity: platform_custody::filesystem_object_identity(path)
                .map_err(|error| format!("read executing corpus image identity: {error}"))?,
            length: metadata.len(),
            modified: metadata.modified().map_err(|error| {
                format!("read executing corpus image modification time: {error}")
            })?,
            created: metadata.created().ok(),
            #[cfg(unix)]
            changed: {
                use std::os::unix::fs::MetadataExt;
                (metadata.ctime(), metadata.ctime_nsec())
            },
        })
    }
}
