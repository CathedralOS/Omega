/*
These tests build small x86-64 selected programs by hand and run the runtime
recovery rewrites on them directly. Each positive case first shows that home
assignment refuses the program under register pressure, then that the rewrite
relieves the pressure and assignment succeeds. Each negative twin changes one
input and pins the refusal.
*/

use std::sync::Arc;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_homes::AllocatorAvailabilityPolicy;
use register_model::{RegisterClassId, RegisterConstraintKey, RegisterOperandAccess};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedFunction, SelectedFunctionContracts, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedLocalStorageSlot,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedProgram,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin, selected_instruction_plan_identity,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, WorkScheduleIdentity,
};
use target::NativeTarget;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::runtime_spill::BlockDominance;
use crate::allocation::analyses::{
    analyze_allocation_legality, analyze_live_ranges, analyze_liveness,
    materialize_allocator_availability,
};
use crate::allocation::assignment::runtime_spill::recovery::candidates;
use crate::allocation::{
    RegisterHomeError, RuntimeRematerializationError, RuntimeSpillError, SelectedProgramView,
    assign_register_homes, rematerialize_selected_runtime_value, spill_selected_runtime_value,
};

fn environment() -> ValidatedTargetRegisterEnvironment {
    baseline_target_register_environment(NativeTarget::linux_x64()).unwrap()
}

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1 << 20, 1 << 20, 1 << 30, 1 << 20, 1 << 20).unwrap()
}

fn unsigned(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
}

fn scratch(id: u32, class: RegisterClassId, scalar_type: ScalarType) -> VirtualRegister {
    VirtualRegister {
        id: VirtualRegisterId(id),
        scalar_type,
        class,
        origin: VirtualRegisterOrigin::InstructionScratch {
            instruction: SelectedInstructionId(id),
            operand: 0,
        },
        definition_site: None,
        entry_fixed_view: None,
    }
}

/// Builds instructions whose ids count up from zero, so register `n` can name
/// the `n`th instruction as its definition.
struct Body<'environment> {
    environment: &'environment ValidatedTargetRegisterEnvironment,
    instructions: Vec<SelectedInstruction>,
}

impl Body<'_> {
    fn push(
        &mut self,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        registers: &[u32],
    ) -> SelectedInstructionId {
        let id = SelectedInstructionId(self.instructions.len() as u32);
        let row = self.environment.constraint(key).unwrap();
        let registers: Vec<_> = registers.iter().copied().map(VirtualRegisterId).collect();
        self.instructions
            .push(crate::allocation::rewrites::instruction(
                id, kind, row, &registers,
            ));
        id
    }
}

fn program(
    environment: &ValidatedTargetRegisterEnvironment,
    local_storage_slots: Vec<SelectedLocalStorageSlot>,
    virtual_registers: Vec<VirtualRegister>,
    body: Body<'_>,
    memory_accesses: Vec<SelectedMemoryAccess>,
) -> SelectedProgram {
    let instructions = body.instructions;
    let return_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap();
    let terminator = crate::allocation::rewrites::instruction(
        SelectedInstructionId(instructions.len() as u32),
        SelectedInstructionKind::ReturnUnit,
        return_row,
        &[],
    );
    let function = SelectedFunction {
        contracts: Arc::new(SelectedFunctionContracts {
            structural_block_parameters: Vec::new(),
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots,
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses,
            boundary_settlements: Vec::new(),
            virtual_registers,
        }),
        entry_block: SelectedBlockId(0),
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions,
            terminator: SelectedTerminator::Return {
                instruction: terminator,
                psi_return_edge: EdgeId::new(1).unwrap(),
            },
        }]
        .into(),
    };
    let plan = SelectedInstructionPlan {
        contracts: Default::default(),
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        work_schedule: WorkScheduleIdentity::new(1).unwrap(),
        target: environment.target(),
        entry: MachineId::new(1).unwrap(),
        functions: vec![function].into(),
    };
    SelectedProgram {
        identity: selected_instruction_plan_identity(&plan),
        plan: Arc::new(plan),
        optimization_unit: OptimizationUnitIdentity::from_bytes([3; 32]),
        work_schedule: WorkScheduleIdentity::new(1).unwrap(),
    }
}

