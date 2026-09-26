//! Fixtures shared by the package compilation tests: temporary trees,
//! identities, bindings and generated bundles.

mod captured_source_inputs;
mod generated_bundles_and_source_roots;
mod semantic_bindings_and_inputs;

use crate::package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, PackageCompilationInputs,
    PackageDependencyBinding, PackageGeneratedSource, PackageGeneratedSourceBundle,
    PackageKeyIdentity, PackageSourceBinding, PackageSourceConsumptionCommitment, Path, PathBuf,
};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-compilation-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary package tree");
        Self(path)
    }

    fn package(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).expect("create package root");
        path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

fn accepted_console_binding(package: PackageKeyIdentity, marker: u8) -> AcceptedSemanticBinding {
    AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        package,
        "Console",
        effects::provider_plan::ServiceSchemaDigest::from_digest([marker; 32]),
        effects::provider_plan::ProviderPlanDigest::from_digest([marker.wrapping_add(1); 32]),
    )
    .expect("valid accepted semantic binding")
}

fn generated_bundle(
    inputs: &PackageCompilationInputs,
    package: PackageKeyIdentity,
    target: target::TargetProfile,
    commitment_marker: u8,
    sources: Vec<PackageGeneratedSource>,
) -> PackageGeneratedSourceBundle {
    PackageGeneratedSourceBundle::from_checked(
        package,
        build_declarations::DependencyPurpose::Product,
        target,
        target::TargetProfile::host_if_supported(),
        inputs.dependency_closure_for(package),
        PackageSourceConsumptionCommitment::for_test([commitment_marker; 32]),
        sources,
    )
}

fn generated_source(relative_path: &[u8], bytes: &[u8]) -> PackageGeneratedSource {
    let tree = build_output::from_entries(&[build_output::OutputTreeEntry::regular_file(
        relative_path,
        bytes,
        false,
    )])
    .expect("test source must form a canonical retained output tree");
    build_output::select_included_sources(&tree, &[relative_path.to_vec()])
        .expect("test source must be explicitly included")
        .pop()
        .expect("one included test source must be retained")
}

fn three_package_generated_inputs(tree: &TempTree) -> PackageCompilationInputs {
    PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", tree.package("root")),
            PackageSourceBinding::new(identity(2), "middle", tree.package("middle")),
            PackageSourceBinding::new(identity(3), "leaf", tree.package("leaf")),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("generated-source test graph should close")
}

fn seal_source_tree(root: &Path) {
    set_source_tree_permissions(root, true)
}

fn unseal_source_tree(root: &Path) {
    set_source_tree_permissions(root, false)
}

#[cfg(unix)]
fn set_source_tree_permissions(root: &Path, sealed: bool) {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::symlink_metadata(root).expect("inspect source tree entry");
    if metadata.file_type().is_symlink() {
        return;
    }
    if metadata.is_dir() {
        if !sealed {
            fs::set_permissions(root, fs::Permissions::from_mode(0o755))
                .expect("unseal source directory");
        }
        for entry in fs::read_dir(root).expect("enumerate source directory") {
            set_source_tree_permissions(&entry.expect("read source entry").path(), sealed);
        }
        if sealed {
            fs::set_permissions(root, fs::Permissions::from_mode(0o555))
                .expect("seal source directory");
        }
    } else {
        let executable = metadata.permissions().mode() & 0o111 != 0;
        let mode = match (sealed, executable) {
            (true, true) => 0o555,
            (true, false) => 0o444,
            (false, true) => 0o755,
            (false, false) => 0o644,
        };
        fs::set_permissions(root, fs::Permissions::from_mode(mode))
            .expect("set source file permissions");
    }
}

#[cfg(not(unix))]
fn set_source_tree_permissions(_root: &Path, _sealed: bool) {}
