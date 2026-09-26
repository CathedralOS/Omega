//! Shared admission for boundary-decided predicate materialization: locate
//! the named ordering `MaterializeBoolean*`, confirm its clean
//! `[def result]` shape, resolve every implicit flag unit it reads to the
//! same compare definition — in-block or across predecessor edges — and
//! prove one compare operand sits at a carrier-domain pole that fixes the
//! predicate, or collapses it to equality, without consulting the other.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::BoundaryBooleanError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::{
    ConditionStateError, adjacency, backward_cone, boundary_operands, entry_index, instruction_at,
    reaching_event,
};

/// What the pole operand forces on the reader's predicate over
/// `left - right`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Outcome {
    /// The predicate is decided for every surviving operand value: the
    /// reader becomes the target's own `MaterializeI64` of the constant.
    Constant(bool),
    /// The predicate holds exactly at the pole: `x <= 0` unsigned,
    /// `u64::MAX <= x` unsigned, `x <= i64::MIN` signed, and
    /// `i64::MAX <= x` signed each coincide with the equality condition on
    /// the same published state, so the reader keeps its shape and only
    /// its kind becomes `MaterializeBooleanEqual`.
    Equal,
}

/// The four ordering boolean kinds a pole can decide. `MaterializeBooleanEqual`
/// is absent: `left == right` varies with every unknown side, so no single
/// operand pole can fix it.
fn decidable_kind(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::MaterializeBooleanU64LessThan
            | SelectedInstructionKind::MaterializeBooleanI64LessThan
            | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
            | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
    )
}

/// The boundary decision over the compare's resolved operand poles. The
/// strict orderings fold only at the far pole — a strict less-than against
/// the domain minimum, or from the domain maximum, can never hold — and the
/// remaining pole cases collapse to a negation (`x != bound`) that no
/// boolean materialization kind expresses. The inclusive orderings are
/// tautologies at the near pole and equalities at the far one. Constant
/// outcomes take precedence where two rules could fire: `(0, 0)` under
/// `U64LessOrEqual` is decided `true` rather than rewritten to the equality
/// reader that would compute the same value.
fn outcome(
    kind: SelectedInstructionKind,
    left: Option<u64>,
    right: Option<u64>,
) -> Option<Outcome> {
    match kind {
        SelectedInstructionKind::MaterializeBooleanU64LessThan => {
            // `x <u 0` never holds; `u64::MAX <u x` never holds. The
            // remaining poles are `0 <u x` ≡ `x != 0` and `x <u MAX` ≡
            // `x != MAX`, negations no materialization kind selects.
            if right == Some(0) || left == Some(u64::MAX) {
                Some(Outcome::Constant(false))
            } else {
                None
            }
        }
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            // `x <s i64::MIN` never holds; `i64::MAX <s x` never holds.
            if right == Some(i64::MIN as u64) || left == Some(i64::MAX as u64) {
                Some(Outcome::Constant(false))
            } else {
                None
            }
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => {
            // `0 <=u x` and `x <=u u64::MAX` always hold; `x <=u 0` and
            // `u64::MAX <=u x` each hold exactly at equality.
            if left == Some(0) || right == Some(u64::MAX) {
                Some(Outcome::Constant(true))
            } else if right == Some(0) || left == Some(u64::MAX) {
                Some(Outcome::Equal)
            } else {
                None
            }
        }
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            // `i64::MIN <=s x` and `x <=s i64::MAX` always hold;
            // `x <=s i64::MIN` and `i64::MAX <=s x` hold exactly at
            // equality.
            if left == Some(i64::MIN as u64) || right == Some(i64::MAX as u64) {
                Some(Outcome::Constant(true))
            } else if right == Some(i64::MIN as u64) || left == Some(i64::MAX as u64) {
                Some(Outcome::Equal)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub materialization_index: usize,
    pub materialization_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    /// The boolean result register: the rewritten materialization defines
    /// it unchanged under either outcome.
    pub result: VirtualRegisterId,
    pub outcome: Outcome,
    /// The materialize row the constant-folded instruction is built from —
    /// present only for `Outcome::Constant`; the equality collapse reuses
    /// the reader's own row and surface verbatim.
    pub row: Option<&'source RegisterInstructionConstraint>,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, BoundaryBooleanError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(BoundaryBooleanError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(BoundaryBooleanError::SourceMismatch)?;
    let (block_index, materialization_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == materialization)
                .map(|materialization_index| (block_index, materialization_index))
        })
        .ok_or(BoundaryBooleanError::SourceMismatch)?;
    let materialization_instruction =
        &function.blocks[block_index].instructions[materialization_index];
    // The materialization must be the emitted `[def result]` flag reader:
    // one plain definition, no implicit definitions or clobbers, and a
    // nonempty flag-use roster — a boolean observing no condition state has
    // no predicate to decide. The equality kind is refused with the shape:
    // no pole can decide `left == right`.
    if !decidable_kind(materialization_instruction.kind)
        || materialization_instruction.operands.len() != 1
        || materialization_instruction.implicit_uses.is_empty()
        || !materialization_instruction.implicit_defs.is_empty()
        || !materialization_instruction.clobbers.is_empty()
    {
        return Err(BoundaryBooleanError::UnsupportedInstruction);
    }
    let result_operand = &materialization_instruction.operands[0];
    if result_operand.operand != 0
        || result_operand.access != RegisterOperandAccess::Def
        || result_operand.fixed_view.is_some()
        || result_operand.tied_to.is_some()
        || result_operand.early_clobber
    {
        return Err(BoundaryBooleanError::UnsupportedInstruction);
    }
    let result = result_operand.virtual_register;
    let result_register = function
        .virtual_registers
        .iter()
        .find(|entry| entry.id == result)
        .ok_or(BoundaryBooleanError::UnsupportedUse)?;
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // materialization's block — only their entry sets can feed the result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(BoundaryBooleanError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the materialization reads must reach from the same
    // compare: on every execution path to the materialization, the last
    // event touching each used unit is that one instruction, and the unit
    // is among its published definitions — a clobber there would leave the
    // observed value unknown. Other readers of the same units are
    // unaffected: the compare stays.
    let mut compare_site = None;
    for unit in &materialization_instruction.implicit_uses {
        let event = reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            materialization_index,
            *unit,
        )
        .map_err(|_| BoundaryBooleanError::UnsupportedUse)?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(BoundaryBooleanError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    let compare_site = compare_site.ok_or(BoundaryBooleanError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if materialization_instruction
        .implicit_uses
        .iter()
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(BoundaryBooleanError::UnsupportedUse);
    }
    let (left, right) =
        boundary_operands(function, compare_instruction).map_err(|error| match error {
            ConditionStateError::Use => BoundaryBooleanError::UnsupportedUse,
            ConditionStateError::Literal | ConditionStateError::Producer => {
                BoundaryBooleanError::UnsupportedLiteral
            }
        })?;
    let outcome = outcome(materialization_instruction.kind, left, right)
        .ok_or(BoundaryBooleanError::UnsupportedLiteral)?;
    // A decided predicate rewrites to the target's own materialize row: a
    // single `[def]` at the boolean result's class carrying no unit
    // traffic — the fold drops the flag uses, and any implicit definition
    // or clobber the row added would publish unit state the source
    // instruction never had. The equality collapse needs no row at all: the
    // reader keeps its own — every flag-reading boolean kind shares it.
    let row = match outcome {
        Outcome::Constant(_) => {
            let row = environment
                .constraint(environment.selected_keys().materialize_i64)
                .ok_or(BoundaryBooleanError::ConstraintMismatch)?;
            if row.operands.len() != 1
                || row.operands[0].operand != 0
                || row.operands[0].access != RegisterOperandAccess::Def
                || row.operands[0].class != result_register.class
                || row.operands[0].fixed_view.is_some()
                || row.operands[0].tied_to.is_some()
                || row.operands[0].early_clobber
                || !row.implicit_uses.is_empty()
                || !row.implicit_defs.is_empty()
                || !row.clobbers.is_empty()
            {
                return Err(BoundaryBooleanError::ConstraintMismatch);
            }
            Some(row)
        }
        Outcome::Equal => None,
    };
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    // Producer scans walk the whole function once per compared register —
    // two at most. The flag walk's shared setup resolves every edge's
    // target index, fills the predecessor lists, and marks the backward
    // cone. Each used unit then scans the materialization block's prefix
    // and — on an in-block miss — every block's stream for its last event,
    // after which entry-set propagation requeues a block only while its
    // set grows: a set holds at most one element per event site plus the
    // unknown marker, so pops stay under `blocks × (elements + 1)` and
    // each pop visits its out-edges — no more than the widest terminator's
    // — at a bounded union cost.
    let elements = function_scan
        .checked_add(1)
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(BoundaryBooleanError::IdentityOverflow)?,
        )
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(BoundaryBooleanError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let reach_scan = materialization_instruction
        .implicit_uses
        .len()
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
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
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| BoundaryBooleanError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(BoundaryBooleanError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        materialization_index,
        materialization_id: materialization,
        provenance: materialization_instruction.provenance.clone(),
        result,
        outcome,
        row,
    })
}