/// Analyze the program from scratch and assign homes for every register.
fn assign(
    environment: &ValidatedTargetRegisterEnvironment,
    selected: &impl SelectedProgramView,
) -> Result<(), RegisterHomeError> {
    let keys = environment.allocation_constraint_keys();
    let availability = materialize_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &keys,
        AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
    )
    .unwrap();
    let liveness = analyze_liveness(selected).unwrap();
    let ranges = analyze_live_ranges(selected, &liveness).unwrap();
    let legality = analyze_allocation_legality(
        &ranges,
        &availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &keys,
    )
    .unwrap();
    assign_register_homes(&legality, &ranges, environment).map(|_| ())
}

/// How many registers of `class` home assignment may choose from.
fn homes_in(environment: &ValidatedTargetRegisterEnvironment, class: RegisterClassId) -> usize {
    let physical = environment.physical().model();
    let reserved = environment.reservations().reserved_units();
    let class = physical.classes.iter().find(|row| row.id == class).unwrap();
    class
        .views
        .iter()
        .filter(|view| {
            let view = &physical.views[usize::from(view.0)];
            view.allocatable
                && view
                    .units
                    .iter()
                    .chain(&view.write_units)
                    .all(|unit| reserved.binary_search(unit).is_err())
        })
        .count()
}

fn dominance(selected: &SelectedProgram) -> BlockDominance {
    BlockDominance::of_function(0, &selected.plan().functions[0]).unwrap()
}

fn slot_of(slot: LocalStorageSlotId) -> FrameStorageSlotId {
    FrameStorageSlotId::Local(slot)
}

const VECTORS: u32 = 17;

/// Seventeen packed vectors are all live at once, one more than x86-64 has
/// `xmm` registers. Vector promotion creates exactly this shape: zeroed
/// `VECTOR128` scratch registers stored to a local array afterwards, last
/// vector first, so vector 0 lives longest.
fn seventeen_live_vectors(
    vector_type: ScalarType,
) -> (ValidatedTargetRegisterEnvironment, SelectedProgram) {
    let environment = environment();
    let keys = environment.selected_keys();
    let zero = keys.packed_materialize_zero.unwrap();
    let store = keys.packed_store_128.unwrap();
    let frame_address = keys.frame_address.unwrap();
    let vector_class = environment.constraint(zero).unwrap().operands[0].class;
    let pointer_class = environment.constraint(frame_address).unwrap().operands[0].class;
    assert_eq!(homes_in(&environment, vector_class), 16);
    let array = LocalStorageSlotId::Inlined {
        call_site: 0,
        slot_index: 0,
    };
    let mut body = Body {
        environment: &environment,
        instructions: Vec::new(),
    };
    let mut registers = Vec::new();
    for vector in 0..VECTORS {
        body.push(
            SelectedInstructionKind::PackedMaterializeZero,
            zero,
            &[vector],
        );
        registers.push(scratch(vector, vector_class, vector_type));
    }
    let pointer = VECTORS;
    body.push(
        SelectedInstructionKind::FrameAddress {
            slot: slot_of(array),
            byte_offset: 0,
        },
        frame_address,
        &[pointer],
    );
    registers.push(scratch(pointer, pointer_class, unsigned(64)));
    for vector in (0..VECTORS).rev() {
        body.push(
            SelectedInstructionKind::PackedStore128 {
                byte_offset: 16 * vector,
            },
            store,
            &[pointer, vector],
        );
    }
    let slots = vec![SelectedLocalStorageSlot {
        id: array,
        byte_size: 16 * VECTORS,
        alignment: 16,
    }];
    let selected = program(&environment, slots, registers, body, Vec::new());
    (environment, selected)
}

