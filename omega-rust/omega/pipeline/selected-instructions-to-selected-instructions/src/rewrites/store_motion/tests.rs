use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedCasePayloadBinding,
    SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand,
    SelectedStructuralBinding, SelectedStructuralCaseEdge, SelectedStructuralTransport,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding,
    SelectedValueTransport, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId,
    OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::{
    StoreMutationMotionError, StoreMutationMotionReceipt, ValidatedStoreMutationMotion,
    sink_selected_store_mutation, validate_store_mutation_motion,
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
    settlement_at(SelectedBlockId(0), position)
}

fn settlement_at(block: SelectedBlockId, position: u32) -> SelectedBoundarySettlement {
    SelectedBoundarySettlement {
        block,
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
/// return`. The first store slides forward to just before the second, which
/// is the first access on its place.
fn fixture(target: NativeTarget) -> ValidatedStoreMutationMotion {
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
    ValidatedStoreMutationMotion {
        receipt: StoreMutationMotionReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

#[test]
fn same_block_store_sinks_to_the_next_place_access() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = fixture(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let result =
            sink_selected_store_mutation(&source, 0, STORE, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        let instructions = &function.blocks[0].instructions;
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
        );
        // The moved store keeps its identity, kind, operands, and provenance.
        assert_eq!(
            instructions[2].kind,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            }
        );
        assert_eq!(
            instructions[2],
            source.transformed().functions[0].blocks[0].instructions[1]
        );
        // Roster rows name instructions by identity: the roster is retained
        // unchanged even though the write sits later in the block.
        assert_eq!(
            function.memory_accesses,
            source.transformed().functions[0].memory_accesses
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
        validate_store_mutation_motion(
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
        validate_store_mutation_motion(&source, 0, STORE, &environment, budget(), detached)
            .unwrap();
        // The store already sits at its latest position; replaying the motion
        // admits nothing.
        let replayed = crate::OwnedSelectedProgram::retain(&result);
        assert_eq!(
            sink_selected_store_mutation(&replayed, 0, STORE, &environment, budget()).unwrap_err(),
            StoreMutationMotionError::UnsupportedPair
        );
    }
}

#[test]
fn replay_rejects_anything_but_the_exact_motion() {
    let target = NativeTarget::linux_x64();
    let source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let result = sink_selected_store_mutation(&source, 0, STORE, &environment, budget()).unwrap();
    for mutation in 0..10 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The store must sit at the admitted position, not its old one.
            0 => {
                let moved = function.blocks[0].instructions.remove(2);
                function.blocks[0].instructions.insert(1, moved);
            }
            // The moved store must not disappear.
            1 => {
                function.blocks[0].instructions.remove(2);
            }
            // A different instruction must not move instead.
            2 => {
                let moved = function.blocks[0].instructions.remove(1);
                function.blocks[0].instructions.insert(2, moved);
            }
            // The moved write row must stay with the store.
            3 => {
                function
                    .memory_accesses
                    .retain(|access| access.instruction != STORE);
            }
            // A phantom row must not appear.
            4 => {
                function.memory_accesses.push(access(
                    BETWEEN,
                    3,
                    place(),
                    8,
                    SelectedMemoryAccessRole::ReadPlace,
                ));
            }
            // The surviving covering store must remain untouched.
            5 => {
                function.blocks[0].instructions[3].kind = SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                };
            }
            // A different instruction id on the moved store.
            6 => function.blocks[0].instructions[2].id = BETWEEN,
            // Fresh provenance must stay the store's.
            7 => {
                function.blocks[0].instructions[2]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // An unrelated register must stay identical.
            8 => function.virtual_registers[1].scalar_type = ScalarType::Boolean,
            // A settlement row must not appear from nowhere.
            9 => function.boundary_settlements.push(settlement(3)),
            _ => unreachable!(),
        }
        assert!(
            validate_store_mutation_motion(&source, 0, STORE, &environment, budget(), proposed)
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
) -> ValidatedStoreMutationMotion {
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

fn sink(
    source: &ValidatedStoreMutationMotion,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedStoreMutationMotion, StoreMutationMotionError> {
    sink_selected_store_mutation(source, 0, STORE, environment, budget())
}

fn landed_ids(result: &ValidatedStoreMutationMotion) -> Vec<SelectedInstructionId> {
    result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect()
}

#[test]
fn observing_accesses_land_the_store_just_before_them() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A read of the moved range two positions later still observes the write:
    // the store lands immediately before it.
    let read = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[0].instructions.insert(
            3,
            instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                load,
                &[POINTER, SCRATCH],
            ),
        );
        function.memory_accesses.insert(
            2,
            access(
                SelectedInstructionId(6),
                3,
                place(),
                0,
                SelectedMemoryAccessRole::ReadPlace,
            ),
        );
    });
    let result = sink(&read, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            BETWEEN,
            STORE,
            SelectedInstructionId(6),
            KILLER
        ]
    );
    // A disjoint range of the same place cannot observe the moved bytes, so
    // the store slides past it to the covering store.
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
    let result = sink(&disjoint, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // A read immediately after the store leaves no later position.
    let immediate = mutated(target, |function, environment| {
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
        sink(&immediate, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A dynamic-extent write to the same place cannot be proven disjoint, so
    // it bounds the motion just as an exact row does.
    let dynamic = mutated(target, |function, _| {
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
        sink(&dynamic, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A place-backed local slot write targets the moved place's storage.
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
        sink(&local, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}

#[test]
fn carried_register_definitions_bound_the_motion() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Redefining the value register immediately after the store leaves no
    // later position.
    let value_def = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[SCRATCH, VALUE],
        );
    });
    assert_eq!(
        sink(&value_def, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // The same redefinition one instruction later lets the store slide once.
    let one_step = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, SCRATCH],
        );
        function.blocks[0].instructions.insert(
            3,
            instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::CopyI64,
                copy,
                &[SCRATCH, POINTER],
            ),
        );
    });
    let result = sink(&one_step, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            BETWEEN,
            STORE,
            SelectedInstructionId(6),
            KILLER
        ]
    );
}

#[test]
fn calls_unaccounted_and_settlement_positions_bound_the_motion() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A call observes through routes the roster cannot see.
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
        sink(&call, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // An unaccounted referent write (no row) bounds the window.
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
        sink(&unaccounted, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A settlement immediately after the store would observe the place before
    // the moved write, so nothing later is reachable.
    let inside = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(2));
    });
    assert_eq!(
        sink(&inside, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A settlement before the covering store keeps its position while the
    // store slides ahead of it.
    let boundary = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(3));
    });
    let result = sink(&boundary, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    assert_eq!(
        result.transformed().functions[0]
            .boundary_settlements
            .iter()
            .map(|settlement| settlement.instruction_index)
            .collect::<Vec<_>>(),
        vec![3]
    );
    // A settlement after the body also keeps its position.
    let tail = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(4));
    });
    let result = sink(&tail, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0]
            .boundary_settlements
            .iter()
            .map(|settlement| settlement.instruction_index)
            .collect::<Vec<_>>(),
        vec![4]
    );
}

