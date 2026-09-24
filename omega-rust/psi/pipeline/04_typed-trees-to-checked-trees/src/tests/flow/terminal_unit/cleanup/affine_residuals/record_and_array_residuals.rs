use crate::tests::flow::terminal_unit::cleanup::{
    assert_token_cleanup_partition, fixed_cleanup_path,
};
use crate::tests::flow::terminal_unit::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape, PrimitiveType, checked,
    machine_named,
};

#[test]
fn retains_source_ordered_direct_field_transfers_with_exact_residual_affine_cleanup() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Quartet { first: Token; second: Token; third: Token; fourth: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Quartet) {
            Sink::take(value.third);
            Sink::take(value.first);
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_none(),
        "path-sensitive cleanup must not leak through the root-only terminal lane"
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine)
        .expect("direct-field transfers with exact affine sibling cleanup");
    let moved_paths = plan.machine.operations[..2]
        .iter()
        .map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                claim_transfers,
                ..
            } if structural_arguments.len() == 1 && claim_transfers.is_empty() => {
                assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
                structural_arguments[0].path.clone()
            }
            _ => panic!("partial cleanup requires source-ordered direct Unit calls"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        moved_paths,
        vec![
            vec![CheckedUnitStructuralPathSegment::Field("third".to_owned())],
            vec![CheckedUnitStructuralPathSegment::Field("first".to_owned())],
        ]
    );
    assert!(matches!(
        plan.machine.operations.last(),
        Some(CheckedUnitEffectOperationPlan::Complete {
            statement_index: 2,
            trivial_affine_discards,
            ..
        }) if trivial_affine_discards.is_empty()
    ));
    assert_eq!(plan.residual_affine_discards.len(), 2);
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| {
                assert_eq!(
                    discard.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 0
                    }
                );
                assert!(discard.type_identity.contains("Token"));
                discard.path.clone()
            })
            .collect::<Vec<_>>(),
        vec![
            vec![CheckedUnitStructuralPathSegment::Field("fourth".to_owned())],
            vec![CheckedUnitStructuralPathSegment::Field("second".to_owned())],
        ]
    );
}

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
fn partial_cleanup_accepts_fully_consumed_records() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data One { right: Token; }
        data Inner { right: Token; }
        data Outer { left: Token; inner: Inner; }
        data Pair { left: Token; right: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::missing(value: One) {
            Sink::take(value.right);
        }
        machine Root::complete(value: Pair) {
            Sink::take(value.right);
            Sink::take(value.left);
        }
        "#,
    );

    let field = |name: &str| vec![CheckedUnitStructuralPathSegment::Field(name.to_owned())];
    assert_token_cleanup_partition(&checked, "missing", &[field("right")], &[]);
    assert_token_cleanup_partition(&checked, "complete", &[field("right"), field("left")], &[]);
}

#[test]
fn mixed_scalar_and_affine_record_retains_only_structural_residual_cleanup() {
    let checked = checked(
        r#"
        domain [u8; 3]::Utf8
        requires
            valid_utf8(self);
        domain [u8; 8]::Utf8
        requires
            valid_utf8(self);
        data Token { value: u64; }
        data Mixed {
            before: u8;
            before_bytes: [u8; 3] in Utf8;
            before_float: f32;
            left: Token;
            between: bool;
            between_bytes: [u8; 8] in Utf8;
            between_float: f64;
            right: Token;
            after: u64;
        }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Mixed) {
            Sink::take(value.right);
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("scalar fields participate in shape identity without acquiring cleanup");
    let root_identity = &plan.machine.structural_parameters[0].type_identity;
    let root = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .structural_types
        .iter()
        .find(|shape| &shape.identity == root_identity)
        .expect("mixed root shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &root.shape else {
        panic!("mixed root remains a record")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| {
                (
                    field.identity.as_str(),
                    matches!(field.field_type, CheckedUnitStructuralFieldType::Scalar(_)),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("before", true),
            ("before_bytes", false),
            ("before_float", true),
            ("left", false),
            ("between", true),
            ("between_bytes", false),
            ("between_float", true),
            ("right", false),
            ("after", true),
        ]
    );
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| match field.field_type {
                CheckedUnitStructuralFieldType::Scalar(PrimitiveType::F32) => {
                    Some((field.identity.as_str(), PrimitiveType::F32))
                }
                CheckedUnitStructuralFieldType::Scalar(PrimitiveType::F64) => {
                    Some((field.identity.as_str(), PrimitiveType::F64))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![
            ("before_float", PrimitiveType::F32),
            ("between_float", PrimitiveType::F64),
        ],
        "both exact IEEE source formats remain ordered checked shape identity"
    );
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| match field.field_type {
                CheckedUnitStructuralFieldType::ByteSequence(carrier) => {
                    Some((field.identity.as_str(), carrier))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "before_bytes",
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: 3 },
            ),
            (
                "between_bytes",
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: 8 },
            ),
        ],
        "bounded byte carriers retain exact source capacities and declaration order"
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| discard.path.clone())
            .collect::<Vec<_>>(),
        vec![vec![CheckedUnitStructuralPathSegment::Field(
            "left".to_owned()
        )]],
        "scalar, float, and bounded-byte fields are cleanup-free even before, between, and after affine fields"
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

#[test]
fn two_element_affine_array_moves_one_literal_index_and_discards_its_sibling() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::first(values: [Token; 2]) {
            Sink::take(values[0]);
        }
        machine Root::second(values: [Token; 2]) {
            Sink::take(values[1]);
        }
        "#,
    );
    for (machine, moved, residual) in [("first", 0, 1), ("second", 1, 0)] {
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine_named(&checked, machine))
            .expect("one literal array move leaves one exact affine sibling");
        assert_eq!(plan.machine.operations.len(), 2);
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } = &plan.machine.operations[0]
        else {
            panic!("array cleanup starts with one ordinary Unit call")
        };
        assert!(claim_transfers.is_empty());
        assert_eq!(
            structural_arguments[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(moved)]
        );
        assert_eq!(
            plan.residual_affine_discards[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(residual)]
        );
        assert_eq!(plan.residual_affine_discards.len(), 1);
        assert_eq!(
            plan.residual_affine_discards[0].type_identity,
            structural_arguments[0].type_identity
        );
    }
}

