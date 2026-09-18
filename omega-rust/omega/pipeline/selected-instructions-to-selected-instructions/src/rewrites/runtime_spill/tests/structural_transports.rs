//! Stored structural transports — `Descriptor` and `WholeValue` snapshot
//! arguments on an edge-transfer continuation — name the register holding the
//! source place's address. The bridge's own chunk loads are the real uses of
//! the victim; the binding's `argument` field must follow the single register
//! those loads name after rewriting.
//!
//! The fixtures reuse `control_flow::cfg_fixture`: block 1 is already the
//! `EdgeTransfer { edge: 2, target: 3 }` bridge whose `Jump` successor is the
//! edge's continuation. Adding the snapshot's loads and stores plus the
//! continuation's structural binding produces the exact idiom edge-transfer
//! selection emits.

use super::control_flow::cfg_fixture;
use super::{
    Arc, EdgeId, NativeTarget, SelectedInstructionId, SelectedInstructionKind, SelectedTerminator,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    baseline_target_register_environment, selected_instruction_plan_identity,
};
use crate::RuntimeSpillError;
use crate::ValidatedRuntimeSpill;
use crate::rewrites::runtime_spill::admission;
use crate::rewrites::runtime_spill::tests::budget;
use crate::spill_selected_runtime_value;
use crate::validate_runtime_spill;
use register_model::RegisterUnitId;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedStructuralBinding,
    SelectedStructuralTransport, SelectedSuccessorRole,
};
use semantic_vocabulary::{OperationId, PlaceId};
use terminal_psi::StructuralAccess;

fn targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ]
}

const ARGUMENT_PLACE: u64 = 10;
const PARAMETER_PLACE: u64 = 11;

fn argument_place() -> PlaceId {
    PlaceId::new(ARGUMENT_PLACE).unwrap()
}

fn destination() -> (LocalStorageSlotId, PlaceId) {
    let place = PlaceId::new(PARAMETER_PLACE).unwrap();
    (
        LocalStorageSlotId::Structural {
            operation: OperationId::new(1).unwrap(),
            place,
        },
        place,
    )
}

