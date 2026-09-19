// The disabled-policy axis is not applicable to this rule: the fold is not
// an `Optimization` selection-vocabulary member — admission is an explicit
// per-instruction validated call, matching the memory-rewrite families. The
// legal-second-input fixed-point leg lives in
// `fold_is_deterministic_and_terminal`.
use crate::AddressFoldError;
use crate::AddressFoldReceipt;
use crate::ValidatedAddressFold;
use crate::ValidatedSelectedAnalysis;
use crate::fold_selected_address;
use crate::validate_address_fold;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
    SelectedSuccessor, SelectedTerminator, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    PlaceId, ScalarType, ValueId,
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

const ADDRESS: SelectedInstructionId = SelectedInstructionId(2);
const CONSUMER: SelectedInstructionId = SelectedInstructionId(3);
const ROOT: VirtualRegisterId = VirtualRegisterId(0);
const POINTER: VirtualRegisterId = VirtualRegisterId(1);
const OUTPUT: VirtualRegisterId = VirtualRegisterId(2);
const VALUE: VirtualRegisterId = VirtualRegisterId(3);
const SPARE: VirtualRegisterId = VirtualRegisterId(4);

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
/// `r1 = address-offset r0, k; <consumer>; return`. The consumer reads `r1`
/// as its base pointer with displacement `m`.
fn fixture(
    target: NativeTarget,
    producer_offset: u32,
    consumer_kind: SelectedInstructionKind,
    consumer_key: register_model::RegisterConstraintKey,
    consumer_registers: &[VirtualRegisterId],
) -> ValidatedAddressFold {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let address = environment
        .constraint(keys.address_offset.unwrap())
        .unwrap();
    let consumer_row = environment.constraint(consumer_key).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = address.operands[0].class;
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let registers = vec![
        VirtualRegister {
            id: ROOT,
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
            POINTER,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: ADDRESS,
                source_value: ValueId::new(2).unwrap(),
            },
        ),
        register(
            OUTPUT,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: CONSUMER,
                source_value: ValueId::new(3).unwrap(),
            },
        ),
        VirtualRegister {
            id: VALUE,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(4).unwrap(),
                parameter_index: 1,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(1)),
            entry_fixed_view: None,
        },
    ];
    let mut consumer_instruction =
        instruction(CONSUMER, consumer_kind, consumer_row, consumer_registers);
    consumer_instruction.provenance.operations = vec![OperationId::new(7).unwrap()];
    consumer_instruction.provenance.values = vec![ValueId::new(3).unwrap()];
    let instructions = vec![
        instruction(
            ADDRESS,
            SelectedInstructionKind::AddressOffset {
                byte_offset: producer_offset,
            },
            address,
            &[ROOT, POINTER],
        ),
        consumer_instruction,
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
                        SelectedInstructionId(6),
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
    ValidatedAddressFold {
        receipt: AddressFoldReceipt {
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

fn load_fixture(
    target: NativeTarget,
    producer_offset: u32,
    consumer_kind: SelectedInstructionKind,
    consumer_key: register_model::RegisterConstraintKey,
    consumer_offset: u32,
) -> ValidatedAddressFold {
    fixture(
        target,
        producer_offset,
        kind_with_offset(consumer_kind, consumer_offset),
        consumer_key,
        &[POINTER, OUTPUT],
    )
}

fn store_fixture(
    target: NativeTarget,
    producer_offset: u32,
    consumer_offset: u32,
    byte_size: u8,
    value: VirtualRegisterId,
) -> ValidatedAddressFold {
    let environment = baseline_target_register_environment(target).unwrap();
    fixture(
        target,
        producer_offset,
        SelectedInstructionKind::Store {
            byte_offset: consumer_offset,
            byte_size,
        },
        keys(&environment).store.unwrap(),
        &[POINTER, value],
    )
}

/// Every displaced consumer form folds on every target: the pointer operand
/// rebinds to the producer's base and the kind carries the combined
/// displacement while identity, row, operands, and provenance stay.
#[test]
fn displaced_consumers_fold_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for (kind, key) in [
            (
                SelectedInstructionKind::Load8 { byte_offset: 24 },
                keys(&environment).load8.unwrap(),
            ),
            (
                SelectedInstructionKind::Load16 { byte_offset: 24 },
                keys(&environment).load16.unwrap(),
            ),
            (
                SelectedInstructionKind::Load32 { byte_offset: 24 },
                keys(&environment).load32.unwrap(),
            ),
            (
                SelectedInstructionKind::Load64 { byte_offset: 24 },
                keys(&environment).load64.unwrap(),
            ),
        ] {
            let source = load_fixture(target, 8, kind, key, 24);
            let result =
                fold_selected_address(&source, 0, CONSUMER, &environment, budget()).unwrap();
            let rewritten = &result.transformed().functions[0].blocks[0].instructions[1];
            assert_eq!(rewritten.id, CONSUMER);
            let expected_offset = match kind {
                SelectedInstructionKind::Load8 { .. } => {
                    SelectedInstructionKind::Load8 { byte_offset: 32 }
                }
                SelectedInstructionKind::Load16 { .. } => {
                    SelectedInstructionKind::Load16 { byte_offset: 32 }
                }
                SelectedInstructionKind::Load32 { .. } => {
                    SelectedInstructionKind::Load32 { byte_offset: 32 }
                }
                _ => SelectedInstructionKind::Load64 { byte_offset: 32 },
            };
            assert_eq!(rewritten.kind, expected_offset);
            assert_eq!(rewritten.constraint, key);
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].operand, 0);
            assert_eq!(rewritten.operands[0].virtual_register, ROOT);
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
            assert_eq!(
                rewritten.provenance,
                source.transformed().functions[0].blocks[0].instructions[1].provenance
            );
            // The producer stays for other readers.
            assert_eq!(
                result.transformed().functions[0].blocks[0].instructions[0],
                source.transformed().functions[0].blocks[0].instructions[0]
            );
            assert_eq!(
                result.receipt().source_selected(),
                source.selected_identity()
            );
            assert_eq!(
                result.receipt().transformed_selected(),
                selected_instruction_plan_identity(result.transformed())
            );
            validate_address_fold(
                &source,
                0,
                CONSUMER,
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap();
            // A detached, separately allocated proposal replays by content.
            let mut detached = result.transformed().clone();
            detached.functions = detached.functions.iter().cloned().collect();
            validate_address_fold(&source, 0, CONSUMER, &environment, budget(), detached).unwrap();
        }
    }
}

