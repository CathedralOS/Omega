//! Fixtures shared by the package compilation input tests: identities and
//! written programs.

#[path = "package_compilation_inputs/artifact_identities_and_entries.rs"]
mod artifact_identities_and_entries;
#[path = "package_compilation_inputs/authority_and_build_files.rs"]
mod authority_and_build_files;
#[path = "package_compilation_inputs/cross_package_visibility.rs"]
mod cross_package_visibility;
#[path = "fixture_rosters/package_compilation_inputs.rs"]
mod fixtures;
#[path = "package_compilation_inputs/generated_sources_and_dependencies.rs"]
mod generated_sources_and_dependencies;
#[path = "package_compilation_inputs/generic_visibility.rs"]
mod generic_visibility;
#[path = "package_compilation_inputs/independent_components.rs"]
mod independent_components;
#[path = "package_compilation_inputs/module_constants.rs"]
mod module_constants;
#[path = "package_compilation_inputs/module_generic_data.rs"]
mod module_generic_data;
#[path = "package_compilation_inputs/module_namespaces.rs"]
mod module_namespaces;
#[path = "package_compilation_inputs/module_template_methods.rs"]
mod module_template_methods;
#[path = "package_compilation_inputs/selected_const_evaluation.rs"]
mod selected_const_evaluation;

use compiler::{CheckedCompileRequest, compile_to_checked};
use package_compilation::{
    BuildDeclarationKind, PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-inputs-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary package compilation tree");
        Self(path)
    }

    fn package(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).expect("create package directory");
        path
    }

    fn write(path: impl AsRef<Path>, source: &str) {
        fs::write(path, source).expect("write Omega package test source");
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

fn generated_source(relative_path: &[u8], bytes: &[u8]) -> build_output::PackageGeneratedSource {
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

fn compile_provider_mode_fixture(
    marker: u8,
    package_name: &str,
    extra_source: &str,
    selections: &str,
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let tree = TempTree::new();
    let root = tree.package(package_name);

    TempTree::write(
        root.join("main.omg"),
        &format!(
            r#"pub boundary trait Pair {{ machine first(); }}
pub data Provider {{ first: addr; }}
machine Provider::first() satisfies Pair::first via Binding::VtableField(first);
{extra_source}
"#
        ),
    );
    TempTree::write(
        root.join("build.omg"),
        &format!(
            r#"machine build(builder: &mut Build) {{
    builder.package("{package_name}");
{selections}
}}
"#
        ),
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(marker),
        vec![PackageSourceBinding::new(
            identity(marker),
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("one-package provider graph");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
}

fn host_target_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows_x86_64"),
        ("linux", "x86_64") => Some("linux_x86_64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        _ => None,
    }
}
