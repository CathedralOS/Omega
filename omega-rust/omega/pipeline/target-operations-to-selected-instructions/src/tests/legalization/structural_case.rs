//! Lawful owned read-result case graph shared by producer and receiving tests.
use abstract_operations::{
    AbstractBlockEntry, AbstractOperation, AbstractOperationPlan, AbstractParameter,
    AbstractStructuralCasePayloadBinding, AbstractStructuralCaseSuccessor,
};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, ValueId,
};
use target_operations::TargetOperationPlan;

pub(crate) fn fixture(
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
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
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
            execution: AdmittedBoundaryExecution::CompilerBuiltin(target_operations::CompilerBuiltinExecution::HostedReadByte),
            realization: target_operations::HostedReadByteRealization.into(),
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