/// A referent `Store` folds the same way: operand one keeps the stored
/// value's register and the kind keeps its byte size.
#[test]
fn store_folds_the_combined_scaled_displacement() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = store_fixture(target, 16, 8, 8, VALUE);
    let result = fold_selected_address(&source, 0, CONSUMER, &environment, budget()).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[0].instructions[1];
    assert_eq!(
        rewritten.kind,
        SelectedInstructionKind::Store {
            byte_offset: 24,
            byte_size: 8
        }
    );
    assert_eq!(rewritten.operands[0].virtual_register, ROOT);
    assert_eq!(rewritten.operands[1].virtual_register, VALUE);
    assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Use);
}

/// A store whose value operand names the pointer or the base keeps reading
/// it: only operand zero, the base pointer, rebinds.
#[test]
fn store_value_register_survives_the_rebind() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for value in [POINTER, ROOT] {
        let source = store_fixture(target, 8, 0, 1, value);
        let result = fold_selected_address(&source, 0, CONSUMER, &environment, budget()).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[1];
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::Store {
                byte_offset: 8,
                byte_size: 1
            }
        );
        assert_eq!(rewritten.operands[0].virtual_register, ROOT);
        assert_eq!(rewritten.operands[1].virtual_register, value);
    }
}

/// A projection chained onto a projection folds to the single combined
/// offset off the outer base.
#[test]
fn address_offset_chains_fold() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        8,
        SelectedInstructionKind::AddressOffset { byte_offset: 16 },
        keys(&environment).address_offset.unwrap(),
        &[POINTER, OUTPUT],
    );
    let result = fold_selected_address(&source, 0, CONSUMER, &environment, budget()).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[0].instructions[1];
    assert_eq!(
        rewritten.kind,
        SelectedInstructionKind::AddressOffset { byte_offset: 24 }
    );
    assert_eq!(rewritten.operands[0].virtual_register, ROOT);
    assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
}

/// Definitions carried by the consumer itself do not disturb the read:
/// operand uses read pre-definition, so a `Load64` defining the pointer —
/// or even the producer's base — on its result operand still folds.
#[test]
fn consumer_own_definitions_are_safe() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for output in [POINTER, ROOT] {
        let source = mutated(target, |function, _| {
            function.blocks[0].instructions[1].operands[1].virtual_register = output;
        });
        let result = fold(&source, &environment).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[1];
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::Load64 { byte_offset: 24 }
        );
        assert_eq!(rewritten.operands[0].virtual_register, ROOT);
        assert_eq!(rewritten.operands[1].virtual_register, output);
    }
}