#[test]
fn two_element_affine_array_may_move_both_elements_without_residual_cleanup() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::forward(values: [Token; 2]) {
            Sink::take(values[0]);
            Sink::take(values[1]);
        }
        machine Root::reverse(values: [Token; 2]) {
            Sink::take(values[1]);
            Sink::take(values[0]);
        }
        "#,
    );
    for (machine, expected_paths) in [("forward", [0, 1]), ("reverse", [1, 0])] {
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine_named(&checked, machine))
            .expect("both exact array elements should transfer in authored order");
        assert!(plan.residual_affine_discards.is_empty());
        assert_eq!(plan.machine.operations.len(), 3);
        assert_eq!(
            plan.machine.operations[..2]
                .iter()
                .map(|operation| {
                    let CheckedUnitEffectOperationPlan::CallUnit {
                        structural_arguments,
                        claim_transfers,
                        ..
                    } = operation
                    else {
                        panic!("full array consumption contains only Unit calls before return")
                    };
                    assert!(claim_transfers.is_empty());
                    let [argument] = structural_arguments.as_slice() else {
                        panic!("each array move supplies one argument")
                    };
                    let [CheckedUnitStructuralPathSegment::FixedIndex(index)] =
                        argument.path.as_slice()
                    else {
                        panic!("each move retains one literal array index")
                    };
                    *index
                })
                .collect::<Vec<_>>(),
            expected_paths
        );
        assert!(matches!(
            plan.machine.operations[2],
            CheckedUnitEffectOperationPlan::Complete {
                ref trivial_affine_local_discard_ordinals,
                ref trivial_affine_discards,
                ..
            } if trivial_affine_local_discard_ordinals.is_empty()
                && trivial_affine_discards.is_empty()
        ));
    }
}

