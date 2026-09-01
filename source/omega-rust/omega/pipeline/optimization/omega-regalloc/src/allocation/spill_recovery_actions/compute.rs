//! Canonical source traversal for epoch-one logical recovery actions.

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedTerminator,
    VirtualRegisterOrigin,
};
use psi_core::{IntegerSign, IntegerType, ScalarType};

use crate::{
    LogicalSpillStorageClass, SpillRecoveryActionError, SpillRecoveryActionPlan,
    SpillRecoveryActionPolicy, SpillRecoveryLogicalAction, SpillRecoveryLogicalReload,
    SpillRecoveryLogicalReloadId, SpillRecoveryLogicalStorage, SpillRecoveryLogicalStorageId,
    SpillRecoveryLogicalStore, SpillRecoveryLogicalUseRewrite, SpillRecoveryVictimChoice,
    SpillRecoveryWorkItem, ValidatedAbstractSpillInsertion, ValidatedAllocationLegality,
    ValidatedLiveRanges, ValidatedSelectedAnalysis, ValidatedSpillRecoveryChoices,
    ValidatedSpillRecoveryWorklist, VirtualFixedConstraintSite,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    insertion: &ValidatedAbstractSpillInsertion,
    worklist: &ValidatedSpillRecoveryWorklist,
    choices: &ValidatedSpillRecoveryChoices,
    policy: SpillRecoveryActionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<SpillRecoveryActionPlan, SpillRecoveryActionError> {
    admit_roots(selected, ranges, legality, insertion, worklist, choices)?;
    admit_policy(policy)?;
    let mut actions = Vec::with_capacity(choices.plan().choices.len());
    for choice in &choices.plan().choices {
        let item = source_item(worklist, choice)?;
        actions.push(build_action(
            choice,
            item,
            &selected.selected_plan().functions[choice.function],
            &ranges.plan().functions[choice.function],
            &legality.plan().functions[choice.function],
            &insertion.plan().functions[choice.function],
        )?);
    }
    actions.sort_by_key(|action| (action.source_work_item, action.function));
    let usage = work_usage(&actions)?;
    if !usage.within(budget) {
        return Err(SpillRecoveryActionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let choice_receipt = choices.receipt();
    let worklist_receipt = worklist.receipt();
    Ok(SpillRecoveryActionPlan {
        selected: selected.selected_identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        abstract_spill_insertion: insertion.receipt().identity(),
        worklist: worklist_receipt.identity(),
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

pub(super) fn admit_policy(
    policy: SpillRecoveryActionPolicy,
) -> Result<(), SpillRecoveryActionError> {
    if policy
        != SpillRecoveryActionPolicy::EpochOneActiveResidentInstructionResultU64LaterFlexibleUsesV1
    {
        return Err(SpillRecoveryActionError::UnsupportedPolicy);
    }
    Ok(())
}

fn admit_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    insertion: &ValidatedAbstractSpillInsertion,
    worklist: &ValidatedSpillRecoveryWorklist,
    choices: &ValidatedSpillRecoveryChoices,
) -> Result<(), SpillRecoveryActionError> {
    let choice = choices.receipt();
    let work = worklist.receipt();
    if ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || insertion.receipt().optimization_unit() != selected.optimization_unit_identity()
        || insertion.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || work.abstract_spill_insertion() != insertion.receipt().identity()
        || work.legality() != legality.receipt().identity()
        || work.ranges() != ranges.receipt().identity()
        || work.optimization_unit() != selected.optimization_unit_identity()
        || work.fuel_schedule() != selected.fuel_schedule_identity()
        || choice.worklist() != work.identity()
        || choice.abstract_spill_insertion() != insertion.receipt().identity()
        || choice.legality() != legality.receipt().identity()
        || choice.ranges() != ranges.receipt().identity()
        || choice.register_environment() != work.register_environment()
        || choice.allocator_availability() != work.allocator_availability()
        || selected.selected_plan().functions.len() != ranges.plan().functions.len()
        || selected.selected_plan().functions.len() != legality.plan().functions.len()
        || selected.selected_plan().functions.len() != insertion.plan().functions.len()
    {
        return Err(SpillRecoveryActionError::RootMismatch);
    }
    Ok(())
}

fn source_item<'a>(
    worklist: &'a ValidatedSpillRecoveryWorklist,
    choice: &SpillRecoveryVictimChoice,
) -> Result<&'a SpillRecoveryWorkItem, SpillRecoveryActionError> {
    let mut matches = worklist
        .plan()
        .epochs
        .iter()
        .flat_map(|epoch| &epoch.work_items)
        .filter(|item| item.synthetic == choice.work_item);
    let item = matches
        .next()
        .ok_or(SpillRecoveryActionError::SourceWorkItemMismatch)?;
    if matches.next().is_some()
        || item.synthetic.epoch != 1
        || item.machine != choice.machine
        || item.block != choice.block
        || item.start != choice.point
        || item.class != choice.reload_class
        || item.candidates != choice.reload_candidates
    {
        return Err(SpillRecoveryActionError::SourceWorkItemMismatch);
    }
    Ok(item)
}

fn build_action(
    choice: &SpillRecoveryVictimChoice,
    item: &SpillRecoveryWorkItem,
    selected: &SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    legality: &crate::FunctionAllocationLegality,
    inserted: &crate::FunctionAbstractSpillInsertion,
) -> Result<SpillRecoveryLogicalAction, SpillRecoveryActionError> {
    let function = choice.function;
    if selected.machine != choice.machine
        || ranges.machine != choice.machine
        || legality.machine != choice.machine
        || inserted.machine != choice.machine
    {
        return Err(SpillRecoveryActionError::FunctionMismatch { function });
    }
    let source = inserted
        .action
        .as_ref()
        .filter(|action| {
            action.reload.result == item.source_reload
                && action.reload.destination_class == item.class
                && action.rewrites.first().is_some_and(|rewrite| {
                    rewrite.block == item.block && rewrite.point == item.start
                })
        })
        .ok_or(SpillRecoveryActionError::SourceWorkItemMismatch)?;
    let resident = choice
        .active_residents
        .iter()
        .find(|resident| {
            resident.virtual_register == choice.selected_victim
                && resident.view == choice.selected_victim_view
        })
        .ok_or(SpillRecoveryActionError::UnsupportedVictimRole {
            function,
            register: choice.selected_victim.0,
        })?;
    if choice.selected_victim_view != choice.reclaimed_view {
        return Err(SpillRecoveryActionError::UnsupportedVictimRole {
            function,
            register: choice.selected_victim.0,
        });
    }
    let victim = selected
        .virtual_registers
        .iter()
        .find(|register| register.id == choice.selected_victim)
        .ok_or(SpillRecoveryActionError::FunctionMismatch { function })?;
    let expected_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).expect("unsigned u64 is valid"),
    );
    if victim.scalar_type != expected_type {
        return Err(SpillRecoveryActionError::UnsupportedScalarType {
            function,
            register: victim.id.0,
        });
    }
    let VirtualRegisterOrigin::InstructionResult {
        instruction: definition,
        ..
    } = victim.origin
    else {
        return Err(SpillRecoveryActionError::UnsupportedOrigin {
            function,
            register: victim.id.0,
        });
    };
    let selected_block = selected
        .blocks
        .iter()
        .find(|block| block.id == choice.block)
        .ok_or(SpillRecoveryActionError::FunctionMismatch { function })?;
    if !matches!(
        victim.definition_site,
        ValueDefinitionSite::Node { block, .. } if block == selected_block.source_block
    ) {
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
    if victim.class != resident.class
        || victim.class != range.class
        || victim.class != legal.class
        || range.fragments.as_slice()
            != [crate::LiveRangeFragment {
                block: choice.block,
                start: resident.start,
                end: resident.exclusive_end,
            }]
        || !range.edge_connectors.is_empty()
        || resident.start >= choice.point
        || resident.exclusive_end <= choice.point
    {
        return Err(SpillRecoveryActionError::UnsupportedRangeShape {
            function,
            register: victim.id.0,
        });
    }
    let definitions = range
        .occurrences
        .iter()
        .filter(|occurrence| {
            occurrence.instruction == definition
                && occurrence.access == RegisterOperandAccess::Def
                && occurrence.point < choice.point
        })
        .count();
    if definitions != 1
        || !instruction(selected, choice.block, definition).is_some_and(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.virtual_register == victim.id
                    && operand.access == RegisterOperandAccess::Def
                    && operand.class == victim.class
            })
        })
    {
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
    if range.fixed_constraints.iter().any(|fixed| {
        matches!(fixed.site, VirtualFixedConstraintSite::Operand { point, .. } if point > choice.point)
    }) {
        return Err(SpillRecoveryActionError::FutureFixedUse {
            function,
            register: victim.id.0,
        });
    }
    let result = SpillRecoveryLogicalReloadId {
        epoch: item.synthetic.epoch,
        ordinal: item.synthetic.ordinal,
    };
    let mut rewrites = Vec::new();
    for occurrence in range
        .occurrences
        .iter()
        .filter(|occurrence| occurrence.point > choice.point)
    {
        let operand = instruction(selected, choice.block, occurrence.instruction)
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
    if !rewrites.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(SpillRecoveryActionError::FutureUseMismatch {
            function,
            register: victim.id.0,
        });
    }
    let storage_id = SpillRecoveryLogicalStorageId {
        epoch: item.synthetic.epoch,
        ordinal: item.synthetic.ordinal,
    };
    Ok(SpillRecoveryLogicalAction {
        source_work_item: item.synthetic,
        function,
        machine: choice.machine,
        block: choice.block,
        pressure_point: choice.point,
        source_reload: item.source_reload,
        incoming_class: item.class,
        victim: victim.id,
        victim_class: victim.class,
        victim_scalar_type: victim.scalar_type,
        victim_origin: victim.origin,
        victim_definition_site: victim.definition_site,
        current_view: choice.selected_victim_view,
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

pub(super) fn work_usage(
    actions: &[SpillRecoveryLogicalAction],
) -> Result<OptimizationWorkUsage, SpillRecoveryActionError> {
    let action_count =
        u64::try_from(actions.len()).map_err(|_| SpillRecoveryActionError::WorkOverflow)?;
    let rewrites = actions.iter().try_fold(0_u64, |total, action| {
        total
            .checked_add(
                u64::try_from(action.rewrites.len())
                    .map_err(|_| SpillRecoveryActionError::WorkOverflow)?,
            )
            .ok_or(SpillRecoveryActionError::WorkOverflow)
    })?;
    let validation_steps = action_count
        .checked_mul(4)
        .and_then(|fixed| fixed.checked_add(rewrites))
        .ok_or(SpillRecoveryActionError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: action_count,
        candidates: action_count,
        validation_steps,
        commits: action_count,
        iterations: action_count,
    })
}

fn instruction(
    selected: &SelectedFunction,
    block: omega_selected_instructions::SelectedBlockId,
    id: SelectedInstructionId,
) -> Option<&SelectedInstruction> {
    let block = selected
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?;
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
