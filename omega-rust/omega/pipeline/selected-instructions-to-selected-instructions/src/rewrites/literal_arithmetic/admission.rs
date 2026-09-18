//! Shared admission for literal arithmetic selection: locate the named
//! `ExactAddI64`/`ExactSubtractI64`, confirm its clean
//! `[use left, use right, def result]` register shape, and prove one operand's
//! unique defining instruction is a `MaterializeI64` publishing a literal the
//! target's immediate arithmetic form encodes — operand 1 for either kind,
//! operand 0 only for the commutative add.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedTerminator, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::LiteralArithmeticError;
use crate::ValidatedSelectedAnalysis;

/// The unsigned bound both target encoders enforce for the immediate
/// arithmetic forms: each `u12` gate admits `Unsigned(v)` only for
/// `v <= 4095`, and the selected-lowering pair rules declare the same limit.
const ARITHMETIC_IMMEDIATE_LIMIT: u64 = 4095;

/// Which register operand the folded literal occupied in the
/// register-register form. Operand 1 is the addend/subtrahend position both
/// immediate forms encode; operand 0 folds only for `ExactAddI64`, where the
/// commute moves the literal into the immediate field and the former right
/// operand becomes the surviving register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LiteralPosition {
    Left,
    Right,
}

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub instruction_index: usize,
    pub instruction_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    /// The surviving register input: the operand that stays a register read.
    pub input: VirtualRegisterId,
    /// The result register the rewritten `Def` operand keeps defining.
    pub result: VirtualRegisterId,
    /// The rewritten instruction's kind: `ExactAddI64Immediate` or
    /// `ExactSubtractI64Immediate` carrying the literal and the operation's
    /// obligation and accepted fact.
    pub kind: SelectedInstructionKind,
    /// The selected form's own constraint row.
    pub row: &'source RegisterInstructionConstraint,
}

/// Every instruction of `block`, including the one its terminator carries: a
/// register definition there still counts toward the unique-producer rule.
fn block_instructions(block: &SelectedBlock) -> impl Iterator<Item = &SelectedInstruction> {
    let terminator = match &block.terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    };
    block.instructions.iter().chain(std::iter::once(terminator))
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
    (bits <= ARITHMETIC_IMMEDIATE_LIMIT).then_some(bits)
}

