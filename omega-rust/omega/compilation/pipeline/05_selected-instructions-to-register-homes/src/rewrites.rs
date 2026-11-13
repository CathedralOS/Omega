/*
We coordinate allocation-recovery rewrites over the selected CFG: staging
active-resident rematerialization (`rematerialization`), materializing
operation-local fixed occurrences (`fixed_occurrences`), staging precolored
segment homes and fixed-view copies (`fixed_view`), and shifting settlement
indices across inserted instructions (`settlement_gaps`).
*/

pub(crate) mod rematerialization {
    pub(crate) mod compute {
        use crate::assignment::post_allocation_manifest::project_post_allocation_optimization_manifest;
        use crate::{PressureRematerializationPolicy, rematerialize_selected_active_resident};
        use optimization_core::OptimizationWorkBudget;
        use register_homes::{
            PostAllocationSelectedTransformation, RecoveryClassificationPolicy, SpillChoicePolicy,
        };
        use selected_instruction_analyses::{
            analyze_allocation_legality, analyze_live_ranges, analyze_liveness,
            choose_spill_victims, classify_pressure_recovery,
        };

        use selected_instruction_analyses::StagedOptimizedAllocationLegality;

        use super::custody::{custody_receipt, pressure_custody_receipt};
        use super::{
            OptimizedActiveResidentRematerializationError,
            StagedOptimizedActiveResidentRematerialization,
            StagedOptimizedActiveResidentRematerializationPressure,
        };

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn stage_optimized_active_resident_rematerialization_pressure(
            source: StagedOptimizedAllocationLegality,
            choice_policy: SpillChoicePolicy,
            classification_policy: RecoveryClassificationPolicy,
            rematerialization_policy: PressureRematerializationPolicy,
            budget: OptimizationWorkBudget,
        ) -> Result<
            StagedOptimizedActiveResidentRematerializationPressure,
            OptimizedActiveResidentRematerializationError,
        > {
            if choice_policy != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1
                || classification_policy
                    != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1
                || rematerialization_policy
                    != PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1
            {
                return Err(OptimizedActiveResidentRematerializationError::UnsupportedPolicy);
            }
            let source_receipt = source.custody();
            let environment = source.register_environment();
            let selected = source.selected();
            let source_ranges = source.ranges();
            let choices = choose_spill_victims(
                source.legality(),
                source_ranges,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
                choice_policy,
                budget,
            )
            .map_err(OptimizedActiveResidentRematerializationError::SpillChoice)?;
            let classifications = classify_pressure_recovery(
                selected,
                source_ranges,
                source.legality(),
                &choices,
                classification_policy,
                budget,
            )
            .map_err(OptimizedActiveResidentRematerializationError::Classification)?;
            let rematerialization = rematerialize_selected_active_resident(
                selected,
                source_ranges,
                source.legality(),
                &choices,
                &classifications,
                source.allocator_availability(),
                environment.identity(),
                environment.constraints(),
                &environment.allocation_constraint_keys(),
                rematerialization_policy,
                budget,
            )
            .map_err(OptimizedActiveResidentRematerializationError::Rematerialization)?;
            if rematerialization.receipt().applied_count() == 0 {
                return Err(OptimizedActiveResidentRematerializationError::NoAppliedAction);
            }

            let liveness = analyze_liveness(&rematerialization)
                .map_err(OptimizedActiveResidentRematerializationError::Liveness)?;
            let ranges = analyze_live_ranges(&rematerialization, &liveness)
                .map_err(OptimizedActiveResidentRematerializationError::Ranges)?;
            let legality = analyze_allocation_legality(
                &ranges,
                source.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
            )
            .map_err(OptimizedActiveResidentRematerializationError::Legality)?;
            if legality.receipt().entry_transition_count() != 0 {
                return Err(
                    OptimizedActiveResidentRematerializationError::RemainingTransitions {
                        count: legality.receipt().entry_transition_count(),
                    },
                );
            }
            let custody = pressure_custody_receipt(
                source_receipt,
                &choices,
                &classifications,
                &rematerialization,
                &liveness,
                &ranges,
                &legality,
            );
            Ok(StagedOptimizedActiveResidentRematerializationPressure {
                source,
                rematerialization,
                liveness,
                ranges,
                legality,
                custody,
            })
        }

        pub(crate) fn complete_optimized_active_resident_rematerialization(
            pressure: StagedOptimizedActiveResidentRematerializationPressure,
            homes: crate::ValidatedRegisterHomes,
        ) -> Result<
            StagedOptimizedActiveResidentRematerialization,
            OptimizedActiveResidentRematerializationError,
        > {
            let StagedOptimizedActiveResidentRematerializationPressure {
                source,
                rematerialization,
                liveness,
                ranges,
                legality,
                custody: prefix_custody,
            } = pressure;
            let manifest = project_post_allocation_optimization_manifest(
                prefix_custody.source().manifest(),
                None,
                None,
                &[
                    PostAllocationSelectedTransformation::PressureRematerialization(
                        rematerialization.receipt().identity(),
                    ),
                ],
                &ranges,
                &legality,
                &homes,
            )
            .map_err(OptimizedActiveResidentRematerializationError::Manifest)?;
            let custody = custody_receipt(prefix_custody, &homes, &manifest);
            Ok(StagedOptimizedActiveResidentRematerialization {
                source,
                rematerialization,
                liveness,
                ranges,
                legality,
                homes,
                manifest,
                custody,
            })
        }
    }
    pub(crate) mod custody {
        use crate::ValidatedPressureRematerialization;
        use crate::{ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes};
        use selected_instruction_analyses::{
            ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
            ValidatedRecoveryClassifications, ValidatedSpillChoices,
        };

        use selected_instruction_analyses::StagedOptimizedAllocationLegalityCustodyReceipt;

