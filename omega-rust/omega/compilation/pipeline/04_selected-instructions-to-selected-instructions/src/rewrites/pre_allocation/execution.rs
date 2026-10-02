//! Joint pre-allocation execution, fixed-point iteration, and independent
//! replay.
//!
//! One sweep walks [`PRE_ALLOCATION_RULE_CATALOG`] in canonical descriptor
//! order, running each enabled family's discovery pass over the current
//! program. The first admissible candidate commits and the whole sweep
//! restarts over the transformed plan: an extension removal publishes a
//! `CopyI64` the copy-removal pass then owns, so families share the joint
//! fixed point rather than each converging alone.
use crate::{StoredLoadForwardingError, forward_selected_stored_load};
use std::collections::BTreeSet;

use crate::AddressFoldError;
use crate::ConstantBooleanError;
use crate::ConstantBranchError;
use crate::CopiedCallOperandError;
use crate::CopyRemovalError;
use crate::DeadCompareError;
use crate::EquivalentCompareError;
use crate::OptimizedPreAllocationCustodyError;
use crate::PreAllocationPolicy;
use crate::RedundantCompareError;
use crate::RedundantExtensionError;
use crate::SelectedProgramRef;
use crate::StagedOptimizedAllocationLegality;
use crate::StagedOptimizedAllocationLegalityCustodyReceipt;
use crate::StagedOptimizedPreAllocationAttempt;
use crate::StagedOptimizedPreAllocationAttemptReceipt;
use crate::StagedOptimizedPreAllocationIterationReceipt;
use crate::StagedOptimizedPreAllocationStep;
use crate::StagedPreAllocationOptimizationCustodyReceipt;
use crate::StagedPreAllocationOptimizationRun;
use crate::ValidatedPreAllocationTransformation;
use crate::ValidatedSelectedAnalysis;
use crate::analyses::validated_machine_effect_catalog;
use crate::analyze_allocation_legality;
use crate::analyze_live_ranges;
use crate::analyze_liveness;
use crate::resolve_pre_allocation_rules;
use crate::rewrites::block_edges::terminator_instruction;
use crate::rewrites::condition_state::adjacency;
use crate::rewrites::{
    address_fold_measured_steps, constant_boolean_measured_steps, constant_boolean_surface_sizes,
    constant_branch_measured_steps, constant_branch_surface_sizes,
    copied_call_operand_measured_steps, copy_removal_measured_steps, dead_compare_measured_steps,
    equivalent_compare_measured_steps, fold_selected_address, fold_selected_constant_boolean,
    fold_selected_constant_branch, fold_selected_copied_call_operand,
    redundant_compare_measured_steps, redundant_extension_measured_steps, remove_dead_compare,
    remove_equivalent_compare, remove_redundant_compare, remove_selected_copy,
    remove_selected_redundant_extension,
};
use crate::validate_optimized_allocation_legality_custody;
use optimization_core::{
    Optimization, OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
    PreAllocationOptimizationCompletionIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedInstructionPlan,
};

use super::PRE_ALLOCATION_RULE_CATALOG;

/// One discovery pass's outcome: either the first admissible candidate
/// committed, or every candidate in the plan declined.
enum PreAllocationPass {
    Clean {
        candidates: usize,
        declined: usize,
        validation_steps: u64,
    },
    Applied {
        transformation: ValidatedPreAllocationTransformation,
        declined: usize,
        evaluated: usize,
        validation_steps: u64,
    },
}

/// Whether the instruction kind is one of the carrier extensions the
/// redundant-extension family considers: `ZeroExtend*`/`SignExtend*`.
/// Admission decides redundancy; this is only the source-bound candidate
/// surface, scanned in plan order.
fn is_extension_candidate(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::ZeroExtendU8
            | SelectedInstructionKind::ZeroExtendU16
            | SelectedInstructionKind::ZeroExtendU32
            | SelectedInstructionKind::SignExtendI8
            | SelectedInstructionKind::SignExtendI16
            | SelectedInstructionKind::SignExtendI32
    )
}

/// Whether the instruction kind is a displacement-carrying consumer the
/// address-fold family considers: the referent `Load*` forms, the
/// byte-addressable referent `Store` widths, and `AddressOffset` itself.
/// Admission decides foldability; this is only the source-bound candidate
/// surface, scanned in plan order.
fn is_fold_consumer(kind: SelectedInstructionKind) -> bool {
    match kind {
        SelectedInstructionKind::Load8 { .. }
        | SelectedInstructionKind::Load16 { .. }
        | SelectedInstructionKind::Load32 { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::AddressOffset { .. } => true,
        SelectedInstructionKind::Store { byte_size, .. } => {
            matches!(byte_size, 1 | 2 | 4 | 8)
        }
        _ => false,
    }
}

/// Whether the instruction kind is a flag-reading `MaterializeBoolean*`
/// the constant-boolean family considers. Admission decides foldability;
/// this is only the source-bound candidate surface, scanned in plan order.
fn is_boolean_reader(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::MaterializeBooleanEqual
            | SelectedInstructionKind::MaterializeBooleanU64LessThan
            | SelectedInstructionKind::MaterializeBooleanI64LessThan
            | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
            | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
    )
}

/// Whether the terminator-carried instruction kind is a flag-reading
/// `ConditionalBranch*` the constant-branch family considers. Admission
/// decides foldability — the terminator/kind pairing, the zero-operand
/// shape, and the flag partition; this is only the source-bound candidate
/// surface, scanned in function/block order.
fn is_branch_reader(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::ConditionalBranchNonZero
            | SelectedInstructionKind::ConditionalBranchU64LessThan
            | SelectedInstructionKind::ConditionalBranchI64LessThan
    )
}

/// Whether the instruction kind is one of the flag-publisher compares the
/// dead-, redundant-, and equivalent-compare families consider:
/// `CompareI64`, `CompareI64Immediate`, or `CompareI64Zero`. Admission
/// decides removability — the canonical operand shape, the constraint
/// surface, the unit-wise audits, and the roster exclusions; this is only
/// the source-bound candidate surface, scanned in function/block/
/// instruction order.
fn is_compare_publisher(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::CompareI64
            | SelectedInstructionKind::CompareI64Immediate { .. }
            | SelectedInstructionKind::CompareI64Zero
    )
}

/// The distinct condition-state units a compare-shaped instruction
/// publishes — the `implicit_defs`/`clobbers` union size the compare
/// families' measured-step contracts charge per published unit.
fn published_flag_units(instruction: &SelectedInstruction) -> usize {
    instruction
        .implicit_defs
        .iter()
        .chain(instruction.clobbers.iter())
        .collect::<BTreeSet<&register_model::RegisterUnitId>>()
        .len()
}

