/*
We represent pressure-victim evidence (`SpillChoicePlan`), recovery
eligibility classification (`RecoveryClassificationPlan`), and in-process
fixed-view copy insertion (`FixedViewCopyPlan`). Later checks compare these
plans and the programs they share, not digests of them.
*/

use ::selected_instructions::{
    LiveRangePoint, SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionProvenance, VirtualFixedConstraintSite, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use operation_evidence::provenance::ValueDefinitionSite;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    RegisterClassId, RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use semantic_vocabulary::{IntegerValue, MachineId, ScalarType, ValueId, WorkScheduleIdentity};
use std::sync::Arc;

use crate::AllocatorAvailabilityIdentity;

pub mod classification {
    pub use super::{
        FunctionRecoveryClassification, NoAdmittedRecoveryReason, PressureRecoveryClassification,
        RecoveryClassification, RecoveryClassificationPlan, RecoveryClassificationPolicy,
        RecoveryFutureUse, RecoveryVictimRole,
    };
}

pub mod spill_choice {
    pub use super::{
        FunctionSpillChoices, PressureContender, PressureResident, SpillChoice, SpillChoicePlan,
        SpillChoicePolicy,
    };
}

pub mod fixed_view_copy {
    pub use super::{
        FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryClassificationPolicy {
    SelectedVictimImmediateU64EligibilityV1, // Named bounded policy for classifying the already selected pressure victim
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryClassificationPlan {
    pub selected: Arc<SelectedInstructionPlan>,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub policy: RecoveryClassificationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionRecoveryClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRecoveryClassification {
    pub machine: MachineId,
    pub classification: Option<PressureRecoveryClassification>, // Present when the rooted spill-choice function selected a victim
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureRecoveryClassification {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub victim: VirtualRegisterId,
    pub role: RecoveryVictimRole,
    pub scalar_type: ScalarType,
    pub class: RegisterClassId,
    pub origin: VirtualRegisterOrigin,
    pub definition_site: Option<ValueDefinitionSite>,
    pub classification: RecoveryClassification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryVictimRole {
    Incoming,
    ActiveResident {
        current_view: RegisterViewId,
        reclaimed_view: RegisterViewId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryClassification {
    ImmediateU64RematerializationCandidate {
        defining_instruction: SelectedInstructionId,
        source_value: ValueId,
        value: IntegerValue,
        provenance: SelectedInstructionProvenance,
        future_uses: Vec<RecoveryFutureUse>,
    },
    NoAdmittedRecovery {
        reason: NoAdmittedRecoveryReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecoveryFutureUse {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoAdmittedRecoveryReason {
    UnsupportedScalarType,
    EntryParameter,
    UnsupportedRangeShape,
    FutureFixedUse,
    NonMaterializeDefinition,
    ProofBearingDefinition,
    NoFutureUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpillChoicePolicy {
    SingleBlockFarthestEndThenHighestVregV1, // Structural policy for the first locally witnessed pressure point
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillChoicePlan {
    pub selected: Arc<SelectedInstructionPlan>, // Program whose pressure points these choices cover
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub policy: SpillChoicePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionSpillChoices>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSpillChoices {
    pub machine: MachineId,
    pub choice: Option<SpillChoice>, // First pressure point only; None proves the bounded walk encountered no pressure
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillChoice {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub incoming: VirtualRegisterId,
    pub incoming_class: RegisterClassId,
    pub incoming_common_candidates: Vec<RegisterViewId>,
    pub active_residents: Vec<PressureResident>,
    pub contenders: Vec<PressureContender>,
    pub selected_victim: VirtualRegisterId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureResident {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureContender {
    pub virtual_register: VirtualRegisterId,
    pub exclusive_end: LiveRangePoint,
    pub reclaimed_view: Option<RegisterViewId>, // Lowest legal incoming view recovered by evicting the active resident
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedViewCopyPolicy {
    LeafLocalBeforeFixedUseV1, // One copy immediately before each fixed leaf use
    SharedEntryAfterCompareBeforeBranchV1, // Shared-exit leg under entry-parameter admission gate
    ImmediateBeforeFixedUseV1, // One copy in the fixed-use site's own block immediately before the instruction
    SharedSourceExitBeforeFixedUseV1, // Shared copy at a dominating block terminator when leaving one source segment end
}

/// Where active-resident rematerialization places a chosen immediate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PressureRematerializationPolicy {
    SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
    SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedViewCopyPlan {
    pub source_selected: Arc<SelectedInstructionPlan>,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub policy: FixedViewCopyPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub copies: Vec<FixedViewCopy>,
    pub transformed: Arc<SelectedInstructionPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedViewCopy {
    pub function: u32,
    pub machine: MachineId,
    pub source_virtual_register: VirtualRegisterId,
    pub source_value: ValueId,
    pub source_definition_site: ValueDefinitionSite,
    pub from_view: RegisterViewId,
    pub to_view: RegisterViewId, // Common destination view repeated across every destination row
    pub insertion_block: SelectedBlockId,
    pub before_instruction: SelectedInstructionId,
    pub destinations: Vec<FixedViewCopyDestination>,
    pub copy_instruction: SelectedInstructionId,
    pub result_virtual_register: VirtualRegisterId,
    pub copy_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedViewCopyDestination {
    pub site: VirtualFixedConstraintSite, // Exact fixed-use site rewritten to the copy result
    pub block: SelectedBlockId,           // Leaf containing `site`
    pub view: RegisterViewId, // Fixed view required by `site`; equal to the action-wide `to_view`
}

#[cfg(test)]
mod tests {
    use super::{FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy};
    use ::selected_instructions::{
        LiveRangePoint, LivenessPosition, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionPlan, VirtualFixedConstraintSite, VirtualRegisterId,
        test_support::placeholder_plan,
    };
    use operation_evidence::provenance::ValueDefinitionSite;
    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use register_model::{
        RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess, RegisterViewId,
        TargetRegisterEnvironmentIdentity,
    };
    use semantic_vocabulary::{MachineId, ValueId, WorkScheduleIdentity};
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn fixed_view_copy_plan() -> FixedViewCopyPlan {
        FixedViewCopyPlan {
            source_selected: placeholder_plan(1),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            allocator_availability: crate::AllocatorAvailabilityIdentity::from_bytes([5; 32]),
            policy: FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
            budget: OptimizationWorkBudget::new(10, 10, 10, 10, 10).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 2,
                validation_steps: 3,
                commits: 4,
                iterations: 1,
            },
            copies: vec![FixedViewCopy {
                function: 0,
                machine: MachineId::new(1).unwrap(),
                source_virtual_register: VirtualRegisterId(1),
                source_value: ValueId::new(2).unwrap(),
                source_definition_site: ValueDefinitionSite::FunctionParameter(1),
                from_view: RegisterViewId(3),
                to_view: RegisterViewId(7),
                insertion_block: SelectedBlockId(8),
                before_instruction: SelectedInstructionId(6),
                destinations: vec![
                    FixedViewCopyDestination {
                        site: VirtualFixedConstraintSite::Operand {
                            position: LivenessPosition(4),
                            point: LiveRangePoint(5),
                            instruction: SelectedInstructionId(6),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        },
                        block: SelectedBlockId(8),
                        view: RegisterViewId(7),
                    },
                    FixedViewCopyDestination {
                        site: VirtualFixedConstraintSite::Operand {
                            position: LivenessPosition(9),
                            point: LiveRangePoint(10),
                            instruction: SelectedInstructionId(11),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        },
                        block: SelectedBlockId(12),
                        view: RegisterViewId(7),
                    },
                ],
                copy_instruction: SelectedInstructionId(9),
                result_virtual_register: VirtualRegisterId(10),
                copy_constraint: RegisterConstraintKey {
                    family: RegisterConstraintFamily::Instruction,
                    variant: 11,
                },
            }],
            transformed: SelectedInstructionPlan {
                contracts: Default::default(),
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([12; 32]),
                },
                work_schedule: WorkScheduleIdentity::new(1).unwrap(),
                target: target::NativeTarget::linux_x64(),
                entry: MachineId::new(1).unwrap(),
                functions: Default::default(),
            }
            .into(),
        }
    }

    #[test]
    fn fixed_view_copy_plans_compare_their_source_and_transformed_programs() {
        let baseline = fixed_view_copy_plan();
        let mut changed = fixed_view_copy_plan();
        changed.source_selected = placeholder_plan(2);
        assert_ne!(baseline, changed);
        let mut changed = fixed_view_copy_plan();
        std::sync::Arc::make_mut(&mut changed.transformed).entry = MachineId::new(2).unwrap();
        assert_ne!(baseline, changed);
    }
}