/// The literal one operand register provably reads: exactly one instruction
/// in the function may define `register` — a `UseDef` rewrite or a
/// terminator-carried definition counts as a second one — and it must be a
/// `MaterializeI64` whose own shape is the emitted `[def]` record with no
/// unit traffic. The literal guarantee then holds wherever the operation
/// could read the register, not only at one definition site.
fn operand_literal(
    function: &SelectedFunction,
    register: VirtualRegisterId,
) -> Result<u64, LiteralArithmeticError> {
    let mut producers = function.blocks.iter().flat_map(|block| {
        block_instructions(block).filter(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
            })
        })
    });
    let producer = producers
        .next()
        .ok_or(LiteralArithmeticError::UnsupportedProducer)?;
    if producers.next().is_some() {
        return Err(LiteralArithmeticError::UnsupportedProducer);
    }
    let SelectedInstructionKind::MaterializeI64 { value } = producer.kind else {
        return Err(LiteralArithmeticError::UnsupportedProducer);
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
        return Err(LiteralArithmeticError::UnsupportedProducer);
    }
    admitted_literal(value).ok_or(LiteralArithmeticError::UnsupportedLiteral)
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    instruction_id: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, LiteralArithmeticError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(LiteralArithmeticError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(LiteralArithmeticError::SourceMismatch)?;
    let (block_index, instruction_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == instruction_id)
                .map(|instruction_index| (block_index, instruction_index))
        })
        .ok_or(LiteralArithmeticError::SourceMismatch)?;
    let instruction = &function.blocks[block_index].instructions[instruction_index];
    let (rewritten_key, commutes) = match instruction.kind {
        SelectedInstructionKind::ExactAddI64 { .. } => {
            (environment.selected_keys().add_i64_immediate, true)
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            (environment.selected_keys().subtract_i64_immediate, false)
        }
        _ => return Err(LiteralArithmeticError::UnsupportedInstruction),
    };
    // The operation must be the emitted `[use left, use right, def result]`
    // triple: nothing else (no implicit uses, fixed views, ties, or early
    // clobbers) rides along that the immediate form would silently drop.
    // Implicit definitions and clobbers stay as emitted: the source row must
    // declare exactly that surface below, and the rewritten row must publish
    // the identical one.
    if instruction.operands.len() != 3 {
        return Err(LiteralArithmeticError::UnsupportedInstruction);
    }
    let left_operand = &instruction.operands[0];
    let right_operand = &instruction.operands[1];
    let result_operand = &instruction.operands[2];
    if left_operand.operand != 0
        || left_operand.access != RegisterOperandAccess::Use
        || right_operand.operand != 1
        || right_operand.access != RegisterOperandAccess::Use
        || result_operand.operand != 2
        || result_operand.access != RegisterOperandAccess::Def
        || instruction.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
        || !instruction.implicit_uses.is_empty()
    {
        return Err(LiteralArithmeticError::UnsupportedInstruction);
    }
    let result = result_operand.virtual_register;
    // Operand 1 is the position both immediate forms encode; operand 0 folds
    // only when the operation commutes. When operand 1 does not resolve to
    // an admitted literal an add retries on operand 0; if that also refuses,
    // an out-of-bound literal is the more precise refusal when either side
    // found one, and otherwise operand 0's refusal stands.
    let (position, input, literal) = match operand_literal(function, right_operand.virtual_register)
    {
        Ok(literal) => (
            LiteralPosition::Right,
            left_operand.virtual_register,
            literal,
        ),
        Err(right_error) => {
            if !commutes {
                return Err(right_error);
            }
            match operand_literal(function, left_operand.virtual_register) {
                Ok(literal) => (
                    LiteralPosition::Left,
                    right_operand.virtual_register,
                    literal,
                ),
                Err(left_error) => {
                    return Err(match (right_error, left_error) {
                        (LiteralArithmeticError::UnsupportedLiteral, _)
                        | (_, LiteralArithmeticError::UnsupportedLiteral) => {
                            LiteralArithmeticError::UnsupportedLiteral
                        }
                        (_, error) => error,
                    });
                }
            }
        }
    };
    let find_register = |register| {
        function
            .virtual_registers
            .iter()
            .find(|entry| entry.id == register)
    };
    let left_register = find_register(left_operand.virtual_register).ok_or(
        if position == LiteralPosition::Left {
            LiteralArithmeticError::UnsupportedProducer
        } else {
            LiteralArithmeticError::UnsupportedUse
        },
    )?;
    let right_register = find_register(right_operand.virtual_register).ok_or(
        if position == LiteralPosition::Right {
            LiteralArithmeticError::UnsupportedProducer
        } else {
            LiteralArithmeticError::UnsupportedUse
        },
    )?;
    let input_register = find_register(input).ok_or(LiteralArithmeticError::UnsupportedUse)?;
    let result_register =
        find_register(result).ok_or(LiteralArithmeticError::ConstraintMismatch)?;
    // The operation's own constraint row must declare the same
    // `[use, use, def]` shape its operands carry, at the operand classes the
    // roster assigns, and the instruction must carry exactly the implicit
    // surface its row declares — an implicit unit it invented has no
    // declared surface to preserve.
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(LiteralArithmeticError::ConstraintMismatch)?;
    if row.operands.len() != 3
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[0].class != left_register.class
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || row.operands[1].class != right_register.class
        || row.operands[2].operand != 2
        || row.operands[2].access != RegisterOperandAccess::Def
        || row.operands[2].class != result_register.class
        || row.implicit_uses != instruction.implicit_uses
        || row.implicit_defs != instruction.implicit_defs
        || row.clobbers != instruction.clobbers
    {
        return Err(LiteralArithmeticError::ConstraintMismatch);
    }
    let kind = match instruction.kind {
        SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        } => SelectedInstructionKind::ExactAddI64Immediate {
            immediate: IntegerValue::Unsigned(u128::from(literal)),
            obligation,
            accepted_fact,
        },
        SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: IntegerValue::Unsigned(u128::from(literal)),
            obligation,
            accepted_fact,
        },
        _ => return Err(LiteralArithmeticError::UnsupportedInstruction),
    };
    let selected_row = environment
        .constraint(rewritten_key)
        .ok_or(LiteralArithmeticError::ConstraintMismatch)?;
    // The selected row must be the `[use surviving, def result]` shape at
    // the roster's classes and must publish exactly the implicit surface the
    // operation carried — the same implicit uses, definitions, and clobbers.
    if selected_row.operands.len() != 2
        || selected_row.operands[0].operand != 0
        || selected_row.operands[0].access != RegisterOperandAccess::Use
        || selected_row.operands[0].class != input_register.class
        || selected_row.operands[0].fixed_view.is_some()
        || selected_row.operands[0].tied_to.is_some()
        || selected_row.operands[0].early_clobber
        || selected_row.operands[1].operand != 1
        || selected_row.operands[1].access != RegisterOperandAccess::Def
        || selected_row.operands[1].class != result_register.class
        || selected_row.operands[1].fixed_view.is_some()
        || selected_row.operands[1].tied_to.is_some()
        || selected_row.operands[1].early_clobber
        || selected_row.implicit_uses != instruction.implicit_uses
        || selected_row.implicit_defs != instruction.implicit_defs
        || selected_row.clobbers != instruction.clobbers
    {
        return Err(LiteralArithmeticError::ConstraintMismatch);
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
            // instructions locates and counts each candidate literal's
            // producers.
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .ok_or(LiteralArithmeticError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| LiteralArithmeticError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(LiteralArithmeticError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        instruction_index,
        instruction_id,
        provenance: instruction.provenance.clone(),
        input,
        result,
        kind,
        row: selected_row,
    })
}

/// The one-instruction proposal shape shared with replay: the target's own
/// immediate arithmetic row supplies the operand interface while the
/// surviving input register, the result register, the instruction identity,
/// and the operation's provenance stay with the result.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: admitted.instruction_id,
        kind: admitted.kind,
        constraint: admitted.row.key,
        operands: admitted
            .row
            .operands
            .iter()
            .zip([admitted.input, admitted.result])
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
