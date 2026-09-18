//! Shared admission for left-operand literal folding: locate the named
//! `CompareI64`, confirm its clean `[use minuend, use subtrahend]` register
//! shape, prove the minuend's unique defining instruction is a
//! `MaterializeI64` publishing a literal the target's immediate compare
//! form encodes, and audit every consumer the compare's condition-state
//! definitions can reach for equality-sensing kinds.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::LiteralMinuendError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{block_instructions, terminator_successors};

/// The unsigned bound both target encoders enforce for
/// `CompareI64Immediate`: each `u12` gate admits `Unsigned(v)` only for
/// `v <= 4095`, and the selected-lowering compare pair rule declares the
/// same limit.
const COMPARE_IMMEDIATE_LIMIT: u64 = 4095;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub compare_index: usize,
    pub compare_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    /// The surviving register input: the compare's right (subtrahend)
    /// operand, which the rewritten immediate form reads as its minuend.
    pub input: VirtualRegisterId,
    /// The rewritten instruction's kind: `CompareI64Zero` or
    /// `CompareI64Immediate` carrying the literal.
    pub kind: SelectedInstructionKind,
    /// The selected form's own constraint row.
    pub row: &'source RegisterInstructionConstraint,
}

/// The bit pattern a `MaterializeI64` publishes, admitted only when it fits
/// the shared twelve-bit unsigned immediate bound. Signed literals fold only
/// at nonnegative values: the rewritten form carries `Unsigned` either way
/// because both encoders admit no other payload.
fn admitted_literal(value: IntegerValue) -> Option<u64> {
    let bits = match value {
        IntegerValue::Signed(value) => u64::try_from(value).ok()?,
        IntegerValue::Unsigned(value) => u64::try_from(value).ok()?,
    };
    (bits <= COMPARE_IMMEDIATE_LIMIT).then_some(bits)
}

/// Whether `kind` reads only the zero condition out of the compare's
/// published flag state. The zero bit is the one predicate the swapped
/// subtraction `register - literal` preserves from `literal - register`;
/// the boolean-equal materialization and the generic conditional branch
/// are its only consumers in the selected catalog. Ordering predicates —
/// the `*LessThan`/`*LessOrEqual` materializations and predicate-aware
/// terminators — observe the inverted relation and refuse.
fn equality_sensing(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::MaterializeBooleanEqual
            | SelectedInstructionKind::ConditionalBranchNonZero
    )
}

