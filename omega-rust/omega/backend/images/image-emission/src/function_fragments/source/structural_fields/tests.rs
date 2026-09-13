use super::retained;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use target_operations::{
    TargetControlBlock, TargetControlGraph, TargetControlTerminator, TargetFunction,
    TargetStructuralParameter, TargetUnitOperation, TerminalPsiProvenance,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment,
};

fn field_fixture(scalar_type: ScalarType) -> (AbstractFunction, TargetFunction) {
    // These fixtures isolate source membership, not object/load replay certificates.
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let edge = EdgeId::new(1).unwrap();
    let psi_operation = OperationId::new(1).unwrap();
    let result = AbstractResult {
        value: ValueId::new(1).unwrap(),
        scalar_type,
    };
    let field = StructuralFieldId::new(1).unwrap();
    let parameter = StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        is_self: true,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
    };
    let operation = match scalar_type {
        ScalarType::Boolean => AbstractOperation::BooleanStructuralField {
            psi_operation,
            result: result.value,
            source: parameter.place,
            field,
        },
        ScalarType::Integer(_) => AbstractOperation::IntegerStructuralField {
            psi_operation,
            result,
            source: parameter.place,
            field,
        },
        ScalarType::IeeeFloat(_) => panic!("fixture covers integer and Boolean field reads"),
    };
    let shape = ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let target_parameter = TargetStructuralParameter {
        place: parameter.place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        access: parameter.access,
        projected_qualifications: vec![],
        shape,
        placement: call_plan.parameters[0].clone(),
    };
    let target_operation = TargetUnitOperation::StructuralScalarFieldRead {
        psi_operation,
        result,
        source: StructuralArgument {
            place: parameter.place,
            access: parameter.access,
            path: vec![],
        },
        field,
    };
    let function = AbstractFunction {
        machine,
        attachment: None,
        entry: block,
        parameters: vec![],
        structural_parameters: vec![parameter],
        result: AbstractFunctionResult::Unit,
        entry_claims: vec![],
        published_service_ceiling: vec![],
        block_entries: vec![],
        operations: vec![
            operation,
            AbstractOperation::ReturnUnit {
                psi_edge: edge,
                cleanup_actions: vec![],
            },
        ],
    };
    let target = TargetFunction {
        machine,
        attachment: None,
        scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance: TerminalPsiProvenance {
            operations: vec![psi_operation],
            edges: vec![edge],
        },
        graph: TargetControlGraph {
            structural_types: vec![].into(),
            call_plan,
            scalar_parameters: vec![],
            parameters: vec![target_parameter],
            entry: block,
            blocks: vec![TargetControlBlock {
                block,
                parameters: vec![],
                structural_parameters: vec![],
                operations: vec![target_operation],
                terminator: TargetControlTerminator::Return {
                    psi_edge: edge,
                    cleanup_actions: vec![],
                },
            }],
        },
    };
    (function, target)
}

fn field_types() -> [ScalarType; 3] {
    [
        ScalarType::Boolean,
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap()),
    ]
}

fn owned_field_fixture(
    scalar_type: ScalarType,
    multiplicity: StructuralMultiplicity,
) -> (AbstractFunction, TargetFunction) {
    let (mut function, mut target) = field_fixture(scalar_type);
    let parameter = &mut function.structural_parameters[0];
    parameter.is_self = false;
    parameter.access = StructuralAccess::Owned;
    parameter.multiplicity = multiplicity;
    target.graph.parameters[0].access = StructuralAccess::Owned;
    target.graph.parameters[0].multiplicity = multiplicity;
    let TargetUnitOperation::StructuralScalarFieldRead { source, .. } =
        &mut target.graph.blocks[0].operations[0]
    else {
        unreachable!()
    };
    source.access = StructuralAccess::Owned;
    (function, target)
}