#[test]
fn seventeen_live_packed_vectors_spill_whole_through_a_sixteen_byte_slot() {
    let (environment, selected) = seventeen_live_vectors(unsigned(128));
    assert!(matches!(
        assign(&environment, &selected),
        Err(RegisterHomeError::NoCompatibleHome { function: 0, .. })
    ));
    let victim = VirtualRegisterId(0);
    assert!(candidates(selected.plan(), 0).contains(&(0, victim)));

    let spilled = spill_selected_runtime_value(
        &selected,
        0,
        victim,
        &environment,
        budget(),
        &dominance(&selected),
    )
    .unwrap();
    let function = &spilled.selected_plan().functions[0];
    let spill_slot = LocalStorageSlotId::Spill { register: victim };
    let home = function
        .local_storage_slots
        .iter()
        .find(|slot| slot.id == spill_slot)
        .unwrap();
    assert_eq!((home.byte_size, home.alignment), (16, 16));

    // The store and the reload each move all 16 bytes through a pointer to the
    // spill slot; no 8-byte move touches the vector.
    let instructions = &function.blocks[0].instructions;
    let address_of = |register: VirtualRegisterId| {
        instructions
            .iter()
            .find(|row| {
                row.operands.first().map(|operand| operand.virtual_register) == Some(register)
                    && matches!(row.kind, SelectedInstructionKind::FrameAddress { .. })
            })
            .map(|row| row.kind)
    };
    let spill_address = Some(SelectedInstructionKind::FrameAddress {
        slot: slot_of(spill_slot),
        byte_offset: 0,
    });
    let store = instructions
        .iter()
        .find(|row| {
            row.kind == SelectedInstructionKind::PackedStore128 { byte_offset: 0 }
                && row.operands[1].virtual_register == victim
        })
        .unwrap();
    assert_eq!(
        address_of(store.operands[0].virtual_register),
        spill_address
    );
    let reload = instructions
        .iter()
        .find(|row| row.kind == SelectedInstructionKind::PackedLoad128 { byte_offset: 0 })
        .unwrap();
    assert_eq!(
        address_of(reload.operands[0].virtual_register),
        spill_address
    );
    let reloaded = function
        .virtual_register(reload.operands[1].virtual_register)
        .unwrap();
    assert_eq!(reloaded.scalar_type, unsigned(128));
    assert_eq!(
        reloaded.origin,
        VirtualRegisterOrigin::InstructionScratch {
            instruction: reload.id,
            operand: 1,
        }
    );
    let last_store = instructions.last().unwrap();
    assert_eq!(
        last_store.kind,
        SelectedInstructionKind::PackedStore128 { byte_offset: 0 }
    );
    assert_eq!(last_store.operands[1].virtual_register, reloaded.id);
    assert!(!instructions.iter().any(|row| matches!(
        row.kind,
        SelectedInstructionKind::Load64 { .. } | SelectedInstructionKind::Store64 { .. }
    )));

    assign(&environment, &spilled).unwrap();
}

#[test]
fn a_vector_register_typed_as_a_word_is_refused_instead_of_spilling_half() {
    // Vector promotion used to type these registers `f64`. Spilling by that type
    // would store only the low 8 bytes, so the spill refuses scratch that does
    // not state a whole 128-bit vector, and recovery never offers it.
    let (environment, selected) =
        seventeen_live_vectors(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64));
    let victim = VirtualRegisterId(0);
    assert!(!candidates(selected.plan(), 0).contains(&(0, victim)));
    assert_eq!(
        spill_selected_runtime_value(
            &selected,
            0,
            victim,
            &environment,
            budget(),
            &dominance(&selected),
        )
        .err(),
        Some(RuntimeSpillError::UnsupportedValue)
    );
}

/// One 128-bit value moves from a word into its vector class and back. Only
/// the target differs between the twins.
fn vector_through_bits(
    target: NativeTarget,
) -> (ValidatedTargetRegisterEnvironment, SelectedProgram) {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let to_vector = keys.bits_to_float64.unwrap();
    let to_word = keys.float64_to_bits.unwrap();
    let word_class = environment.constraint(to_vector).unwrap().operands[0].class;
    let vector_class = environment.constraint(to_vector).unwrap().operands[1].class;
    let mut body = Body {
        environment: &environment,
        instructions: Vec::new(),
    };
    body.push(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(7),
        },
        keys.materialize_i64,
        &[0],
    );
    body.push(SelectedInstructionKind::BitsToFloat64, to_vector, &[0, 1]);
    body.push(SelectedInstructionKind::Float64ToBits, to_word, &[1, 2]);
    let defined_as_result = |register: VirtualRegister| VirtualRegister {
        origin: VirtualRegisterOrigin::InstructionScratch {
            instruction: SelectedInstructionId(register.id.0),
            operand: 1,
        },
        ..register
    };
    let registers = vec![
        scratch(0, word_class, unsigned(64)),
        defined_as_result(scratch(1, vector_class, unsigned(128))),
        defined_as_result(scratch(2, word_class, unsigned(64))),
    ];
    let selected = program(&environment, Vec::new(), registers, body, Vec::new());
    (environment, selected)
}