/// The one-instruction proposal shape shared with replay. A decided
/// predicate becomes the target's own materialize row carrying the outcome,
/// while the boolean's result register, instruction identity, and
/// provenance stay with the result. An equality collapse keeps the source
/// instruction whole — the flag-reading kinds share one row — and swaps
/// only its kind field for `MaterializeBooleanEqual`.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
    match admitted.outcome {
        Outcome::Constant(value) => {
            let row = admitted.row.expect("constant outcome carries its row");
            SelectedInstruction {
                id: admitted.materialization_id,
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(u128::from(u8::from(value))),
                },
                constraint: row.key,
                operands: row
                    .operands
                    .iter()
                    .zip([admitted.result])
                    .map(
                        |(operand, register)| selected_instructions::SelectedOperand {
                            operand: operand.operand,
                            virtual_register: register,
                            access: operand.access,
                            class: operand.class,
                            fixed_view: operand.fixed_view,
                            tied_to: operand.tied_to,
                            early_clobber: operand.early_clobber,
                        },
                    )
                    .collect(),
                implicit_uses: row.implicit_uses.clone(),
                implicit_defs: row.implicit_defs.clone(),
                clobbers: row.clobbers.clone(),
                provenance: admitted.provenance.clone(),
            }
        }
        Outcome::Equal => {
            let source = &admitted.function.blocks[admitted.block_index].instructions
                [admitted.materialization_index];
            let mut rewritten = source.clone();
            rewritten.kind = SelectedInstructionKind::MaterializeBooleanEqual;
            rewritten
        }
    }
}