/// The definition-chain length one `Use` operand adds to the joint
/// measure: the number of in-block definitions reached by following each
/// defining instruction's own operand-0 `Use` backward from the
/// operand's register. An address fold rebinds the consumer to the
/// producer's earlier base and skips the producer's node in this chain —
/// and in every chain routed through the consumer's result — a
/// copied-call-operand reroute drops exactly the copy's node from every
/// rebound call operand's chain, while a copy removal erases the copy's
/// node from every rebound chain.
fn operand_chain_length(
    instructions: &[SelectedInstruction],
    consumer_index: usize,
    operand_index: usize,
) -> usize {
    let mut register = match instructions[consumer_index].operands.get(operand_index) {
        Some(operand) if operand.access == RegisterOperandAccess::Use => operand.virtual_register,
        _ => return 0,
    };
    let mut position = consumer_index;
    let mut links = 0usize;
    while let Some(definition) = instructions[..position].iter().rposition(|instruction| {
        instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
        })
    }) {
        links += 1;
        match instructions[definition].operands.first() {
            Some(next) if next.access == RegisterOperandAccess::Use => {
                register = next.virtual_register;
                position = definition;
            }
            _ => break,
        }
    }
    links
}

/// Whether the instruction kind is one of the direct internal calls the
/// copied-call-operand family considers: `CallUnit`, `CallScalar`, or
/// `CallAggregate`, whose leading `Use` operands carry the argument
/// roster. Admission decides the pair; this is only the source-bound
/// candidate surface and the joint measure's call term.
fn is_call_consumer(instruction: &SelectedInstruction) -> bool {
    matches!(
        instruction.kind,
        SelectedInstructionKind::CallUnit { .. }
            | SelectedInstructionKind::CallScalar { .. }
            | SelectedInstructionKind::CallAggregate { .. }
    )
}

/// The joint measure a committed step must strictly drop: the plan's
/// virtual registers and retained memory access rows, plus its remaining extension candidates, plus the
/// summed operand-0 definition-chain lengths of the fold-shaped consumers,
/// plus the summed per-`Use`-operand definition-chain lengths of the
/// direct internal calls, plus every flag unit the remaining
/// `MaterializeBoolean*` readers still observe, plus every implicit use
/// the remaining `ConditionalBranch*` terminators carry, plus one per
/// remaining flag-publisher compare. A copy removal drops the copy and
/// its destination's roster row and can only shorten chains; an
/// extension removal retires one extension while keeping every register
/// and chain; an address fold keeps both but skips a producer node in
/// its consumer's chain; a copied-call-operand reroute keeps the copy,
/// its register, and the call published but skips the copy's node in
/// every rebound operand's chain; a constant-boolean fold keeps
/// registers, chains, and instruction count while dropping the flag uses
/// the folded reader carried; a constant-branch fold keeps registers,
/// chains, and instruction count while the `Jump` row that replaces the
/// branch carries none of its flag uses; a compare removal drops one
/// compare-publisher term while a compare carries no register definitions
/// or flag uses the other terms charge.
fn measure(plan: &SelectedInstructionPlan, virtual_registers: usize) -> usize {
    let mut total = virtual_registers
        + plan
            .functions
            .iter()
            .map(|function| function.memory_accesses.len())
            .sum::<usize>();
    for function in &plan.functions {
        for block in &function.blocks {
            for (index, instruction) in block.instructions.iter().enumerate() {
                if is_extension_candidate(instruction.kind) {
                    total += 1;
                }
                if is_fold_consumer(instruction.kind) {
                    total += operand_chain_length(&block.instructions, index, 0);
                }
                if is_call_consumer(instruction) {
                    for operand_index in 0..instruction.operands.len() {
                        total += operand_chain_length(&block.instructions, index, operand_index);
                    }
                }
                if is_boolean_reader(instruction.kind) {
                    total += instruction.implicit_uses.len();
                }
                if is_compare_publisher(instruction.kind) {
                    total += 1;
                }
            }
            let terminator = terminator_instruction(&block.terminator);
            if is_branch_reader(terminator.kind) {
                total += terminator.implicit_uses.len();
            }
        }
    }
    total
}