#[test]
fn owned_parameter_field_membership_keeps_exact_value_access_and_graph_replay() {
    for scalar_type in field_types() {
        for multiplicity in [
            StructuralMultiplicity::Affine,
            StructuralMultiplicity::Unrestricted,
        ] {
            let (function, target) = owned_field_fixture(scalar_type, multiplicity);
            assert!(retained(&function, &function.operations[0], &target));
            assert!(super::super::requires_graph_storage_replay(
                &function.operations
            ));
            for mutation in 0..6 {
                let mut changed = target.clone();
                let TargetUnitOperation::StructuralScalarFieldRead {
                    result,
                    source,
                    field,
                    ..
                } = &mut changed.graph.blocks[0].operations[0]
                else {
                    unreachable!()
                };
                match mutation {
                    0 => source.access = StructuralAccess::SharedBorrow,
                    1 => changed.graph.parameters[0].access = StructuralAccess::SharedBorrow,
                    2 => source.place = PlaceId::new(2).unwrap(),
                    3 => source
                        .path
                        .push(StructuralPathSegment::Field("nested".into())),
                    4 => result.value = ValueId::new(2).unwrap(),
                    _ => *field = StructuralFieldId::new(2).unwrap(),
                }
                assert!(!retained(&function, &function.operations[0], &changed));
            }
        }
        // Even matching declarations cannot grant access to a linear or write-only root.
        let (function, target) = owned_field_fixture(scalar_type, StructuralMultiplicity::Linear);
        assert!(!retained(&function, &function.operations[0], &target));
        let (mut function, mut target) =
            owned_field_fixture(scalar_type, StructuralMultiplicity::Affine);
        function.structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
        target.graph.parameters[0].access = StructuralAccess::WriteOnlyBorrow;
        let TargetUnitOperation::StructuralScalarFieldRead { source, .. } =
            &mut target.graph.blocks[0].operations[0]
        else {
            unreachable!()
        };
        source.access = StructuralAccess::WriteOnlyBorrow;
        assert!(!retained(&function, &function.operations[0], &target));
    }
}

#[test]
fn observed_owned_parameter_discard_keeps_exact_scalar_exit_accounting() {
    use super::super::control_flow;
    use target_operations::{
        TargetBooleanExpression, TargetScalarExpression, TargetUnitScalarHomeRequirement,
    };
    use terminal_psi::TerminalAffineCleanupAction;

    let (mut function, mut target) =
        owned_field_fixture(ScalarType::Boolean, StructuralMultiplicity::Affine);
    let edge = EdgeId::new(1).unwrap();
    let value = ValueId::new(1).unwrap();
    let actions = vec![TerminalAffineCleanupAction::DiscardRoot(
        function.structural_parameters[0].place,
    )];
    function.operations[1] = AbstractOperation::Return {
        psi_edge: edge,
        result: value,
        value,
        scalar_type: ScalarType::Boolean,
        cleanup_actions: actions.clone(),
    };
    target.graph.blocks[0].terminator = TargetControlTerminator::ReturnScalar {
        psi_edge: edge,
        source_value: value,
        expression: TargetScalarExpression::Boolean(TargetBooleanExpression::ScalarHome(
            TargetUnitScalarHomeRequirement {
                defining_operation: OperationId::new(1).unwrap(),
                source_value: value,
                scalar_type: ScalarType::Boolean,
                shape: ValueShape::integer(1, 1),
            },
        )),
        cleanup_actions: actions,
    };
    assert!(retained(&function, &function.operations[0], &target));
    assert!(control_flow::retained(&function.operations[1], &target));
    for mutation in 0..5 {
        let mut changed = target.clone();
        let TargetControlTerminator::ReturnScalar {
            psi_edge,
            source_value,
            cleanup_actions,
            ..
        } = &mut changed.graph.blocks[0].terminator
        else {
            unreachable!()
        };
        match mutation {
            0 => cleanup_actions.clear(),
            1 => {
                cleanup_actions[0] =
                    TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(2).unwrap())
            }
            2 => *psi_edge = EdgeId::new(2).unwrap(),
            3 => *source_value = ValueId::new(2).unwrap(),
            _ => changed.graph.blocks.push(changed.graph.blocks[0].clone()),
        }
        assert!(!control_flow::retained(&function.operations[1], &changed));
    }
}

