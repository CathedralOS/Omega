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
             match r { Reading::Sample { value } -> value,  Reading::Idle -> limit } }";
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
             match r { Reading::Sample { value } -> value,  Reading::Idle -> base } }";
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
             transition r { Reading::Sample { value } -> value,  Reading::Idle -> limit } }";
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
             Reading::Sample { value as inner } -> inner,  Reading::Idle -> value }, \
             Reading::Idle -> 0 } }";
    accepts(positive);
    let message = refusal(&positive.replace("value as inner } -> inner", "value } -> value"));
    assert!(
        message.contains("pattern binding `value` conflicts with an existing name"),
        "{message}"
    );
}

/// "A runtime parameter or local cannot reuse the name of a visible value
/// parameter or local": with two parameters of one name, every use reaches
/// the first and the second cannot be named. The twin renames one parameter.
#[test]
fn two_parameters_of_one_machine_may_not_share_a_name() {
    let positive = "machine pick(first: u32, second: u32) -> u32 { first }";
    accepts(positive);
    let message = refusal(&positive.replace("second", "first"));
    assert!(
        message.contains("has duplicate parameter `first`"),
        "{message}"
    );
}

/// The rule holds for each machine's own parameter list, nested ones too.
#[test]
fn a_nested_machine_may_not_repeat_a_parameter_name() {
    let positive = "machine outer(seed: u32) -> u32 { \
             transition { _ -> inner(seed, seed) } \
             machine inner(left: u32, right: u32) -> u32 { left } }";
    accepts(positive);
    let message = refusal(&positive.replace("right: u32", "left: u32"));
    assert!(
        message.contains("has duplicate parameter `left`"),
        "{message}"
    );
}

#[test]
fn different_arms_of_one_match_may_reuse_a_binding_name() {
    accepts(
        "machine pick(a: Reading, b: Reading) -> u32 { \
             let first: u32 = match a { Reading::Sample { value } -> value,  Reading::Idle -> 0 }; \
             match b { Reading::Sample { value } -> value + first,  Reading::Idle -> first } }",
    );
}

#[test]
fn visible_value_binders_reject_local_shadowing() {
    for source in [
        "machine pick(current: u32, sample: u32) -> u32 { match sample { current -> current, _ -> 0u32 } }",
        "machine pick(sample: u32) -> u32 { let current: u32 = 1u32; match sample { current -> current, _ -> 0u32 } }",
        "machine pick(current: u32, sample: u32) -> u32 { transition sample { current -> current, _ -> 0u32 } }",
        "machine pick(sample: u32) -> u32 { match sample { current -> match sample { current -> current } } }",
    ] {
        let errors = resolve_result(source).expect_err(source);
        assert!(
            errors.iter().any(|error| error.message.contains("current")
                && error.message.contains("conflicts with an existing name")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn generic_name_overlaps_reach_typing_with_the_binding_span() {
    for source in [
        "machine pick<const current: u32>(sample: u32) -> u32 { match sample { current -> current, _ -> 0u32 } }",
        "machine pick<const current: u32>(sample: u32) -> u32 { let current: u32 = sample; current }",
        "machine pick<const current: u32>(current: u32) -> u32 { current }",
        "machine pick<const current: u32>(sample: u32) -> u32 { transition (sample, sample) { (current, _) -> current, _ -> 0u32 } }",
        "data Reading { value: u32; } machine pick<const current: u32>(reading: Reading) -> u32 { match reading { Reading { value as current } -> current } }",
    ] {
        let program =
            resolve_result(source).unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
        let [overlap] = program.generic_binding_overlaps.as_slice() else {
            panic!("expected one generic overlap in {source}");
        };
        assert_eq!(program.symbols.name(overlap.parameter), "current");
        assert_eq!(
            &source[overlap.binding.span.start..overlap.binding.span.end],
            "current"
        );
    }
}

#[test]
fn fresh_subject_patterns_bind_the_value_without_testing_equality() {
    for source in [
        "const current: u32 = 12u32; machine pick(sample: u32) -> u32 { match sample { current -> current } }",
        "machine pick<T>(sample: u32) -> u32 { match sample { T -> T } }",
        "machine pick(sample: u32) -> u32 { match sample { current if current == 0u32 -> current, current -> current } }",
        "machine pick(sample: u32) -> u32 { transition (sample, sample) { (current, _) -> current, (_, current) -> current } }",
    ] {
        let program =
            resolve_result(source).unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
        assert!(
            program
                .tables
                .bodies
                .expressions
                .iter_expressions()
                .any(|(_, node)| matches!(
                    node,
                    crate::source_resolved::expression::ExpressionNode::SavedSubject(_)
                )),
            "{source}"
        );
    }
    accepts(
        "machine pick(current: u32) -> u32 { next(current) machine next(current: u32) -> u32 { current } }",
    );
    accepts("machine pick<T>(sample: u32) -> u32 { let T: u32 = sample; T }");
    accepts(r"machine pick<const T: core\Type<0>>(sample: u32) -> u32 { let T: u32 = sample; T }");
    accepts("machine pick(ready: bool) -> u32 { transition { ready -> 1u32, _ -> 0u32 } }");
}
