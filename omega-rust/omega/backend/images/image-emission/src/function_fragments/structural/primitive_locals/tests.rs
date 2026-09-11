use super::access::operation_retained;
use super::establishment::establishment;
use super::frame_location::frame_location;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use machine_code::StructuralSourceLocation;
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedLocalStorageSlot, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand, SelectedTerminator,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType,
    StructuralTypeId, ValueId,
};
use terminal_psi::{StructuralMultiplicity, StructuralTypeShape};

fn fixture() -> (AbstractFunction, SelectedFunction) {
    let operation = OperationId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = AbstractResult {
        value: ValueId::new(1).unwrap(),
        scalar_type,
    };
    let result = terminal_psi::StructuralOperationResult {
        place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![],
        projected_qualifications: vec![],
        claims: vec![],
    };
    let function = AbstractFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: BlockId::new(1).unwrap(),
        parameters: vec![],
        structural_parameters: vec![],
        result: AbstractFunctionResult::Unit,
        entry_claims: vec![],
        published_service_ceiling: vec![],
        block_entries: vec![],
        operations: vec![AbstractOperation::EstablishPrimitiveLocal {
            psi_operation: operation,
            result,
            value,
        }],
    };
    let slot = LocalStorageSlotId::Structural { operation, place };
    let operand = |register, access| SelectedOperand {
        operand: 0,
        virtual_register: VirtualRegisterId(register),
        access,
        class: RegisterClassId(0),
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    };
    let instruction = |id, kind, operands| SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 0,
        },
        operands,
        implicit_uses: vec![],
        implicit_defs: vec![],
        clobbers: vec![],
        provenance: SelectedInstructionProvenance {
            operations: vec![operation],
            values: vec![value.value],
            ..Default::default()
        },
    };
    let selected = SelectedFunction {
        machine: function.machine,
        attachment: None,
        provenance: Default::default(),
        ranked: None,
        structural: Some(legalized_operations::LegalizedStructuralContract {
            result: None,
            structural_types: vec![terminal_psi::StructuralTypeDeclaration {
                id: structural_type,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
            }]
            .into(),
            parameters: vec![],
            structural_places: vec![terminal_psi::StructuralPlaceDeclaration {
                id: place,
                kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type,
                },
            }],
            entry_claims: vec![],
            published_service_ceiling: vec![],
        }),
        local_storage_slots: vec![SelectedLocalStorageSlot {
            id: slot,
            byte_size: 8,
            alignment: 8,
        }],
        outgoing_arguments: vec![],
        calls: vec![],
        boundary_settlements: vec![],
        memory_accesses: vec![
            SelectedMemoryAccess {
                instruction: SelectedInstructionId(0),
                origin: SelectedMemoryAccessOrigin::Operation(operation),
                place,
                byte_offset: 0,
                byte_count: 8,
                role: SelectedMemoryAccessRole::AddressLocal { slot },
            },
            SelectedMemoryAccess {
                instruction: SelectedInstructionId(1),
                origin: SelectedMemoryAccessOrigin::Operation(operation),
                place,
                byte_offset: 0,
                byte_count: 8,
                role: SelectedMemoryAccessRole::WritePlace,
            },
        ],
        entry_block: SelectedBlockId(0),
        virtual_registers: vec![
            VirtualRegister {
                id: VirtualRegisterId(0),
                scalar_type,
                class: RegisterClassId(0),
                origin: VirtualRegisterOrigin::AbiTransport {
                    instruction: SelectedInstructionId(0),
                    place,
                    byte_offset: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
            VirtualRegister {
                id: VirtualRegisterId(1),
                scalar_type,
                class: RegisterClassId(0),
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: value.value,
                    parameter_index: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
        ],
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(function.entry),
            instructions: vec![
                instruction(
                    0,
                    SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset: 0,
                    },
                    vec![operand(0, RegisterOperandAccess::Def)],
                ),
                instruction(
                    1,
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 8,
                    },
                    vec![
                        operand(0, RegisterOperandAccess::Use),
                        operand(1, RegisterOperandAccess::Use),
                    ],
                ),
            ],
            terminator: SelectedTerminator::Return {
                instruction: instruction(2, SelectedInstructionKind::ReturnUnit, vec![]),
                psi_return_edge: EdgeId::new(1).unwrap(),
            },
        }],
    };
    (function, selected)
}

