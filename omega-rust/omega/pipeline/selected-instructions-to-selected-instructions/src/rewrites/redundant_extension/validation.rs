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
fn measured_steps(
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
        },
        transformed: Arc::new(proposed),
    })
}

#[cfg(test)]
mod independence_tests {
    use std::sync::Arc;

    use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
    use optimization_unit::ValueDefinitionSite;
    use register_environment::baseline_target_register_environment;
    use register_model::RegisterInstructionConstraint;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
        SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ScalarType,
        ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        RedundantExtensionError, RedundantExtensionReceipt, ValidatedRedundantExtension,
        validate_redundant_extension_removal,
    };

    const PRODUCER: SelectedInstructionId = SelectedInstructionId(2);
    const EXTENSION: SelectedInstructionId = SelectedInstructionId(3);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);

    const INPUT: VirtualRegisterId = VirtualRegisterId(0);
    const VALUE: VirtualRegisterId = VirtualRegisterId(1);
    const OUTPUT: VirtualRegisterId = VirtualRegisterId(2);

    fn instruction(
        id: SelectedInstructionId,
        kind: SelectedInstructionKind,
        row: &RegisterInstructionConstraint,
        registers: &[VirtualRegisterId],
    ) -> SelectedInstruction {
        SelectedInstruction {
            id,
            kind,
            constraint: row.key,
            operands: row
                .operands
                .iter()
                .zip(registers)
                .map(|(operand, register)| SelectedOperand {
                    operand: operand.operand,
                    virtual_register: *register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                })
                .collect(),
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance: Default::default(),
        }
    }

    fn u64_scalar() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
    }

    /// `producer_kind` selects which instruction defines the extension's
    /// input. `CopyI64` carries no high-bit promise at all, so the removal it
    /// feeds is a legality error: the validator's own audit must refuse it
    /// with `UnsupportedProducer` even when the proposal is byte-for-byte the
    /// copy a defective producer's `rewritten` would emit.
    fn fixture(
        producer_kind: SelectedInstructionKind,
        producer_row_key: fn(
            &selected_instructions::SelectedConstraintKeys,
        ) -> register_model::RegisterConstraintKey,
    ) -> (
        register_environment::ValidatedTargetRegisterEnvironment,
        ValidatedRedundantExtension,
    ) {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let copy_row = environment.constraint(keys.copy_i64).unwrap();
        let producer_row = environment.constraint(producer_row_key(&keys)).unwrap();
        let return_row = environment.constraint(keys.return_unit).unwrap();
        let class = copy_row.operands[0].class;
        let machine = MachineId::new(1).unwrap();
        let function = SelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: vec![
                VirtualRegister {
                    id: INPUT,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::EntryParameter {
                        source_value: ValueId::new(1).unwrap(),
                        parameter_index: 0,
                    },
                    definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: VALUE,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: PRODUCER,
                        source_value: ValueId::new(2).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: OUTPUT,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: EXTENSION,
                        source_value: ValueId::new(3).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
            ],
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![
                    instruction(PRODUCER, producer_kind, producer_row, &[INPUT, VALUE]),
                    instruction(
                        EXTENSION,
                        SelectedInstructionKind::ZeroExtendU16,
                        copy_row,
                        &[VALUE, OUTPUT],
                    ),
                ],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        TERMINAL,
                        SelectedInstructionKind::ReturnUnit,
                        return_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(2).unwrap(),
                },
            }],
        };
        let plan = SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target,
            entry: machine,
            functions: vec![function].into(),
        };
        let identity = selected_instruction_plan_identity(&plan);
        let source = ValidatedRedundantExtension {
            receipt: RedundantExtensionReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: plan.fuel_schedule,
            },
            transformed: Arc::new(plan),
        };
        (environment, source)
    }

    /// The forged proposal a defective producer would publish: the extension
    /// replaced by the contract's copy shape even though the input's producer
    /// promises nothing about the bits the extension would fix. The
    /// validator's own audit must refuse the legality with
    /// `UnsupportedProducer`, not merely diff the proposal.
    #[test]
    fn validator_refuses_a_proposal_its_own_audit_rejects() {
        let (environment, source) = fixture(SelectedInstructionKind::CopyI64, |keys| keys.copy_i64);
        let copy_row = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0].instructions[1] = instruction(
            EXTENSION,
            SelectedInstructionKind::CopyI64,
            copy_row,
            &[VALUE, OUTPUT],
        );
        let budget = OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap();
        assert_eq!(
            validate_redundant_extension_removal(
                &source,
                0,
                EXTENSION,
                &environment,
                budget,
                proposed
            )
            .unwrap_err(),
            RedundantExtensionError::UnsupportedProducer
        );
    }

    /// A legal removal — a `Load8` producer zeroes everything above bit 8 —
    /// whose proposal keeps the extension in place must still fail the
    /// demanded-function comparison with `ReplayMismatch`.
    #[test]
    fn validator_refuses_a_proposal_that_keeps_the_extension() {
        let (environment, source) =
            fixture(SelectedInstructionKind::Load8 { byte_offset: 0 }, |keys| {
                keys.load8.unwrap()
            });
        let proposed = source.transformed().clone();
        let budget = OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap();
        assert_eq!(
            validate_redundant_extension_removal(
                &source,
                0,
                EXTENSION,
                &environment,
                budget,
                proposed
            )
            .unwrap_err(),
            RedundantExtensionError::ReplayMismatch
        );
    }
}
