//! Independent indexed reconstruction of logical recovery actions.

use std::collections::BTreeMap;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedTerminator,
    VirtualRegisterOrigin,
};
use psi_core::{IntegerCarrier, IntegerSign, ScalarType};

use crate::{
    LogicalSpillStorageClass, SpillRecoveryActionError, SpillRecoveryActionPlan,
    SpillRecoveryActionPolicy, SpillRecoveryLogicalAction, SpillRecoveryLogicalReload,
    SpillRecoveryLogicalReloadId, SpillRecoveryLogicalStorage, SpillRecoveryLogicalStorageId,
    SpillRecoveryLogicalStore, SpillRecoveryLogicalUseRewrite, SpillRecoveryVictimChoice,
    SpillRecoveryWorkItem, SyntheticReloadValueId, ValidatedAbstractSpillInsertion,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedSelectedAnalysis,
    ValidatedSpillRecoveryChoices, ValidatedSpillRecoveryWorklist, VirtualFixedConstraintSite,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn replay<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    insertion: &ValidatedAbstractSpillInsertion,
    worklist: &ValidatedSpillRecoveryWorklist,
    choices: &ValidatedSpillRecoveryChoices,
    policy: SpillRecoveryActionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<SpillRecoveryActionPlan, SpillRecoveryActionError> {
    super::compute::admit_policy(policy)?;
    let items = item_index(worklist)?;
    let mut ordered_choices = choices.plan().choices.iter().collect::<Vec<_>>();
    ordered_choices.sort_by_key(|choice| (choice.work_item, choice.function));
    let mut actions = Vec::with_capacity(ordered_choices.len());
    for choice in ordered_choices {
        let item = items
            .get(&choice.work_item)
            .copied()
            .ok_or(SpillRecoveryActionError::SourceWorkItemMismatch)?;
        replay_item_join(choice, item)?;
        let selected_function = selected
            .selected_plan()
            .functions
            .get(choice.function)
            .ok_or(SpillRecoveryActionError::FunctionMismatch {
                function: choice.function,
            })?;
        let range_function = ranges.plan().functions.get(choice.function).ok_or(
            SpillRecoveryActionError::FunctionMismatch {
                function: choice.function,
            },
        )?;
        let legality_function = legality.plan().functions.get(choice.function).ok_or(
            SpillRecoveryActionError::FunctionMismatch {
                function: choice.function,
            },
        )?;
        let insertion_function = insertion.plan().functions.get(choice.function).ok_or(
            SpillRecoveryActionError::FunctionMismatch {
                function: choice.function,
            },
        )?;
        actions.push(reconstruct(
            choice,
            item,
            selected_function,
            range_function,
            legality_function,
            insertion_function,
        )?);
    }
    let usage = replay_usage(&actions)?;
    if !usage.within(budget) {
        return Err(SpillRecoveryActionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let choice_receipt = choices.receipt();
    Ok(SpillRecoveryActionPlan {
        selected: selected.selected_identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        abstract_spill_insertion: insertion.receipt().identity(),
        worklist: worklist.receipt().identity(),
        choices: choice_receipt.identity(),
        register_environment: choice_receipt.register_environment(),
        allocator_availability: choice_receipt.allocator_availability(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        policy,
        budget,
        usage,
        actions,
    })
}

fn item_index(
    worklist: &ValidatedSpillRecoveryWorklist,
) -> Result<BTreeMap<SyntheticReloadValueId, &SpillRecoveryWorkItem>, SpillRecoveryActionError> {
    let mut index = BTreeMap::new();
    for epoch in &worklist.plan().epochs {
        for item in &epoch.work_items {
            if index.insert(item.synthetic, item).is_some() {
                return Err(SpillRecoveryActionError::SourceWorkItemMismatch);
            }
        }
    }
    Ok(index)
}

fn replay_item_join(
    choice: &SpillRecoveryVictimChoice,
    item: &SpillRecoveryWorkItem,
) -> Result<(), SpillRecoveryActionError> {
    if item.synthetic.epoch != 1
        || item.machine != choice.machine
        || item.block != choice.block
        || item.start != choice.point
        || item.class != choice.reload_class
        || item.candidates.as_slice() != choice.reload_candidates.as_slice()
    {
        return Err(SpillRecoveryActionError::SourceWorkItemMismatch);
    }
    Ok(())
}

fn reconstruct(
    choice: &SpillRecoveryVictimChoice,
    item: &SpillRecoveryWorkItem,
    selected: &SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    legality: &crate::FunctionAllocationLegality,
    inserted: &crate::FunctionAbstractSpillInsertion,
) -> Result<SpillRecoveryLogicalAction, SpillRecoveryActionError> {
    let function = choice.function;
    if [
        selected.machine,
        ranges.machine,
        legality.machine,
        inserted.machine,
    ]
    .into_iter()
    .any(|machine| machine != choice.machine)
    {
        return Err(SpillRecoveryActionError::FunctionMismatch { function });
    }
    let source = inserted
        .action
        .as_ref()
        .filter(|source| {
            source.reload.result == item.source_reload
                && source.reload.destination_class == item.class
                && source
                    .rewrites
                    .first()
                    .is_some_and(|first| first.block == item.block && first.point == item.start)
        })
        .ok_or(SpillRecoveryActionError::SourceWorkItemMismatch)?;
    let residents = choice
        .active_residents
        .iter()
        .filter(|resident| resident.virtual_register == choice.selected_victim)
        .collect::<Vec<_>>();
    let [resident] = residents.as_slice() else {
        return Err(SpillRecoveryActionError::UnsupportedVictimRole {
            function,
            register: choice.selected_victim.0,
        });
    };
    if resident.view != choice.selected_victim_view
        || choice.selected_victim_view != choice.reclaimed_view
    {
        return Err(SpillRecoveryActionError::UnsupportedVictimRole {
            function,
            register: choice.selected_victim.0,
        });
    }
    let register_index = selected
        .virtual_registers
        .iter()
        .map(|register| (register.id, register))
        .collect::<BTreeMap<_, _>>();
    let victim = register_index
        .get(&choice.selected_victim)
        .copied()
        .ok_or(SpillRecoveryActionError::FunctionMismatch { function })?;
    let scalar_ok = matches!(
        victim.scalar_type,
        ScalarType::Integer(integer)
            if integer.carrier() == IntegerCarrier::Fixed
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    );
    if !scalar_ok {
        return Err(SpillRecoveryActionError::UnsupportedScalarType {
            function,
            register: victim.id.0,
        });
    }
    let definition = match victim.origin {
        VirtualRegisterOrigin::InstructionResult { instruction, .. } => instruction,
        _ => {
            return Err(SpillRecoveryActionError::UnsupportedOrigin {
                function,
                register: victim.id.0,
            });
        }
    };
    let block = selected
        .blocks
        .iter()
        .find(|block| block.id == choice.block)
        .ok_or(SpillRecoveryActionError::FunctionMismatch { function })?;
    if !matches!(victim.definition_site, ValueDefinitionSite::Node { block: source, .. } if source == block.source_block)
    {
        return Err(SpillRecoveryActionError::UnsupportedOrigin {
            function,
            register: victim.id.0,
        });
    }
    let range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == victim.id)
        .ok_or(SpillRecoveryActionError::FunctionMismatch { function })?;
    let legal = legality
        .virtual_registers
        .iter()
        .find(|legal| legal.virtual_register == victim.id)
        .ok_or(SpillRecoveryActionError::FunctionMismatch { function })?;
    let fragment_ok = range.fragments.as_slice()
        == [crate::LiveRangeFragment {
            block: choice.block,
            start: resident.start,
            end: resident.exclusive_end,
        }];
    if !fragment_ok
        || !range.edge_connectors.is_empty()
        || [range.class, legal.class, resident.class]
            .into_iter()
            .any(|class| class != victim.class)
        || resident.start >= choice.point
        || resident.exclusive_end <= choice.point
    {
        return Err(SpillRecoveryActionError::UnsupportedRangeShape {
            function,
            register: victim.id.0,
        });
    }
    let instruction_index = instruction_index(selected, choice.block);
    let definitions = range
        .occurrences
        .iter()
        .filter(|occurrence| {
            occurrence.instruction == definition
                && occurrence.access == RegisterOperandAccess::Def
                && occurrence.point < choice.point
        })
        .count();
    let definition_ok = instruction_index
        .get(&definition)
        .is_some_and(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.virtual_register == victim.id
                    && operand.access == RegisterOperandAccess::Def
                    && operand.class == victim.class
            })
        });
    if definitions != 1 || !definition_ok {
        return Err(SpillRecoveryActionError::UnsupportedOrigin {
            function,
            register: victim.id.0,
        });
    }
    if range
        .occurrences
        .iter()
        .any(|occurrence| occurrence.point == choice.point)
    {
        return Err(SpillRecoveryActionError::VictimUsedAtPressure {
            function,
            register: victim.id.0,
        });
    }
    if range
        .fixed_constraints
        .iter()
        .any(|fixed| match fixed.site {
            VirtualFixedConstraintSite::Operand { point, .. } => point > choice.point,
            VirtualFixedConstraintSite::Entry => false,
        })
    {
        return Err(SpillRecoveryActionError::FutureFixedUse {
            function,
            register: victim.id.0,
        });
    }
    let result = SpillRecoveryLogicalReloadId {
        epoch: choice.work_item.epoch,
        ordinal: choice.work_item.ordinal,
    };
    let mut later = range
        .occurrences
        .iter()
        .filter(|occurrence| occurrence.point > choice.point)
        .collect::<Vec<_>>();
    later.sort_by_key(|occurrence| (occurrence.point, occurrence.instruction, occurrence.operand));
    let mut rewrites = Vec::with_capacity(later.len());
    for occurrence in later {
        let operand = instruction_index
            .get(&occurrence.instruction)
            .and_then(|instruction| {
                instruction
                    .operands
                    .iter()
                    .find(|operand| operand.operand == occurrence.operand)
            })
            .filter(|operand| {
                occurrence.access == RegisterOperandAccess::Use
                    && operand.virtual_register == victim.id
                    && operand.access == RegisterOperandAccess::Use
                    && operand.class == victim.class
                    && operand.fixed_view.is_none()
                    && operand.tied_to.is_none()
                    && !operand.early_clobber
            })
            .ok_or(SpillRecoveryActionError::FutureUseMismatch {
                function,
                register: victim.id.0,
            })?;
        rewrites.push(SpillRecoveryLogicalUseRewrite {
            block: choice.block,
            point: occurrence.point,
            instruction: occurrence.instruction,
            operand: operand.operand,
            result,
        });
    }
    if rewrites.is_empty() {
        return Err(SpillRecoveryActionError::NoFutureUse {
            function,
            register: victim.id.0,
        });
    }
    if rewrites.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SpillRecoveryActionError::FutureUseMismatch {
            function,
            register: victim.id.0,
        });
    }
    let storage_id = SpillRecoveryLogicalStorageId {
        epoch: choice.work_item.epoch,
        ordinal: choice.work_item.ordinal,
    };
    Ok(SpillRecoveryLogicalAction {
        source_work_item: choice.work_item,
        function,
        machine: choice.machine,
        block: choice.block,
        pressure_point: choice.point,
        source_reload: item.source_reload,
        incoming_class: choice.reload_class,
        victim: victim.id,
        victim_class: victim.class,
        victim_scalar_type: victim.scalar_type,
        victim_origin: victim.origin,
        victim_definition_site: victim.definition_site,
        current_view: resident.view,
        reclaimed_view: choice.reclaimed_view,
        storage: SpillRecoveryLogicalStorage {
            id: storage_id,
            class: LogicalSpillStorageClass::NonAddressUnsignedU64V1,
        },
        store: SpillRecoveryLogicalStore {
            before_source_reload: item.source_reload,
            before_instruction: source.reload.before_instruction,
            source: victim.id,
            storage: storage_id,
        },
        reload: SpillRecoveryLogicalReload {
            before_instruction: rewrites[0].instruction,
            storage: storage_id,
            result,
            destination_class: victim.class,
        },
        rewrites,
    })
}