#[test]
fn sub_width_stores_slide_within_the_same_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let narrow = mutated(target, |function, _| {
        function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store {
            byte_offset: 4,
            byte_size: 4,
        };
        function.memory_accesses[0].byte_offset = 4;
        function.memory_accesses[0].byte_count = 4;
    });
    let result = sink(&narrow, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
}

#[test]
fn admission_boundaries_and_budget_hold() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A nonexistent instruction id cannot move.
    assert_eq!(
        sink_selected_store_mutation(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        StoreMutationMotionError::SourceMismatch
    );
    // A non-store instruction id cannot move.
    assert_eq!(
        sink_selected_store_mutation(&source, 0, BETWEEN, &environment, budget()).unwrap_err(),
        StoreMutationMotionError::UnsupportedInstruction
    );
    // The wrong target's environment is a different source.
    let foreign = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        sink(&source, &foreign).unwrap_err(),
        StoreMutationMotionError::SourceMismatch
    );
    // The store's write row is required; a private store has nothing to prove.
    let rowless = mutated(target, |function, _| {
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        sink(&rowless, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedInstruction
    );
    // A read row on the store instruction is not a write.
    let mislabeled = mutated(target, |function, _| {
        function.memory_accesses[0].role = SelectedMemoryAccessRole::ReadPlace;
    });
    assert_eq!(
        sink(&mislabeled, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A second row on the store is not the exact write surface.
    let extra = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            access(STORE, 9, place(), 0, SelectedMemoryAccessRole::WritePlace),
        );
    });
    assert_eq!(
        sink(&extra, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A row offset that does not match the instruction's encoding rejects.
    let shifted_row = mutated(target, |function, _| {
        function.memory_accesses[0].byte_offset = 8;
    });
    assert_eq!(
        sink(&shifted_row, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A store width outside the produced grammar is unsupported.
    let exotic = mutated(target, |function, _| {
        function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 3,
        };
        function.memory_accesses[0].byte_count = 3;
    });
    assert_eq!(
        sink(&exotic, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedInstruction
    );
    // A different constraint row does not give the plain [use, use] surface.
    let wrong_row = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[1].constraint = copy.key;
    });
    assert_eq!(
        sink(&wrong_row, &environment).unwrap_err(),
        StoreMutationMotionError::ConstraintMismatch
    );
    let tiny = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        sink_selected_store_mutation(&source, 0, STORE, &environment, tiny).unwrap_err(),
        StoreMutationMotionError::WorkBudgetExceeded
    );
}

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

/// The fixture stretched across one edge: block 0 keeps the moved store and
/// jumps to block 1, which opens with the covering store and returns. Block 0
/// is block 1's only predecessor and every edge out of block 0 reaches it, so
/// the write lands at block 1's head still before the covering store.
fn chained(target: NativeTarget) -> ValidatedStoreMutationMotion {
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
) -> ValidatedStoreMutationMotion {
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

/// The edge from block 0 into the covering store's block.
fn crossed_edge(function: &mut SelectedFunction) -> &mut SelectedSuccessor {
    let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    successor
}

#[test]
fn cross_block_store_sinks_across_the_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = chained(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let result =
            sink_selected_store_mutation(&source, 0, STORE, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        // The store lands at the head of the successor, before the covering
        // store that bounds the window.
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![STORE, KILLER]
        );
        assert_eq!(
            function.memory_accesses,
            source.transformed().functions[0].memory_accesses
        );
        validate_store_mutation_motion(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

#[test]
fn cross_block_walk_crosses_converging_legs_and_intermediate_blocks() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A conditional whose legs both reach the covering block still carries
    // the write on every path forward.
    let converged = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
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
            when_zero: successor(1),
        };
    });
    let result = sink(&converged, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
    // The store slides through an empty bridge block into the covering one.
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
        function.blocks.insert(
            1,
            SelectedBlock {
                id: SelectedBlockId(2),
                origin: SelectedBlockOrigin::EdgeTransfer {
                    edge: EdgeId::new(2).unwrap(),
                    target: BlockId::new(2).unwrap(),
                },
                instructions: Vec::new(),
                terminator: mid_terminator,
            },
        );
    });
    let result = sink(&three, &environment).unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN]
    );
    assert!(function.blocks[1].instructions.is_empty());
    assert_eq!(
        function.blocks[2]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
}