/// The scaled displacement bound table, per architecture: AArch64 admits a
/// combined offset only as a multiple of its width no larger than
/// `width * 4095`; x86-64's disp32 admits every nonnegative byte offset
/// through `i32::MAX`. Each case lists `(aarch64, x86_64)` expectations.
#[test]
fn combined_offset_admission_table() {
    let load64 = |environment: &register_environment::ValidatedTargetRegisterEnvironment| {
        keys(environment).load64.unwrap()
    };
    type Expected = (Result<u32, AddressFoldError>, Result<u32, AddressFoldError>);
    let cases: &[(
        u32,
        u32,
        SelectedInstructionKind,
        fn(
            &register_environment::ValidatedTargetRegisterEnvironment,
        ) -> register_model::RegisterConstraintKey,
        Expected,
    )] = &[
        // Load64 admits combined <= 32760 in multiples of eight on AArch64;
        // x86-64's disp32 admits the same sum without scaling.
        (
            8,
            16,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load64,
            (Ok(24), Ok(24)),
        ),
        (
            8,
            32752,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load64,
            (Ok(32760), Ok(32760)),
        ),
        (
            8,
            32760,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load64,
            (Err(AddressFoldError::UnsupportedOffset), Ok(32768)),
        ),
        // An unaligned sum cannot encode the scaled immediate, but encodes as
        // a byte-exact disp32.
        (
            8,
            20,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load64,
            (Err(AddressFoldError::UnsupportedOffset), Ok(28)),
        ),
        (
            4,
            8,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load64,
            (Err(AddressFoldError::UnsupportedOffset), Ok(12)),
        ),
        // Load8 scales by one; disp32 admits the byte offset regardless.
        (
            0,
            4095,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            |environment| keys(environment).load8.unwrap(),
            (Ok(4095), Ok(4095)),
        ),
        (
            1,
            4095,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            |environment| keys(environment).load8.unwrap(),
            (Err(AddressFoldError::UnsupportedOffset), Ok(4096)),
        ),
        // A combined offset past disp32's positive half refuses everywhere.
        (
            8,
            i32::MAX as u32,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            |environment| keys(environment).load8.unwrap(),
            (
                Err(AddressFoldError::UnsupportedOffset),
                Err(AddressFoldError::UnsupportedOffset),
            ),
        ),
        // Load16 and Load32 scale by their widths on AArch64 only.
        (
            2,
            8188,
            SelectedInstructionKind::Load16 { byte_offset: 0 },
            |environment| keys(environment).load16.unwrap(),
            (Ok(8190), Ok(8190)),
        ),
        (
            4,
            8188,
            SelectedInstructionKind::Load16 { byte_offset: 0 },
            |environment| keys(environment).load16.unwrap(),
            (Err(AddressFoldError::UnsupportedOffset), Ok(8192)),
        ),
        (
            4,
            16376,
            SelectedInstructionKind::Load32 { byte_offset: 0 },
            |environment| keys(environment).load32.unwrap(),
            (Ok(16380), Ok(16380)),
        ),
        (
            8,
            16376,
            SelectedInstructionKind::Load32 { byte_offset: 0 },
            |environment| keys(environment).load32.unwrap(),
            (Err(AddressFoldError::UnsupportedOffset), Ok(16384)),
        ),
        // A chained projection shares the unscaled twelve-bit bound on
        // AArch64; disp32 admits it byte-exact.
        (
            8,
            4087,
            SelectedInstructionKind::AddressOffset { byte_offset: 0 },
            |environment| keys(environment).address_offset.unwrap(),
            (Ok(4095), Ok(4095)),
        ),
        (
            8,
            4088,
            SelectedInstructionKind::AddressOffset { byte_offset: 0 },
            |environment| keys(environment).address_offset.unwrap(),
            (Err(AddressFoldError::UnsupportedOffset), Ok(4096)),
        ),
    ];
    for target in [NativeTarget::linux_arm64(), NativeTarget::linux_x64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for (producer_offset, consumer_offset, kind, key, (aarch64, x64)) in cases {
            let expected = match target.architecture {
                target::Architecture::Aarch64 => aarch64,
                target::Architecture::X86_64 => x64,
            };
            let source = fixture(
                target,
                *producer_offset,
                kind_with_offset(*kind, *consumer_offset),
                key(&environment),
                &[POINTER, OUTPUT],
            );
            let result = fold_selected_address(&source, 0, CONSUMER, &environment, budget());
            match expected {
                Ok(combined) => {
                    let rewritten = &result.as_ref().unwrap().transformed().functions[0].blocks[0]
                        .instructions[1];
                    let offset = match rewritten.kind {
                        SelectedInstructionKind::Load8 { byte_offset }
                        | SelectedInstructionKind::Load16 { byte_offset }
                        | SelectedInstructionKind::Load32 { byte_offset }
                        | SelectedInstructionKind::Load64 { byte_offset }
                        | SelectedInstructionKind::AddressOffset { byte_offset } => byte_offset,
                        _ => panic!("unexpected folded kind"),
                    };
                    assert_eq!(offset, *combined, "{kind:?} {consumer_offset}");
                }
                Err(error) => {
                    assert_eq!(result.unwrap_err(), *error, "{kind:?} {consumer_offset}")
                }
            }
        }
    }
}

