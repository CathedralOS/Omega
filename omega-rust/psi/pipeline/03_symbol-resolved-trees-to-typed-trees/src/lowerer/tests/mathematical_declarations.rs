//! Mathematical `let`/`boundary let` declarations type into their typed-tree
//! mirror (PROOF-CONTRACT-MIGRATION). These tests cover shape retention only:
//! interpreting the grammar — dependent telescopes, universe classification,
//! kernel elaboration — is the checked-trees leg, which refuses until then.

use crate::lowerer::seeded_continuation::{SeededContinuationError, lower_seeded_extension};
use crate::lowerer::tests::seeded_plain_data_inputs;
use typed_trees::mathematical::{MathematicalBody, MathematicalType};

#[test]
fn dependent_and_application_result_types_preserve_their_shape() {
    let typed =
        crate::front_end::typed_program_result("let fam(u: core::Level): A -> B -> C = term;")
            .expect("types");
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

    let typed = crate::front_end::typed_program_result(
        "let dependent(u: core::Level): (value: A) -> F(value) = term;",
    )
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
fn arrow_and_application_parameter_types_preserve_their_shape() {
    let typed = crate::front_end::typed_program_result(
        "let compose(f: A -> B, g: B -> C, x: A): C = g(f(x));",
    )
    .expect("types");

    let definition = &typed.mathematical_definitions()[0];
    let parameters = typed.mathematical_parameters(definition.parameters);
    assert_eq!(parameters.len(), 3);
    assert!(matches!(
        typed.mathematical_type(parameters[0].ty),
        MathematicalType::Arrow { .. }
    ));
    assert!(matches!(
        typed.mathematical_type(parameters[1].ty),
        MathematicalType::Arrow { .. }
    ));
    assert!(matches!(
        typed.mathematical_type(parameters[2].ty),
        MathematicalType::Ordinary(_)
    ));

    let typed = crate::front_end::typed_program_result("let apply(F: C(x, y), x: u64): F = term;")
        .expect("types");
    let definition = &typed.mathematical_definitions()[0];
    let parameters = typed.mathematical_parameters(definition.parameters);
    let MathematicalType::Application { arguments, .. } = typed.mathematical_type(parameters[0].ty)
    else {
        panic!("a type-level application parameter stays an application")
    };
    assert_eq!(
        typed.expression_table.expression_handles(*arguments).len(),
        2
    );
}

#[test]
fn boundary_let_retains_binders_and_visibility() {
    let typed =
        crate::front_end::typed_program_result("pub boundary let choose<A: core::Type>(x: A): A;")
            .expect("types");

    let definition = &typed.mathematical_definitions()[0];
    assert!(definition.is_public);
    assert_eq!(definition.body, MathematicalBody::Assumption);
    let binders = typed.data_type_parameters.span_or_empty(definition.binders);
    assert_eq!(binders.len(), 1);
    assert_eq!(binders[0].name.to_string(), "A");
    let typed_trees::data::TypeParameterKind::Value { type_reference } = binders[0].kind else {
        panic!("`A: core::Type` keeps its authored value-binder carrier")
    };
    assert!(type_reference.is_valid());
}

#[test]
fn mathematical_definitions_keep_authored_order_alongside_ordinary_roots() {
    let typed = crate::front_end::typed_program_result(
        "let first(): u64 = 0; machine main() -> u64 { 0 } let second(): u64 = 1;",
    )
    .expect("types");

    let names: Vec<String> = typed
        .mathematical_definitions()
        .iter()
        .map(|definition| definition.name.to_string())
        .collect();
    assert_eq!(names, ["first", "second"]);
    assert_eq!(typed.machines().len(), 1);
}

#[test]
fn seeded_continuation_fences_extension_mathematical_declarations() {
    let (base, extension) = seeded_plain_data_inputs(
        "let base_fn(x: u64): u64 = x; data Authored { value: u16; }",
        "let extra(y: u64): u64 = base_fn(y); data Generated { value: u32; }",
    );
    let expected = base.typed().clone();
    let Err((returned, error)) = lower_seeded_extension(extension, base) else {
        panic!("an extension-authored `let` must reject transactionally")
    };
    assert_eq!(error, SeededContinuationError::UnsupportedExtensionShape);
    assert_eq!(returned.into_typed(), expected);
}
