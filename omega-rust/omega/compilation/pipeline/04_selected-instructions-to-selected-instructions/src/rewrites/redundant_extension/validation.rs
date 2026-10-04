//! Independent validation of redundant-extension removal.
//!
//! The validator never calls [`super::admission`]: it re-derives the removal's
//! legality from the source records — the named instruction's extension kind
//! and `[use, def]` shape against the bound constraint row, the result
//! register's origin naming this instruction, the input's unique producer and
//! defining operand, the normalization that producer's contract promises, the
//! identity table over extension form and promise, and the target's own copy
//! row — then rebuilds the function the contract demands and requires the
//! proposal to equal it. Restoring the extension instruction must reproduce
//! the complete source by content. A producer admission error therefore fails
//! validation even when the proposal is exactly what that producer emitted.

use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, VirtualRegisterOrigin,
};
use semantic_vocabulary::IntegerValue;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{RedundantExtensionError, RedundantExtensionReceipt, ValidatedRedundantExtension};
use crate::ValidatedSelectedAnalysis;

/// The validator's own reconstruction of the removal the contract permits:
/// the named extension's coordinates, the instruction it must become, and the
/// source instruction restore reinserts. It shares no state with the
/// producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    block_index: usize,
    extension_index: usize,
    /// The extension instruction the source carried at those coordinates.
    extension: SelectedInstruction,
    /// The `CopyI64` the contract demands in its place.
    copy: SelectedInstruction,
}

/// The carrier normalization the named instruction performs, re-decoded by
/// the validator from the instruction kind alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtensionForm {
    /// Result bits at or above `width` are zero.
    Zero { width: u8 },
    /// Result bits at or above `width - 1` replicate bit `width - 1`.
    Sign { width: u8 },
}

fn extension_form(kind: SelectedInstructionKind) -> Option<ExtensionForm> {
    Some(match kind {
        SelectedInstructionKind::ZeroExtendU8 => ExtensionForm::Zero { width: 8 },
        SelectedInstructionKind::ZeroExtendU16 => ExtensionForm::Zero { width: 16 },
        SelectedInstructionKind::ZeroExtendU32 => ExtensionForm::Zero { width: 32 },
        SelectedInstructionKind::SignExtendI8 => ExtensionForm::Sign { width: 8 },
        SelectedInstructionKind::SignExtendI16 => ExtensionForm::Sign { width: 16 },
        SelectedInstructionKind::SignExtendI32 => ExtensionForm::Sign { width: 32 },
        _ => return None,
    })
}

/// What the validator re-derives that one defining operand of the input's
/// producer guarantees about the bits an extension would write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProducerPromise {
    /// Every bit at or above `width` is zero.
    Zeroed { width: u8 },
    /// Every bit at or above `width - 1` replicates bit `width - 1`.
    Replicated { width: u8 },
    /// The low `width` bits carry the meaningful result and the contract
    /// leaves every bit at or above `width` not meaningful.
    LowOnly { width: u8 },
    /// The low sixty-four bits the instruction writes, known exactly.
    Exact(u64),
}

/// `defined_operand` is the producer operand index that wrote the extension's
/// input register. It decides which register the promise describes: a packed
/// load's assembled result is operand one while operand two is instruction
/// scratch whose contents the contract does not describe.
fn producer_promise(
    kind: SelectedInstructionKind,
    defined_operand: u16,
) -> Option<ProducerPromise> {
    Some(match kind {
        SelectedInstructionKind::Load8 { .. } | SelectedInstructionKind::Load8Indexed => {
            ProducerPromise::Zeroed { width: 8 }
        }
        SelectedInstructionKind::Load16 { .. } => ProducerPromise::Zeroed { width: 16 },
        SelectedInstructionKind::Load32 { .. } => ProducerPromise::Zeroed { width: 32 },
        SelectedInstructionKind::LoadPacked { width, .. } if defined_operand == 1 => {
            ProducerPromise::LowOnly {
                width: width.byte_size() * 8,
            }
        }
        SelectedInstructionKind::ZeroExtendU8 => ProducerPromise::Zeroed { width: 8 },
        SelectedInstructionKind::ZeroExtendU16 => ProducerPromise::Zeroed { width: 16 },
        SelectedInstructionKind::ZeroExtendU32 => ProducerPromise::LowOnly { width: 32 },
        SelectedInstructionKind::SignExtendI8 => ProducerPromise::Replicated { width: 8 },
        SelectedInstructionKind::SignExtendI16 => ProducerPromise::Replicated { width: 16 },
        SelectedInstructionKind::SignExtendI32 => ProducerPromise::Replicated { width: 32 },
        SelectedInstructionKind::Float32ToBits => ProducerPromise::LowOnly { width: 32 },
        SelectedInstructionKind::MaterializeBooleanEqual
        | SelectedInstructionKind::MaterializeBooleanU64LessThan
        | SelectedInstructionKind::MaterializeBooleanI64LessThan
        | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
        | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            ProducerPromise::Zeroed { width: 1 }
        }
        SelectedInstructionKind::MaterializeI64 { value } => {
            let pattern = match value {
                IntegerValue::Signed(value) => value as i64 as u64,
                IntegerValue::Unsigned(value) => value as u64,
            };
            ProducerPromise::Exact(pattern)
        }
        _ => return None,
    })
}

