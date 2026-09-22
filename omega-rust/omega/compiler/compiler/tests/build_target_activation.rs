//! Fixtures shared by the build target activation tests: temporary
//! projects, package inputs and the hosted macOS entry program.

#[path = "build_target_activation/activation_identifiers_and_publication.rs"]
mod activation_identifiers_and_publication;
#[path = "build_target_activation/executed_provider_selection.rs"]
mod executed_provider_selection;
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "fixture_rosters/build_target_activation.rs"]
mod fixtures;
#[path = "build_target_activation/foreign_helper_product_queries.rs"]
mod foreign_helper_product_queries;
#[path = "build_target_activation/product_entry_signatures.rs"]
mod product_entry_signatures;
#[path = "build_target_activation/product_query_paths.rs"]
mod product_query_paths;
#[path = "build_target_activation/qualified_provider_selection.rs"]
mod qualified_provider_selection;
#[path = "build_target_activation/qualified_root_bindings.rs"]
mod qualified_root_bindings;
#[path = "build_target_activation/x86_feature_admission.rs"]
mod x86_feature_admission;

use build_declarations::DependencyPurpose;
use compiler::CheckedCompileRequest;
use compiler::compile_to_checked;
use fixture_package_inputs::{
    bundled_standard_library_root, console_acceptance, fixture_package_identity,
    linux_entry_acceptance, macos_entry_acceptance, repository_fixture_package_inputs,
    standard_library_package_inputs, windows_entry_acceptance,
};
use package_compilation::{
    BuildDeclarationKind, PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempProject(PathBuf);

impl TempProject {
    fn new(build: &str) -> Self {
        Self::with_main("const ANSWER: u32 = 42;\n", build)
    }

    fn with_main(main: &str, build: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../target/omega-test-projects")
            .join(format!(
                "omega-build-target-activation-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).expect("create temporary in-repository Omega project");
        fs::write(path.join("main.omg"), main).expect("write temporary Omega source");
        fs::write(path.join("build.omg"), build).expect("write temporary Omega build source");
        Self(path)
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn exact_target_build(body: &str) -> String {
    application_build(body)
}

fn application_build(body: &str) -> String {
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"target-activation\");\n{body}\n}}\n"
    )
}

fn pass_canary_main(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/omega/pass")
        .join(name)
        .join("main.omg")
}

/// Root + bundled-std package inputs for one std-linked fixture: the graph a
/// repository canary authored in its `build.omg`, or, for a temporary project
/// that writes no dependency row, the standard library stated explicitly.
fn package_inputs_with_standard_library(main: &std::path::Path) -> PackageCompilationInputs {
    repository_fixture_package_inputs(main).unwrap_or_else(|| {
        standard_library_package_inputs(
            main.parent().expect("project root"),
            &bundled_standard_library_root(),
        )
    })
}

fn diagnostic_text(project: &TempProject) -> String {
    compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("immutable target violation must reject")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n")
}

const MACOS_HOSTED_MAIN: &str = "data Main { }\nmachine Main::main() { }\n";

/// The owner's `build.omg` imports `support::setup`, so `support` is a
/// build-scope edge: the root build entry's imports resolve only through
/// `build_depend`/`build_depend_as` declarations, and a product `depend` edge
/// used from build code rejects
/// (wiki/spec/build/scoped_execution.md, "Dependency declarations and
/// discovery"). The helper holds no product-scope binding, so the product
/// queries below still answer for the query occurrence's own package.
fn foreign_helper_inputs(project: &TempProject, helper: &TempProject) -> PackageCompilationInputs {
    PackageCompilationInputs::new(
        fixture_package_identity(1),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(
                fixture_package_identity(1),
                "root-binding-owner",
                project.0.clone(),
            ),
            PackageSourceBinding::new(
                fixture_package_identity(2),
                "root-binding-helper",
                helper.0.clone(),
            ),
        ],
        vec![PackageDependencyBinding::for_purpose(
            fixture_package_identity(1),
            "support",
            fixture_package_identity(2),
            DependencyPurpose::Build,
        )],
    )
    .expect("explicit package graph")
}

/// The same owner/helper package pair, but `support` is declared under
/// product scope (`depend`, the `PackageDependencyBinding::new` default). The
/// owner's product sources import helper modules ordinarily, and a qualified
/// `builder.product.*` query path may select the dependency's public
/// declarations under the query occurrence's own product authority.
fn foreign_product_inputs(project: &TempProject, helper: &TempProject) -> PackageCompilationInputs {
    PackageCompilationInputs::new(
        fixture_package_identity(1),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(
                fixture_package_identity(1),
                "root-binding-owner",
                project.0.clone(),
            ),
            PackageSourceBinding::new(
                fixture_package_identity(2),
                "root-binding-helper",
                helper.0.clone(),
            ),
        ],
        vec![PackageDependencyBinding::new(
            fixture_package_identity(1),
            "support",
            fixture_package_identity(2),
        )],
    )
    .expect("explicit package graph")
}
