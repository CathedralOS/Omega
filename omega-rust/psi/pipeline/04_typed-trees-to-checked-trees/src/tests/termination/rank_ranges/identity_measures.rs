use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/identity_measure_rank_range/main.omg"
));

#[test]
fn identity_measure_carrier_matching_does_not_widen_or_discharge_qualifications() {
    for carrier in ["u8", "u16", "u32", "u64"] {
        let source = COUNTDOWN.replace("u64", carrier);
        for other in ["u8", "u16", "u32", "u64", "i32", "bool"] {
            if carrier == other {
                continue;
            }
            reject(&source.replace(&format!("(value: {carrier})"), &format!("(value: {other})")));
            reject(&source.replace(
                &format!("-> {carrier} {{ value }}"),
                &format!("-> {other} {{ value }}"),
            ));
            reject(&source.replace(
                &format!("remaining: {carrier}"),
                &format!("remaining: {other}"),
            ));
        }
        reject(&source.replace(
            &format!("(value: {carrier})"),
            &format!("(value: {carrier} [1..=5])"),
        ));
        reject(&source.replace(
            &format!("-> {carrier} {{ value }}"),
            &format!("-> {carrier} [1..=5] {{ value }}"),
        ));
        reject(&source.replace("{ value }", "{ value + 1 }"));
    }
}

#[test]
fn scalar_measure_subject_uses_its_exact_state_parameter_not_its_spelling() {
    let source = COUNTDOWN.replace("u64", "u32");
    let program = typed_program(&source);
    let machine = &program.machines()[0];
    let subject = typed_trees::ranking::resolve_machine_witness_subjects(&program, machine)
        .expect("retained ranking subject")[0];
    let foreign = program.measures()[0]
        .parameter
        .as_ref()
        .expect("measure binder")
        .symbol;
    for replacement in [symbols::SymbolHandle::invalid(), foreign] {
        let mut changed = program.clone();
        let typed_trees::expression::ExpressionNode::Name(path) =
            changed.expression_table.expression_mut(subject)
        else {
            panic!("parameter subject")
        };
        path.symbol = replacement;
        path.head_symbol = replacement;
        crate::checks::termination::check_machine_termination(&changed)
            .expect_err("same spelling cannot replace the state parameter");
    }
}

#[test]
fn unsigned_identity_views_do_not_authorize_custom_arithmetic_meaning() {
    for carrier in ["u8", "u16", "u32", "u64"] {
        let source = COUNTDOWN.replace("u64", carrier);
        for (operator, result) in [("-", carrier), (">", "bool")] {
            let selected = format!(
                "operator {operator} {carrier}::custom(left: {carrier}, right: {carrier}) -> {result}; {source}"
            );
            reject(&selected);
            reject(&selected.replace(" -> Countdown::Remaining", ""));
        }
        prove(&format!(
            "operator + {carrier}::unused(left: {carrier}, right: {carrier}) -> {carrier}; {source}"
        ));
        reject(&format!(
            "operator == bool::custom(left: bool, right: bool) -> bool; {}",
            source.replace("remaining > 0", "(remaining > 0) == true"),
        ));
    }
    prove(&format!(
        "operator - u64::unrelated(left: u64, right: u64) -> u64; {}",
        COUNTDOWN.replace("u64", "u32"),
    ));
}

fn prove(source: &str) {
    let program = typed_program(source);
    assert!(typed_trees::visibility::requires_declaration_visibility(
        symbols::SymbolKind::Measure
    ));
    let visibility =
        typed_trees::visibility::declaration_visibility(&program, program.measures()[0].symbol)
            .expect("measure declaration visibility");
    assert!(!visibility.is_public());
    assert_eq!(visibility.kind(), "measure");
    assert!(matches!(
        crate::infer_machine_termination_summary(&program, program.machines()[0].symbol),
        Some(language_semantics::TerminationGuarantee::Terminates { .. })
    ));
    let checked = lower_typed_trees(program, &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    let witness = checked.machines()[0]
        .termination_plan
        .implementation_witness
        .as_ref()
        .expect("private authored witness");
    assert_eq!(witness.view_path, "Countdown::Remaining");
    assert_eq!(
        witness.ranking_view,
        language_semantics::RankingViewId::NULL
    );
}

fn reject(source: &str) {
    let program = typed_program(source);
    crate::checks::termination::check_machine_termination(&program)
        .expect_err("the authored measure still owes range and descent proofs");
    assert!(lower_typed_trees(program, &CheckingRequest::settled()).is_err());
}

#[test]
fn identity_measure_does_not_waive_membership_geometry_or_descent() {
    for range in ["1..=5", "0..=4", "0..5"] {
        reject(&COUNTDOWN.replace("in 0..=5", &format!("in {range}")));
    }
    for actual in ["remaining", "remaining - 2", "remaining + 1"] {
        reject(&COUNTDOWN.replace("walk(remaining - 1)", &format!("walk({actual})")));
    }
    reject(&COUNTDOWN.replace("false -> remaining", "false -> self"));
}

#[test]
fn constrained_measure_signature_is_discharged_against_the_subject() {
    let constrained = COUNTDOWN
        .replace("(value: u64)", "(value: u64 [0..=5])")
        .replace("-> u64 {", "-> u64 [0..=5] {");
    prove(&constrained);
    // A declared domain wider than the subject's bounds still covers it,
    // including an exclusive end normalized to the same `0..=5` extent.
    prove(&constrained.replace("value: u64 [0..=5])", "value: u64 [0..=6])"));
    prove(&constrained.replace("value: u64 [0..=5])", "value: u64 [0..6])"));
    // The corpus fixture carries the subject bound in a requires clause,
    // not a parameter refinement. Ensure removing it really changes the case.
    let subject_requirement = "requires 0 <= remaining && remaining <= 5";
    assert!(constrained.contains(subject_requirement));
    for source in [
        // The subject `0..=5` does not satisfy a `1..=5` domain or result.
        constrained.replace("value: u64 [0..=5])", "value: u64 [1..=5])"),
        constrained.replace("-> u64 [0..=5] {", "-> u64 [1..=5] {"),
        // A narrower declared domain does not cover the subject either.
        constrained.replace("value: u64 [0..=5])", "value: u64 [0..=4])"),
        // A non-range refinement cannot be discharged by bounds.
        constrained.replace("value: u64 [0..=5])", "value: u64 in Saturating)"),
        // An unrestricted subject cannot promise the declared domain.
        constrained.replace(subject_requirement, ""),
    ] {
        reject(&source);
    }
}
