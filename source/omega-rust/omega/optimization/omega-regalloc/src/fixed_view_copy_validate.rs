use std::collections::BTreeSet;

use omega_optimization_core::OptimizationWorkUsage;
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
use omega_terminal_target_operations_to_selected_instructions::{
    ValidatedTerminalSelectedInstructions, terminal_selected_instruction_plan_identity,
};
use psi_core::{IntegerSign, ScalarType};

use crate::{
    TerminalFixedViewCopy, TerminalFixedViewCopyDestination, TerminalFixedViewCopyError,
    TerminalFixedViewCopyPlan, TerminalFixedViewCopyPolicy, TerminalFixedViewCopyValidationReceipt,
    TerminalVirtualFixedConstraintSite, ValidatedTerminalAllocationLegality,
    ValidatedTerminalFixedViewCopies, ValidatedTerminalLiveRanges,
    terminal_fixed_view_copy_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_terminal_fixed_view_copies(
    selected: &ValidatedTerminalSelectedInstructions,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: TerminalFixedViewCopyPlan,
) -> Result<ValidatedTerminalFixedViewCopies, TerminalFixedViewCopyError> {
    validate_roots(
        selected,
        ranges,
        legality,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        &plan,
    )?;
    let row = validated_copy_row(constraints, selected_keys)?;
    let expected_usage = replay_usage(selected, legality, plan.policy)?;
    if plan.usage != expected_usage {
        return Err(TerminalFixedViewCopyError::ReceiptMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(TerminalFixedViewCopyError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let (expected_copies, expected_transformed) =
        replay_transformation(selected, legality, selected_keys, row, plan.policy)?;
    if plan.copies != expected_copies {
        let index = plan
            .copies
            .iter()
            .zip(&expected_copies)
            .position(|(actual, expected)| actual != expected)
            .unwrap_or(plan.copies.len().min(expected_copies.len()));
        return Err(TerminalFixedViewCopyError::CopyMismatch { index });
    }
    if plan.transformed != expected_transformed {
        return Err(TerminalFixedViewCopyError::TransformedPlanMismatch);
    }
    let transformed_selected = terminal_selected_instruction_plan_identity(&plan.transformed);
    let receipt = TerminalFixedViewCopyValidationReceipt {
        identity: terminal_fixed_view_copy_identity(&plan),
        source_selected: plan.source_selected,
        source_ranges: plan.source_ranges,
        source_legality: plan.source_legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        transformed_selected,
        optimization_unit: selected.receipt().optimization_unit(),
        fuel_schedule: selected.receipt().fuel_schedule(),
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.transformed.functions.len(),
        copy_count: plan.copies.len(),
    };
    Ok(ValidatedTerminalFixedViewCopies { plan, receipt })
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
    plan: &TerminalFixedViewCopyPlan,
) -> Result<(), TerminalFixedViewCopyError> {
    if plan.source_selected != selected.receipt().identity()
        || plan.source_ranges != ranges.receipt().identity()
        || plan.source_legality != legality.receipt().identity()
        || plan.register_environment != register_environment
        || plan.allocator_availability != legality.receipt().allocator_availability()
        || ranges.plan().selected != selected.receipt().identity()
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

fn validated_copy_row(
    constraints: &ValidatedRegisterConstraintCatalog,
    keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<&RegisterInstructionConstraint, TerminalFixedViewCopyError> {
    let Some(row) = constraints
        .catalog()
        .constraints
        .iter()
        .find(|candidate| candidate.key == keys.copy_i64)
    else {
        return Err(TerminalFixedViewCopyError::CopyConstraintMismatch);
    };
    let operand_shape = row.operands.as_slice();
    let [source, result] = operand_shape else {
        return Err(TerminalFixedViewCopyError::CopyConstraintMismatch);
    };
    if source.operand != 0
        || source.access != RegisterOperandAccess::Use
        || result.operand != 1
        || result.access != RegisterOperandAccess::Def
        || source.class != result.class
        || [source, result].iter().any(|operand| {
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

fn replay_usage(
    selected: &ValidatedTerminalSelectedInstructions,
    legality: &ValidatedTerminalAllocationLegality,
    policy: TerminalFixedViewCopyPolicy,
) -> Result<OptimizationWorkUsage, TerminalFixedViewCopyError> {
    let functions = u64::try_from(selected.plan().functions.len())
        .map_err(|_| TerminalFixedViewCopyError::WorkOverflow)?;
    let requirements = legality
        .plan()
        .functions
        .iter()
        .flat_map(|function| &function.virtual_registers)
        .flat_map(|register| &register.entry_transitions)
        .try_fold(0_u64, |count, _| count.checked_add(1))
        .ok_or(TerminalFixedViewCopyError::WorkOverflow)?;
    let commits = match policy {
        TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => requirements,
        TerminalFixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => legality
            .plan()
            .functions
            .iter()
            .try_fold(0_u64, |count, function| {
                count.checked_add(u64::from(
                    function
                        .virtual_registers
                        .iter()
                        .any(|r| !r.entry_transitions.is_empty()),
                ))
            })
            .ok_or(TerminalFixedViewCopyError::WorkOverflow)?,
    };
    Ok(OptimizationWorkUsage {
        rule_evaluations: functions,
        candidates: requirements,
        validation_steps: requirements,
        commits,
        iterations: 1,
    })
}

fn replay_transformation(
    selected: &ValidatedTerminalSelectedInstructions,
    legality: &ValidatedTerminalAllocationLegality,
    keys: TargetRegisterEnvironmentConstraintKeys,
    row: &RegisterInstructionConstraint,
    policy: TerminalFixedViewCopyPolicy,
) -> Result<
    (
        Vec<TerminalFixedViewCopy>,
        omega_terminal_selected_instructions::TerminalSelectedInstructionPlan,
    ),
    TerminalFixedViewCopyError,
> {
    let mut output = selected.plan().clone();
    let mut expected = Vec::new();
    for function_index in 0..selected.plan().functions.len() {
        let source_function = &selected.plan().functions[function_index];
        let legality_function = &legality.plan().functions[function_index];
        if source_function.machine != legality_function.machine {
            return Err(TerminalFixedViewCopyError::FunctionMismatch {
                function: function_index,
            });
        }
        let mut instruction_ids = source_function
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .map(|instruction| instruction.id.0)
                    .chain(std::iter::once(terminator(&block.terminator).id.0))
            })
            .collect::<Vec<_>>();
        instruction_ids.sort_unstable();
        let instruction_count = u32::try_from(instruction_ids.len()).map_err(|_| {
            TerminalFixedViewCopyError::IdentifierOverflow {
                function: function_index,
            }
        })?;
        if instruction_ids != (0..instruction_count).collect::<Vec<_>>()
            || source_function
                .virtual_registers
                .iter()
                .enumerate()
                .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
        {
            return Err(TerminalFixedViewCopyError::FunctionMismatch {
                function: function_index,
            });
        }
        let mut next_instruction = instruction_count;
        let mut next_register =
            u32::try_from(source_function.virtual_registers.len()).map_err(|_| {
                TerminalFixedViewCopyError::IdentifierOverflow {
                    function: function_index,
                }
            })?;
        if policy == TerminalFixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 {
            if let Some(copy) = replay_shared_entry_copy(
                function_index,
                source_function,
                legality_function,
                row,
                keys.copy_i64,
                next_instruction,
                next_register,
            )? {
                replay_apply(
                    function_index,
                    &mut output.functions[function_index],
                    &copy,
                    row,
                )?;
                expected.push(copy);
            }
            continue;
        }
        let mut seen = BTreeSet::new();
        for legality_register in &legality_function.virtual_registers {
            for transition in &legality_register.entry_transitions {
                let TerminalVirtualFixedConstraintSite::Operand {
                    instruction,
                    operand,
                    access,
                    ..
                } = transition.to_site
                else {
                    return Err(TerminalFixedViewCopyError::UnsupportedTransitionSite {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                };
                if access != RegisterOperandAccess::Use || !seen.insert((instruction, operand)) {
                    return Err(TerminalFixedViewCopyError::NonCanonicalCopies);
                }
                let source = source_function
                    .virtual_registers
                    .iter()
                    .find(|register| register.id == legality_register.virtual_register)
                    .ok_or(TerminalFixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    })?;
                let TerminalVirtualRegisterOrigin::EntryParameter { source_value, .. } =
                    source.origin
                else {
                    return Err(TerminalFixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                };
                if source.class != legality_register.class
                    || source.entry_fixed_view != Some(transition.from_view)
                    || !replay_is_u64(source.scalar_type)
                    || row.operands[0].class != source.class
                {
                    return Err(TerminalFixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                }
                let block = replay_leaf_block(
                    function_index,
                    source_function,
                    instruction,
                    operand,
                    source.id,
                    transition.to_view,
                )?;
                let copy = TerminalFixedViewCopy {
                    function: u32::try_from(function_index).map_err(|_| {
                        TerminalFixedViewCopyError::IdentifierOverflow {
                            function: function_index,
                        }
                    })?,
                    machine: source_function.machine,
                    source_virtual_register: source.id,
                    source_value,
                    source_definition_site: source.definition_site,
                    from_view: transition.from_view,
                    to_view: transition.to_view,
                    insertion_block: block,
                    before_instruction: instruction,
                    destinations: vec![TerminalFixedViewCopyDestination {
                        site: transition.to_site,
                        block,
                        view: transition.to_view,
                    }],
                    copy_instruction: TerminalSelectedInstructionId(next_instruction),
                    result_virtual_register: TerminalVirtualRegisterId(next_register),
                    copy_constraint: keys.copy_i64,
                };
                replay_apply(
                    function_index,
                    &mut output.functions[function_index],
                    &copy,
                    row,
                )?;
                expected.push(copy);
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
    Ok((expected, output))
}

fn replay_leaf_block(
    function_index: usize,
    function: &omega_terminal_selected_instructions::TerminalSelectedFunction,
    instruction: TerminalSelectedInstructionId,
    operand: u16,
    source: TerminalVirtualRegisterId,
    view: omega_register_model::RegisterViewId,
) -> Result<omega_terminal_selected_instructions::TerminalSelectedBlockId, TerminalFixedViewCopyError>
{
    let block = function
        .blocks
        .iter()
        .find(|block| terminator(&block.terminator).id == instruction)
        .ok_or(TerminalFixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: instruction.0,
        })?;
    let TerminalSelectedTerminator::Return {
        instruction: destination,
        ..
    } = &block.terminator
    else {
        return Err(TerminalFixedViewCopyError::NonLeafDestination {
            function: function_index,
            instruction: instruction.0,
        });
    };
    if block.id == function.entry_block
        || !destination.operands.iter().any(|candidate| {
            candidate.operand == operand
                && candidate.virtual_register == source
                && candidate.access == RegisterOperandAccess::Use
                && candidate.fixed_view == Some(view)
        })
    {
        return Err(TerminalFixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: instruction.0,
        });
    }
    Ok(block.id)
}

#[allow(clippy::too_many_arguments)]
fn replay_shared_entry_copy(
    function_index: usize,
    function: &omega_terminal_selected_instructions::TerminalSelectedFunction,
    legality: &crate::TerminalFunctionAllocationLegality,
    row: &RegisterInstructionConstraint,
    constraint: omega_register_model::RegisterConstraintKey,
    instruction_id: u32,
    register_id: u32,
) -> Result<Option<TerminalFixedViewCopy>, TerminalFixedViewCopyError> {
    let mut requirements = Vec::new();
    for register in &legality.virtual_registers {
        for transition in &register.entry_transitions {
            requirements.push((register, transition));
        }
    }
    if requirements.is_empty() {
        return Ok(None);
    }
    if requirements.len() != 2
        || requirements[0].0.virtual_register != requirements[1].0.virtual_register
        || requirements[0].1.from_view != requirements[1].1.from_view
        || requirements[0].1.to_view != requirements[1].1.to_view
    {
        return Err(TerminalFixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let requirement = requirements[0].0;
    let source = function
        .virtual_registers
        .iter()
        .find(|candidate| candidate.id == requirement.virtual_register)
        .ok_or(TerminalFixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: requirement.virtual_register.0,
        })?;
    let TerminalVirtualRegisterOrigin::EntryParameter { source_value, .. } = source.origin else {
        return Err(TerminalFixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: source.id.0,
        });
    };
    if source.class != requirement.class
        || source.entry_fixed_view != Some(requirements[0].1.from_view)
        || !replay_is_u64(source.scalar_type)
        || row.operands[0].class != source.class
        || row.operands[1].class != source.class
    {
        return Err(TerminalFixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: source.id.0,
        });
    }
    let entry = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == function.entry_block)
        .ok_or(TerminalFixedViewCopyError::FunctionMismatch {
            function: function_index,
        })?;
    let [compare] = entry.instructions.as_slice() else {
        return Err(TerminalFixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    };
    if compare.kind != TerminalSelectedInstructionKind::CompareI64Zero {
        return Err(TerminalFixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let TerminalSelectedTerminator::ConditionalBranch {
        instruction: branch,
        when_nonzero,
        when_zero,
    } = &entry.terminator
    else {
        return Err(TerminalFixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    };
    let expected_leaves = BTreeSet::from([when_nonzero.block, when_zero.block]);
    if expected_leaves.len() != 2 {
        return Err(TerminalFixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let mut actual_leaves = BTreeSet::new();
    let mut destinations = Vec::new();
    let from_view = requirements[0].1.from_view;
    for (_, transition) in requirements {
        let TerminalVirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access,
            ..
        } = transition.to_site
        else {
            return Err(TerminalFixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        };
        if access != RegisterOperandAccess::Use {
            return Err(TerminalFixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        }
        let block = replay_leaf_block(
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
            .unwrap();
        if !leaf.instructions.is_empty() || !actual_leaves.insert(block) {
            return Err(TerminalFixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        }
        destinations.push(TerminalFixedViewCopyDestination {
            site: transition.to_site,
            block,
            view: transition.to_view,
        });
    }
    if actual_leaves != expected_leaves {
        return Err(TerminalFixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    destinations.sort_by_key(|destination| match destination.site {
        TerminalVirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            ..
        } => (instruction.0, operand),
        TerminalVirtualFixedConstraintSite::Entry => (u32::MAX, u16::MAX),
    });
    Ok(Some(TerminalFixedViewCopy {
        function: u32::try_from(function_index).map_err(|_| {
            TerminalFixedViewCopyError::IdentifierOverflow {
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
        copy_instruction: TerminalSelectedInstructionId(instruction_id),
        result_virtual_register: TerminalVirtualRegisterId(register_id),
        copy_constraint: constraint,
    }))
}

fn replay_apply(
    function_index: usize,
    function: &mut omega_terminal_selected_instructions::TerminalSelectedFunction,
    copy: &TerminalFixedViewCopy,
    row: &RegisterInstructionConstraint,
) -> Result<(), TerminalFixedViewCopyError> {
    let source = function
        .virtual_registers
        .iter()
        .find(|register| register.id == copy.source_virtual_register)
        .cloned()
        .ok_or(TerminalFixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: copy.source_virtual_register.0,
        })?;
    for destination in &copy.destinations {
        let TerminalVirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access: RegisterOperandAccess::Use,
            ..
        } = destination.site
        else {
            return Err(TerminalFixedViewCopyError::UnsupportedTransitionSite {
                function: function_index,
                register: copy.source_virtual_register.0,
            });
        };
        let block = function
            .blocks
            .iter_mut()
            .find(|block| block.id == destination.block)
            .ok_or(TerminalFixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?;
        let TerminalSelectedTerminator::Return {
            instruction: return_instruction,
            ..
        } = &mut block.terminator
        else {
            return Err(TerminalFixedViewCopyError::NonLeafDestination {
                function: function_index,
                instruction: instruction.0,
            });
        };
        return_instruction
            .operands
            .iter_mut()
            .find(|candidate| candidate.operand == operand)
            .ok_or(TerminalFixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?
            .virtual_register = copy.result_virtual_register;
    }
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == copy.insertion_block)
        .ok_or(TerminalFixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: copy.before_instruction.0,
        })?;
    if terminator(&block.terminator).id != copy.before_instruction {
        return Err(TerminalFixedViewCopyError::InvalidInsertionSite {
            function: function_index,
            instruction: copy.before_instruction.0,
        });
    }
    function.virtual_registers.push(TerminalVirtualRegister {
        id: copy.result_virtual_register,
        scalar_type: source.scalar_type,
        class: source.class,
        origin: match source.origin {
            TerminalVirtualRegisterOrigin::LegalizationTemporary { temporary, .. } => {
                TerminalVirtualRegisterOrigin::LegalizationTemporary {
                    instruction: copy.copy_instruction,
                    temporary,
                    source_value: copy.source_value,
                }
            }
            _ => TerminalVirtualRegisterOrigin::InstructionResult {
                instruction: copy.copy_instruction,
                source_value: copy.source_value,
            },
        },
        definition_site: copy.source_definition_site,
        entry_fixed_view: None,
    });
    block.instructions.push(TerminalSelectedInstruction {
        id: copy.copy_instruction,
        kind: TerminalSelectedInstructionKind::CopyI64,
        constraint: copy.copy_constraint,
        operands: vec![
            replay_operand(&row.operands[0], copy.source_virtual_register),
            replay_operand(&row.operands[1], copy.result_virtual_register),
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

fn replay_operand(
    constraint: &omega_register_model::RegisterOperandConstraint,
    virtual_register: TerminalVirtualRegisterId,
) -> TerminalSelectedOperand {
    TerminalSelectedOperand {
        operand: constraint.operand,
        virtual_register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}

fn terminator(
    terminator: &TerminalSelectedTerminator,
) -> &omega_terminal_selected_instructions::TerminalSelectedInstruction {
    match terminator {
        TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
        | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
    }
}

fn replay_is_u64(scalar: ScalarType) -> bool {
    match scalar {
        ScalarType::Integer(integer) => {
            integer.sign() == IntegerSign::Unsigned && integer.bits() == 64
        }
        _ => false,
    }
}
