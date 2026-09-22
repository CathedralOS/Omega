//! Literal-dispatch unit-plan stop: the `runtime_string_literal_dispatch_exit`
//! pass canary is registered active in `canary_suite.rs`, but still fails
//! ProgramEntry establishment once its target entry binding is admitted: the
//! selected ProgramEntry rejoins zero Terminal attachment identities because
//! the machine's unit plan was omitted at local construction. The fixture
//! pins its omission cause (its integer sibling,
//! `control_flow/runtime_integer_literal_dispatch_exit`, compiles clean since
//! the transition-chain tail became an admitted unit-plan terminator, so its
//! pin retired into the fixture's active-pass claim):
//!
//! - `control_flow/runtime_string_literal_dispatch_exit` stops at
//!   `structural field store: record literal field` — establishing the
//!   `"look"`/`"quit"` input string reaches a record-literal field store the
//!   unit plan does not admit.
//!
//! The refusal surfaces during checked compilation as soon as an entry binding
//! authorizes ProgramEntry establishment, so the pin compiles to checked and
//! asserts the exact diagnostic — the terminal-artifact route reports the
//! identical refusal. When the unit-plan admission lands, this pin flips to
//! the fixture's active-pass claim and this file can be retired.

use compiler::{CheckedCompileRequest, compile_to_checked};
use diagnostics::Diagnostic;
use fixture_package_inputs::{
    bundled_standard_library_root, candidate_program_entry_binding, fixture_package_identity,
    repo_root, repository_fixture_package_inputs,
};
use package_compilation::PackageCompilationInputs;
use std::path::Path;

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

const STRING_DISPATCH_CANARY: &str = "control_flow/runtime_string_literal_dispatch_exit";
const ENTRY_REJOIN_PREFIX: &str =
    "selected ProgramEntry establishment rejoins 0 Terminal attachment identities";
const STRING_OMISSION: &str = "structural field store: record literal field";

/// The fixture's authored `omega_language_std` package closure with the exact
/// entry schema for `target_name` accepted, so the checked compile attempts
/// ProgramEntry establishment. Test acceptance only; it is not evidence that
/// a production audit occurred.
fn entry_admitted_package_inputs(
    project_root: &Path,
    target_name: &str,
) -> Result<PackageCompilationInputs, Vec<Diagnostic>> {
    let package_inputs = repository_fixture_package_inputs(&project_root.join("main.omg"))
        .expect("a literal-dispatch canary authors its std dependency");
    let entry = candidate_program_entry_binding(
        target_name,
        &bundled_standard_library_root(),
        fixture_package_identity(2),
    )?
    .unwrap_or_else(|| panic!("literal-dispatch canary has no entry binding for {target_name}"));

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
fn string_literal_dispatch_stops_at_record_literal_field_store() {
    assert_stops_at_omitted_unit_plan(STRING_DISPATCH_CANARY, STRING_OMISSION);
}