#[test]
fn a_target_without_sixteen_byte_packed_moves_refuses_to_spill_a_vector() {
    // x86-64 spills the vector through a 16-byte slot. aarch64 has no
    // `PackedLoad128`/`PackedStore128` rows and never promotes vectors; should a
    // 128-bit value reach its vector class anyway, the spill refuses it instead
    // of falling back to an 8-byte word.
    let spill = |target| {
        let (environment, selected) = vector_through_bits(target);
        spill_selected_runtime_value(
            &selected,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
            &dominance(&selected),
        )
    };
    let spilled = spill(NativeTarget::linux_x64()).unwrap();
    assert!(
        spilled.selected_plan().functions[0]
            .local_storage_slots
            .iter()
            .any(|slot| slot.byte_size == 16)
    );
    let arm = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    let keys = arm.selected_keys();
    assert!(keys.packed_load_128.is_none() && keys.packed_store_128.is_none());
    assert_eq!(
        spill(NativeTarget::linux_arm64()).err(),
        Some(RuntimeSpillError::UnsupportedValue)
    );
}

/// A pointer to a local array stays live while every general-purpose register
/// holds another value, and is used once afterwards.
fn pointer_live_across_pressure(
    described: bool,
) -> (ValidatedTargetRegisterEnvironment, SelectedProgram) {
    let environment = environment();
    let keys = environment.selected_keys();
    let frame_address = keys.frame_address.unwrap();
    let materialize = keys.materialize_i64;
    let store = keys.store.unwrap();
    let pointer_class = environment.constraint(frame_address).unwrap().operands[0].class;
    let values = homes_in(&environment, pointer_class) as u32;
    let array = LocalStorageSlotId::Inlined {
        call_site: 0,
        slot_index: 0,
    };
    let mut body = Body {
        environment: &environment,
        instructions: Vec::new(),
    };
    let pointer = body.push(
        SelectedInstructionKind::FrameAddress {
            slot: slot_of(array),
            byte_offset: 0,
        },
        frame_address,
        &[0],
    );
    let mut registers = vec![scratch(0, pointer_class, unsigned(64))];
    for value in 1..=values {
        body.push(
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(value)),
            },
            materialize,
            &[value],
        );
        registers.push(scratch(value, pointer_class, unsigned(64)));
    }
    // Each value is consumed by storing it through itself, so the values die
    // before the pointer's one use and leave a free register behind.
    for value in 1..=values {
        body.push(
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[value, value],
        );
    }
    body.push(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 8,
        },
        store,
        &[0, 0],
    );
    let memory_accesses = if described {
        vec![SelectedMemoryAccess {
            instruction: pointer,
            origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(1).unwrap()),
            place: PlaceId::new(1).unwrap(),
            byte_offset: 0,
            byte_count: 8,
            role: SelectedMemoryAccessRole::AddressLocal { slot: array },
        }]
    } else {
        Vec::new()
    };
    let slots = vec![SelectedLocalStorageSlot {
        id: array,
        byte_size: 8,
        alignment: 8,
    }];
    let selected = program(&environment, slots, registers, body, memory_accesses);
    (environment, selected)
}

