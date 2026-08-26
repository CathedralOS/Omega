use std::collections::BTreeSet;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_terminal_selected_instructions::{
    TerminalSelectedInstruction, TerminalSelectedInstructionId, TerminalSelectedInstructionKind,
    TerminalSelectedInstructionProvenance, TerminalSelectedOperand, TerminalSelectedTerminator,
    TerminalVirtualRegister, TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
};
use omega_terminal_target_operations_to_selected_instructions::ValidatedTerminalSelectedInstructions;
use psi_core::{IntegerSign, ScalarType};

use crate::{
    TerminalFixedViewCopy, TerminalFixedViewCopyError, TerminalFixedViewCopyPlan,
    TerminalFixedViewCopyPolicy, TerminalVirtualFixedConstraintSite,
    ValidatedTerminalAllocationLegality, ValidatedTerminalLiveRanges,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_fixed_view_copies(
    selected: &ValidatedTerminalSelectedInstructions,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: TerminalFixedViewCopyPolicy,
    budget: OptimizationWorkBudget,
) -> Result<TerminalFixedViewCopyPlan, TerminalFixedViewCopyError> {
    validate_roots(
        selected,
        ranges,
        legality,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    if policy != TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 {
        return Err(TerminalFixedViewCopyError::UnsupportedPolicy);
    }
    let copy_row = copy_row(constraints, selected_keys)?;
    let usage = work_usage(selected, legality)?;
    if !usage.within(budget) {
        return Err(TerminalFixedViewCopyError::BudgetExceeded {
            required: usage,
            budget,
        });
    }

    let mut transformed = selected.plan().clone();
    let mut copies = Vec::new();
    for (function_index, (source_function, legality_function)) in selected
        .plan()
        .functions
        .iter()
        .zip(&legality.plan().functions)
        .enumerate()
    {
        if source_function.machine != legality_function.machine {
            return Err(TerminalFixedViewCopyError::FunctionMismatch {
                function: function_index,
            });
        }
        let mut next_instruction = next_instruction_id(function_index, source_function)?;
        let mut next_register = next_register_id(function_index, source_function)?;
        let mut destinations = BTreeSet::new();
        for legality_register in &legality_function.virtual_registers {
            for transition in &legality_register.entry_transitions {
                let TerminalVirtualFixedConstraintSite::Operand {
                    instruction,
                    operand,
                    access: RegisterOperandAccess::Use,
                    ..
                } = transition.to_site
                else {
                    return Err(TerminalFixedViewCopyError::UnsupportedTransitionSite {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                };
                if !destinations.insert((instruction, operand)) {
                    return Err(TerminalFixedViewCopyError::NonCanonicalCopies);
                }
                let source_register = source_function
                    .virtual_registers
                    .get(
                        usize::try_from(legality_register.virtual_register.0).map_err(|_| {
                            TerminalFixedViewCopyError::UnsupportedSourceRegister {
                                function: function_index,
                                register: legality_register.virtual_register.0,
                            }
                        })?,
                    )
                    .filter(|register| {
                        register.id == legality_register.virtual_register
                            && register.class == legality_register.class
                            && register.entry_fixed_view == Some(transition.from_view)
                    })
                    .ok_or(TerminalFixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    })?;
                let TerminalVirtualRegisterOrigin::EntryParameter { source_value, .. } =
                    source_register.origin
                else {
                    return Err(TerminalFixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                };
                if !is_u64(source_register.scalar_type)
                    || copy_row.operands[0].class != source_register.class
                    || copy_row.operands[1].class != source_register.class
                {
                    return Err(TerminalFixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                }
                let function_u32 = u32::try_from(function_index).map_err(|_| {
                    TerminalFixedViewCopyError::IdentifierOverflow {
                        function: function_index,
                    }
                })?;
                let copy = TerminalFixedViewCopy {
                    function: function_u32,
                    machine: source_function.machine,
                    source_virtual_register: source_register.id,
                    source_value,
                    source_definition_site: source_register.definition_site,
                    from_view: transition.from_view,
                    destination_site: transition.to_site,
                    to_view: transition.to_view,
                    block: find_leaf_block(
                        function_index,
                        source_function,
                        instruction,
                        operand,
                        source_register.id,
                        transition.to_view,
                    )?,
                    before_instruction: instruction,
                    copy_instruction: TerminalSelectedInstructionId(next_instruction),
                    result_virtual_register: TerminalVirtualRegisterId(next_register),
                    copy_constraint: selected_keys.copy_i64,
                };
                apply_copy(
                    function_index,
                    &mut transformed.functions[function_index],
                    copy,
                    source_register,
                    copy_row,
                    operand,
                )?;
                copies.push(copy);
                next_instruction = next_instruction.checked_add(1).ok_or(
                    TerminalFixedViewCopyError::IdentifierOverflow {
                        function: function_index,
                    },
                )?;
                next_register = next_register.checked_add(1).ok_or(
                    TerminalFixedViewCopyError::IdentifierOverflow {
                        function: function_index,
                    },
                )?;
            }
        }
    }

    Ok(TerminalFixedViewCopyPlan {
        source_selected: selected.receipt().identity(),
        source_ranges: ranges.receipt().identity(),
        source_legality: legality.receipt().identity(),
        register_environment,
        policy,
        budget,
        usage,
        copies,
        transformed,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_roots(
    selected: &ValidatedTerminalSelectedInstructions,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), TerminalFixedViewCopyError> {
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
        return Err(TerminalFixedViewCopyError::RootMismatch);
    }
    Ok(())
}

fn copy_row(
    constraints: &ValidatedRegisterConstraintCatalog,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<&RegisterInstructionConstraint, TerminalFixedViewCopyError> {
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == selected_keys.copy_i64)
        .ok_or(TerminalFixedViewCopyError::CopyConstraintMismatch)?;
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
        return Err(TerminalFixedViewCopyError::CopyConstraintMismatch);
    }
    Ok(row)
}

fn work_usage(
    selected: &ValidatedTerminalSelectedInstructions,
    legality: &ValidatedTerminalAllocationLegality,
) -> Result<OptimizationWorkUsage, TerminalFixedViewCopyError> {
    let functions = u64::try_from(selected.plan().functions.len())
        .map_err(|_| TerminalFixedViewCopyError::WorkOverflow)?;
    let requirements = legality
        .plan()
        .functions
        .iter()
        .flat_map(|function| &function.virtual_registers)
        .map(|register| register.entry_transitions.len())
        .try_fold(0_u64, |total, count| {
            total.checked_add(u64::try_from(count).ok()?)
        })
        .ok_or(TerminalFixedViewCopyError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: functions,
        candidates: requirements,
        validation_steps: requirements,
        commits: requirements,
        iterations: 1,
    })
}

fn next_instruction_id(
    function_index: usize,
    function: &omega_terminal_selected_instructions::TerminalSelectedFunction,
) -> Result<u32, TerminalFixedViewCopyError> {
    let mut ids = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain(std::iter::once(match &block.terminator {
                    TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
                    | TerminalSelectedTerminator::Return { instruction, .. } => instruction.id.0,
                }))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    if ids
        != (0..u32::try_from(ids.len()).map_err(|_| {
            TerminalFixedViewCopyError::IdentifierOverflow {
                function: function_index,
            }
        })?)
            .collect::<Vec<_>>()
    {
        return Err(TerminalFixedViewCopyError::FunctionMismatch {
            function: function_index,
        });
    }
    u32::try_from(ids.len()).map_err(|_| TerminalFixedViewCopyError::IdentifierOverflow {
        function: function_index,
    })
}

fn next_register_id(
    function_index: usize,
    function: &omega_terminal_selected_instructions::TerminalSelectedFunction,
) -> Result<u32, TerminalFixedViewCopyError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
    {
        return Err(TerminalFixedViewCopyError::FunctionMismatch {
            function: function_index,
        });
    }
    u32::try_from(function.virtual_registers.len()).map_err(|_| {
        TerminalFixedViewCopyError::IdentifierOverflow {
            function: function_index,
        }
    })
}

fn find_leaf_block(
    function_index: usize,
    function: &omega_terminal_selected_instructions::TerminalSelectedFunction,
    instruction: TerminalSelectedInstructionId,
    operand: u16,
    source: TerminalVirtualRegisterId,
    to_view: omega_register_model::RegisterViewId,
) -> Result<omega_terminal_selected_instructions::TerminalSelectedBlockId, TerminalFixedViewCopyError>
{
    for block in &function.blocks {
        let TerminalSelectedTerminator::Return {
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
            return Err(TerminalFixedViewCopyError::NonLeafDestination {
                function: function_index,
                instruction: instruction.0,
            });
        }
        let Some(destination_operand) = destination
            .operands
            .iter()
            .find(|candidate| candidate.operand == operand)
        else {
            return Err(TerminalFixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            });
        };
        if destination_operand.virtual_register != source
            || destination_operand.access != RegisterOperandAccess::Use
            || destination_operand.fixed_view != Some(to_view)
        {
            return Err(TerminalFixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            });
        }
        return Ok(block.id);
    }
    Err(TerminalFixedViewCopyError::MissingDestination {
        function: function_index,
        instruction: instruction.0,
    })
}

fn apply_copy(
    function_index: usize,
    function: &mut omega_terminal_selected_instructions::TerminalSelectedFunction,
    copy: TerminalFixedViewCopy,
    source: &TerminalVirtualRegister,
    row: &RegisterInstructionConstraint,
    destination_operand: u16,
) -> Result<(), TerminalFixedViewCopyError> {
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == copy.block)
        .ok_or(TerminalFixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: copy.before_instruction.0,
        })?;
    let TerminalSelectedTerminator::Return { instruction, .. } = &mut block.terminator else {
        return Err(TerminalFixedViewCopyError::NonLeafDestination {
            function: function_index,
            instruction: copy.before_instruction.0,
        });
    };
    let operand = instruction
        .operands
        .iter_mut()
        .find(|operand| operand.operand == destination_operand)
        .ok_or(TerminalFixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: copy.before_instruction.0,
        })?;
    operand.virtual_register = copy.result_virtual_register;
    function.virtual_registers.push(TerminalVirtualRegister {
        id: copy.result_virtual_register,
        scalar_type: source.scalar_type,
        class: source.class,
        origin: TerminalVirtualRegisterOrigin::InstructionResult {
            instruction: copy.copy_instruction,
            source_value: copy.source_value,
        },
        definition_site: copy.source_definition_site,
        entry_fixed_view: None,
    });
    block.instructions.push(TerminalSelectedInstruction {
        id: copy.copy_instruction,
        kind: TerminalSelectedInstructionKind::CopyI64,
        constraint: copy.copy_constraint,
        operands: vec![
            selected_operand(&row.operands[0], copy.source_virtual_register),
            selected_operand(&row.operands[1], copy.result_virtual_register),
        ],
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: TerminalSelectedInstructionProvenance {
            operations: Vec::new(),
            values: vec![copy.source_value],
            edges: Vec::new(),
            obligations: Vec::new(),
            fuel: Vec::new(),
        },
    });
    Ok(())
}

fn selected_operand(
    constraint: &omega_register_model::RegisterOperandConstraint,
    register: TerminalVirtualRegisterId,
) -> TerminalSelectedOperand {
    TerminalSelectedOperand {
        operand: constraint.operand,
        virtual_register: register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}

fn is_u64(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Integer(integer)
            if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64
    )
}
