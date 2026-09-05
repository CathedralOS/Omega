use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub(crate) struct TempPackage(pub(crate) PathBuf);

impl TempPackage {
    pub(crate) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-evidence-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create package review fixture");
        Self(path)
    }

    pub(crate) fn write(&self, path: impl AsRef<Path>, source: &str) {
        fs::write(self.0.join(path), source).expect("write package review fixture source");
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn package_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([41; 32]).expect("nonzero package identity")
}

pub(crate) fn package_inputs(root: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new_package(
        package_identity(),
        vec![PackageSourceBinding::new(
            package_identity(),
            "review-fixture",
            root.to_owned(),
        )],
        Vec::new(),
    )
    .expect("single-package review graph should validate")
}
