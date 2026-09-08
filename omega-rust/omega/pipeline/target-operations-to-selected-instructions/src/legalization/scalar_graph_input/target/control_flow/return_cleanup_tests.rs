//! Branch-local read results retain exact return cleanup, edge, and fuel custody.

use super::*;
use abstract_operations::{
    AbstractBlockEntry, AbstractBoundaryResult, AbstractParameter,
    AbstractStructuralCasePayloadBinding, AbstractStructuralCaseSuccessor,
};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use legalized_operations::LegalizedScalarTerminator;
use optimization_unit::OwnershipEvent;
use semantic_vocabulary::{
    BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, OperationId,
    PlaceId, StructuralCaseId, StructuralFieldId,
};
use terminal_psi::TerminalAffineCleanupAction;

fn place(ordinal: u64) -> PlaceId {
    PlaceId::new(ordinal).unwrap()
}
fn cleanup(ordinals: &[u64]) -> Vec<TerminalAffineCleanupAction> {
    ordinals
        .iter()
        .map(|ordinal| TerminalAffineCleanupAction::DiscardRoot(place(*ordinal)))
        .collect()
}

fn source_fixture(native: ::target::NativeTarget) -> AbstractOperationPlan {
    let (mut plan, _, _) = crate::tests::legalization::byte_input::fixture(native);
    let function = &mut plan.functions[0];
    let entry = function.entry;
    let empty = BlockId::new(20).unwrap();
    let present = BlockId::new(30).unwrap();
    let payload = ValueId::new(10).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let template = function.operations[0].clone();
    let read = |ordinal| {
        let mut read = template.clone();
        let AbstractOperation::BoundaryCall {
            psi_operation,
            result: AbstractBoundaryResult::Structural(result),
            ..
        } = &mut read
        else {
            panic!("read result");
        };
        *psi_operation = OperationId::new(ordinal).unwrap();
        result.place = place(ordinal);
        read
    };
    // The first result is consumed on both case edges. Each arm then owns two
    // distinct fresh results; neither sibling's places are live on its return.
    function.operations = vec![
        read(1),
        AbstractOperation::StructuralCase {
            source: place(1),
            cases: vec![
                AbstractStructuralCaseSuccessor {
                    psi_edge: EdgeId::new(1).unwrap(),
                    target: empty,
                    case: StructuralCaseId::new(1).unwrap(),
                    payloads: Vec::new(),
                    trivial_affine_discards: vec![place(1)],
                },
                AbstractStructuralCaseSuccessor {
                    psi_edge: EdgeId::new(2).unwrap(),
                    target: present,
                    case: StructuralCaseId::new(2).unwrap(),
                    payloads: vec![AbstractStructuralCasePayloadBinding {
                        parameter: payload,
                        field: StructuralFieldId::new(1).unwrap(),
                        scalar_type,
                    }],
                    trivial_affine_discards: vec![place(1)],
                },
            ],
        },
        read(2),
        read(3),
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(3).unwrap(),
            cleanup_actions: cleanup(&[3, 2]),
        },
        read(4),
        read(5),
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(4).unwrap(),
            cleanup_actions: cleanup(&[5, 4]),
        },
    ];
    function.block_entries = vec![
        AbstractBlockEntry {
            block: entry,
            operation_offset: 0,
            structural_parameters: Vec::new(),
            parameters: Vec::new(),
        },
        AbstractBlockEntry {
            block: empty,
            operation_offset: 2,
            structural_parameters: Vec::new(),
            parameters: Vec::new(),
        },
        AbstractBlockEntry {
            block: present,
            operation_offset: 5,
            structural_parameters: Vec::new(),
            parameters: vec![AbstractParameter {
                value: payload,
                scalar_type,
            }],
        },
    ];
    plan
}

fn current(plan: &AbstractOperationPlan) -> PsiOptimizationUnit {
    optimization_unit::reconstruct_psi_optimization_unit_seed(
        plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

fn target(plan: &AbstractOperationPlan, native: ::target::NativeTarget) -> TargetOperationPlan {
    abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        plan,
        native,
        &[AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(1).unwrap(),
            execution: AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedReadByte,
            ),
            realization: target_operations::HostedReadByteRealization.into(),
        }],
    )
    .expect("both read-result arms retain ordinary return cleanup")
}

fn hostile_cleanup(mutation: &str) -> Vec<TerminalAffineCleanupAction> {
    cleanup(match mutation {
        "missing" => &[],
        "sibling" => &[5, 2],
        "duplicate" => &[3, 3, 2],
        "already consumed" => &[3, 2, 1],
        "out of order" => &[2, 3],
        _ => panic!("unknown cleanup mutation"),
    })
}

