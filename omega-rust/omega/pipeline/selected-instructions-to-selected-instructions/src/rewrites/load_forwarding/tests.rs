use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedCasePayloadBinding, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand,
    SelectedStructuralBinding, SelectedStructuralCaseEdge, SelectedStructuralTransport,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding,
    SelectedValueTransport, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, ValueId,
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

const STORE: SelectedInstructionId = SelectedInstructionId(2);
const BETWEEN: SelectedInstructionId = SelectedInstructionId(3);
const LOAD: SelectedInstructionId = SelectedInstructionId(4);
const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const VALUE: VirtualRegisterId = VirtualRegisterId(1);
const SCRATCH: VirtualRegisterId = VirtualRegisterId(2);
const OUTPUT: VirtualRegisterId = VirtualRegisterId(3);

fn place() -> PlaceId {
    PlaceId::new(1).unwrap()
}

fn access(
    instruction: SelectedInstructionId,
    operation: u64,
    place: PlaceId,
    byte_offset: u32,
    role: SelectedMemoryAccessRole,
) -> SelectedMemoryAccess {
    SelectedMemoryAccess {
        instruction,
        origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(operation).unwrap()),
        place,
        byte_offset,
        byte_count: 8,
        role,
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r1 = copy r0; store [r0+0] <- r1; r3 = copy r0; r2 = load [r0+0]; return`.
fn fixture(target: NativeTarget) -> ValidatedStoredLoadForwarding {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let store = environment.constraint(keys.store.unwrap()).unwrap();
    let load = environment.constraint(keys.load64.unwrap()).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = copy.operands[0].class;
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let source_value = ValueId::new(1).unwrap();
    let registers = vec![
        VirtualRegister {
            id: POINTER,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value,
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        VirtualRegister {
            id: VALUE,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(1),
                source_value: ValueId::new(2).unwrap(),
            },
            definition_site: None,
            entry_fixed_view: None,
        },
        VirtualRegister {
            id: SCRATCH,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: BETWEEN,
                source_value: ValueId::new(3).unwrap(),
            },
            definition_site: None,
            entry_fixed_view: None,
        },
        VirtualRegister {
            id: OUTPUT,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: LOAD,
                source_value: ValueId::new(4).unwrap(),
            },
            definition_site: None,
            entry_fixed_view: None,
        },
    ];
    let instructions = vec![
        instruction(
            SelectedInstructionId(1),
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, VALUE],
        ),
        instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, VALUE],
        ),
        instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, SCRATCH],
        ),
        {
            let mut forwarded_load = instruction(
                LOAD,
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                load,
                &[POINTER, OUTPUT],
            );
            forwarded_load.provenance.operations = vec![OperationId::new(2).unwrap()];
            forwarded_load.provenance.values = vec![ValueId::new(4).unwrap()];
            forwarded_load
        },
    ];
    let machine = MachineId::new(1).unwrap();
    let place = place();
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
            memory_accesses: vec![
                access(STORE, 1, place, 0, SelectedMemoryAccessRole::WritePlace),
                access(LOAD, 2, place, 0, SelectedMemoryAccessRole::ReadPlace),
            ],
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
    ValidatedStoredLoadForwarding {
        receipt: StoredLoadForwardingReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

#[test]
fn same_block_store_forwards_through_copy_and_drops_the_read_row() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = fixture(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let source_load = source.transformed().functions[0].blocks[0].instructions[3].clone();
        let result =
            forward_selected_stored_load(&source, 0, LOAD, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        let rewritten = &function.blocks[0].instructions[3];
        assert_eq!(rewritten.id, LOAD);
        assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VALUE);
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.provenance, source_load.provenance);
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| access.role)
                .collect::<Vec<_>>(),
            vec![SelectedMemoryAccessRole::WritePlace]
        );
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        // The rewritten function detached from the source's shared storage.
        assert!(!std::ptr::eq(
            &source.transformed().functions[0],
            &result.transformed().functions[0]
        ));
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), detached)
            .unwrap();
    }
}

#[test]
fn replay_rejects_anything_but_the_exact_forwarding() {
    let target = NativeTarget::linux_x64();
    let source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let result = forward_selected_stored_load(&source, 0, LOAD, &environment, budget()).unwrap();
    for mutation in 0..9 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // Forwarded from the wrong register.
            0 => {
                function.blocks[0].instructions[3].operands[0].virtual_register = POINTER;
            }
            // Different result register.
            1 => {
                function.blocks[0].instructions[3].operands[1].virtual_register = SCRATCH;
            }
            // Kept the read row instead of dropping it.
            2 => {
                function.memory_accesses.push(access(
                    LOAD,
                    2,
                    place(),
                    0,
                    SelectedMemoryAccessRole::ReadPlace,
                ));
            }
            // Dropped the store's row as well.
            3 => {
                function.memory_accesses.remove(0);
            }
            // The surviving store must remain untouched.
            4 => {
                function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                };
            }
            // A different instruction id on the copy.
            5 => function.blocks[0].instructions[3].id = BETWEEN,
            // Fresh provenance must stay the read's.
            6 => {
                function.blocks[0].instructions[3]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // An unrelated register must stay identical.
            7 => function.virtual_registers[1].scalar_type = ScalarType::Boolean,
            // The read must not survive elsewhere in the block.
            8 => {
                function.blocks[0].instructions[3].kind =
                    SelectedInstructionKind::Load64 { byte_offset: 0 };
            }
            _ => unreachable!(),
        }
        assert!(
            validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

/// Edit the single fixture function, then refresh the receipt identities so the
/// mutated plan is a well-formed analysis source.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedStoredLoadForwarding {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(target);
    edit(
        &mut std::sync::Arc::make_mut(&mut source.transformed).functions[0],
        &environment,
    );
    let identity = selected_instruction_plan_identity(&source.transformed);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

fn forward(
    source: &ValidatedStoredLoadForwarding,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedStoredLoadForwarding, StoredLoadForwardingError> {
    forward_selected_stored_load(source, 0, LOAD, environment, budget())
}

#[test]
fn overlapping_or_dynamic_writes_between_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Partial overwrite of the forwarded range cannot forward.
    let partial = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 4,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 4,
                ..access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace)
            },
        );
    });
    assert_eq!(
        forward(&partial, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A dynamic-extent byte-sequence write to the same place cannot be proven
    // disjoint.
    let dynamic = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                0,
                SelectedMemoryAccessRole::WriteByteSequence {
                    index: ValueId::new(5).unwrap(),
                    value: ValueId::new(6).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32],
                    ),
                },
            ),
        );
    });
    assert_eq!(
        forward(&dynamic, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A place-backed local slot write targets the forwarded place's storage.
    let local = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(9).unwrap(),
            place: place(),
        };
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                0,
                SelectedMemoryAccessRole::WriteLocal { slot },
            ),
        );
    });
    assert_eq!(
        forward(&local, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // Materializing the place-backed local address lets later writes reach it.
    let address = mutated(target, |function, _| {
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                0,
                SelectedMemoryAccessRole::AddressLocal { slot },
            ),
        );
    });
    assert_eq!(
        forward(&address, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // An unaccounted referent write (no row) rejects.
    let unaccounted = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
    });
    assert_eq!(
        forward(&unaccounted, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

#[test]
fn harmless_accesses_and_private_slots_still_forward() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A different place root cannot share writable storage with the forwarded
    // place under place exclusivity.
    let other_place = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                PlaceId::new(2).unwrap(),
                0,
                SelectedMemoryAccessRole::WritePlace,
            ),
        );
    });
    forward(&other_place, &environment).unwrap();
    // A disjoint range of the same place cannot touch the forwarded bytes.
    let disjoint = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 16,
                byte_size: 8,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                16,
                SelectedMemoryAccessRole::WritePlace,
            ),
        );
    });
    forward(&disjoint, &environment).unwrap();
    // A re-read of the forwarded range cannot clobber it.
    let reread = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 0, SelectedMemoryAccessRole::ReadPlace),
        );
    });
    forward(&reread, &environment).unwrap();
    // Private spill-slot traffic carries no row and cannot alias a place.
    let spill = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(LocalStorageSlotId::Spill {
                    register: VirtualRegisterId(9),
                }),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
    });
    forward(&spill, &environment).unwrap();
}

/// The fixture narrowed to an exact-width sub-word pair: `Store { byte_size }`
/// and the matching `Load8`/`Load16`/`Load32` with `byte_count`-matched rows.
fn narrowed(target: NativeTarget, byte_size: u8) -> ValidatedStoredLoadForwarding {
    mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let store = environment.constraint(keys.store.unwrap()).unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size,
            },
            store,
            &[POINTER, VALUE],
        );
        let (kind, key) = match byte_size {
            1 => (
                SelectedInstructionKind::Load8 { byte_offset: 0 },
                keys.load8,
            ),
            2 => (
                SelectedInstructionKind::Load16 { byte_offset: 0 },
                keys.load16,
            ),
            _ => (
                SelectedInstructionKind::Load32 { byte_offset: 0 },
                keys.load32,
            ),
        };
        let load = environment.constraint(key.unwrap()).unwrap();
        let mut rewritten = instruction(LOAD, kind, load, &[POINTER, OUTPUT]);
        rewritten.provenance.operations = vec![OperationId::new(2).unwrap()];
        rewritten.provenance.values = vec![ValueId::new(4).unwrap()];
        function.blocks[0].instructions[3] = rewritten;
        function.memory_accesses[0].byte_count = u32::from(byte_size);
        function.memory_accesses[1].byte_count = u32::from(byte_size);
    })
}

#[test]
fn sub_width_loads_forward_through_exact_width_stores() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for (byte_size, expected) in [
            (1u8, SelectedInstructionKind::ZeroExtendU8),
            (2, SelectedInstructionKind::ZeroExtendU16),
            (4, SelectedInstructionKind::ZeroExtendU32),
        ] {
            let source = narrowed(target, byte_size);
            let source_load = source.transformed().functions[0].blocks[0].instructions[3].clone();
            let result = forward(&source, &environment).unwrap();
            let function = &result.transformed().functions[0];
            let rewritten = &function.blocks[0].instructions[3];
            assert_eq!(rewritten.id, LOAD);
            assert_eq!(rewritten.kind, expected);
            assert_eq!(rewritten.constraint, environment.selected_keys().copy_i64);
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].virtual_register, VALUE);
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
            assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
            assert_eq!(rewritten.provenance, source_load.provenance);
            assert_eq!(
                function
                    .memory_accesses
                    .iter()
                    .map(|access| (access.role, access.byte_count))
                    .collect::<Vec<_>>(),
                vec![(SelectedMemoryAccessRole::WritePlace, u32::from(byte_size))]
            );
            // Replay restores the complete source by content.
            validate_stored_load_forwarding(
                &source,
                0,
                LOAD,
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap();
        }
    }
}

#[test]
fn sub_width_loads_reject_wider_shifted_or_mismatched_sources() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A wider store keeps bytes the one-byte read cannot name portably: byte
    // order is the target's, so only an exact-width writer forwards.
    let wider = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, OUTPUT],
        );
        function.memory_accesses[1].byte_count = 1;
    });
    assert_eq!(
        forward(&wider, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A byte read inside a wider store's range is not the forwarded value.
    let shifted = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 4 },
            load,
            &[POINTER, OUTPUT],
        );
        function.memory_accesses[1].byte_offset = 4;
        function.memory_accesses[1].byte_count = 1;
    });
    assert_eq!(
        forward(&shifted, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A roster width disagreeing with the load kind is not the read's
    // identity; the pair is unsupported rather than a bad forward.
    let mismatched = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, OUTPUT],
        );
    });
    assert_eq!(
        forward(&mismatched, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // A store row claiming one byte while the instruction writes eight leaves
    // the read's neighbors unaccounted.
    let narrower_row = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let load = environment.constraint(keys.load8.unwrap()).unwrap();
        function.blocks[0].instructions[3] = instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, OUTPUT],
        );
        function.memory_accesses[0].byte_count = 1;
        function.memory_accesses[1].byte_count = 1;
    });
    assert_eq!(
        forward(&narrower_row, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

#[test]
fn sub_width_replay_rejects_a_full_width_copy() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = narrowed(target, 1);
    let result = forward(&source, &environment).unwrap();
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[0].instructions[3].kind = SelectedInstructionKind::CopyI64;
    assert_eq!(
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), proposed)
            .unwrap_err(),
        StoredLoadForwardingError::ReplayMismatch
    );
}

#[test]
fn calls_hosted_effects_and_use_violations_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let call = mutated(target, |function, environment| {
        let call = environment
            .constraint(environment.selected_keys().call_unit[0])
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CallUnit {
                callee: MachineId::new(2).unwrap(),
            },
            call,
            &[],
        );
    });
    assert_eq!(
        forward(&call, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedInstruction
    );
    // Redefining the carried value between store and load must not forward.
    let redefined = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, VALUE],
        );
    });
    assert_eq!(
        forward(&redefined, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
}

#[test]
fn admission_boundaries_and_budget_hold() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A nonexistent instruction id cannot forward.
    assert_eq!(
        forward_selected_stored_load(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        StoredLoadForwardingError::SourceMismatch
    );
    // A non-load instruction id cannot forward.
    assert_eq!(
        forward_selected_stored_load(&source, 0, STORE, &environment, budget()).unwrap_err(),
        StoredLoadForwardingError::UnsupportedInstruction
    );
    // The wrong target's environment is a different source.
    let foreign = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        forward(&source, &foreign).unwrap_err(),
        StoredLoadForwardingError::SourceMismatch
    );
    // Without a preceding same-range store there is nothing to forward.
    let storeless = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(1);
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        forward(&storeless, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // The load's read row is required; a private reload has nothing to prove.
    let rowless = mutated(target, |function, _| {
        function.memory_accesses.remove(1);
    });
    assert_eq!(
        forward(&rowless, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedInstruction
    );
    let tiny = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        forward_selected_stored_load(&source, 0, LOAD, &environment, tiny).unwrap_err(),
        StoredLoadForwardingError::WorkBudgetExceeded
    );
}

/// A plain semantic edge to `block` carrying no transfers; tests mutate its
/// bindings, structural bindings, and case metadata to exercise edge-level
/// killers.
fn successor(block: u32) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        psi_edge: EdgeId::new(2).unwrap(),
        block: SelectedBlockId(block),
        source_target: BlockId::new(u64::from(block) + 1).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    }
}

/// The fixture stretched across one edge: block 0 keeps the store and jumps
/// to block 1, which holds the load and returns. Every path to the load
/// passes block 0's tail, so the store still decides on all of them.
fn chained(target: NativeTarget) -> ValidatedStoredLoadForwarding {
    mutated(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let tail_instructions = function.blocks[0].instructions.split_off(3);
        let tail_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(6),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(1),
            },
        );
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: tail_instructions,
            terminator: tail_terminator,
        });
    })
}

/// Edit the chained fixture's function, then refresh the receipt identities
/// so the mutated plan is a well-formed analysis source.
fn mutated_chained(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedStoredLoadForwarding {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = chained(target);
    edit(
        &mut std::sync::Arc::make_mut(&mut source.transformed).functions[0],
        &environment,
    );
    let identity = selected_instruction_plan_identity(&source.transformed);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// The edge from block 0 into the load's block.
fn crossed_edge(function: &mut SelectedFunction) -> &mut SelectedSuccessor {
    let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    successor
}

#[test]
fn cross_block_store_forwards_through_a_unique_predecessor() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = chained(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let source_load = source.transformed().functions[0].blocks[1].instructions[0].clone();
        let result = forward(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        let rewritten = &function.blocks[1].instructions[0];
        assert_eq!(rewritten.id, LOAD);
        assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VALUE);
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.provenance, source_load.provenance);
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| access.role)
                .collect::<Vec<_>>(),
            vec![SelectedMemoryAccessRole::WritePlace]
        );
        // Replay restores the complete source by content.
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

#[test]
fn cross_block_walk_crosses_every_intermediate_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Store in block 0, the between-copy alone in a middle bridge block 2,
    // the load in block 1: the walk crosses two edges and one full body.
    let three = mutated_chained(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let mid_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(2),
            },
        );
        let middle = function.blocks[0].instructions.remove(2);
        function.blocks.insert(
            1,
            SelectedBlock {
                id: SelectedBlockId(2),
                origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
                instructions: vec![middle],
                terminator: mid_terminator,
            },
        );
    });
    forward(&three, &environment).unwrap();
    // A conditional predecessor still decides on every path to the load when
    // it is the only predecessor block.
    let branched = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &function.blocks[0].terminator
        else {
            unreachable!()
        };
        let edge = edge.clone();
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: edge,
            when_zero: successor(2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::ReturnUnit,
                    return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    forward(&branched, &environment).unwrap();
}

#[test]
fn cross_block_joins_unreachable_and_entry_blocks_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second predecessor block admits a path that never passed the store.
    let join = mutated_chained(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(1),
            },
        });
    });
    assert_eq!(
        forward(&join, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // With no predecessor the load's block is unreachable and no pair forms.
    let detached = mutated_chained(target, |function, environment| {
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        let exit = SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::ReturnUnit,
                    return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(4).unwrap(),
            },
        };
        let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
            unreachable!()
        };
        *successor = self::successor(2);
        function.blocks.push(exit);
    });
    assert_eq!(
        forward(&detached, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // A self-loop on the load's block makes the block its own predecessor.
    let looped = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: successor(1),
            when_zero: successor(2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::ReturnUnit,
                    return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(4).unwrap(),
            },
        });
    });
    assert_eq!(
        forward(&looped, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // Removing the store walks to the entry block's top, where the implicit
    // entry path keeps the pair unproven.
    let storeless = mutated_chained(target, |function, _| {
        function.blocks[0].instructions.remove(1);
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        forward(&storeless, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
}

#[test]
fn cross_block_edge_transports_and_terminator_rows_decide() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // An edge register parameter redefining the carried value cannot forward.
    let value_binding = |parameter: VirtualRegisterId| SelectedValueBinding {
        semantic: abstract_operations::ValueBinding {
            parameter: ValueId::new(5).unwrap(),
            argument: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        },
        transport: SelectedValueTransport::Registers {
            argument: SCRATCH,
            parameter,
        },
    };
    let carried = mutated_chained(target, |function, _| {
        crossed_edge(function).bindings.push(value_binding(VALUE));
    });
    assert_eq!(
        forward(&carried, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // The same transport writing the load's output register is equally fatal.
    let result_register = mutated_chained(target, |function, _| {
        crossed_edge(function).bindings.push(value_binding(OUTPUT));
    });
    assert_eq!(
        forward(&result_register, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // An unrelated edge parameter does not touch the carried registers.
    let unrelated = mutated_chained(target, |function, _| {
        crossed_edge(function).bindings.push(value_binding(POINTER));
    });
    forward(&unrelated, &environment).unwrap();
    // A case-payload register parameter redefining the carried value rejects.
    let payload = mutated_chained(target, |function, _| {
        crossed_edge(function).structural_case = Some(SelectedStructuralCaseEdge {
            slot: LocalStorageSlotId::Structural {
                operation: OperationId::new(9).unwrap(),
                place: PlaceId::new(2).unwrap(),
            },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: vec![SelectedCasePayloadBinding {
                semantic: legalized_operations::LegalizedStructuralCasePayload {
                    field: StructuralFieldId::new(1).unwrap(),
                    field_byte_offset: 0,
                    parameter: legalized_operations::LegalizedValueDefinition {
                        value: ValueId::new(5).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        ),
                        definition_site: ValueDefinitionSite::BlockParameter {
                            block: BlockId::new(2).unwrap(),
                            position: 0,
                        },
                    },
                },
                transport: SelectedCasePayloadTransport::Registers {
                    argument: SCRATCH,
                    parameter: VALUE,
                },
            }],
            trivial_affine_discards: Vec::new(),
        });
    });
    assert_eq!(
        forward(&payload, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // A structural write into the forwarded place's storage aliases.
    let aliased = mutated_chained(target, |function, _| {
        crossed_edge(function)
            .structural_bindings
            .push(SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(2).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(2).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: SCRATCH,
                    destination: LocalStorageSlotId::Structural {
                        operation: OperationId::new(9).unwrap(),
                        place: place(),
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    assert_eq!(
        forward(&aliased, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // The same transport into a different place's slot stays harmless.
    let disjoint = mutated_chained(target, |function, _| {
        crossed_edge(function)
            .structural_bindings
            .push(SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(2).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(2).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: SCRATCH,
                    destination: LocalStorageSlotId::Structural {
                        operation: OperationId::new(9).unwrap(),
                        place: PlaceId::new(2).unwrap(),
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    forward(&disjoint, &environment).unwrap();
    // A trivially discarded case binding on the forwarded place is a write.
    let discarded = mutated_chained(target, |function, _| {
        crossed_edge(function).structural_case = Some(SelectedStructuralCaseEdge {
            slot: LocalStorageSlotId::Structural {
                operation: OperationId::new(9).unwrap(),
                place: PlaceId::new(2).unwrap(),
            },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: Vec::new(),
            trivial_affine_discards: vec![place()],
        });
    });
    assert_eq!(
        forward(&discarded, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A roster row on the crossed terminator's instruction decides first.
    let terminator_write = mutated_chained(target, |function, _| {
        function.memory_accesses.insert(
            1,
            access(
                SelectedInstructionId(6),
                3,
                place(),
                0,
                SelectedMemoryAccessRole::WritePlace,
            ),
        );
    });
    assert_eq!(
        forward(&terminator_write, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

#[test]
fn cross_block_predecessor_body_killers_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Redefining the carried value in the predecessor body must not forward.
    let redefined = mutated_chained(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, VALUE],
        );
    });
    assert_eq!(
        forward(&redefined, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // An unaccounted referent write in the predecessor body rejects.
    let unaccounted = mutated_chained(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
    });
    assert_eq!(
        forward(&unaccounted, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A partial overwrite in a crossed middle block is a killer, not a
    // forwarding source.
    let middle = mutated_chained(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let mid_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(2),
            },
        );
        function.blocks.insert(
            1,
            SelectedBlock {
                id: SelectedBlockId(2),
                origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
                instructions: vec![instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::Store {
                        byte_offset: 4,
                        byte_size: 4,
                    },
                    store,
                    &[POINTER, VALUE],
                )],
                terminator: mid_terminator,
            },
        );
        function.memory_accesses.push(SelectedMemoryAccess {
            byte_count: 4,
            ..access(
                SelectedInstructionId(10),
                3,
                place(),
                4,
                SelectedMemoryAccessRole::WritePlace,
            )
        });
    });
    assert_eq!(
        forward(&middle, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

#[test]
fn cross_block_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chained(target);
    let result = forward(&source, &environment).unwrap();
    // Forwarded from the wrong register.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[1].instructions[0].operands[0].virtual_register = POINTER;
    assert_eq!(
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), proposed)
            .unwrap_err(),
        StoredLoadForwardingError::ReplayMismatch
    );
    // Any structural drift beyond the forwarding itself mismatches.
    let mut proposed = result.transformed().clone();
    let jump = environment
        .constraint(environment.selected_keys().jump)
        .unwrap();
    proposed.functions[0].blocks.push(SelectedBlock {
        id: SelectedBlockId(2),
        origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
        instructions: Vec::new(),
        terminator: SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(7),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            successor: successor(1),
        },
    });
    assert_eq!(
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), proposed)
            .unwrap_err(),
        StoredLoadForwardingError::ReplayMismatch
    );
}
