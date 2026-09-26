//! Shared admission for constant condition branch folding: locate the
//! named conditional-branch terminator, confirm its emitted zero-operand
//! shape, partition its implicit uses by the flag universe — flag units
//! resolve to the one compare through the shared reaching walk, non-flag
//! units must lie in the jump row's implicit surface — and decide the
//! successor from the compare's compile-time-constant operands.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterUnitId};
use selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedSuccessor, SelectedTerminator,
};

use super::ConstantBranchError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::terminator_instruction;
use crate::rewrites::condition_state::{
    ConditionStateError, adjacency, backward_cone, constant_operands, entry_index, instruction_at,
    reaching_event,
};

impl From<ConditionStateError> for ConstantBranchError {
    fn from(error: ConditionStateError) -> Self {
        match error {
            ConditionStateError::Use => ConstantBranchError::UnsupportedUse,
            ConditionStateError::Producer => ConstantBranchError::UnsupportedProducer,
            ConditionStateError::Literal => ConstantBranchError::UnsupportedLiteral,
        }
    }
}

pub(super) struct Admission<'source> {
    pub block_index: usize,
    pub branch_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    /// The decided successor the rebuilt `Jump` terminator carries
    /// verbatim — bindings, structural payloads, case custody, and fuel
    /// included.
    pub successor: SelectedSuccessor,
    /// The jump row the rebuilt terminator instruction is built from.
    pub row: &'source RegisterInstructionConstraint,
}

/// The successor the branch selects when its compare observed
/// `(left, right)` — the predicates the selected catalog defines over
/// `left - right`, evaluated at compile time: `ConditionalBranchNonZero`
/// is the zero-condition reader selection emits for equality, so it takes
/// `when_nonzero` exactly when the difference is nonzero, and the
/// predicate-aware forms take `when_less` on their strict ordering.
fn decided(
    terminator: &SelectedTerminator,
    kind: SelectedInstructionKind,
    left: u64,
    right: u64,
) -> Option<&SelectedSuccessor> {
    match (terminator, kind) {
        (
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            },
            SelectedInstructionKind::ConditionalBranchNonZero,
        ) => Some(if left != right {
            when_nonzero
        } else {
            when_zero
        }),
        (
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            },
            SelectedInstructionKind::ConditionalBranchU64LessThan,
        ) => Some(if left < right {
            when_less
        } else {
            when_not_less
        }),
        (
            SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            },
            SelectedInstructionKind::ConditionalBranchI64LessThan,
        ) => Some(if (left as i64) < (right as i64) {
            when_less
        } else {
            when_not_less
        }),
        _ => None,
    }
}

