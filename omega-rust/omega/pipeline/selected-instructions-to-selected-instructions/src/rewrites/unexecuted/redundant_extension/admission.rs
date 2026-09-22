//! Shared admission for redundant-extension removal: locate the named
//! extension, confirm its clean `[use, def]` register shape, and prove the
//! input's unique defining instruction already carries the bits the
//! extension would write — either because the producer fixes the normalized
//! high bits, or because the producer is itself a partial carrier feeding
//! `ZeroExtendU32`, the one extension whose own result leaves them
//! unmeaningful.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::IntegerValue;

use super::RedundantExtensionError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub block_index: usize,
    pub extension_index: usize,
    pub extension_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    pub value: VirtualRegisterId,
    pub output: VirtualRegisterId,
    pub copy: &'source RegisterInstructionConstraint,
}

/// The normalization a `ZeroExtend*`/`SignExtend*` consumer performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Extension {
    /// Result bits at or above `width` are zero.
    Zero { width: u8 },
    /// Result bits at or above `width - 1` replicate bit `width - 1`.
    Sign { width: u8 },
}

fn extension(kind: SelectedInstructionKind) -> Option<Extension> {
    Some(match kind {
        SelectedInstructionKind::ZeroExtendU8 => Extension::Zero { width: 8 },
        SelectedInstructionKind::ZeroExtendU16 => Extension::Zero { width: 16 },
        SelectedInstructionKind::ZeroExtendU32 => Extension::Zero { width: 32 },
        SelectedInstructionKind::SignExtendI8 => Extension::Sign { width: 8 },
        SelectedInstructionKind::SignExtendI16 => Extension::Sign { width: 16 },
        SelectedInstructionKind::SignExtendI32 => Extension::Sign { width: 32 },
        _ => return None,
    })
}

/// The bit guarantee a producer kind makes about its result, or the exact
/// pattern a materialization publishes. `ZeroExtended`, `SignExtended`, and
/// `Pattern` fix the high bits; `MeaningfulLow` is the partial carrier whose
/// contract promises the low `width` bits and fixes nothing above them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Normalization {
    /// Every bit at or above `width` is zero.
    ZeroExtended { width: u8 },
    /// Every bit at or above `width - 1` replicates bit `width - 1`.
    SignExtended { width: u8 },
    /// The low `width` bits carry the meaningful result and the contract
    /// leaves every bit at or above `width` not meaningful.
    MeaningfulLow { width: u8 },
    /// The low sixty-four bits the instruction writes, known exactly.
    Pattern(u64),
}

/// `defined_operand` is the producer operand index that wrote the extension's
/// input register. It decides which register the guarantee describes: a packed
/// load's assembled result is operand one while operand two is instruction
/// scratch whose contents the contract does not describe.
fn producer_normalization(
    kind: SelectedInstructionKind,
    defined_operand: u16,
) -> Option<Normalization> {
    Some(match kind {
        SelectedInstructionKind::Load8 { .. } | SelectedInstructionKind::Load8Indexed => {
            Normalization::ZeroExtended { width: 8 }
        }
        SelectedInstructionKind::Load16 { .. } => Normalization::ZeroExtended { width: 16 },
        SelectedInstructionKind::Load32 { .. } => Normalization::ZeroExtended { width: 32 },
        SelectedInstructionKind::LoadPacked { width, .. } if defined_operand == 1 => {
            Normalization::MeaningfulLow {
                width: width.byte_size() * 8,
            }
        }
        SelectedInstructionKind::ZeroExtendU8 => Normalization::ZeroExtended { width: 8 },
        SelectedInstructionKind::ZeroExtendU16 => Normalization::ZeroExtended { width: 16 },
        SelectedInstructionKind::ZeroExtendU32 => Normalization::MeaningfulLow { width: 32 },
        SelectedInstructionKind::SignExtendI8 => Normalization::SignExtended { width: 8 },
        SelectedInstructionKind::SignExtendI16 => Normalization::SignExtended { width: 16 },
        SelectedInstructionKind::SignExtendI32 => Normalization::SignExtended { width: 32 },
        SelectedInstructionKind::Float32ToBits => Normalization::MeaningfulLow { width: 32 },
        SelectedInstructionKind::MaterializeBooleanEqual
        | SelectedInstructionKind::MaterializeBooleanU64LessThan
        | SelectedInstructionKind::MaterializeBooleanI64LessThan
        | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
        | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            Normalization::ZeroExtended { width: 1 }
        }
        SelectedInstructionKind::MaterializeI64 { value } => {
            let pattern = match value {
                IntegerValue::Signed(value) => value as i64 as u64,
                IntegerValue::Unsigned(value) => value as u64,
            };
            Normalization::Pattern(pattern)
        }
        _ => return None,
    })
}

