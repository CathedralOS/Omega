/*
We represent pressure-victim evidence (`SpillChoicePlan`), recovery
eligibility classification (`RecoveryClassificationPlan`), and in-process
fixed-view copy insertion (`FixedViewCopyPlan`), together with their
domain-separated identities.
*/

use ::selected_instructions::{
    FixedViewCopyIdentity, LiveRangeIdentity, LiveRangePoint, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, VirtualFixedConstraintSite, VirtualRegisterId,
    VirtualRegisterOrigin, selected_instruction_plan_identity,
};
use operation_evidence::provenance::{
    PsiProvenance, ValueDefinitionSite, encode_value_definition_site_identity,
};
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    RegisterClassId, RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use semantic_vocabulary::{
    IntegerSign, IntegerValue, MachineId, ScalarType, ValueId, WorkScheduleIdentity,
};

use crate::codec_bytes::{domain_identity, length, list, option, push_encoded};
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSegmentHomePlanIdentity, FixedPrecoloredSplitRequirementPlanIdentity,
    encode_fixed_constraint_site_identity,
};

pub mod classification {
    pub use super::{
        FunctionRecoveryClassification, NoAdmittedRecoveryReason, PressureRecoveryClassification,
        RecoveryClassification, RecoveryClassificationIdentity, RecoveryClassificationPlan,
        RecoveryClassificationPolicy, RecoveryFutureUse, RecoveryVictimRole,
        recovery_classification_identity,
    };
}

pub mod spill_choice {
    pub use super::{
        FunctionSpillChoices, PressureContender, PressureResident, SpillChoice,
        SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy, spill_choice_identity,
    };
}

pub mod fixed_view_copy {
    pub use super::{
        FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy,
        FixedViewCopySourceEvidence, fixed_view_copy_identity,
    };
}