/// Store widths admit their architecture's bound; a width outside the byte
/// forms is malformed and refuses before the bound table applies.
#[test]
fn store_width_bound_table() {
    for target in [NativeTarget::linux_arm64(), NativeTarget::linux_x64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for (byte_size, producer, consumer, aarch64, x64) in [
            (1u8, 8u32, 4087u32, Ok(4095u32), Ok(4095u32)),
            (
                1,
                8,
                4088,
                Err(AddressFoldError::UnsupportedOffset),
                Ok(4096),
            ),
            (2, 8, 8182, Ok(8190), Ok(8190)),
            // An unaligned sum cannot encode the scaled immediate; disp32 is
            // byte-exact.
            (
                2,
                9,
                8182,
                Err(AddressFoldError::UnsupportedOffset),
                Ok(8191),
            ),
            (
                2,
                8,
                8184,
                Err(AddressFoldError::UnsupportedOffset),
                Ok(8192),
            ),
            (4, 8, 16372, Ok(16380), Ok(16380)),
            (8, 16, 32744, Ok(32760), Ok(32760)),
            (
                8,
                16,
                32752,
                Err(AddressFoldError::UnsupportedOffset),
                Ok(32768),
            ),
        ] {
            let expected = match target.architecture {
                target::Architecture::Aarch64 => aarch64,
                target::Architecture::X86_64 => x64,
            };
            let source = store_fixture(target, producer, consumer, byte_size, VALUE);
            let result = fold_selected_address(&source, 0, CONSUMER, &environment, budget());
            match expected {
                Ok(combined) => assert_eq!(
                    result.unwrap().transformed().functions[0].blocks[0].instructions[1].kind,
                    SelectedInstructionKind::Store {
                        byte_offset: combined,
                        byte_size
                    },
                    "size {byte_size}"
                ),
                Err(error) => assert_eq!(result.unwrap_err(), error, "size {byte_size}"),
            }
        }
        for byte_size in [0u8, 3, 16, 255] {
            let source = store_fixture(target, 8, 0, byte_size, VALUE);
            assert_eq!(
                fold_selected_address(&source, 0, CONSUMER, &environment, budget()).unwrap_err(),
                AddressFoldError::UnsupportedInstruction,
                "size {byte_size}"
            );
        }
    }
}

/// A `u32` sum overflow cannot encode any displacement.
#[test]
fn offset_sum_overflow_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = load_fixture(
        target,
        u32::MAX,
        SelectedInstructionKind::Load8 { byte_offset: 0 },
        keys(&environment).load8.unwrap(),
        1,
    );
    assert_eq!(
        fold_selected_address(&source, 0, CONSUMER, &environment, budget()).unwrap_err(),
        AddressFoldError::UnsupportedOffset
    );
}

fn kind_with_offset(kind: SelectedInstructionKind, byte_offset: u32) -> SelectedInstructionKind {
    match kind {
        SelectedInstructionKind::Load8 { .. } => SelectedInstructionKind::Load8 { byte_offset },
        SelectedInstructionKind::Load16 { .. } => SelectedInstructionKind::Load16 { byte_offset },
        SelectedInstructionKind::Load32 { .. } => SelectedInstructionKind::Load32 { byte_offset },
        SelectedInstructionKind::Load64 { .. } => SelectedInstructionKind::Load64 { byte_offset },
        SelectedInstructionKind::Store { byte_size, .. } => SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        },
        SelectedInstructionKind::AddressOffset { .. } => {
            SelectedInstructionKind::AddressOffset { byte_offset }
        }
        other => other,
    }
}

