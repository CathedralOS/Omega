use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand,
    SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId,
    OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::{
    DeadStoreEliminationError, DeadStoreEliminationReceipt, ValidatedDeadStoreElimination,
    eliminate_selected_dead_store, validate_dead_store_elimination,
};
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
const KILLER: SelectedInstructionId = SelectedInstructionId(4);
const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const VALUE: VirtualRegisterId = VirtualRegisterId(1);
const SCRATCH: VirtualRegisterId = VirtualRegisterId(2);

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

fn settlement(position: u32) -> SelectedBoundarySettlement {
    SelectedBoundarySettlement {
        block: SelectedBlockId(0),
        instruction_index: position,
        settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
            operation: OperationId::new(7).unwrap(),
            boundary: BoundaryMachineId::new(1).unwrap(),
            source: ValueId::new(9).unwrap(),
        },
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r1 = copy r0; store [r0+0] <- r1; r2 = copy r0; store [r0+0] <- r2;
/// return`. The first store's bytes are overwritten unobserved by the second.
fn fixture(target: NativeTarget) -> ValidatedDeadStoreElimination {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let store = environment.constraint(keys.store.unwrap()).unwrap();
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
            let mut killer = instruction(
                KILLER,
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                store,
                &[POINTER, SCRATCH],
            );
            killer.provenance.operations = vec![OperationId::new(2).unwrap()];
            killer.provenance.values = vec![ValueId::new(4).unwrap()];
            killer
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
                access(KILLER, 2, place, 0, SelectedMemoryAccessRole::WritePlace),
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
    ValidatedDeadStoreElimination {
        receipt: DeadStoreEliminationReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

#[test]
fn same_block_covering_store_eliminates_and_drops_the_write_row() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = fixture(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let result =
            eliminate_selected_dead_store(&source, 0, STORE, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        let instructions = &function.blocks[0].instructions;
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, KILLER]
        );
        // The covering store keeps its identity, operands, and provenance.
        assert_eq!(
            instructions[2].kind,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            }
        );
        assert_eq!(
            instructions[2].provenance,
            source.transformed().functions[0].blocks[0].instructions[3].provenance
        );
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WritePlace)]
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
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), detached)
            .unwrap();
    }
}

