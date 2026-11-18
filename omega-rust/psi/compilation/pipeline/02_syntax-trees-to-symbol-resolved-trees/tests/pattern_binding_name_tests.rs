/*
    A match or transition arm binding shares one namespace with the
    machine's parameters and earlier locals
    (`wiki/spec/language/state_contracts.md`, "Bindings"). Each negative
    program differs from its positive twin only in the binding's name, so
    a refusal can come only from the reused name.
*/

use super::domain_lookup_tests::resolve_result;

const READING: &str = "data Reading { case Sample(value: u32); case Idle; }\n";

fn refusal(body: &str) -> String {
    let source = format!("{READING}{body}");
    let errors = resolve_result(&source).expect_err("a reused binding name rejects");
    errors
        .iter()
        .map(|error| error.message.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

fn accepts(body: &str) {
    let source = format!("{READING}{body}");
    if let Err(errors) = resolve_result(&source) {
        panic!("expected acceptance, got {errors:?}");
    }
}

#[test]
fn a_match_arm_binding_may_not_reuse_a_parameter_name() {
    let positive = "machine pick(limit: u32, r: Reading) -> u32 { \
             match r { Reading::Sample { value } -> value  Reading::Idle -> limit } }";
    accepts(positive);
    let message = refusal(&positive.replace("limit", "value"));
    assert!(
        message.contains("pattern binding `value` conflicts with an existing name"),
        "{message}"
    );
}

#[test]
fn a_match_arm_binding_may_not_reuse_an_earlier_local_name() {
    let positive = "machine pick(r: Reading) -> u32 { let base: u32 = 1; \
             match r { Reading::Sample { value } -> value  Reading::Idle -> base } }";
    accepts(positive);
    let message = refusal(&positive.replace("base", "value"));
    assert!(
        message.contains("pattern binding `value` conflicts with an existing name"),
        "{message}"
    );
}

#[test]
fn a_transition_arm_binding_may_not_reuse_a_parameter_name() {
    let positive = "machine pick(limit: u32, r: Reading) -> u32 { \
             transition r { Reading::Sample { value } -> value  Reading::Idle -> limit } }";
    accepts(positive);
    let message = refusal(&positive.replace("limit", "value"));
    assert!(
        message.contains("pattern binding `value` conflicts with an existing name"),
        "{message}"
    );
}

#[test]
fn a_nested_arm_binding_may_not_reuse_its_enclosing_arm_binding() {
    let positive = "machine pick(r: Reading, s: Reading) -> u32 { \
             match r { Reading::Sample { value } -> match s { \
             Reading::Sample { value as inner } -> inner  Reading::Idle -> value } \
             Reading::Idle -> 0 } }";
    accepts(positive);
    let message = refusal(&positive.replace("value as inner } -> inner", "value } -> value"));
    assert!(
        message.contains("pattern binding `value` conflicts with an existing name"),
        "{message}"
    );
}

#[test]
fn different_arms_of_one_match_may_reuse_a_binding_name() {
    accepts(
        "machine pick(a: Reading, b: Reading) -> u32 { \
             let first: u32 = match a { Reading::Sample { value } -> value  Reading::Idle -> 0 }; \
             match b { Reading::Sample { value } -> value + first  Reading::Idle -> first } }",
    );
}
