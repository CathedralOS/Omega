//! Index evidence belongs to the actual reached extent, not just its selector.
//! The same storage operations handle array construction, copies and writes.

use super::*;
use structure_elaboration::StorageShape;
use terminal_psi::ArrayExtent;

fn array_types() -> IdTable<StructuralTypeId, RecordType> {
    let mut types = IdTable::default();
    types.push(RecordType {
        copyable: true,
        identity: "u32-storage".into(),
        shape: StorageShape::PrimitiveScalar {
            scalar_type: u32_type(),
            policy: None,
        },
        requires: Vec::new(),
    });
    for (element, length, identity) in [(1, 4, "four"), (1, 2, "two"), (2, 2, "two-by-four")] {
        types.push(RecordType {
            copyable: true,
            identity: identity.into(),
            shape: StorageShape::FixedArray {
                element: id(element),
                length: ArrayExtent::Literal(length),
            },
            requires: Vec::new(),
        });
    }
    types
}

fn array(raw: u64) -> ValueType {
    ValueType::record(id(raw))
}

fn index(base: Expression, selector: Expression, result: ValueType) -> Expression {
    of_type(
        ExpressionKind::Index {
            base: Box::new(base),
            index: Box::new(selector),
        },
        result,
    )
}

fn caller(
    parameters: Vec<ValueType>,
    requires: Vec<Proposition>,
    statements: Vec<Statement>,
) -> Shaped {
    Shaped {
        parameters,
        locals: Vec::new(),
        result: u32_type().into(),
        requires,
        statements,
    }
}

#[test]
fn equal_selectors_do_not_share_bounds_between_different_extents() {
    let program = caller(
        vec![array(2), array(3), u32_type().into()],
        vec![at_most(1, 3)],
        vec![returns(strict(
            Strict::BitwiseOr,
            index(place(0, array(2)), slot(2, u32_type()), u32_type().into()),
            index(place(1, array(3)), slot(2, u32_type()), u32_type().into()),
        ))],
    );
    let mut elaboration = elaborate_over(vec![program], &array_types());
    let result = discharge(&mut elaboration).unwrap();
    let actual = verdicts(&elaboration, &result, 1, safety);
    assert_eq!(actual.len(), 2);
    assert_kernel_checked(&elaboration, 1, actual[0].0, certificate(actual[0].1));
    assert_unproved(actual[1].1);
}

#[test]
fn nested_index_bounds_follow_each_actual_array_prefix() {
    for (outer, inner, accepted) in [(1, 3, true), (3, 1, false)] {
        let nested = index(place(0, array(4)), slot(1, u32_type()), array(2));
        let program = caller(
            vec![array(4), u32_type().into(), u32_type().into()],
            vec![at_most(1, outer), at_most(2, inner)],
            vec![returns(index(
                nested,
                slot(2, u32_type()),
                u32_type().into(),
            ))],
        );
        let verdict = judge(Ok(elaborate_over(vec![program], &array_types()))).unwrap();
        assert_eq!(
            matches!(verdict, RouteVerdict::Accepted(_)),
            accepted,
            "{verdict:?}"
        );
    }
}

#[test]
fn signed_selectors_need_a_nonnegative_bound_as_well_as_the_extent() {
    for (lower, accepted) in [(0, true), (-1, false)] {
        let signed = i32_type();
        let requirements = vec![
            Proposition::LessOrEqual(
                bound(signed, IntegerValue::Signed(lower)),
                formal(1, signed),
            ),
            Proposition::LessOrEqual(formal(1, signed), bound(signed, IntegerValue::Signed(3))),
        ];
        let program = caller(
            vec![array(2), signed.into()],
            requirements,
            vec![returns(index(
                place(0, array(2)),
                slot(1, signed),
                u32_type().into(),
            ))],
        );
        let verdict = judge(Ok(elaborate_over(vec![program], &array_types()))).unwrap();
        assert_eq!(
            matches!(verdict, RouteVerdict::Accepted(_)),
            accepted,
            "{verdict:?}"
        );
    }
}

#[test]
fn array_construction_copy_and_indexed_replacement_preserve_other_elements() {
    let literal = of_type(
        ExpressionKind::Array {
            elements: vec![unsigned(1), unsigned(2), unsigned(3), unsigned(4)],
        },
        array(2),
    );
    let mut program = caller(
        Vec::new(),
        Vec::new(),
        vec![
            bind(0, literal),
            bind(1, place(0, array(2))),
            Statement::Store {
                destination: index(place(0, array(2)), unsigned(0), u32_type().into()),
                value: unsigned(9),
                source: nowhere(),
            },
            returns(call(
                2,
                vec![index(place(1, array(2)), unsigned(0), u32_type().into())],
            )),
        ],
    );
    program.locals = vec![array(2), array(2)];
    let requirement = caller(
        vec![u32_type().into()],
        vec![at_most(1, 1)],
        vec![returns(slot(0, u32_type()))],
    );
    assert!(matches!(
        judge(Ok(elaborate_over(
            vec![program, requirement],
            &array_types()
        ))),
        Ok(RouteVerdict::Accepted(_))
    ));
}

