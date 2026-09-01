use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use omega_optimization_unit::{
    PsiOptimizationUnit, recompute_psi_optimization_unit_identity,
    reconstruct_psi_optimization_unit_seed,
};
use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::rules::tests::id;

pub(crate) fn unreachable_private_machine_unit() -> PsiOptimizationUnit {
    let machine = id(36_001, MachineId::new);
    let entry = id(36_002, BlockId::new);
    let mut unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([86; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: entry,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::ReturnUnit {
                    psi_edge: id(36_003, EdgeId::new),
                    cleanup_actions: Vec::new(),
                }],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let mut private = unit.functions[0].clone();
    private.machine = id(36_004, MachineId::new);
    unit.functions.push(private);
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    unit
}