#[test]
fn structural_field_membership_requires_exact_source_result_type_access_path_and_field() {
    for scalar_type in field_types() {
        let (function, target) = field_fixture(scalar_type);
        let operation = &function.operations[0];
        assert!(retained(&function, operation, &target));
        for mutation in 0..7 {
            let mut changed = target.clone();
            let TargetUnitOperation::StructuralScalarFieldRead {
                psi_operation,
                result,
                source,
                field,
            } = &mut changed.graph.blocks[0].operations[0]
            else {
                unreachable!()
            };
            match mutation {
                0 => *psi_operation = OperationId::new(2).unwrap(),
                1 => result.value = ValueId::new(2).unwrap(),
                2 => {
                    result.scalar_type = if scalar_type == ScalarType::Boolean {
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
                    } else {
                        ScalarType::Boolean
                    }
                }
                3 => source.place = PlaceId::new(2).unwrap(),
                4 => source.access = StructuralAccess::MutableBorrow,
                5 => source
                    .path
                    .push(StructuralPathSegment::Field("nested".to_owned())),
                _ => *field = StructuralFieldId::new(2).unwrap(),
            }
            assert!(
                !retained(&function, operation, &changed),
                "{scalar_type:?}: mutation {mutation}"
            );
        }
    }
}

#[test]
fn structural_field_membership_requires_one_matching_read_across_all_blocks() {
    for scalar_type in field_types() {
        let (function, target) = field_fixture(scalar_type);
        let operation = &function.operations[0];
        let mut changed = target.clone();
        changed.graph.blocks[0].operations.clear();
        assert!(!retained(&function, operation, &changed));
        let mut second_block = target.graph.blocks[0].clone();
        second_block.block = BlockId::new(2).unwrap();
        changed.graph.blocks.push(second_block);
        assert!(retained(&function, operation, &changed));
        changed.graph.blocks[0].operations = target.graph.blocks[0].operations.clone();
        assert!(!retained(&function, operation, &changed));
        let TargetUnitOperation::StructuralScalarFieldRead { field, .. } =
            &mut changed.graph.blocks[1].operations[0]
        else {
            unreachable!()
        };
        *field = StructuralFieldId::new(2).unwrap();
        assert!(!retained(&function, operation, &changed));
        let TargetUnitOperation::StructuralScalarFieldRead { psi_operation, .. } =
            &mut changed.graph.blocks[1].operations[0]
        else {
            unreachable!()
        };
        *psi_operation = OperationId::new(2).unwrap();
        assert!(retained(&function, operation, &changed));
    }
}

#[test]
fn structural_field_membership_requires_unique_exact_parameter_declaration() {
    for scalar_type in field_types() {
        let (function, target) = field_fixture(scalar_type);
        let operation = &function.operations[0];
        let mut missing = function.clone();
        missing.structural_parameters.clear();
        assert!(!retained(&missing, operation, &target));
        let mut duplicate = function.clone();
        duplicate
            .structural_parameters
            .push(function.structural_parameters[0].clone());
        assert!(!retained(&duplicate, operation, &target));
        duplicate.structural_parameters[1].access = StructuralAccess::MutableBorrow;
        assert!(!retained(&duplicate, operation, &target));
        if let AbstractOperation::IntegerStructuralField { .. } = operation {
            for mutation in 0..4 {
                let mut changed = function.clone();
                let source = &mut changed.structural_parameters[0];
                match mutation {
                    0 => source.structural_type = StructuralTypeId::new(2).unwrap(),
                    1 => source.access = StructuralAccess::Owned,
                    2 => source.multiplicity = StructuralMultiplicity::Affine,
                    _ => source.position = 1,
                }
                assert!(
                    !retained(&changed, operation, &target),
                    "mutation {mutation}"
                );
            }
        }
        assert!(!retained(&function, &function.operations[1], &target));
    }
}

#[test]
fn structural_field_reads_require_graph_storage_replay() {
    let unrelated = AbstractOperation::BooleanConstant {
        psi_operation: OperationId::new(2).unwrap(),
        result: ValueId::new(2).unwrap(),
        value: true,
    };
    assert!(!super::super::requires_graph_storage_replay(
        std::slice::from_ref(&unrelated)
    ));
    for scalar_type in field_types() {
        let (function, _) = field_fixture(scalar_type);
        let operation = &function.operations[0];
        assert!(super::super::requires_graph_storage_replay(
            std::slice::from_ref(operation)
        ));
        assert!(super::super::requires_graph_storage_replay(&[
            unrelated.clone(),
            operation.clone()
        ]));
    }
}
