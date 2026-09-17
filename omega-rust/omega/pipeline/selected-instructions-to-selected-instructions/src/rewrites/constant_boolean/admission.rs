//! Shared admission for constant condition materialization: locate the
//! named `MaterializeBoolean*`, confirm its clean `[def result]` shape,
//! resolve every implicit flag unit it reads to the same in-block compare
//! definition, and prove that compare's operands compile-time constant so
//! the observed predicate is decidable.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedTerminator, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::ConstantBooleanError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub block_index: usize,
    pub materialization_index: usize,
    pub materialization_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    /// The boolean result register: the rewritten materialization defines
    /// it unchanged.
    pub result: VirtualRegisterId,
    /// The folded predicate outcome the rewritten `MaterializeI64` carries.
    pub value: bool,
    /// The materialize row the rewritten instruction is built from.
    pub row: &'source RegisterInstructionConstraint,
}

/// Every instruction of `block`, including the one its terminator carries:
/// a register definition there still counts toward the unique-producer
/// rule, and a flag event there still ends a unit's live range.
fn block_instructions(block: &SelectedBlock) -> impl Iterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
}

fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

/// The sixty-four-bit pattern a `MaterializeI64` publishes. The literal
/// must fit the register it is written into: a signed value is its i64
/// pattern, an unsigned value its u64 pattern, and anything wider is a
/// malformed materialization rather than a foldable literal.
fn literal_bits(value: IntegerValue) -> Option<u64> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value).ok().map(|value| value as u64),
        IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
    }
}

/// The bit pattern a `CompareI64Immediate` encodes: the target forms carry
/// an unsigned twelve-bit payload, so any other value is malformed here.
fn immediate_bits(value: IntegerValue) -> Option<u64> {
    match value {
        IntegerValue::Signed(value) => u64::try_from(value).ok(),
        IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
    }
}

/// The unique-producer guarantee the literal folds use, lifted to a bit
/// pattern: exactly one instruction in the function may define `register`
/// — a `UseDef` rewrite or a terminator-carried definition counts as a
/// second one — and it must be a `MaterializeI64` whose own shape is the
/// emitted `[def]` record with no unit traffic. The literal guarantee then
/// holds wherever the compare could read the register, not only at one
/// site.
fn materialized_bits(
    function: &SelectedFunction,
    register: VirtualRegisterId,
) -> Result<u64, ConstantBooleanError> {
    let mut producers = function.blocks.iter().flat_map(|block| {
        block_instructions(block).filter(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
            })
        })
    });
    let producer = producers
        .next()
        .ok_or(ConstantBooleanError::UnsupportedProducer)?;
    if producers.next().is_some() {
        return Err(ConstantBooleanError::UnsupportedProducer);
    }
    let SelectedInstructionKind::MaterializeI64 { value } = producer.kind else {
        return Err(ConstantBooleanError::UnsupportedProducer);
    };
    if producer.operands.len() != 1
        || producer.operands[0].operand != 0
        || producer.operands[0].access != RegisterOperandAccess::Def
        || producer.operands[0].virtual_register != register
        || producer.operands[0].fixed_view.is_some()
        || producer.operands[0].tied_to.is_some()
        || producer.operands[0].early_clobber
        || !producer.implicit_uses.is_empty()
        || !producer.implicit_defs.is_empty()
        || !producer.clobbers.is_empty()
    {
        return Err(ConstantBooleanError::UnsupportedProducer);
    }
    literal_bits(value).ok_or(ConstantBooleanError::UnsupportedLiteral)
}

/// A plain `Use` operand at `position`, carrying no fixed view, tie, or
/// early clobber: the register the flag computation reads must be the
/// operand's own, not a restricted view of it.
fn plain_use(
    instruction: &SelectedInstruction,
    position: usize,
) -> Result<VirtualRegisterId, ConstantBooleanError> {
    let operand = instruction
        .operands
        .get(position)
        .ok_or(ConstantBooleanError::UnsupportedUse)?;
    if operand.operand != position as u16
        || operand.access != RegisterOperandAccess::Use
        || operand.fixed_view.is_some()
        || operand.tied_to.is_some()
        || operand.early_clobber
    {
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    Ok(operand.virtual_register)
}

/// The `(left, right)` bit patterns the compare's published flag state
/// describes — `left - right` in the compare's own direction — admitted
/// only when both are compile-time constant.
fn constant_operands(
    function: &SelectedFunction,
    compare: &SelectedInstruction,
) -> Result<(u64, u64), ConstantBooleanError> {
    match compare.kind {
        SelectedInstructionKind::CompareI64 => {
            if compare.operands.len() != 2 {
                return Err(ConstantBooleanError::UnsupportedUse);
            }
            let left = plain_use(compare, 0)?;
            let right = plain_use(compare, 1)?;
            // `register - register` is zero on every lane: the flag state
            // is constant whatever the register holds, so no producer is
            // needed for either side.
            if left == right {
                return Ok((0, 0));
            }
            Ok((
                materialized_bits(function, left)?,
                materialized_bits(function, right)?,
            ))
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            if compare.operands.len() != 1 {
                return Err(ConstantBooleanError::UnsupportedUse);
            }
            let left = plain_use(compare, 0)?;
            let immediate =
                immediate_bits(immediate).ok_or(ConstantBooleanError::UnsupportedLiteral)?;
            Ok((materialized_bits(function, left)?, immediate))
        }
        SelectedInstructionKind::CompareI64Zero => {
            if compare.operands.len() != 1 {
                return Err(ConstantBooleanError::UnsupportedUse);
            }
            let left = plain_use(compare, 0)?;
            Ok((materialized_bits(function, left)?, 0))
        }
        _ => Err(ConstantBooleanError::UnsupportedUse),
    }
}

/// The constant a flag-reading `MaterializeBoolean*` materializes when the
/// compare observed `(left, right)` — the predicates the selected catalog
/// defines over `left - right`, evaluated at compile time.
fn predicate_outcome(kind: SelectedInstructionKind, left: u64, right: u64) -> Option<bool> {
    match kind {
        SelectedInstructionKind::MaterializeBooleanEqual => Some(left == right),
        SelectedInstructionKind::MaterializeBooleanU64LessThan => Some(left < right),
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            Some((left as i64) < (right as i64))
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => Some(left <= right),
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            Some((left as i64) <= (right as i64))
        }
        _ => None,
    }
}

