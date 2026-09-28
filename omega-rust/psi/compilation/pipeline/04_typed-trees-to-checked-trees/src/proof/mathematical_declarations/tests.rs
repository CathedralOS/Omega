use super::build_checked_mathematical_declarations;
use crate::tests::front_end::typed_program;
use checked_trees::{CheckedMathematicalBinderKind, CheckedMathematicalBody};

fn elaborate(source: &str) -> Vec<checked_trees::CheckedMathematicalDeclaration> {
    build_checked_mathematical_declarations(&typed_program(source)).expect("elaborate")
}

#[test]
fn plain_definition_elaborates_to_a_transparent_term() {
    let declarations = elaborate("let double(x: u64): u64 = x;");

    assert_eq!(declarations.len(), 1);
    let declaration = &declarations[0];
    assert_eq!(declaration.name, "double");
    assert!(!declaration.is_public);
    assert!(declaration.binders.is_empty());
    assert_eq!(declaration.parameters.len(), 1);
    assert_eq!(declaration.parameters[0].name, "x");
    assert_eq!(declaration.parameters[0].type_identity, "u64");
    assert_eq!(declaration.result, "u64");
    assert_eq!(
        declaration.body,
        CheckedMathematicalBody::Definition {
            term_identity: "x".to_owned()
        }
    );
    assert!(!declaration.is_assumption());
}

#[test]
fn core_named_carriers_classify_universe_and_type_binders() {
    let declarations = elaborate(
        "pub let compose<u: core::Level, v: core::Level, A: core::Type<u>, B: core::Type<v>, const N: usize, count: usize>(f: B, g: A): A = g;",
    );

    let declaration = &declarations[0];
    assert!(declaration.is_public);
    assert_eq!(
        declaration
            .binders
            .iter()
            .map(|binder| (binder.name.as_str(), binder.kind.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("u", CheckedMathematicalBinderKind::Level),
            ("v", CheckedMathematicalBinderKind::Level),
            (
                "A",
                CheckedMathematicalBinderKind::Type {
                    universe: Some("u".to_owned())
                }
            ),
            (
                "B",
                CheckedMathematicalBinderKind::Type {
                    universe: Some("v".to_owned())
                }
            ),
            (
                "N",
                CheckedMathematicalBinderKind::Const {
                    type_identity: "usize".to_owned()
                }
            ),
            (
                "count",
                CheckedMathematicalBinderKind::Value {
                    type_identity: "usize".to_owned()
                }
            ),
        ]
    );
    assert_eq!(declaration.level_arity(), 2);
}

#[test]
fn bare_and_unapplied_type_binders_record_no_universe() {
    let declarations = elaborate("let id<T, A: core::Type>(x: A): A = x;");

    assert_eq!(
        declarations[0]
            .binders
            .iter()
            .map(|binder| binder.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            CheckedMathematicalBinderKind::Type { universe: None },
            CheckedMathematicalBinderKind::Type { universe: None },
        ]
    );
}

#[test]
fn arrows_applications_and_bodies_render_their_identities() {
    let declarations = elaborate(
        "let dep<u: core::Level, A: core::Type<u>>(f: (value: A) -> A, x: A): F(x) = f(x);",
    );

    let declaration = &declarations[0];
    assert_eq!(declaration.parameters[0].type_identity, "(value: A) -> A");
    assert_eq!(declaration.parameters[1].type_identity, "A");
    assert_eq!(declaration.result, "F(x)");
    assert_eq!(
        declaration.body,
        CheckedMathematicalBody::Definition {
            term_identity: "f(x)".to_owned()
        }
    );
}

#[test]
fn boundary_let_elaborates_to_a_named_assumption() {
    let declarations = elaborate(
        "boundary let choose<u: core::Level, A: core::Type<u>>(inhabited: core::Squash<A>): A;",
    );

    let declaration = &declarations[0];
    assert!(declaration.is_assumption());
    assert_eq!(declaration.body, CheckedMathematicalBody::Assumption);
    assert_eq!(declaration.parameters[0].type_identity, "core::Squash<A>");
    assert_eq!(declaration.result, "A");
    assert_eq!(declaration.level_arity(), 1);
}

#[test]
fn level_carrier_with_arguments_refuses() {
    let diagnostics = build_checked_mathematical_declarations(&typed_program(
        "let bad<u: core::Level<x>>(x: u64): u64 = x;",
    ))
    .expect_err("core::Level takes no arguments");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("core::Level")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn bounded_binder_refuses() {
    let diagnostics = build_checked_mathematical_declarations(&typed_program(
        "let bad<T [copy]>(x: u64): u64 = x;",
    ))
    .expect_err("mathematical binders cannot carry bounds");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("property bounds")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}