/// Source-bound candidate discovery: every `CopyI64` instruction in the
/// current validated plan, in function/block/instruction order, evaluated
/// until the first admissible one commits. A declined candidate is the
/// admission's legality refusal and the scan continues; budget exhaustion,
/// identity arithmetic failure, or a producer/validator disagreement fails
/// the pass — none is permission to publish a partial candidate.
///
/// The pass meters `validation_steps` in the family's published measured-step
/// contract: each evaluated candidate costs one audit scan of the current
/// plan, and a committed candidate costs a second scan for the independent
/// validation.
fn copy_removal_pass(
    current: SelectedProgramRef<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<PreAllocationPass, OptimizedPreAllocationCustodyError> {
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    for (function_index, function) in current.selected_plan().functions.iter().enumerate() {
        let audit_cost = copy_removal_measured_steps(current.selected_plan(), function)
            .map_err(OptimizedPreAllocationCustodyError::CopyRemoval)?;
        for block in &function.blocks {
            for instruction in &block.instructions {
                if instruction.kind != SelectedInstructionKind::CopyI64 {
                    continue;
                }
                evaluated += 1;
                validation_steps = validation_steps
                    .checked_add(audit_cost)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                match remove_selected_copy(
                    &current,
                    function_index,
                    instruction.id,
                    environment,
                    budget,
                ) {
                    Ok(removal) => {
                        validation_steps = validation_steps
                            .checked_add(audit_cost)
                            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                        return Ok(PreAllocationPass::Applied {
                            transformation: ValidatedPreAllocationTransformation::CopyRemoval(
                                removal,
                            ),
                            declined,
                            evaluated,
                            validation_steps,
                        });
                    }
                    Err(
                        error @ (CopyRemovalError::WorkBudgetExceeded
                        | CopyRemovalError::IdentityOverflow
                        | CopyRemovalError::ReplayMismatch),
                    ) => {
                        return Err(OptimizedPreAllocationCustodyError::CopyRemoval(error));
                    }
                    Err(_) => declined += 1,
                }
            }
        }
    }
    Ok(PreAllocationPass::Clean {
        candidates: evaluated,
        declined,
        validation_steps,
    })
}

/// Source-bound candidate discovery for the redundant-extension family:
/// every `ZeroExtend*`/`SignExtend*` in the current validated plan, in
/// function/block/instruction order, evaluated until the first admissible
/// one commits. Declines and hard failures follow the copy pass's contract
/// exactly, and the measured-step contract is the extension family's own.
fn extension_removal_pass(
    current: SelectedProgramRef<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<PreAllocationPass, OptimizedPreAllocationCustodyError> {
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    for (function_index, function) in current.selected_plan().functions.iter().enumerate() {
        let audit_cost = redundant_extension_measured_steps(current.selected_plan(), function)
            .map_err(OptimizedPreAllocationCustodyError::RedundantExtension)?;
        for block in &function.blocks {
            for instruction in &block.instructions {
                if !is_extension_candidate(instruction.kind) {
                    continue;
                }
                evaluated += 1;
                validation_steps = validation_steps
                    .checked_add(audit_cost)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                match remove_selected_redundant_extension(
                    &current,
                    function_index,
                    instruction.id,
                    environment,
                    budget,
                ) {
                    Ok(removal) => {
                        validation_steps = validation_steps
                            .checked_add(audit_cost)
                            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                        return Ok(PreAllocationPass::Applied {
                            transformation:
                                ValidatedPreAllocationTransformation::RedundantExtension(removal),
                            declined,
                            evaluated,
                            validation_steps,
                        });
                    }
                    Err(
                        error @ (RedundantExtensionError::WorkBudgetExceeded
                        | RedundantExtensionError::IdentityOverflow
                        | RedundantExtensionError::ReplayMismatch),
                    ) => {
                        return Err(OptimizedPreAllocationCustodyError::RedundantExtension(
                            error,
                        ));
                    }
                    Err(_) => declined += 1,
                }
            }
        }
    }
    Ok(PreAllocationPass::Clean {
        candidates: evaluated,
        declined,
        validation_steps,
    })
}

/// Source-bound candidate discovery for the address-fold family: every
/// displacement-carrying consumer — `Load8`/`Load16`/`Load32`/`Load64`,
/// the byte-addressable referent `Store` widths, and `AddressOffset` — in
/// the current validated plan, in function/block/instruction order,
/// evaluated until the first admissible one commits. Declines and hard
/// failures follow the copy pass's contract exactly, and the measured-step
/// contract is the fold family's own.
fn address_fold_pass(
    current: SelectedProgramRef<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<PreAllocationPass, OptimizedPreAllocationCustodyError> {
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    for (function_index, function) in current.selected_plan().functions.iter().enumerate() {
        let audit_cost = address_fold_measured_steps(current.selected_plan(), function)
            .map_err(OptimizedPreAllocationCustodyError::AddressFold)?;
        for block in &function.blocks {
            for instruction in &block.instructions {
                if !is_fold_consumer(instruction.kind) {
                    continue;
                }
                evaluated += 1;
                validation_steps = validation_steps
                    .checked_add(audit_cost)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                match fold_selected_address(
                    &current,
                    function_index,
                    instruction.id,
                    environment,
                    budget,
                ) {
                    Ok(fold) => {
                        validation_steps = validation_steps
                            .checked_add(audit_cost)
                            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                        return Ok(PreAllocationPass::Applied {
                            transformation: ValidatedPreAllocationTransformation::AddressFold(fold),
                            declined,
                            evaluated,
                            validation_steps,
                        });
                    }
                    Err(
                        error @ (AddressFoldError::WorkBudgetExceeded
                        | AddressFoldError::IdentityOverflow
                        | AddressFoldError::ReplayMismatch),
                    ) => {
                        return Err(OptimizedPreAllocationCustodyError::AddressFold(error));
                    }
                    Err(_) => declined += 1,
                }
            }
        }
    }
    Ok(PreAllocationPass::Clean {
        candidates: evaluated,
        declined,
        validation_steps,
    })
}

/// Source-bound candidate discovery for the constant-boolean family: every
/// flag-reading `MaterializeBoolean*` in the current validated plan, in
/// function/block/instruction order, evaluated until the first admissible
/// one commits. Declines and hard failures follow the copy pass's contract
/// exactly, and the measured-step contract is the fold family's own — the
/// flag-universe and materialize-surface sizes are charged once per pass
/// from the target's rows, and the reached compare's published definitions
/// stay bounded by the flag universe, so the pre-admission charge uses it.
fn constant_boolean_pass(
    current: SelectedProgramRef<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<PreAllocationPass, OptimizedPreAllocationCustodyError> {
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    let (flag_universe, materialize_surface) = constant_boolean_surface_sizes(environment)
        .map_err(OptimizedPreAllocationCustodyError::ConstantBoolean)?;
    // The condition-materialization pair descriptors admit the fold
    // through the environment's own validated catalog — a pure function of
    // the target and the constraint catalog, so the custody replay
    // re-running this pass binds the same declarations.
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints())
            .map_err(OptimizedPreAllocationCustodyError::MachineEffects)?;
    for (function_index, function) in current.selected_plan().functions.iter().enumerate() {
        let (successors, _) = adjacency(function);
        for block in &function.blocks {
            for instruction in &block.instructions {
                if !is_boolean_reader(instruction.kind) {
                    continue;
                }
                evaluated += 1;
                let audit_cost = constant_boolean_measured_steps(
                    current.selected_plan(),
                    function,
                    &successors,
                    instruction.implicit_uses.len(),
                    flag_universe,
                    materialize_surface,
                    flag_universe,
                )
                .map_err(OptimizedPreAllocationCustodyError::ConstantBoolean)?;
                validation_steps = validation_steps
                    .checked_add(audit_cost)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                match fold_selected_constant_boolean(
                    &current,
                    function_index,
                    instruction.id,
                    environment,
                    &effect_catalog,
                    budget,
                ) {
                    Ok(fold) => {
                        validation_steps = validation_steps
                            .checked_add(audit_cost)
                            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                        return Ok(PreAllocationPass::Applied {
                            transformation: ValidatedPreAllocationTransformation::ConstantBoolean(
                                fold,
                            ),
                            declined,
                            evaluated,
                            validation_steps,
                        });
                    }
                    Err(
                        error @ (ConstantBooleanError::WorkBudgetExceeded
                        | ConstantBooleanError::IdentityOverflow
                        | ConstantBooleanError::ReplayMismatch),
                    ) => {
                        return Err(OptimizedPreAllocationCustodyError::ConstantBoolean(error));
                    }
                    Err(_) => declined += 1,
                }
            }
        }
    }
    Ok(PreAllocationPass::Clean {
        candidates: evaluated,
        declined,
        validation_steps,
    })
}

/// Source-bound candidate discovery for the constant-branch family: every
/// flag-reading `ConditionalBranch*` terminator in the current validated
/// plan, in function/block order, evaluated until the first admissible one
/// commits. Declines and hard failures follow the copy pass's contract
/// exactly, and the measured-step contract is the fold family's own — the
/// flag-universe and jump-surface sizes are charged once per pass from the
/// target's rows, and the reached compare's published definitions stay
/// bounded by the flag universe, so the pre-admission charge uses it.
fn constant_branch_pass(
    current: SelectedProgramRef<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<PreAllocationPass, OptimizedPreAllocationCustodyError> {
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    let (flag_universe, jump_surface) = constant_branch_surface_sizes(environment)
        .map_err(OptimizedPreAllocationCustodyError::ConstantBranch)?;
    // The decided-branch pair descriptors admit the control-flow
    // relationship through the environment's own validated catalog — a
    // pure function of the target and the constraint catalog, so the
    // custody replay re-running this pass binds the same declarations.
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints())
            .map_err(OptimizedPreAllocationCustodyError::MachineEffects)?;
    for (function_index, function) in current.selected_plan().functions.iter().enumerate() {
        let (successors, _) = adjacency(function);
        for block in &function.blocks {
            let instruction = terminator_instruction(&block.terminator);
            if !is_branch_reader(instruction.kind) {
                continue;
            }
            evaluated += 1;
            let audit_cost = constant_branch_measured_steps(
                current.selected_plan(),
                function,
                &successors,
                instruction.implicit_uses.len(),
                flag_universe,
                jump_surface,
                flag_universe,
            )
            .map_err(OptimizedPreAllocationCustodyError::ConstantBranch)?;
            validation_steps = validation_steps
                .checked_add(audit_cost)
                .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
            match fold_selected_constant_branch(
                &current,
                function_index,
                instruction.id,
                environment,
                &effect_catalog,
                budget,
            ) {
                Ok(fold) => {
                    validation_steps = validation_steps
                        .checked_add(audit_cost)
                        .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                    return Ok(PreAllocationPass::Applied {
                        transformation: ValidatedPreAllocationTransformation::ConstantBranch(fold),
                        declined,
                        evaluated,
                        validation_steps,
                    });
                }
                Err(
                    error @ (ConstantBranchError::WorkBudgetExceeded
                    | ConstantBranchError::IdentityOverflow
                    | ConstantBranchError::ReplayMismatch),
                ) => {
                    return Err(OptimizedPreAllocationCustodyError::ConstantBranch(error));
                }
                Err(_) => declined += 1,
            }
        }
    }
    Ok(PreAllocationPass::Clean {
        candidates: evaluated,
        declined,
        validation_steps,
    })
}

/// Source-bound candidate discovery for the dead-compare family: every
/// `CompareI64`/`CompareI64Immediate`/`CompareI64Zero` in the current
/// validated plan, in function/block/instruction order, evaluated until
/// the first admissible one commits. Declines and hard failures follow the
/// copy pass's contract exactly, and the measured-step contract is the
/// removal family's own — its per-candidate cost includes the published
/// unit count, so the audit charge is computed per evaluated instruction.
fn dead_compare_pass(
    current: SelectedProgramRef<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<PreAllocationPass, OptimizedPreAllocationCustodyError> {
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    for (function_index, function) in current.selected_plan().functions.iter().enumerate() {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if !is_compare_publisher(instruction.kind) {
                    continue;
                }
                evaluated += 1;
                let audit_cost = dead_compare_measured_steps(
                    current.selected_plan(),
                    function,
                    published_flag_units(instruction),
                )
                .map_err(OptimizedPreAllocationCustodyError::DeadCompare)?;
                validation_steps = validation_steps
                    .checked_add(audit_cost)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                match remove_dead_compare(
                    &current,
                    function_index,
                    instruction.id,
                    environment,
                    budget,
                ) {
                    Ok(removal) => {
                        validation_steps = validation_steps
                            .checked_add(audit_cost)
                            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                        return Ok(PreAllocationPass::Applied {
                            transformation: ValidatedPreAllocationTransformation::DeadCompare(
                                removal,
                            ),
                            declined,
                            evaluated,
                            validation_steps,
                        });
                    }
                    Err(
                        error @ (DeadCompareError::WorkBudgetExceeded
                        | DeadCompareError::IdentityOverflow
                        | DeadCompareError::ReplayMismatch),
                    ) => {
                        return Err(OptimizedPreAllocationCustodyError::DeadCompare(error));
                    }
                    Err(_) => declined += 1,
                }
            }
        }
    }
    Ok(PreAllocationPass::Clean {
        candidates: evaluated,
        declined,
        validation_steps,
    })
}

/// Source-bound candidate discovery for the redundant-compare family:
/// every flag-publisher compare in the current validated plan, in
/// function/block/instruction order, evaluated until the first admissible
/// one commits. Declines and hard failures follow the copy pass's contract
/// exactly, and the measured-step contract is the family's own — its
/// per-candidate cost includes the published unit count over the
/// function's successor adjacency, so the audit charge is computed per
/// evaluated instruction.
fn redundant_compare_pass(
    current: SelectedProgramRef<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<PreAllocationPass, OptimizedPreAllocationCustodyError> {
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    for (function_index, function) in current.selected_plan().functions.iter().enumerate() {
        let (successors, _) = adjacency(function);
        for block in &function.blocks {
            for instruction in &block.instructions {
                if !is_compare_publisher(instruction.kind) {
                    continue;
                }
                evaluated += 1;
                let audit_cost = redundant_compare_measured_steps(
                    current.selected_plan(),
                    function,
                    &successors,
                    published_flag_units(instruction),
                )
                .map_err(OptimizedPreAllocationCustodyError::RedundantCompare)?;
                validation_steps = validation_steps
                    .checked_add(audit_cost)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                match remove_redundant_compare(
                    &current,
                    function_index,
                    instruction.id,
                    environment,
                    budget,
                ) {
                    Ok(removal) => {
                        validation_steps = validation_steps
                            .checked_add(audit_cost)
                            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                        return Ok(PreAllocationPass::Applied {
                            transformation: ValidatedPreAllocationTransformation::RedundantCompare(
                                removal,
                            ),
                            declined,
                            evaluated,
                            validation_steps,
                        });
                    }
                    Err(
                        error @ (RedundantCompareError::WorkBudgetExceeded
                        | RedundantCompareError::IdentityOverflow
                        | RedundantCompareError::ReplayMismatch),
                    ) => {
                        return Err(OptimizedPreAllocationCustodyError::RedundantCompare(error));
                    }
                    Err(_) => declined += 1,
                }
            }
        }
    }
    Ok(PreAllocationPass::Clean {
        candidates: evaluated,
        declined,
        validation_steps,
    })
}

/// Source-bound candidate discovery for the equivalent-compare family:
/// every flag-publisher compare in the current validated plan, in
/// function/block/instruction order, evaluated until the first admissible
/// one commits. Declines and hard failures follow the copy pass's contract
/// exactly, and the measured-step contract is the family's own — its
/// per-candidate cost includes the published unit count over the
/// function's successor adjacency, so the audit charge is computed per
/// evaluated instruction.
fn equivalent_compare_pass(
    current: SelectedProgramRef<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<PreAllocationPass, OptimizedPreAllocationCustodyError> {
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    for (function_index, function) in current.selected_plan().functions.iter().enumerate() {
        let (successors, _) = adjacency(function);
        for block in &function.blocks {
            for instruction in &block.instructions {
                if !is_compare_publisher(instruction.kind) {
                    continue;
                }
                evaluated += 1;
                let audit_cost = equivalent_compare_measured_steps(
                    current.selected_plan(),
                    function,
                    &successors,
                    published_flag_units(instruction),
                )
                .map_err(OptimizedPreAllocationCustodyError::EquivalentCompare)?;
                validation_steps = validation_steps
                    .checked_add(audit_cost)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                match remove_equivalent_compare(
                    &current,
                    function_index,
                    instruction.id,
                    environment,
                    budget,
                ) {
                    Ok(removal) => {
                        validation_steps = validation_steps
                            .checked_add(audit_cost)
                            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                        return Ok(PreAllocationPass::Applied {
                            transformation: ValidatedPreAllocationTransformation::EquivalentCompare(
                                removal,
                            ),
                            declined,
                            evaluated,
                            validation_steps,
                        });
                    }
                    Err(
                        error @ (EquivalentCompareError::WorkBudgetExceeded
                        | EquivalentCompareError::IdentityOverflow
                        | EquivalentCompareError::ReplayMismatch),
                    ) => {
                        return Err(OptimizedPreAllocationCustodyError::EquivalentCompare(error));
                    }
                    Err(_) => declined += 1,
                }
            }
        }
    }
    Ok(PreAllocationPass::Clean {
        candidates: evaluated,
        declined,
        validation_steps,
    })
}

/// The measured work of one discovery pass. `rule_evaluations` counts the
/// candidates the pass reached; `validation_steps` carries the summed
/// measured-step cost each admit — and, for a commit, the independent
/// validate — charged against the per-pass budget.
fn pass_usage(
    candidates: usize,
    evaluated: usize,
    validation_steps: u64,
    commits: usize,
) -> Result<OptimizationWorkUsage, OptimizedPreAllocationCustodyError> {
    let count = |value: usize| {
        u64::try_from(value).map_err(|_| OptimizedPreAllocationCustodyError::WorkOverflow)
    };
    Ok(OptimizationWorkUsage {
        rule_evaluations: count(evaluated)?,
        candidates: count(candidates)?,
        validation_steps,
        commits: count(commits)?,
        iterations: 1,
    })
}

pub(super) fn add_usage(
    left: OptimizationWorkUsage,
    right: OptimizationWorkUsage,
) -> Result<OptimizationWorkUsage, OptimizedPreAllocationCustodyError> {
    Ok(OptimizationWorkUsage {
        rule_evaluations: left
            .rule_evaluations
            .checked_add(right.rule_evaluations)
            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?,
        candidates: left
            .candidates
            .checked_add(right.candidates)
            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?,
        validation_steps: left
            .validation_steps
            .checked_add(right.validation_steps)
            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?,
        commits: left
            .commits
            .checked_add(right.commits)
            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?,
        iterations: left
            .iterations
            .checked_add(right.iterations)
            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?,
    })
}

pub(super) fn ensure_pre_allocation_budget(
    usage: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
) -> Result<(), OptimizedPreAllocationCustodyError> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(
            OptimizedPreAllocationCustodyError::PreAllocationBudgetExceeded {
                required: usage,
                budget,
            },
        )
    }
}

/// Rebuild the analyses the committed step must carry: a transformation may
/// not leave an illegal program behind, so the transformed plan re-stages
/// liveness, ranges, and legality before the step is admitted.
fn complete_transformation(
    transformation: ValidatedPreAllocationTransformation,
    declined: usize,
    evaluated: usize,
    source: &StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedPreAllocationStep, OptimizedPreAllocationCustodyError> {
    let environment = source.register_environment();
    let liveness =
        analyze_liveness(&transformation).map_err(OptimizedPreAllocationCustodyError::Liveness)?;
    let ranges = analyze_live_ranges(&transformation, &liveness)
        .map_err(OptimizedPreAllocationCustodyError::LiveRanges)?;
    let legality = analyze_allocation_legality(
        &ranges,
        source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedPreAllocationCustodyError::AllocationLegality)?;
    let transition_count = legality.receipt().entry_transition_count();
    if transition_count != 0 {
        return Err(OptimizedPreAllocationCustodyError::RemainingTransitions {
            count: transition_count,
        });
    }
    Ok(StagedOptimizedPreAllocationStep {
        transformation,
        liveness,
        ranges,
        legality,
        declined,
        evaluated,
    })
}

/// The position of the last in-block definition of `register` ahead of
/// `position` — the same last-definition scan the family's admission
/// re-derives; discovery uses it only to name producer candidates.
fn last_definition_before(
    instructions: &[SelectedInstruction],
    position: usize,
    register: selected_instructions::VirtualRegisterId,
) -> Option<usize> {
    instructions[..position].iter().rposition(|instruction| {
        instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
        })
    })
}

/// Nominate memory reads through the ordinary catalog sweep and analysis
/// restaging. The surface bound covers both independent walks, including
/// predecessor joins and register transport scans. Unknown aliases refuse.
fn stored_load_forwarding_pass(
    current: SelectedProgramRef<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<PreAllocationPass, OptimizedPreAllocationCustodyError> {
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    let surface = current
        .selected_plan()
        .functions
        .iter()
        .try_fold(1u64, |total, function| {
            let members = function.blocks.iter().try_fold(0u64, |total, block| {
                total
                    .checked_add(block.instructions.len() as u64)?
                    .checked_add(1)
            })?;
            total
                .checked_add(members)?
                .checked_add(function.memory_accesses.len() as u64)?
                .checked_add(function.virtual_registers.len() as u64)
        })
        .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
    let audit_cost = surface
        .checked_mul(surface)
        .and_then(|value| value.checked_mul(surface))
        .and_then(|value| value.checked_mul(2))
        .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
    for (function_index, function) in current.selected_plan().functions.iter().enumerate() {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if !matches!(
                    instruction.kind,
                    SelectedInstructionKind::Load64 { .. }
                        | SelectedInstructionKind::Load32 { .. }
                        | SelectedInstructionKind::Load16 { .. }
                        | SelectedInstructionKind::Load8 { .. }
                        | SelectedInstructionKind::Load8Indexed { .. }
                ) {
                    continue;
                }
                evaluated += 1;
                validation_steps = validation_steps
                    .checked_add(audit_cost)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                if validation_steps > budget.validation_steps() {
                    return Err(OptimizedPreAllocationCustodyError::StoredLoadForwarding(
                        StoredLoadForwardingError::WorkBudgetExceeded,
                    ));
                }
                match forward_selected_stored_load(
                    &current,
                    function_index,
                    instruction.id,
                    environment,
                    budget,
                ) {
                    Ok(forwarding) => {
                        return Ok(PreAllocationPass::Applied {
                            transformation:
                                ValidatedPreAllocationTransformation::StoredLoadForwarding(
                                    forwarding,
                                ),
                            declined,
                            evaluated,
                            validation_steps,
                        });
                    }
                    Err(
                        error @ (StoredLoadForwardingError::WorkBudgetExceeded
                        | StoredLoadForwardingError::IdentityOverflow
                        | StoredLoadForwardingError::ReplayMismatch),
                    ) => {
                        return Err(OptimizedPreAllocationCustodyError::StoredLoadForwarding(
                            error,
                        ));
                    }
                    Err(_) => declined += 1,
                }
            }
        }
    }
    Ok(PreAllocationPass::Clean {
        candidates: evaluated,
        declined,
        validation_steps,
    })
}

