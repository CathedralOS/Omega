/*
  Register-pressure recovery eligibility classification
  (`classify_pressure_recovery`, `ValidatedRecoveryClassifications`).

  Classifies spill-victim candidates across functions for immediate
  rematerialization or frame-slot spill eligibility under bounded work
  accounting.
*/

use crate::SelectedProgramView;
use crate::ValidatedAllocationLegality;
use crate::ValidatedLiveRanges;
use crate::ValidatedSpillChoices;
use register_homes::RecoveryClassificationPolicy;

pub mod compute {

    mod function_classification {

        use crate::RecoveryClassificationError;
        use register_homes::{
            FunctionRecoveryClassification, PressureRecoveryClassification, RecoveryVictimRole,
        };

        pub(super) fn classify(
            function: usize,
            selected: &selected_instructions::SelectedFunction,
            ranges: &selected_instructions::FunctionLiveRanges,
            legality: &register_homes::FunctionAllocationLegality,
            choices: &register_homes::FunctionSpillChoices,
        ) -> Result<FunctionRecoveryClassification, RecoveryClassificationError> {
            if selected.machine != ranges.machine
                || selected.machine != legality.machine
                || selected.machine != choices.machine
                || selected.virtual_registers.len() != ranges.virtual_registers.len()
                || selected.virtual_registers.len() != legality.virtual_registers.len()
            {
                return Err(RecoveryClassificationError::FunctionMismatch { function });
            }
            let Some(choice) = &choices.choice else {
                return Ok(FunctionRecoveryClassification {
                    machine: selected.machine,
                    classification: None,
                });
            };
            let victim = selected.virtual_register(choice.selected_victim).ok_or(
                RecoveryClassificationError::VictimMismatch {
                    function,
                    register: choice.selected_victim.0,
                },
            )?;
            let range = ranges
                .virtual_registers
                .iter()
                .find(|range| range.virtual_register == victim.id && range.class == victim.class)
                .ok_or(RecoveryClassificationError::VictimMismatch {
                    function,
                    register: victim.id.0,
                })?;
            if !legality
                .virtual_registers
                .iter()
                .any(|row| row.virtual_register == victim.id && row.class == victim.class)
            {
                return Err(RecoveryClassificationError::VictimMismatch {
                    function,
                    register: victim.id.0,
                });
            }
            let role = victim_role(function, choice)?;
            let classification = super::immediate_eligibility::classify(
                function, selected, ranges, choice, victim, range,
            )?;
            Ok(FunctionRecoveryClassification {
                machine: selected.machine,
                classification: Some(PressureRecoveryClassification {
                    block: choice.block,
                    point: choice.point,
                    victim: victim.id,
                    role,
                    scalar_type: victim.scalar_type,
                    class: victim.class,
                    origin: victim.origin,
                    definition_site: victim.definition_site,
                    classification,
                }),
            })
        }

        fn victim_role(
            function: usize,
            choice: &register_homes::SpillChoice,
        ) -> Result<RecoveryVictimRole, RecoveryClassificationError> {
            if choice.selected_victim == choice.incoming {
                let contender = choice
                    .contenders
                    .iter()
                    .find(|row| row.virtual_register == choice.selected_victim)
                    .ok_or(RecoveryClassificationError::ChoiceMismatch { function })?;
                if contender.reclaimed_view.is_some() {
                    return Err(RecoveryClassificationError::ChoiceMismatch { function });
                }
                return Ok(RecoveryVictimRole::Incoming);
            }
            let resident = choice
                .active_residents
                .iter()
                .find(|row| row.virtual_register == choice.selected_victim)
                .ok_or(RecoveryClassificationError::ChoiceMismatch { function })?;
            let reclaimed_view = choice
                .contenders
                .iter()
                .find(|row| row.virtual_register == choice.selected_victim)
                .and_then(|row| row.reclaimed_view)
                .ok_or(RecoveryClassificationError::ChoiceMismatch { function })?;
            Ok(RecoveryVictimRole::ActiveResident {
                current_view: resident.view,
                reclaimed_view,
            })
        }
    }
    mod immediate_eligibility {

        use register_model::RegisterOperandAccess;
        use selected_instructions::{
            SelectedInstruction, SelectedInstructionKind, VirtualRegisterId, VirtualRegisterOrigin,
        };
        use semantic_vocabulary::{IntegerSign, ScalarType};

        use crate::RecoveryClassificationError;
        use register_homes::{NoAdmittedRecoveryReason, RecoveryClassification, RecoveryFutureUse};
        use selected_instructions::VirtualFixedConstraintSite;