/// Append the snapshot loads to the bridge (block 1), each reading the victim
/// register 1 through `Load{width}` at the transport's chunk offsets, with the
/// matching `ReadPlace` memory accesses. Returns the chunk instruction ids.
fn snapshot_loads(
    source: &mut ValidatedRuntimeSpill,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    chunks: &[(u32, u8)],
) -> Vec<SelectedInstructionId> {
    let keys = environment.selected_keys();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let scalar_type = function.virtual_registers[1].scalar_type;
    let class = function.virtual_registers[1].class;
    let mut ids = Vec::new();
    for (ordinal, (byte_offset, width)) in chunks.iter().enumerate() {
        let instruction = SelectedInstructionId(10 + ordinal as u32);
        let output = VirtualRegisterId(5 + ordinal as u32);
        function.virtual_registers.push(VirtualRegister {
            id: output,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::AbiTransport {
                instruction,
                place: argument_place(),
                byte_offset: *byte_offset,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        let (kind, key) = match width {
            8 => (
                SelectedInstructionKind::Load64 {
                    byte_offset: *byte_offset,
                },
                keys.load64,
            ),
            4 => (
                SelectedInstructionKind::Load32 {
                    byte_offset: *byte_offset,
                },
                keys.load32,
            ),
            2 => (
                SelectedInstructionKind::Load16 {
                    byte_offset: *byte_offset,
                },
                keys.load16,
            ),
            _ => (
                SelectedInstructionKind::Load8 {
                    byte_offset: *byte_offset,
                },
                keys.load8,
            ),
        };
        function.blocks[1].instructions.push(admission::instruction(
            instruction,
            kind,
            environment.constraint(key.unwrap()).unwrap(),
            &[VirtualRegisterId(1), output],
        ));
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction,
            origin: SelectedMemoryAccessOrigin::Edge(EdgeId::new(2).unwrap()),
            place: argument_place(),
            byte_offset: *byte_offset,
            byte_count: u32::from(*width),
            role: SelectedMemoryAccessRole::ReadPlace,
        });
        ids.push(instruction);
    }
    ids
}

/// Append the destination replacement stores (and the whole-value pointer
/// address) after the snapshot loads, with their memory accesses.
fn destination_stores(
    source: &mut ValidatedRuntimeSpill,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    chunks: &[(u32, u8)],
    whole: bool,
) {
    let keys = environment.selected_keys();
    let (destination, parameter_place) = destination();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let byte_size: u32 = chunks.iter().map(|(_, width)| u32::from(*width)).sum();
    function.local_storage_slots.push(SelectedLocalStorageSlot {
        id: destination,
        byte_size,
        alignment: 8,
    });
    let emit = |function: &mut selected_instructions::SelectedFunction,
                instruction,
                kind,
                row,
                registers: &[VirtualRegisterId],
                place,
                byte_offset,
                byte_count,
                role| {
        function.blocks[1].instructions.push(admission::instruction(
            instruction,
            kind,
            environment.constraint(row).unwrap(),
            registers,
        ));
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction,
            origin: SelectedMemoryAccessOrigin::Edge(EdgeId::new(2).unwrap()),
            place,
            byte_offset,
            byte_count,
            role,
        });
    };
    let pointer = if whole {
        let instruction = SelectedInstructionId(30);
        let output = VirtualRegisterId(30);
        let source_type = function.virtual_registers[1].scalar_type;
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(VirtualRegister {
            id: output,
            scalar_type: source_type,
            class,
            origin: VirtualRegisterOrigin::AbiTransport {
                instruction,
                place: parameter_place,
                byte_offset: 0,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        emit(
            function,
            instruction,
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(destination),
                byte_offset: 0,
            },
            keys.frame_address.unwrap(),
            &[output],
            parameter_place,
            0,
            byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot: destination },
        );
        Some(output)
    } else {
        None
    };
    for (ordinal, (byte_offset, width)) in chunks.iter().enumerate() {
        let word = VirtualRegisterId(5 + ordinal as u32);
        let instruction = SelectedInstructionId(31 + ordinal as u32);
        let (kind, key, registers) = if let Some(pointer) = pointer {
            (
                SelectedInstructionKind::Store {
                    byte_offset: *byte_offset,
                    byte_size: *width,
                },
                keys.store.unwrap(),
                vec![pointer, word],
            )
        } else {
            (
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(destination),
                    byte_offset: *byte_offset,
                },
                keys.store64.unwrap(),
                vec![word],
            )
        };
        emit(
            function,
            instruction,
            kind,
            key,
            &registers,
            parameter_place,
            *byte_offset,
            u32::from(*width),
            SelectedMemoryAccessRole::WriteLocal { slot: destination },
        );
    }
}

/// Attach the continuation's structural binding to the bridge terminator.
fn structural_binding(source: &mut ValidatedRuntimeSpill, transport: SelectedStructuralTransport) {
    let (_, parameter_place) = destination();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[1].terminator else {
        unreachable!()
    };
    successor
        .structural_bindings
        .push(SelectedStructuralBinding {
            semantic: abstract_operations::AbstractStructuralBinding {
                parameter: parameter_place,
                argument: terminal_psi::StructuralArgument {
                    place: argument_place(),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                },
            },
            transport,
        });
}

fn seal(source: &mut ValidatedRuntimeSpill) {
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
}

/// The `Descriptor` idiom: two 8-byte `Load64` chunk loads plus two `Store64`
/// destination replacements; the continuation binding's `argument` is the
/// victim register.
fn descriptor_fixture(target: NativeTarget) -> ValidatedRuntimeSpill {
    let mut source = cfg_fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let chunks = [(0, 8u8), (8, 8u8)];
    snapshot_loads(&mut source, &environment, &chunks);
    destination_stores(&mut source, &environment, &chunks, false);
    let (slot, _) = destination();
    structural_binding(
        &mut source,
        SelectedStructuralTransport::Descriptor {
            argument: VirtualRegisterId(1),
            destination: slot,
        },
    );
    seal(&mut source);
    source
}