/// Only the referent `AddressOffset` may witness the producer: every other
/// defining instruction — and every later in-block definition — refuses, as
/// do a self-referential projection and a producer that never lands in the
/// consumer's block.
#[test]
fn producer_admission_table() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = keys(&environment);
    // Non-AddressOffset producers for the same pointer register.
    use SelectedInstructionKind::*;
    for (producer_kind, producer_key) in [
        (CopyI64, keys.copy_i64),
        (
            MaterializeI64 {
                value: semantic_vocabulary::IntegerValue::Unsigned(8),
            },
            keys.materialize_i64,
        ),
        (Load64 { byte_offset: 0 }, keys.load64.unwrap()),
    ] {
        let source = mutated(target, |function, environment| {
            let row = environment.constraint(producer_key).unwrap().clone();
            let registers: &[VirtualRegisterId] = match producer_kind {
                CopyI64 | Load64 { .. } => &[ROOT, POINTER],
                _ => &[POINTER],
            };
            function.blocks[0].instructions[0] =
                instruction(ADDRESS, producer_kind, &row, registers);
        });
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            AddressFoldError::UnsupportedProducer,
            "{producer_kind:?}"
        );
    }
    // A pointer with no in-block definition — an entry or block parameter —
    // has no producer to fold.
    let parameter = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(0);
        function.virtual_registers[1].origin = VirtualRegisterOrigin::BlockParameter {
            source_value: ValueId::new(2).unwrap(),
            block: SelectedBlockId(0),
            parameter_index: 0,
        };
    });
    assert_eq!(
        fold(&parameter, &environment).unwrap_err(),
        AddressFoldError::UnsupportedProducer
    );
    // A projection writing its own base cannot rebind to a pre-offset value.
    let self_projection = mutated(target, |function, _| {
        function.blocks[0].instructions[0].operands[0].virtual_register = POINTER;
    });
    assert_eq!(
        fold(&self_projection, &environment).unwrap_err(),
        AddressFoldError::UnsupportedProducer
    );
    // A later in-block definition of the pointer, not the `AddressOffset`,
    // is what the consumer observes.
    let redefined = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions.insert(
            1,
            instruction(SelectedInstructionId(7), CopyI64, &copy, &[ROOT, POINTER]),
        );
    });
    assert_eq!(
        fold(&redefined, &environment).unwrap_err(),
        AddressFoldError::UnsupportedProducer
    );
    // A malformed producer shape: extra operand, `UseDef` result, decorated
    // operand, implicit traffic.
    for mutation in 0..5 {
        let source = mutated(target, |function, _| {
            let producer = &mut function.blocks[0].instructions[0];
            match mutation {
                0 => producer.operands.push(producer.operands[0]),
                1 => producer.operands[1].access = RegisterOperandAccess::UseDef,
                2 => {
                    producer.operands[0].fixed_view = Some(register_model::RegisterViewId(0));
                }
                3 => producer.implicit_uses = vec![register_model::RegisterUnitId(0)],
                _ => producer.clobbers = vec![register_model::RegisterUnitId(0)],
            }
        });
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            AddressFoldError::UnsupportedProducer,
            "mutation {mutation}"
        );
    }
    // The pointer's roster row must exist.
    let missing_pointer = mutated(target, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != POINTER);
    });
    assert_eq!(
        fold(&missing_pointer, &environment).unwrap_err(),
        AddressFoldError::UnsupportedProducer
    );
    // The base register must exist in the roster at the consumer operand's
    // declared class.
    let missing_base = mutated(target, |function, _| {
        function.virtual_registers.retain(|entry| entry.id != ROOT);
    });
    assert_eq!(
        fold(&missing_base, &environment).unwrap_err(),
        AddressFoldError::UnsupportedUse
    );
    let wrong_class = mutated(target, |function, _| {
        function.virtual_registers[0].class = register_model::RegisterClassId(u16::MAX);
    });
    assert_eq!(
        fold(&wrong_class, &environment).unwrap_err(),
        AddressFoldError::UnsupportedUse
    );
}

/// Anything strictly between the producer and the consumer that redefines
/// the producer's base leaves operand zero reading the new value — the fold
/// would observe the wrong base.
#[test]
fn intervening_base_redefinition_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let redefined = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions.insert(
            1,
            instruction(
                SelectedInstructionId(7),
                SelectedInstructionKind::CopyI64,
                &copy,
                &[OUTPUT, ROOT],
            ),
        );
    });
    assert_eq!(
        fold(&redefined, &environment).unwrap_err(),
        AddressFoldError::UnsupportedUse
    );
    // A `UseDef` rewrite of the base counts as a redefinition too.
    let usedef = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        let mut extra = instruction(
            SelectedInstructionId(7),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[ROOT, OUTPUT],
        );
        extra.operands[0].access = RegisterOperandAccess::UseDef;
        extra.operands[0].virtual_register = ROOT;
        function.blocks[0].instructions.insert(1, extra);
    });
    assert_eq!(
        fold(&usedef, &environment).unwrap_err(),
        AddressFoldError::UnsupportedUse
    );
}

/// Only the six displacement-carrying referent forms admit the fold; every
/// other kind — indexed, slot, packed, or register-only — has no referent
/// displacement to absorb the producer's offset.
#[test]
fn unsupported_consumer_kinds_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = keys(&environment);
    use SelectedInstructionKind::*;
    let cases: &[(
        SelectedInstructionKind,
        register_model::RegisterConstraintKey,
        &[VirtualRegisterId],
    )] = &[
        (
            Load8Indexed,
            keys.load8_indexed.unwrap(),
            &[POINTER, ROOT, OUTPUT],
        ),
        (CopyI64, keys.copy_i64, &[POINTER, OUTPUT]),
        (CompareI64, keys.compare_i64, &[POINTER, ROOT]),
        (
            Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(
                    selected_instructions::LocalStorageSlotId::Spill { register: SPARE },
                ),
                byte_offset: 0,
            },
            keys.store64.unwrap(),
            &[POINTER],
        ),
        (
            FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::Local(
                    selected_instructions::LocalStorageSlotId::Spill { register: SPARE },
                ),
                byte_offset: 0,
            },
            keys.frame_address.unwrap(),
            &[OUTPUT],
        ),
        // Packed forms carry a `byte_offset` but their operand zero is a
        // referent pointer with a different operand roster; the fold does
        // not reach into them.
        (
            LoadPacked {
                byte_offset: 0,
                width: selected_instructions::PackedByteWidth::Five,
            },
            keys.load_packed.unwrap(),
            &[POINTER, OUTPUT, SPARE],
        ),
        (
            StorePacked {
                byte_offset: 0,
                width: selected_instructions::PackedByteWidth::Five,
            },
            keys.store_packed.unwrap(),
            &[POINTER, VALUE, SPARE],
        ),
    ];
    for (kind, key, registers) in cases {
        let source = fixture(target, 8, *kind, *key, registers);
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            AddressFoldError::UnsupportedInstruction,
            "{kind:?}"
        );
    }
}