        pub(super) fn classify(
            function: usize,
            selected: &selected_instructions::SelectedFunction,
            ranges: &selected_instructions::FunctionLiveRanges,
            choice: &register_homes::SpillChoice,
            victim: &selected_instructions::VirtualRegister,
            range: &selected_instructions::VirtualLiveRange,
        ) -> Result<RecoveryClassification, RecoveryClassificationError> {
            if !is_fixed_unsigned_u64(victim.scalar_type) {
                return no_recovery(NoAdmittedRecoveryReason::UnsupportedScalarType);
            }
            let (defining_id, source_value) = match victim.origin {
                VirtualRegisterOrigin::BlockParameter { .. }
                | VirtualRegisterOrigin::SpillAddress { .. }
                | VirtualRegisterOrigin::StructuralObservation { .. }
                | VirtualRegisterOrigin::ScalarAbiAddress { .. }
                | VirtualRegisterOrigin::InstructionScratch { .. }
                | VirtualRegisterOrigin::AbiTransport { .. } => {
                    return no_recovery(NoAdmittedRecoveryReason::UnsupportedRangeShape);
                }
                VirtualRegisterOrigin::EntryParameter { .. }
                | VirtualRegisterOrigin::StructuralParameter { .. } => {
                    return no_recovery(NoAdmittedRecoveryReason::EntryParameter);
                }
                VirtualRegisterOrigin::InstructionResult {
                    instruction,
                    source_value,
                } => (instruction, source_value),
            };
            if crate::liveness::edge_values::has_edge_use(selected, victim.id)
                || range.fragments.len() != 1
                || range.fragments[0].block != choice.block
                || !range.edge_connectors.is_empty()
            {
                return no_recovery(NoAdmittedRecoveryReason::UnsupportedRangeShape);
            }
            if range.fixed_constraints.iter().any(|fixed| {
                matches!(
                    fixed.site,
                    VirtualFixedConstraintSite::Operand { point, .. } if point >= choice.point
                )
            }) {
                return no_recovery(NoAdmittedRecoveryReason::FutureFixedUse);
            }
            let defining = unique_definition(function, selected, victim.id, defining_id)?;
            match defining.kind {
                SelectedInstructionKind::ExactAddI64 { .. }
                | SelectedInstructionKind::ExactAddI64Immediate { .. }
                | SelectedInstructionKind::ExactSubtractI64 { .. }
                | SelectedInstructionKind::ExactMultiplyI64 { .. }
                | SelectedInstructionKind::ExactRemainderU64 { .. }
                | SelectedInstructionKind::WrappingDivideI64 { .. }
                | SelectedInstructionKind::ExactDivideI64 { .. }
                | SelectedInstructionKind::ExactRemainderI64 { .. } => {
                    return no_recovery(NoAdmittedRecoveryReason::ProofBearingDefinition);
                }
                SelectedInstructionKind::MaterializeI64 { .. } => {}
                _ => return no_recovery(NoAdmittedRecoveryReason::NonMaterializeDefinition),
            }
            let SelectedInstructionKind::MaterializeI64 { value } = defining.kind else {
                unreachable!("materialize kind established above")
            };
            if defining.operands.len() != 1
                || defining.operands[0].virtual_register != victim.id
                || defining.operands[0].access != RegisterOperandAccess::Def
                || defining.provenance.values.as_slice() != [source_value]
                || defining.provenance.operations.len() != 1
                || !defining.provenance.edges.is_empty()
                || !defining.provenance.obligations.is_empty()
                || defining.provenance.work.is_empty()
                || !defining.provenance.work.iter().all(|work| {
                    work.site
                        == optimization_unit::PsiProvenance::Operation(
                            defining.provenance.operations[0],
                        )
                })
                || !matches!(victim.scalar_type, ScalarType::Integer(integer) if integer.admits(value))
            {
                return Err(RecoveryClassificationError::VictimMismatch {
                    function,
                    register: victim.id.0,
                });
            }
            let future_uses = future_uses(function, selected, ranges, choice, victim.id, range)?;
            if future_uses.is_empty() {
                return no_recovery(NoAdmittedRecoveryReason::NoFutureUse);
            }
            Ok(
                RecoveryClassification::ImmediateU64RematerializationCandidate {
                    defining_instruction: defining.id,
                    source_value,
                    value,
                    provenance: defining.provenance.clone(),
                    future_uses,
                },
            )
        }