/// The `WholeValue` idiom at a single chunk: one `Load64`, a `FrameAddress`
/// pointer, and one typed `Store`.
fn whole_value_fixture(target: NativeTarget, byte_size: u16) -> ValidatedRuntimeSpill {
    let mut source = cfg_fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let mut offset = 0u32;
    let mut chunks = Vec::new();
    while offset < u32::from(byte_size) {
        let width = [8u8, 4, 2, 1]
            .into_iter()
            .find(|width| u32::from(*width) <= u32::from(byte_size) - offset)
            .unwrap();
        chunks.push((offset, width));
        offset += u32::from(width);
    }
    snapshot_loads(&mut source, &environment, &chunks);
    destination_stores(&mut source, &environment, &chunks, true);
    let (slot, _) = destination();
    structural_binding(
        &mut source,
        SelectedStructuralTransport::WholeValue {
            argument: VirtualRegisterId(1),
            destination: slot,
            byte_size,
            alignment: 8,
        },
    );
    seal(&mut source);
    source
}

fn bridge_terminator_binding(
    function: &selected_instructions::SelectedFunction,
) -> selected_instructions::SelectedStructuralTransport {
    let SelectedTerminator::Jump { successor, .. } = &function.blocks[1].terminator else {
        unreachable!()
    };
    successor.structural_bindings[0].transport
}

fn bridge_instruction(
    function: &selected_instructions::SelectedFunction,
    id: u32,
) -> &selected_instructions::SelectedInstruction {
    function.blocks[1]
        .instructions
        .iter()
        .find(|instruction| instruction.id == SelectedInstructionId(id))
        .unwrap()
}

#[test]
fn descriptor_snapshot_argument_moves_to_the_shared_chunk_reload() {
    for target in targets() {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = descriptor_fixture(target);
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let transformed = &result.transformed().functions[0];
        // Both chunk loads read the block's one shared reload — the same
        // register the earlier flexible uses name.
        let first = bridge_instruction(transformed, 10).operands[0].virtual_register;
        let second = bridge_instruction(transformed, 11).operands[0].virtual_register;
        assert_eq!(first, second);
        assert_ne!(first, VirtualRegisterId(1));
        assert_eq!(
            bridge_instruction(transformed, 2).operands[0].virtual_register,
            first
        );
        let SelectedStructuralTransport::Descriptor { argument, .. } =
            bridge_terminator_binding(transformed)
        else {
            unreachable!()
        };
        assert_eq!(argument, first);
        assert!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
        for mutation in 0..4 {
            let mut proposed = result.transformed().clone();
            let function = &mut proposed.functions[0];
            match mutation {
                // The binding left naming the victim is not what replay builds.
                0 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[1].terminator
                    else {
                        unreachable!()
                    };
                    let SelectedStructuralTransport::Descriptor { argument, .. } =
                        &mut successor.structural_bindings[0].transport
                    else {
                        unreachable!()
                    };
                    *argument = VirtualRegisterId(1);
                }
                // A wrong register — here a chunk load's own output — is
                // equally rejected.
                1 => {
                    let wrong = bridge_instruction(&result.transformed().functions[0], 11).operands
                        [1]
                    .virtual_register;
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[1].terminator
                    else {
                        unreachable!()
                    };
                    let SelectedStructuralTransport::Descriptor { argument, .. } =
                        &mut successor.structural_bindings[0].transport
                    else {
                        unreachable!()
                    };
                    *argument = wrong;
                }
                // A dropped chunk load breaks the consumed stream.
                2 => {
                    function.blocks[1]
                        .instructions
                        .retain(|instruction| instruction.id != SelectedInstructionId(10));
                }
                // A load redirected to the wrong reload breaks the stream.
                3 => {
                    for instruction in &mut function.blocks[1].instructions {
                        if instruction.id == SelectedInstructionId(11) {
                            instruction.operands[0].virtual_register = VirtualRegisterId(1);
                        }
                    }
                }
                _ => unreachable!(),
            }
            assert_eq!(
                validate_runtime_spill(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    proposed
                )
                .unwrap_err(),
                RuntimeSpillError::ReplayMismatch,
                "target {target:?} mutation {mutation}"
            );
        }
    }
}

