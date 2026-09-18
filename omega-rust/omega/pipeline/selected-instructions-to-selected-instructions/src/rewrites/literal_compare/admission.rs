//! Shared admission for literal compare/test selection: locate the named
//! `CompareI64`, confirm its clean `[use left, use right]` register shape,
//! and prove the right operand's unique defining instruction is a
//! `MaterializeI64` publishing a literal the target's immediate compare form
//! encodes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::LiteralCompareError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::block_instructions;

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
    /// The surviving register input: the compare's left (minuend) operand.
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

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, LiteralCompareError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(LiteralCompareError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(LiteralCompareError::SourceMismatch)?;
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
        .ok_or(LiteralCompareError::SourceMismatch)?;
    let compare_instruction = &function.blocks[block_index].instructions[compare_index];
    if compare_instruction.kind != SelectedInstructionKind::CompareI64 {
        return Err(LiteralCompareError::UnsupportedInstruction);
    }
    // The compare must be the emitted `[use left, use right]` flag-publishing
    // pair: operand zero reads the minuend, operand one reads the subtrahend,
    // and nothing else (no implicit uses, clobbers, fixed views, ties, or
    // early clobbers) rides along that the immediate form would silently
    // drop. Implicit definitions stay: the rewritten row must publish the
    // identical ones below.
    if compare_instruction.operands.len() != 2
        || !compare_instruction.implicit_uses.is_empty()
        || !compare_instruction.clobbers.is_empty()
    {
        return Err(LiteralCompareError::UnsupportedInstruction);
    }
    let left_operand = &compare_instruction.operands[0];
    let right_operand = &compare_instruction.operands[1];
    if left_operand.operand != 0
        || left_operand.access != RegisterOperandAccess::Use
        || right_operand.operand != 1
        || right_operand.access != RegisterOperandAccess::Use
        || compare_instruction.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(LiteralCompareError::UnsupportedInstruction);
    }
    let input = left_operand.virtual_register;
    let literal_register = right_operand.virtual_register;
    let find_register = |register| {
        function
            .virtual_registers
            .iter()
            .find(|entry| entry.id == register)
    };
    let input_register = find_register(input).ok_or(LiteralCompareError::UnsupportedUse)?;
    let literal_register_row =
        find_register(literal_register).ok_or(LiteralCompareError::UnsupportedProducer)?;
    // The compare's own constraint row must declare the same `[use, use]`
    // shape its operands carry, at the operand classes the roster assigns.
    let row = environment
        .constraint(compare_instruction.constraint)
        .ok_or(LiteralCompareError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[0].class != input_register.class
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || row.operands[1].class != literal_register_row.class
    {
        return Err(LiteralCompareError::ConstraintMismatch);
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
        .ok_or(LiteralCompareError::UnsupportedProducer)?;
    if producers.next().is_some() {
        return Err(LiteralCompareError::UnsupportedProducer);
    }
    let SelectedInstructionKind::MaterializeI64 { value } = producer.kind else {
        return Err(LiteralCompareError::UnsupportedProducer);
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
        return Err(LiteralCompareError::UnsupportedProducer);
    }
    let literal = admitted_literal(value).ok_or(LiteralCompareError::UnsupportedLiteral)?;
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
        .ok_or(LiteralCompareError::ConstraintMismatch)?;
    // The selected row must be the single-`use` shape at the surviving
    // input's class and must publish exactly the implicit surface the
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
        return Err(LiteralCompareError::ConstraintMismatch);
    }
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
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .ok_or(LiteralCompareError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| LiteralCompareError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(LiteralCompareError::WorkBudgetExceeded);
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
/// surviving input register, the instruction identity, and the compare's
/// provenance stay with the result.
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