/// The branch/kind pairings `decided` admits without consulting operands —
/// used by the shape gate before any flag resolution runs.
fn branch_kind(terminator: &SelectedTerminator, instruction: &SelectedInstruction) -> bool {
    decided(terminator, instruction.kind, 0, 0).is_some()
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, ConstantBranchError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConstantBranchError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ConstantBranchError::SourceMismatch)?;
    // The branch names its terminator-carried instruction: the block whose
    // terminator instruction carries the id holds the read position at the
    // end of its body stream.
    let block_index = function
        .blocks
        .iter()
        .position(|block| terminator_instruction(&block.terminator).id == branch)
        .ok_or(ConstantBranchError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let branch_instruction = terminator_instruction(&block.terminator);
    // The terminator must be the emitted zero-operand flag reader: the
    // variant/kind pairing must match — a `ConditionalBranch` terminator
    // carrying a less-than kind is not the shape selection emits — explicit
    // operands are never present, and a use roster holding no flag unit has
    // no condition to decide.
    if !branch_kind(&block.terminator, branch_instruction)
        || !branch_instruction.operands.is_empty()
        || branch_instruction.implicit_uses.is_empty()
    {
        return Err(ConstantBranchError::UnsupportedInstruction);
    }
    let keys = environment.selected_keys();
    // The flag universe partitions the branch's implicit uses: the units
    // the target's three compare rows publish are the condition state the
    // fold decides, and every other observed unit must survive on the jump
    // row's own implicit surface.
    let mut flag_universe = BTreeSet::new();
    for key in [
        keys.compare_i64,
        keys.compare_i64_immediate,
        keys.compare_i64_zero,
    ] {
        flag_universe.extend(
            environment
                .constraint(key)
                .ok_or(ConstantBranchError::ConstraintMismatch)?
                .implicit_defs
                .iter()
                .copied(),
        );
    }
    let jump_row = environment
        .constraint(keys.jump)
        .ok_or(ConstantBranchError::ConstraintMismatch)?;
    let jump_surface: BTreeSet<RegisterUnitId> = jump_row
        .implicit_uses
        .iter()
        .chain(jump_row.implicit_defs.iter())
        .chain(jump_row.clobbers.iter())
        .copied()
        .collect();
    // The rebuilt terminator publishes the jump row's surface where the
    // branch's stood, so the two must agree on definitions and clobbers:
    // a definition or clobber the jump lacks would hand downstream readers
    // an older event at this site, and one the branch lacked would publish
    // new state the source never had.
    if !jump_row.operands.is_empty()
        || BTreeSet::from_iter(jump_row.implicit_defs.iter().copied())
            != BTreeSet::from_iter(branch_instruction.implicit_defs.iter().copied())
        || BTreeSet::from_iter(jump_row.clobbers.iter().copied())
            != BTreeSet::from_iter(branch_instruction.clobbers.iter().copied())
    {
        return Err(ConstantBranchError::ConstraintMismatch);
    }
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // branch's block — only their entry sets can feed the result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(ConstantBranchError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the branch reads must reach from the same compare:
    // on every execution path to the terminator, the last event touching
    // each used flag unit is that one instruction, and the unit is among
    // its published definitions — a clobber there would leave the observed
    // value unknown. Every other used unit must keep its observation on
    // the jump row's surface. The compare stays published for readers the
    // rewrite leaves behind.
    let read_position = block.instructions.len();
    let mut flag_uses = 0usize;
    let mut compare_site = None;
    for unit in &branch_instruction.implicit_uses {
        if !flag_universe.contains(unit) {
            if !jump_surface.contains(unit) {
                return Err(ConstantBranchError::UnsupportedUse);
            }
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
            return Err(ConstantBranchError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    if flag_uses == 0 {
        return Err(ConstantBranchError::UnsupportedInstruction);
    }
    let compare_site = compare_site.ok_or(ConstantBranchError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if branch_instruction
        .implicit_uses
        .iter()
        .filter(|unit| flag_universe.contains(unit))
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(ConstantBranchError::UnsupportedUse);
    }
    let (left, right) = constant_operands(function, compare_instruction)?;
    let successor = decided(&block.terminator, branch_instruction.kind, left, right)
        .ok_or(ConstantBranchError::UnsupportedInstruction)?
        .clone();
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    // Producer scans walk the whole function once per compared register —
    // two at most. The flag walk's shared setup resolves every edge's
    // target index, fills the predecessor lists, and marks the backward
    // cone. Each used unit then pays its partition checks — flag-universe
    // and jump-surface membership and the compare-definitions lookup —
    // plus, for a flag unit, the block-prefix scan and — on an in-block
    // miss — every block's stream for its last event, after which
    // entry-set propagation requeues a block only while its set grows: a
    // set holds at most one element per event site plus the unknown
    // marker, so pops stay under `blocks × (elements + 1)` and each pop
    // visits its out-edges — no more than the widest terminator's — at a
    // bounded union cost. Charging every use for the full flag walk keeps
    // the bound independent of the partition split.
    let elements = function_scan
        .checked_add(1)
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(ConstantBranchError::IdentityOverflow)?,
        )
        .ok_or(ConstantBranchError::IdentityOverflow)?;
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
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(ConstantBranchError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .and_then(|total| total.checked_add(flag_universe.len()))
        .and_then(|total| total.checked_add(jump_surface.len()))
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let reach_scan = branch_instruction
        .implicit_uses
        .len()
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            total
                .checked_add(function_scan.checked_mul(2)?)?
                .checked_add(reach_scan)
        })
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ConstantBranchError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ConstantBranchError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        branch_id: branch,
        provenance: branch_instruction.provenance.clone(),
        successor,
        row: jump_row,
    })
}

/// The one-terminator proposal shape shared with replay: the target's own
/// jump row — zero-operand by admission — supplies the implicit surface
/// while the branch's instruction identity and provenance stay with the
/// terminator, and the decided successor carries its record verbatim.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedTerminator {
    SelectedTerminator::Jump {
        instruction: SelectedInstruction {
            id: admitted.branch_id,
            kind: SelectedInstructionKind::Jump,
            constraint: admitted.row.key,
            operands: Vec::new(),
            implicit_uses: admitted.row.implicit_uses.clone(),
            implicit_defs: admitted.row.implicit_defs.clone(),
            clobbers: admitted.row.clobbers.clone(),
            provenance: admitted.provenance.clone(),
        },
        successor: admitted.successor.clone(),
    }
}