/// Source-bound candidate discovery for the copied-call-operand family:
/// every direct internal call — `CallUnit`, `CallScalar`, or
/// `CallAggregate` — in the current validated plan, in
/// function/block/instruction order, each `Use` argument operand naming
/// the instruction whose last definition produced it; a producer that is
/// a `CopyI64` is a candidate pair, evaluated in operand order until the
/// first admissible one commits. A call can reach several copies — one
/// per argument — and each producer id is evaluated once per call.
/// Declines and hard failures follow the copy pass's contract exactly,
/// and the measured-step contract is the family's own: the
/// whole-function scan, the producer scan, the last-definition walk,
/// and the interval audit charged per evaluated pair and again on a
/// commit's independent validation. The retained-call surface binds the
/// environment's validated effect catalog the same way the
/// constant-branch family binds it.

fn copied_call_operand_pass(
    current: SelectedProgramRef<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<PreAllocationPass, OptimizedPreAllocationCustodyError> {
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    // The copied-call-operand pair descriptors admit the retained-call
    // relationship through the environment's own validated catalog — a
    // pure function of the target and the constraint catalog, so the
    // custody replay re-running this pass binds the same declarations.
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints())
            .map_err(OptimizedPreAllocationCustodyError::MachineEffects)?;
    for (function_index, function) in current.selected_plan().functions.iter().enumerate() {
        for block in &function.blocks {
            let audit_cost = copied_call_operand_measured_steps(current.selected_plan(), block)
                .map_err(OptimizedPreAllocationCustodyError::CopiedCallOperand)?;
            for (position, instruction) in block.instructions.iter().enumerate() {
                if !is_call_consumer(instruction) {
                    continue;
                }
                let mut tried = BTreeSet::new();
                for operand in instruction
                    .operands
                    .iter()
                    .filter(|operand| operand.access == RegisterOperandAccess::Use)
                {
                    let Some(producer_index) = last_definition_before(
                        &block.instructions,
                        position,
                        operand.virtual_register,
                    ) else {
                        continue;
                    };
                    let producer = &block.instructions[producer_index];
                    if producer.kind != SelectedInstructionKind::CopyI64
                        || !tried.insert(producer.id)
                    {
                        continue;
                    }
                    evaluated += 1;
                    validation_steps = validation_steps
                        .checked_add(audit_cost)
                        .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                    match fold_selected_copied_call_operand(
                        &current,
                        function_index,
                        producer.id,
                        instruction.id,
                        environment,
                        &effect_catalog,
                        budget,
                    ) {
                        Ok(reroute) => {
                            validation_steps = validation_steps
                                .checked_add(audit_cost)
                                .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                            return Ok(PreAllocationPass::Applied {
                                transformation:
                                    ValidatedPreAllocationTransformation::CopiedCallOperand(reroute),
                                declined,
                                evaluated,
                                validation_steps,
                            });
                        }
                        Err(
                            error @ (CopiedCallOperandError::WorkBudgetExceeded
                            | CopiedCallOperandError::IdentityOverflow
                            | CopiedCallOperandError::ReplayMismatch),
                        ) => {
                            return Err(OptimizedPreAllocationCustodyError::CopiedCallOperand(
                                error,
                            ));
                        }
                        Err(_) => {
                            declined += 1;
                        }
                    }
                }
            }
        }
    }
    Ok(PreAllocationPass::Clean {
        candidates: evaluated,
        declined,
        validation_steps,
    })
}