#[test]
fn a_frame_address_under_pressure_is_rematerialized_before_its_use_not_spilled() {
    let (environment, selected) = pointer_live_across_pressure(false);
    assert!(matches!(
        assign(&environment, &selected),
        Err(RegisterHomeError::NoCompatibleHome { function: 0, .. })
    ));
    let victim = VirtualRegisterId(0);
    assert!(candidates(selected.plan(), 0).contains(&(0, victim)));

    let rematerialized = rematerialize_selected_runtime_value(
        &selected,
        0,
        victim,
        &environment,
        budget(),
        &dominance(&selected),
    )
    .unwrap();
    let function = &rematerialized.selected_plan().functions[0];
    assert_eq!(
        function.local_storage_slots,
        selected.plan().functions[0].local_storage_slots
    );
    // Each use operand of the final store reads its own copy of the address,
    // defined just before the store by the same `FrameAddress`.
    let instructions = &function.blocks[0].instructions;
    let original = &instructions[0];
    let (last_store, before) = instructions.split_last().unwrap();
    let copies = &before[before.len() - last_store.operands.len()..];
    for (operand, copy) in last_store.operands.iter().zip(copies) {
        assert_eq!(
            (copy.kind, copy.constraint),
            (original.kind, original.constraint)
        );
        assert_eq!(copy.operands[0].access, RegisterOperandAccess::Def);
        let regenerated = copy.operands[0].virtual_register;
        assert_ne!(regenerated, victim);
        assert_eq!(operand.virtual_register, regenerated);
        assert_eq!(
            function.virtual_register(regenerated).unwrap().origin,
            VirtualRegisterOrigin::InstructionScratch {
                instruction: copy.id,
                operand: 0,
            }
        );
    }

    assign(&environment, &rematerialized).unwrap();
}

#[test]
fn a_frame_address_that_a_memory_access_row_names_keeps_its_one_instruction() {
    // The row ties the address to its defining instruction's identity, which a
    // copy would not carry.
    let (environment, selected) = pointer_live_across_pressure(true);
    assert_eq!(
        rematerialize_selected_runtime_value(
            &selected,
            0,
            VirtualRegisterId(0),
            &environment,
            budget(),
            &dominance(&selected),
        )
        .err(),
        Some(RuntimeRematerializationError::UnsupportedValue)
    );
}

/// The dominator tree must answer exactly what "every path from the entry to
/// `block` passes through `anchor`" means, including on a loop and for a block
/// the entry never reaches.
#[test]
fn block_dominance_matches_paths_that_avoid_the_anchor() {
    // 0 -> {1, 2}; 1 -> 3; 2 -> 3; 3 -> {4, 1}; 4 returns; 5 -> 3 is unreachable.
    let edges: [&[u32]; 6] = [&[1, 2], &[3], &[3], &[4, 1], &[], &[3]];
    let environment = environment();
    let return_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap();
    let successor = |block: u32| SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        psi_edge: Some(EdgeId::new(u64::from(block) + 1).unwrap()),
        block: SelectedBlockId(block),
        source_target: BlockId::new(u64::from(block) + 1).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        work: Vec::new(),
    };
    let blocks: Vec<SelectedBlock> = edges
        .iter()
        .enumerate()
        .map(|(index, targets)| {
            let instruction = crate::allocation::rewrites::instruction(
                SelectedInstructionId(index as u32),
                SelectedInstructionKind::ReturnUnit,
                return_row,
                &[],
            );
            let terminator = match **targets {
                [] => SelectedTerminator::Return {
                    instruction,
                    psi_return_edge: EdgeId::new(99).unwrap(),
                },
                [only] => SelectedTerminator::Jump {
                    instruction,
                    successor: successor(only),
                },
                [first, second] => SelectedTerminator::ConditionalBranch {
                    instruction,
                    when_nonzero: successor(first),
                    when_zero: successor(second),
                },
                _ => unreachable!(),
            };
            SelectedBlock {
                id: SelectedBlockId(index as u32),
                origin: SelectedBlockOrigin::Source(BlockId::new(index as u64 + 1).unwrap()),
                instructions: Vec::new(),
                terminator,
            }
        })
        .collect();
    let (_, selected) = pointer_live_across_pressure(false);
    let mut function = selected.plan().functions[0].clone();
    function.blocks = blocks.into();
    let dominance = BlockDominance::of_function(0, &function).unwrap();

    let reaches_avoiding = |avoid: Option<usize>, target: usize| {
        let mut seen = [false; 6];
        let mut pending = vec![0usize];
        while let Some(block) = pending.pop() {
            if Some(block) == avoid || seen[block] {
                continue;
            }
            seen[block] = true;
            pending.extend(edges[block].iter().map(|&next| next as usize));
        }
        seen[target]
    };
    for anchor in 0..edges.len() {
        for block in 0..edges.len() {
            let expected = reaches_avoiding(None, block)
                && (anchor == block || !reaches_avoiding(Some(anchor), block));
            assert_eq!(
                dominance.dominates(anchor, block),
                expected,
                "{anchor} dominates {block}"
            );
        }
    }
}
