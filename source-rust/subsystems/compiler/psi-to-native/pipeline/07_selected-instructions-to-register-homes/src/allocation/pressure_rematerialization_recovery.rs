/*
We compute, encode, and validate active-resident immediate `u64`
rematerialization (`ValidatedPressureRematerialization`) before single or
multiple future flexible uses to relieve register pressure without spilling to
the stack frame.
*/

use crate::allocation::{
    ValidatedAllocationLegality, ValidatedAllocatorAvailability, ValidatedLiveRanges,
    ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
};
use register_model::{
    PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity,
};

pub(crate) mod compute {
    pub(crate) mod application {
        use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
        use selected_instructions::{
            SelectedFunction, SelectedInstruction, SelectedInstructionKind,
            SelectedInstructionProvenance, VirtualRegister, VirtualRegisterId,
            VirtualRegisterOrigin,
        };

        use crate::allocation::{
            PressureRematerializationAction, PressureRematerializationError,
            PressureRematerializationRewrite,
        };

        use super::selected_structure;

        pub(crate) fn apply(
            function_index: usize,
            function: &mut SelectedFunction,
            action: &PressureRematerializationAction,
            row: &RegisterInstructionConstraint,
        ) -> Result<(), PressureRematerializationError> {
            let source = function.virtual_register(action.victim).cloned().ok_or(
                PressureRematerializationError::DecisionMismatch {
                    function: function_index,
                },
            )?;
            function.virtual_registers.push(VirtualRegister {
                id: action.result_virtual_register,
                scalar_type: source.scalar_type,
                class: source.class,
                origin: VirtualRegisterOrigin::InstructionResult {
                    instruction: action.fresh_materialize,
                    source_value: action.source_value,
                },
                definition_site: source.definition_site,
                entry_fixed_view: None,
            });
            let new_instruction = SelectedInstruction {
                id: action.fresh_materialize,
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: action.value,
                },
                constraint: action.materialize_constraint,
                operands: vec![selected_structure::operand(
                    &row.operands[0],
                    action.result_virtual_register,
                )],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance {
                    values: vec![action.source_value],
                    ..Default::default()
                },
            };
            let contracts = std::sync::Arc::make_mut(&mut function.contracts);
            let block = function
                .blocks
                .iter_mut()
                .find(|block| block.id == action.block)
                .ok_or(PressureRematerializationError::DecisionMismatch {
                    function: function_index,
                })?;
            for rewrite in &action.rewrites {
                rewrite_operand(
                    function_index,
                    block,
                    action.victim,
                    action.result_virtual_register,
                    *rewrite,
                )?;
            }
            let first = action.rewrites.first().ok_or(
                PressureRematerializationError::DecisionMismatch {
                    function: function_index,
                },
            )?;
            if let Some(index) = block.instruction_index(first.instruction) {
                let insertion_index = u32::try_from(index)
                    .map_err(|_| PressureRematerializationError::WorkOverflow)?;
                for settlement in &mut contracts.boundary_settlements {
                    if settlement.block == block.id
                        && settlement.instruction_index >= insertion_index
                    {
                        settlement.instruction_index = settlement
                            .instruction_index
                            .checked_add(1)
                            .ok_or(PressureRematerializationError::WorkOverflow)?;
                    }
                }
                block.instructions.insert(index, new_instruction);
            } else {
                let terminator_id =
                    selected_instructions::terminator_instruction(&block.terminator).id;
                if terminator_id != first.instruction {
                    return Err(PressureRematerializationError::DecisionMismatch {
                        function: function_index,
                    });
                }
                let insertion_index = u32::try_from(block.instructions.len())
                    .map_err(|_| PressureRematerializationError::WorkOverflow)?;
                for settlement in &mut contracts.boundary_settlements {
                    if settlement.block == block.id
                        && settlement.instruction_index >= insertion_index
                    {
                        settlement.instruction_index = settlement
                            .instruction_index
                            .checked_add(1)
                            .ok_or(PressureRematerializationError::WorkOverflow)?;
                    }
                }
                block.instructions.push(new_instruction);
            }
            Ok(())
        }