#[test]
fn an_indexed_match_reuses_its_captured_selector_in_the_arm() {
    let mut typed = typed_trees::TypedTrees::default();
    let marker = typed
        .expression_table
        .insert(typed_trees::expression::ExpressionNode::Boolean(true));
    let capture = typed.expression_table.insert_match_subject(marker);
    let foreign_capture = typed.expression_table.insert_match_subject(marker);
    let mut types = array_types();
    types.push(RecordType {
        copyable: true,
        identity: "Item".into(),
        shape: StorageShape::Sum(vec![
            CaseType {
                id: id(1),
                identity: "Present".into(),
                fields: vec![RecordField {
                    id: id(1),
                    identity: "value".into(),
                    value_type: u32_type().into(),
                }],
            },
            CaseType {
                id: id(2),
                identity: "Absent".into(),
                fields: Vec::new(),
            },
        ]),
        requires: Vec::new(),
    });
    types.push(RecordType {
        copyable: true,
        identity: "Items".into(),
        shape: StorageShape::FixedArray {
            element: id(5),
            length: ArrayExtent::Literal(2),
        },
        requires: Vec::new(),
    });
    let program = |saved| {
        caller(
            vec![array(6)],
            Vec::new(),
            vec![returns(of_type(
                ExpressionKind::Match {
                    capture: Some(capture),
                    subject: Box::new(index(
                        place(0, array(6)),
                        strict(Strict::Add, unsigned(0), unsigned(0)),
                        array(5),
                    )),
                    arms: vec![(
                        0,
                        of_type(
                            ExpressionKind::Payload {
                                base: Box::new(of_type(
                                    ExpressionKind::SavedSubject(saved),
                                    array(5),
                                )),
                                case: 0,
                                field: 0,
                            },
                            u32_type().into(),
                        ),
                    )],
                    default: Some(Box::new(unsigned(0))),
                },
                u32_type().into(),
            ))],
        )
    };
    let elaboration = elaborate_over(vec![program(capture)], &types);
    assert_eq!(
        elaboration.module.machines[0]
            .operations()
            .filter(|operation| matches!(operation.kind, OperationKind::ExactIntegerAdd { .. }))
            .count(),
        1,
        "the selector computes once before dispatch, never again in its arm"
    );
    assert!(matches!(
        judge(Ok(elaboration)),
        Ok(RouteVerdict::Accepted(_))
    ));
    // The closed-body entrance refuses a capture belonging to another match.
    let invalid = program(foreign_capture);
    let mut bodies = IdTable::default();
    bodies.push(Body {
        slots: invalid.parameters.clone(),
        statements: invalid.statements,
    });
    let mut signatures = IdTable::default();
    signatures.push(Signature {
        parameters: invalid.parameters,
        erased: Vec::new(),
        borrowed: Vec::new(),
        result: Some(invalid.result),
        requires: Vec::new(),
        ensures: Vec::new(),
        reaches: structure_elaboration::Ceiling::Inferred,
        invokes: structure_elaboration::Ceiling::Inferred,
        crashes: CrashCeiling::Authored(Vec::new()),
        termination: Termination::Silent,
    });
    assert!(
        matches!(structure_elaboration::lower_bodies(id(1), &bodies, &signatures, &types),
        Err(Refusal { detail, .. }) if detail.contains("outside its match capture"))
    );
}

#[test]
fn an_array_move_vacates_only_its_exact_element() {
    let mut types = IdTable::default();
    types.push(RecordType {
        copyable: false,
        identity: "OwnedElement".into(),
        shape: StorageShape::Record(vec![RecordField {
            id: id(1),
            identity: "value".into(),
            value_type: u32_type().into(),
        }]),
        requires: Vec::new(),
    });
    types.push(RecordType {
        copyable: false,
        identity: "OwnedElements".into(),
        shape: StorageShape::FixedArray {
            element: id(1),
            length: ArrayExtent::Literal(2),
        },
        requires: Vec::new(),
    });
    for (read_index, accepted) in [(1, true), (0, false)] {
        let mut program = caller(
            vec![array(2)],
            Vec::new(),
            vec![
                bind(1, index(place(0, array(2)), unsigned(0), array(1))),
                returns(read(
                    index(place(0, array(2)), unsigned(read_index), array(1)),
                    0,
                )),
            ],
        );
        program.locals.push(array(1));
        let verdict = judge(Ok(elaborate_over(vec![program], &types))).unwrap();
        if accepted {
            assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
        } else {
            let RouteVerdict::Rejected {
                formations,
                unproved,
                unestablished,
                conflicts,
                ..
            } = verdict
            else {
                panic!("the moved element must fail storage checking: {verdict:?}");
            };
            assert!(formations.is_empty() && unproved.is_empty() && conflicts.is_empty());
            assert!(
                unestablished.iter().any(|used| used.detail
                    == "owned storage is used after its value was transferred or consumed"),
                "{unestablished:?}"
            );
        }
    }
}