/// Malformed consumer shapes: a written or decorated base operand, implicit
/// unit traffic, or the wrong operand count.
#[test]
fn malformed_consumer_shapes_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..8 {
        let source = mutated(target, |function, _| {
            let consumer = &mut function.blocks[0].instructions[1];
            match mutation {
                // The base operand must be a read, not a write or rewrite.
                0 => consumer.operands[0].access = RegisterOperandAccess::Def,
                1 => consumer.operands[0].access = RegisterOperandAccess::UseDef,
                // Operand constraints cannot ride into the rebound operand.
                2 => {
                    consumer.operands[0].fixed_view = Some(register_model::RegisterViewId(0));
                }
                3 => consumer.operands[1].tied_to = Some(0),
                4 => consumer.operands[1].early_clobber = true,
                // Implicit unit traffic has no place on these canonical forms.
                5 => consumer.implicit_uses = vec![register_model::RegisterUnitId(0)],
                6 => consumer.implicit_defs = vec![register_model::RegisterUnitId(0)],
                _ => consumer.operands.push(consumer.operands[0]),
            }
        });
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            AddressFoldError::UnsupportedInstruction,
            "mutation {mutation}"
        );
    }
    // A clobber the canonical form does not declare rejects as well.
    let clobber = mutated(target, |function, _| {
        function.blocks[0].instructions[1].clobbers = vec![register_model::RegisterUnitId(0)];
    });
    assert_eq!(
        fold(&clobber, &environment).unwrap_err(),
        AddressFoldError::UnsupportedInstruction
    );
}

/// The declared rows must agree with the emitted operands and the roster:
/// a wrong key, a mismatched roster class, or a producer row that does not
/// declare `[use, def]` all refuse.
#[test]
fn constraint_and_effect_surfaces_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The consumer's declared row must publish its operand shape: a
    // `[use, use]` compare row is not the load's `[use, def]`.
    let wrong_row = mutated(target, |function, environment| {
        function.blocks[0].instructions[1].constraint = environment.selected_keys().compare_i64;
    });
    assert_eq!(
        fold(&wrong_row, &environment).unwrap_err(),
        AddressFoldError::ConstraintMismatch
    );
    // The producer's row must declare the `[use, def]` shape.
    let wrong_producer_row = mutated(target, |function, environment| {
        function.blocks[0].instructions[0].constraint = environment.selected_keys().compare_i64;
    });
    assert_eq!(
        fold(&wrong_producer_row, &environment).unwrap_err(),
        AddressFoldError::ConstraintMismatch
    );
    // The pointer's roster class must match the operand and row classes.
    let wrong_pointer_class = mutated(target, |function, _| {
        function.virtual_registers[1].class = register_model::RegisterClassId(u16::MAX);
    });
    assert_eq!(
        fold(&wrong_pointer_class, &environment).unwrap_err(),
        AddressFoldError::ConstraintMismatch
    );
    // The result register must exist in the roster.
    let missing_output = mutated(target, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != OUTPUT);
    });
    assert_eq!(
        fold(&missing_output, &environment).unwrap_err(),
        AddressFoldError::UnsupportedUse
    );
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = load_fixture(
        target,
        8,
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        keys(&environment).load64.unwrap(),
        16,
    );
    assert_eq!(
        fold_selected_address(&source, 1, CONSUMER, &environment, budget()).unwrap_err(),
        AddressFoldError::SourceMismatch
    );
    assert_eq!(
        fold_selected_address(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        AddressFoldError::SourceMismatch
    );
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        fold_selected_address(&source, 0, CONSUMER, &wrong_environment, budget()).unwrap_err(),
        AddressFoldError::SourceMismatch
    );
}

