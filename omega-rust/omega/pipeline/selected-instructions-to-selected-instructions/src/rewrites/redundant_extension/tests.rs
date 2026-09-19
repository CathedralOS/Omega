use crate::RedundantExtensionError;
use crate::RedundantExtensionReceipt;
use crate::ValidatedRedundantExtension;
use crate::ValidatedSelectedAnalysis;
use crate::remove_selected_redundant_extension;
use crate::validate_redundant_extension_removal;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    PackedByteWidth, SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedOperand, SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap()
}

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

const PRODUCER: SelectedInstructionId = SelectedInstructionId(2);
const EXTENSION: SelectedInstructionId = SelectedInstructionId(3);
const SINK_COPY: SelectedInstructionId = SelectedInstructionId(4);
const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const SOURCE: VirtualRegisterId = VirtualRegisterId(1);
const OUTPUT: VirtualRegisterId = VirtualRegisterId(2);
const SINK: VirtualRegisterId = VirtualRegisterId(3);
const SCRATCH: VirtualRegisterId = VirtualRegisterId(4);

fn register(
    id: VirtualRegisterId,
    scalar_type: ScalarType,
    class: register_model::RegisterClassId,
    origin: VirtualRegisterOrigin,
) -> VirtualRegister {
    VirtualRegister {
        id,
        scalar_type,
        class,
        origin,
        definition_site: None,
        entry_fixed_view: None,
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r1 = <producer> ...; r2 = <extension> r1; r3 = copy r2; return`.
fn fixture(
    target: NativeTarget,
    producer_kind: SelectedInstructionKind,
    producer_key: register_model::RegisterConstraintKey,
    producer_registers: &[VirtualRegisterId],
    extension_kind: SelectedInstructionKind,
) -> ValidatedRedundantExtension {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let producer_row = environment.constraint(producer_key).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = copy.operands[0].class;
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let mut registers = vec![
        VirtualRegister {
            id: POINTER,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        register(
            SOURCE,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: PRODUCER,
                source_value: ValueId::new(2).unwrap(),
            },
        ),
        register(
            OUTPUT,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: EXTENSION,
                source_value: ValueId::new(3).unwrap(),
            },
        ),
        register(
            SINK,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SINK_COPY,
                source_value: ValueId::new(4).unwrap(),
            },
        ),
    ];
    // A packed producer writes a second, instruction-local scratch register;
    // keep its roster row when the fixture names it so the plan stays whole.
    if producer_registers.contains(&SCRATCH) {
        registers.push(register(
            SCRATCH,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionScratch {
                instruction: PRODUCER,
                operand: 2,
            },
        ));
    }
    let mut extension_instruction = instruction(EXTENSION, extension_kind, copy, &[SOURCE, OUTPUT]);
    extension_instruction.provenance.operations = vec![OperationId::new(7).unwrap()];
    extension_instruction.provenance.values = vec![ValueId::new(3).unwrap()];
    let instructions = vec![
        instruction(PRODUCER, producer_kind, producer_row, producer_registers),
        extension_instruction,
        instruction(
            SINK_COPY,
            SelectedInstructionKind::CopyI64,
            copy,
            &[OUTPUT, SINK],
        ),
    ];
    let machine = MachineId::new(1).unwrap();
    let plan = SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target,
        entry: machine,
        functions: vec![SelectedFunction {
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
            virtual_registers: registers,
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions,
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        SelectedInstructionId(5),
                        SelectedInstructionKind::ReturnUnit,
                        terminal_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        }]
        .into(),
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedRedundantExtension {
        receipt: RedundantExtensionReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

fn keys(
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> selected_instructions::SelectedConstraintKeys {
    environment.selected_keys()
}

/// A byte load result feeding any wider zero extension is already normalized.
#[test]
fn zero_extended_load_result_collapses_to_copy() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for extension_kind in [
            SelectedInstructionKind::ZeroExtendU8,
            SelectedInstructionKind::ZeroExtendU16,
            SelectedInstructionKind::ZeroExtendU32,
        ] {
            let source = fixture(
                target,
                SelectedInstructionKind::Load8 { byte_offset: 0 },
                keys(&environment).load8.unwrap(),
                &[POINTER, SOURCE],
                extension_kind,
            );
            let result =
                remove_selected_redundant_extension(&source, 0, EXTENSION, &environment, budget())
                    .unwrap();
            let rewritten = &result.transformed().functions[0].blocks[0].instructions[1];
            assert_eq!(rewritten.id, EXTENSION);
            assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
            assert_eq!(rewritten.constraint, keys(&environment).copy_i64);
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].virtual_register, SOURCE);
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
            assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
            // The extension's provenance moves to the copy.
            assert_eq!(
                rewritten.provenance,
                source.transformed().functions[0].blocks[0].instructions[1].provenance
            );
            assert_eq!(
                result.receipt().source_selected(),
                source.selected_identity()
            );
            assert_eq!(
                result.receipt().transformed_selected(),
                selected_instruction_plan_identity(result.transformed())
            );
            validate_redundant_extension_removal(
                &source,
                0,
                EXTENSION,
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap();
            // A detached, separately allocated proposal replays by content.
            let mut detached = result.transformed().clone();
            detached.functions = detached.functions.iter().cloned().collect();
            validate_redundant_extension_removal(
                &source,
                0,
                EXTENSION,
                &environment,
                budget(),
                detached,
            )
            .unwrap();
        }
    }
}

/// A partial-carrier producer feeding `ZeroExtendU32` collapses to the copy:
/// the extension re-normalizes the producer's low 32 bits and promises
/// nothing above them, so it is the identity on the producer's surface.
#[test]
fn partial_carrier_result_collapses_to_copy() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = keys(&environment);
        for (producer_kind, producer_key, producer_registers) in [
            (
                SelectedInstructionKind::ZeroExtendU32,
                keys.copy_i64,
                &[POINTER, SOURCE][..],
            ),
            (
                SelectedInstructionKind::LoadPacked {
                    byte_offset: 0,
                    width: PackedByteWidth::Three,
                },
                keys.load_packed.unwrap(),
                &[POINTER, SOURCE, SCRATCH][..],
            ),
            (
                SelectedInstructionKind::Float32ToBits,
                keys.float32_to_bits.unwrap(),
                &[POINTER, SOURCE][..],
            ),
        ] {
            let source = fixture(
                target,
                producer_kind,
                producer_key,
                producer_registers,
                SelectedInstructionKind::ZeroExtendU32,
            );
            let result =
                remove_selected_redundant_extension(&source, 0, EXTENSION, &environment, budget())
                    .unwrap_or_else(|error| panic!("{producer_kind:?} on {target:?}: {error}"));
            let rewritten = &result.transformed().functions[0].blocks[0].instructions[1];
            assert_eq!(rewritten.id, EXTENSION);
            assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
            assert_eq!(rewritten.constraint, keys.copy_i64);
            assert_eq!(rewritten.operands[0].virtual_register, SOURCE);
            assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
            assert_eq!(
                result.receipt().source_selected(),
                source.selected_identity()
            );
            validate_redundant_extension_removal(
                &source,
                0,
                EXTENSION,
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap();
        }
    }
}

/// The complete admission table over producer and consumer widths: an
/// extension is the identity exactly when the producer's guarantee covers it.
#[test]
fn producer_normalization_table() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = keys(&environment);
    let copy_key = keys.copy_i64;
    use SelectedInstructionKind::*;
    let cases: &[(
        SelectedInstructionKind,
        register_model::RegisterConstraintKey,
        &[VirtualRegisterId],
        SelectedInstructionKind,
        bool,
    )] = &[
        (
            Load8 { byte_offset: 0 },
            keys.load8.unwrap(),
            &[POINTER, SOURCE],
            SignExtendI8,
            false,
        ),
        (
            Load8 { byte_offset: 0 },
            keys.load8.unwrap(),
            &[POINTER, SOURCE],
            SignExtendI16,
            true,
        ),
        (
            Load8 { byte_offset: 0 },
            keys.load8.unwrap(),
            &[POINTER, SOURCE],
            SignExtendI32,
            true,
        ),
        (
            Load16 { byte_offset: 0 },
            keys.load16.unwrap(),
            &[POINTER, SOURCE],
            ZeroExtendU8,
            false,
        ),
        (
            Load16 { byte_offset: 0 },
            keys.load16.unwrap(),
            &[POINTER, SOURCE],
            ZeroExtendU16,
            true,
        ),
        (
            Load16 { byte_offset: 0 },
            keys.load16.unwrap(),
            &[POINTER, SOURCE],
            ZeroExtendU32,
            true,
        ),
        (
            Load16 { byte_offset: 0 },
            keys.load16.unwrap(),
            &[POINTER, SOURCE],
            SignExtendI16,
            false,
        ),
        (
            Load16 { byte_offset: 0 },
            keys.load16.unwrap(),
            &[POINTER, SOURCE],
            SignExtendI32,
            true,
        ),
        (
            Load32 { byte_offset: 0 },
            keys.load32.unwrap(),
            &[POINTER, SOURCE],
            ZeroExtendU32,
            true,
        ),
        (
            Load32 { byte_offset: 0 },
            keys.load32.unwrap(),
            &[POINTER, SOURCE],
            SignExtendI32,
            false,
        ),
        (
            ZeroExtendU8,
            copy_key,
            &[POINTER, SOURCE],
            ZeroExtendU16,
            true,
        ),
        (
            ZeroExtendU8,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI8,
            false,
        ),
        (
            ZeroExtendU8,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI16,
            true,
        ),
        (
            ZeroExtendU16,
            copy_key,
            &[POINTER, SOURCE],
            ZeroExtendU8,
            false,
        ),
        (
            ZeroExtendU16,
            copy_key,
            &[POINTER, SOURCE],
            ZeroExtendU32,
            true,
        ),
        (
            ZeroExtendU16,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI16,
            false,
        ),
        (
            ZeroExtendU16,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI32,
            true,
        ),
        // ZeroExtendU32's contract leaves its upper bits unmeaningful rather
        // than zero, so its output is a partial carrier: the one extension
        // whose own result stays partial — ZeroExtendU32 itself — is the
        // identity over it, while the fixed-result extensions still cannot
        // be witnessed.
        (
            ZeroExtendU32,
            copy_key,
            &[POINTER, SOURCE],
            ZeroExtendU32,
            true,
        ),
        (
            ZeroExtendU32,
            copy_key,
            &[POINTER, SOURCE],
            ZeroExtendU16,
            false,
        ),
        (
            ZeroExtendU32,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI32,
            false,
        ),
        (
            SignExtendI8,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI8,
            true,
        ),
        (
            SignExtendI8,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI32,
            true,
        ),
        (
            SignExtendI8,
            copy_key,
            &[POINTER, SOURCE],
            ZeroExtendU32,
            false,
        ),
        (
            SignExtendI16,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI8,
            false,
        ),
        (
            SignExtendI16,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI32,
            true,
        ),
        (
            SignExtendI32,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI16,
            false,
        ),
        (
            SignExtendI32,
            copy_key,
            &[POINTER, SOURCE],
            SignExtendI32,
            true,
        ),
        (
            MaterializeBooleanEqual,
            keys.materialize_boolean,
            &[SOURCE],
            ZeroExtendU8,
            true,
        ),
        (
            MaterializeBooleanI64LessOrEqual,
            keys.materialize_boolean,
            &[SOURCE],
            SignExtendI8,
            true,
        ),
        // The packed-load result is a partial carrier of exactly its loaded
        // bytes: the three-byte width fits inside ZeroExtendU32's
        // normalization, the wider widths genuinely narrow, and no packed
        // width can witness an extension that fixes result bits.
        (
            LoadPacked {
                byte_offset: 0,
                width: PackedByteWidth::Three,
            },
            keys.load_packed.unwrap(),
            &[POINTER, SOURCE, SCRATCH],
            ZeroExtendU32,
            true,
        ),
        (
            LoadPacked {
                byte_offset: 0,
                width: PackedByteWidth::Five,
            },
            keys.load_packed.unwrap(),
            &[POINTER, SOURCE, SCRATCH],
            ZeroExtendU32,
            false,
        ),
        (
            LoadPacked {
                byte_offset: 0,
                width: PackedByteWidth::Seven,
            },
            keys.load_packed.unwrap(),
            &[POINTER, SOURCE, SCRATCH],
            ZeroExtendU32,
            false,
        ),
        (
            LoadPacked {
                byte_offset: 0,
                width: PackedByteWidth::Three,
            },
            keys.load_packed.unwrap(),
            &[POINTER, SOURCE, SCRATCH],
            ZeroExtendU8,
            false,
        ),
        (
            LoadPacked {
                byte_offset: 0,
                width: PackedByteWidth::Three,
            },
            keys.load_packed.unwrap(),
            &[POINTER, SOURCE, SCRATCH],
            SignExtendI32,
            false,
        ),
        // The same instruction's scratch definition shares none of the
        // result's contract; operand position, not the instruction kind,
        // decides which guarantee the input carries.
        (
            LoadPacked {
                byte_offset: 0,
                width: PackedByteWidth::Three,
            },
            keys.load_packed.unwrap(),
            &[POINTER, SCRATCH, SOURCE],
            ZeroExtendU32,
            false,
        ),
        // The float bit transfers publish only their payload width:
        // Float32ToBits is a 32-bit partial carrier, Float64ToBits a full
        // result the extension would have to narrow.
        (
            Float32ToBits,
            keys.float32_to_bits.unwrap(),
            &[POINTER, SOURCE],
            ZeroExtendU32,
            true,
        ),
        (
            Float32ToBits,
            keys.float32_to_bits.unwrap(),
            &[POINTER, SOURCE],
            ZeroExtendU16,
            false,
        ),
        (
            Float32ToBits,
            keys.float32_to_bits.unwrap(),
            &[POINTER, SOURCE],
            SignExtendI32,
            false,
        ),
        (
            Float64ToBits,
            keys.float64_to_bits.unwrap(),
            &[POINTER, SOURCE],
            ZeroExtendU32,
            false,
        ),
        // An unbounded producer can never witness the removal.
        (
            Load64 { byte_offset: 0 },
            keys.load64.unwrap(),
            &[POINTER, SOURCE],
            ZeroExtendU32,
            false,
        ),
        (CopyI64, copy_key, &[POINTER, SOURCE], ZeroExtendU8, false),
        (
            MaterializeI64 {
                value: IntegerValue::Unsigned(255),
            },
            keys.materialize_i64,
            &[SOURCE],
            ZeroExtendU8,
            true,
        ),
        (
            MaterializeI64 {
                value: IntegerValue::Unsigned(256),
            },
            keys.materialize_i64,
            &[SOURCE],
            ZeroExtendU8,
            false,
        ),
        (
            MaterializeI64 {
                value: IntegerValue::Signed(-1),
            },
            keys.materialize_i64,
            &[SOURCE],
            SignExtendI8,
            true,
        ),
        (
            MaterializeI64 {
                value: IntegerValue::Unsigned(128),
            },
            keys.materialize_i64,
            &[SOURCE],
            SignExtendI8,
            false,
        ),
        (
            MaterializeI64 {
                value: IntegerValue::Signed(-129),
            },
            keys.materialize_i64,
            &[SOURCE],
            SignExtendI8,
            false,
        ),
        (
            MaterializeI64 {
                value: IntegerValue::Signed(-129),
            },
            keys.materialize_i64,
            &[SOURCE],
            SignExtendI16,
            true,
        ),
    ];
    for (producer_kind, producer_key, producer_registers, extension_kind, admitted) in cases {
        let source = fixture(
            target,
            *producer_kind,
            *producer_key,
            producer_registers,
            *extension_kind,
        );
        let result =
            remove_selected_redundant_extension(&source, 0, EXTENSION, &environment, budget());
        assert_eq!(
            result.is_ok(),
            *admitted,
            "{producer_kind:?} feeding {extension_kind:?}"
        );
        if !admitted {
            assert_eq!(
                result.unwrap_err(),
                RedundantExtensionError::UnsupportedProducer
            );
        }
    }
}

/// Edit the single fixture function, then refresh the receipt identities so the
/// mutated plan is a well-formed analysis source.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedRedundantExtension {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(
        target,
        SelectedInstructionKind::Load8 { byte_offset: 0 },
        environment.selected_keys().load8.unwrap(),
        &[POINTER, SOURCE],
        SelectedInstructionKind::ZeroExtendU16,
    );
    edit(
        &mut std::sync::Arc::make_mut(&mut source.transformed).functions[0],
        &environment,
    );
    let identity = selected_instruction_plan_identity(&source.transformed);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

fn remove(
    source: &ValidatedRedundantExtension,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedRedundantExtension, RedundantExtensionError> {
    remove_selected_redundant_extension(source, 0, EXTENSION, environment, budget())
}

#[test]
fn malformed_extension_shapes_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A pinned result cannot become a clean copy through the target's copy row.
    let pinned_def = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[1].fixed_view =
            Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        remove(&pinned_def, &environment).unwrap_err(),
        RedundantExtensionError::UnsupportedInstruction
    );
    // A tied result stays an allocation-shaped instruction, not a free copy.
    let tied_def = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[1].tied_to = Some(0);
    });
    assert_eq!(
        remove(&tied_def, &environment).unwrap_err(),
        RedundantExtensionError::UnsupportedInstruction
    );
    // An implicit unit write would be silently dropped by the copy.
    let implicit = mutated(target, |function, environment| {
        let unit = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .implicit_defs
            .first()
            .copied();
        let Some(unit) = unit else {
            function.blocks[0].instructions[1].implicit_defs =
                vec![register_model::RegisterUnitId(0)];
            return;
        };
        function.blocks[0].instructions[1].implicit_defs = vec![unit];
    });
    assert_eq!(
        remove(&implicit, &environment).unwrap_err(),
        RedundantExtensionError::UnsupportedInstruction
    );
    // The extension kind check rejects non-extension instructions.
    let not_extension = mutated(target, |function, _| {
        function.blocks[0].instructions[1].kind = SelectedInstructionKind::CopyI64;
    });
    assert_eq!(
        remove(&not_extension, &environment).unwrap_err(),
        RedundantExtensionError::UnsupportedInstruction
    );
    // A self-referential operand pair is not a forwarding shape.
    let self_referential = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].virtual_register = OUTPUT;
    });
    assert_eq!(
        remove(&self_referential, &environment).unwrap_err(),
        RedundantExtensionError::UnsupportedUse
    );
    // A result whose declared origin names another instruction is malformed.
    let foreign_origin = mutated(target, |function, _| {
        function.virtual_registers[2].origin = VirtualRegisterOrigin::InstructionResult {
            instruction: SINK_COPY,
            source_value: ValueId::new(3).unwrap(),
        };
    });
    assert_eq!(
        remove(&foreign_origin, &environment).unwrap_err(),
        RedundantExtensionError::UnsupportedUse
    );
    // A second definition of the input — even through a UseDef rewrite —
    // breaks the wherever-read guarantee.
    let second_definition = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        let mut extra = instruction(
            SelectedInstructionId(6),
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, SOURCE],
        );
        extra.operands[1].access = RegisterOperandAccess::UseDef;
        function.blocks[0].instructions.insert(1, extra);
    });
    assert_eq!(
        remove(&second_definition, &environment).unwrap_err(),
        RedundantExtensionError::UnsupportedProducer
    );
}

#[test]
fn replay_rejects_anything_but_the_exact_copy() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        SelectedInstructionKind::Load8 { byte_offset: 0 },
        environment.selected_keys().load8.unwrap(),
        &[POINTER, SOURCE],
        SelectedInstructionKind::ZeroExtendU16,
    );
    let result = remove(&source, &environment).unwrap();
    for mutation in 0..8 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // Copied from the wrong register.
            0 => {
                function.blocks[0].instructions[1].operands[0].virtual_register = POINTER;
            }
            // Different result register.
            1 => {
                function.blocks[0].instructions[1].operands[1].virtual_register = SINK;
            }
            // A different instruction id on the copy.
            2 => function.blocks[0].instructions[1].id = PRODUCER,
            // The extension's provenance must survive intact.
            3 => {
                function.blocks[0].instructions[1]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // The surviving producer must remain untouched.
            4 => {
                function.blocks[0].instructions[0].kind =
                    SelectedInstructionKind::Load16 { byte_offset: 0 };
            }
            // The extension kind must not survive in the rewritten slot.
            5 => {
                function.blocks[0].instructions[1].kind = SelectedInstructionKind::ZeroExtendU16;
            }
            // An unrelated register must stay identical.
            6 => function.virtual_registers[3].scalar_type = ScalarType::Boolean,
            // A fabricated roster row has no source counterpart.
            7 => {
                function
                    .memory_accesses
                    .push(selected_instructions::SelectedMemoryAccess {
                        instruction: EXTENSION,
                        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                            OperationId::new(4).unwrap(),
                        ),
                        place: PlaceId::new(1).unwrap(),
                        byte_offset: 0,
                        byte_count: 8,
                        role: selected_instructions::SelectedMemoryAccessRole::ReadPlace,
                    });
            }
            _ => unreachable!(),
        }
        assert!(
            validate_redundant_extension_removal(
                &source,
                0,
                EXTENSION,
                &environment,
                budget(),
                proposed
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then one per instruction
/// of the admitted function for the producer scan — seven steps for the
/// three-instruction fixture, nine once a fourth instruction joins the
/// block — so the exact count admits the removal on both the proposal and
/// the independent replay path while one step below rejects both.
#[test]
fn validation_budget_covers_the_producer_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        SelectedInstructionKind::Load8 { byte_offset: 0 },
        keys(&environment).load8.unwrap(),
        &[POINTER, SOURCE],
        SelectedInstructionKind::ZeroExtendU16,
    );
    // A fourth body instruction extends both scans: the enumeration charges
    // (1 block) + (4 instructions) and the producer scan charges 4 more.
    let wider = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.virtual_registers.push(register(
            SCRATCH,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            function.virtual_registers[0].class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(6),
                source_value: ValueId::new(5).unwrap(),
            },
        ));
        function.blocks[0].instructions.push(instruction(
            SelectedInstructionId(6),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[SINK, SCRATCH],
        ));
    });
    for (source, exact_steps) in [(source, 7u64), (wider, 9u64)] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            remove_selected_redundant_extension(&source, 0, EXTENSION, &environment, exact)
                .unwrap();
        validate_redundant_extension_removal(
            &source,
            0,
            EXTENSION,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            remove_selected_redundant_extension(&source, 0, EXTENSION, &environment, starved)
                .unwrap_err(),
            RedundantExtensionError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_redundant_extension_removal(
                &source,
                0,
                EXTENSION,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            RedundantExtensionError::WorkBudgetExceeded
        );
    }
}

/// Two runs over the identical source produce the identical validated result,
/// and the published plan is a legal second input: the sealed transformed
/// program already sits at the rule's fixed point — the extension keeps its
/// instruction identity as the emitted copy, so a second removal at the same
/// site finds no extension shape to admit.
#[test]
fn removal_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = remove(
        &fixture(
            target,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            keys(&environment).load8.unwrap(),
            &[POINTER, SOURCE],
            SelectedInstructionKind::ZeroExtendU16,
        ),
        &environment,
    )
    .unwrap();
    let second = remove(
        &fixture(
            target,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            keys(&environment).load8.unwrap(),
            &[POINTER, SOURCE],
            SelectedInstructionKind::ZeroExtendU16,
        ),
        &environment,
    )
    .unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. Re-running on
    // it is terminal: instruction EXTENSION is the emitted CopyI64 now, not
    // a carrier extension.
    assert_eq!(
        remove_selected_redundant_extension(&first, 0, EXTENSION, &environment, budget())
            .unwrap_err(),
        RedundantExtensionError::UnsupportedInstruction
    );
}

/// Replay corruption in a block the removal never touched still rejects:
/// the restore-by-content check compares the complete plan, not just the
/// block carrying the collapsed extension.
#[test]
fn replay_rejects_drift_outside_the_rewritten_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Stretch the fixture across one edge: the extension's block collapses
    // and jumps to a second block that returns.
    let source = mutated(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let tail = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: selected_instructions::SelectedSuccessor {
                    role: selected_instructions::SelectedSuccessorRole::Semantic,
                    structural_case: None,
                    structural_bindings: Vec::new(),
                    psi_edge: EdgeId::new(2).unwrap(),
                    block: SelectedBlockId(1),
                    source_target: BlockId::new(2).unwrap(),
                    bindings: Vec::new(),
                    fuel: Vec::new(),
                },
            },
        );
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: tail,
        });
    });
    let result = remove(&source, &environment).unwrap();
    assert_eq!(result.transformed().functions[0].blocks.len(), 2);
    // An extra instruction in the untouched landing block rejects.
    let mut proposed = result.transformed().clone();
    let copy = environment
        .constraint(keys(&environment).copy_i64)
        .unwrap()
        .clone();
    proposed.functions[0].blocks[1]
        .instructions
        .push(instruction(
            SelectedInstructionId(11),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[POINTER, SINK],
        ));
    assert_eq!(
        validate_redundant_extension_removal(
            &source,
            0,
            EXTENSION,
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        RedundantExtensionError::ReplayMismatch
    );
    // Drift in the untouched block's terminator rejects.
    let mut proposed = result.transformed().clone();
    let SelectedTerminator::Return {
        instruction: return_instruction,
        ..
    } = &mut proposed.functions[0].blocks[1].terminator
    else {
        unreachable!()
    };
    return_instruction.id = SelectedInstructionId(12);
    assert_eq!(
        validate_redundant_extension_removal(
            &source,
            0,
            EXTENSION,
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        RedundantExtensionError::ReplayMismatch
    );
    // A phantom trailing block rejects.
    let mut proposed = result.transformed().clone();
    let return_row = environment
        .constraint(keys(&environment).return_unit)
        .unwrap()
        .clone();
    proposed.functions[0].blocks.push(SelectedBlock {
        id: SelectedBlockId(2),
        origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
        instructions: Vec::new(),
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(13),
                SelectedInstructionKind::ReturnUnit,
                &return_row,
                &[],
            ),
            psi_return_edge: EdgeId::new(3).unwrap(),
        },
    });
    assert_eq!(
        validate_redundant_extension_removal(
            &source,
            0,
            EXTENSION,
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        RedundantExtensionError::ReplayMismatch
    );
}