/// Run discovery sweeps to the joint fixed point over `source`'s admitted
/// plan. Both the executor and the custody replay call this; neither accepts
/// the other's steps as authority.
fn run_passes(
    source: &StagedOptimizedAllocationLegality,
    policy: PreAllocationPolicy,
    iteration_bound: usize,
) -> Result<
    (
        Vec<StagedOptimizedPreAllocationStep>,
        StagedOptimizedPreAllocationAttempt,
        OptimizationWorkUsage,
    ),
    OptimizedPreAllocationCustodyError,
> {
    let budget = source.budget_per_pass();
    let environment = source.register_environment();
    let mut usage = OptimizationWorkUsage::default();
    let mut steps = Vec::new();
    let mut previous_measure = iteration_bound;
    let mut current = SelectedProgramRef::new(source.selected());
    let attempt = loop {
        // The terminal attempt records only the final complete sweep's
        // totals; a mid-sweep commit discards the partial scan and restarts.
        let mut sweep_candidates = 0usize;
        let mut sweep_declined = 0usize;
        let mut committed = false;
        for row in PRE_ALLOCATION_RULE_CATALOG {
            if !policy.contains(row.payload().policy()) {
                continue;
            }
            let pass = match row.optimization() {
                Optimization::SelectedSameBlockCopyI64RemovalV1 => {
                    copy_removal_pass(current, environment, budget)?
                }
                Optimization::SelectedRedundantExtensionRemovalV1 => {
                    extension_removal_pass(current, environment, budget)?
                }
                Optimization::SelectedAddressOffsetFoldV1 => {
                    address_fold_pass(current, environment, budget)?
                }
                Optimization::SelectedConstantBooleanFoldV1 => {
                    constant_boolean_pass(current, environment, budget)?
                }
                Optimization::SelectedConstantBranchFoldV1 => {
                    constant_branch_pass(current, environment, budget)?
                }
                Optimization::SelectedDeadCompareRemovalV1 => {
                    dead_compare_pass(current, environment, budget)?
                }
                Optimization::SelectedRedundantCompareRemovalV1 => {
                    redundant_compare_pass(current, environment, budget)?
                }
                Optimization::SelectedEquivalentCompareRemovalV1 => {
                    equivalent_compare_pass(current, environment, budget)?
                }
                Optimization::SelectedCopiedCallOperandFoldV1 => {
                    copied_call_operand_pass(current, environment, budget)?
                }
                Optimization::SelectedStoredLoadForwardingV1 => {
                    stored_load_forwarding_pass(current, environment, budget)?
                }
                // The catalog is closed over this phase's owned rules.
                _ => continue,
            };
            match pass {
                PreAllocationPass::Clean {
                    candidates,
                    declined,
                    validation_steps,
                } => {
                    usage = add_usage(usage, pass_usage(candidates, 0, validation_steps, 0)?)?;
                    ensure_pre_allocation_budget(usage, budget)?;
                    sweep_candidates += candidates;
                    sweep_declined += declined;
                }
                PreAllocationPass::Applied {
                    transformation,
                    declined,
                    evaluated,
                    validation_steps,
                } => {
                    if steps.len() >= iteration_bound {
                        return Err(
                            OptimizedPreAllocationCustodyError::PreAllocationIterationBoundExceeded {
                                bound: iteration_bound,
                            },
                        );
                    }
                    usage = add_usage(
                        usage,
                        pass_usage(evaluated, evaluated, validation_steps, 1)?,
                    )?;
                    ensure_pre_allocation_budget(usage, budget)?;
                    let step =
                        complete_transformation(transformation, declined, evaluated, source)?;
                    // The joint measure must strictly drop: a copy and its
                    // destination's roster row leave together and rebound
                    // chains shorten, one extension retires into a copy, a
                    // fold skips a producer node in its consumer's operand-0
                    // definition chain, or a folded boolean drops the flag
                    // uses it carried.
                    let current_measure = measure(
                        step.transformation.transformed(),
                        step.legality.receipt().virtual_register_count(),
                    );
                    if current_measure >= previous_measure {
                        return Err(
                            OptimizedPreAllocationCustodyError::PreAllocationMeasureMismatch {
                                previous: previous_measure,
                                current: current_measure,
                            },
                        );
                    }
                    previous_measure = current_measure;
                    steps.push(step);
                    current = SelectedProgramRef::new(
                        &steps.last().expect("applied step").transformation,
                    );
                    committed = true;
                    break;
                }
            }
        }
        if !committed {
            break StagedOptimizedPreAllocationAttempt {
                source_selected: current.selected_identity(),
                candidates: sweep_candidates,
                declined: sweep_declined,
            };
        }
    };
    Ok((steps, attempt, usage))
}

