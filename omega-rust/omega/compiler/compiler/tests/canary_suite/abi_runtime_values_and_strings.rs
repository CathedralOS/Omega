//! Fixtures shared by the ABI, runtime value and string canaries.

#[path = "abi_runtime_values_and_strings/abi_imports_and_results.rs"]
mod abi_imports_and_results;
#[path = "../fixture_rosters/abi_runtime_values_and_strings.rs"]
pub(super) mod fixture_roster;
#[path = "abi_runtime_values_and_strings/indexed_writes_and_loops.rs"]
mod indexed_writes_and_loops;
#[path = "abi_runtime_values_and_strings/runtime_text_and_transitions.rs"]
mod runtime_text_and_transitions;

fn application_build() -> String {
    "machine build(builder: &mut Build) {\n    builder.application(\"cross_target_canary\");\n}\n"
        .to_owned()
}