/// Whether the form is the identity on an input carrying `promise`. The
/// validator re-decides the bit-exact table: `ZeroExtendU32` is the one
/// extension whose own contract leaves its result's upper bits unmeaningful,
/// so a partial carrier witnesses it exactly when the producer's meaningful
/// content already fits in 32 bits; every other extension fixes result bits a
/// partial producer never promised.
fn promise_proves_identity(form: ExtensionForm, promise: ProducerPromise) -> bool {
    match (form, promise) {
        (ExtensionForm::Zero { width }, ProducerPromise::Zeroed { width: n }) => n <= width,
        (ExtensionForm::Zero { width: 32 }, ProducerPromise::LowOnly { width: n }) => n <= 32,
        (ExtensionForm::Zero { .. }, ProducerPromise::LowOnly { .. }) => false,
        (ExtensionForm::Zero { width }, ProducerPromise::Exact(pattern)) => {
            pattern < (1u64 << width)
        }
        (ExtensionForm::Zero { .. }, ProducerPromise::Replicated { .. }) => false,
        (ExtensionForm::Sign { width }, ProducerPromise::Replicated { width: n }) => n <= width,
        (ExtensionForm::Sign { width }, ProducerPromise::Zeroed { width: n }) => n < width,
        (ExtensionForm::Sign { width }, ProducerPromise::Exact(pattern)) => {
            let shift = 64 - u32::from(width);
            (((pattern << shift) as i64) >> shift) as u64 == pattern
        }
        (ExtensionForm::Sign { .. }, ProducerPromise::LowOnly { .. }) => false,
    }
}