        fn rewrite_operand(
            function: usize,
            block: &mut selected_instructions::SelectedBlock,
            victim: VirtualRegisterId,
            result: VirtualRegisterId,
            rewrite: PressureRematerializationRewrite,
        ) -> Result<(), PressureRematerializationError> {
            let instruction = block
                .instructions
                .iter_mut()
                .find(|instruction| instruction.id == rewrite.instruction)
                .or_else(|| {
                    let terminator =
                        selected_instructions::terminator_instruction_mut(&mut block.terminator);
                    (terminator.id == rewrite.instruction).then_some(terminator)
                })
                .ok_or(PressureRematerializationError::DecisionMismatch { function })?;
            let operand = instruction
                .operands
                .iter_mut()
                .find(|operand| {
                    operand.operand == rewrite.operand
                        && operand.virtual_register == victim
                        && operand.access == RegisterOperandAccess::Use
                        && operand.fixed_view.is_none()
                })
                .ok_or(PressureRematerializationError::DecisionMismatch { function })?;
            operand.virtual_register = result;
            Ok(())
        }
    }
    pub(crate) mod candidate_action {
        use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
        use selected_instructions::{SelectedFunction, SelectedInstructionId, VirtualRegisterId};

        use crate::allocation::{
            PressureRematerializationAction, PressureRematerializationError,
            PressureRematerializationPolicy, PressureRematerializationRewrite,
        };
        use register_homes::{
            PressureRecoveryClassification, RecoveryClassification, RecoveryVictimRole,
        };
        use selected_instructions::FunctionLiveRanges;

        use super::selected_structure;

        pub(crate) fn derive(
            function_index: usize,
            function: &SelectedFunction,
            ranges: &FunctionLiveRanges,
            candidate: &PressureRecoveryClassification,
            row: &RegisterInstructionConstraint,
            policy: PressureRematerializationPolicy,
        ) -> Result<Option<PressureRematerializationAction>, PressureRematerializationError>
        {
            // A victim that is not an active-resident immediate, or whose future
            // uses are not the count the policy rewrites, is not eligible: the
            // function keeps its program and the rule records no action for it.
            let RecoveryVictimRole::ActiveResident {
                current_view,
                reclaimed_view,
            } = candidate.role
            else {
                return Ok(None);
            };
            let RecoveryClassification::ImmediateU64RematerializationCandidate {
                defining_instruction,
                source_value,
                value,
                provenance,
                future_uses,
            } = &candidate.classification
            else {
                return Ok(None);
            };
            let eligible_arity = match policy {
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 => future_uses.len() == 1,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1 => future_uses.len() >= 2,
            };
            if !eligible_arity {
                return Ok(None);
            }
            if future_uses.windows(2).any(|pair| pair[0] >= pair[1])
                || future_uses.iter().any(|future| {
                    future.block != candidate.block || future.point <= candidate.point
                })
            {
                return Err(PressureRematerializationError::FutureUseMismatch {
                    function: function_index,
                });
            }
            let victim = function.virtual_register(candidate.victim).ok_or(
                PressureRematerializationError::MaterializeMismatch {
                    function: function_index,
                },
            )?;
            if victim.scalar_type != candidate.scalar_type
                || victim.class != candidate.class
                || victim.origin != candidate.origin
                || victim.definition_site != candidate.definition_site
                || victim.entry_fixed_view.is_some()
                || row.operands[0].class != victim.class
            {
                return Err(PressureRematerializationError::MaterializeMismatch {
                    function: function_index,
                });
            }
            let block = function.block(candidate.block).ok_or(
                PressureRematerializationError::MaterializeMismatch {
                    function: function_index,
                },
            )?;
            let original = block.instruction(*defining_instruction).ok_or(
                PressureRematerializationError::MaterializeMismatch {
                    function: function_index,
                },
            )?;
            if original.kind
                != (selected_instructions::SelectedInstructionKind::MaterializeI64 {
                    value: *value,
                })
                || original.constraint != row.key
                || original.provenance != *provenance
                || original.operands.as_slice()
                    != [selected_structure::operand(
                        &row.operands[0],
                        candidate.victim,
                    )]
            {
                return Err(PressureRematerializationError::MaterializeMismatch {
                    function: function_index,
                });
            }
            let range = ranges
                .virtual_registers
                .iter()
                .find(|range| range.virtual_register == candidate.victim)
                .ok_or(PressureRematerializationError::MaterializeMismatch {
                    function: function_index,
                })?;
            if !range.occurrences.iter().any(|occurrence| {
                occurrence.instruction == *defining_instruction
                    && occurrence.access == register_model::RegisterOperandAccess::Def
                    && occurrence.point < candidate.point
            }) {
                return Err(PressureRematerializationError::MaterializeMismatch {
                    function: function_index,
                });
            }
            for future in future_uses {
                let future_instruction =
                    selected_structure::find_instruction(block, future.instruction).ok_or(
                        PressureRematerializationError::FutureUseMismatch {
                            function: function_index,
                        },
                    )?;
                let matching = future_instruction
                    .operands
                    .iter()
                    .filter(|operand| operand.operand == future.operand)
                    .collect::<Vec<_>>();
                if matching.len() != 1
                    || matching[0].virtual_register != candidate.victim
                    || matching[0].access != RegisterOperandAccess::Use
                    || matching[0].fixed_view.is_some()
                    || matching[0].class != candidate.class
                {
                    return Err(PressureRematerializationError::FutureUseMismatch {
                        function: function_index,
                    });
                }
            }
            let fresh_instruction = SelectedInstructionId(selected_structure::instruction_count(
                function_index,
                function,
            )?);
            let fresh_register =
                VirtualRegisterId(u32::try_from(function.virtual_registers.len()).map_err(
                    |_| PressureRematerializationError::IdentifierOverflow {
                        function: function_index,
                    },
                )?);
            Ok(Some(PressureRematerializationAction {
                block: candidate.block,
                pressure_point: candidate.point,
                victim: candidate.victim,
                current_view,
                reclaimed_view,
                original_materialize: *defining_instruction,
                source_value: *source_value,
                value: *value,
                rewrites: future_uses
                    .iter()
                    .map(|future| PressureRematerializationRewrite {
                        point: future.point,
                        instruction: future.instruction,
                        operand: future.operand,
                    })
                    .collect(),
                fresh_materialize: fresh_instruction,
                result_virtual_register: fresh_register,
                materialize_constraint: row.key,
            }))
        }
    }
    pub(crate) mod function_plans {
        use register_model::RegisterInstructionConstraint;
        use selected_instructions::SelectedInstructionPlan;

        use crate::allocation::{
            FunctionPressureRematerialization, PressureRematerializationError,
            PressureRematerializationPolicy,
        };
        use register_homes::RecoveryClassificationPlan;
        use selected_instructions::LiveRangePlan;

        use super::{application, candidate_action, selected_structure};

        pub(crate) fn build_functions(
            selected: &SelectedInstructionPlan,
            ranges: &LiveRangePlan,
            recovery: &RecoveryClassificationPlan,
            row: &RegisterInstructionConstraint,
            policy: PressureRematerializationPolicy,
        ) -> Result<
            (
                Vec<FunctionPressureRematerialization>,
                SelectedInstructionPlan,
            ),
            PressureRematerializationError,
        > {
            let mut transformed = selected.clone();
            let mut functions = Vec::with_capacity(transformed.functions.len());
            for index in 0..transformed.functions.len() {
                let source = &selected.functions[index];
                let range_function = ranges
                    .functions
                    .get(index)
                    .ok_or(PressureRematerializationError::FunctionMismatch { function: index })?;
                let recovery_function = recovery
                    .functions
                    .get(index)
                    .ok_or(PressureRematerializationError::FunctionMismatch { function: index })?;
                if source.machine != range_function.machine
                    || source.machine != recovery_function.machine
                {
                    return Err(PressureRematerializationError::FunctionMismatch {
                        function: index,
                    });
                }
                selected_structure::validate_dense(index, source)?;
                let action = match &recovery_function.classification {
                    None => None,
                    Some(candidate) => candidate_action::derive(
                        index,
                        source,
                        range_function,
                        candidate,
                        row,
                        policy,
                    )?,
                };
                if let Some(action) = &action {
                    application::apply(index, &mut transformed.functions[index], action, row)?;
                }
                functions.push(FunctionPressureRematerialization {
                    machine: source.machine,
                    action,
                });
            }
            Ok((functions, transformed))
        }
    }
    pub(crate) mod materialize_constraint {
        use register_model::{
            PreparedRegisterConstraintCatalog, RegisterInstructionConstraint,
            RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
        };

        use crate::allocation::PressureRematerializationError;

        pub(crate) fn select<'a>(
            constraints: &'a PreparedRegisterConstraintCatalog,
            keys: &TargetRegisterEnvironmentConstraintKeys,
        ) -> Result<&'a RegisterInstructionConstraint, PressureRematerializationError> {
            let row = register_model::find_register_constraint(
                &constraints.catalog().constraints,
                keys.materialize_i64,
            )
            .ok_or(PressureRematerializationError::MaterializeConstraintMismatch)?;
            let [result] = row.operands.as_slice() else {
                return Err(PressureRematerializationError::MaterializeConstraintMismatch);
            };
            if result.operand != 0
                || result.access != RegisterOperandAccess::Def
                || result.fixed_view.is_some()
                || result.tied_to.is_some()
                || result.early_clobber
                || !row.implicit_uses.is_empty()
                || !row.implicit_defs.is_empty()
                || !row.clobbers.is_empty()
            {
                return Err(PressureRematerializationError::MaterializeConstraintMismatch);
            }
            Ok(row)
        }
    }
    pub(crate) mod selected_structure {
        use selected_instructions::{
            SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedTerminator,
        };

        use crate::allocation::PressureRematerializationError;
        use crate::allocation::rewrites::block_edges::block_instruction_ids;

        pub(crate) use crate::allocation::rewrites::selected_operands::selected_operand as operand;

        pub(crate) fn find_instruction(
            block: &selected_instructions::SelectedBlock,
            id: SelectedInstructionId,
        ) -> Option<&SelectedInstruction> {
            block.instruction(id).or_else(|| match &block.terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. }
                | SelectedTerminator::Jump { instruction, .. }
                | selected_instructions::terminator_no_successor_pattern!(instruction, ..)
                    if instruction.id == id =>
                {
                    Some(instruction)
                }
                _ => None,
            })
        }

        pub(crate) fn validate_dense(
            function_index: usize,
            function: &SelectedFunction,
        ) -> Result<(), PressureRematerializationError> {
            if !function.ids_are_positions() {
                return Err(PressureRematerializationError::FunctionMismatch {
                    function: function_index,
                });
            }
            let count = instruction_count(function_index, function)?;
            let mut ids = function
                .blocks
                .iter()
                .flat_map(block_instruction_ids)
                .collect::<Vec<_>>();
            ids.sort_unstable();
            if ids != (0..count).collect::<Vec<_>>() {
                return Err(PressureRematerializationError::FunctionMismatch {
                    function: function_index,
                });
            }
            Ok(())
        }

        pub(crate) fn instruction_count(
            function_index: usize,
            function: &SelectedFunction,
        ) -> Result<u32, PressureRematerializationError> {
            let count = crate::allocation::rewrites::function_scan(function).ok_or(
                PressureRematerializationError::IdentifierOverflow {
                    function: function_index,
                },
            )?;
            u32::try_from(count).map_err(|_| PressureRematerializationError::IdentifierOverflow {
                function: function_index,
            })
        }
    }
    pub(crate) mod work {
        use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
        use selected_instructions::SelectedInstructionPlan;

        use crate::allocation::{
            FunctionPressureRematerialization, PressureRematerializationError,
        };

        pub(crate) fn action_counts(
            functions: &[FunctionPressureRematerialization],
        ) -> Result<(usize, usize), PressureRematerializationError> {
            let applied = functions
                .iter()
                .filter(|function| function.action.is_some())
                .count();
            if applied == 0 {
                return Err(PressureRematerializationError::NoAction);
            }
            let rewritten_uses = functions
                .iter()
                .filter_map(|function| function.action.as_ref())
                .try_fold(0usize, |total, action| {
                    total.checked_add(action.rewrites.len())
                })
                .ok_or(PressureRematerializationError::WorkOverflow)?;
            Ok((applied, rewritten_uses))
        }

        pub(crate) fn required_usage(
            selected: &SelectedInstructionPlan,
            applied: usize,
            rewritten_uses: usize,
        ) -> Result<OptimizationWorkUsage, PressureRematerializationError> {
            let rule_evaluations = u64::try_from(selected.functions.len())
                .map_err(|_| PressureRematerializationError::WorkOverflow)?;
            let validation_steps = selected
                .functions
                .iter()
                .try_fold(0u64, |total, function| {
                    let instructions = function.blocks.iter().try_fold(0u64, |count, block| {
                        count.checked_add(
                            u64::try_from(block.instructions.len())
                                .ok()?
                                .checked_add(1)?,
                        )
                    })?;
                    total
                        .checked_add(u64::try_from(function.virtual_registers.len()).ok()?)?
                        .checked_add(instructions)
                })
                .ok_or(PressureRematerializationError::WorkOverflow)?
                .checked_add(
                    u64::try_from(rewritten_uses)
                        .map_err(|_| PressureRematerializationError::WorkOverflow)?,
                )
                .ok_or(PressureRematerializationError::WorkOverflow)?;
            let applied =
                u64::try_from(applied).map_err(|_| PressureRematerializationError::WorkOverflow)?;
            Ok(OptimizationWorkUsage {
                rule_evaluations,
                candidates: applied,
                validation_steps,
                commits: applied,
                iterations: 1,
            })
        }

        pub(crate) fn ensure_budget(
            usage: OptimizationWorkUsage,
            budget: OptimizationWorkBudget,
        ) -> Result<(), PressureRematerializationError> {
            crate::allocation::work_accounting::admit(usage, budget, |required, budget| {
                PressureRematerializationError::BudgetExceeded { required, budget }
            })
        }
    }

    use optimization_core::OptimizationWorkBudget;
    use register_model::{
        PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
        TargetRegisterEnvironmentIdentity,
    };

    use crate::allocation::{
        PressureRematerializationError, PressureRematerializationPlan,
        PressureRematerializationPolicy, ValidatedAllocationLegality,
        ValidatedAllocatorAvailability, ValidatedLiveRanges, ValidatedRecoveryClassifications,
        ValidatedSelectedAnalysis, ValidatedSpillChoices,
    };

    pub(crate) use function_plans::build_functions;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compute_terminal_pressure_rematerialization<S: ValidatedSelectedAnalysis>(
        selected: &S,
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
        spill_choices: &ValidatedSpillChoices,
        recovery: &ValidatedRecoveryClassifications,
        availability: &ValidatedAllocatorAvailability,
        register_environment: TargetRegisterEnvironmentIdentity,
        constraints: &PreparedRegisterConstraintCatalog,
        selected_keys: &TargetRegisterEnvironmentConstraintKeys,
        policy: PressureRematerializationPolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<ComputedPressureRematerialization, PressureRematerializationError> {
        let materialize = materialize_constraint::select(constraints, selected_keys)?;
        let (functions, transformed) = build_functions(
            selected.selected_plan(),
            ranges.plan(),
            recovery.plan(),
            materialize,
            policy,
        )?;
        let (applied, rewritten_uses) = work::action_counts(&functions)?;
        let usage = work::required_usage(selected.selected_plan(), applied, rewritten_uses)?;
        work::ensure_budget(usage, budget)?;

        let transformed = std::sync::Arc::new(transformed);
        let plan = PressureRematerializationPlan {
            source_selected: selected.shared_selected_plan(),
            spill_choices: spill_choices.receipt().identity(),
            recovery_classifications: recovery.receipt().identity(),
            ranges: ranges.receipt().identity(),
            legality: legality.receipt().identity(),
            register_environment,
            allocator_availability: availability.receipt().identity(),
            optimization_unit: selected.optimization_unit_identity(),
            work_schedule: selected.work_schedule_identity(),
            policy,
            budget,
            usage,
            functions,
            transformed_selected: std::sync::Arc::clone(&transformed),
        };
        Ok(ComputedPressureRematerialization {
            plan,
            transformed,
            applied,
            rewritten_uses,
        })
    }

    pub(crate) struct ComputedPressureRematerialization {
        pub(crate) plan: PressureRematerializationPlan,
        pub(crate) transformed: std::sync::Arc<selected_instructions::SelectedInstructionPlan>,
        pub(crate) applied: usize,
        pub(crate) rewritten_uses: usize,
    }
}
pub(crate) mod receipt {
    use crate::allocation::rewrites::pressure_rematerialization_identity;
    use crate::allocation::{PressureRematerializationPlan, PressureRematerializationReceipt};