#[test]
fn branch_local_read_returns_replay_exact_cleanup_and_graph_custody() {
    for native in [
        ::target::NativeTarget::linux_x64(),
        ::target::NativeTarget::linux_arm64(),
        ::target::NativeTarget::macos_arm64(),
    ] {
        let plan = source_fixture(native);
        let unit = current(&plan);
        optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
        let target = target(&plan, native);
        super::super::validate_target(
            &target.functions[0],
            &plan.functions[0],
            &unit.functions[0],
            &target,
            &plan,
            &unit,
        )
        .unwrap();
        let legal = crate::legalize_target_operations(&target, &plan, &unit).unwrap();
        crate::validate_legalized_operations(&target, &plan, &unit, legal.plan().clone()).unwrap();
        let function = &legal.plan().scalar_functions[0];
        assert_eq!(
            function
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            unit.functions[0]
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>()
        );
        for (block_position, expected) in [(1, cleanup(&[3, 2])), (2, cleanup(&[5, 4]))] {
            let LegalizedScalarTerminator::Return(returned) =
                &function.blocks[block_position].terminator
            else {
                panic!("Unit return");
            };
            let source = unit.functions[0].blocks[block_position]
                .nodes
                .last()
                .unwrap();
            assert_eq!(returned.ownership, vec![OwnershipEvent::Cleanup(expected)]);
            assert_eq!(returned.fuel, source.fuel);
            assert_eq!(returned.effect, source.effect);
            let AbstractOperation::ReturnUnit { psi_edge, .. } = source.operation else {
                panic!("source return");
            };
            assert_eq!(returned.edge, psi_edge);
        }
    }
}

#[test]
fn current_unit_rejects_inexact_branch_return_frontier_even_with_recomputed_metadata() {
    let source = source_fixture(::target::NativeTarget::linux_x64());
    optimization_unit_semantics::validate_psi_optimization_unit(&current(&source)).unwrap();
    for mutation in [
        "missing",
        "sibling",
        "duplicate",
        "already consumed",
        "out of order",
    ] {
        let mut changed = source.clone();
        let AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } = &mut changed.functions[0].operations[4]
        else {
            panic!("return");
        };
        *cleanup_actions = hostile_cleanup(mutation);
        // Reconstruct ownership metadata and canonical identity from the changed
        // operation: rejection must not rely on an accidentally stale digest.
        assert!(
            optimization_unit_semantics::validate_psi_optimization_unit(&current(&changed))
                .is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn target_return_replay_rejects_missing_sibling_duplicate_consumed_order_and_edge() {
    let native = ::target::NativeTarget::linux_x64();
    let plan = source_fixture(native);
    let unit = current(&plan);
    let target = target(&plan, native);
    for mutation in [
        "missing",
        "sibling",
        "duplicate",
        "already consumed",
        "out of order",
        "edge",
    ] {
        let mut changed = target.clone();
        let TargetOperation::ControlGraph(graph) = &mut changed.functions[0].operation else {
            panic!("graph");
        };
        let TargetControlTerminator::Return {
            psi_edge,
            cleanup_actions,
        } = &mut graph.blocks[1].terminator
        else {
            panic!("return");
        };
        if mutation == "edge" {
            *psi_edge = EdgeId::new(4).unwrap();
        } else {
            *cleanup_actions = hostile_cleanup(mutation);
        }
        assert!(
            super::super::validate_target(
                &changed.functions[0],
                &plan.functions[0],
                &unit.functions[0],
                &changed,
                &plan,
                &unit
            )
            .is_err(),
            "accepted {mutation}"
        );
        assert!(
            crate::legalize_target_operations(&changed, &plan, &unit).is_err(),
            "legalized {mutation}"
        );
    }
}

#[test]
fn legalized_return_replay_rejects_cleanup_edge_fuel_and_effect_substitution() {
    let native = ::target::NativeTarget::linux_arm64();
    let plan = source_fixture(native);
    let unit = current(&plan);
    let target = target(&plan, native);
    let legal = crate::legalize_target_operations(&target, &plan, &unit).unwrap();
    let identity = legalized_operations::legalized_operation_plan_identity(legal.plan());
    for mutation in [
        "missing",
        "sibling",
        "duplicate",
        "already consumed",
        "out of order",
        "edge",
        "fuel",
        "effect",
    ] {
        let mut changed = legal.plan().clone();
        let LegalizedScalarTerminator::Return(returned) =
            &mut changed.scalar_functions[0].blocks[1].terminator
        else {
            panic!("return");
        };
        match mutation {
            "edge" => returned.edge = EdgeId::new(4).unwrap(),
            "fuel" => returned.fuel.clear(),
            "effect" => returned.effect.output += 1,
            _ => returned.ownership = vec![OwnershipEvent::Cleanup(hostile_cleanup(mutation))],
        }
        assert_ne!(
            legalized_operations::legalized_operation_plan_identity(&changed),
            identity,
            "identity omitted {mutation}"
        );
        assert!(
            crate::validate_legalized_operations(&target, &plan, &unit, changed).is_err(),
            "accepted {mutation}"
        );
    }
}
