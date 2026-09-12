use super::typed_source;
use crate::lower_typed_trees;

#[test]
fn concrete_erased_proof_output_call_requires_a_complete_type_application() {
    let typed = typed_source(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        machine produce<Element>()
        requires incoming: ready()
        ensures copied: ready()
        { copied = incoming; }
        machine invoke()
        requires incoming: ready()
        ensures copied: ready()
        {
            let (; copied: local) = produce(; incoming);
            copied = local;
        }
    "#,
    )
    .expect("unresolved erased application remains available for checking");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("erased call cannot hide an unresolved type application");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("complete concrete application")),
        "{diagnostics:#?}"
    );
}

fn fixed_width_argument_source(argument: &str) -> String {
    format!(
        r#"
        trait Input<Value> {{ machine take(value: Value); }}
        machine invoke<Element, Choice: Element satisfies Input<u8>>() {{
            Choice::take({argument});
        }}
    "#
    )
}

#[test]
fn named_conformance_fixed_width_argument_accepts_fitting_literal() {
    let typed =
        typed_source(&fixed_width_argument_source("2")).expect("fixed-width evidence call types");
    lower_typed_trees(typed).expect("bound u8 input accepts a fitting literal");
}

#[test]
fn named_conformance_fixed_width_argument_rejects_overflowing_literal() {
    let typed = typed_source(&fixed_width_argument_source("300"))
        .expect("literal landing is checked after typing");
    lower_typed_trees(typed).expect_err("bound u8 input must reject 300 without a closed caller");
}

#[test]
fn named_conformance_fixed_width_argument_checks_computed_actuals() {
    let fitting = typed_source(&fixed_width_argument_source("1 + 1"))
        .expect("computed fixed-width input types");
    lower_typed_trees(fitting).expect("bound u8 input accepts a fitting computation");
    let overflowing = typed_source(&fixed_width_argument_source("200 + 100"))
        .expect("computed narrowing is checked after typing");
    lower_typed_trees(overflowing)
        .expect_err("bound u8 input must reject a computation yielding 300");
}

fn source(body: &str) -> String {
    format!(
        r#"
        trait Ranked {{
            machine Self::before(&self, other: &Self) -> bool;
        }}

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element,
            wrong: &bool
        ) -> bool {{
            {body}
        }}
        "#,
    )
}

fn rejects(body: &str, fragment: &str) {
    let typed = typed_source(&source(body)).expect("named evidence body should type");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("invalid retained generic evidence call must reject without a closed caller");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(fragment)),
        "expected {fragment:?}: {diagnostics:#?}",
    );
}

#[test]
fn retained_named_conformance_body_checks_without_a_closed_caller() {
    let typed = typed_source(&source("Order::before(left, right)"))
        .expect("named evidence body should type");
    let checked =
        lower_typed_trees(typed).expect("open evidence call should check its declared requirement");
    let template = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .expect("retained template");
    assert!(!checked.machine_type_parameters(template).is_empty());
    assert!(
        !checked
            .machine_specializations
            .iter()
            .any(|specialization| specialization.template == template.symbol)
    );
    let definition = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Ranked")
        .expect("public requirement owner");
    let requirement = &checked.trait_machine_signatures(definition)[0];
    let calls = checked
        .facts
        .borrow
        .calls
        .iter()
        .map(|(_, call)| call)
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one exact abstract call: {calls:#?}");
    };
    assert_eq!(call.target_symbol, requirement.symbol);
    assert_eq!(call.call_ordinal, 0);
    assert!(
        !call.has_receiver,
        "evidence is a namespace, not an implicit runtime self"
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .argument_accesses
            .span_or_empty(call.accesses)
            .len(),
        2
    );
}

#[test]
fn retained_named_conformance_call_rejects_wrong_explicit_self_type() {
    rejects(
        "Order::before(wrong, right)",
        "does not match its bound subject",
    );
}

#[test]
fn retained_named_conformance_call_requires_explicit_self_argument() {
    rejects("Order::before(right)", "argument");
}

#[test]
fn retained_named_conformance_call_rejects_missing_requirement() {
    rejects(
        "Order::missing(left, right)",
        "conformance-evidence call has no unique exact requirement",
    );
}

#[test]
fn retained_named_conformance_evidence_is_not_a_runtime_value() {
    rejects("Order", "not a declared local, parameter, field, or type");
}
