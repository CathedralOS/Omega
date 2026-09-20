//! Fixtures shared by the domain, control and structure canaries.

#[path = "domains_control_and_structures/arithmetic_and_float_control.rs"]
mod arithmetic_and_float_control;
#[path = "domains_control_and_structures/domain_guards.rs"]
mod domain_guards;
#[path = "../fixture_rosters/domains_control_and_structures.rs"]
pub(super) mod fixture_roster;
#[path = "domains_control_and_structures/ranked_callee_projected_receiver.rs"]
mod ranked_callee_projected_receiver;
#[path = "domains_control_and_structures/structure_and_slice_canaries.rs"]
mod structure_and_slice_canaries;

use crate::{
    Command, compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host,
    executable_name, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

fn assert_float_trapping_policy_canary_aborts(name: &str, reason_fragment: &str) {
    let canary = pass_canary(name);
    let suffix = name.replace(['/', '\\'], "-");
    let build_dir = std::env::temp_dir().join(format!("omega-f5-{suffix}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("Trapping float policy canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("Trapping float policy canary should run");
    assert_ne!(
        output.status.code(),
        Some(7),
        "expected `{name}` to trap before its sailed-past exit"
    );
    assert!(
        !output.status.success(),
        "expected `{name}` to terminate abnormally"
    );
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("Trapping float policy canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    let reason = outcome
        .error
        .expect("the interpreter must trap the float policy violation");
    assert!(
        reason.contains(reason_fragment),
        "expected `{name}` trap reason to contain `{reason_fragment}`, got: {reason}"
    );
    let _ = fs::remove_dir_all(&build_dir);
}