        use super::{
            StagedOptimizedActiveResidentRematerializationCustodyReceipt,
            StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
        };

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn pressure_custody_receipt(
            source: StagedOptimizedAllocationLegalityCustodyReceipt,
            choices: &ValidatedSpillChoices,
            classifications: &ValidatedRecoveryClassifications,
            rematerialization: &ValidatedPressureRematerialization,
            liveness: &ValidatedLiveness,
            ranges: &ValidatedLiveRanges,
            legality: &ValidatedAllocationLegality,
        ) -> StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
            StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
                source,
                choices: choices.receipt().identity(),
                choice_policy: choices.receipt().policy(),
                choice_usage: choices.receipt().usage(),
                classifications: classifications.receipt().identity(),
                classification_policy: classifications.receipt().policy(),
                classification_usage: classifications.receipt().usage(),
                rematerialization: rematerialization.receipt().identity(),
                rematerialization_policy: rematerialization.receipt().policy(),
                rematerialization_usage: rematerialization.receipt().usage(),
                budget: rematerialization.plan().budget,
                transformed_selected: rematerialization.receipt().transformed_selected(),
                liveness: liveness.receipt().identity(),
                ranges: ranges.receipt().identity(),
                legality: legality.receipt().identity(),
                function_count: rematerialization.receipt().function_count(),
                virtual_register_count: legality.receipt().virtual_register_count(),
                applied_count: rematerialization.receipt().applied_count(),
                rewritten_use_count: rematerialization.receipt().rewritten_use_count(),
            }
        }

        pub(crate) fn custody_receipt(
            prefix: StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
            homes: &ValidatedRegisterHomes,
            manifest: &ValidatedPostAllocationOptimizationManifest,
        ) -> StagedOptimizedActiveResidentRematerializationCustodyReceipt {
            StagedOptimizedActiveResidentRematerializationCustodyReceipt {
                prefix,
                homes: homes.receipt().identity(),
                manifest: manifest.record().identity,
                assignment_count: homes.receipt().assignment_count(),
            }
        }
    }

    use optimization_core::OptimizationWorkBudget;

    use crate::{
        PressureRematerializationError, PressureRematerializationPolicy,
        ValidatedPressureRematerialization,
    };
    use crate::{
        RegisterHomeError, ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
    };
    use register_homes::{
        AllocationLegalityIdentity, PostAllocationOptimizationManifestError,
        RecoveryClassificationIdentity, RecoveryClassificationPolicy, SpillChoiceIdentity,
        SpillChoicePolicy,
    };
    use selected_instruction_analyses::{
        AllocationLegalityError, LiveRangeError, LivenessError, RecoveryClassificationError,
        SpillChoiceError, StagedOptimizedAllocationLegality,
        StagedOptimizedAllocationLegalityCustodyReceipt, ValidatedAllocationLegality,
        ValidatedLiveRanges, ValidatedLiveness,
    };
    use selected_instructions::{
        LiveRangeIdentity, LivenessIdentity, PressureRematerializationIdentity,
    };

    pub(crate) use compute::{
        complete_optimized_active_resident_rematerialization,
        stage_optimized_active_resident_rematerialization_pressure,
    };

    #[derive(Debug)]
    pub(crate) struct StagedOptimizedActiveResidentRematerialization {
        source: StagedOptimizedAllocationLegality,
        rematerialization: ValidatedPressureRematerialization,
        liveness: ValidatedLiveness,
        ranges: ValidatedLiveRanges,
        legality: ValidatedAllocationLegality,
        homes: ValidatedRegisterHomes,
        manifest: ValidatedPostAllocationOptimizationManifest,
        custody: StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    }

    impl StagedOptimizedActiveResidentRematerialization {
        admitted_stage_accessors!(const pub, source);
        receipt_accessor_rows!(borrow StagedOptimizedActiveResidentRematerialization {
            rematerialization: &ValidatedPressureRematerialization,
            liveness: &ValidatedLiveness,
            ranges: &ValidatedLiveRanges,
            legality: &ValidatedAllocationLegality,
            homes: &ValidatedRegisterHomes,
        });
        pub const fn post_allocation_manifest(
            &self,
        ) -> &ValidatedPostAllocationOptimizationManifest {
            &self.manifest
        }
        receipt_accessor_rows!(ref StagedOptimizedActiveResidentRematerialization {
            custody: StagedOptimizedActiveResidentRematerializationCustodyReceipt,
        });
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StagedOptimizedActiveResidentRematerializationCustodyReceipt {
        prefix: StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
        homes: crate::RegisterHomeIdentity,
        manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
        assignment_count: usize,
    }

    impl StagedOptimizedActiveResidentRematerializationCustodyReceipt {
        pub const fn rematerialization(&self) -> PressureRematerializationIdentity {
            self.prefix.rematerialization
        }

        receipt_accessor_rows!(
            StagedOptimizedActiveResidentRematerializationCustodyReceipt {
                homes: crate::RegisterHomeIdentity,
                manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
                assignment_count: usize,
            }
        );
    }

    #[derive(Debug)]
    pub(crate) struct StagedOptimizedActiveResidentRematerializationPressure {
        source: StagedOptimizedAllocationLegality,
        rematerialization: ValidatedPressureRematerialization,
        liveness: ValidatedLiveness,
        ranges: ValidatedLiveRanges,
        legality: ValidatedAllocationLegality,
        custody: StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
    }

    impl StagedOptimizedActiveResidentRematerializationPressure {
        receipt_accessor_rows!(borrow StagedOptimizedActiveResidentRematerializationPressure {
            source: &StagedOptimizedAllocationLegality,
            rematerialization: &ValidatedPressureRematerialization,
            liveness: &ValidatedLiveness,
            ranges: &ValidatedLiveRanges,
            legality: &ValidatedAllocationLegality,
        });
        receipt_accessor_rows!(ref StagedOptimizedActiveResidentRematerializationPressure {
            custody: StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
        });
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
        source: StagedOptimizedAllocationLegalityCustodyReceipt,
        choices: SpillChoiceIdentity,
        choice_policy: SpillChoicePolicy,
        choice_usage: optimization_core::OptimizationWorkUsage,
        classifications: RecoveryClassificationIdentity,
        classification_policy: RecoveryClassificationPolicy,
        classification_usage: optimization_core::OptimizationWorkUsage,
        rematerialization: PressureRematerializationIdentity,
        rematerialization_policy: PressureRematerializationPolicy,
        rematerialization_usage: optimization_core::OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
        transformed_selected: selected_instructions::SelectedInstructionPlanIdentity,
        liveness: LivenessIdentity,
        ranges: LiveRangeIdentity,
        legality: AllocationLegalityIdentity,
        function_count: usize,
        virtual_register_count: usize,
        applied_count: usize,
        rewritten_use_count: usize,
    }

    impl StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
        receipt_accessor_rows!(
            StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
                source: StagedOptimizedAllocationLegalityCustodyReceipt,
                rematerialization: PressureRematerializationIdentity,
            }
        );
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedActiveResidentRematerializationError {
        UnsupportedPolicy,
        SpillChoice(SpillChoiceError),
        Classification(RecoveryClassificationError),
        Rematerialization(PressureRematerializationError),
        NoAppliedAction,
        Liveness(LivenessError),
        Ranges(LiveRangeError),
        Legality(AllocationLegalityError),
        RemainingTransitions { count: usize },
        Homes(RegisterHomeError),
        Manifest(PostAllocationOptimizationManifestError),
        ReceiptMismatch,
    }

    stage_error_display!(
        OptimizedActiveResidentRematerializationError,
        "optimized active-resident rematerialization failed"
    );
}

pub(crate) use selected_instruction_analyses::rewriting::*;
pub(crate) use selected_instruction_analyses::sealed;

pub(crate) use crate::fixed_view_copy_recovery as allocation_recovery;
pub(crate) mod fixed_occurrences {
    use std::sync::Arc;

