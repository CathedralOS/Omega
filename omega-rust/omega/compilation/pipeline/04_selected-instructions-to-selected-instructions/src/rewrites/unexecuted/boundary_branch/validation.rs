//! Independent validation of boundary-decided conditional-branch folding.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! fold's legality from the source records — the terminator-carried
//! instruction's strict-ordering branch/kind pairing and emitted
//! zero-operand shape, the flag partition every implicit use must satisfy
//! (flag units resolving through the shared condition-state walk to the
//! one compare whose published definitions cover them, and — for the
//! decided `Jump` outcome — every other observed unit carried on the jump
//! row's own implicit surface), the compare's resolved operand poles, and
//! the pole-outcome table deciding between a `Jump` fold and the
//! `ConditionalBranchNonZero` collapse — then rebuilds the terminator the
//! contract demands and requires the proposal to carry it in the branch's
//! block. Reinserting the source terminator must restore the complete
//! source by content: every other instruction, register, call,
//! settlement, and function included. A producer admission error
//! therefore fails validation even when the proposal is exactly what that
//! producer emitted.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedTerminator,
    terminator_instruction,
};

use super::{BoundaryBranchError, BoundaryBranchReceipt, Outcome, ValidatedBoundaryBranch};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;

/// The validator's own reconstruction of the fold the contract permits:
/// the admitted branch's block, identity, and emitted provenance, the
/// decided outcome, the jump row a decided fold is built from, the
/// resolved-index successor lists the shared walk ran over, and the
/// surface sizes the measured-step contract charges. It shares no state
/// with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    branch_id: SelectedInstructionId,
    provenance: SelectedInstructionProvenance,
    outcome: Outcome,
    /// The jump row the decided fold rebuilds on — `Outcome::Constant`
    /// only; the collapse retains the branch's own row.
    row: Option<&'source RegisterInstructionConstraint>,
    successors: Vec<Vec<usize>>,
    implicit_uses: usize,
    flag_universe: usize,
    jump_surface: usize,
    compare_defs: usize,
}

/// The family's own mapping of a shared condition-state rejection onto
/// its error vocabulary. The producer's `From` implementation lives in
/// `admission`; the validator maps each site itself so a changed producer
/// mapping cannot relabel its refusal.

/// Reconstruct the legality of folding `branch` from first principles:
/// locate the terminator-carried instruction, verify the emitted
/// branch/kind pairing and zero-operand flag-reader shape, build the flag
/// universe from the target's compare rows and the jump surface from its
/// jump row, run each used flag unit's reaching event on the module's
/// shared condition-state walk, require the one compare site whose
/// published definitions cover them all, and decide the outcome on the
/// validator's own pole table. A decided fold then demands the jump
/// row's exact implicit-surface match and a carrier for every non-flag
/// observation; the collapse instead keeps the branch's own row, so it
/// demands the conditional-branch constraint and resolves every
/// flag-universe unit — not only the ones the source branch declared —
/// to the same published compare. Nothing in this audit reads the
/// producer's admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, BoundaryBranchError> {
    let (_, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            BoundaryBranchError::SourceMismatch
        })?;
    let derivation =
        super::derive_branch_fold(function, branch, environment, BoundaryBranchError::from)?;
    Ok(Reconstructed {
        function,
        block_index: derivation.block_index,
        branch_id: branch,
        provenance: derivation.provenance,
        outcome: derivation.outcome,
        row: derivation.row,
        successors: derivation.successors,
        implicit_uses: derivation.implicit_uses,
        flag_universe: derivation.flag_universe.len(),
        jump_surface: derivation.jump_surface.len(),
        compare_defs: derivation.compare_defs,
    })
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: the shared rewrite scan (one step per block
/// and instruction of the function, one per plan function), two scans of the reconstructed function's blocks for
/// the producer audit, and the shared walk's setup and fixpoint bound
/// charged per walked unit — the branch's use roster plus the whole flag
/// universe, so the bound stays independent of the outcome split.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    reconstructed: &Reconstructed<'_>,
) -> Result<u64, BoundaryBranchError> {
    let function = reconstructed.function;
    let cone = crate::rewrites::cone_propagation(function, &reconstructed.successors, || {
        BoundaryBranchError::IdentityOverflow
    })?;
    let per_unit = Some(cone.per_unit_base)
        .and_then(|total| {
            total.checked_add(
                reconstructed
                    .flag_universe
                    .checked_add(reconstructed.jump_surface)?
                    .checked_add(reconstructed.compare_defs)?,
            )
        })
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let walk_setup = Some(cone.walk_setup_base)
        .and_then(|total| total.checked_add(reconstructed.flag_universe))
        .and_then(|total| total.checked_add(reconstructed.jump_surface))
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let function_scan = cone.function_scan;
    let reach_scan = reconstructed
        .implicit_uses
        .checked_add(reconstructed.flag_universe)
        .and_then(|total| total.checked_mul(per_unit))
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            total
                .checked_add(function_scan.checked_mul(2)?)?
                .checked_add(reach_scan)
        })
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| BoundaryBranchError::IdentityOverflow)
}

