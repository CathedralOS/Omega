use super::*;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult,
    RegisterTargetApplicability, TerminalMachine, Terminator, ValueDeclaration,
};

fn value(raw: u64, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(raw).unwrap(),
        scalar_type,
        qualifications: Default::default(),
    }
}

fn constant(raw: u64, scalar_type: ScalarType, kind: OperationKind) -> Operation {
    Operation::unsuspended(
        OperationId::new(raw).unwrap(),
        OperationResult::Scalar(value(raw, scalar_type)),
        kind,
    )
}

#[test]
fn whole_byte_windows_require_zero_and_their_own_immediate_measurement() {
    let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let root = PlaceId::new(1).unwrap();
    let source =
        terminal_psi::StructuralArgument::whole(root, StructuralAccess::SharedBorrow);
    let mut entry = Block::plain(
        BlockId::new(1).unwrap(),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    entry.operations = vec![
        constant(
            1,
            count,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        ),
        constant(
            2,
            count,
            OperationKind::ByteSequenceLength {
                source: source.clone(),
            },
        ),
        Operation::unsuspended(
            OperationId::new(3).unwrap(),
            OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: PlaceId::new(2).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::ByteSequenceSubslice {
                source,
                start: ValueId::new(1).unwrap(),
                end: ValueId::new(2).unwrap(),
                length: ValueId::new(2).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            },
        ),
    ];
    let machine = TerminalMachine {
        assembly_target_requirement: RegisterTargetApplicability::Any,
        id: MachineId::new(1).unwrap(),
        attachment: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: entry.id,
        blocks: vec![entry],
        contract: MachineContract::empty(ContractId::new(1).unwrap()),
    };
    let whole = |machine: &TerminalMachine| {
        byte_subslice_is_whole(machine, &machine.blocks[0].operations[2])
    };
    assert!(whole(&machine));

    let mut partial = machine.clone();
    partial.blocks[0].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(1),
    };
    assert!(!whole(&partial), "a UTF-8 codepoint may cross the cut");

    let mut foreign = machine.clone();
    let OperationKind::ByteSequenceLength { source } =
        &mut foreign.blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    source.place = PlaceId::new(3).unwrap();
    assert!(
        !whole(&foreign),
        "an unrelated equal-sized view is not this source"
    );

    let mut wrong_width = machine.clone();
    wrong_width.blocks[0].operations[0].result = OperationResult::Scalar(value(
        1,
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    ));
    assert!(!whole(&wrong_width));

    let mut duplicate = machine.clone();
    let mut extra = duplicate.blocks[0].operations[0].clone();
    extra.id = OperationId::new(4).unwrap();
    duplicate.blocks[0].operations.push(extra);
    assert!(!whole(&duplicate), "the zero must have one actual producer");

    let mut shortened = machine.clone();
    let OperationKind::ByteSequenceSubslice { end, .. } =
        &mut shortened.blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    *end = ValueId::new(1).unwrap();
    assert!(!whole(&shortened));
}

#[test]
fn boolean_literals_keep_their_type_and_exact_zero_or_one_value() {
    let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    for boolean in [false, true] {
        let operation = constant(
            1,
            ScalarType::Boolean,
            OperationKind::BooleanConstant { value: boolean },
        );
        assert_eq!(
            scalar_operation_literal(&operation, ValueId::new(1).unwrap()),
            Some((
                ScalarType::Boolean,
                IntegerValue::Unsigned(u128::from(u8::from(boolean)))
            ))
        );
        assert_eq!(
            scalar_operation_literal(&operation, ValueId::new(2).unwrap()),
            None
        );
        let wrong_type = constant(
            1,
            u8_type,
            OperationKind::BooleanConstant { value: boolean },
        );
        assert_eq!(
            scalar_operation_literal(&wrong_type, ValueId::new(1).unwrap()),
            None
        );
    }
    let integer = constant(
        1,
        u8_type,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(7),
        },
    );
    assert_eq!(
        scalar_operation_literal(&integer, ValueId::new(1).unwrap()),
        Some((u8_type, IntegerValue::Unsigned(7)))
    );
    let wrong_type = constant(
        1,
        ScalarType::Boolean,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(1),
        },
    );
    assert_eq!(
        scalar_operation_literal(&wrong_type, ValueId::new(1).unwrap()),
        None
    );
}

