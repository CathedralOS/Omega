//! Cargo may put its outputs inside the source tree. A nonempty --target-dir
//! has no cache tag, and the command-line option is not exported as an environment
//! variable. Use Cargo's locked profile directory in that case.
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub(crate) struct CargoOutputs<'a> {
    dedicated: Option<PathBuf>,
    profile: Option<&'a Path>,
    target_root: Option<&'a Path>,
}

impl<'a> CargoOutputs<'a> {
    pub(crate) fn new(
        output: &'a Path,
        configured: Option<PathBuf>,
        target: Option<&OsStr>,
    ) -> Self {
        let dedicated = configured
            .filter(|path| output.starts_with(path) && !path.join("Cargo.toml").is_file())
            .or_else(|| {
                output
                    .ancestors()
                    .find(|path| {
                        path.join("CACHEDIR.TAG").is_file() && !path.join("Cargo.toml").is_file()
                    })
                    .map(Path::to_owned)
            });
        let profile = output
            .ancestors()
            .filter(|path| path.file_name() == Some(OsStr::new("build")))
            .filter_map(Path::parent)
            .find(|path| path.join(".cargo-lock").is_file());
        let target_root = profile.and_then(Path::parent).and_then(|parent| {
            if target.is_some() && parent.file_name() == target {
                parent.parent()
            } else {
                Some(parent)
            }
        });
        Self {
            dedicated,
            profile,
            target_root,
        }
    }

    pub(crate) fn contains(&self, path: &Path) -> bool {
        if self.dedicated.as_deref() == Some(path) || self.profile == Some(path) {
            return true;
        }
        let Some(relative) = self
            .target_root
            .and_then(|root| path.strip_prefix(root).ok())
        else {
            return false;
        };
        // An unmarked target may share a directory with authored inputs. Omit
        // Cargo's own runtime metadata and other locked profiles, keeping
        // source files beside them in the fingerprint.
        relative == Path::new(".rustc_info.json")
            || matches!(relative.components().count(), 1 | 2)
                && path.join(".cargo-lock").is_file()
                && path.join("build").is_dir()
    }
}
