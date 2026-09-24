//! Joint pre-allocation execution, fixed-point iteration, and independent
//! replay.
//!
//! One sweep walks [`PRE_ALLOCATION_RULE_CATALOG`] in canonical descriptor
//! order, running each enabled family's discovery pass over the current
//! program. The first admissible candidate commits and the whole sweep
//! restarts over the transformed plan: an extension removal publishes a
//! `CopyI64` the copy-removal pass then owns, so families share the joint
//! fixed point rather than each converging alone.
use crate::CopyRemovalError;
use crate::OptimizedPreAllocationCustodyError;
use crate::PreAllocationPolicy;
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
use crate::analyze_allocation_legality;
use crate::analyze_live_ranges;
use crate::analyze_liveness;
use crate::resolve_pre_allocation_rules;
use crate::rewrites::{
    copy_removal_measured_steps, redundant_extension_measured_steps, remove_selected_copy,
    remove_selected_redundant_extension,
};
use crate::validate_optimized_allocation_legality_custody;
use optimization_core::{
    Optimization, OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
    PreAllocationOptimizationCompletionIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionKind, SelectedInstructionPlan};

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

/// The joint measure a committed step must drop by exactly one: the plan's
/// virtual registers plus its remaining extension candidates. A copy removal
/// drops the copy and its destination's roster row; an extension removal
/// retires one extension while keeping every register.
fn measure(plan: &SelectedInstructionPlan, virtual_registers: usize) -> usize {
    let extensions = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| is_extension_candidate(instruction.kind))
        .count();
    virtual_registers
        .checked_add(extensions)
        .expect("pre-allocation measure fits usize")
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
                    // The joint measure must drop by exactly one: a copy and
                    // its destination's roster row leave together, or one
                    // extension retires into a copy.
                    let current_measure = measure(
                        step.transformation.transformed(),
                        step.legality.receipt().virtual_register_count(),
                    );
                    if previous_measure.checked_sub(1) != Some(current_measure) {
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
    canonical.extend_from_slice(b"omega.pre-allocation-optimization-completion.v2\0");
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