/// Re-derive the removal's legality from the source records, without the
/// producer's `admission` routine. A legality error surfaces here even when
/// the proposal matches the edit the producer emitted.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    extension: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, RedundantExtensionError> {
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
                .position(|instruction| instruction.id == extension)
                .map(|extension_index| (block_index, extension_index))
        })
        .ok_or(RedundantExtensionError::SourceMismatch)?;
    let extension_instruction = function.blocks[block_index].instructions[extension_index].clone();
    let form = extension_form(extension_instruction.kind)
        .ok_or(RedundantExtensionError::UnsupportedInstruction)?;
    // The extension must be the emitted `[use, def]` carrier pair: operand
    // zero reads the value, operand one defines the result, and nothing else
    // rides along that a plain copy would silently drop.
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
    let defines_here = matches!(
        output_register.origin,
        VirtualRegisterOrigin::InstructionResult {
            instruction: origin,
            ..
        } if origin == extension
    ) || matches!(
        output_register.origin,
        VirtualRegisterOrigin::InstructionScratch {
            instruction: origin,
            operand: origin_operand,
        } if origin == extension && origin_operand == def_operand.operand
    );
    if !defines_here || output_register.entry_fixed_view.is_some() {
        return Err(RedundantExtensionError::UnsupportedUse);
    }
    // The extension's own constraint row must declare the same `[use, def]`
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
    // as a second definition and refuses, since the promise must hold at
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
    // register the promise describes: a packed load's assembled result and its
    // undocumented instruction scratch are different definitions riding the
    // same instruction, and only one of them carries the contract.
    let mut defined = producer.operands.iter().filter(|operand| {
        operand.access != RegisterOperandAccess::Use && operand.virtual_register == value
    });
    let Some(defined_operand) = defined.next() else {
        return Err(RedundantExtensionError::UnsupportedProducer);
    };
    if defined.next().is_some() {
        return Err(RedundantExtensionError::UnsupportedProducer);
    }
    let promise = producer_promise(producer.kind, defined_operand.operand)
        .ok_or(RedundantExtensionError::UnsupportedProducer)?;
    if !promise_proves_identity(form, promise) {
        return Err(RedundantExtensionError::UnsupportedProducer);
    }
    // The copy the contract demands takes the target's own copy row: its
    // operand interface, implicit unit traffic, and clobbers must be the
    // clean `[use, def]` pair this rewrite substitutes.
    let copy_row = environment
        .constraint(environment.selected_keys().copy_i64)
        .ok_or(RedundantExtensionError::ConstraintMismatch)?;
    if copy_row.operands.len() != 2
        || copy_row.operands[0].operand != 0
        || copy_row.operands[0].access != RegisterOperandAccess::Use
        || copy_row.operands[0].class != value_register.class
        || copy_row.operands[1].operand != 1
        || copy_row.operands[1].access != RegisterOperandAccess::Def
        || copy_row.operands[1].class != output_register.class
        || !copy_row.implicit_uses.is_empty()
        || !copy_row.implicit_defs.is_empty()
        || !copy_row.clobbers.is_empty()
        || copy_row.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(RedundantExtensionError::ConstraintMismatch);
    }
    let copy = SelectedInstruction {
        id: extension,
        kind: SelectedInstructionKind::CopyI64,
        constraint: copy_row.key,
        operands: copy_row
            .operands
            .iter()
            .zip([value, output])
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
        implicit_uses: copy_row.implicit_uses.clone(),
        implicit_defs: copy_row.implicit_defs.clone(),
        clobbers: copy_row.clobbers.clone(),
        provenance: extension_instruction.provenance.clone(),
    };
    Ok(Reconstructed {
        function,
        function_index,
        block_index,
        extension_index,
        extension: extension_instruction,
        copy,
    })
}

/// The validation work this audit performs, in the measured-step contract the
/// family publishes: one step per block plus one per instruction across the
/// plan, then a second scan of the reconstructed function's instructions that
/// locates and counts the input's producers.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
) -> Result<u64, RedundantExtensionError> {
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())
            })
        })
        .ok_or(RedundantExtensionError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| RedundantExtensionError::IdentityOverflow)
}

/// Build the function the contract demands from the validator's own record:
/// the named extension is the target's `CopyI64` row over the same register
/// pair, keeping its instruction identity and provenance. The producer's
/// `rewritten` is not consulted; both sides derive the same function from the
/// source alone.
fn expect(reconstructed: &Reconstructed<'_>) -> SelectedFunction {
    let mut expected = reconstructed.function.clone();
    expected.blocks[reconstructed.block_index].instructions[reconstructed.extension_index] =
        reconstructed.copy.clone();
    expected
}

/// Reinsert the extension instruction and require the complete source by
/// content: every other function, block, instruction, register, call,
/// settlement, and access is retained bit-identical.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, RedundantExtensionError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(RedundantExtensionError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(RedundantExtensionError::ReplayMismatch)?;
    let slot = block
        .instructions
        .get_mut(reconstructed.extension_index)
        .ok_or(RedundantExtensionError::ReplayMismatch)?;
    *slot = reconstructed.extension.clone();
    Ok(restored)
}

/// Independently consume the proposed program: reconstruct the removal's
/// legality from the source records, require the proposed function to equal
/// the one the contract demands, and require the restored program to equal
/// the complete source. The producer's `admission` routine is never
/// consulted, so a wrong legality decision fails here even when the proposal
/// matches the edit the producer emitted.
pub fn validate_redundant_extension_removal(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    extension: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRedundantExtension, RedundantExtensionError> {
    let reconstructed = reconstruct(source, function_index, extension, environment)?;
    if measured_steps(source.selected_plan(), reconstructed.function)? > budget.validation_steps() {
        return Err(RedundantExtensionError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)) {
        return Err(RedundantExtensionError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(RedundantExtensionError::ReplayMismatch);
    }
    Ok(ValidatedRedundantExtension {
        receipt: RedundantExtensionReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
            function_index,
            extension,
        },
        transformed: Arc::new(proposed),
    })
}
