use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
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
            "review_fixture",
            root.to_owned(),
        )],
        Vec::new(),
    )
    .expect("single-package review graph should validate")
}

pub(crate) fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("Omega repository root")
}

/// The bundled standard library bound as an ordinary dependency.
///
/// Target-selected compilations seed the closed entry contract, which declares
/// its calling vocabulary through `omega::language::std::calling`. A fixture
/// that also copies `calling.omg` into its own package declares that vocabulary
/// twice in one program. Binding `source/library/std` under the
/// `omega_language_std` alias makes the bundled package the single supplier;
/// package-aware sources spell the import `use omega_language_std::calling;`.
pub(crate) fn standard_library_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([77; 32]).expect("nonzero standard-library package identity")
}

pub(crate) fn standard_library_source() -> PackageSourceBinding {
    PackageSourceBinding::new(
        standard_library_identity(),
        "omega_language_std",
        repository_root().join("source/library/std"),
    )
}

pub(crate) fn standard_library_dependency(
    dependent: PackageKeyIdentity,
) -> PackageDependencyBinding {
    PackageDependencyBinding::new(dependent, "omega_language_std", standard_library_identity())
}

pub(crate) fn package_inputs_with_std(root: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new_package(
        package_identity(),
        vec![
            PackageSourceBinding::new(package_identity(), "review_fixture", root.to_owned()),
            standard_library_source(),
        ],
        vec![standard_library_dependency(package_identity())],
    )
    .expect("review graph with the bundled standard library should validate")
}
