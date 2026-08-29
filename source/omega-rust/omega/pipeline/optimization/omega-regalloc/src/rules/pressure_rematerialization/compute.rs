use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedOperand, SelectedTerminator,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;

use crate::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_pressure_rematerialization<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: PressureRematerializationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<PressureRematerializationPlan, PressureRematerializationError> {
    validate_roots(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    let materialize = materialize_row(constraints, selected_keys)?;
    let (functions, transformed) = build_functions(
        selected.selected_plan(),
        ranges.plan(),
        recovery.plan(),
        materialize,
        policy,
    )?;
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
    let usage = required_usage(selected.selected_plan(), applied, rewritten_uses)?;
    ensure_budget(usage, budget)?;
    Ok(PressureRematerializationPlan {
        source_selected: selected.selected_identity(),
        spill_choices: spill_choices.receipt().identity(),
        recovery_classifications: recovery.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment,
        allocator_availability: availability.receipt().identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        policy,
        budget,
        usage,
        functions,
        transformed_selected: selected_instruction_plan_identity(&transformed),
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), PressureRematerializationError> {
    if ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
        || legality.receipt().allocator_availability() != availability.receipt().identity()
        || availability.receipt().register_environment() != register_environment
        || availability.receipt().physical() != physical.identity()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != selected.selected_plan().target
        || target_register_environment_identity(
            selected.selected_plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
        || spill_choices.receipt().ranges() != ranges.receipt().identity()
        || spill_choices.receipt().legality() != legality.receipt().identity()
        || spill_choices.receipt().register_environment() != register_environment
        || spill_choices.receipt().allocator_availability() != availability.receipt().identity()
        || recovery.receipt().selected() != selected.selected_identity()
        || recovery.receipt().ranges() != ranges.receipt().identity()
        || recovery.receipt().legality() != legality.receipt().identity()
        || recovery.receipt().spill_choices() != spill_choices.receipt().identity()
        || recovery.receipt().register_environment() != register_environment
        || recovery.receipt().allocator_availability() != availability.receipt().identity()
        || recovery.receipt().optimization_unit() != selected.optimization_unit_identity()
        || recovery.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || selected.selected_plan().functions.len() != recovery.plan().functions.len()
        || selected.selected_plan().functions.len() != ranges.plan().functions.len()
    {
        return Err(PressureRematerializationError::RootMismatch);
    }
    Ok(())
}

fn materialize_row(
    constraints: &ValidatedRegisterConstraintCatalog,
    keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<&RegisterInstructionConstraint, PressureRematerializationError> {
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == keys.materialize_i64)
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
        if source.machine != range_function.machine || source.machine != recovery_function.machine {
            return Err(PressureRematerializationError::FunctionMismatch { function: index });
        }
        validate_dense(index, source)?;
        let action = match &recovery_function.classification {
            None => None,
            Some(candidate) => Some(action_from_candidate(
                index,
                source,
                range_function,
                candidate,
                row,
                policy,
            )?),
        };
        if let Some(action) = &action {
            apply_action(index, &mut transformed.functions[index], action, row)?;
        }
        functions.push(FunctionPressureRematerialization {
            machine: source.machine,
            action,
        });
    }
    Ok((functions, transformed))
}

fn action_from_candidate(
    function_index: usize,
    function: &SelectedFunction,
    ranges: &FunctionLiveRanges,
    candidate: &PressureRecoveryClassification,
    row: &RegisterInstructionConstraint,
    policy: PressureRematerializationPolicy,
) -> Result<PressureRematerializationAction, PressureRematerializationError> {
    let RecoveryVictimRole::ActiveResident {
        current_view,
        reclaimed_view,
    } = candidate.role
    else {
        return Err(PressureRematerializationError::UnsupportedVictimRole {
            function: function_index,
        });
    };
    let RecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        source_value,
        value,
        provenance,
        future_uses,
    } = &candidate.classification
    else {
        return Err(PressureRematerializationError::ClassificationNotAdmitted {
            function: function_index,
        });
    };
    let valid_arity = match policy {
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 => future_uses.len() == 1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1 => future_uses.len() >= 2,
    };
    if !valid_arity
        || future_uses.windows(2).any(|pair| pair[0] >= pair[1])
        || future_uses
            .iter()
            .any(|future| future.block != candidate.block || future.point <= candidate.point)
    {
        return Err(PressureRematerializationError::FutureUseMismatch {
            function: function_index,
        });
    }
    let victim = function
        .virtual_registers
        .iter()
        .find(|register| register.id == candidate.victim)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
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
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.block)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
    let original = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == *defining_instruction)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
    if original.kind != (SelectedInstructionKind::MaterializeI64 { value: *value })
        || original.constraint != row.key
        || original.provenance != *provenance
        || original.operands.as_slice() != [selected_operand(&row.operands[0], candidate.victim)]
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
            && occurrence.access == RegisterOperandAccess::Def
            && occurrence.point < candidate.point
    }) {
        return Err(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        });
    }
    for future in future_uses {
        let future_instruction = find_instruction(block, future.instruction).ok_or(
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
    let fresh_instruction = SelectedInstructionId(instruction_count(function_index, function)?);
    let fresh_register = VirtualRegisterId(
        u32::try_from(function.virtual_registers.len()).map_err(|_| {
            PressureRematerializationError::IdentifierOverflow {
                function: function_index,
            }
        })?,
    );
    Ok(PressureRematerializationAction {
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
    })
}

fn apply_action(
    function_index: usize,
    function: &mut SelectedFunction,
    action: &PressureRematerializationAction,
    row: &RegisterInstructionConstraint,
) -> Result<(), PressureRematerializationError> {
    let source = function
        .virtual_registers
        .iter()
        .find(|register| register.id == action.victim)
        .cloned()
        .ok_or(PressureRematerializationError::DecisionMismatch {
            function: function_index,
        })?;
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
        operands: vec![selected_operand(
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
    let first =
        action
            .rewrites
            .first()
            .ok_or(PressureRematerializationError::DecisionMismatch {
                function: function_index,
            })?;
    if let Some(index) = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == first.instruction)
    {
        block.instructions.insert(index, new_instruction);
    } else {
        let terminator_id = match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction.id,
        };
        if terminator_id != first.instruction {
            return Err(PressureRematerializationError::DecisionMismatch {
                function: function_index,
            });
        }
        block.instructions.push(new_instruction);
    }
    Ok(())
}

fn rewrite_operand(
    function: usize,
    block: &mut omega_selected_instructions::SelectedBlock,
    victim: VirtualRegisterId,
    result: VirtualRegisterId,
    rewrite: PressureRematerializationRewrite,
) -> Result<(), PressureRematerializationError> {
    let instruction = block
        .instructions
        .iter_mut()
        .find(|instruction| instruction.id == rewrite.instruction)
        .or_else(|| {
            let terminator = match &mut block.terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. }
                | SelectedTerminator::Return { instruction, .. } => instruction,
            };
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

fn selected_operand(
    constraint: &omega_register_model::RegisterOperandConstraint,
    register: VirtualRegisterId,
) -> SelectedOperand {
    SelectedOperand {
        operand: constraint.operand,
        virtual_register: register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}

fn find_instruction(
    block: &omega_selected_instructions::SelectedBlock,
    id: SelectedInstructionId,
) -> Option<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .find(|instruction| instruction.id == id)
        .or_else(|| match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
                if instruction.id == id =>
            {
                Some(instruction)
            }
            _ => None,
        })
}

fn validate_dense(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<(), PressureRematerializationError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
    {
        return Err(PressureRematerializationError::FunctionMismatch {
            function: function_index,
        });
    }
    let count = instruction_count(function_index, function)?;
    let mut ids = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain(std::iter::once(match &block.terminator {
                    SelectedTerminator::ConditionalBranch { instruction, .. }
                    | SelectedTerminator::Return { instruction, .. } => instruction.id.0,
                }))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    if ids != (0..count).collect::<Vec<_>>() {
        return Err(PressureRematerializationError::FunctionMismatch {
            function: function_index,
        });
    }
    Ok(())
}

fn instruction_count(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<u32, PressureRematerializationError> {
    let count = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len().checked_add(1)?)
        })
        .ok_or(PressureRematerializationError::IdentifierOverflow {
            function: function_index,
        })?;
    u32::try_from(count).map_err(|_| PressureRematerializationError::IdentifierOverflow {
        function: function_index,
    })
}

pub(super) fn required_usage(
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

pub(super) fn ensure_budget(
    usage: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
) -> Result<(), PressureRematerializationError> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(PressureRematerializationError::BudgetExceeded {
            required: usage,
            budget,
        })
    }
}
