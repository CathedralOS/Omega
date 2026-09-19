//! Shared admission for address folding: locate the named displacement-
//! carrying consumer, confirm its clean two-operand shape, and prove the
//! pointer operand's last definition before it in the same block is an
//! `AddressOffset` whose own base register still holds the read value.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    VirtualRegisterId,
};

use super::AddressFoldError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub consumer_index: usize,
    /// The producer's base register: the consumer's operand 0 rebinds to it.
    pub input: VirtualRegisterId,
    /// The consumer's kind with the combined displacement.
    pub kind: SelectedInstructionKind,
}

/// The access-width scale the shared displacement bound multiplies by 4095,
/// and the operand-1 access the kind's canonical form declares. Only forms
/// whose operand 0 is a referent base pointer and whose kind carries a
/// `byte_offset` can fold: indexed, slot, packed, hosted, and register-only
/// forms have no referent displacement to absorb.
fn consumer_shape(kind: SelectedInstructionKind) -> Option<(u32, RegisterOperandAccess)> {
    use SelectedInstructionKind::*;
    let scale = match kind {
        Load8 { .. } | AddressOffset { .. } => 1,
        Load16 { .. } => 2,
        Load32 { .. } => 4,
        Load64 { .. } => 8,
        // The referent store exists only at the byte-addressable widths
        // every target encodes; another width is a malformed form.
        Store { byte_size, .. } => match byte_size {
            1 | 2 | 4 | 8 => u32::from(byte_size),
            _ => return None,
        },
        _ => return None,
    };
    let second = match kind {
        Load8 { .. } | Load16 { .. } | Load32 { .. } | Load64 { .. } | AddressOffset { .. } => {
            RegisterOperandAccess::Def
        }
        Store { .. } => RegisterOperandAccess::Use,
        _ => return None,
    };
    Some((scale, second))
}

/// The combined displacement must encode under the bound the plan's
/// architecture admits for the consumer's form: the AArch64 scaled unsigned
/// immediate (`displacement` a multiple of `scale`, `displacement / scale <=
/// 4095`), or x86-64's unscaled disp32, whose positive half admits every
/// nonnegative byte offset through `i32::MAX` (`byte_offset` carries no
/// negative values).
fn admitted_offset(combined: u32, scale: u32, architecture: target::Architecture) -> bool {
    match architecture {
        target::Architecture::Aarch64 => combined.is_multiple_of(scale) && combined / scale <= 4095,
        target::Architecture::X86_64 => combined <= i32::MAX as u32,
    }
}

