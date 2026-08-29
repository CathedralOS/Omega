mod active_aliases;
mod extraction;
mod policy;
mod requests;
mod target_conditions;

use super::{
    DependencyProjectionError, DependencySourceRequest, extract_dependency_projection,
    extraction::BUILD_FILE_NAME,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct PackageFixture {
    root: PathBuf,
}

impl PackageFixture {
    fn empty() -> Self {
        let nonce = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "omega-dependency-projection-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create package fixture");
        Self { root }
    }

    fn with_source(source: &str) -> Self {
        let fixture = Self::empty();
        fs::write(fixture.root.join(BUILD_FILE_NAME), source).expect("write build.omg");
        fixture
    }

    fn extract(&self) -> Result<Vec<DependencySourceRequest>, DependencyProjectionError> {
        extract_dependency_projection(&self.root)
    }
}

impl Drop for PackageFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
