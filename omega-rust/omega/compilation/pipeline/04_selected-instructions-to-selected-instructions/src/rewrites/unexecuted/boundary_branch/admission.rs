//! Shared admission for boundary-decided conditional-branch folding:
//! locate the named strict-ordering conditional-branch terminator, confirm
//! its emitted zero-operand shape, partition its implicit uses by the flag
//! universe — flag units resolve to the one compare through the shared
//! reaching walk, non-flag units must lie in the jump row's implicit
//! surface when the fold lands on `Jump` — and decide the outcome from the
//! compare's operand poles: a far pole or two known operands select the
//! `Jump`'s successor outright, while a near pole collapses the ordering
//! to the nonzero condition on the identical published flag state.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedTerminator, terminator_instruction,
};

use super::{BoundaryBranchError, Outcome};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::ConditionStateError;

impl From<ConditionStateError> for BoundaryBranchError {
    fn from(error: ConditionStateError) -> Self {
        match error {
            ConditionStateError::Use => BoundaryBranchError::UnsupportedUse,
            ConditionStateError::Producer | ConditionStateError::Literal => {
                BoundaryBranchError::UnsupportedLiteral
            }
        }
    }
}

/// What the resolved operand poles force on the branch's strict ordering
/// over `left - right`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub branch_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    pub outcome: Outcome,
    /// The jump row the decided fold rebuilds on — `Outcome::Constant`
    /// only; the collapse retains the branch's own row.
    pub row: Option<&'source RegisterInstructionConstraint>,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, BoundaryBranchError> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            BoundaryBranchError::SourceMismatch
        })?;
    // The branch names its terminator-carried instruction: the block whose
    // terminator instruction carries the id holds the read position at the
    // end of its body stream.
    let derivation =
        super::derive_branch_fold(function, branch, environment, BoundaryBranchError::from)?;
    let cone = crate::rewrites::cone_propagation(function, &derivation.successors, || {
        BoundaryBranchError::IdentityOverflow
    })?;
    let per_unit = Some(cone.per_unit_base)
        .and_then(|total| {
            total.checked_add(
                derivation
                    .flag_universe
                    .len()
                    .checked_add(derivation.jump_surface.len())?
                    .checked_add(derivation.compare_defs)?,
            )
        })
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let walk_setup = Some(cone.walk_setup_base)
        .and_then(|total| total.checked_add(derivation.flag_universe.len()))
        .and_then(|total| total.checked_add(derivation.jump_surface.len()))
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let function_scan = cone.function_scan;
    let reach_scan = derivation
        .implicit_uses
        .checked_add(derivation.flag_universe.len())
        .and_then(|total| total.checked_mul(per_unit))
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let steps = crate::rewrites::plan_instruction_scan(plan)
        .and_then(|total| {
            total
                .checked_add(function_scan.checked_mul(2)?)?
                .checked_add(reach_scan)
        })
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || BoundaryBranchError::IdentityOverflow,
        || BoundaryBranchError::WorkBudgetExceeded,
    )?;
    Ok(Admission {
        function,
        block_index: derivation.block_index,
        branch_id: branch,
        provenance: derivation.provenance,
        outcome: derivation.outcome,
        row: derivation.row,
    })
}

/// The one-terminator proposal shape shared with replay. A decided
/// predicate becomes the target's own jump row carrying the selected
/// successor record verbatim; a near-pole collapse keeps the source
/// instruction whole — the conditional-branch kinds share one row — and
/// swaps only its kind field for `ConditionalBranchNonZero`, republishing
/// `when_less`/`when_not_less` as `when_nonzero`/`when_zero`. Either way
/// the branch's instruction identity and provenance stay with the
/// terminator.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedTerminator {
    let block = &admitted.function.blocks[admitted.block_index];
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
        _ => unreachable!("admission gated the terminator variant"),
    };
    match admitted.outcome {
        Outcome::Constant(holds) => {
            let row = admitted.row.expect("constant outcome carries its row");
            SelectedTerminator::Jump {
                instruction: SelectedInstruction {
                    id: admitted.branch_id,
                    kind: SelectedInstructionKind::Jump,
                    constraint: row.key,
                    operands: Vec::new(),
                    implicit_uses: row.implicit_uses.clone(),
                    implicit_defs: row.implicit_defs.clone(),
                    clobbers: row.clobbers.clone(),
                    provenance: admitted.provenance.clone(),
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