    use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
    use register_environment::ValidatedTargetRegisterEnvironment;
    use register_model::{RegisterClassId, RegisterInstructionConstraint, RegisterOperandAccess};
    use selected_instructions::{
        SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionPlan, SelectedInstructionPlanIdentity, VirtualRegister,
        VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{ScalarType, WorkScheduleIdentity};

    use crate::ValidatedSelectedAnalysis;
    use crate::rewrites::runtime_spill::control_mut;
    use crate::rewrites::selected_operands::copy_shaped;

    validated_transformed_receipt!(ValidatedFixedOccurrences, FixedOccurrencesReceipt);

    rewrite_receipt!(FixedOccurrencesReceipt);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FixedOccurrencesError {
        SourceMismatch,
        UnsupportedCarrier,
        ConstraintMismatch,
        CopyConstraintMismatch {
            register: VirtualRegisterId,
            class: RegisterClassId,
        },
        IdentityOverflow,
        WorkBudgetExceeded {
            function: usize,
            required: usize,
            limit: u64,
            registers: usize,
            operands: usize,
            transports: usize,
            tie_steps: usize,
            settlement_steps: usize,
        },
    }

    failed_display!(
        FixedOccurrencesError,
        "fixed-occurrence materialization failed"
    );

    pub fn materialize_selected_fixed_occurrences(
        source: &impl ValidatedSelectedAnalysis,
        environment: &ValidatedTargetRegisterEnvironment,
        budget: OptimizationWorkBudget,
    ) -> Result<ValidatedFixedOccurrences, FixedOccurrencesError> {
        if source.selected_plan().target != environment.target()
            || source.selected_plan().work_schedule != source.work_schedule_identity()
        {
            return Err(FixedOccurrencesError::SourceMismatch);
        }
        let row = environment
            .constraint(environment.selected_keys().copy_i64)
            .ok_or(FixedOccurrencesError::ConstraintMismatch)?;
        let mut transformed = source.selected_plan().clone();
        let function_count = transformed.functions.len();
        for function_index in 0..function_count {
            let function = &transformed.functions[function_index];
            let operand_count = function
                .blocks
                .total_operand_count()
                .ok_or(FixedOccurrencesError::IdentityOverflow)?;
            let transport_count = super::function_transports(function)
                .ok_or(FixedOccurrencesError::IdentityOverflow)?;
            let mut constrained_operand_bound = 0usize;
            let mut tie_steps = 0usize;
            for instruction in function.instructions().chain(
                function
                    .blocks
                    .iter()
                    .map(|block| selected_instructions::terminator_instruction(&block.terminator)),
            ) {
                let arity = instruction.operands.len();
                if instruction
                    .operands
                    .iter()
                    .any(|operand| operand.fixed_view.is_some())
                {
                    constrained_operand_bound = constrained_operand_bound
                        .checked_add(arity)
                        .ok_or(FixedOccurrencesError::IdentityOverflow)?;
                }
                let ties = instruction
                    .operands
                    .iter()
                    .filter(|operand| operand.tied_to.is_some())
                    .count();
                let step = arity
                    .checked_mul(2)
                    .and_then(|base| {
                        base.checked_add(
                            ties.checked_mul(arity)?
                                .checked_mul(arity.checked_add(1)?)?
                                .checked_mul(2)?,
                        )
                    })
                    .ok_or(FixedOccurrencesError::IdentityOverflow)?;
                tie_steps = tie_steps
                    .checked_add(step)
                    .ok_or(FixedOccurrencesError::IdentityOverflow)?;
            }
            let register_depth =
                usize::BITS as usize - constrained_operand_bound.max(1).leading_zeros() as usize;
            let settlement_steps = function
                .blocks
                .len()
                .checked_mul(function.boundary_settlements.len())
                .ok_or(FixedOccurrencesError::IdentityOverflow)?;
            let steps = super::function_scan(function)
                .and_then(|scan| scan.checked_mul(10))
                .and_then(|steps| {
                    steps.checked_add(operand_count.checked_mul(register_depth.checked_add(16)?)?)
                })
                .and_then(|steps| {
                    steps.checked_add(
                        transport_count
                            .checked_mul(register_depth.checked_mul(2)?.checked_add(4)?)?,
                    )
                })
                .and_then(|steps| {
                    steps.checked_add(
                        function
                            .virtual_registers
                            .len()
                            .checked_mul(register_depth.checked_add(4)?)?,
                    )
                })
                .and_then(|steps| {
                    steps.checked_add(
                        constrained_operand_bound
                            .checked_mul(register_depth.checked_mul(4)?.checked_add(12)?)?,
                    )
                })
                .and_then(|steps| steps.checked_add(tie_steps))
                .and_then(|steps| steps.checked_add(settlement_steps))
                .and_then(|steps| steps.checked_add(function_count))
                .ok_or(FixedOccurrencesError::IdentityOverflow)?;
            let required_budget =
                super::required_rewrite_budget(source.selected_plan(), function, budget)
                    .ok_or(FixedOccurrencesError::IdentityOverflow)?;
            let registers_len = function.virtual_registers.len();
            super::charge_steps(
                steps,
                required_budget,
                || FixedOccurrencesError::IdentityOverflow,
                || FixedOccurrencesError::WorkBudgetExceeded {
                    function: function_index,
                    required: steps,
                    limit: required_budget.validation_steps(),
                    registers: registers_len,
                    operands: operand_count,
                    transports: transport_count,
                    tie_steps,
                    settlement_steps,
                },
            )?;
            rewrite_function(
                &mut transformed.functions,
                function_index,
                row,
                constrained_operand_bound,
            )?;
        }
        Ok(super::sealed!(
            ValidatedFixedOccurrences,
            FixedOccurrencesReceipt,
            source,
            transformed
        ))
    }

    fn bounded_settlements(function: &SelectedFunction) -> Result<(), FixedOccurrencesError> {
        for block in &function.blocks {
            if u32::try_from(block.instructions.len()).is_err() {
                return Err(FixedOccurrencesError::IdentityOverflow);
            }
            for settlement in &function.boundary_settlements {
                if settlement.block == block.id
                    && (settlement.instruction_index as usize) > block.instructions.len()
                {
                    return Err(FixedOccurrencesError::SourceMismatch);
                }
            }
        }
        Ok(())
    }

    fn rewrite_function(
        functions: &mut selected_instructions::SelectedFunctions,
        function_index: usize,
        row: &RegisterInstructionConstraint,
        constrained_operand_bound: usize,
    ) -> Result<(), FixedOccurrencesError> {
        let function = &functions[function_index];
        let mut next_register = super::runtime_spill::next_register_id(function)
            .ok_or(FixedOccurrencesError::IdentityOverflow)?;
        let mut next_instruction = super::runtime_spill::next_instruction_id(function)
            .ok_or(FixedOccurrencesError::IdentityOverflow)?;
        if constrained_operand_bound == 0 {
            for instruction in function.instructions().chain(
                function
                    .blocks
                    .iter()
                    .map(|block| selected_instructions::terminator_instruction(&block.terminator)),
            ) {
                for operand in &instruction.operands {
                    if let Some(tied) = operand.tied_to
                        && !instruction
                            .operands
                            .iter()
                            .any(|candidate| candidate.operand == tied)
                    {
                        return Err(FixedOccurrencesError::ConstraintMismatch);
                    }
                }
            }
            return bounded_settlements(function);
        }
        let occurrences = local_occurrence_registers(function)?;
        if occurrences
            .registers
            .iter()
            .all(|register| register.instruction_local)
        {
            return bounded_settlements(function);
        }
        let mut prepared_instructions = occurrences.instructions.iter();
        let mut registers: Option<Vec<VirtualRegister>> = None;
        let mut block_updates = Vec::new();
        let mut settlement_updates = Vec::new();
        for (block_index, block) in function.blocks.iter().enumerate() {
            let block_inst_count = block.instructions.len();
            let slice = prepared_instructions.as_slice();
            if slice.len() < block_inst_count + 1 {
                return Err(FixedOccurrencesError::SourceMismatch);
            }
            let block_prepared = &slice[..block_inst_count + 1];
            let block_has_candidate = block_prepared.iter().any(|prepared| {
                prepared.operands.iter().any(|slot| {
                    slot.is_some_and(|roster_index| {
                        occurrences
                            .registers
                            .get(roster_index)
                            .is_none_or(|occ| !occ.instruction_local)
                    })
                })
            });
            if !block_has_candidate {
                if u32::try_from(block_inst_count).is_err() {
                    return Err(FixedOccurrencesError::IdentityOverflow);
                }
                for (original, prepared) in block
                    .instructions
                    .iter()
                    .chain(std::iter::once(
                        selected_instructions::terminator_instruction(&block.terminator),
                    ))
                    .zip(block_prepared)
                {
                    if original.id != prepared.id
                        || original.operands.len() != prepared.operands.len()
                    {
                        return Err(FixedOccurrencesError::SourceMismatch);
                    }
                }
                for _ in 0..=block_inst_count {
                    prepared_instructions.next();
                }
                for settlement in &function.boundary_settlements {
                    if settlement.block == block.id
                        && (settlement.instruction_index as usize) > block_inst_count
                    {
                        return Err(FixedOccurrencesError::SourceMismatch);
                    }
                }
                continue;
            }
            let regs = registers.get_or_insert_with(|| function.virtual_registers.clone());
            let mut new_instructions = Vec::with_capacity(block_inst_count + 4);
            let mut positions = Vec::with_capacity(block_inst_count + 1);
            let mut block_changed = false;
            for original_inst in &block.instructions {
                positions.push(
                    u32::try_from(new_instructions.len())
                        .map_err(|_| FixedOccurrencesError::IdentityOverflow)?,
                );
                let mut instruction = original_inst.clone();
                let (before, after) = materialize(
                    &mut instruction,
                    regs,
                    &occurrences,
                    prepared_instructions
                        .next()
                        .ok_or(FixedOccurrencesError::SourceMismatch)?,
                    row,
                    &mut next_register,
                    &mut next_instruction,
                )?;
                if !before.is_empty()
                    || !after.is_empty()
                    || instruction.operands != original_inst.operands
                {
                    block_changed = true;
                }
                new_instructions.extend(before);
                new_instructions.push(instruction);
                new_instructions.extend(after);
            }
            positions.push(
                u32::try_from(new_instructions.len())
                    .map_err(|_| FixedOccurrencesError::IdentityOverflow)?,
            );
            for (settlement_idx, settlement) in function.boundary_settlements.iter().enumerate() {
                if settlement.block == block.id {
                    let new_pos = *positions
                        .get(settlement.instruction_index as usize)
                        .ok_or(FixedOccurrencesError::SourceMismatch)?;
                    if new_pos != settlement.instruction_index {
                        settlement_updates.push((settlement_idx, new_pos));
                    }
                }
            }
            let mut new_terminator = block.terminator.clone();
            let (before, after) = materialize(
                control_mut(&mut new_terminator),
                regs,
                &occurrences,
                prepared_instructions
                    .next()
                    .ok_or(FixedOccurrencesError::SourceMismatch)?,
                row,
                &mut next_register,
                &mut next_instruction,
            )?;
            if !after.is_empty() {
                return Err(FixedOccurrencesError::ConstraintMismatch);
            }
            if !before.is_empty()
                || control_mut(&mut new_terminator).operands
                    != selected_instructions::terminator_instruction(&block.terminator).operands
            {
                block_changed = true;
            }
            new_instructions.extend(before);
            if block_changed {
                block_updates.push((block_index, new_instructions, new_terminator));
            }
        }
        if prepared_instructions.next().is_some() {
            return Err(FixedOccurrencesError::SourceMismatch);
        }
        if !block_updates.is_empty() || !settlement_updates.is_empty() {
            let function = functions
                .get_mut(function_index)
                .ok_or(FixedOccurrencesError::SourceMismatch)?;
            if let Some(regs) = registers {
                function.virtual_registers = regs;
            }
            for (settlement_idx, new_pos) in settlement_updates {
                function.boundary_settlements[settlement_idx].instruction_index = new_pos;
            }
            for (block_index, new_instructions, new_terminator) in block_updates {
                function.blocks.replace_block_contents(
                    block_index,
                    new_instructions,
                    new_terminator,
                    0,
                    0,
                );
            }
        }
        Ok(())
    }

    fn materialize(
        instruction: &mut SelectedInstruction,
        registers: &mut Vec<VirtualRegister>,
        occurrences: &OccurrencePlan,
        prepared: &PreparedInstruction,
        row: &RegisterInstructionConstraint,
        next_register: &mut u32,
        next_instruction: &mut u32,
    ) -> Result<(Vec<SelectedInstruction>, Vec<SelectedInstruction>), FixedOccurrencesError> {
        if instruction.id != prepared.id || instruction.operands.len() != prepared.operands.len() {
            return Err(FixedOccurrencesError::SourceMismatch);
        }
        let mut before = Vec::new();
        let mut after = Vec::new();
        let operations = instruction.provenance.operations.clone();
        for (operand, roster_index) in instruction.operands.iter_mut().zip(&prepared.operands) {
            let Some(roster_index) = *roster_index else {
                continue;
            };
            let occurrence = occurrences
                .registers
                .get(roster_index)
                .ok_or(FixedOccurrencesError::SourceMismatch)?;
            if occurrence.id != operand.virtual_register {
                return Err(FixedOccurrencesError::SourceMismatch);
            }
            if occurrence.instruction_local {
                continue;
            }
            let original_index = occurrence.source_index;
            let original = registers[original_index].clone();
            if matches!(
                original.origin,
                VirtualRegisterOrigin::InstructionScratch { .. }
            ) {
                continue;
            }
            if operand.class != original.class {
                return Err(FixedOccurrencesError::SourceMismatch);
            }
            let copy_class = row
                .operands
                .first()
                .ok_or(FixedOccurrencesError::ConstraintMismatch)?
                .class;
            if original.class != copy_class {
                continue;
            }
            admit_copy(row, &original)?;
            let fresh = VirtualRegisterId(take_identity(next_register)?);
            let mut temporary = original.clone();
            temporary.id = fresh;
            temporary.entry_fixed_view = None;
            let reads = matches!(
                operand.access,
                RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
            );
            let writes = matches!(
                operand.access,
                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
            );
            if reads {
                let id = SelectedInstructionId(take_identity(next_instruction)?);
                before.push(copy(row, id, original.id, fresh, &original, &operations));
                temporary.origin = origin_at(original.origin, id)?;
            } else {
                temporary.origin = origin_at(original.origin, instruction.id)?;
            }
            if writes {
                let id = SelectedInstructionId(take_identity(next_instruction)?);
                after.push(copy(row, id, fresh, original.id, &original, &[]));
                registers[original_index].origin = origin_at(original.origin, id)?;
            }
            registers.push(temporary);
            operand.virtual_register = fresh;
        }
        Ok((before, after))
    }

    fn fixed_component(
        instruction: &SelectedInstruction,
    ) -> Result<Vec<bool>, FixedOccurrencesError> {
        let mut local: Vec<bool> = instruction
            .operands
            .iter()
            .map(|operand| operand.fixed_view.is_some())
            .collect();
        loop {
            let mut changed = false;
            for (index, operand) in instruction.operands.iter().enumerate() {
                if let Some(tied) = operand.tied_to {
                    let other = instruction
                        .operands
                        .iter()
                        .position(|candidate| candidate.operand == tied)
                        .ok_or(FixedOccurrencesError::ConstraintMismatch)?;
                    if local[index] != local[other] {
                        local[index] = true;
                        local[other] = true;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        Ok(local)
    }

    fn take_identity(next: &mut u32) -> Result<u32, FixedOccurrencesError> {
        let identity = *next;
        *next = next
            .checked_add(1)
            .ok_or(FixedOccurrencesError::IdentityOverflow)?;
        Ok(identity)
    }

    struct OccurrencePlan {
        registers: Vec<PreparedOccurrenceRegister>,
        instructions: Vec<PreparedInstruction>,
    }

    struct PreparedInstruction {
        id: SelectedInstructionId,
        operands: Vec<Option<usize>>,
    }

    struct PreparedOccurrenceRegister {
        id: VirtualRegisterId,
        source_index: usize,
        instruction_local: bool,
    }

    fn local_occurrence_registers(
        function: &SelectedFunction,
    ) -> Result<OccurrencePlan, FixedOccurrencesError> {
        let mut selected_registers = Vec::new();
        let mut instruction_components = Vec::new();
        for instruction in function.blocks.iter().flat_map(|block| {
            block.instructions.iter().chain(std::iter::once(
                selected_instructions::terminator_instruction(&block.terminator),
            ))
        }) {
            let components = fixed_component(instruction)?;
            selected_registers.extend(
                instruction
                    .operands
                    .iter()
                    .zip(&components)
                    .filter_map(|(operand, selected)| selected.then_some(operand.virtual_register)),
            );
            instruction_components.push((instruction.id, components));
        }
        selected_registers.sort_unstable();
        selected_registers.dedup();
        let mut roster: Vec<_> = selected_registers
            .into_iter()
            .map(|identity| (identity, false, Vec::new(), None))
            .collect();
        for (source_index, register) in function.virtual_registers.iter().enumerate() {
            if let Ok(position) = roster.binary_search_by_key(&register.id, |entry| entry.0) {
                if roster[position].3.replace(source_index).is_some() {
                    return Err(FixedOccurrencesError::SourceMismatch);
                }
                roster[position].1 = register.entry_fixed_view.is_some();
            }
        }
        let mut instruction_components = instruction_components.into_iter();
        let mut instructions = Vec::new();
        for (block_index, block) in function.blocks.iter().enumerate() {
            for successor in selected_instructions::terminator_successors(&block.terminator) {
                let mut escape = |register| {
                    if let Ok(index) = roster.binary_search_by_key(&register, |entry| entry.0) {
                        roster[index].1 = true;
                    }
                };
                super::runtime_split::visit_structural_registers(successor, &mut escape);
                for binding in &successor.bindings {
                    if let selected_instructions::SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } = binding.transport
                    {
                        escape(argument);
                        escape(parameter);
                    }
                }
            }
            let mut destructive_prefix = 0usize;
            for (position, instruction) in block
                .instructions
                .iter()
                .chain(std::iter::once(
                    selected_instructions::terminator_instruction(&block.terminator),
                ))
                .enumerate()
            {
                let (identity, components) = instruction_components
                    .next()
                    .ok_or(FixedOccurrencesError::SourceMismatch)?;
                if identity != instruction.id {
                    return Err(FixedOccurrencesError::SourceMismatch);
                }
                let mut prepared_operands = Vec::with_capacity(instruction.operands.len());
                for (operand_index, operand) in instruction.operands.iter().enumerate() {
                    let Ok(index) =
                        roster.binary_search_by_key(&operand.virtual_register, |entry| entry.0)
                    else {
                        prepared_operands.push(None);
                        continue;
                    };
                    prepared_operands.push(components[operand_index].then_some(index));
                    if roster[index].2.len() < 4 {
                        roster[index].2.push((
                            block_index,
                            position,
                            instruction,
                            operand,
                            components[operand_index],
                            destructive_prefix,
                        ));
                    }
                }
                instructions.push(PreparedInstruction {
                    id: instruction.id,
                    operands: prepared_operands,
                });
                destructive_prefix += usize::from(!transport_kind(instruction.kind));
            }
        }
        if instruction_components.next().is_some() {
            return Err(FixedOccurrencesError::SourceMismatch);
        }
        let mut registers = Vec::with_capacity(roster.len());
        for (register, escapes, references, index) in roster {
            registers.push(PreparedOccurrenceRegister {
                id: register,
                source_index: index.ok_or(FixedOccurrencesError::SourceMismatch)?,
                instruction_local: false,
            });
            if escapes || references.is_empty() || references.len() > 3 {
                continue;
            }
            let mut constrained = references.iter().filter(|reference| reference.4);
            let Some(occurrence) = constrained.next() else {
                continue;
            };
            if constrained.next().is_some()
                || references
                    .iter()
                    .any(|reference| reference.0 != occurrence.0)
            {
                continue;
            }
            let first = &references[0];
            let last = &references[references.len() - 1];
            if first.1 < last.1 {
                let between = last.5 - first.5 - usize::from(!transport_kind(first.2.kind));
                let occurrence_inside = usize::from(
                    occurrence.1 > first.1
                        && occurrence.1 < last.1
                        && !transport_kind(occurrence.2.kind),
                );
                if between != occurrence_inside {
                    continue;
                }
            }
            let compatible = references.iter().all(|reference| {
                reference.4
                    || transport_kind(reference.2.kind)
                    || (occurrence.3.access == RegisterOperandAccess::Use
                        && reference.3.access == RegisterOperandAccess::Def
                        && reference.1 < occurrence.1
                        && reference.3.fixed_view.is_none()
                        && reference.3.tied_to.is_none()
                        && !reference.3.early_clobber)
            });
            if compatible
                && (references.len() > 1 || occurrence.3.access == RegisterOperandAccess::Def)
            {
                registers
                    .last_mut()
                    .ok_or(FixedOccurrencesError::SourceMismatch)?
                    .instruction_local = true;
            }
        }
        Ok(OccurrencePlan {
            registers,
            instructions,
        })
    }

    fn transport_kind(kind: SelectedInstructionKind) -> bool {
        matches!(
            kind,
            SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::Float32ToBits
                | SelectedInstructionKind::Float64ToBits
                | SelectedInstructionKind::BitsToFloat32
                | SelectedInstructionKind::BitsToFloat64
                | SelectedInstructionKind::ZeroExtendU8
                | SelectedInstructionKind::ZeroExtendU16
                | SelectedInstructionKind::ZeroExtendU32
                | SelectedInstructionKind::SignExtendI8
                | SelectedInstructionKind::SignExtendI16
                | SelectedInstructionKind::SignExtendI32
        )
    }

    fn admit_copy(
        row: &RegisterInstructionConstraint,
        register: &VirtualRegister,
    ) -> Result<(), FixedOccurrencesError> {
        let payload = match register.scalar_type {
            ScalarType::Callable(_) | ScalarType::Boolean => true,
            ScalarType::Integer(integer) => {
                matches!(integer.bits(), 8 | 16 | 32 | 64)
            }
            ScalarType::IeeeFloat(_) => true,
        };
        if !payload {
            return Err(FixedOccurrencesError::UnsupportedCarrier);
        }
        let flexible = |operand: &register_model::RegisterOperandConstraint, access| {
            operand.class == register.class
                && operand.access == access
                && operand.fixed_view.is_none()
                && operand.tied_to.is_none()
                && !operand.early_clobber
        };
        if row.operands.len() != 2
            || !flexible(&row.operands[0], RegisterOperandAccess::Use)
            || !flexible(&row.operands[1], RegisterOperandAccess::Def)
            || !row.implicit_uses.is_empty()
            || !row.implicit_defs.is_empty()
            || !row.clobbers.is_empty()
        {
            return Err(FixedOccurrencesError::CopyConstraintMismatch {
                register: register.id,
                class: register.class,
            });
        }
        Ok(())
    }

    fn origin_at(
        origin: VirtualRegisterOrigin,
        instruction: SelectedInstructionId,
    ) -> Result<VirtualRegisterOrigin, FixedOccurrencesError> {
        Ok(match origin {
            VirtualRegisterOrigin::InstructionResult { source_value, .. }
            | VirtualRegisterOrigin::EntryParameter { source_value, .. }
            | VirtualRegisterOrigin::BlockParameter { source_value, .. } => {
                VirtualRegisterOrigin::InstructionResult {
                    instruction,
                    source_value,
                }
            }
            VirtualRegisterOrigin::StructuralObservation {
                place, byte_offset, ..
            }
            | VirtualRegisterOrigin::AbiTransport {
                place, byte_offset, ..
            } => VirtualRegisterOrigin::AbiTransport {
                instruction,
                place,
                byte_offset,
            },
            _ => return Err(FixedOccurrencesError::UnsupportedCarrier),
        })
    }

    fn copy(
        row: &RegisterInstructionConstraint,
        id: SelectedInstructionId,
        input: VirtualRegisterId,
        output: VirtualRegisterId,
        register: &VirtualRegister,
        operations: &[semantic_vocabulary::OperationId],
    ) -> SelectedInstruction {
        let mut instruction = copy_shaped(
            id,
            SelectedInstructionKind::CopyI64,
            row,
            input,
            output,
            selected_instructions::SelectedInstructionProvenance {
                operations: operations.to_vec(),
                ..Default::default()
            },
        );
        match register.origin {
            VirtualRegisterOrigin::InstructionResult { source_value, .. }
            | VirtualRegisterOrigin::EntryParameter { source_value, .. }
            | VirtualRegisterOrigin::BlockParameter { source_value, .. } => {
                instruction.provenance.values.push(source_value)
            }
            _ => {}
        }
        instruction
    }
}
pub(crate) mod fixed_view {
    pub(crate) mod fixed_precolored_segment_homes {
        pub(crate) mod compute {
            use crate::{
                ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSegmentHomes,
                ValidatedFixedPrecoloredSplitRequirements, analyze_fixed_precolored_intervals,
                analyze_fixed_precolored_split_requirements, assign_fixed_precolored_segment_homes,
            };
            use optimization_core::OptimizationWorkBudget;
            use register_homes::{
                FixedPrecoloredIntervalPolicy, FixedPrecoloredSegmentHomePolicy,
                FixedPrecoloredSplitRequirementPolicy,
            };

            use crate::StagedOptimizedAllocationLegality;

            use super::OptimizedFixedPrecoloredSegmentHomeCustodyError;

            pub(crate) fn derive(
                source: &StagedOptimizedAllocationLegality,
                budget: OptimizationWorkBudget,
            ) -> Result<
                (
                    ValidatedFixedPrecoloredIntervals,
                    ValidatedFixedPrecoloredSplitRequirements,
                    ValidatedFixedPrecoloredSegmentHomes,
                ),
                OptimizedFixedPrecoloredSegmentHomeCustodyError,
            > {
                let ranges = source.ranges();
                let fixed = analyze_fixed_precolored_intervals(
                    ranges,
                    source.legality(),
                    FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1,
                    budget,
                )
                .map_err(OptimizedFixedPrecoloredSegmentHomeCustodyError::FixedIntervals)?;
                let requirements = analyze_fixed_precolored_split_requirements(
                    ranges,
                    source.legality(),
                    &fixed,
                    FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1,
                    budget,
                )
                .map_err(OptimizedFixedPrecoloredSegmentHomeCustodyError::SplitRequirements)?;
                let environment = source.register_environment();
                let homes = assign_fixed_precolored_segment_homes(
                    ranges,
                    source.legality(),
                    &fixed,
                    &requirements,
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    &environment.allocation_constraint_keys(),
                    FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1,
                    budget,
                )
                .map_err(OptimizedFixedPrecoloredSegmentHomeCustodyError::SegmentHomes)?;
                Ok((fixed, requirements, homes))
            }
        }
        pub(crate) mod custody {
            use crate::{
                FixedPrecoloredIntervalValidationReceipt,
                FixedPrecoloredSegmentHomeValidationReceipt,
                FixedPrecoloredSplitRequirementValidationReceipt,
            };

            use crate::StagedOptimizedAllocationLegalityCustodyReceipt;

            use super::StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt;

            pub(crate) const fn seal(
                upstream: StagedOptimizedAllocationLegalityCustodyReceipt,
                fixed: FixedPrecoloredIntervalValidationReceipt,
                requirements: FixedPrecoloredSplitRequirementValidationReceipt,
                homes: FixedPrecoloredSegmentHomeValidationReceipt,
            ) -> StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
                StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
                    upstream,
                    fixed,
                    requirements,
                    homes,
                }
            }
        }

        use optimization_core::OptimizationWorkBudget;

        use crate::{
            FixedPrecoloredIntervalError, FixedPrecoloredIntervalValidationReceipt,
            FixedPrecoloredSegmentHomeError, FixedPrecoloredSegmentHomeValidationReceipt,
            FixedPrecoloredSplitRequirementError, FixedPrecoloredSplitRequirementValidationReceipt,
            StagedOptimizedAllocationLegality, StagedOptimizedAllocationLegalityCustodyReceipt,
            ValidatedAllocationLegality, ValidatedAllocatorAvailability,
            ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSegmentHomes,
            ValidatedFixedPrecoloredSplitRequirements, ValidatedLiveRanges, ValidatedLiveness,
            WorkDefect,
        };
        use optimization_core::OptimizationSelections;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::SelectedProgram;

        pub fn probe_optimized_fixed_precolored_segment_homes(
            source: &StagedOptimizedAllocationLegality,
            budget: OptimizationWorkBudget,
        ) -> Result<(), OptimizedFixedPrecoloredSegmentHomeCustodyError> {
            compute::derive(source, budget)?;
            Ok(())
        }

        pub fn stage_optimized_fixed_precolored_segment_homes(
            source: StagedOptimizedAllocationLegality,
            budget: OptimizationWorkBudget,
        ) -> Result<
            StagedOptimizedFixedPrecoloredSegmentHomes,
            OptimizedFixedPrecoloredSegmentHomeCustodyError,
        > {
            let (fixed, requirements, homes) = compute::derive(&source, budget)?;
            let custody = custody::seal(
                source.custody(),
                fixed.receipt(),
                requirements.receipt(),
                homes.receipt(),
            );
            Ok(StagedOptimizedFixedPrecoloredSegmentHomes {
                source,
                fixed,
                requirements,
                homes,
                custody,
            })
        }

        #[derive(Debug)]
        pub struct StagedOptimizedFixedPrecoloredSegmentHomes {
            source: StagedOptimizedAllocationLegality,
            fixed: ValidatedFixedPrecoloredIntervals,
            requirements: ValidatedFixedPrecoloredSplitRequirements,
            homes: ValidatedFixedPrecoloredSegmentHomes,
            custody: StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
        }

        impl StagedOptimizedFixedPrecoloredSegmentHomes {
            pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
                &self.source
            }

            pub const fn selected(&self) -> &SelectedProgram {
                self.source.selected()
            }

            pub const fn liveness(&self) -> &ValidatedLiveness {
                self.source.liveness()
            }

            pub const fn ranges(&self) -> &ValidatedLiveRanges {
                self.source.ranges()
            }

            pub const fn legality(&self) -> &ValidatedAllocationLegality {
                self.source.legality()
            }

            pub const fn allocator_availability(&self) -> &ValidatedAllocatorAvailability {
                self.source.allocator_availability()
            }

            pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
                self.source.register_environment()
            }

            pub fn selections(&self) -> &OptimizationSelections {
                self.source.selections()
            }

            pub fn budget_per_pass(&self) -> OptimizationWorkBudget {
                self.source.budget_per_pass()
            }

            pub const fn fixed_intervals(&self) -> &ValidatedFixedPrecoloredIntervals {
                &self.fixed
            }
            pub const fn split_requirements(&self) -> &ValidatedFixedPrecoloredSplitRequirements {
                &self.requirements
            }
            pub const fn segment_homes(&self) -> &ValidatedFixedPrecoloredSegmentHomes {
                &self.homes
            }
            receipt_accessor_rows!(ref StagedOptimizedFixedPrecoloredSegmentHomes {
                custody: StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
            });
        }

        copy_receipt! {
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
                upstream: StagedOptimizedAllocationLegalityCustodyReceipt,
                fixed: FixedPrecoloredIntervalValidationReceipt,
                requirements: FixedPrecoloredSplitRequirementValidationReceipt,
                homes: FixedPrecoloredSegmentHomeValidationReceipt,
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum OptimizedFixedPrecoloredSegmentHomeCustodyError {
            FixedIntervals(FixedPrecoloredIntervalError),
            SplitRequirements(FixedPrecoloredSplitRequirementError),
            SegmentHomes(FixedPrecoloredSegmentHomeError),
            ReceiptMismatch,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum FixedPrecoloredSegmentHomeDecline {
            SegmentPressure,
            BudgetExceeded,
        }

        impl OptimizedFixedPrecoloredSegmentHomeCustodyError {
            pub fn capacity_decline(&self) -> Option<FixedPrecoloredSegmentHomeDecline> {
                match self {
                    Self::SegmentHomes(FixedPrecoloredSegmentHomeError::SegmentPressure {
                        ..
                    }) => Some(FixedPrecoloredSegmentHomeDecline::SegmentPressure),
                    Self::FixedIntervals(FixedPrecoloredIntervalError::Work(
                        WorkDefect::BudgetExceeded { .. },
                    ))
                    | Self::SplitRequirements(FixedPrecoloredSplitRequirementError::Work(
                        WorkDefect::BudgetExceeded { .. },
                    ))
                    | Self::SegmentHomes(FixedPrecoloredSegmentHomeError::Work(
                        WorkDefect::BudgetExceeded { .. },
                    )) => Some(FixedPrecoloredSegmentHomeDecline::BudgetExceeded),
                    _ => None,
                }
            }
        }

        failed_display!(
            OptimizedFixedPrecoloredSegmentHomeCustodyError,
            "fixed/precolored segment-home staging failed"
        );
    }
    pub(crate) mod fixed_view_copies {
        pub(crate) mod compute {
            use crate::rewrites::materialize_fixed_view_copies;
            use crate::{FixedViewCopyPolicy, ValidatedFixedViewCopies};
            use optimization_core::OptimizationWorkBudget;

            use crate::StagedOptimizedFixedPrecoloredSegmentHomes;

            use super::OptimizedFixedViewCopyCustodyError;

            pub(crate) fn compute_fixed_view_copies(
                source: &StagedOptimizedFixedPrecoloredSegmentHomes,
                policy: FixedViewCopyPolicy,
                budget: OptimizationWorkBudget,
            ) -> Result<ValidatedFixedViewCopies, OptimizedFixedViewCopyCustodyError> {
                let environment = source.register_environment();
                materialize_fixed_view_copies(
                    source.selected(),
                    source.ranges(),
                    source.legality(),
                    source.fixed_intervals(),
                    source.split_requirements(),
                    source.segment_homes(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    &environment.allocation_constraint_keys(),
                    policy,
                    budget,
                )
                .map_err(OptimizedFixedViewCopyCustodyError::Materialization)
            }
        }
        pub(crate) mod custody {
            use crate::FixedViewCopyValidationReceipt;

            use crate::StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt;

            use super::StagedOptimizedFixedViewCopyCustodyReceipt;

            pub(crate) fn fixed_view_copy_custody_receipt(
                segment_homes: StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
                copies: FixedViewCopyValidationReceipt,
            ) -> StagedOptimizedFixedViewCopyCustodyReceipt {
                StagedOptimizedFixedViewCopyCustodyReceipt {
                    core: segment_homes.upstream().core(),
                    allocator_availability: segment_homes.upstream().allocator_availability(),
                    source_selected: segment_homes.upstream().selected(),
                    source_liveness: segment_homes.upstream().liveness(),
                    source_ranges: segment_homes.upstream().ranges(),
                    source_legality: segment_homes.upstream().legality(),
                    fixed_intervals: segment_homes.fixed().identity(),
                    split_requirements: segment_homes.requirements().identity(),
                    segment_homes: segment_homes.homes().identity(),
                    transformation: copies.identity(),
                    transformed_selected: copies.transformed_selected(),
                    policy: copies.policy(),
                    usage: copies.usage(),
                    function_count: copies.function_count(),
                    copy_count: copies.copy_count(),
                }
            }
        }
        use crate::FixedViewCopyPolicy;
        use optimization_core::OptimizationWorkBudget;

        use crate::StagedOptimizedFixedPrecoloredSegmentHomes;
        use crate::{
            FixedViewCopyError, FixedViewCopyIdentity,
            OptimizedFixedPrecoloredSegmentHomeCustodyError, StagedOptimizedAllocationLegality,
            ValidatedAllocatorAvailability, ValidatedFixedViewCopies,
        };
        use optimization_core::{
            OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
            OptimizationWorkUsage, OptimizedAbstractPlanProjectionIdentity,
            PrePhysicalOptimizationManifestIdentity,
        };
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::SelectedInstructionPlanIdentity;
        use selected_instructions::StagedOptimizedCustodyCore;
        use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
        use terminal_psi::TerminalPsiIdentity;

        pub fn stage_optimized_fixed_view_copies(
            source: StagedOptimizedFixedPrecoloredSegmentHomes,
            policy: FixedViewCopyPolicy,
            budget: OptimizationWorkBudget,
        ) -> Result<StagedOptimizedFixedViewCopies, OptimizedFixedViewCopyCustodyError> {
            let copies = compute::compute_fixed_view_copies(&source, policy, budget)?;
            let custody =
                custody::fixed_view_copy_custody_receipt(source.custody(), copies.receipt());
            Ok(StagedOptimizedFixedViewCopies {
                source,
                copies,
                custody,
            })
        }

        #[derive(Debug)]
        pub struct StagedOptimizedFixedViewCopies {
            source: StagedOptimizedFixedPrecoloredSegmentHomes,
            copies: ValidatedFixedViewCopies,
            custody: StagedOptimizedFixedViewCopyCustodyReceipt,
        }

        impl StagedOptimizedFixedViewCopies {
            staged_producer_accessors!(
                source; source_legality_stage, register_environment, allocator_availability, selections,
                budget_per_pass
            );

            pub const fn copies(&self) -> &ValidatedFixedViewCopies {
                &self.copies
            }
            receipt_accessor_rows!(ref StagedOptimizedFixedViewCopies {
                custody: StagedOptimizedFixedViewCopyCustodyReceipt,
            });
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct StagedOptimizedFixedViewCopyCustodyReceipt {
            core: StagedOptimizedCustodyCore,
            allocator_availability: register_homes::AllocatorAvailabilityIdentity,
            source_selected: SelectedInstructionPlanIdentity,
            source_liveness: selected_instructions::LivenessIdentity,
            source_ranges: selected_instructions::LiveRangeIdentity,
            source_legality: register_homes::AllocationLegalityIdentity,
            fixed_intervals: register_homes::FixedPrecoloredIntervalPlanIdentity,
            split_requirements: register_homes::FixedPrecoloredSplitRequirementPlanIdentity,
            segment_homes: register_homes::FixedPrecoloredSegmentHomePlanIdentity,
            transformation: FixedViewCopyIdentity,
            transformed_selected: SelectedInstructionPlanIdentity,
            policy: FixedViewCopyPolicy,
            usage: OptimizationWorkUsage,
            function_count: usize,
            copy_count: usize,
        }

        impl StagedOptimizedFixedViewCopyCustodyReceipt {
            selected_instructions::staging_custody_core_accessors!();

            receipt_accessor_rows!(StagedOptimizedFixedViewCopyCustodyReceipt {
                allocator_availability: register_homes::AllocatorAvailabilityIdentity,
                source_selected: SelectedInstructionPlanIdentity,
                source_liveness: selected_instructions::LivenessIdentity,
                source_ranges: selected_instructions::LiveRangeIdentity,
                source_legality: register_homes::AllocationLegalityIdentity,
                transformed_selected: SelectedInstructionPlanIdentity,
                fixed_intervals: register_homes::FixedPrecoloredIntervalPlanIdentity,
                segment_homes: register_homes::FixedPrecoloredSegmentHomePlanIdentity,
                transformation: FixedViewCopyIdentity,
                policy: FixedViewCopyPolicy,
                usage: OptimizationWorkUsage,
                function_count: usize,
                copy_count: usize,
            });
            pub const fn split_requirements(
                self,
            ) -> register_homes::FixedPrecoloredSplitRequirementPlanIdentity {
                self.split_requirements
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum OptimizedFixedViewCopyCustodyError {
            UpstreamSegmentHomes(OptimizedFixedPrecoloredSegmentHomeCustodyError),
            Materialization(FixedViewCopyError),
            Revalidation(FixedViewCopyError),
            ReceiptMismatch,
        }

        failed_display!(
            OptimizedFixedViewCopyCustodyError,
            "optimized fixed-view copy staging failed"
        );
    }

    pub use fixed_precolored_segment_homes::{
        OptimizedFixedPrecoloredSegmentHomeCustodyError,
        probe_optimized_fixed_precolored_segment_homes,
        stage_optimized_fixed_precolored_segment_homes,
    };
    pub(crate) use fixed_precolored_segment_homes::{
        StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
        StagedOptimizedFixedPrecoloredSegmentHomes,
    };
    pub use fixed_view_copies::{
        OptimizedFixedViewCopyCustodyError, stage_optimized_fixed_view_copies,
    };
    pub(crate) use fixed_view_copies::{
        StagedOptimizedFixedViewCopies, StagedOptimizedFixedViewCopyCustodyReceipt,
    };
}
pub(crate) use crate::runtime_spill_and_split::runtime_rematerialization;
pub(crate) use crate::runtime_spill_and_split::runtime_spill;
pub(crate) use crate::runtime_spill_and_split::runtime_split;
pub(crate) mod settlement_gaps {
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBoundarySettlement,
        SelectedBoundarySettlementPayload,
    };

    pub(crate) fn gap_tables(carries: bool, block: &SelectedBlock) -> (Vec<u32>, Vec<u32>) {
        let capacity = usize::from(carries) * (block.instructions.len() + 1);
        (Vec::with_capacity(capacity), Vec::with_capacity(capacity))
    }

    pub(crate) fn push_position(table: &mut Vec<u32>, carries: bool, at: usize) -> Option<()> {
        if carries {
            table.push(u32::try_from(at).ok()?);
        }
        Some(())
    }

    pub(crate) fn remap(
        settlements: &mut [SelectedBoundarySettlement],
        boundaries: &[u32],
        positions: &[u32],
        block: SelectedBlockId,
    ) -> Option<()> {
        for settlement in settlements {
            if settlement.block != block {
                continue;
            }
            let table = match settlement.settlement {
                SelectedBoundarySettlementPayload::ClaimCompletion(_) => boundaries,
                _ => positions,
            };
            settlement.instruction_index = *table.get(settlement.instruction_index as usize)?;
        }
        Some(())
    }
}

pub(crate) use allocation_recovery::FixedViewCopyDestination;
pub use allocation_recovery::{
    FixedViewCopy, FixedViewCopyPlan, FixedViewCopyPolicy, FixedViewCopySourceEvidence,
    PressureRematerializationError, PressureRematerializationPolicy, ValidatedFixedViewCopies,
    ValidatedPressureRematerialization, fixed_view_copy_identity,
    rematerialize_selected_active_resident,
};
pub(crate) use allocation_recovery::{
    FixedViewCopyError, FixedViewCopyValidationReceipt, FunctionPressureRematerialization,
    PressureRematerializationAction, PressureRematerializationPlan,
    PressureRematerializationRewrite, PressureRematerializationValidationReceipt,
    materialize_fixed_view_copies, pressure_rematerialization_identity,
};
pub use fixed_occurrences::{
    FixedOccurrencesError, ValidatedFixedOccurrences, materialize_selected_fixed_occurrences,
};
pub use fixed_view::{
    OptimizedFixedPrecoloredSegmentHomeCustodyError, OptimizedFixedViewCopyCustodyError,
    probe_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_precolored_segment_homes,
    stage_optimized_fixed_view_copies,
};
pub(crate) use fixed_view::{
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    StagedOptimizedFixedPrecoloredSegmentHomes, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt,
};
pub use runtime_rematerialization::{
    RuntimeRematerializationError, ValidatedRuntimeRematerialization,
    rematerialize_selected_runtime_value,
};
pub use runtime_spill::{
    RuntimeSpillError, RuntimeSpillSpanPolicy, ValidatedRuntimeSpill, spill_selected_runtime_value,
    spill_selected_runtime_value_with_crossing_status,
    spill_selected_runtime_value_with_span_policy,
};
pub use runtime_split::{RuntimeSplitError, ValidatedRuntimeSplit, split_selected_runtime_value};

pub(crate) use rematerialization::complete_optimized_active_resident_rematerialization;

pub(crate) use rematerialization::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedActiveResidentRematerializationPressure,
    stage_optimized_active_resident_rematerialization_pressure,
};
