//! Payloadless Unit-return source without structural or provider custody.

use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use omega_optimization_unit::PsiOptimizationUnit;
use omega_target_operations::TargetOperationPlan;
use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

pub(in crate::tests) fn plain_unit_fixture() -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let abstract_plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x50; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            }],
        }],
    };
    let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        omega_target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let unit = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
        &abstract_plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (abstract_plan, target, unit)
}
