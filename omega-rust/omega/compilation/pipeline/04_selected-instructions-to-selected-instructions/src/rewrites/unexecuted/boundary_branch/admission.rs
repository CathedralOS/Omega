//! Shared admission for boundary-decided conditional-branch folding:
//! locate the named strict-ordering conditional-branch terminator, confirm
//! its emitted zero-operand shape, partition its implicit uses by the flag
//! universe — flag units resolve to the one compare through the shared
//! reaching walk, non-flag units must lie in the jump row's implicit
//! surface when the fold lands on `Jump` — and decide the outcome from the
//! compare's operand poles: a far pole or two known operands select the
//! `Jump`'s successor outright, while a near pole collapses the ordering
//! to the nonzero condition on the identical published flag state.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterUnitId};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedTerminator, terminator_instruction,
};

use super::{BoundaryBranchError, Outcome, branch_domain, outcome};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::{
    ConditionStateError, adjacency, backward_cone, boundary_operands, entry_index, instruction_at,
    reaching_event,
};

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
    let block_index = function
        .blocks
        .iter()
        .position(|block| terminator_instruction(&block.terminator).id == branch)
        .ok_or(BoundaryBranchError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let branch_instruction = terminator_instruction(&block.terminator);
    // The terminator must be the emitted zero-operand strict-ordering flag
    // reader: the variant/kind pairing must match — a
    // `ConditionalBranchU64LessThan` terminator carrying a nonzero kind is
    // not the shape selection emits — explicit operands are never present,
    // and a use roster holding no flag unit has no condition to decide.
    let signed = branch_domain(&block.terminator, branch_instruction)
        .ok_or(BoundaryBranchError::UnsupportedInstruction)?;
    if !branch_instruction.operands.is_empty() || branch_instruction.implicit_uses.is_empty() {
        return Err(BoundaryBranchError::UnsupportedInstruction);
    }
    let keys = environment.selected_keys();
    // The flag universe partitions the branch's implicit uses: the units
    // the target's three compare rows publish are the condition state the
    // fold decides, and every other observed unit must survive on the jump
    // row's own implicit surface when the fold lands on `Jump`.
    let mut flag_universe = BTreeSet::new();
    for key in [
        keys.compare_i64,
        keys.compare_i64_immediate,
        keys.compare_i64_zero,
    ] {
        flag_universe.extend(
            environment
                .constraint(key)
                .ok_or(BoundaryBranchError::ConstraintMismatch)?
                .implicit_defs
                .iter()
                .copied(),
        );
    }
    let jump_row = environment
        .constraint(keys.jump)
        .ok_or(BoundaryBranchError::ConstraintMismatch)?;
    let jump_surface: BTreeSet<RegisterUnitId> = jump_row
        .implicit_uses
        .iter()
        .chain(jump_row.implicit_defs.iter())
        .chain(jump_row.clobbers.iter())
        .copied()
        .collect();
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // branch's block — only their entry sets can feed the result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(BoundaryBranchError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the branch reads must reach from the same compare:
    // on every execution path to the terminator, the last event touching
    // each used flag unit is that one instruction, and the unit is among
    // its published definitions — a clobber there would leave the observed
    // value unknown. The compare stays published for readers the rewrite
    // leaves behind.
    let read_position = block.instructions.len();
    let mut flag_uses = 0usize;
    let mut compare_site = None;
    for unit in &branch_instruction.implicit_uses {
        if !flag_universe.contains(unit) {
            continue;
        }
        flag_uses += 1;
        let event = reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            read_position,
            *unit,
        )?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(BoundaryBranchError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    if flag_uses == 0 {
        return Err(BoundaryBranchError::UnsupportedInstruction);
    }
    let compare_site = compare_site.ok_or(BoundaryBranchError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if branch_instruction
        .implicit_uses
        .iter()
        .filter(|unit| flag_universe.contains(unit))
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(BoundaryBranchError::UnsupportedUse);
    }
    let (left, right) = boundary_operands(function, compare_instruction)?;
    let outcome = outcome(signed, left, right).ok_or(BoundaryBranchError::UnsupportedLiteral)?;
    let row = match outcome {
        Outcome::Constant(_) => {
            super::constant_row(jump_row, branch_instruction, &flag_universe, &jump_surface)?
        }
        Outcome::NonZero => {
            super::nonzero_flag_resolution(
                function,
                entry,
                &successors,
                &cone,
                block_index,
                read_position,
                &flag_universe,
                branch_instruction,
                compare_instruction,
                compare_site,
                &keys,
            )?;
            None
        }
    };
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    // Producer scans walk the whole function once per compared register —
    // two at most. The flag walk's shared setup resolves every edge's
    // target index, fills the predecessor lists, and marks the backward
    // cone. Each walked unit pays its partition checks — flag-universe and
    // jump-surface membership and the compare-definitions lookup — plus
    // the block-prefix scan and — on an in-block miss — every block's
    // stream for its last event, after which entry-set propagation
    // requeues a block only while its set grows: a set holds at most one
    // element per event site plus the unknown marker, so pops stay under
    // `blocks × (elements + 1)` and each pop visits its out-edges — no
    // more than the widest terminator's — at a bounded union cost.
    // Charging the collapse's full flag-universe walk on top of the branch
    // use roster keeps the bound independent of the outcome split.
    let elements = function_scan
        .checked_add(1)
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(BoundaryBranchError::IdentityOverflow)?,
        )
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .and_then(|total| {
            total.checked_add(
                flag_universe
                    .len()
                    .checked_add(jump_surface.len())?
                    .checked_add(compare_instruction.implicit_defs.len())?,
            )
        })
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(BoundaryBranchError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .and_then(|total| total.checked_add(flag_universe.len()))
        .and_then(|total| total.checked_add(jump_surface.len()))
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let reach_scan = branch_instruction
        .implicit_uses
        .len()
        .checked_add(flag_universe.len())
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
        block_index,
        branch_id: branch,
        provenance: branch_instruction.provenance.clone(),
        outcome,
        row,
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