/// Whether `extension` is the identity on an input carrying `normalization`.
fn identity(extension: Extension, normalization: Normalization) -> bool {
    match (extension, normalization) {
        (Extension::Zero { width }, Normalization::ZeroExtended { width: n }) => n <= width,
        // `ZeroExtendU32` is the one extension whose contract leaves its own
        // result's upper bits unmeaningful: it re-normalizes the low 32 input
        // bits and promises nothing above them. A partial producer is the
        // identity on it exactly when the producer's meaningful content
        // already fits in 32 bits — the copy then propagates the producer's
        // own unspecified surface, identical specified bits over an
        // identically unspecified upper region. A wider partial producer
        // genuinely narrows rather than witnesses, and every other extension
        // fixes result bits a partial producer never promised.
        (Extension::Zero { width: 32 }, Normalization::MeaningfulLow { width: n }) => n <= 32,
        (Extension::Zero { .. }, Normalization::MeaningfulLow { .. }) => false,
        (Extension::Zero { width }, Normalization::Pattern(pattern)) => pattern < (1u64 << width),
        (Extension::Zero { .. }, Normalization::SignExtended { .. }) => false,
        (Extension::Sign { width }, Normalization::SignExtended { width: n }) => n <= width,
        // A zero-normalized input has bit `width - 1` clear whenever `n < m`;
        // replicating that zero leaves the pattern untouched.
        (Extension::Sign { width }, Normalization::ZeroExtended { width: n }) => n < width,
        (Extension::Sign { width }, Normalization::Pattern(pattern)) => {
            let shift = 64 - u32::from(width);
            (((pattern << shift) as i64) >> shift) as u64 == pattern
        }
        (Extension::Sign { .. }, Normalization::MeaningfulLow { .. }) => false,
    }
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    extension_id: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, RedundantExtensionError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RedundantExtensionError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RedundantExtensionError::SourceMismatch)?;
    let (block_index, extension_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == extension_id)
                .map(|extension_index| (block_index, extension_index))
        })
        .ok_or(RedundantExtensionError::SourceMismatch)?;
    let extension_instruction = &function.blocks[block_index].instructions[extension_index];
    let form = extension(extension_instruction.kind)
        .ok_or(RedundantExtensionError::UnsupportedInstruction)?;
    // The extension must be the emitted `[use, def]` carrier pair: operand
    // zero reads the value, operand one defines the result, and nothing else
    // (no fixed views, ties, early clobbers, or implicit unit effects) rides
    // along that a plain copy would silently drop.
    if extension_instruction.operands.len() != 2
        || !extension_instruction.implicit_uses.is_empty()
        || !extension_instruction.implicit_defs.is_empty()
        || !extension_instruction.clobbers.is_empty()
    {
        return Err(RedundantExtensionError::UnsupportedInstruction);
    }
    let use_operand = &extension_instruction.operands[0];
    let def_operand = &extension_instruction.operands[1];
    if use_operand.operand != 0
        || use_operand.access != RegisterOperandAccess::Use
        || def_operand.operand != 1
        || def_operand.access != RegisterOperandAccess::Def
        || extension_instruction.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(RedundantExtensionError::UnsupportedInstruction);
    }
    let value = use_operand.virtual_register;
    let output = def_operand.virtual_register;
    if value == output {
        return Err(RedundantExtensionError::UnsupportedUse);
    }
    let find_register = |register| {
        function
            .virtual_registers
            .iter()
            .find(|entry| entry.id == register)
    };
    let value_register =
        find_register(value).ok_or(RedundantExtensionError::UnsupportedProducer)?;
    let output_register = find_register(output).ok_or(RedundantExtensionError::UnsupportedUse)?;
    // The result register's declared origin must name this instruction; the
    // copy keeps the instruction id, so that origin stays accurate.
    let defines_here = |instruction: SelectedInstructionId, operand: u16| {
        matches!(
            output_register.origin,
            VirtualRegisterOrigin::InstructionResult {
                instruction: origin,
                ..
            } if origin == instruction
        ) || matches!(
            output_register.origin,
            VirtualRegisterOrigin::InstructionScratch {
                instruction: origin,
                operand: origin_operand,
            } if origin == instruction && origin_operand == operand
        )
    };
    if !defines_here(extension_id, def_operand.operand)
        || output_register.entry_fixed_view.is_some()
    {
        return Err(RedundantExtensionError::UnsupportedUse);
    }
    // The extension's own constraint row must declare the same [use, def]
    // shape its operands carry.
    let row = environment
        .constraint(extension_instruction.constraint)
        .ok_or(RedundantExtensionError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[0].class != value_register.class
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Def
        || row.operands[1].class != output_register.class
    {
        return Err(RedundantExtensionError::ConstraintMismatch);
    }
    // Exactly one instruction may define the input; a `UseDef` rewrite counts
    // as a second definition and refuses, since the guarantee must hold at
    // every point the extension could read the register.
    let mut producers = function.blocks.iter().flat_map(|block| {
        block.instructions.iter().filter(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.access != RegisterOperandAccess::Use && operand.virtual_register == value
            })
        })
    });
    let producer = producers
        .next()
        .ok_or(RedundantExtensionError::UnsupportedProducer)?;
    if producers.next().is_some() {
        return Err(RedundantExtensionError::UnsupportedProducer);
    }
    // Inside the producer the defining operand position decides which written
    // register the guarantee describes: a packed load's assembled result and
    // its undocumented instruction scratch are different definitions riding
    // the same instruction, and only one of them carries the contract.
    let mut defined = producer.operands.iter().filter(|operand| {
        operand.access != RegisterOperandAccess::Use && operand.virtual_register == value
    });
    let Some(defined_operand) = defined.next() else {
        return Err(RedundantExtensionError::UnsupportedProducer);
    };
    if defined.next().is_some() {
        return Err(RedundantExtensionError::UnsupportedProducer);
    }
    let normalization = producer_normalization(producer.kind, defined_operand.operand)
        .ok_or(RedundantExtensionError::UnsupportedProducer)?;
    if !identity(form, normalization) {
        return Err(RedundantExtensionError::UnsupportedProducer);
    }
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .ok_or(RedundantExtensionError::ConstraintMismatch)?;
    if copy.operands.len() != 2
        || copy.operands[0].operand != 0
        || copy.operands[0].access != RegisterOperandAccess::Use
        || copy.operands[0].class != value_register.class
        || copy.operands[1].operand != 1
        || copy.operands[1].access != RegisterOperandAccess::Def
        || copy.operands[1].class != output_register.class
        || !copy.implicit_uses.is_empty()
        || !copy.implicit_defs.is_empty()
        || !copy.clobbers.is_empty()
        || copy.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(RedundantExtensionError::ConstraintMismatch);
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
            // A second full instruction scan locates and counts producers.
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())
            })
        })
        .ok_or(RedundantExtensionError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RedundantExtensionError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RedundantExtensionError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        extension_index,
        extension_id,
        provenance: extension_instruction.provenance.clone(),
        value,
        output,
        copy,
    })
}

/// The one-instruction proposal shape shared with replay: the target's own
/// copy row supplies the operand interface while the register identity, the
/// instruction identity, and the extension's provenance stay with the result.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: admitted.extension_id,
        kind: SelectedInstructionKind::CopyI64,
        constraint: admitted.copy.key,
        operands: admitted
            .copy
            .operands
            .iter()
            .zip([admitted.value, admitted.output])
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
        implicit_uses: admitted.copy.implicit_uses.clone(),
        implicit_defs: admitted.copy.implicit_defs.clone(),
        clobbers: admitted.copy.clobbers.clone(),
        provenance: admitted.provenance.clone(),
    }
}
