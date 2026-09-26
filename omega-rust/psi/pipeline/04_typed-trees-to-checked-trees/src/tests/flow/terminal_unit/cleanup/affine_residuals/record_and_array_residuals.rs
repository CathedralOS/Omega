use crate::tests::flow::terminal_unit::cleanup::assert_token_cleanup_partition;
use crate::tests::flow::terminal_unit::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape, checked, machine_named,
};

#[test]
fn retains_mixed_prefix_disjoint_field_transfers_with_maximal_residual_cleanup() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Deep { low: Token; middle: Token; high: Token; }
        data Branch { head: Token; deep: Deep; tail: Token; }
        data Outer { first: Token; left: Branch; right: Branch; last: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Outer) {
            Sink::take(value.left.deep.middle);
            Sink::take(value.right.tail);
            Sink::take(value.first);
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("mixed disjoint field moves have an exact maximal residual plan");
    assert_eq!(
        plan.machine.operations[..3]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => structural_arguments[0].path.clone(),
                _ => panic!("partial cleanup begins with source-ordered Unit calls"),
            })
            .collect::<Vec<_>>(),
        vec![
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
                CheckedUnitStructuralPathSegment::Field("middle".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("right".to_owned()),
                CheckedUnitStructuralPathSegment::Field("tail".to_owned()),
            ],
            vec![CheckedUnitStructuralPathSegment::Field("first".to_owned())],
        ]
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| discard.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![CheckedUnitStructuralPathSegment::Field("last".to_owned())],
            vec![
                CheckedUnitStructuralPathSegment::Field("right".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("right".to_owned()),
                CheckedUnitStructuralPathSegment::Field("head".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("tail".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
                CheckedUnitStructuralPathSegment::Field("high".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
                CheckedUnitStructuralPathSegment::Field("low".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("head".to_owned()),
            ],
        ]
    );
}

#[test]
fn bounded_integer_fields_preserve_partial_cleanup_without_scalar_residuals() {
    let checked = checked(
        r#"
        data Token { value: i16 [-10..=20]; }
        data Pair { before: u8 [0..=10]; tokens: [Token; 2]; after: i32 [-5..=5]; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::partial(value: Pair) {
            Sink::take(value.tokens[0]);
        }
        machine Root::complete(value: Pair) {
            Sink::take(value.tokens[0]);
            Sink::take(value.tokens[1]);
        }
        "#,
    );
    let path = |index| {
        vec![
            CheckedUnitStructuralPathSegment::Field("tokens".to_owned()),
            CheckedUnitStructuralPathSegment::FixedIndex(index),
        ]
    };
    assert_token_cleanup_partition(
        &checked,
        "partial",
        &[path(0)],
        &[(path(1), "named(name(Token))".to_owned())],
    );
    assert_token_cleanup_partition(&checked, "complete", &[path(0), path(1)], &[]);
    let bounded_fields = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .structural_types
        .iter()
        .filter_map(|declaration| match &declaration.shape {
            CheckedUnitStructuralTypeShape::Record { fields } => Some(fields),
            _ => None,
        })
        .flatten()
        .filter(|field| {
            matches!(
                field.field_type,
                CheckedUnitStructuralFieldType::BoundedInteger(_)
            )
        })
        .count();
    assert_eq!(
        bounded_fields, 3,
        "cleanup must retain every numeric restriction"
    );
}

#[test]
fn partial_cleanup_keeps_borrowed_byte_views_fenced() {
    let checked = checked(
        r#"
        domain [u8]::Utf8
        requires
            valid_utf8(self);
        data Token { value: u64; }
        data Mixed { view: &[u8] in Utf8; left: Token; right: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Mixed) {
            Sink::take(value.right);
        }
        "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "a borrowed byte view needs explicit loan retirement and cannot enter no-code cleanup"
    );
}
