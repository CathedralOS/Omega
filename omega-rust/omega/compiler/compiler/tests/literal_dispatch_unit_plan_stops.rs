//! Literal-dispatch unit-plan stops: the two `runtime_*_literal_dispatch_exit`
//! pass canaries are registered active in `canary_suite.rs`, but each still
//! fails ProgramEntry establishment once its target entry binding is admitted:
//! the selected ProgramEntry rejoins zero Terminal attachment identities
//! because the machine's unit plan was omitted at local construction. Each
//! fixture pins its own omission cause:
//!
//! - `control_flow/runtime_integer_literal_dispatch_exit` stops at
//!   `state graph: terminator: unsupported tail: transition chain` — the
//!   integer `transition` chain is not yet an admitted unit-plan tail.
//! - `control_flow/runtime_string_literal_dispatch_exit` stops at
//!   `structural field store: record literal field` — establishing the
//!   `"look"`/`"quit"` input string reaches a record-literal field store the
//!   unit plan does not admit.
//!
//! The refusal surfaces during checked compilation as soon as an entry binding
//! authorizes ProgramEntry establishment, so the pins compile to checked and
//! assert the exact diagnostics — the terminal-artifact route reports the
//! identical refusals. When the unit-plan admissions land, these pins flip
//! to the fixtures' active-pass claim and this file can be retired.

use build_declarations::{BuildDeclaration, extract_build_declaration};
use compiler::{CheckedCompileRequest, compile_to_checked};
use diagnostics::Diagnostic;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};

#[path = "support/linux_entry_acceptance.rs"]
mod linux_entry_acceptance;
#[path = "support/macos_entry_acceptance.rs"]
mod macos_entry_acceptance;
#[path = "support/windows_entry_acceptance.rs"]
mod windows_entry_acceptance;

const INTEGER_DISPATCH_CANARY: &str = "control_flow/runtime_integer_literal_dispatch_exit";
const STRING_DISPATCH_CANARY: &str = "control_flow/runtime_string_literal_dispatch_exit";
const ENTRY_REJOIN_PREFIX: &str =
    "selected ProgramEntry establishment rejoins 0 Terminal attachment identities";
const INTEGER_OMISSION: &str = "state graph: terminator: unsupported tail: transition chain";
const STRING_OMISSION: &str = "structural field store: record literal field";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler lives under omega-rust/omega/compiler/compiler")
        .to_path_buf()
}

fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32])
        .expect("repository fixture package identity is nonzero")
}

/// Rebuild the fixture's declared `omega_language_std` package closure the
/// same way `canary_suite.rs::reviewed_repository_fixture_package_inputs`
/// does and accept the exact entry schema for `target_name`, so the checked
/// compile attempts ProgramEntry establishment. Test acceptance only; it is
/// not evidence that a production audit occurred.
fn entry_admitted_package_inputs(
    project_root: &Path,
    target_name: &str,
) -> Result<PackageCompilationInputs, Vec<Diagnostic>> {
    let declaration = extract_build_declaration(project_root)
        .unwrap_or_else(|error| panic!("fixture {}: {error}", project_root.display()));
    let root_role = declaration.kind();
    let root_name = match declaration {
        BuildDeclaration::Application(application) => application.name,
        BuildDeclaration::Package(package) => package.name,
        BuildDeclaration::Workspace(_) => {
            panic!(
                "fixture {} cannot be a workspace root",
                project_root.display()
            )
        }
    };
    let root_identity = fixture_package_identity(1);
    let standard_library_identity = fixture_package_identity(2);
    let package_inputs = PackageCompilationInputs::new(
        root_identity,
        root_role,
        vec![
            PackageSourceBinding::new(
                root_identity,
                root_name.into_string(),
                project_root.to_path_buf(),
            ),
            PackageSourceBinding::new(
                standard_library_identity,
                "omega-language-std",
                repo_root().join("source/library/std"),
            ),
        ],
        vec![PackageDependencyBinding::new(
            root_identity,
            "omega_language_std",
            standard_library_identity,
        )],
    )
    .unwrap_or_else(|errors| panic!("fixture {}: {errors:#?}", project_root.display()));

    let standard_library_root = repo_root().join("source/library/std");
    let entry = match target_name {
        "linux_x86_64" => linux_entry_acceptance::candidate_linux_x86_64_entry_binding(
            &standard_library_root,
            standard_library_identity,
        )?,
        "linux_arm64" => linux_entry_acceptance::candidate_linux_arm64_entry_binding(
            &standard_library_root,
            standard_library_identity,
        )?,
        "windows_x86_64" => windows_entry_acceptance::candidate_windows_x86_64_entry_binding(
            &standard_library_root,
            standard_library_identity,
        )?,
        "macos_arm64" => macos_entry_acceptance::candidate_macos_entry_binding(
            &standard_library_root,
            standard_library_identity,
        )?,
        other => panic!("literal-dispatch canary has no entry binding for {other}"),
    };

    package_inputs
        .with_accepted_semantic_bindings(vec![entry])
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit literal-dispatch entry binding: {errors:?}"
            ))]
        })
}

fn assert_stops_at_omitted_unit_plan(canary_path: &str, omission: &str) {
    let project_root = repo_root().join("tests/omega/pass").join(canary_path);
    let target_name = target::TargetProfile::host().target_name();
    let package_inputs =
        entry_admitted_package_inputs(&project_root, target_name).unwrap_or_else(|diagnostics| {
            panic!(
                "{target_name} entry admission for {canary_path} should resolve:\n{diagnostics:#?}"
            )
        });
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&project_root.join("main.omg"), Some(target_name))
    })
    .expect_err(&format!(
        "{canary_path} on {target_name} should still stop at the omitted unit plan \
         `{omission}`; the fixture compiled clean — retire this pin into the active-pass claim"
    ));
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(ENTRY_REJOIN_PREFIX),
        "{canary_path} should reject inside ProgramEntry establishment, got:\n{rendered}"
    );
    assert!(
        rendered.contains(omission),
        "{canary_path} unit plan should be omitted at `{omission}`, got:\n{rendered}"
    );
}

#[test]
fn integer_literal_dispatch_stops_at_transition_chain_tail() {
    assert_stops_at_omitted_unit_plan(INTEGER_DISPATCH_CANARY, INTEGER_OMISSION);
}

#[test]
fn string_literal_dispatch_stops_at_record_literal_field_store() {
    assert_stops_at_omitted_unit_plan(STRING_DISPATCH_CANARY, STRING_OMISSION);
}
