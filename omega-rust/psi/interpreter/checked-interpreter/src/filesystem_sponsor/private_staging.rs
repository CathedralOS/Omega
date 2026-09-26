//! Fresh private directory custody shared by clones of one sponsor.
//!
//! Creation never adopts an existing directory. Explicit disposal reports
//! errors and revokes eligibility immediately; final-owner drop retries cleanup.
//! This is filesystem lifecycle evidence, not grant or acceptance policy.

use super::{FilesystemSponsor, normalize_absolute};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(super) struct PrivateStaging {
    root: PathBuf,
    state: Mutex<StagingState>,
}

#[derive(Debug)]
struct StagingState {
    live: bool,
    removed: bool,
}

impl PrivateStaging {
    fn dispose(&self) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("private staging custody lock poisoned"))?;
        state.live = false;
        if state.removed {
            return Ok(());
        }
        match std::fs::remove_dir_all(&self.root) {
            Ok(()) => state.removed = true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => state.removed = true,
            Err(error) => return Err(error),
        }
        Ok(())
    }
}

impl Drop for PrivateStaging {
    fn drop(&mut self) {
        let _ = self.dispose();
    }
}

impl FilesystemSponsor {
    /// Create, rather than adopt, one private bounded staging session.
    /// Unix permissions exclude other users; Windows uses the caller's
    /// existing inherited-directory isolation premise.
    pub fn create_private(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "private staging requires a directory leaf",
            )
        })?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = std::fs::canonicalize(parent)?;
        let root = parent.join(name);
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&root)?;
        let owner = Arc::new(PrivateStaging {
            root: root.clone(),
            state: Mutex::new(StagingState {
                live: true,
                removed: false,
            }),
        });
        let canonical = std::fs::canonicalize(&root)?;
        if canonical != root || canonical.parent() != Some(parent.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "private staging escaped its canonical parent",
            ));
        }
        let mut sponsor = Self::new(&canonical).map_err(io::Error::other)?;
        sponsor.private_staging = Some(owner);
        Ok(sponsor)
    }

    /// Whether a strict descendant belongs to this live private session.
    /// Grant validation must still enforce its own no-follow path protocol.
    pub fn owns_private_staging_root(&self, root: &Path) -> bool {
        let Some(owner) = &self.private_staging else {
            return false;
        };
        let Ok(root) = normalize_absolute(root) else {
            return false;
        };
        owner
            .state
            .lock()
            .is_ok_and(|state| state.live && root != owner.root && root.starts_with(&owner.root))
    }

    /// Dispose private staging and revoke all clones' ownership eligibility.
    /// A generic sponsor owns no directory and has nothing to dispose.
    pub fn dispose_private_staging(&self) -> io::Result<()> {
        match &self.private_staging {
            Some(owner) => owner.dispose(),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FilesystemSponsor;
    use std::io;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "omega-private-sponsor-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn generic_sponsors_have_no_private_directory_custody() {
        let path = root();
        let sponsor = FilesystemSponsor::new(&path).unwrap();
        assert!(!sponsor.owns_private_staging_root(&path.join("output")));
        sponsor.dispose_private_staging().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn clones_retain_private_directory_until_last_owner_drops() {
        let path = root();
        let sponsor = FilesystemSponsor::create_private(&path).unwrap();
        let canonical = sponsor.session_root().unwrap();
        assert!(sponsor.owns_private_staging_root(&canonical.join("output")));
        assert!(!sponsor.owns_private_staging_root(&canonical));
        assert!(!sponsor.owns_private_staging_root(&canonical.join("../foreign")));
        let retained = sponsor.clone();
        assert_eq!(sponsor, retained);
        drop(sponsor);
        assert!(path.is_dir());
        drop(retained);
        assert!(!path.exists());
    }

    #[test]
    fn explicit_disposal_revokes_every_clone_and_is_idempotent() {
        let path = root();
        let sponsor = FilesystemSponsor::create_private(&path).unwrap();
        let output = sponsor.session_root().unwrap().join("output");
        let retained = sponsor.clone();
        sponsor.dispose_private_staging().unwrap();
        assert!(!retained.owns_private_staging_root(&output));
        assert!(!path.exists());
        retained.dispose_private_staging().unwrap();
    }

    #[test]
    fn existing_directory_is_never_adopted_or_removed() {
        let path = root();
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("retained"), b"existing").unwrap();
        assert_eq!(
            FilesystemSponsor::create_private(&path).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(path.join("retained")).unwrap(), b"existing");
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn cleanup_failure_is_reported_and_revokes_eligibility() {
        let path = root();
        let sponsor = FilesystemSponsor::create_private(&path).unwrap();
        let output = sponsor.session_root().unwrap().join("output");
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, b"replacement is not a directory").unwrap();
        assert!(sponsor.dispose_private_staging().is_err());
        assert!(!sponsor.owns_private_staging_root(&output));
        std::fs::remove_file(&path).unwrap();
        sponsor.dispose_private_staging().unwrap();
    }
}
