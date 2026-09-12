//! Portable successor copies remain independent of a still-dominating source.

use super::*;

#[test]
fn verified_owned_record_successor_copy_survives_mutation_of_original() {
    let mut module = record_module();
    let mut writer = unit_module().machines.remove(0);
    writer.id = MachineId::new(902).unwrap();
    writer.contract.id = ContractId::new(902).unwrap();
    writer.entry = BlockId::new(902).unwrap();
    writer.blocks[0].id = writer.entry;
    let mut parameter = getter().structural_parameters.remove(0);
    parameter.place = PlaceId::new(21).unwrap();
    parameter.is_self = false;
    parameter.access = StructuralAccess::MutableBorrow;
    writer.structural_parameters = vec![parameter.clone()];
    writer.structural_places = vec![StructuralPlaceDeclaration {
        id: parameter.place,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    writer.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: OperationId::new(21).unwrap(),
            result: OperationResult::Scalar(scalar(21)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(99),
            },
        },
        Operation {
            static_reach_binding: None,
            id: OperationId::new(22).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: parameter.place,
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
                value: ValueId::new(21).unwrap(),
            },
        },
    ];
    writer.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(902).unwrap(),
        trivial_affine_discards: Vec::new(),
    };

    let caller = &mut module.machines[0];
    let target = BlockId::new(904).unwrap();
    parameter.place = PlaceId::new(2).unwrap();
    parameter.access = StructuralAccess::Owned;
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: parameter.place,
        kind: StructuralPlaceKind::BlockParameter {
            block: target,
            position: 0,
        },
    });
    caller.blocks[0].terminator = Terminator::Jump {
        edge: EdgeId::new(904).unwrap(),
        target,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(1).unwrap(),
            access: StructuralAccess::Owned,
            path: Vec::new(),
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    caller.result = TerminalMachineResult::Scalar(scalar(6));
    caller.blocks.push(Block {
        id: target,
        parameters: Vec::new(),
        structural_parameters: vec![parameter],
        operations: vec![
            Operation {
                static_reach_binding: None,
                id: OperationId::new(3).unwrap(),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: writer.id,
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: PlaceId::new(1).unwrap(),
                        access: StructuralAccess::MutableBorrow,
                        path: Vec::new(),
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            },
            getter_call(4, 1),
            getter_call(5, 2),
        ],
        terminator: Terminator::Return {
            edge: EdgeId::new(900).unwrap(),
            value: ValueId::new(5).unwrap(),
            cleanup_actions: Vec::new(),
        },
    });
    module.machines.extend([getter(), writer]);
    terminal_verifier::validate_module(&module)
        .expect("dominating source remains legal after an owned unrestricted edge copy");
    // The existing runner canonical-decodes and verifies again, then executes
    // with one-unit replenishments from zero fuel across the copy and mutation.
    let (execution, result) = run(&module, &[unsigned(41)]);
    assert_eq!(
        execution.values[&ValueId::new(4).unwrap()],
        unsigned(99),
        "the original was really mutated"
    );
    assert_eq!(
        result,
        TerminalExecutionResult::Scalar(unsigned(41)),
        "owned successor retained its independent pre-mutation payload"
    );
}