#[test]
fn three_element_affine_array_moves_two_indices_and_discards_the_sole_residual() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::middle(values: [Token; 3]) {
            Sink::take(values[2]);
            Sink::take(values[0]);
        }
        machine Root::last(values: [Token; 3]) {
            Sink::take(values[0]);
            Sink::take(values[1]);
        }
        machine Root::first(values: [Token; 3]) {
            Sink::take(values[1]);
            Sink::take(values[2]);
        }
        machine Root::one_move(values: [Token; 3]) {
            Sink::take(values[0]);
        }
        machine Root::all(values: [Token; 3]) {
            Sink::take(values[0]);
            Sink::take(values[1]);
            Sink::take(values[2]);
        }
        "#,
    );
    for (machine, expected_paths, residual) in [
        ("middle", [2, 0], 1),
        ("last", [0, 1], 2),
        ("first", [1, 2], 0),
    ] {
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine_named(&checked, machine))
            .expect("two distinct moves from an affine triple leave one exact residual");
        assert_eq!(plan.machine.operations.len(), 3);
        assert_eq!(
            plan.machine.operations[..2]
                .iter()
                .map(|operation| {
                    let CheckedUnitEffectOperationPlan::CallUnit {
                        structural_arguments,
                        claim_transfers,
                        ..
                    } = operation
                    else {
                        panic!("triple cleanup contains Unit calls before return")
                    };
                    assert!(claim_transfers.is_empty());
                    let [CheckedUnitStructuralPathSegment::FixedIndex(index)] =
                        structural_arguments[0].path.as_slice()
                    else {
                        panic!("triple move retains one literal index")
                    };
                    *index
                })
                .collect::<Vec<_>>(),
            expected_paths
        );
        assert_eq!(plan.residual_affine_discards.len(), 1);
        assert_eq!(
            plan.residual_affine_discards[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(residual)]
        );
    }
    let one_move = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "one_move"))
        .expect("one triple move leaves two statically ordered residuals");
    assert_eq!(one_move.machine.operations.len(), 2);
    assert_eq!(
        one_move
            .residual_affine_discards
            .iter()
            .map(|discard| match discard.path.as_slice() {
                [CheckedUnitStructuralPathSegment::FixedIndex(index)] => *index,
                _ => panic!("array residual is one literal index"),
            })
            .collect::<Vec<_>>(),
        vec![2, 1],
        "live array siblings clean in decreasing index order",
    );
    assert_token_cleanup_partition(
        &checked,
        "all",
        &[
            fixed_cleanup_path(&[0]),
            fixed_cleanup_path(&[1]),
            fixed_cleanup_path(&[2]),
        ],
        &[],
    );
}

#[test]
fn affine_array_partial_cleanup_accepts_one_and_five_elements() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::one(values: [Token; 1]) {
            Sink::take(values[0]);
        }
        machine Root::five(values: [Token; 5]) {
            Sink::take(values[0]);
            Sink::take(values[1]);
        }
        "#,
    );
    assert_token_cleanup_partition(&checked, "one", &[fixed_cleanup_path(&[0])], &[]);
    assert_token_cleanup_partition(
        &checked,
        "five",
        &[fixed_cleanup_path(&[0]), fixed_cleanup_path(&[1])],
        &[
            (fixed_cleanup_path(&[4]), "named(name(Token))".to_owned()),
            (fixed_cleanup_path(&[3]), "named(name(Token))".to_owned()),
            (fixed_cleanup_path(&[2]), "named(name(Token))".to_owned()),
        ],
    );
}

#[test]
fn four_element_affine_array_moves_two_indices_and_discards_the_complement_decreasing() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::outer(values: [Token; 4]) {
            Sink::take(values[1]);
            Sink::take(values[3]);
        }
        machine Root::inner(values: [Token; 4]) {
            Sink::take(values[2]);
            Sink::take(values[1]);
        }
        machine Root::one(values: [Token; 4]) {
            Sink::take(values[0]);
        }
        machine Root::three(values: [Token; 4]) {
            Sink::take(values[0]);
            Sink::take(values[1]);
            Sink::take(values[2]);
        }
        "#,
    );
    for (machine, moves, residuals) in [("outer", [1, 3], [2, 0]), ("inner", [2, 1], [3, 0])] {
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine_named(&checked, machine))
            .expect("two quartet moves leave the exact decreasing complement");
        assert_eq!(
            plan.machine.operations[..2]
                .iter()
                .map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::CallUnit {
                        structural_arguments,
                        ..
                    } => match structural_arguments[0].path.as_slice() {
                        [CheckedUnitStructuralPathSegment::FixedIndex(index)] => *index,
                        _ => panic!("quartet move is one literal index"),
                    },
                    _ => panic!("quartet cleanup contains calls before return"),
                })
                .collect::<Vec<_>>(),
            moves,
            "authored move order is retained",
        );
        assert_eq!(
            plan.residual_affine_discards
                .iter()
                .map(|discard| match discard.path.as_slice() {
                    [CheckedUnitStructuralPathSegment::FixedIndex(index)] => *index,
                    _ => panic!("quartet residual is one literal index"),
                })
                .collect::<Vec<_>>(),
            residuals,
            "the compiler emits the live complement in decreasing index order",
        );
    }
    assert_token_cleanup_partition(
        &checked,
        "one",
        &[fixed_cleanup_path(&[0])],
        &[
            (fixed_cleanup_path(&[3]), "named(name(Token))".to_owned()),
            (fixed_cleanup_path(&[2]), "named(name(Token))".to_owned()),
            (fixed_cleanup_path(&[1]), "named(name(Token))".to_owned()),
        ],
    );
    assert_token_cleanup_partition(
        &checked,
        "three",
        &[
            fixed_cleanup_path(&[0]),
            fixed_cleanup_path(&[1]),
            fixed_cleanup_path(&[2]),
        ],
        &[(fixed_cleanup_path(&[3]), "named(name(Token))".to_owned())],
    );
}
