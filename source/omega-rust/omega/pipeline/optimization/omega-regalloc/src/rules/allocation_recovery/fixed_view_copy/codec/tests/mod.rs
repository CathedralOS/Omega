use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedViewCopyPolicy,
    LiveRangeIdentity,
};
use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_optimization_unit::{FuelSettlement, PsiProvenance};
use omega_register_model::TargetRegisterEnvironmentIdentity;
use omega_selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use psi_core::{FuelScheduleIdentity, OperationId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::FixedViewCopyPlan;

mod golden;
mod rejection;
mod round_trip;

pub(super) fn plan(policy: FixedViewCopyPolicy) -> FixedViewCopyPlan {
    let (_, _, _, copy, mut function) =
        crate::rules::allocation_recovery::fixed_view_copy::compute::tests::computed_shared_fixture(
        );
    let operation = OperationId::new(1).unwrap();
    function.provenance.operations.push(operation);
    function.blocks[0].instructions[0]
        .provenance
        .operations
        .push(operation);
    function.blocks[0].instructions[0]
        .provenance
        .fuel
        .push(FuelSettlement {
            site: PsiProvenance::Operation(operation),
            units: 7,
        });
    FixedViewCopyPlan {
        source_selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
        source_ranges: LiveRangeIdentity::from_bytes([2; 32]),
        source_legality: AllocationLegalityIdentity::from_bytes([3; 32]),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
        allocator_availability: AllocatorAvailabilityIdentity::from_bytes([5; 32]),
        policy,
        budget: OptimizationWorkBudget::new(3, 3, 3, 3, 1).unwrap(),
        usage: OptimizationWorkUsage {
            rule_evaluations: 1,
            candidates: 2,
            validation_steps: 2,
            commits: 1,
            iterations: 1,
        },
        copies: vec![copy],
        transformed: SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([6; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: omega_target::NativeTarget::linux_x64(),
            entry: function.machine,
            functions: vec![function],
            structural_unit_functions: Vec::new(),
        },
    }
}
