use std::collections::BTreeSet;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use omega_target_operations_to_selected_instructions::ValidatedSelectedInstructions;
use psi_core::{IntegerSign, ScalarType};

use crate::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPlan,
    FixedViewCopyPolicy, FunctionAllocationLegality, ValidatedAllocationLegality,
    ValidatedLiveRanges, VirtualFixedConstraintSite,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_fixed_view_copies(
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: FixedViewCopyPolicy,
    budget: OptimizationWorkBudget,
) -> Result<FixedViewCopyPlan, FixedViewCopyError> {
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
    let copy_row = copy_row(constraints, selected_keys)?;
    let usage = work_usage(selected, legality, policy)?;
    if !usage.within(budget) {
        return Err(FixedViewCopyError::BudgetExceeded {
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
            return Err(FixedViewCopyError::FunctionMismatch {
                function: function_index,
            });
        }
        let mut next_instruction = next_instruction_id(function_index, source_function)?;
        let mut next_register = next_register_id(function_index, source_function)?;
        if policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 {
            if let Some(copy) = build_shared_entry_copy(
                function_index,
                source_function,
                legality_function,
                copy_row,
                selected_keys.copy_i64,
                next_instruction,
                next_register,
            )? {
                apply_copy(
                    function_index,
                    &mut transformed.functions[function_index],
                    &copy,
                    copy_row,
                )?;
                copies.push(copy);
            }
            continue;
        }
        let mut destinations = BTreeSet::new();
        for legality_register in &legality_function.virtual_registers {
            for transition in &legality_register.entry_transitions {
                let VirtualFixedConstraintSite::Operand {
                    instruction,
                    operand,
                    access: RegisterOperandAccess::Use,
                    ..
                } = transition.to_site
                else {
                    return Err(FixedViewCopyError::UnsupportedTransitionSite {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                };
                if !destinations.insert((instruction, operand)) {
                    return Err(FixedViewCopyError::NonCanonicalCopies);
                }
                let source_register = source_function
                    .virtual_registers
                    .get(
                        usize::try_from(legality_register.virtual_register.0).map_err(|_| {
                            FixedViewCopyError::UnsupportedSourceRegister {
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
                    .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    })?;
                let VirtualRegisterOrigin::EntryParameter { source_value, .. } =
                    source_register.origin
                else {
                    return Err(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                };
                if !is_u64(source_register.scalar_type)
                    || copy_row.operands[0].class != source_register.class
                    || copy_row.operands[1].class != source_register.class
                {
                    return Err(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                }
                let function_u32 = u32::try_from(function_index).map_err(|_| {
                    FixedViewCopyError::IdentifierOverflow {
                        function: function_index,
                    }
                })?;
                let copy = FixedViewCopy {
                    function: function_u32,
                    machine: source_function.machine,
                    source_virtual_register: source_register.id,
                    source_value,
                    source_definition_site: source_register.definition_site,
                    from_view: transition.from_view,
                    to_view: transition.to_view,
                    insertion_block: find_leaf_block(
                        function_index,
                        source_function,
                        instruction,
                        operand,
                        source_register.id,
                        transition.to_view,
                    )?,
                    before_instruction: instruction,
                    destinations: vec![FixedViewCopyDestination {
                        site: transition.to_site,
                        block: find_leaf_block(
                            function_index,
                            source_function,
                            instruction,
                            operand,
                            source_register.id,
                            transition.to_view,
                        )?,
                        view: transition.to_view,
                    }],
                    copy_instruction: SelectedInstructionId(next_instruction),
                    result_virtual_register: VirtualRegisterId(next_register),
                    copy_constraint: selected_keys.copy_i64,
                };
                apply_copy(
                    function_index,
                    &mut transformed.functions[function_index],
                    &copy,
                    copy_row,
                )?;
                copies.push(copy);
                next_instruction = next_instruction.checked_add(1).ok_or(
                    FixedViewCopyError::IdentifierOverflow {
                        function: function_index,
                    },
                )?;
                next_register =
                    next_register
                        .checked_add(1)
                        .ok_or(FixedViewCopyError::IdentifierOverflow {
                            function: function_index,
                        })?;
            }
        }
    }

    Ok(FixedViewCopyPlan {
        source_selected: selected.receipt().identity(),
        source_ranges: ranges.receipt().identity(),
        source_legality: legality.receipt().identity(),
        register_environment,
        allocator_availability: legality.receipt().allocator_availability(),
        policy,
        budget,
        usage,
        copies,
        transformed,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_roots(
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

fn copy_row(
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

fn work_usage(
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

fn next_instruction_id(
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

fn next_register_id(
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

fn find_leaf_block(
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

#[allow(clippy::too_many_arguments)]
fn build_shared_entry_copy(
    function_index: usize,
    function: &omega_selected_instructions::SelectedFunction,
    legality: &FunctionAllocationLegality,
    row: &RegisterInstructionConstraint,
    copy_constraint: omega_register_model::RegisterConstraintKey,
    next_instruction: u32,
    next_register: u32,
) -> Result<Option<FixedViewCopy>, FixedViewCopyError> {
    let transitions = legality
        .virtual_registers
        .iter()
        .flat_map(|register| {
            register
                .entry_transitions
                .iter()
                .map(move |transition| (register, transition))
        })
        .collect::<Vec<_>>();
    if transitions.is_empty() {
        return Ok(None);
    }
    if transitions.len() != 2
        || transitions[0].0.virtual_register != transitions[1].0.virtual_register
        || transitions[0].1.from_view != transitions[1].1.from_view
        || transitions[0].1.to_view != transitions[1].1.to_view
    {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let legality_register = transitions[0].0;
    let source = function
        .virtual_registers
        .get(
            usize::try_from(legality_register.virtual_register.0).map_err(|_| {
                FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: legality_register.virtual_register.0,
                }
            })?,
        )
        .filter(|source| {
            source.id == legality_register.virtual_register
                && source.class == legality_register.class
                && source.entry_fixed_view == Some(transitions[0].1.from_view)
        })
        .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: legality_register.virtual_register.0,
        })?;
    let VirtualRegisterOrigin::EntryParameter { source_value, .. } = source.origin else {
        return Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: source.id.0,
        });
    };
    if !is_u64(source.scalar_type)
        || row.operands[0].class != source.class
        || row.operands[1].class != source.class
    {
        return Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: source.id.0,
        });
    }
    let entry = function
        .blocks
        .iter()
        .find(|block| block.id == function.entry_block)
        .ok_or(FixedViewCopyError::FunctionMismatch {
            function: function_index,
        })?;
    if entry.instructions.len() != 1
        || entry.instructions[0].kind != SelectedInstructionKind::CompareI64Zero
    {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let SelectedTerminator::ConditionalBranch {
        instruction: branch,
        when_nonzero,
        when_zero,
    } = &entry.terminator
    else {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    };
    if when_nonzero.block == when_zero.block {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let successor_blocks = BTreeSet::from([when_nonzero.block, when_zero.block]);
    let from_view = transitions[0].1.from_view;
    let mut destinations = Vec::with_capacity(2);
    let mut destination_blocks = BTreeSet::new();
    for (_, transition) in transitions {
        let VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access: RegisterOperandAccess::Use,
            ..
        } = transition.to_site
        else {
            return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        };
        let block = find_leaf_block(
            function_index,
            function,
            instruction,
            operand,
            source.id,
            transition.to_view,
        )?;
        let leaf = function
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
            .ok_or(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?;
        if !leaf.instructions.is_empty() || !destination_blocks.insert(block) {
            return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        }
        destinations.push(FixedViewCopyDestination {
            site: transition.to_site,
            block,
            view: transition.to_view,
        });
    }
    destinations.sort_by_key(|destination| match destination.site {
        VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            ..
        } => (instruction.0, operand),
        VirtualFixedConstraintSite::Entry => (u32::MAX, u16::MAX),
    });
    if destination_blocks != successor_blocks {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    Ok(Some(FixedViewCopy {
        function: u32::try_from(function_index).map_err(|_| {
            FixedViewCopyError::IdentifierOverflow {
                function: function_index,
            }
        })?,
        machine: function.machine,
        source_virtual_register: source.id,
        source_value,
        source_definition_site: source.definition_site,
        from_view,
        to_view: destinations[0].view,
        insertion_block: entry.id,
        before_instruction: branch.id,
        destinations,
        copy_instruction: SelectedInstructionId(next_instruction),
        result_virtual_register: VirtualRegisterId(next_register),
        copy_constraint,
    }))
}

fn apply_copy(
    function_index: usize,
    function: &mut omega_selected_instructions::SelectedFunction,
    copy: &FixedViewCopy,
    row: &RegisterInstructionConstraint,
) -> Result<(), FixedViewCopyError> {
    let source = function
        .virtual_registers
        .iter()
        .find(|register| register.id == copy.source_virtual_register)
        .cloned()
        .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: copy.source_virtual_register.0,
        })?;
    for destination in &copy.destinations {
        let VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            ..
        } = destination.site
        else {
            return Err(FixedViewCopyError::UnsupportedTransitionSite {
                function: function_index,
                register: copy.source_virtual_register.0,
            });
        };
        let destination_block = function
            .blocks
            .iter_mut()
            .find(|block| block.id == destination.block)
            .ok_or(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?;
        let SelectedTerminator::Return {
            instruction: return_instruction,
            ..
        } = &mut destination_block.terminator
        else {
            return Err(FixedViewCopyError::NonLeafDestination {
                function: function_index,
                instruction: instruction.0,
            });
        };
        return_instruction
            .operands
            .iter_mut()
            .find(|candidate| candidate.operand == operand)
            .ok_or(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?
            .virtual_register = copy.result_virtual_register;
    }
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == copy.insertion_block)
        .ok_or(FixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: copy.before_instruction.0,
        })?;
    if terminator_instruction_id(&block.terminator) != copy.before_instruction {
        return Err(FixedViewCopyError::InvalidInsertionSite {
            function: function_index,
            instruction: copy.before_instruction.0,
        });
    }
    function.virtual_registers.push(VirtualRegister {
        id: copy.result_virtual_register,
        scalar_type: source.scalar_type,
        class: source.class,
        origin: match source.origin {
            VirtualRegisterOrigin::LegalizationTemporary { temporary, .. } => {
                VirtualRegisterOrigin::LegalizationTemporary {
                    instruction: copy.copy_instruction,
                    temporary,
                    source_value: copy.source_value,
                }
            }
            _ => VirtualRegisterOrigin::InstructionResult {
                instruction: copy.copy_instruction,
                source_value: copy.source_value,
            },
        },
        definition_site: copy.source_definition_site,
        entry_fixed_view: None,
    });
    block.instructions.push(SelectedInstruction {
        id: copy.copy_instruction,
        kind: SelectedInstructionKind::CopyI64,
        constraint: copy.copy_constraint,
        operands: vec![
            selected_operand(&row.operands[0], copy.source_virtual_register),
            selected_operand(&row.operands[1], copy.result_virtual_register),
        ],
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: SelectedInstructionProvenance {
            operations: Vec::new(),
            values: vec![copy.source_value],
            edges: Vec::new(),
            obligations: Vec::new(),
            fuel: Vec::new(),
        },
    });
    Ok(())
}

fn terminator_instruction_id(terminator: &SelectedTerminator) -> SelectedInstructionId {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction.id,
    }
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

fn is_u64(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Integer(integer)
            if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use omega_optimization_unit::ValueDefinitionSite;
    use omega_register_model::{
        RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
        RegisterOperandConstraint, RegisterViewId,
    };
    use omega_selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedFunction, SelectedSuccessor,
    };
    use psi_core::{BlockId, EdgeId, IntegerType, MachineId, ValueId};

    use super::*;
    use crate::{
        EntryFixedViewTransition, FunctionAllocationLegality, LiveRangePoint, LivenessPosition,
        VirtualRegisterAllocationLegality,
    };

    fn key(variant: u32) -> RegisterConstraintKey {
        RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant,
        }
    }

    fn instruction(
        id: u32,
        kind: SelectedInstructionKind,
        operands: Vec<SelectedOperand>,
    ) -> SelectedInstruction {
        SelectedInstruction {
            id: SelectedInstructionId(id),
            kind,
            constraint: key(id),
            operands,
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance::default(),
        }
    }

    fn use_operand(
        register: u32,
        class: RegisterClassId,
        view: Option<RegisterViewId>,
    ) -> SelectedOperand {
        SelectedOperand {
            operand: 0,
            virtual_register: VirtualRegisterId(register),
            access: RegisterOperandAccess::Use,
            class,
            fixed_view: view,
            tied_to: None,
            early_clobber: false,
        }
    }

    pub(crate) fn fixture() -> (
        SelectedFunction,
        FunctionAllocationLegality,
        RegisterInstructionConstraint,
    ) {
        let machine = MachineId::new(1).unwrap();
        let class = RegisterClassId(0);
        let from = RegisterViewId(1);
        let to = RegisterViewId(2);
        let source_value = ValueId::new(2).unwrap();
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let compare = instruction(
            0,
            SelectedInstructionKind::CompareI64Zero,
            vec![use_operand(0, class, None)],
        );
        let branch = instruction(
            1,
            SelectedInstructionKind::ConditionalBranchNonZero,
            Vec::new(),
        );
        let return_a = instruction(
            2,
            SelectedInstructionKind::ReturnI64,
            vec![use_operand(1, class, Some(to))],
        );
        let return_b = instruction(
            3,
            SelectedInstructionKind::ReturnI64,
            vec![use_operand(1, class, Some(to))],
        );
        let function = SelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            entry_block: SelectedBlockId(0),
            virtual_registers: vec![
                VirtualRegister {
                    id: VirtualRegisterId(0),
                    scalar_type: scalar,
                    class,
                    origin: VirtualRegisterOrigin::EntryParameter {
                        source_value: ValueId::new(1).unwrap(),
                        parameter_index: 0,
                    },
                    definition_site: ValueDefinitionSite::FunctionParameter(0),
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: VirtualRegisterId(1),
                    scalar_type: scalar,
                    class,
                    origin: VirtualRegisterOrigin::EntryParameter {
                        source_value,
                        parameter_index: 1,
                    },
                    definition_site: ValueDefinitionSite::FunctionParameter(1),
                    entry_fixed_view: Some(from),
                },
            ],
            blocks: vec![
                SelectedBlock {
                    id: SelectedBlockId(0),
                    source_block: BlockId::new(1).unwrap(),
                    instructions: vec![compare],
                    terminator: SelectedTerminator::ConditionalBranch {
                        instruction: branch,
                        when_nonzero: SelectedSuccessor {
                            psi_edge: EdgeId::new(1).unwrap(),
                            block: SelectedBlockId(1),
                            source_target: BlockId::new(2).unwrap(),
                            bindings: Vec::new(),
                            fuel: Vec::new(),
                        },
                        when_zero: SelectedSuccessor {
                            psi_edge: EdgeId::new(2).unwrap(),
                            block: SelectedBlockId(2),
                            source_target: BlockId::new(3).unwrap(),
                            bindings: Vec::new(),
                            fuel: Vec::new(),
                        },
                    },
                },
                SelectedBlock {
                    id: SelectedBlockId(1),
                    source_block: BlockId::new(2).unwrap(),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: return_a,
                        psi_return_edge: EdgeId::new(3).unwrap(),
                    },
                },
                SelectedBlock {
                    id: SelectedBlockId(2),
                    source_block: BlockId::new(3).unwrap(),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: return_b,
                        psi_return_edge: EdgeId::new(4).unwrap(),
                    },
                },
            ],
        };
        let site = |instruction| VirtualFixedConstraintSite::Operand {
            position: LivenessPosition(instruction),
            point: LiveRangePoint(instruction),
            instruction: SelectedInstructionId(instruction),
            operand: 0,
            access: RegisterOperandAccess::Use,
        };
        let legality = FunctionAllocationLegality {
            machine,
            virtual_registers: vec![
                VirtualRegisterAllocationLegality {
                    virtual_register: VirtualRegisterId(0),
                    class,
                    points: Vec::new(),
                    early_clobber_points: Vec::new(),
                    entry_transitions: Vec::new(),
                },
                VirtualRegisterAllocationLegality {
                    virtual_register: VirtualRegisterId(1),
                    class,
                    points: Vec::new(),
                    early_clobber_points: Vec::new(),
                    entry_transitions: vec![
                        EntryFixedViewTransition {
                            from_view: from,
                            to_site: site(2),
                            to_view: to,
                        },
                        EntryFixedViewTransition {
                            from_view: from,
                            to_site: site(3),
                            to_view: to,
                        },
                    ],
                },
            ],
        };
        let row = RegisterInstructionConstraint {
            id: RegisterConstraintId(9),
            key: key(9),
            operands: vec![
                RegisterOperandConstraint {
                    operand: 0,
                    access: RegisterOperandAccess::Use,
                    class,
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                },
                RegisterOperandConstraint {
                    operand: 1,
                    access: RegisterOperandAccess::Def,
                    class,
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                },
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        };
        (function, legality, row)
    }

    pub(crate) fn computed_shared_fixture() -> (
        SelectedFunction,
        FunctionAllocationLegality,
        RegisterInstructionConstraint,
        FixedViewCopy,
        SelectedFunction,
    ) {
        let (function, legality, row) = fixture();
        let copy = build_shared_entry_copy(0, &function, &legality, &row, row.key, 4, 2)
            .unwrap()
            .unwrap();
        let mut transformed = function.clone();
        apply_copy(0, &mut transformed, &copy, &row).unwrap();
        (function, legality, row, copy, transformed)
    }

    #[test]
    fn shared_entry_policy_inserts_one_copy_after_compare_and_rewrites_both_returns() {
        let (_, _, _, copy, transformed) = computed_shared_fixture();
        assert_eq!(copy.insertion_block, SelectedBlockId(0));
        assert_eq!(copy.before_instruction, SelectedInstructionId(1));
        assert_eq!(copy.destinations.len(), 2);
        assert_eq!(transformed.blocks[0].instructions.len(), 2);
        assert_eq!(
            transformed.blocks[0].instructions[1].kind,
            SelectedInstructionKind::CopyI64
        );
        for leaf in &transformed.blocks[1..] {
            let SelectedTerminator::Return { instruction, .. } = &leaf.terminator else {
                panic!()
            };
            assert_eq!(
                instruction.operands[0].virtual_register,
                VirtualRegisterId(2)
            );
            assert!(leaf.instructions.is_empty());
        }
    }

    #[test]
    fn shared_entry_policy_rejects_noncanonical_compare_copy_branch_shape() {
        let (mut function, legality, row) = fixture();
        function.blocks[0].instructions.push(instruction(
            4,
            SelectedInstructionKind::CopyI64,
            Vec::new(),
        ));
        assert!(matches!(
            build_shared_entry_copy(0, &function, &legality, &row, row.key, 5, 2),
            Err(FixedViewCopyError::UnsupportedSharedTransitionSet { function: 0 })
        ));
    }
}
