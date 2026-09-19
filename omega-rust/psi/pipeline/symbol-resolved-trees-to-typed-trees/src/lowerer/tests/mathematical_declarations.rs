//! Mathematical `let`/`boundary let` declarations type into their typed-tree
//! mirror (PROOF-CONTRACT-MIGRATION). These tests cover shape retention only:
//! interpreting the grammar — dependent telescopes, universe classification,
//! kernel elaboration — is the checked-trees leg, which refuses until then.

use crate::lowerer::tests::lower_source;
use typed_trees::mathematical::{MathematicalBody, MathematicalType};

#[test]
fn let_definition_types_into_typed_tree() {
    let typed = lower_source("let double(x: u64): u64 = x;").expect("types");

    let definitions = typed.mathematical_definitions();
    assert_eq!(definitions.len(), 1);
    let definition = &definitions[0];
    assert!(definition.symbol.is_valid());
    assert_eq!(definition.name.to_string(), "double");
    assert!(!definition.is_public);

    let parameters = typed.mathematical_parameters(definition.parameters);
    assert_eq!(parameters.len(), 1);
    assert!(parameters[0].symbol.is_valid());
    assert_eq!(parameters[0].name.to_string(), "x");
    assert_eq!(
        parameters[0].relevance,
        language_core::BindingRelevance::Relevant
    );
    assert!(matches!(
        typed.mathematical_type(parameters[0].ty),
        MathematicalType::Ordinary(_)
    ));
    assert!(matches!(
        typed.mathematical_type(definition.result),
        MathematicalType::Ordinary(_)
    ));
    let MathematicalBody::Definition(term) = definition.body else {
        panic!("a `let` body types as a transparent definition term");
    };
    assert!(term.is_valid());
}

#[test]
fn boundary_let_types_as_named_assumption() {
    let typed = lower_source("boundary let choose(inhabited: u64): u64;").expect("types");

    let definitions = typed.mathematical_definitions();
    assert_eq!(definitions.len(), 1);
    let definition = &definitions[0];
    assert_eq!(definition.name.to_string(), "choose");
    assert_eq!(
        typed.mathematical_parameters(definition.parameters).len(),
        1
    );
    assert_eq!(definition.body, MathematicalBody::Assumption);
}

#[test]
fn parameterized_let_types_binders_and_visibility() {
    let typed = lower_source("pub let id<A: core::Type>(x: A): A = x;").expect("types");

    let definition = &typed.mathematical_definitions()[0];
    assert!(definition.is_public);
    let binders = typed.data_type_parameters.span_or_empty(definition.binders);
    assert_eq!(binders.len(), 1);
    assert_eq!(binders[0].name.to_string(), "A");
    assert!(binders[0].symbol.is_valid());
}

#[test]
fn dependent_and_application_result_types_preserve_their_shape() {
    let typed = lower_source("let fam(u: core::Level): A -> B -> C = term;").expect("types");
    let definition = &typed.mathematical_definitions()[0];
    let MathematicalType::Arrow {
        binder,
        domain,
        codomain,
    } = typed.mathematical_type(definition.result)
    else {
        panic!("an arrow result stays an arrow in typed trees");
    };
    assert!(binder.is_none());
    assert!(matches!(
        typed.mathematical_type(*domain),
        MathematicalType::Ordinary(_)
    ));
    assert!(matches!(
        typed.mathematical_type(*codomain),
        MathematicalType::Arrow { .. }
    ));

    let typed = lower_source("let dependent(u: core::Level): (value: A) -> F(value) = term;")
        .expect("types");
    let definition = &typed.mathematical_definitions()[0];
    let MathematicalType::Arrow {
        binder, codomain, ..
    } = typed.mathematical_type(definition.result)
    else {
        panic!("a dependent arrow result stays an arrow in typed trees");
    };
    assert_eq!(
        binder.as_ref().map(|name| name.to_string()).as_deref(),
        Some("value")
    );
    let MathematicalType::Application { callee, arguments } = typed.mathematical_type(*codomain)
    else {
        panic!("a type-level application stays an application in typed trees");
    };
    assert!(matches!(
        typed.mathematical_type(*callee),
        MathematicalType::Ordinary(_)
    ));
    assert_eq!(
        typed.expression_table.expression_handles(*arguments).len(),
        1
    );
}

#[test]
fn erased_parameter_relevance_carries_through() {
    let typed = lower_source("let keep(proof [erased]: P): Q = term;").expect("types");

    let parameters = typed.mathematical_parameters(typed.mathematical_definitions()[0].parameters);
    assert_eq!(
        parameters[0].relevance,
        language_core::BindingRelevance::Erased
    );
}

#[test]
fn mathematical_definitions_keep_authored_order_alongside_ordinary_roots() {
    let typed =
        lower_source("let first(): u64 = 0; machine main() -> u64 { 0 } let second(): u64 = 1;")
            .expect("types");

    let names: Vec<String> = typed
        .mathematical_definitions()
        .iter()
        .map(|definition| definition.name.to_string())
        .collect();
    assert_eq!(names, ["first", "second"]);
    assert_eq!(typed.machines().len(), 1);
}