#[test]
fn initialized_local_requires_exact_producer_address_and_executed_store() {
    // These are publication predicate inputs, not complete physical replay certificates.
    let (function, selected) = fixture();
    let place = PlaceId::new(1).unwrap();
    assert!(establishment(&function, &selected, place).is_some());
    for mutation in 0..13 {
        let mut changed = selected.clone();
        match mutation {
            0 => changed.memory_accesses.truncate(1),
            1 => changed.memory_accesses.swap(0, 1),
            2 => changed.blocks[0].instructions.swap(0, 1),
            3 => {
                changed.blocks[0].instructions[1].operands[0].virtual_register =
                    VirtualRegisterId(2)
            }
            4 => changed.blocks[0].instructions[1].provenance.values.clear(),
            5 => changed.local_storage_slots[0].byte_size = 16,
            6 => changed.local_storage_slots[0].alignment = 4,
            7 => changed
                .local_storage_slots
                .push(changed.local_storage_slots[0].clone()),
            8 => changed.memory_accesses[1].place = PlaceId::new(2).unwrap(),
            9 => changed.memory_accesses[1].role = SelectedMemoryAccessRole::ReadPlace,
            10 => {
                changed.structural.as_mut().unwrap().structural_places[0].kind =
                    semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: OperationId::new(2).unwrap(),
                        structural_type: StructuralTypeId::new(1).unwrap(),
                    }
            }
            11 => {
                changed.blocks[0].instructions[1].kind =
                    SelectedInstructionKind::Load64 { byte_offset: 0 }
            }
            _ => {
                changed.blocks[0].instructions[1].operands[1].virtual_register =
                    VirtualRegisterId(0)
            }
        }
        assert!(
            establishment(&function, &changed, place).is_none(),
            "mutation {mutation}"
        );
    }
    let mut duplicate = function.clone();
    duplicate.operations.push(duplicate.operations[0].clone());
    assert!(establishment(&duplicate, &selected, place).is_none());
    let mut changed = function.clone();
    let AbstractOperation::EstablishPrimitiveLocal { result, .. } = &mut changed.operations[0]
    else {
        unreachable!()
    };
    result.place = PlaceId::new(2).unwrap();
    assert!(establishment(&changed, &selected, place).is_none());
}

#[test]
fn local_frame_identity_cannot_be_replaced_by_matching_geometry() {
    let slot = LocalStorageSlotId::Structural {
        operation: OperationId::new(1).unwrap(),
        place: PlaceId::new(1).unwrap(),
    };
    let home = machine_code::LocalStorageFrameSlot {
        id: slot,
        frame_offset_bytes: 16,
        size_bytes: 8,
        alignment_bytes: 8,
    };
    assert_eq!(
        frame_location(std::slice::from_ref(&home), 24, slot, 8).unwrap(),
        StructuralSourceLocation::Stack { byte_offset: 16 }
    );
    assert!(frame_location(&[], 24, slot, 8).is_err());
    assert!(frame_location(&[home.clone(), home.clone()], 24, slot, 8).is_err());
    assert!(frame_location(std::slice::from_ref(&home), 23, slot, 8).is_err());
    for mutation in 0..6 {
        let mut changed = home.clone();
        match mutation {
            0 => {
                changed.id = LocalStorageSlotId::Structural {
                    operation: OperationId::new(2).unwrap(),
                    place: PlaceId::new(1).unwrap(),
                }
            }
            1 => {
                changed.id = LocalStorageSlotId::Structural {
                    operation: OperationId::new(1).unwrap(),
                    place: PlaceId::new(2).unwrap(),
                }
            }
            2 => changed.frame_offset_bytes = 17,
            3 => changed.frame_offset_bytes = u64::MAX - 7,
            4 => changed.size_bytes = 16,
            _ => changed.alignment_bytes = 4,
        }
        assert!(
            frame_location(&[changed], 24, slot, 8).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn primitive_observation_requires_a_fresh_load_result() {
    let (mut function, mut selected) = fixture();
    let operation = OperationId::new(3).unwrap();
    let value = ValueId::new(2).unwrap();
    let result = AbstractResult {
        value,
        scalar_type: selected.virtual_registers[0].scalar_type,
    };
    let read = AbstractOperation::PrimitiveScalarRead {
        psi_operation: operation,
        result,
        source: PlaceId::new(1).unwrap(),
    };
    function.operations.push(read.clone());
    let mut load = selected.blocks[0].instructions[1].clone();
    load.id = SelectedInstructionId(3);
    load.kind = SelectedInstructionKind::Load64 { byte_offset: 0 };
    load.operands[1].virtual_register = VirtualRegisterId(2);
    load.operands[1].access = RegisterOperandAccess::Def;
    load.provenance.operations = vec![operation];
    load.provenance.values = vec![value];
    selected.blocks[0].instructions.push(load);
    selected.virtual_registers.push(VirtualRegister {
        id: VirtualRegisterId(2),
        scalar_type: result.scalar_type,
        class: RegisterClassId(0),
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(3),
            source_value: value,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    selected.memory_accesses.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(3),
        origin: SelectedMemoryAccessOrigin::Operation(operation),
        place: PlaceId::new(1).unwrap(),
        byte_offset: 0,
        byte_count: 8,
        role: SelectedMemoryAccessRole::ReadPlace,
    });
    assert!(operation_retained(&function, &selected, &read));
    for mutation in 0..5 {
        let mut changed = selected.clone();
        match mutation {
            0 => {
                changed.memory_accesses.pop();
            }
            1 => changed.blocks[0].instructions[2].kind = SelectedInstructionKind::CopyI64,
            2 => {
                changed.blocks[0].instructions[2].operands[1].virtual_register =
                    VirtualRegisterId(1)
            }
            3 => {
                changed.virtual_registers[2].origin = VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(1),
                    source_value: value,
                }
            }
            _ => changed.memory_accesses[2].place = PlaceId::new(2).unwrap(),
        }
        assert!(
            !operation_retained(&function, &changed, &read),
            "mutation {mutation}"
        );
    }
}