pub(super) fn execute_pre_allocation_optimizations(
    source: StagedOptimizedAllocationLegality,
    selections: OptimizationSelections,
    pre_allocation_selections: OptimizationSelections,
    policy: PreAllocationPolicy,
) -> Result<StagedPreAllocationOptimizationRun, OptimizedPreAllocationCustodyError> {
    let upstream = validate_source(&source)?;
    let budget = source.budget_per_pass();
    let iteration_bound = measure(
        source.selected().selected_plan(),
        source.legality().receipt().virtual_register_count(),
    );
    let (steps, attempt, usage) = run_passes(&source, policy, iteration_bound)?;
    let custody = pre_allocation_custody_receipt(
        upstream,
        &selections,
        &pre_allocation_selections,
        policy,
        &source,
        &steps,
        &attempt,
        budget,
        usage,
        iteration_bound,
    );
    Ok(StagedPreAllocationOptimizationRun {
        source,
        selections,
        pre_allocation_selections,
        policy,
        steps,
        attempt,
        custody,
    })
}

/// Independently replay the retained run: re-resolve the exact selection,
/// re-run discovery and admission over the source plan, re-stage every
/// step's analyses, and require the replayed chain and rebuilt custody
/// receipt to equal the retained ones field for field.
pub fn validate_pre_allocation_optimization_custody(
    run: &StagedPreAllocationOptimizationRun,
) -> Result<StagedPreAllocationOptimizationCustodyReceipt, OptimizedPreAllocationCustodyError> {
    let upstream = validate_source(&run.source)?;
    let expected_budget = run.source.budget_per_pass();
    let pre_allocation = run
        .selections
        .project_phase(optimization_core::OptimizationExecutionPhase::PreAllocation);
    let (projected, policy) = resolve_pre_allocation_rules(&pre_allocation)?;
    if run.selections != *run.source.selections()
        || run.custody.selections != run.source.selections().identity()
        || run.custody.budget != expected_budget
    {
        return Err(OptimizedPreAllocationCustodyError::SelectionProjectionMismatch);
    }
    if projected != run.pre_allocation_selections
        || policy != run.policy
        || run.custody.policy != policy
        || run.custody.pre_allocation_selections != run.pre_allocation_selections.identity()
    {
        return Err(OptimizedPreAllocationCustodyError::SelectionProjectionMismatch);
    }
    let iteration_bound = measure(
        run.source.selected().selected_plan(),
        run.source.legality().receipt().virtual_register_count(),
    );
    let (replayed, terminal, usage) = run_passes(&run.source, policy, iteration_bound)?;
    if replayed != run.steps {
        return Err(OptimizedPreAllocationCustodyError::StepMismatch { step: 0 });
    }
    if terminal != run.attempt {
        return Err(OptimizedPreAllocationCustodyError::TerminalAttemptMismatch);
    }
    let receipt = pre_allocation_custody_receipt(
        upstream,
        &run.selections,
        &run.pre_allocation_selections,
        policy,
        &run.source,
        &replayed,
        &terminal,
        expected_budget,
        usage,
        iteration_bound,
    );
    if receipt != run.custody {
        return Err(OptimizedPreAllocationCustodyError::ReceiptMismatch);
    }
    Ok(receipt)
}