/// Audit one condition-state unit the compare defines: walk forward from
/// the instruction after the compare, requiring every reached reader of
/// `unit` to be equality-sensing, until an implicit definition or clobber
/// ends the unit's live range. A unit still live past the terminator
/// resumes at the head of each successor block; an edge naming a block the
/// function does not contain leaves the unit's readers unprovable and
/// refuses. `(block, start)` pairs dedupe the walk, so a loop carrying the
/// unit back into the compare's own block re-scans from its head — the
/// compare's own definition ends the unit there.
fn audit_flag_unit(
    function: &SelectedFunction,
    block_index: usize,
    compare_index: usize,
    unit: RegisterUnitId,
) -> Result<(), LiteralMinuendError> {
    let mut visited = BTreeSet::new();
    let mut frontier = vec![(block_index, compare_index + 1)];
    while let Some((current, start)) = frontier.pop() {
        if !visited.insert((current, start)) {
            continue;
        }
        let block = &function.blocks[current];
        let mut killed = false;
        for instruction in block_instructions(block).skip(start) {
            if instruction.implicit_uses.contains(&unit) && !equality_sensing(instruction.kind) {
                return Err(LiteralMinuendError::UnsupportedUse);
            }
            if instruction.implicit_defs.contains(&unit) || instruction.clobbers.contains(&unit) {
                killed = true;
                break;
            }
        }
        if killed {
            continue;
        }
        for successor in terminator_successors(&block.terminator) {
            let target = function
                .blocks
                .iter()
                .position(|block| block.id == successor.block)
                .ok_or(LiteralMinuendError::UnsupportedUse)?;
            frontier.push((target, 0));
        }
    }
    Ok(())
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, LiteralMinuendError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(LiteralMinuendError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(LiteralMinuendError::SourceMismatch)?;
    let (block_index, compare_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == compare)
                .map(|compare_index| (block_index, compare_index))
        })
        .ok_or(LiteralMinuendError::SourceMismatch)?;
    let compare_instruction = &function.blocks[block_index].instructions[compare_index];
    if compare_instruction.kind != SelectedInstructionKind::CompareI64 {
        return Err(LiteralMinuendError::UnsupportedInstruction);
    }
    // The compare must be the emitted `[use minuend, use subtrahend]`
    // flag-publishing pair: operand zero reads the minuend, operand one
    // reads the subtrahend, and nothing else (no implicit uses, clobbers,
    // fixed views, ties, or early clobbers) rides along that the immediate
    // form would silently drop. Implicit definitions stay: the rewritten
    // row must publish the identical ones below.
    if compare_instruction.operands.len() != 2
        || !compare_instruction.implicit_uses.is_empty()
        || !compare_instruction.clobbers.is_empty()
    {
        return Err(LiteralMinuendError::UnsupportedInstruction);
    }
    let minuend_operand = &compare_instruction.operands[0];
    let subtrahend_operand = &compare_instruction.operands[1];
    if minuend_operand.operand != 0
        || minuend_operand.access != RegisterOperandAccess::Use
        || subtrahend_operand.operand != 1
        || subtrahend_operand.access != RegisterOperandAccess::Use
        || compare_instruction.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(LiteralMinuendError::UnsupportedInstruction);
    }
    let literal_register = minuend_operand.virtual_register;
    let input = subtrahend_operand.virtual_register;
    let find_register = |register| {
        function
            .virtual_registers
            .iter()
            .find(|entry| entry.id == register)
    };
    let literal_register_row =
        find_register(literal_register).ok_or(LiteralMinuendError::UnsupportedProducer)?;
    let input_register = find_register(input).ok_or(LiteralMinuendError::UnsupportedUse)?;
    // The compare's own constraint row must declare the same `[use, use]`
    // shape its operands carry, at the operand classes the roster assigns.
    let row = environment
        .constraint(compare_instruction.constraint)
        .ok_or(LiteralMinuendError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[0].class != literal_register_row.class
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || row.operands[1].class != input_register.class
    {
        return Err(LiteralMinuendError::ConstraintMismatch);
    }
    // Exactly one instruction may define the literal register — a `UseDef`
    // rewrite or a terminator-carried definition counts as a second one —
    // and it must be a `MaterializeI64` whose own shape is the emitted
    // `[def]` record with no unit traffic. The guarantee must hold wherever
    // the compare could read the register, not only at one site.
    let mut producers = function.blocks.iter().flat_map(|block| {
        block_instructions(block).filter(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.access != RegisterOperandAccess::Use
                    && operand.virtual_register == literal_register
            })
        })
    });
    let producer = producers
        .next()
        .ok_or(LiteralMinuendError::UnsupportedProducer)?;
    if producers.next().is_some() {
        return Err(LiteralMinuendError::UnsupportedProducer);
    }
    let SelectedInstructionKind::MaterializeI64 { value } = producer.kind else {
        return Err(LiteralMinuendError::UnsupportedProducer);
    };
    if producer.operands.len() != 1
        || producer.operands[0].operand != 0
        || producer.operands[0].access != RegisterOperandAccess::Def
        || producer.operands[0].virtual_register != literal_register
        || producer.operands[0].fixed_view.is_some()
        || producer.operands[0].tied_to.is_some()
        || producer.operands[0].early_clobber
        || !producer.implicit_uses.is_empty()
        || !producer.implicit_defs.is_empty()
        || !producer.clobbers.is_empty()
    {
        return Err(LiteralMinuendError::UnsupportedProducer);
    }
    let literal = admitted_literal(value).ok_or(LiteralMinuendError::UnsupportedLiteral)?;
    // Zero selects the dedicated test form; any other admitted literal
    // selects the immediate form publishing the identical flag surface.
    let (kind, key) = if literal == 0 {
        (
            SelectedInstructionKind::CompareI64Zero,
            environment.selected_keys().compare_i64_zero,
        )
    } else {
        (
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(u128::from(literal)),
            },
            environment.selected_keys().compare_i64_immediate,
        )
    };
    let selected_row = environment
        .constraint(key)
        .ok_or(LiteralMinuendError::ConstraintMismatch)?;
    // The selected row must be the single-`use` shape at the surviving
    // subtrahend's class and must publish exactly the implicit surface the
    // compare carried — the same flag definitions, and no implicit uses or
    // clobbers the source instruction did not already have.
    if selected_row.operands.len() != 1
        || selected_row.operands[0].operand != 0
        || selected_row.operands[0].access != RegisterOperandAccess::Use
        || selected_row.operands[0].class != input_register.class
        || selected_row.operands[0].fixed_view.is_some()
        || selected_row.operands[0].tied_to.is_some()
        || selected_row.operands[0].early_clobber
        || selected_row.implicit_uses != compare_instruction.implicit_uses
        || selected_row.implicit_defs != compare_instruction.implicit_defs
        || selected_row.clobbers != compare_instruction.clobbers
    {
        return Err(LiteralMinuendError::ConstraintMismatch);
    }
    // The rewritten form subtracts the literal from the subtrahend register
    // — the swapped comparison. Its zero condition is the source's, but its
    // ordering predicates invert, so every consumer the compare's flag
    // definitions can reach must read the zero condition alone. When both
    // operands name the same register the subtraction itself is unchanged
    // and no reader can observe the rewrite at all.
    if literal_register != input {
        for unit in &compare_instruction.implicit_defs {
            audit_flag_unit(function, block_index, compare_index, *unit)?;
        }
    }
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(LiteralMinuendError::IdentityOverflow)?;
    // The flag audit visits each block once per entry position per defined
    // unit. Only the compare's own block carries a nonzero start, so two
    // visits per block per unit bound the scan; each visit walks the body's
    // instructions and terminator plus, when the unit stays live, the
    // successor edges.
    let edge_count = function
        .blocks
        .iter()
        .map(|block| terminator_successors(&block.terminator).len())
        .sum::<usize>();
    let flag_audit = compare_instruction
        .implicit_defs
        .len()
        .checked_mul(
            function_scan
                .checked_mul(2)
                .and_then(|scan| scan.checked_add(edge_count))
                .ok_or(LiteralMinuendError::IdentityOverflow)?,
        )
        .ok_or(LiteralMinuendError::IdentityOverflow)?;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            // A second scan of this function's instructions and terminator
            // instructions locates and counts the literal's producers.
            total.checked_add(function_scan)
        })
        .and_then(|total| total.checked_add(flag_audit))
        .ok_or(LiteralMinuendError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| LiteralMinuendError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(LiteralMinuendError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        compare_index,
        compare_id: compare,
        provenance: compare_instruction.provenance.clone(),
        input,
        kind,
        row: selected_row,
    })
}

/// The one-instruction proposal shape shared with replay: the target's own
/// immediate or zero compare row supplies the operand interface while the
/// surviving subtrahend register, the instruction identity, and the
/// compare's provenance stay with the result.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: admitted.compare_id,
        kind: admitted.kind,
        constraint: admitted.row.key,
        operands: admitted
            .row
            .operands
            .iter()
            .zip([admitted.input])
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
        implicit_uses: admitted.row.implicit_uses.clone(),
        implicit_defs: admitted.row.implicit_defs.clone(),
        clobbers: admitted.row.clobbers.clone(),
        provenance: admitted.provenance.clone(),
    }
}
