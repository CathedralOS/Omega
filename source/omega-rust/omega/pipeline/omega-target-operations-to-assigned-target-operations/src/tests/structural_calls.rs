use crate::assign_registers;
use crate::assignment::shared::{
    AssignedUnitOperation, CallSignature, CallingPolicy, MachineId, TargetOperationPlan,
    TargetUnitOperation, evaluate_call_plan,
};
use omega_assigned_target_operations::AssignedOperation;
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetOperation, TerminalPsiProvenance};
use psi_core::{OperationId, PlaceId, StructuralTypeId};
use psi_terminal::{
    SemanticFingerprint, StructuralPathSegment, TerminalPsiIdentity, VocabularyMarker,
};

#[test]
fn unit_assignment_retains_typed_structural_argument_paths() {
    let target = NativeTarget::linux_x64();
    let shape = omega_calling_conventions::ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let place = PlaceId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let path = vec![StructuralPathSegment::FixedIndex(1)];
    let plan = TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
        },
        target,
        entry: MachineId::new(1).unwrap(),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::UnitBody(omega_target_operations::TargetUnitBody {
                structural_types: Vec::new(),
                call_plan: call_plan.clone(),
                parameters: Vec::new(),
                operations: vec![TargetUnitOperation::Call {
                    psi_operation: OperationId::new(1).unwrap(),
                    callee: MachineId::new(2).unwrap(),
                    arguments: vec![omega_target_operations::TargetStructuralArgument {
                        place,
                        access: psi_terminal::StructuralAccess::Owned,
                        path: path.clone(),
                        root_structural_type: structural_type,
                        structural_type,
                        shape,
                        source_byte_offset: 0,
                        fixed_array_length: None,
                        element_stride: None,
                        source: call_plan.parameters[0].clone(),
                        destination: call_plan.parameters[0].clone(),
                    }],
                    claim_transfers: Vec::new(),
                }],
            }),
        }],
    };

    let assigned = assign_registers(&plan).unwrap();
    let AssignedOperation::UnitBody(body) = &assigned.functions[0].operation else {
        panic!("Unit body")
    };
    let AssignedUnitOperation::Call { copies, .. } = &body.operations[0] else {
        panic!("Unit call")
    };
    assert_eq!(copies[0].path, path);
}