        fn unique_definition(
            function: usize,
            selected: &selected_instructions::SelectedFunction,
            victim: VirtualRegisterId,
            expected: selected_instructions::SelectedInstructionId,
        ) -> Result<&SelectedInstruction, RecoveryClassificationError> {
            let mut definitions = Vec::new();
            for block in &selected.blocks {
                for instruction in block_instructions(block) {
                    if instruction.operands.iter().any(|operand| {
                        operand.virtual_register == victim
                            && matches!(
                                operand.access,
                                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                            )
                    }) {
                        definitions.push(instruction);
                    }
                }
            }
            if definitions.len() != 1 || definitions[0].id != expected {
                return Err(RecoveryClassificationError::VictimMismatch {
                    function,
                    register: victim.0,
                });
            }
            Ok(definitions[0])
        }

        fn future_uses(
            function: usize,
            selected: &selected_instructions::SelectedFunction,
            ranges: &selected_instructions::FunctionLiveRanges,
            choice: &register_homes::SpillChoice,
            victim: VirtualRegisterId,
            range: &selected_instructions::VirtualLiveRange,
        ) -> Result<Vec<RecoveryFutureUse>, RecoveryClassificationError> {
            let mut uses = Vec::new();
            for occurrence in &range.occurrences {
                if occurrence.point < choice.point
                    || occurrence.access != RegisterOperandAccess::Use
                {
                    continue;
                }
                let block = ranges
                    .block_domains
                    .iter()
                    .find(|domain| {
                        domain.start <= occurrence.point && occurrence.point < domain.end
                    })
                    .ok_or(RecoveryClassificationError::VictimMismatch {
                        function,
                        register: victim.0,
                    })?;
                if block.block != choice.block
                    || !selected.blocks.iter().any(|candidate| {
                        candidate.id == block.block
                            && block_instructions(candidate)
                                .into_iter()
                                .any(|instruction| {
                                    instruction.id == occurrence.instruction
                                        && instruction.operands.iter().any(|operand| {
                                            operand.operand == occurrence.operand
                                                && operand.virtual_register == victim
                                                && operand.access == RegisterOperandAccess::Use
                                                && operand.fixed_view.is_none()
                                        })
                                })
                    })
                {
                    return Err(RecoveryClassificationError::VictimMismatch {
                        function,
                        register: victim.0,
                    });
                }
                uses.push(RecoveryFutureUse {
                    block: block.block,
                    point: occurrence.point,
                    instruction: occurrence.instruction,
                    operand: occurrence.operand,
                });
            }
            uses.sort_unstable();
            uses.dedup();
            Ok(uses)
        }

        fn block_instructions(
            block: &selected_instructions::SelectedBlock,
        ) -> Vec<&SelectedInstruction> {
            let terminator = selected_instructions::terminator_instruction(&block.terminator);
            block
                .instructions
                .iter()
                .chain(std::iter::once(terminator))
                .collect()
        }

        fn is_fixed_unsigned_u64(scalar: ScalarType) -> bool {
            matches!(
                scalar,
                ScalarType::Integer(integer)
                    if !integer.is_address()
                        && integer.sign() == IntegerSign::Unsigned
                        && integer.bits() == 64
            )
        }

        fn no_recovery(
            reason: NoAdmittedRecoveryReason,
        ) -> Result<RecoveryClassification, RecoveryClassificationError> {
            Ok(RecoveryClassification::NoAdmittedRecovery { reason })
        }
    }
    mod work_usage {

        use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

        use crate::work_accounting::Work;
        use crate::{
            RecoveryClassificationError, SelectedProgramView, ValidatedLiveRanges,
            ValidatedSpillChoices,
        };

        pub(super) fn required(
            selected: &impl SelectedProgramView,
            ranges: &ValidatedLiveRanges,
            spill_choices: &ValidatedSpillChoices,
            budget: OptimizationWorkBudget,
        ) -> Result<OptimizationWorkUsage, RecoveryClassificationError> {
            let mut work = Work::new(OptimizationWorkUsage {
                iterations: 1,
                ..Default::default()
            });
            for ((selected, ranges), choices) in selected
                .selected_plan()
                .functions
                .iter()
                .zip(&ranges.plan().functions)
                .zip(&spill_choices.plan().functions)
            {
                work.rule_evaluations(1)?;
                work.validation_steps(selected.virtual_registers.len() as u64)?;
                let instruction_count = selected
                    .blocks
                    .iter()
                    .map(|block| block.instructions.len() as u64 + 1)
                    .sum::<u64>();
                work.validation_steps(instruction_count)?;
                work.validation_steps(ranges.virtual_registers.len() as u64)?;
                if choices.choice.is_some() {
                    work.candidates(1)?;
                    work.commits(1)?;
                }
            }
            Ok(work.finish(budget)?)
        }
    }

    use optimization_core::OptimizationWorkBudget;

    use crate::{
        RecoveryClassificationError, SelectedProgramView, ValidatedAllocationLegality,
        ValidatedLiveRanges, ValidatedSpillChoices,
    };
    use register_homes::{RecoveryClassificationPlan, RecoveryClassificationPolicy};