/// Resolve one implicit flag use to its reaching event: the nearest
/// earlier in-block instruction whose implicit definitions or clobbers
/// name `unit`. A definition there is the value the materialization
/// observes; a clobber leaves the unit unknown; no in-block event means
/// the unit reaches in from a predecessor — outside this family's bound —
/// and refuses.
fn reaching_event(
    block: &SelectedBlock,
    before: usize,
    unit: RegisterUnitId,
) -> Result<usize, ConstantBooleanError> {
    block.instructions[..before]
        .iter()
        .rposition(|instruction| {
            instruction.implicit_defs.contains(&unit) || instruction.clobbers.contains(&unit)
        })
        .ok_or(ConstantBooleanError::UnsupportedUse)
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, ConstantBooleanError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConstantBooleanError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ConstantBooleanError::SourceMismatch)?;
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
        .ok_or(ConstantBooleanError::SourceMismatch)?;
    let materialization_instruction =
        &function.blocks[block_index].instructions[materialization_index];
    // The materialization must be the emitted `[def result]` flag reader:
    // one plain definition, no implicit definitions or clobbers, and a
    // nonempty flag-use roster — a boolean observing no condition state has
    // no constant to evaluate.
    if predicate_outcome(materialization_instruction.kind, 0, 0).is_none()
        || materialization_instruction.operands.len() != 1
        || materialization_instruction.implicit_uses.is_empty()
        || !materialization_instruction.implicit_defs.is_empty()
        || !materialization_instruction.clobbers.is_empty()
    {
        return Err(ConstantBooleanError::UnsupportedInstruction);
    }
    let result_operand = &materialization_instruction.operands[0];
    if result_operand.operand != 0
        || result_operand.access != RegisterOperandAccess::Def
        || result_operand.fixed_view.is_some()
        || result_operand.tied_to.is_some()
        || result_operand.early_clobber
    {
        return Err(ConstantBooleanError::UnsupportedInstruction);
    }
    let result = result_operand.virtual_register;
    let result_register = function
        .virtual_registers
        .iter()
        .find(|entry| entry.id == result)
        .ok_or(ConstantBooleanError::UnsupportedUse)?;
    let block = &function.blocks[block_index];
    // Every flag unit the materialization reads must reach from the same
    // compare: the nearest earlier event for each used unit is that one
    // instruction, and the unit is among its published definitions — a
    // clobber there would leave the observed value unknown. Other readers
    // of the same units are unaffected: the compare stays.
    let mut compare_index = None;
    for unit in &materialization_instruction.implicit_uses {
        let event = reaching_event(block, materialization_index, *unit)?;
        if compare_index.is_some_and(|index| index != event) {
            return Err(ConstantBooleanError::UnsupportedUse);
        }
        compare_index = Some(event);
    }
    let compare_index = compare_index.ok_or(ConstantBooleanError::UnsupportedUse)?;
    let compare_instruction = &block.instructions[compare_index];
    if materialization_instruction
        .implicit_uses
        .iter()
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    let (left, right) = constant_operands(function, compare_instruction)?;
    let value = predicate_outcome(materialization_instruction.kind, left, right)
        .ok_or(ConstantBooleanError::UnsupportedInstruction)?;
    // The rewritten instruction is the target's own materialize row: a
    // single `[def]` at the boolean result's class carrying no unit
    // traffic — the fold drops the flag uses, and any implicit definition
    // or clobber the row added would publish unit state the source
    // instruction never had.
    let row = environment
        .constraint(environment.selected_keys().materialize_i64)
        .ok_or(ConstantBooleanError::ConstraintMismatch)?;
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
        return Err(ConstantBooleanError::ConstraintMismatch);
    }
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    // Producer scans walk the whole function once per compared register —
    // two at most — and the flag-unit resolution walks the positions before
    // the materialization once per used unit.
    let reach_scan = materialization_index
        .checked_mul(materialization_instruction.implicit_uses.len())
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
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
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ConstantBooleanError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ConstantBooleanError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        materialization_index,
        materialization_id: materialization,
        provenance: materialization_instruction.provenance.clone(),
        result,
        value,
        row,
    })
}

/// The one-instruction proposal shape shared with replay: the target's own
/// materialize row supplies the operand interface while the boolean's
/// result register, instruction identity, and provenance stay with the
/// result. The folded constant is the predicate outcome the retained
/// compare publishes.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: admitted.materialization_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(u8::from(admitted.value))),
        },
        constraint: admitted.row.key,
        operands: admitted
            .row
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
        implicit_uses: admitted.row.implicit_uses.clone(),
        implicit_defs: admitted.row.implicit_defs.clone(),
        clobbers: admitted.row.clobbers.clone(),
        provenance: admitted.provenance.clone(),
    }
}