#[test]
fn the_existing_machine_index_resolves_boolean_forwarding_without_rebinding_parameters() {
    let first = BlockId::new(1).unwrap();
    let second = BlockId::new(2).unwrap();
    let formal = value(1, ScalarType::Boolean);
    let mut entry = Block::plain(
        first,
        vec![formal],
        Terminator::Jump {
            edge: EdgeId::new(1).unwrap(),
            target: second,
            arguments: vec![ValueId::new(2).unwrap()],
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        },
    );
    entry.operations.push(constant(
        2,
        ScalarType::Boolean,
        OperationKind::BooleanConstant { value: true },
    ));
    let join = Block::plain(
        second,
        vec![value(3, ScalarType::Boolean)],
        Terminator::ReturnUnit {
            edge: EdgeId::new(2).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    let machine = TerminalMachine {
        assembly_target_requirement: RegisterTargetApplicability::Any,
        id: MachineId::new(1).unwrap(),
        attachment: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: first,
        blocks: vec![entry, join],
        contract: MachineContract::empty(ContractId::new(1).unwrap()),
    };
    let index = MachineScalarIndex::new(&machine);
    let expected = Some((ScalarType::Boolean, IntegerValue::Unsigned(1)));
    assert_eq!(index.constant_value(ValueId::new(2).unwrap()), expected);
    assert_eq!(index.constant_value(ValueId::new(3).unwrap()), expected);
    assert_eq!(index.constant_value(ValueId::new(1).unwrap()), None);
    assert_eq!(index.constant_value(ValueId::new(99).unwrap()), None);
}

#[test]
fn computed_scalars_follow_typed_operations_and_memoize_shared_inputs() {
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let id = |raw| ValueId::new(raw).unwrap();
    let operations = [
        constant(
            1,
            integer,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        ),
        constant(
            2,
            integer,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1),
            },
        ),
        constant(
            3,
            integer,
            OperationKind::ExactIntegerAdd {
                left: id(1),
                right: id(2),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            },
        ),
        constant(
            4,
            integer,
            OperationKind::WrappingIntegerAdd {
                left: id(3),
                right: id(3),
            },
        ),
    ];
    let types = operations
        .iter()
        .map(|op| (op.result.scalar_ref().unwrap().id, integer))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut constants = ScalarConstants::for_values(
        operations
            .iter()
            .map(|operation| operation.result.scalar_ref().unwrap().id),
    );
    let mut visits = [0; 4];
    for _ in 0..2 {
        assert_eq!(
            constants.value(id(4), |value| {
                let index = usize::try_from(value.get() - 1).ok()?;
                let operation = operations.get(index)?;
                visits[index] += 1;
                scalar_operation_origin(operation, value, &types)
            }),
            Some((integer, IntegerValue::Unsigned(2)))
        );
    }
    assert_eq!(
        visits, [1; 4],
        "shared operands and repeated actuals resolve once"
    );
    assert!(scalar_operation_origin(&operations[2], id(99), &types).is_none());
    let mut wrong_types = types.clone();
    wrong_types.insert(id(2), ScalarType::Boolean);
    assert!(scalar_operation_origin(&operations[2], id(3), &wrong_types).is_none());
}

