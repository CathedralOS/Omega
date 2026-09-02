//! Source-custody preflight, work accounting, and insertion-site discovery.

use super::*;

pub(super) fn validate_roots(
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), FixedViewCopyError> {
    if ranges.plan().selected != selected.receipt().identity()
        || ranges.plan().optimization_unit != selected.receipt().optimization_unit()
        || ranges.plan().fuel_schedule != selected.receipt().fuel_schedule()
        || ranges.plan().target != selected.plan().target
        || legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != selected.plan().target
        || target_register_environment_identity(
            selected.plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
        || selected.plan().functions.len() != legality.plan().functions.len()
    {
        return Err(FixedViewCopyError::RootMismatch);
    }
    Ok(())
}

pub(super) fn copy_row(
    constraints: &ValidatedRegisterConstraintCatalog,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<&RegisterInstructionConstraint, FixedViewCopyError> {
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == selected_keys.copy_i64)
        .ok_or(FixedViewCopyError::CopyConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Def
        || row.operands[0].class != row.operands[1].class
        || row.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(FixedViewCopyError::CopyConstraintMismatch);
    }
    Ok(row)
}

pub(super) fn work_usage(
    selected: &ValidatedSelectedInstructions,
    legality: &ValidatedAllocationLegality,
    policy: FixedViewCopyPolicy,
) -> Result<OptimizationWorkUsage, FixedViewCopyError> {
    let functions = u64::try_from(selected.plan().functions.len())
        .map_err(|_| FixedViewCopyError::WorkOverflow)?;
    let requirements = legality
        .plan()
        .functions
        .iter()
        .flat_map(|function| &function.virtual_registers)
        .map(|register| register.entry_transitions.len())
        .try_fold(0_u64, |total, count| {
            total.checked_add(u64::try_from(count).ok()?)
        })
        .ok_or(FixedViewCopyError::WorkOverflow)?;
    let commits = match policy {
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => requirements,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => legality
            .plan()
            .functions
            .iter()
            .try_fold(0_u64, |count, function| {
                let has_transitions = function
                    .virtual_registers
                    .iter()
                    .any(|r| !r.entry_transitions.is_empty());
                count.checked_add(u64::from(has_transitions))
            })
            .ok_or(FixedViewCopyError::WorkOverflow)?,
    };
    Ok(OptimizationWorkUsage {
        rule_evaluations: functions,
        candidates: requirements,
        validation_steps: requirements,
        commits,
        iterations: 1,
    })
}

pub(super) fn next_instruction_id(
    function_index: usize,
    function: &omega_selected_instructions::SelectedFunction,
) -> Result<u32, FixedViewCopyError> {
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
                    | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                    | SelectedTerminator::Return { instruction, .. } => instruction.id.0,
                }))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    if ids
        != (0..u32::try_from(ids.len()).map_err(|_| FixedViewCopyError::IdentifierOverflow {
            function: function_index,
        })?)
            .collect::<Vec<_>>()
    {
        return Err(FixedViewCopyError::FunctionMismatch {
            function: function_index,
        });
    }
    u32::try_from(ids.len()).map_err(|_| FixedViewCopyError::IdentifierOverflow {
        function: function_index,
    })
}

pub(super) fn next_register_id(
    function_index: usize,
    function: &omega_selected_instructions::SelectedFunction,
) -> Result<u32, FixedViewCopyError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
    {
        return Err(FixedViewCopyError::FunctionMismatch {
            function: function_index,
        });
    }
    u32::try_from(function.virtual_registers.len()).map_err(|_| {
        FixedViewCopyError::IdentifierOverflow {
            function: function_index,
        }
    })
}

pub(super) fn find_leaf_block(
    function_index: usize,
    function: &omega_selected_instructions::SelectedFunction,
    instruction: SelectedInstructionId,
    operand: u16,
    source: VirtualRegisterId,
    to_view: omega_register_model::RegisterViewId,
) -> Result<omega_selected_instructions::SelectedBlockId, FixedViewCopyError> {
    for block in &function.blocks {
        let SelectedTerminator::Return {
            instruction: destination,
            ..
        } = &block.terminator
        else {
            continue;
        };
        if destination.id != instruction {
            continue;
        }
        if block.id == function.entry_block {
            return Err(FixedViewCopyError::NonLeafDestination {
                function: function_index,
                instruction: instruction.0,
            });
        }
        let Some(destination_operand) = destination
            .operands
            .iter()
            .find(|candidate| candidate.operand == operand)
        else {
            return Err(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            });
        };
        if destination_operand.virtual_register != source
            || destination_operand.access != RegisterOperandAccess::Use
            || destination_operand.fixed_view != Some(to_view)
        {
            return Err(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            });
        }
        return Ok(block.id);
    }
    Err(FixedViewCopyError::MissingDestination {
        function: function_index,
        instruction: instruction.0,
    })
}
