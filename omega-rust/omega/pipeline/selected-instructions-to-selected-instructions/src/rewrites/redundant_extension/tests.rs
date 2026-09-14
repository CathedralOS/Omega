use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
    SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::*;
use crate::ValidatedSelectedAnalysis;

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
    let registers = vec![
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
        // than zero; it cannot witness a further extension.
        (
            ZeroExtendU32,
            copy_key,
            &[POINTER, SOURCE],
            ZeroExtendU32,
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