#[test]
fn validation_budget_covers_the_scans() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = load_fixture(
        target,
        8,
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        keys(&environment).load64.unwrap(),
        16,
    );
    let tiny = OptimizationWorkBudget::new(100, 100, 4, 100, 100).unwrap();
    assert_eq!(
        fold_selected_address(&source, 0, CONSUMER, &environment, tiny).unwrap_err(),
        AddressFoldError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan and two scans of the
/// consumer's block — seven steps for the two-instruction fixture — so the
/// exact count admits the fold on both the proposal and the independent
/// replay path while one step below rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = load_fixture(
        target,
        8,
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        keys(&environment).load64.unwrap(),
        16,
    );
    // (1 block) + (2 instructions) + (2 scans of 2) = 7 measured steps.
    let exact = OptimizationWorkBudget::new(1, 1, 7, 1, 1).unwrap();
    let result = fold_selected_address(&source, 0, CONSUMER, &environment, exact).unwrap();
    validate_address_fold(
        &source,
        0,
        CONSUMER,
        &environment,
        exact,
        result.transformed().clone(),
    )
    .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, 6, 1, 1).unwrap();
    assert_eq!(
        fold_selected_address(&source, 0, CONSUMER, &environment, starved).unwrap_err(),
        AddressFoldError::WorkBudgetExceeded
    );
    assert_eq!(
        validate_address_fold(
            &source,
            0,
            CONSUMER,
            &environment,
            starved,
            result.transformed().clone(),
        )
        .unwrap_err(),
        AddressFoldError::WorkBudgetExceeded
    );
    // A three-instruction block scans longer: (1 block) + (3 instructions)
    // + (2 scans of 3) = 10 measured steps.
    let wider = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.virtual_registers.push(register(
            SPARE,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            function.virtual_registers[0].class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(7),
                source_value: ValueId::new(5).unwrap(),
            },
        ));
        function.blocks[0].instructions.insert(
            2,
            instruction(
                SelectedInstructionId(7),
                SelectedInstructionKind::CopyI64,
                &copy,
                &[POINTER, SPARE],
            ),
        );
    });
    let exact = OptimizationWorkBudget::new(1, 1, 10, 1, 1).unwrap();
    let result = fold_selected_address(&wider, 0, CONSUMER, &environment, exact).unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, 9, 1, 1).unwrap();
    assert_eq!(
        fold_selected_address(&wider, 0, CONSUMER, &environment, starved).unwrap_err(),
        AddressFoldError::WorkBudgetExceeded
    );
    assert_eq!(
        validate_address_fold(
            &wider,
            0,
            CONSUMER,
            &environment,
            starved,
            result.transformed().clone(),
        )
        .unwrap_err(),
        AddressFoldError::WorkBudgetExceeded
    );
}

/// Other readers of the pointer register do not block the fold; the
/// `AddressOffset` producer is retained for them.
#[test]
fn other_uses_of_the_pointer_survive() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let copy = environment
        .constraint(keys(&environment).copy_i64)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        function.virtual_registers.push(register(
            SPARE,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            function.virtual_registers[0].class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(7),
                source_value: ValueId::new(5).unwrap(),
            },
        ));
        let mut reader = instruction(
            SelectedInstructionId(7),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[POINTER, SPARE],
        );
        reader.provenance.values = vec![ValueId::new(6).unwrap()];
        function.blocks[0].instructions.insert(2, reader);
    });
    let result = fold(&source, &environment).unwrap();
    let transformed = result.transformed();
    let instructions = &transformed.functions[0].blocks[0].instructions;
    assert_eq!(
        instructions[0].kind,
        SelectedInstructionKind::AddressOffset { byte_offset: 8 }
    );
    assert_eq!(
        instructions[1].kind,
        SelectedInstructionKind::Load64 { byte_offset: 24 }
    );
    assert_eq!(instructions[1].operands[0].virtual_register, ROOT);
    assert_eq!(instructions[2].kind, SelectedInstructionKind::CopyI64);
    assert_eq!(instructions[2].operands[0].virtual_register, POINTER);
}

/// A validated fold is itself a sealed analysis source: a second, disjoint
/// address fold applies on the transformed program without re-entering the
/// stage.
#[test]
fn validated_results_compose() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = keys(&environment);
    let second_address = SelectedInstructionId(8);
    let second_consumer = SelectedInstructionId(9);
    let source = mutated(target, |function, environment| {
        let address = environment
            .constraint(keys.address_offset.unwrap())
            .unwrap()
            .clone();
        let load = environment
            .constraint(keys.load64.unwrap())
            .unwrap()
            .clone();
        let class = function.virtual_registers[0].class;
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        function.virtual_registers.push(register(
            SPARE,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: second_address,
                source_value: ValueId::new(5).unwrap(),
            },
        ));
        function.virtual_registers.push(register(
            VirtualRegisterId(5),
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: second_consumer,
                source_value: ValueId::new(6).unwrap(),
            },
        ));
        function.blocks[0].instructions.push(instruction(
            second_address,
            SelectedInstructionKind::AddressOffset { byte_offset: 32 },
            &address,
            &[POINTER, SPARE],
        ));
        function.blocks[0].instructions.push(instruction(
            second_consumer,
            SelectedInstructionKind::Load64 { byte_offset: 8 },
            &load,
            &[SPARE, VirtualRegisterId(5)],
        ));
    });
    let first = fold(&source, &environment).unwrap();
    let second = fold_selected_address(&first, 0, second_consumer, &environment, budget()).unwrap();
    let instructions = &second.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        instructions[1].kind,
        SelectedInstructionKind::Load64 { byte_offset: 24 }
    );
    assert_eq!(instructions[1].operands[0].virtual_register, ROOT);
    // The chained fold composes: SPARE was POINTER + 32, so the second load
    // now reads the first producer's pointer at combined 40 — its own base
    // operand is the retained first projection, which folded elsewhere.
    assert_eq!(
        instructions[3].kind,
        SelectedInstructionKind::Load64 { byte_offset: 40 }
    );
    assert_eq!(instructions[3].operands[0].virtual_register, POINTER);
    assert_eq!(
        second.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
}