crate::identity_newtype! { pub struct RecoveryClassificationIdentity(pub(crate) [u8; 32]); }
crate::identity_newtype! { pub struct SpillChoiceIdentity(pub(crate) [u8; 32]); }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryClassificationPolicy {
    SelectedVictimImmediateU64EligibilityV1, // Named bounded policy for classifying the already selected pressure victim
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryClassificationPlan {
    pub selected: SelectedInstructionPlanIdentity,
    pub spill_choices: SpillChoiceIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
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

pub fn recovery_classification_identity(
    plan: &RecoveryClassificationPlan,
) -> RecoveryClassificationIdentity {
    RecoveryClassificationIdentity(domain_identity(
        b"omega.terminal-recovery-classification.v7\0",
        &encode_terminal_recovery_classification_content(plan),
    ))
}

pub(crate) fn encode_terminal_recovery_classification_content(
    plan: &RecoveryClassificationPlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.spill_choices.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(
        &mut bytes,
        plan.functions.len(),
        "recovery-classification identity",
    );
    for function in &plan.functions {
        push_encoded(&mut bytes, function.machine.get().to_le_bytes());
        bytes.push(u8::from(function.classification.is_some()));
        if let Some(row) = &function.classification {
            push_encoded(&mut bytes, row.block.0.to_le_bytes());
            push_encoded(&mut bytes, row.point.0.to_le_bytes());
            push_encoded(&mut bytes, row.victim.0.to_le_bytes());
            match row.role {
                RecoveryVictimRole::Incoming => bytes.push(0),
                RecoveryVictimRole::ActiveResident {
                    current_view,
                    reclaimed_view,
                } => {
                    bytes.push(1);
                    push_encoded(&mut bytes, current_view.0.to_le_bytes());
                    push_encoded(&mut bytes, reclaimed_view.0.to_le_bytes());
                }
            }
            encode_classification_scalar_type(&mut bytes, row.scalar_type);
            push_encoded(&mut bytes, row.class.0.to_le_bytes());
            row.origin.encode_identity(&mut bytes);
            match row.definition_site {
                Some(site) => {
                    bytes.push(1);
                    encode_value_definition_site_identity(&mut bytes, site);
                }
                None => bytes.push(0),
            }
            match &row.classification {
                RecoveryClassification::ImmediateU64RematerializationCandidate {
                    defining_instruction,
                    source_value,
                    value,
                    provenance,
                    future_uses,
                } => {
                    bytes.push(0);
                    push_encoded(&mut bytes, defining_instruction.0.to_le_bytes());
                    push_encoded(&mut bytes, source_value.get().to_le_bytes());
                    target_vocabulary::encode_integer(&mut bytes, *value);
                    encode_classification_provenance(&mut bytes, provenance);
                    length(
                        &mut bytes,
                        future_uses.len(),
                        "recovery-classification identity",
                    );
                    for future_use in future_uses {
                        push_encoded(&mut bytes, future_use.block.0.to_le_bytes());
                        push_encoded(&mut bytes, future_use.point.0.to_le_bytes());
                        push_encoded(&mut bytes, future_use.instruction.0.to_le_bytes());
                        push_encoded(&mut bytes, future_use.operand.to_le_bytes());
                    }
                }
                RecoveryClassification::NoAdmittedRecovery { reason } => {
                    bytes.push(1);
                    bytes.push(match reason {
                        NoAdmittedRecoveryReason::UnsupportedScalarType => 0,
                        NoAdmittedRecoveryReason::EntryParameter => 1,
                        NoAdmittedRecoveryReason::UnsupportedRangeShape => 2,
                        NoAdmittedRecoveryReason::FutureFixedUse => 3,
                        NoAdmittedRecoveryReason::NonMaterializeDefinition => 4,
                        NoAdmittedRecoveryReason::ProofBearingDefinition => 5,
                        NoAdmittedRecoveryReason::NoFutureUse => 6,
                    });
                }
            }
        }
    }
    bytes
}

fn encode_classification_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Callable(contract) => {
            bytes.push(5);
            push_encoded(bytes, contract.get().to_le_bytes());
        }
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) if integer.is_address() => {
            bytes.push(3);
            integer
                .width()
                .write_identity(|part| bytes.extend_from_slice(part));
        }
        ScalarType::Integer(integer) => {
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            });
            integer
                .width()
                .write_identity(|part| bytes.extend_from_slice(part));
        }
        ScalarType::IeeeFloat(format) => {
            bytes.push(4);
            bytes.push(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

fn encode_classification_provenance(
    bytes: &mut Vec<u8>,
    provenance: &SelectedInstructionProvenance,
) {
    const LABEL: &str = "recovery-classification identity";
    list(bytes, &provenance.operations, LABEL, |bytes, operation| {
        push_encoded(bytes, operation.get().to_le_bytes());
    });
    list(bytes, &provenance.values, LABEL, |bytes, value| {
        push_encoded(bytes, value.get().to_le_bytes());
    });
    list(bytes, &provenance.edges, LABEL, |bytes, edge| {
        push_encoded(bytes, edge.get().to_le_bytes());
    });
    list(
        bytes,
        &provenance.obligations,
        LABEL,
        |bytes, obligation| {
            push_encoded(bytes, obligation.get().to_le_bytes());
        },
    );
    list(bytes, &provenance.work, LABEL, |bytes, settlement| {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                push_encoded(bytes, operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                push_encoded(bytes, edge.get().to_le_bytes());
            }
        }
        push_encoded(bytes, settlement.units.to_le_bytes());
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpillChoicePolicy {
    SingleBlockFarthestEndThenHighestVregV1, // Structural policy for the first locally witnessed pressure point
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillChoicePlan {
    pub legality: AllocationLegalityIdentity,
    pub ranges: LiveRangeIdentity,
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

pub fn spill_choice_identity(plan: &SpillChoicePlan) -> SpillChoiceIdentity {
    SpillChoiceIdentity(domain_identity(
        b"omega.terminal-spill-choices.v2\0",
        &encode_terminal_spill_choice_content(plan),
    ))
}

pub(crate) fn encode_terminal_spill_choice_content(plan: &SpillChoicePlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.push(match plan.policy {
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len(), "spill-choice identity");
    for function in &plan.functions {
        push_encoded(&mut bytes, function.machine.get().to_le_bytes());
        bytes.push(u8::from(function.choice.is_some()));
        if let Some(choice) = &function.choice {
            push_encoded(&mut bytes, choice.block.0.to_le_bytes());
            push_encoded(&mut bytes, choice.point.0.to_le_bytes());
            push_encoded(&mut bytes, choice.incoming.0.to_le_bytes());
            push_encoded(&mut bytes, choice.incoming_class.0.to_le_bytes());
            length(
                &mut bytes,
                choice.incoming_common_candidates.len(),
                "spill-choice identity",
            );
            for view in &choice.incoming_common_candidates {
                push_encoded(&mut bytes, view.0.to_le_bytes());
            }
            length(
                &mut bytes,
                choice.active_residents.len(),
                "spill-choice identity",
            );
            for resident in &choice.active_residents {
                push_encoded(&mut bytes, resident.virtual_register.0.to_le_bytes());
                push_encoded(&mut bytes, resident.class.0.to_le_bytes());
                push_encoded(&mut bytes, resident.start.0.to_le_bytes());
                push_encoded(&mut bytes, resident.exclusive_end.0.to_le_bytes());
                push_encoded(&mut bytes, resident.view.0.to_le_bytes());
            }
            length(&mut bytes, choice.contenders.len(), "spill-choice identity");
            for contender in &choice.contenders {
                push_encoded(&mut bytes, contender.virtual_register.0.to_le_bytes());
                push_encoded(&mut bytes, contender.exclusive_end.0.to_le_bytes());
                option(&mut bytes, contender.reclaimed_view, |bytes, view| {
                    push_encoded(bytes, view.0.to_le_bytes());
                });
            }
            push_encoded(&mut bytes, choice.selected_victim.0.to_le_bytes());
        }
    }
    bytes
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedViewCopySourceEvidence {
    // Recovery consumes these three admitted plans together. There is no
    // alternate source contract based only on historical legality transitions.
    pub fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub split_requirements: FixedPrecoloredSplitRequirementPlanIdentity,
    pub segment_homes: FixedPrecoloredSegmentHomePlanIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedViewCopyPlan {
    pub source_selected: SelectedInstructionPlanIdentity,
    pub source_ranges: LiveRangeIdentity,
    pub source_legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub source_evidence: FixedViewCopySourceEvidence,
    pub policy: FixedViewCopyPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub copies: Vec<FixedViewCopy>,
    pub transformed: std::sync::Arc<SelectedInstructionPlan>,
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

pub fn fixed_view_copy_identity(plan: &FixedViewCopyPlan) -> FixedViewCopyIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.source_ranges.bytes());
    bytes.extend_from_slice(&plan.source_legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.source_evidence.fixed_intervals.bytes());
    bytes.extend_from_slice(&plan.source_evidence.split_requirements.bytes());
    bytes.extend_from_slice(&plan.source_evidence.segment_homes.bytes());
    bytes.push(match plan.policy {
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => 0,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => 1,
        FixedViewCopyPolicy::ImmediateBeforeFixedUseV1 => 2,
        FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1 => 3,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    crate::codec_bytes::length(&mut bytes, plan.copies.len(), "fixed-view copy identity");
    for copy in &plan.copies {
        crate::codec_bytes::push_encoded(&mut bytes, copy.function.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, copy.machine.get().to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, copy.source_virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, copy.source_value.get().to_le_bytes());
        encode_value_definition_site_identity(&mut bytes, copy.source_definition_site);
        crate::codec_bytes::push_encoded(&mut bytes, copy.from_view.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, copy.to_view.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, copy.insertion_block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, copy.before_instruction.0.to_le_bytes());
        crate::codec_bytes::length(
            &mut bytes,
            copy.destinations.len(),
            "fixed-view copy identity",
        );
        for destination in &copy.destinations {
            encode_fixed_constraint_site_identity(&mut bytes, destination.site);
            crate::codec_bytes::push_encoded(&mut bytes, destination.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, destination.view.0.to_le_bytes());
        }
        crate::codec_bytes::push_encoded(&mut bytes, copy.copy_instruction.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, copy.result_virtual_register.0.to_le_bytes());
        bytes.push(copy.copy_constraint.family.tag());
        crate::codec_bytes::push_encoded(&mut bytes, copy.copy_constraint.variant.to_le_bytes());
    }
    bytes.extend_from_slice(&selected_instruction_plan_identity(&plan.transformed).bytes());
    FixedViewCopyIdentity::from_bytes(crate::codec_bytes::domain_identity(
        b"omega.terminal-fixed-view-copies.v10\0",
        &bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy,
        FixedViewCopySourceEvidence, FunctionRecoveryClassification, NoAdmittedRecoveryReason,
        PressureRecoveryClassification, RecoveryClassification, RecoveryClassificationPlan,
        RecoveryClassificationPolicy, RecoveryFutureUse, RecoveryVictimRole, SpillChoiceIdentity,
        SpillChoicePlan, SpillChoicePolicy, fixed_view_copy_identity,
        recovery_classification_identity, spill_choice_identity,
    };
    use crate::AllocationLegalityIdentity;
    use ::selected_instructions::{
        LiveRangeIdentity, LiveRangePoint, LivenessPosition, SelectedBlockId,
        SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
        SelectedInstructionProvenance, VirtualFixedConstraintSite, VirtualRegisterId,
        VirtualRegisterOrigin,
    };
    use operation_evidence::provenance::{PsiProvenance, ValueDefinitionSite, WorkSettlement};
    use optimization_core::{
        OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    };
    use register_model::{
        RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
        RegisterViewId, TargetRegisterEnvironmentIdentity,
    };
    use semantic_vocabulary::{
        BlockId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType, ValueId,
        WorkScheduleIdentity,
    };
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn classification_plan() -> RecoveryClassificationPlan {
        let operation = OperationId::new(8).unwrap();
        let value = ValueId::new(9).unwrap();
        RecoveryClassificationPlan {
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            spill_choices: SpillChoiceIdentity::from_bytes([2; 32]),
            ranges: LiveRangeIdentity::from_bytes([3; 32]),
            legality: AllocationLegalityIdentity::from_bytes([4; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([5; 32]),
            allocator_availability: crate::AllocatorAvailabilityIdentity::from_bytes([9; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([6; 32]),
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            policy: RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            budget: OptimizationWorkBudget::new(10, 10, 10, 10, 1).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 1,
                validation_steps: 7,
                commits: 1,
                iterations: 1,
            },
            functions: vec![FunctionRecoveryClassification {
                machine: MachineId::new(7).unwrap(),
                classification: Some(PressureRecoveryClassification {
                    block: SelectedBlockId(1),
                    point: LiveRangePoint(2),
                    victim: VirtualRegisterId(3),
                    role: RecoveryVictimRole::ActiveResident {
                        current_view: RegisterViewId(4),
                        reclaimed_view: RegisterViewId(5),
                    },
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                    class: RegisterClassId(6),
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(7),
                        source_value: value,
                    },
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: BlockId::new(10).unwrap(),
                        node: 11,
                    }),
                    classification:
                        RecoveryClassification::ImmediateU64RematerializationCandidate {
                            defining_instruction: SelectedInstructionId(7),
                            source_value: value,
                            value: semantic_vocabulary::IntegerValue::Unsigned(12),
                            provenance: SelectedInstructionProvenance {
                                operations: vec![operation],
                                values: vec![value],
                                edges: Vec::new(),
                                obligations: Vec::new(),
                                work: vec![WorkSettlement {
                                    site: PsiProvenance::Operation(operation),
                                    units: 13,
                                }],
                            },
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(1),
                                point: LiveRangePoint(14),
                                instruction: SelectedInstructionId(15),
                                operand: 0,
                            }],
                        },
                }),
            }],
        }
    }

    #[test]
    fn classification_scalar_identity_keeps_fixed_bytes_and_scopes_rigid_widths() {
        use semantic_vocabulary::{IntegerSign, IntegerType, WidthParameter};
        let encode = |integer| {
            let mut bytes = Vec::new();
            super::encode_classification_scalar_type(&mut bytes, ScalarType::Integer(integer));
            bytes
        };
        assert_eq!(
            encode(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            [2, 64, 0]
        );
        assert_eq!(encode(IntegerType::address(64).unwrap()), [3, 64, 0]);
        let parameter = |owner, value| {
            IntegerType::parameterized(
                IntegerSign::Unsigned,
                WidthParameter {
                    owner: MachineId::new(owner).unwrap(),
                    value: ValueId::new(value).unwrap(),
                },
            )
        };
        let first = encode(parameter(1, 2));
        assert_ne!(first, encode(parameter(3, 2)));
        assert_ne!(first, encode(parameter(1, 4)));
        assert_ne!(
            first,
            encode(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
        );
        assert_eq!(first, encode(parameter(1, 2)));
    }

    #[test]
    fn classification_identity_is_deterministic_and_binds_roots_and_nested_classification() {
        let baseline = recovery_classification_identity(&classification_plan());
        assert_eq!(
            baseline,
            recovery_classification_identity(&classification_plan())
        );

        let mut changed = classification_plan();
        changed.spill_choices = SpillChoiceIdentity::from_bytes([20; 32]);
        assert_ne!(baseline, recovery_classification_identity(&changed));

        let mut changed = classification_plan();
        changed.allocator_availability = crate::AllocatorAvailabilityIdentity::from_bytes([21; 32]);
        assert_ne!(baseline, recovery_classification_identity(&changed));

        let mut changed = classification_plan();
        changed.usage.validation_steps += 1;
        assert_ne!(baseline, recovery_classification_identity(&changed));

        let mut changed = classification_plan();
        let Some(row) = &mut changed.functions[0].classification else {
            unreachable!()
        };
        let RecoveryClassification::ImmediateU64RematerializationCandidate {
            provenance,
            future_uses,
            ..
        } = &mut row.classification
        else {
            unreachable!()
        };
        provenance.work[0].units += 1;
        future_uses[0].point.0 += 1;
        assert_ne!(baseline, recovery_classification_identity(&changed));
    }

    #[test]
    fn scalar_abi_address_recovery_identity_retains_its_distinct_origin() {
        let mut source = classification_plan();
        let value = ValueId::new(17).unwrap();
        let row = source.functions[0].classification.as_mut().unwrap();
        row.origin = VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: SelectedInstructionId(3),
            source_value: value,
        };
        row.classification = RecoveryClassification::NoAdmittedRecovery {
            reason: NoAdmittedRecoveryReason::UnsupportedRangeShape,
        };
        let identity = recovery_classification_identity(&source);
        for origin in [
            VirtualRegisterOrigin::ScalarAbiAddress {
                instruction: SelectedInstructionId(4),
                source_value: value,
            },
            VirtualRegisterOrigin::ScalarAbiAddress {
                instruction: SelectedInstructionId(3),
                source_value: ValueId::new(18).unwrap(),
            },
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(3),
                source_value: value,
            },
        ] {
            let mut changed = source.clone();
            changed.functions[0].classification.as_mut().unwrap().origin = origin;
            assert_ne!(recovery_classification_identity(&changed), identity);
        }
    }

    #[test]
    fn spill_choice_identity_binds_roots_policy_work_and_functions() {
        let plan = SpillChoicePlan {
            legality: AllocationLegalityIdentity::from_bytes([1; 32]),
            ranges: LiveRangeIdentity::from_bytes([2; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            allocator_availability: crate::AllocatorAvailabilityIdentity::from_bytes([5; 32]),
            policy: SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            budget: OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 0,
                validation_steps: 1,
                commits: 0,
                iterations: 1,
            },
            functions: Vec::new(),
        };
        let baseline = spill_choice_identity(&plan);
        let mut changed = plan.clone();
        changed.usage.validation_steps += 1;
        assert_ne!(baseline, spill_choice_identity(&changed));
        changed = plan.clone();
        changed.ranges = LiveRangeIdentity::from_bytes([4; 32]);
        assert_ne!(baseline, spill_choice_identity(&changed));
        changed = plan.clone();
        changed.allocator_availability = crate::AllocatorAvailabilityIdentity::from_bytes([6; 32]);
        assert_ne!(baseline, spill_choice_identity(&changed));
    }

    type FixedViewCopyMutation = fn(&mut FixedViewCopyPlan);

    fn fixed_view_copy_plan() -> FixedViewCopyPlan {
        FixedViewCopyPlan {
            source_selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"s"),
            source_ranges: LiveRangeIdentity::from_bytes([2; 32]),
            source_legality: AllocationLegalityIdentity::from_bytes([3; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            allocator_availability: crate::AllocatorAvailabilityIdentity::from_bytes([5; 32]),
            source_evidence: FixedViewCopySourceEvidence {
                fixed_intervals: crate::FixedPrecoloredIntervalPlanIdentity::from_bytes([6; 32]),
                split_requirements: crate::FixedPrecoloredSplitRequirementPlanIdentity::from_bytes(
                    [7; 32],
                ),
                segment_homes: crate::FixedPrecoloredSegmentHomePlanIdentity::from_bytes([8; 32]),
            },
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
    fn fixed_view_copy_identity_binds_roots_work_copy_rows_and_transformed_plan() {
        let baseline = fixed_view_copy_identity(&fixed_view_copy_plan());
        assert_eq!(baseline, fixed_view_copy_identity(&fixed_view_copy_plan()));
        let mutations: Vec<FixedViewCopyMutation> = vec![
            |plan| {
                plan.source_selected =
                    SelectedInstructionPlanIdentity::from_canonical_bytes(b"changed")
            },
            |plan| plan.source_ranges = LiveRangeIdentity::from_bytes([13; 32]),
            |plan| plan.source_legality = AllocationLegalityIdentity::from_bytes([14; 32]),
            |plan| {
                plan.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([15; 32])
            },
            |plan| {
                plan.allocator_availability =
                    crate::AllocatorAvailabilityIdentity::from_bytes([16; 32])
            },
            |plan| {
                plan.source_evidence.fixed_intervals =
                    crate::FixedPrecoloredIntervalPlanIdentity::from_bytes([17; 32])
            },
            |plan| {
                plan.source_evidence.split_requirements =
                    crate::FixedPrecoloredSplitRequirementPlanIdentity::from_bytes([18; 32])
            },
            |plan| {
                plan.source_evidence.segment_homes =
                    crate::FixedPrecoloredSegmentHomePlanIdentity::from_bytes([19; 32])
            },
            |plan| plan.budget = OptimizationWorkBudget::new(11, 10, 10, 10, 10).unwrap(),
            |plan| plan.usage.commits += 1,
            |plan| plan.copies[0].function += 1,
            |plan| plan.copies[0].machine = MachineId::new(2).unwrap(),
            |plan| plan.copies[0].source_virtual_register.0 += 1,
            |plan| plan.copies[0].source_value = ValueId::new(3).unwrap(),
            |plan| {
                plan.copies[0].source_definition_site = ValueDefinitionSite::FunctionParameter(2)
            },
            |plan| plan.copies[0].from_view.0 += 1,
            |plan| plan.copies[0].to_view.0 += 1,
            |plan| plan.policy = FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            |plan| plan.copies[0].destinations[0].site = VirtualFixedConstraintSite::Entry,
            |plan| plan.copies[0].destinations[0].view.0 += 1,
            |plan| plan.copies[0].destinations[0].block.0 += 1,
            |plan| plan.copies[0].destinations.swap(0, 1),
            |plan| plan.copies[0].insertion_block.0 += 1,
            |plan| plan.copies[0].before_instruction.0 += 1,
            |plan| plan.copies[0].copy_instruction.0 += 1,
            |plan| plan.copies[0].result_virtual_register.0 += 1,
            |plan| plan.copies[0].copy_constraint.variant += 1,
            |plan| plan.copies.clear(),
            |plan| plan.copies[0].destinations.clear(),
            |plan| {
                std::sync::Arc::make_mut(&mut plan.transformed).entry = MachineId::new(2).unwrap()
            },
        ];
        for mutate in mutations {
            let mut changed = fixed_view_copy_plan();
            mutate(&mut changed);
            assert_ne!(baseline, fixed_view_copy_identity(&changed));
        }
    }
}
