use super::*;
use abstract_operations::{AbstractFunctionResult, AbstractResult};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use target_operations::{
    TargetBooleanExpression, TargetControlBlock, TargetControlGraph, TargetScalarExpression,
};

fn fixture() -> (AbstractFunction, TargetFunction, SelectedFunction) {
    let value = ValueId::new(1).unwrap();
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let edge = EdgeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    let parameter = terminal_psi::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: vec![],
        projected_qualifications: vec![],
    };
    let shape = ValueShape::integer(16, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![shape],
            result: Some(ValueShape::integer(1, 1)),
        },
    )
    .unwrap();
    let target_parameter = target_operations::TargetStructuralParameter {
        place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        access: parameter.access,
        projected_qualifications: vec![],
        shape,
        placement: call_plan.parameters[0].clone(),
    };
    let actions = vec![TerminalAffineCleanupAction::DiscardRoot(place)];
    let function = AbstractFunction {
        machine,
        attachment: None,
        entry: block,
        parameters: vec![],
        structural_parameters: vec![parameter.clone()],
        result: AbstractFunctionResult::Scalar(AbstractResult {
            value,
            scalar_type: ScalarType::Boolean,
        }),
        entry_claims: vec![],
        published_service_ceiling: vec![],
        block_entries: vec![],
        operations: vec![AbstractOperation::Return {
            psi_edge: edge,
            result: value,
            value,
            scalar_type: ScalarType::Boolean,
            cleanup_actions: actions.clone(),
        }],
    };
    let provenance = target_operations::TerminalPsiProvenance {
        operations: vec![],
        edges: vec![edge],
    };
    let target = TargetFunction {
        machine,
        attachment: None,
        scalar_abi: None,
        provenance: provenance.clone(),
        mixed_structural_scalar_abi: Some(target_operations::MixedStructuralScalarFunctionAbi {
            call_plan: call_plan.clone(),
            scalar_parameters: vec![],
            structural_parameters: vec![target_parameter.clone()],
            result: target_operations::ScalarAbiValue {
                value,
                scalar_type: ScalarType::Boolean,
                placement: call_plan.result.clone().unwrap(),
            },
        }),
        operation: TargetOperation::ControlGraph(TargetControlGraph {
            structural_types: vec![],
            call_plan,
            scalar_parameters: vec![],
            parameters: vec![target_parameter.clone()],
            entry: block,
            blocks: vec![TargetControlBlock {
                block,
                parameters: vec![],
                structural_parameters: vec![],
                operations: vec![],
                terminator: TargetControlTerminator::ReturnScalar {
                    psi_edge: edge,
                    source_value: value,
                    expression: TargetScalarExpression::Boolean(
                        TargetBooleanExpression::Immediate {
                            source_value: value,
                            value: true,
                        },
                    ),
                    cleanup_actions: actions,
                },
            }],
        }),
    };
    let selected = SelectedFunction {
        machine,
        attachment: None,
        provenance,
        ranked: None,
        structural: Some(legalized_operations::LegalizedStructuralContract {
            result: None,
            structural_types: vec![],
            parameters: vec![legalized_operations::LegalizedCallUnitParameter {
                semantic: parameter,
                target: target_parameter,
            }],
            structural_places: vec![],
            entry_claims: vec![],
            published_service_ceiling: vec![],
        }),
        local_storage_slots: vec![],
        outgoing_arguments: vec![],
        calls: vec![],
        memory_accesses: vec![],
        boundary_settlements: vec![],
        entry_block: selected_instructions::SelectedBlockId(1),
        virtual_registers: vec![],
        blocks: vec![],
    };
    (function, target, selected)
}

#[test]
fn scalar_discard_requires_exact_retained_return() {
    // These predicate fixtures are not complete source replay certificates.
    let (function, target, _) = fixture();
    assert!(scalar_cleanup_retained(&function.operations[0], &target));
    for mutation in 0..5 {
        let mut changed = target.clone();
        let TargetOperation::ControlGraph(graph) = &mut changed.operation else {
            unreachable!()
        };
        let TargetControlTerminator::ReturnScalar {
            psi_edge,
            source_value,
            cleanup_actions,
            ..
        } = &mut graph.blocks[0].terminator
        else {
            unreachable!()
        };
        match mutation {
            0 => cleanup_actions.clear(),
            1 => *psi_edge = EdgeId::new(2).unwrap(),
            2 => *source_value = ValueId::new(2).unwrap(),
            3 => {
                cleanup_actions[0] =
                    TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(2).unwrap())
            }
            _ => graph.blocks.push(graph.blocks[0].clone()),
        }
        assert!(
            !scalar_cleanup_retained(&function.operations[0], &changed),
            "mutation {mutation}"
        );
    }
}

#[test]
fn unused_owned_projection_refuses_borrowed_linear_or_materialized_arrivals() {
    let (function, target, selected) = fixture();
    assert!(arrivals(&function, &target, &selected));
    let mut borrowed = function.clone();
    borrowed.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(!arrivals(&borrowed, &target, &selected));
    let mut linear = function.clone();
    linear.structural_parameters[0].multiplicity = StructuralMultiplicity::Linear;
    assert!(!arrivals(&linear, &target, &selected));
    let mut missing = selected.clone();
    missing.structural = None;
    assert!(!arrivals(&function, &target, &missing));
    let mut materialized = selected.clone();
    materialized
        .local_storage_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                block: BlockId::new(1).unwrap(),
                place: PlaceId::new(1).unwrap(),
            },
            byte_size: 16,
            alignment: 8,
        });
    assert!(!arrivals(&function, &target, &materialized));
    let mut missing_abi = target.clone();
    missing_abi.mixed_structural_scalar_abi = None;
    assert!(!arrivals(&function, &missing_abi, &selected));
    // Missing a legacy mirror cannot authorize unused-input erasure, but the
    // complete graph ABI can still account for the result under mandatory replay.
    let graph_header = super::super::aggregate_results::header;
    assert!(graph_header(&function, &missing_abi, &selected));
    for mutation in 0..3 {
        let mut changed = missing_abi.clone();
        let TargetOperation::ControlGraph(graph) = &mut changed.operation else {
            unreachable!()
        };
        match mutation {
            0 => graph.call_plan.result = None,
            1 => graph.parameters.clear(),
            _ => graph.parameters[0].place = PlaceId::new(2).unwrap(),
        }
        assert!(!graph_header(&function, &changed, &selected));
    }
}

#[test]
fn unobserved_owned_keeps_the_complete_mixed_abi_join() {
    let (function, target, selected) = fixture();
    let admit = |target: &TargetFunction, selected: &SelectedFunction| {
        crate::function_fragments::mixed_scalar_abi::admit(
            &function,
            target,
            selected,
            target.mixed_structural_scalar_abi.as_ref().unwrap(),
        )
    };
    assert!(admit(&target, &selected).is_ok());
    let mut erased = selected.clone();
    erased.structural.as_mut().unwrap().parameters.clear();
    assert!(admit(&target, &erased).is_err());
    for mutation in 0..4 {
        let mut changed = target.clone();
        let abi = changed.mixed_structural_scalar_abi.as_mut().unwrap();
        match mutation {
            0 => abi.structural_parameters.clear(),
            1 => abi.call_plan.parameters.clear(),
            2 => {
                abi.structural_parameters[0].placement.shape = ValueShape::borrowed_reference(16, 8)
            }
            _ => abi.structural_parameters[0].access = StructuralAccess::SharedBorrow,
        }
        assert!(admit(&changed, &selected).is_err(), "mutation {mutation}");
    }
}