/// The instruction defining `register` at `position`: the last instruction
/// in the block before it carrying a non-`Use` operand on that register.
/// Blocks execute in order, so that definition is the value the consumer
/// observes; edge and entry bindings can only reach it when no body
/// instruction defines the register, in which case there is no in-block
/// `AddressOffset` producer to fold.
fn last_definition_before(
    block_instructions: &[SelectedInstruction],
    position: usize,
    register: VirtualRegisterId,
) -> Option<usize> {
    block_instructions[..position]
        .iter()
        .rposition(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
            })
        })
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    access: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, AddressFoldError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(AddressFoldError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(AddressFoldError::SourceMismatch)?;
    let (block_index, consumer_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == access)
                .map(|consumer_index| (block_index, consumer_index))
        })
        .ok_or(AddressFoldError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let consumer = &block.instructions[consumer_index];
    // The consumer must be the emitted two-operand addressing form: operand
    // zero is the referent base pointer `Use`, operand one is the result
    // `Def` (loads and address projections) or the stored value `Use`
    // (referent store), and nothing else — no implicit unit traffic, fixed
    // views, ties, or early clobbers — rides along that the fold would
    // silently keep on a malformed shape.
    let (scale, second_access) =
        consumer_shape(consumer.kind).ok_or(AddressFoldError::UnsupportedInstruction)?;
    if consumer.operands.len() != 2
        || !consumer.implicit_uses.is_empty()
        || !consumer.implicit_defs.is_empty()
        || !consumer.clobbers.is_empty()
    {
        return Err(AddressFoldError::UnsupportedInstruction);
    }
    let pointer_operand = &consumer.operands[0];
    let second_operand = &consumer.operands[1];
    if pointer_operand.operand != 0
        || pointer_operand.access != RegisterOperandAccess::Use
        || second_operand.operand != 1
        || second_operand.access != second_access
        || consumer.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(AddressFoldError::UnsupportedInstruction);
    }
    let pointer = pointer_operand.virtual_register;
    let find_register = |register| {
        function
            .virtual_registers
            .iter()
            .find(|entry| entry.id == register)
    };
    let pointer_register = find_register(pointer).ok_or(AddressFoldError::UnsupportedProducer)?;
    let second_register =
        find_register(second_operand.virtual_register).ok_or(AddressFoldError::UnsupportedUse)?;
    // The consumer's declared constraint row must publish the same operand
    // shape and classes its operands carry, and each operand's class must
    // equal its roster row's — a different row would change what operand
    // zero means, and a roster class the operand does not declare would
    // leave the rebound register publishing the wrong class.
    let row = environment
        .constraint(consumer.constraint)
        .ok_or(AddressFoldError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[0].class != pointer_operand.class
        || pointer_operand.class != pointer_register.class
        || row.operands[1].operand != 1
        || row.operands[1].access != second_access
        || row.operands[1].class != second_operand.class
        || second_operand.class != second_register.class
    {
        return Err(AddressFoldError::ConstraintMismatch);
    }
    // The pointer the consumer reads must come from the block's last
    // definition of it before the consumer, and that instruction must be a
    // clean `AddressOffset`: `[use base, def pointer]` with no implicit
    // traffic or operand constraints. A projection that rewrote its own
    // base (`base == pointer`) leaves operand zero reading the projected
    // value, not the base — refusing rather than folding a shifted sum.
    let producer_index = last_definition_before(&block.instructions, consumer_index, pointer)
        .ok_or(AddressFoldError::UnsupportedProducer)?;
    let producer = &block.instructions[producer_index];
    let SelectedInstructionKind::AddressOffset {
        byte_offset: producer_offset,
    } = producer.kind
    else {
        return Err(AddressFoldError::UnsupportedProducer);
    };
    if producer.operands.len() != 2
        || !producer.implicit_uses.is_empty()
        || !producer.implicit_defs.is_empty()
        || !producer.clobbers.is_empty()
    {
        return Err(AddressFoldError::UnsupportedProducer);
    }
    let base_operand = &producer.operands[0];
    let result_operand = &producer.operands[1];
    if base_operand.operand != 0
        || base_operand.access != RegisterOperandAccess::Use
        || result_operand.operand != 1
        || result_operand.access != RegisterOperandAccess::Def
        || result_operand.virtual_register != pointer
        || producer.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(AddressFoldError::UnsupportedProducer);
    }
    let base = base_operand.virtual_register;
    if base == pointer {
        return Err(AddressFoldError::UnsupportedProducer);
    }
    let base_register = find_register(base).ok_or(AddressFoldError::UnsupportedUse)?;
    // The rebound operand keeps the consumer operand's declared class, so
    // the base register must publish that class — and the producer's own
    // row must declare the `[use, def]` shape its operands carry at the
    // classes the roster assigns.
    if base_register.class != pointer_operand.class {
        return Err(AddressFoldError::UnsupportedUse);
    }
    let producer_row = environment
        .constraint(producer.constraint)
        .ok_or(AddressFoldError::ConstraintMismatch)?;
    if producer_row.operands.len() != 2
        || producer_row.operands[0].operand != 0
        || producer_row.operands[0].access != RegisterOperandAccess::Use
        || producer_row.operands[0].class != base_operand.class
        || base_operand.class != base_register.class
        || producer_row.operands[1].operand != 1
        || producer_row.operands[1].access != RegisterOperandAccess::Def
        || producer_row.operands[1].class != result_operand.class
        || result_operand.class != pointer_register.class
    {
        return Err(AddressFoldError::ConstraintMismatch);
    }
    // Between the producer and the consumer nothing may redefine the base:
    // the folded operand reads the base at the consumer's position, which
    // must be the value the producer offset. The producer itself cannot be
    // inside the interval, and the consumer's own definitions are safe —
    // operand uses read pre-definition — so only the open interval is
    // scanned. The pointer needs no scan: the producer is by construction
    // its last definition before the consumer.
    for instruction in &block.instructions[producer_index + 1..consumer_index] {
        if instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == base
        }) {
            return Err(AddressFoldError::UnsupportedUse);
        }
    }
    let byte_offset = match consumer.kind {
        SelectedInstructionKind::Load8 { byte_offset }
        | SelectedInstructionKind::Load16 { byte_offset }
        | SelectedInstructionKind::Load32 { byte_offset }
        | SelectedInstructionKind::Load64 { byte_offset }
        | SelectedInstructionKind::Store { byte_offset, .. }
        | SelectedInstructionKind::AddressOffset { byte_offset } => byte_offset,
        _ => return Err(AddressFoldError::UnsupportedInstruction),
    };
    let combined = byte_offset
        .checked_add(producer_offset)
        .ok_or(AddressFoldError::UnsupportedOffset)?;
    if !admitted_offset(combined, scale, plan.target.architecture) {
        return Err(AddressFoldError::UnsupportedOffset);
    }
    let kind = match consumer.kind {
        SelectedInstructionKind::Load8 { .. } => SelectedInstructionKind::Load8 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load16 { .. } => SelectedInstructionKind::Load16 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load32 { .. } => SelectedInstructionKind::Load32 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load64 { .. } => SelectedInstructionKind::Load64 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Store { byte_size, .. } => SelectedInstructionKind::Store {
            byte_offset: combined,
            byte_size,
        },
        SelectedInstructionKind::AddressOffset { .. } => SelectedInstructionKind::AddressOffset {
            byte_offset: combined,
        },
        _ => return Err(AddressFoldError::UnsupportedInstruction),
    };
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            // A backward scan of this block locates the pointer's last
            // definition; the interval scan then audits the instructions
            // between it and the consumer for a base redefinition.
            total
                .checked_add(block.instructions.len())?
                .checked_add(block.instructions.len())
        })
        .ok_or(AddressFoldError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| AddressFoldError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(AddressFoldError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        consumer_index,
        input: base,
        kind,
    })
}

/// The one-instruction proposal shape shared with replay: the consumer
/// keeps its identity, constraint row, operands, implicit surfaces, and
/// provenance; only the kind's displacement and operand zero's register
/// change.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
    let mut instruction = admitted.function.blocks[admitted.block_index].instructions
        [admitted.consumer_index]
        .clone();
    instruction.kind = admitted.kind;
    instruction.operands[0].virtual_register = admitted.input;
    instruction
}