fn replay_usage(
    actions: &[SpillRecoveryLogicalAction],
) -> Result<OptimizationWorkUsage, SpillRecoveryActionError> {
    let mut action_count = 0_u64;
    let mut rewrite_count = 0_u64;
    for action in actions {
        action_count = action_count
            .checked_add(1)
            .ok_or(SpillRecoveryActionError::WorkOverflow)?;
        for _ in &action.rewrites {
            rewrite_count = rewrite_count
                .checked_add(1)
                .ok_or(SpillRecoveryActionError::WorkOverflow)?;
        }
    }
    let validation_steps = action_count
        .checked_mul(4)
        .and_then(|fixed| fixed.checked_add(rewrite_count))
        .ok_or(SpillRecoveryActionError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: action_count,
        candidates: action_count,
        validation_steps,
        commits: action_count,
        iterations: action_count,
    })
}

fn instruction_index(
    selected: &SelectedFunction,
    block: omega_selected_instructions::SelectedBlockId,
) -> BTreeMap<SelectedInstructionId, &SelectedInstruction> {
    let Some(block) = selected
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
    else {
        return BTreeMap::new();
    };
    let mut index = block
        .instructions
        .iter()
        .map(|instruction| (instruction.id, instruction))
        .collect::<BTreeMap<_, _>>();
    match &block.terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => {
            index.insert(instruction.id, instruction);
        }
    }
    index
}