#[test]
fn whole_value_snapshot_argument_moves_to_its_chunk_reload() {
    for target in targets() {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = whole_value_fixture(target, 8);
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let transformed = &result.transformed().functions[0];
        let reloaded = bridge_instruction(transformed, 10).operands[0].virtual_register;
        assert_ne!(reloaded, VirtualRegisterId(1));
        let SelectedStructuralTransport::WholeValue { argument, .. } =
            bridge_terminator_binding(transformed)
        else {
            unreachable!()
        };
        assert_eq!(argument, reloaded);
    }
}

#[test]
fn structural_argument_uses_reject_unverifiable_or_ambiguous_plans() {
    for target in targets() {
        let environment = baseline_target_register_environment(target).unwrap();
        for mutation in 0..7 {
            let mut source = descriptor_fixture(target);
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            match mutation {
                // No `ReadPlace` access stream: the binding cannot resolve.
                0 => function
                    .memory_accesses
                    .retain(|access| access.role != SelectedMemoryAccessRole::ReadPlace),
                // A truncated stream is not the transport's chunk decomposition.
                1 => {
                    let index = function
                        .memory_accesses
                        .iter()
                        .position(|access| {
                            access.role == SelectedMemoryAccessRole::ReadPlace
                                && access.byte_offset == 8
                        })
                        .unwrap();
                    function.memory_accesses.remove(index);
                }
                // A chunk load reading another register is not a victim use.
                2 => {
                    for instruction in &mut function.blocks[1].instructions {
                        if instruction.id == SelectedInstructionId(10) {
                            instruction.operands[0].virtual_register = VirtualRegisterId(0);
                        }
                    }
                }
                // A pinned chunk operand keeps a private pair, so the two
                // loads can never share one register.
                3 => {
                    let class = function.virtual_registers[1].class;
                    let view = environment
                        .physical()
                        .model()
                        .classes
                        .iter()
                        .find(|row| row.id == class)
                        .and_then(|row| row.views.first())
                        .copied()
                        .unwrap();
                    for instruction in &mut function.blocks[1].instructions {
                        if instruction.id == SelectedInstructionId(11) {
                            instruction.operands[0].fixed_view = Some(view);
                        }
                    }
                }
                // A unit-writing instruction between the chunks closes the
                // shared reload, so the loads cannot name one register.
                4 => {
                    let copy = environment
                        .constraint(environment.selected_keys().copy_i64)
                        .unwrap();
                    let unit = environment
                        .physical()
                        .model()
                        .views
                        .iter()
                        .flat_map(|view| view.units.iter().copied())
                        .next()
                        .unwrap_or(RegisterUnitId(0));
                    let mut instruction = admission::instruction(
                        SelectedInstructionId(14),
                        SelectedInstructionKind::CopyI64,
                        copy,
                        &[VirtualRegisterId(0), VirtualRegisterId(0)],
                    );
                    instruction.clobbers.push(unit);
                    let position = function.blocks[1]
                        .instructions
                        .iter()
                        .position(|instruction| instruction.id == SelectedInstructionId(11))
                        .unwrap();
                    function.blocks[1]
                        .instructions
                        .insert(position, instruction);
                }
                // The same binding on a non-continuation successor is not the
                // bridge's transfer record.
                5 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[1].terminator
                    else {
                        unreachable!()
                    };
                    successor.role = SelectedSuccessorRole::Semantic;
                }
                // A destination outside the continuation's own destination
                // block is still fine only for non-parameter victims, but a
                // different snapshot kind breaks the chunk match.
                6 => {
                    for instruction in &mut function.blocks[1].instructions {
                        if instruction.id == SelectedInstructionId(11) {
                            instruction.kind = SelectedInstructionKind::Load32 { byte_offset: 8 };
                        }
                    }
                }
                _ => unreachable!(),
            }
            seal(&mut source);
            assert_eq!(
                spill_selected_runtime_value(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget()
                )
                .unwrap_err(),
                match mutation {
                    5 => RuntimeSpillError::UnsupportedControlFlow,
                    _ => RuntimeSpillError::UnsupportedUse,
                },
                "target {target:?} mutation {mutation}"
            );
        }
    }
}