#[test]
fn replay_rejects_anything_but_the_exact_elimination() {
    let target = NativeTarget::linux_x64();
    let source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let result = eliminate_selected_dead_store(&source, 0, STORE, &environment, budget()).unwrap();
    for mutation in 0..9 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The dead store must be gone, not retained.
            0 => {
                function.blocks[0].instructions.insert(
                    1,
                    source.transformed().functions[0].blocks[0].instructions[1].clone(),
                );
            }
            // A different instruction must not disappear instead.
            1 => {
                function.blocks[0].instructions.remove(1);
            }
            // Kept the dead write row instead of dropping it.
            2 => {
                function.memory_accesses.push(access(
                    STORE,
                    1,
                    place(),
                    0,
                    SelectedMemoryAccessRole::WritePlace,
                ));
            }
            // Dropped the covering store's row as well.
            3 => {
                function.memory_accesses.clear();
            }
            // The surviving store must remain untouched.
            4 => {
                function.blocks[0].instructions[2].kind = SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                };
            }
            // A different instruction id on the killer.
            5 => function.blocks[0].instructions[2].id = BETWEEN,
            // Fresh provenance must stay the killer's.
            6 => {
                function.blocks[0].instructions[2]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // An unrelated register must stay identical.
            7 => function.virtual_registers[1].scalar_type = ScalarType::Boolean,
            // A settlement row must not appear from nowhere.
            8 => function.boundary_settlements.push(settlement(3)),
            _ => unreachable!(),
        }
        assert!(
            validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), proposed)
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
) -> ValidatedDeadStoreElimination {
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

fn eliminate(
    source: &ValidatedDeadStoreElimination,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedDeadStoreElimination, DeadStoreEliminationError> {
    eliminate_selected_dead_store(source, 0, STORE, environment, budget())
}

#[test]
fn observing_or_partial_accesses_between_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A read of the dead range observes the stored bytes.
    let read = mutated(target, |function, environment| {
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
    assert_eq!(
        eliminate(&read, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A dynamic-extent byte-sequence read of the same place cannot be proven
    // to stay out of the dead range.
    let dynamic_read = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    0,
                    SelectedMemoryAccessRole::ReadByteSequence {
                        index: ValueId::new(5).unwrap(),
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    assert_eq!(
        eliminate(&dynamic_read, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A partial overwrite leaves the remaining dead bytes observable.
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
        eliminate(&partial, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A dynamic-extent write to the same place cannot be proven to cover.
    let dynamic_write = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    0,
                    SelectedMemoryAccessRole::WriteByteSequence {
                        index: ValueId::new(5).unwrap(),
                        value: ValueId::new(6).unwrap(),
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    assert_eq!(
        eliminate(&dynamic_write, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A place-backed local slot write targets the dead place's storage.
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
        eliminate(&local, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // Materializing the place-backed local address lets later accesses reach
    // it by a route the roster cannot prove disjoint.
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
        eliminate(&address, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
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
        eliminate(&unaccounted, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A covering-range store at a different offset leaves the dead bytes live.
    let elsewhere = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store {
                byte_offset: 8,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses[1].byte_offset = 8;
    });
    assert_eq!(
        eliminate(&elsewhere, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
}

#[test]
fn harmless_accesses_and_private_slots_still_eliminate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A different place root cannot share storage with the dead place under
    // place exclusivity.
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
    eliminate(&other_place, &environment).unwrap();
    // A disjoint range of the same place cannot touch the dead bytes.
    let disjoint = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Load64 { byte_offset: 16 },
            load,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 16, SelectedMemoryAccessRole::ReadPlace),
        );
    });
    eliminate(&disjoint, &environment).unwrap();
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
    eliminate(&spill, &environment).unwrap();
    // A private-slot reload without a row cannot observe referent storage.
    let reload = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load,
            &[POINTER, SCRATCH],
        );
    });
    eliminate(&reload, &environment).unwrap();
}

#[test]
fn calls_hosted_effects_and_settlement_positions_reject() {
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
        eliminate(&call, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedInstruction
    );
    // A settlement positioned before an interval instruction could observe
    // the dead bytes at the boundary.
    let inside = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(3));
    });
    assert_eq!(
        eliminate(&inside, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

#[test]
fn settlements_after_the_covering_store_shift_left() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let shifted = mutated(target, |function, _| {
        // Before the dead store: unchanged. After the covering store: shifts.
        function.boundary_settlements.push(settlement(1));
        function.boundary_settlements.push(settlement(4));
    });
    let result = eliminate(&shifted, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0]
            .boundary_settlements
            .iter()
            .map(|settlement| settlement.instruction_index)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
}

#[test]
fn admission_boundaries_and_budget_hold() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A nonexistent instruction id cannot eliminate.
    assert_eq!(
        eliminate_selected_dead_store(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        DeadStoreEliminationError::SourceMismatch
    );
    // A non-store instruction id cannot eliminate.
    assert_eq!(
        eliminate_selected_dead_store(&source, 0, BETWEEN, &environment, budget()).unwrap_err(),
        DeadStoreEliminationError::UnsupportedInstruction
    );
    // The wrong target's environment is a different source.
    let foreign = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        eliminate(&source, &foreign).unwrap_err(),
        DeadStoreEliminationError::SourceMismatch
    );
    // Without a later same-range store the bytes stay observable.
    let open = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(3);
        function.memory_accesses.remove(1);
    });
    assert_eq!(
        eliminate(&open, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // The store's write row is required; a private store has nothing to prove.
    let rowless = mutated(target, |function, _| {
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        eliminate(&rowless, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedInstruction
    );
    // A read row on the store instruction is not a write.
    let mislabeled = mutated(target, |function, _| {
        function.memory_accesses[0].role = SelectedMemoryAccessRole::ReadPlace;
    });
    assert_eq!(
        eliminate(&mislabeled, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    let tiny = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        eliminate_selected_dead_store(&source, 0, STORE, &environment, tiny).unwrap_err(),
        DeadStoreEliminationError::WorkBudgetExceeded
    );
}