#[test]
fn scalar_constants_keep_arithmetic_failures_cycles_and_unknown_inputs_unknown() {
    use semantic_vocabulary::ScalarTerm;
    let ty = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let integer = ScalarType::Integer(ty);
    let id = |raw| ValueId::new(raw).unwrap();
    let mut constants = ScalarConstants::for_values((1..=8).map(id));
    let mut origin = |value: ValueId| {
        Some(match value.get() {
            1 => ScalarConstantOrigin::Literal(integer, IntegerValue::Unsigned(255)),
            2 => ScalarConstantOrigin::Literal(integer, IntegerValue::Unsigned(1)),
            3 => ScalarConstantOrigin::Computed(ScalarTerm::ExactIntegerAdd {
                scalar_type: ty,
                left: Box::new(ScalarTerm::value(id(1), integer)),
                right: Box::new(ScalarTerm::value(id(2), integer)),
            }),
            4 => ScalarConstantOrigin::Computed(ScalarTerm::WrappingIntegerAdd {
                scalar_type: ty,
                left: Box::new(ScalarTerm::value(id(1), integer)),
                right: Box::new(ScalarTerm::value(id(2), integer)),
            }),
            5 => ScalarConstantOrigin::Incoming(vec![id(4), id(4)]),
            6 => ScalarConstantOrigin::Incoming(vec![id(5), id(2)]),
            7 => ScalarConstantOrigin::Computed(ScalarTerm::value(id(7), integer)),
            8 => ScalarConstantOrigin::Literal(integer, IntegerValue::Unsigned(256)),
            _ => return None,
        })
    };
    assert_eq!(constants.value(id(3), &mut origin), None);
    assert_eq!(
        constants.value(id(4), &mut origin),
        Some((integer, IntegerValue::Unsigned(0)))
    );
    assert_eq!(
        constants.value(id(5), &mut origin),
        Some((integer, IntegerValue::Unsigned(0)))
    );
    for raw in [6, 7, 8, 99] {
        assert_eq!(constants.value(id(raw), &mut origin), None);
    }
    let mut budgeted = ScalarConstants::for_values((1..=5000).map(id));
    assert_eq!(
        budgeted.value(id(1), |value| Some(if value.get() == 5000 {
            ScalarConstantOrigin::Literal(integer, IntegerValue::Unsigned(1))
        } else {
            ScalarConstantOrigin::Computed(ScalarTerm::value(id(value.get() + 1), integer))
        })),
        None
    );
    assert_eq!(
        budgeted.value(id(1), |_| panic!("an exhausted root is not searched again")),
        None
    );
}

#[test]
fn prepared_scalar_ids_refuse_duplicates_and_keep_sparse_ids_bounded() {
    let high = ValueId::new(u64::MAX).unwrap();
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let mut sparse = ScalarConstants::for_values([high]);
    assert_eq!(sparse.registered_count(), 1);
    assert_eq!(
        sparse.value(high, |_| Some(ScalarConstantOrigin::Literal(
            integer,
            IntegerValue::Unsigned(1)
        ))),
        Some((integer, IntegerValue::Unsigned(1)))
    );
    let mut duplicate = ScalarConstants::for_values([high, high]);
    assert_eq!(
        duplicate.value(high, |_| panic!("duplicate source IDs supply no scope")),
        None
    );
    let mut draft = ScalarConstants::default();
    assert!(draft.register(ValueId::new(2).unwrap()));
    assert!(!draft.register(ValueId::new(1).unwrap()));
    assert_eq!(
        draft.value(ValueId::new(2).unwrap(), |_| panic!(
            "non-monotonic declarations supply no scope"
        )),
        None
    );
}

#[test]
fn mixed_boolean_incoming_values_do_not_prove_a_constant() {
    for equal in [false, true] {
        let actual =
            ScalarConstants::for_values((1..=3).map(|raw| ValueId::new(raw).unwrap()))
                .value(ValueId::new(3).unwrap(), |value| match value.get() {
                    1 => Some(ScalarConstantOrigin::Literal(
                        ScalarType::Boolean,
                        IntegerValue::Unsigned(1),
                    )),
                    2 => Some(ScalarConstantOrigin::Literal(
                        ScalarType::Boolean,
                        IntegerValue::Unsigned(u128::from(u8::from(equal))),
                    )),
                    3 => Some(ScalarConstantOrigin::Incoming(vec![
                        ValueId::new(1).unwrap(),
                        ValueId::new(2).unwrap(),
                    ])),
                    _ => None,
                });
        assert_eq!(
            actual,
            equal.then_some((ScalarType::Boolean, IntegerValue::Unsigned(1)))
        );
    }
}