    pub(crate) fn bind(
        plan: &PressureRematerializationPlan,
        function_count: usize,
        applied_count: usize,
        rewritten_use_count: usize,
    ) -> PressureRematerializationReceipt {
        PressureRematerializationReceipt {
            identity: pressure_rematerialization_identity(plan),
            spill_choices: plan.spill_choices,
            recovery_classifications: plan.recovery_classifications,
            ranges: plan.ranges,
            legality: plan.legality,
            register_environment: plan.register_environment,
            allocator_availability: plan.allocator_availability,
            optimization_unit: plan.optimization_unit,
            work_schedule: plan.work_schedule,
            policy: plan.policy,
            usage: plan.usage,
            function_count,
            applied_count,
            rewritten_use_count,
        }
    }
}
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RecoveryClassificationIdentity,
    SpillChoiceIdentity,
};
pub use register_homes::{
    FunctionPressureRematerialization, PressureRematerializationAction,
    PressureRematerializationPlan, PressureRematerializationPolicy,
    PressureRematerializationRewrite, pressure_rematerialization_identity,
};
use selected_instructions::{
    LiveRangeIdentity, PressureRematerializationIdentity, SelectedInstructionPlan,
};
use semantic_vocabulary::WorkScheduleIdentity;

pub fn rematerialize_selected_active_resident<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    constraints: &PreparedRegisterConstraintCatalog,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: PressureRematerializationPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedPressureRematerialization, PressureRematerializationError> {
    let compute::ComputedPressureRematerialization {
        plan,
        transformed,
        applied,
        rewritten_uses,
    } = compute::compute_terminal_pressure_rematerialization(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        constraints,
        selected_keys,
        policy,
        budget,
    )?;
    let receipt = receipt::bind(&plan, transformed.functions.len(), applied, rewritten_uses);
    Ok(ValidatedPressureRematerialization {
        plan,
        transformed,
        receipt,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureRematerializationReceipt {
    pub(crate) identity: PressureRematerializationIdentity,
    pub(crate) spill_choices: SpillChoiceIdentity,
    pub(crate) recovery_classifications: RecoveryClassificationIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) work_schedule: WorkScheduleIdentity,
    pub(crate) policy: PressureRematerializationPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) applied_count: usize,
    pub(crate) rewritten_use_count: usize,
}

impl PressureRematerializationReceipt {
    receipt_accessor_rows!(PressureRematerializationReceipt {
        identity: PressureRematerializationIdentity,
        spill_choices: SpillChoiceIdentity,
        recovery_classifications: RecoveryClassificationIdentity,
        policy: PressureRematerializationPolicy,
        usage: OptimizationWorkUsage,
        function_count: usize,
        applied_count: usize,
        rewritten_use_count: usize,
    });
    shared_analysis_identity_accessors!(pub);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPressureRematerialization {
    pub(crate) plan: PressureRematerializationPlan,
    pub(crate) transformed: std::sync::Arc<SelectedInstructionPlan>,
    pub(crate) receipt: PressureRematerializationReceipt,
}

impl ValidatedPressureRematerialization {
    pub const fn plan(&self) -> &PressureRematerializationPlan {
        &self.plan
    }
    pub fn shared_transformed(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        std::sync::Arc::clone(&self.transformed)
    }

    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }
    receipt_accessor_rows!(ref ValidatedPressureRematerialization {
        receipt: PressureRematerializationReceipt,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PressureRematerializationError {
    RootMismatch,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    FunctionMismatch {
        function: usize,
    },
    FutureUseMismatch {
        function: usize,
    },
    MaterializeMismatch {
        function: usize,
    },
    MaterializeConstraintMismatch,
    IdentifierOverflow {
        function: usize,
    },
    DecisionMismatch {
        function: usize,
    },
    NoAction,
}

failed_display!(
    PressureRematerializationError,
    "terminal pressure rematerialization failed"
);