#[test]
fn cross_block_joins_forks_and_cycles_land_at_the_block_end() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second predecessor into the covering block means paths that never
    // carried the write would gain it, so the store only reaches block 0's
    // end.
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
    let result = sink(&join, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // Edges fanning out to distinct blocks admit a path the moved write never
    // runs on; the store lands at the forked block's end.
    let forked = mutated_chained(target, |function, environment| {
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
    let result = sink(&forked, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A successor that can reach back to the walked chain closes a cycle
    // whose unverified interval could reorder an access across the moved
    // write; the store lands at the last proven block's end instead.
    let cycled = mutated_chained(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        function.blocks[1].instructions.clear();
        function.memory_accesses.remove(1);
        function.blocks[1].terminator = SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            successor: successor(0),
        };
    });
    let result = sink(&cycled, &environment).unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    assert!(function.blocks[1].instructions.is_empty());
    // A successor naming no block cannot be walked.
    let dangling = mutated_chained(target, |function, _| {
        crossed_edge(function).block = SelectedBlockId(99);
    });
    assert_eq!(
        sink(&dangling, &environment).unwrap_err(),
        StoreMutationMotionError::SourceMismatch
    );
}

#[test]
fn cross_block_edge_transports_and_terminator_rows_decide() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Register transports cannot reach memory; carrying an unrelated register
    // across the edge still sinks.
    let carried = mutated_chained(target, |function, _| {
        crossed_edge(function).bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(5).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument: SCRATCH,
                parameter: VirtualRegisterId(9),
            },
        });
    });
    let result = sink(&carried, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
    // An edge that redefines the carried value register changes what the
    // moved store would read; the store lands at the crossed block's end.
    let redefined = mutated_chained(target, |function, _| {
        crossed_edge(function).bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(5).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument: SCRATCH,
                parameter: VALUE,
            },
        });
    });
    let result = sink(&redefined, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A structural write into the moved place's storage runs between the old
    // and new positions.
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
    let result = sink(&aliased, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // Case custody on the moved place writes its slot inside the interval.
    let custody = mutated_chained(target, |function, _| {
        crossed_edge(function).structural_case = Some(SelectedStructuralCaseEdge {
            slot: LocalStorageSlotId::Structural {
                operation: OperationId::new(9).unwrap(),
                place: place(),
            },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: Vec::new(),
            trivial_affine_discards: Vec::new(),
        });
    });
    let result = sink(&custody, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A case payload's register parameter redefining the pointer register
    // stops the crossing the same way an edge binding does.
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
                    parameter: POINTER,
                },
            }],
            trivial_affine_discards: Vec::new(),
        });
    });
    let result = sink(&payload, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A roster row on the crossed terminator's instruction lands the store at
    // the block's end before the terminator runs.
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
    let result = sink(&terminator_write, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
}

#[test]
fn cross_block_settlements_shift_over_the_inserted_ordinal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A settlement at the covering block's head sits inside the landing slot;
    // inserting the store there shifts it one ordinal later so it still names
    // the covering store.
    let head = mutated_chained(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement_at(SelectedBlockId(1), 0));
    });
    let result = sink(&head, &environment).unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
    assert_eq!(
        function
            .boundary_settlements
            .iter()
            .map(|settlement| (settlement.block, settlement.instruction_index))
            .collect::<Vec<_>>(),
        vec![(SelectedBlockId(1), 1)]
    );
    // A settlement inside block 0's remaining interval cannot be passed: the
    // boundary event must stay ordered after the write, so the walk stops
    // before it — here that leaves the store nowhere to move.
    let inside = mutated_chained(target, |function, _| {
        function.boundary_settlements.push(settlement(2));
    });
    assert_eq!(
        sink(&inside, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A settlement in the after-body slot of the crossed block bounds the
    // motion at the block's end and keeps its own position.
    let tail = mutated_chained(target, |function, _| {
        function.boundary_settlements.push(settlement(3));
    });
    let result = sink(&tail, &environment).unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    assert_eq!(
        function
            .boundary_settlements
            .iter()
            .map(|settlement| (settlement.block, settlement.instruction_index))
            .collect::<Vec<_>>(),
        vec![(SelectedBlockId(0), 3)]
    );
}