/// An instruction-defined transport register — the `CopyI64` pointer idiom a
/// retained place address uses — spills and replays like a scalar result,
/// but its reload registers carry the declared place and byte offset in an
/// observation origin rather than a source `ValueId`. A nonzero offset rides
/// along: the reload restates the victim's own coordinate.
#[test]
fn instruction_defined_transport_register_spills_through_its_uses() {
    for target in targets() {
        let environment = baseline_target_register_environment(target).unwrap();
        for byte_offset in [0u32, 8] {
            let mut source = descriptor_fixture(target);
            {
                let victim =
                    &mut Arc::make_mut(&mut source.transformed).functions[0].virtual_registers[1];
                victim.origin = VirtualRegisterOrigin::AbiTransport {
                    instruction: SelectedInstructionId(1),
                    place: argument_place(),
                    byte_offset,
                };
                victim.definition_site = None;
            }
            seal(&mut source);
            let result = spill_selected_runtime_value(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
            )
            .unwrap();
            let transformed = &result.transformed().functions[0];
            // The snapshot's chunk loads read the block's shared reload —
            // the same register the earlier flexible uses name — and the
            // binding's `argument` follows it.
            let first = bridge_instruction(transformed, 10).operands[0].virtual_register;
            assert_eq!(
                first,
                bridge_instruction(transformed, 11).operands[0].virtual_register
            );
            assert_ne!(first, VirtualRegisterId(1));
            assert_eq!(
                bridge_instruction(transformed, 2).operands[0].virtual_register,
                first
            );
            let SelectedStructuralTransport::Descriptor { argument, .. } =
                bridge_terminator_binding(transformed)
            else {
                unreachable!()
            };
            assert_eq!(argument, first);
            // Every reload register re-observes the victim's own place and
            // byte offset — there is no source `ValueId` to restate.
            let reload_register = transformed
                .virtual_registers
                .iter()
                .find(|register| register.id == first)
                .unwrap();
            assert!(matches!(
                reload_register.origin,
                VirtualRegisterOrigin::StructuralObservation { place, byte_offset: offset, .. }
                    if place == argument_place() && offset == byte_offset
            ));
            assert!(
                validate_runtime_spill(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    result.transformed().clone()
                )
                .is_ok()
            );
        }
    }
}

#[test]
fn structural_argument_on_the_victims_own_incoming_edge_stays_rejected() {
    for target in targets() {
        let environment = baseline_target_register_environment(target).unwrap();
        // The victim is the destination's block parameter: a snapshot reading
        // its register on an edge that also initializes it could not order
        // against the edge's own definition store.
        let mut source = super::parameters::parameter_fixture(target);
        let (slot, _) = destination();
        structural_binding(
            &mut source,
            SelectedStructuralTransport::Descriptor {
                argument: VirtualRegisterId(1),
                destination: slot,
            },
        );
        seal(&mut source);
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap_err(),
            RuntimeSpillError::UnsupportedUse,
            "target {target:?}"
        );
    }
}

#[test]
fn whole_value_multi_chunk_needs_the_shared_reload() {
    for target in targets() {
        let environment = baseline_target_register_environment(target).unwrap();
        // A 16-byte whole value snapshots through two chunks; admitted under
        // the same shared-reload rule as the descriptor.
        let source = whole_value_fixture(target, 16);
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let transformed = &result.transformed().functions[0];
        let first = bridge_instruction(transformed, 10).operands[0].virtual_register;
        let second = bridge_instruction(transformed, 11).operands[0].virtual_register;
        assert_eq!(first, second);
        let SelectedStructuralTransport::WholeValue { argument, .. } =
            bridge_terminator_binding(transformed)
        else {
            unreachable!()
        };
        assert_eq!(argument, first);
    }
}