    pub(crate) fn compute_terminal_recovery_classifications<S: SelectedProgramView>(
        selected: &S,
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
        spill_choices: &ValidatedSpillChoices,
        policy: RecoveryClassificationPolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<RecoveryClassificationPlan, RecoveryClassificationError> {
        validate_roots(selected, ranges, legality, spill_choices)?;
        if policy != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 {
            return Err(RecoveryClassificationError::UnsupportedPolicy);
        }
        let usage = work_usage::required(selected, ranges, spill_choices, budget)?;
        let functions = selected
            .selected_plan()
            .functions
            .iter()
            .zip(&ranges.plan().functions)
            .zip(&legality.plan().functions)
            .zip(&spill_choices.plan().functions)
            .enumerate()
            .map(|(function, (((selected, ranges), legality), choices))| {
                function_classification::classify(function, selected, ranges, legality, choices)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RecoveryClassificationPlan {
            selected: selected.selected_identity(),
            spill_choices: spill_choices.receipt().identity(),
            ranges: ranges.receipt().identity(),
            legality: legality.receipt().identity(),
            register_environment: legality.receipt().register_environment(),
            allocator_availability: legality.receipt().allocator_availability(),
            optimization_unit: selected.optimization_unit_identity(),
            work_schedule: selected.work_schedule_identity(),
            policy,
            budget,
            usage,
            functions,
        })
    }

    fn validate_roots(
        selected: &impl SelectedProgramView,
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
        spill_choices: &ValidatedSpillChoices,
    ) -> Result<(), RecoveryClassificationError> {
        if ranges.receipt().selected() != selected.selected_identity()
            || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
            || ranges.receipt().work_schedule() != selected.work_schedule_identity()
            || legality.receipt().ranges() != ranges.receipt().identity()
            || spill_choices.receipt().ranges() != ranges.receipt().identity()
            || spill_choices.receipt().legality() != legality.receipt().identity()
            || spill_choices.receipt().register_environment()
                != legality.receipt().register_environment()
            || selected.selected_plan().functions.len() != ranges.plan().functions.len()
            || selected.selected_plan().functions.len() != legality.plan().functions.len()
            || selected.selected_plan().functions.len() != spill_choices.plan().functions.len()
        {
            return Err(RecoveryClassificationError::RootMismatch);
        }
        Ok(())
    }
}

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, SpillChoiceIdentity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{LiveRangeIdentity, SelectedInstructionPlanIdentity};
use semantic_vocabulary::WorkScheduleIdentity;

use crate::WorkDefect;

pub fn classify_pressure_recovery<S: SelectedProgramView>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    policy: RecoveryClassificationPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedRecoveryClassifications, RecoveryClassificationError> {
    let plan = compute::compute_terminal_recovery_classifications(
        selected,
        ranges,
        legality,
        spill_choices,
        policy,
        budget,
    )?;
    seal_recovery_classifications(plan)
}

seal_plan!(
    seal_recovery_classifications(RecoveryClassificationPlan)
        -> ValidatedRecoveryClassifications: RecoveryClassificationValidationReceipt
        = register_homes::recovery_classification_identity, RecoveryClassificationError
    {
        selected,
        spill_choices,
        ranges,
        legality,
        register_environment,
        allocator_availability,
        optimization_unit,
        work_schedule,
        policy,
        usage
    }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryClassificationValidationReceipt {
    pub(crate) identity: RecoveryClassificationIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) spill_choices: SpillChoiceIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) work_schedule: WorkScheduleIdentity,
    pub(crate) policy: RecoveryClassificationPolicy,
    pub(crate) usage: OptimizationWorkUsage,
}

impl RecoveryClassificationValidationReceipt {
    receipt_accessor_rows!(RecoveryClassificationValidationReceipt {
        identity: RecoveryClassificationIdentity,
        selected: SelectedInstructionPlanIdentity,
        spill_choices: SpillChoiceIdentity,
        policy: RecoveryClassificationPolicy,
        usage: OptimizationWorkUsage,
    });
    shared_analysis_identity_accessors!(pub);
}

validated_plan_carrier!(
    ValidatedRecoveryClassifications: RecoveryClassificationPlan,
    RecoveryClassificationValidationReceipt
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryClassificationError {
    RootMismatch,
    UnsupportedPolicy,
    Work(WorkDefect),
    FunctionMismatch { function: usize },
    ChoiceMismatch { function: usize },
    VictimMismatch { function: usize, register: u32 },
    ClassificationMismatch { function: usize },
    UsageMismatch,
}

failed_display!(
    RecoveryClassificationError,
    "Terminal recovery classification failed"
);
