//! Input correspondence controls; selection remains a separate admission boundary.

use super::*;
use abstract_operations::{
    AbstractBlockEntry, AbstractParameter, AbstractStructuralCasePayloadBinding,
    AbstractStructuralCaseSuccessor,
};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use semantic_vocabulary::{
    BoundaryMachineId, EdgeId, FuelScheduleIdentity, OperationId, PlaceId, StructuralCaseId,
    StructuralFieldId,
};

fn fixture(
    native: ::target::NativeTarget,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut plan, _, _) = crate::tests::legalization::byte_input::fixture(native);
    let function = &mut plan.functions[0];
    let entry = function.entry;
    let empty = BlockId::new(20).unwrap();
    let present = BlockId::new(30).unwrap();
    let join = BlockId::new(40).unwrap();
    let parameter = ValueId::new(10).unwrap();
    let scalar_type = ScalarType::Integer(i32_type());
    let read = function.operations[0].clone();
    let jump = |edge| AbstractOperation::Jump {
        psi_edge: EdgeId::new(edge).unwrap(),
        target: join,
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    function.operations = vec![
        read,
        AbstractOperation::StructuralCase {
            source: PlaceId::new(1).unwrap(),
            cases: vec![
                AbstractStructuralCaseSuccessor {
                    psi_edge: EdgeId::new(1).unwrap(),
                    target: empty,
                    case: StructuralCaseId::new(1).unwrap(),
                    payloads: Vec::new(),
                    trivial_affine_discards: vec![PlaceId::new(1).unwrap()],
                },
                AbstractStructuralCaseSuccessor {
                    psi_edge: EdgeId::new(2).unwrap(),
                    target: present,
                    case: StructuralCaseId::new(2).unwrap(),
                    payloads: vec![AbstractStructuralCasePayloadBinding {
                        parameter,
                        field: StructuralFieldId::new(1).unwrap(),
                        scalar_type,
                    }],
                    trivial_affine_discards: vec![PlaceId::new(1).unwrap()],
                },
            ],
        },
        jump(3),
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(5).unwrap(),
            cleanup_actions: Vec::new(),
        },
        jump(4),
    ];
    function.block_entries = [
        (entry, 0, false),
        (empty, 2, false),
        (join, 3, false),
        (present, 4, true),
    ]
    .into_iter()
    .map(|(block, operation_offset, payload)| AbstractBlockEntry {
        block,
        operation_offset,
        structural_parameters: Vec::new(),
        parameters: if payload {
            vec![AbstractParameter {
                value: parameter,
                scalar_type,
            }]
        } else {
            Vec::new()
        },
    })
    .collect();
    function.operations.insert(
        4,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(2).unwrap(),
            boundary: BoundaryMachineId::new(2).unwrap(),
            result: abstract_operations::AbstractBoundaryResult::Unit,
            arguments: vec![parameter],
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    let mut output = plan.boundary_machines[0].clone();
    output.id = BoundaryMachineId::new(2).unwrap();
    output.identity = "test::output".into();
    output.scalar_parameters = vec![scalar_type];
    output.result = terminal_psi::BoundaryMachineResult::Unit;
    plan.boundary_machines.push(output);
    let target = abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        &plan, native, &[AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(1).unwrap(),
            execution: AdmittedBoundaryExecution::CompilerBuiltin(target_operations::CompilerBuiltinExecution::LinuxReadByte),
            realization: target_operations::LinuxReadByteRealization.into(),
        }, AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(2).unwrap(),
            execution: AdmittedBoundaryExecution::CompilerBuiltin(target_operations::CompilerBuiltinExecution::HostedWriteByteI32),
            realization: target_operations::HostedWriteByteI32Realization.into(),
        }],
    ).expect("case arms can jump to an ordinary returning join in non-topological roster order");
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("case ownership and payload graph");
    (plan, target, unit)
}

#[test]
fn structural_case_graph_replays_exact_payloads_and_cleanup() {
    for native in [
        ::target::NativeTarget::linux_x64(),
        ::target::NativeTarget::linux_arm64(),
    ] {
        let (plan, target, unit) = fixture(native);
        super::super::validate_target(
            &target.functions[0],
            &plan.functions[0],
            &unit.functions[0],
            &target,
            &plan,
            &unit,
        )
        .unwrap();
        assert!(
            crate::legalize_target_operations(&target, &plan, &unit).is_err(),
            "target correspondence does not imply implemented native case selection"
        );
    }
}

#[test]
fn structural_case_graph_rejects_substituted_target_custody() {
    let (plan, target, unit) = fixture(::target::NativeTarget::linux_x64());
    for mutation in [
        "edge",
        "case",
        "tag",
        "roster",
        "target",
        "parameter block",
        "parameter value",
        "parameter type",
        "field",
        "offset",
        "producer",
        "place",
        "layout",
        "lost cleanup",
        "duplicate cleanup",
        "missing payload",
        "arm producer",
    ] {
        let mut changed = target.clone();
        let TargetOperation::UnitGraph(graph) = &mut changed.functions[0].operation else {
            panic!("graph")
        };
        let join = graph.blocks[2].block;
        let TargetUnitTerminator::StructuralCase { source, cases } =
            &mut graph.blocks[0].terminator
        else {
            panic!("case")
        };
        match mutation {
            "edge" => cases[0].psi_edge = EdgeId::new(99).unwrap(),
            "case" => cases[0].case = StructuralCaseId::new(99).unwrap(),
            "tag" => cases[0].case_tag = 1,
            "roster" => cases.swap(0, 1),
            "target" => cases[0].target = cases[1].target,
            "parameter block" => cases[1].payloads[0].parameter.block = join,
            "parameter value" => cases[1].payloads[0].parameter.value = ValueId::new(99).unwrap(),
            "parameter type" => {
                cases[1].payloads[0].parameter.scalar_type = ScalarType::Integer(u32_type())
            }
            "field" => cases[1].payloads[0].field = StructuralFieldId::new(99).unwrap(),
            "offset" => cases[1].payloads[0].field_byte_offset = 0,
            "producer" => source.defining_operation = OperationId::new(99).unwrap(),
            "place" => source.result.place = PlaceId::new(99).unwrap(),
            "layout" => {
                let target_operations::TargetStructuralHomeLayout::Sum(layout) = &mut source.layout
                else {
                    panic!("sum")
                };
                layout.payload_byte_offset = 0;
            }
            "lost cleanup" => cases[0].trivial_affine_discards.clear(),
            "duplicate cleanup" => cases[0]
                .trivial_affine_discards
                .push(PlaceId::new(1).unwrap()),
            "missing payload" => cases[1].payloads.clear(),
            "arm producer" => {
                let producer = graph.blocks[0].operations.remove(0);
                graph.blocks[1].operations.push(producer);
            }
            _ => unreachable!(),
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
    }
}
