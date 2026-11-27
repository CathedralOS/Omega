//! A required goal of a form the prover has no rule for leaves the request
//! `Incomplete` at code 36, naming the obligation; a goal it has rules for
//! that is left unproved rejects. `wiki/spec/build/compiler_request.md`,
//! "Unproved goal form publication".
#[path = "support/program.rs"]
mod program;

/// `relay` is public, so its call to `gated` must prove the requirement from
/// `relay`'s own contract, which says nothing of `flag`.
fn relayed(parameter: &str, requirement: &str, argument: &str) -> String {
    format!(
        "machine gated(flag: {parameter}) -> u8 requires {requirement} {{\n    1u8\n}}\n\
         \n\
         public machine relay(flag: {parameter}) -> u8 {{\n    gated(flag)\n}}\n\
         \n\
         machine main() {{\n    _ = relay({argument});\n}}\n"
    )
}

#[test]
fn a_goal_without_a_rule_is_incomplete_at_code_36() {
    let (output, report) =
        program::invoke(&relayed("bool", "flag == false", "false"), &["--check"]);
    assert_eq!(output.status.code(), Some(2), "{report}");
    assert!(
        report.contains("[Incomplete code 36 (coverage_proof_goal_forms): obligation "),
        "{report}"
    );
}

#[test]
fn an_unproved_goal_with_rules_rejects_without_a_provision() {
    let (output, report) = program::invoke(&relayed("u8", "flag == 0u8", "0u8"), &["--check"]);
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("cannot prove requires contract for call gated"),
        "{report}"
    );
    assert!(!report.contains("coverage_proof_goal_forms"), "{report}");
}