/// Replay accepts only the reconstructed folded instruction: any tampered
/// register, displacement, kind, identity, provenance, producer, or roster
/// row fails the restore-by-content check.
#[test]
fn replay_rejects_anything_but_the_exact_form() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = load_fixture(
        target,
        8,
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        keys(&environment).load64.unwrap(),
        16,
    );
    let result = fold(&source, &environment).unwrap();
    for mutation in 0..10 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The wrong surviving base register.
            0 => {
                function.blocks[0].instructions[1].operands[0].virtual_register = VALUE;
            }
            // A different combined displacement.
            1 => {
                function.blocks[0].instructions[1].kind =
                    SelectedInstructionKind::Load64 { byte_offset: 32 };
            }
            // A different width is not the admitted kind.
            2 => {
                function.blocks[0].instructions[1].kind =
                    SelectedInstructionKind::Load32 { byte_offset: 24 };
            }
            // The source displacement without the producer's offset folded
            // in is not the reconstructed form.
            3 => {
                function.blocks[0].instructions[1].kind =
                    SelectedInstructionKind::Load64 { byte_offset: 16 };
            }
            // A different instruction id on the folded form.
            4 => function.blocks[0].instructions[1].id = ADDRESS,
            // The consumer's provenance must survive intact.
            5 => {
                function.blocks[0].instructions[1]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // The retained producer must remain untouched.
            6 => {
                function.blocks[0].instructions[0].kind =
                    SelectedInstructionKind::AddressOffset { byte_offset: 16 };
            }
            // The dropped register must not linger on another operand.
            7 => {
                let operand = function.blocks[0].instructions[1].operands[0];
                function.blocks[0].instructions[1].operands.push(operand);
            }
            // An unrelated register must stay identical.
            8 => function.virtual_registers[2].scalar_type = ScalarType::Boolean,
            // A fabricated roster row has no source counterpart.
            9 => {
                function
                    .memory_accesses
                    .push(selected_instructions::SelectedMemoryAccess {
                        instruction: CONSUMER,
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
            validate_address_fold(&source, 0, CONSUMER, &environment, budget(), proposed).is_err(),
            "mutation {mutation}"
        );
    }
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input: the folded
/// consumer reads the producer's base register now, so a second fold at the
/// same site finds no in-block `AddressOffset` producer for it.
#[test]
fn fold_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let build = || {
        load_fixture(
            target,
            8,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            keys(&environment).load64.unwrap(),
            16,
        )
    };
    let first = fold(&build(), &environment).unwrap();
    let second = fold(&build(), &environment).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        fold_selected_address(&first, 0, CONSUMER, &environment, budget()).unwrap_err(),
        AddressFoldError::UnsupportedProducer
    );
    // The retained producer is equally terminal: its own base operand has
    // no earlier in-block `AddressOffset` definition to fold through.
    assert_eq!(
        fold_selected_address(&first, 0, ADDRESS, &environment, budget()).unwrap_err(),
        AddressFoldError::UnsupportedProducer
    );
}

/// Replay corruption in a block the fold never touched still rejects: the
/// restore-by-content check compares the complete plan, not just the block
/// carrying the folded pair.
#[test]
fn replay_rejects_drift_outside_the_rewritten_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Stretch the fixture across one edge: the entry block folds and jumps
    // to a second block that returns.
    let source = mutated(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let tail = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::Jump,
                    &jump,
                    &[],
                ),
                successor: SelectedSuccessor {
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
    let result = fold(&source, &environment).unwrap();
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
            SelectedInstructionId(7),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[ROOT, VALUE],
        ));
    assert_eq!(
        validate_address_fold(&source, 0, CONSUMER, &environment, budget(), proposed).unwrap_err(),
        AddressFoldError::ReplayMismatch
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
    return_instruction.id = SelectedInstructionId(9);
    assert_eq!(
        validate_address_fold(&source, 0, CONSUMER, &environment, budget(), proposed).unwrap_err(),
        AddressFoldError::ReplayMismatch
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
                SelectedInstructionId(8),
                SelectedInstructionKind::ReturnUnit,
                &return_row,
                &[],
            ),
            psi_return_edge: EdgeId::new(3).unwrap(),
        },
    });
    assert_eq!(
        validate_address_fold(&source, 0, CONSUMER, &environment, budget(), proposed).unwrap_err(),
        AddressFoldError::ReplayMismatch
    );
}

/// Edit the single fixture function, then refresh the receipt identities so
/// the mutated plan is a well-formed analysis source.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedAddressFold {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = load_fixture(
        target,
        8,
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        keys(&environment).load64.unwrap(),
        16,
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

fn fold(
    source: &ValidatedAddressFold,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedAddressFold, AddressFoldError> {
    fold_selected_address(source, 0, CONSUMER, environment, budget())
}