fn validate_source(
    source: &StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedAllocationLegalityCustodyReceipt, OptimizedPreAllocationCustodyError> {
    validate_optimized_allocation_legality_custody(
        source.live_range_stage(),
        source.allocator_availability(),
        source.legality(),
    )
    .map_err(OptimizedPreAllocationCustodyError::UpstreamLegality)
}

#[allow(clippy::too_many_arguments)]
fn pre_allocation_custody_receipt(
    source_receipt: StagedOptimizedAllocationLegalityCustodyReceipt,
    selections: &OptimizationSelections,
    pre_allocation_selections: &OptimizationSelections,
    policy: PreAllocationPolicy,
    source: &StagedOptimizedAllocationLegality,
    steps: &[StagedOptimizedPreAllocationStep],
    attempt: &StagedOptimizedPreAllocationAttempt,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    iteration_bound: usize,
) -> StagedPreAllocationOptimizationCustodyReceipt {
    let (final_selected, final_liveness, final_ranges, final_legality, final_vregs) =
        match steps.last() {
            Some(step) => (
                step.transformation.transformed_selected(),
                step.liveness.receipt().identity(),
                step.ranges.receipt().identity(),
                step.legality.receipt().identity(),
                step.legality.receipt().virtual_register_count(),
            ),
            None => (
                source.selected().receipt().identity(),
                source.liveness().receipt().identity(),
                source.ranges().receipt().identity(),
                source.legality().receipt().identity(),
                source.legality().receipt().virtual_register_count(),
            ),
        };
    let mut receipt = StagedPreAllocationOptimizationCustodyReceipt {
        identity: PreAllocationOptimizationCompletionIdentity::from_canonical_bytes(b"pending"),
        source: source_receipt,
        selections: selections.identity(),
        pre_allocation_selections: pre_allocation_selections.identity(),
        policy,
        budget,
        usage,
        iteration_bound,
        removal_count: steps.len(),
        initial_virtual_register_count: source.legality().receipt().virtual_register_count(),
        iterations: steps.iter().map(iteration_receipt).collect(),
        attempt: attempt_receipt(attempt),
        final_selected,
        final_liveness,
        final_ranges,
        final_legality,
        final_virtual_register_count: final_vregs,
    };
    receipt.identity = pre_allocation_completion_identity(&receipt);
    receipt
}

fn iteration_receipt(
    step: &StagedOptimizedPreAllocationStep,
) -> StagedOptimizedPreAllocationIterationReceipt {
    StagedOptimizedPreAllocationIterationReceipt {
        source_selected: step.transformation.source_selected(),
        transformation: step.transformation.identity(),
        function_index: step.transformation.function_index(),
        instruction: step.transformation.instruction(),
        transformed_selected: step.transformation.transformed_selected(),
        fresh_liveness: step.liveness.receipt().identity(),
        fresh_ranges: step.ranges.receipt().identity(),
        fresh_legality: step.legality.receipt().identity(),
        declined: step.declined,
        evaluated: step.evaluated,
    }
}

fn attempt_receipt(
    attempt: &StagedOptimizedPreAllocationAttempt,
) -> StagedOptimizedPreAllocationAttemptReceipt {
    StagedOptimizedPreAllocationAttemptReceipt {
        source_selected: attempt.source_selected,
        candidates: attempt.candidates,
        declined: attempt.declined,
    }
}

/// Canonical identity of the completed pre-allocation run: the upstream
/// custody chain, both selection identities, the admitted policy, measured
/// work, every iteration receipt — including the exact transformation family
/// — the terminal clean sweep, and the final analysis identities are all
/// part of the durable record.
fn pre_allocation_completion_identity(
    receipt: &StagedPreAllocationOptimizationCustodyReceipt,
) -> PreAllocationOptimizationCompletionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.pre-allocation-optimization-completion.v6\0");
    let source = receipt.source;
    for identity in [
        source.optimization().bytes(),
        source.manifest().bytes(),
        source.register_environment().bytes(),
        source.allocator_availability().bytes(),
        source.selected().bytes(),
        source.liveness().bytes(),
        source.ranges().bytes(),
        source.legality().bytes(),
        receipt.selections.bytes(),
        receipt.pre_allocation_selections.bytes(),
    ] {
        canonical.extend_from_slice(&identity);
    }
    canonical.extend_from_slice(&receipt.policy.canonical_bits().to_le_bytes());
    canonical.extend_from_slice(&receipt.budget.encode());
    canonical.extend_from_slice(&receipt.usage.encode());
    for count in [
        receipt.iteration_bound,
        receipt.removal_count,
        receipt.initial_virtual_register_count,
        receipt.iterations.len(),
    ] {
        encode_count(&mut canonical, count);
    }
    for iteration in &receipt.iterations {
        canonical.extend_from_slice(&iteration.source_selected().bytes());
        match iteration.transformation() {
            crate::PreAllocationTransformationIdentity::CopyRemoval(identity) => {
                canonical.push(1);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::RedundantExtension(identity) => {
                canonical.push(2);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::AddressFold(identity) => {
                canonical.push(3);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::ConstantBoolean(identity) => {
                canonical.push(4);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::ConstantBranch(identity) => {
                canonical.push(5);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::DeadCompare(identity) => {
                canonical.push(6);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::RedundantCompare(identity) => {
                canonical.push(7);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::EquivalentCompare(identity) => {
                canonical.push(8);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::CopiedCallOperand(identity) => {
                canonical.push(9);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::StoredLoadForwarding(identity) => {
                canonical.push(10);
                canonical.extend_from_slice(&identity.bytes());
            }
        }
        encode_count(&mut canonical, iteration.function_index());
        canonical.extend_from_slice(&iteration.instruction().0.to_le_bytes());
        canonical.extend_from_slice(&iteration.transformed_selected().bytes());
        canonical.extend_from_slice(&iteration.fresh_liveness().bytes());
        canonical.extend_from_slice(&iteration.fresh_ranges().bytes());
        canonical.extend_from_slice(&iteration.fresh_legality().bytes());
        encode_count(&mut canonical, iteration.declined());
        encode_count(&mut canonical, iteration.evaluated());
    }
    canonical.extend_from_slice(&receipt.attempt.source_selected().bytes());
    encode_count(&mut canonical, receipt.attempt.candidates());
    encode_count(&mut canonical, receipt.attempt.declined());
    canonical.extend_from_slice(&receipt.final_selected.bytes());
    canonical.extend_from_slice(&receipt.final_liveness.bytes());
    canonical.extend_from_slice(&receipt.final_ranges.bytes());
    canonical.extend_from_slice(&receipt.final_legality.bytes());
    encode_count(&mut canonical, receipt.final_virtual_register_count);
    PreAllocationOptimizationCompletionIdentity::from_canonical_bytes(&canonical)
}

fn encode_count(canonical: &mut Vec<u8>, count: usize) {
    canonical.extend_from_slice(
        &u64::try_from(count)
            .expect("pre-allocation completion count fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod stored_load_catalog_tests {
    use super::*;
    use register_environment::baseline_target_register_environment;
    use target::NativeTarget;

    #[test]
    fn stored_load_scan_commits_and_accounts_for_work() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = crate::rewrites::load_forwarding::tests::fixture(target);
        let budget = OptimizationWorkBudget::new(100, 100, 1_000_000, 100, 100).unwrap();
        match stored_load_forwarding_pass(SelectedProgramRef::new(&source), &environment, budget)
            .unwrap()
        {
            PreAllocationPass::Applied {
                transformation,
                evaluated,
                validation_steps,
                ..
            } => {
                assert!(matches!(
                    transformation,
                    ValidatedPreAllocationTransformation::StoredLoadForwarding(_)
                ));
                assert_eq!(evaluated, 1);
                assert!(validation_steps > 0);
                assert_ne!(
                    transformation.source_selected(),
                    transformation.transformed_selected()
                );
            }
            PreAllocationPass::Clean { .. } => panic!("the stored load must actually commit"),
        }
        let exhausted = OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap();
        assert!(matches!(
            stored_load_forwarding_pass(SelectedProgramRef::new(&source), &environment, exhausted),
            Err(OptimizedPreAllocationCustodyError::StoredLoadForwarding(
                StoredLoadForwardingError::WorkBudgetExceeded
            ))
        ));
    }
}