/// Build the terminator the contract demands from the validator's own
/// record: a decided predicate becomes the target's own jump row — zero
/// operands by the audit — carrying the selected successor record
/// verbatim while the branch's instruction identity and provenance stay
/// with the terminator; a near-pole collapse keeps the source
/// instruction whole — the conditional-branch kinds share one row — and
/// swaps only its kind field for `ConditionalBranchNonZero`,
/// republishing `when_less`/`when_not_less` as `when_nonzero`/`when_zero`.
/// The producer's `rewritten` constructor is not consulted; both sides
/// derive the same terminator from the source alone.
fn expected(reconstructed: &Reconstructed<'_>) -> SelectedTerminator {
    let block = &reconstructed.function.blocks[reconstructed.block_index];
    let (when_less, when_not_less) = match &block.terminator {
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => (when_less, when_not_less),
        _ => unreachable!("reconstruction gated the terminator variant"),
    };
    match reconstructed.outcome {
        Outcome::Constant(holds) => {
            let row = reconstructed.row.expect("constant outcome carries its row");
            SelectedTerminator::Jump {
                instruction: SelectedInstruction {
                    id: reconstructed.branch_id,
                    kind: SelectedInstructionKind::Jump,
                    constraint: row.key,
                    operands: Vec::new(),
                    implicit_uses: row.implicit_uses.clone(),
                    implicit_defs: row.implicit_defs.clone(),
                    clobbers: row.clobbers.clone(),
                    provenance: reconstructed.provenance.clone(),
                },
                successor: if holds {
                    when_less.clone()
                } else {
                    when_not_less.clone()
                },
            }
        }
        Outcome::NonZero => {
            let mut instruction = terminator_instruction(&block.terminator).clone();
            instruction.kind = SelectedInstructionKind::ConditionalBranchNonZero;
            SelectedTerminator::ConditionalBranch {
                instruction,
                when_nonzero: when_less.clone(),
                when_zero: when_not_less.clone(),
            }
        }
    }
}

/// Reinsert the source terminator: undoing the validator's expected edit
/// must restore the complete source by content — every other
/// instruction, register, roster row, call, and settlement included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
    function_index: usize,
) -> Result<SelectedInstructionPlan, BoundaryBranchError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(function_index)
        .ok_or(BoundaryBranchError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(BoundaryBranchError::ReplayMismatch)?;
    block.terminator = reconstructed.function.blocks[reconstructed.block_index]
        .terminator
        .clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the fold's preconditions from the source, requires the proposed
/// terminator in the branch's block to equal the form its own record
/// produces — the `Jump` a decided pole permits or the
/// `ConditionalBranchNonZero` collapse a near pole permits — and restores
/// the complete source by content: every other instruction, register,
/// roster row, call, and settlement included. The producer's
/// `admission::admit` is never consulted, so a wrong legality decision
/// fails here even when the proposal matches the edit the producer
/// emitted.
pub(crate) fn validate_boundary_branch_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedBoundaryBranch, BoundaryBranchError> {
    let reconstructed = reconstruct(source, function_index, branch, environment)?;
    if measured_steps(source.selected_plan(), &reconstructed)? > budget.validation_steps() {
        return Err(BoundaryBranchError::WorkBudgetExceeded);
    }
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .map(|block| &block.terminator)
        != Some(&expected(&reconstructed))
    {
        return Err(BoundaryBranchError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed, function_index)? != *source.selected_plan() {
        return Err(BoundaryBranchError::ReplayMismatch);
    }
    Ok(sealed!(
        ValidatedBoundaryBranch,
        BoundaryBranchReceipt,
        source,
        proposed
    ))
}
