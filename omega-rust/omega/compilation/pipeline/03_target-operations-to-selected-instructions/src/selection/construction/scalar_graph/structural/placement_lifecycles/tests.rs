//! Execute the selected data-transfer instructions against bounded byte storage.
//! These are lowering checks, not a lifecycle settlement or native boot receipt.

use super::*;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionProvenance, SelectedSelectionConstraints,
};
use semantic_vocabulary::{IntegerValue, OperationId};
use target::NativeTarget;

pub(super) fn with_builder(
    target: NativeTarget,
    test: impl FnOnce(&mut Builder<'_>, &LegalizedScalarInstruction),
) {
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
    let constraints = SelectedSelectionConstraints {
        keys: environment.selected_keys(),
        fixed_inputs: vec![],
    };
    let catalog = environment.constraints();
    let materialize =
        crate::selection::constraints::row(catalog, constraints.keys.materialize_i64).unwrap();
    let mut builder = Builder {
        environment: &environment,
        pending_provenance: SelectedInstructionProvenance::default(),
        required_values: Default::default(),
        transport: Default::default(),
        class: materialize.operands[0].class,
        constraints: &constraints,
        catalog,
        registers: vec![],
        pending_result_origins: vec![],
        instructions: vec![],
        implementation_blocks: vec![],
        open_block: None,
        next_block: 0,
        block_body_end: None,
        edge_work: vec![],
        definitions: vec![],
    };
    let row = LegalizedScalarInstruction {
        operation: OperationId::new(71).unwrap(),
        result: None,
        kind: LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(0)),
        fuel: vec![],
        effect: optimization_unit::EffectLink {
            input: 0,
            output: 1,
        },
        ownership: vec![],
    };
    test(&mut builder, &row);
}

pub(super) fn execute(
    instructions: &[SelectedInstruction],
    registers: &mut [u64],
    bytes: &mut [u8],
) -> (u64, u64) {
    execute_with_frame(instructions, registers, bytes, &[])
}

pub(super) fn execute_with_frame(
    instructions: &[SelectedInstruction],
    registers: &mut [u64],
    bytes: &mut [u8],
    frame: &[(selected_instructions::FrameStorageSlotId, u64)],
) -> (u64, u64) {
    let mut compared = (0_u64, 0_u64);
    for instruction in instructions {
        let register =
            |operand: u16| instruction.operand(operand).unwrap().virtual_register.0 as usize;
        let value = |operand| registers[register(operand)];
        let (destination, result) = match instruction.kind {
            SelectedInstructionKind::FrameAddress { slot, byte_offset } => {
                let (_, base) = frame
                    .iter()
                    .find(|(candidate, _)| *candidate == slot)
                    .expect("every allocated scratch slot needs a test frame address");
                (0, base.checked_add(u64::from(byte_offset)).unwrap())
            }
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(bits),
            } => (0, bits as u64),
            SelectedInstructionKind::Load8 { byte_offset } => (
                1,
                u64::from(bytes[(value(0) + u64::from(byte_offset)) as usize]),
            ),
            SelectedInstructionKind::Load16 { byte_offset }
            | SelectedInstructionKind::Load32 { byte_offset }
            | SelectedInstructionKind::Load64 { byte_offset } => {
                let width = match instruction.kind {
                    SelectedInstructionKind::Load16 { .. } => 2,
                    SelectedInstructionKind::Load32 { .. } => 4,
                    _ => 8,
                };
                let start = (value(0) + u64::from(byte_offset)) as usize;
                let mut word = [0; 8];
                word[..width].copy_from_slice(&bytes[start..start + width]);
                (1, u64::from_le_bytes(word))
            }
            SelectedInstructionKind::AddressOffset { byte_offset } => {
                (1, value(0).checked_add(u64::from(byte_offset)).unwrap())
            }
            SelectedInstructionKind::CopyI64 => (1, value(0)),
            SelectedInstructionKind::ZeroBytes => {
                let length = usize::try_from(value(1)).unwrap();
                if length != 0 {
                    let start = usize::try_from(value(0)).unwrap();
                    bytes[start..start + length].fill(0);
                }
                continue;
            }
            SelectedInstructionKind::BitwiseOrI64 => (2, value(0) | value(1)),
            SelectedInstructionKind::BitwiseAndI64 => (2, value(0) & value(1)),
            SelectedInstructionKind::BitwiseXorI64 => (2, value(0) ^ value(1)),
            SelectedInstructionKind::WrappingSubtractI64 => (2, value(0).wrapping_sub(value(1))),
            SelectedInstructionKind::WrappingAddI64 => (2, value(0).wrapping_add(value(1))),
            SelectedInstructionKind::ByteViewAddress => {
                (2, value(0).checked_add(value(1)).unwrap())
            }
            SelectedInstructionKind::WrappingMultiplyI64 => (2, value(0).wrapping_mul(value(1))),
            SelectedInstructionKind::CompareI64 => {
                compared = (value(0), value(1));
                continue;
            }
            SelectedInstructionKind::CompareI64Zero => {
                compared = (value(0), 0);
                continue;
            }
            SelectedInstructionKind::MaterializeBooleanEqual => {
                (0, u64::from(compared.0 == compared.1))
            }
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => {
                (0, u64::from(compared.0 <= compared.1))
            }
            SelectedInstructionKind::CopyBytes => {
                let length = usize::try_from(value(2)).unwrap();
                if length != 0 {
                    let source = usize::try_from(value(0)).unwrap();
                    let destination = usize::try_from(value(1)).unwrap();
                    assert!(
                        source + length <= destination || destination + length <= source,
                        "staged image must not overlap the backing"
                    );
                    bytes.copy_within(source..source + length, destination);
                }
                continue;
            }
            SelectedInstructionKind::WrappingShiftLeftI64 => {
                (2, value(0).wrapping_shl(value(1) as u32))
            }
            SelectedInstructionKind::WrappingShiftRightU64 => {
                (2, value(0).wrapping_shr(value(1) as u32))
            }
            SelectedInstructionKind::Store {
                byte_offset,
                byte_size,
            } => {
                let address = (value(0) + u64::from(byte_offset)) as usize;
                bytes[address..address + usize::from(byte_size)]
                    .copy_from_slice(&value(1).to_le_bytes()[..usize::from(byte_size)]);
                continue;
            }
            ref unexpected => panic!("unexpected resident codec instruction: {unexpected:?}"),
        };
        registers[register(destination)] = result;
    }
    compared
}

pub(super) fn targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ]
}

/// Execute one emitted operation through its physical control graph. A trap
/// has no continuation; returning None is not a recoverable rejection result.
pub(super) fn execute_operation(
    builder: &Builder<'_>,
    bytes: &mut [u8],
    frame: &[(selected_instructions::FrameStorageSlotId, u64)],
) -> Option<Vec<u64>> {
    use selected_instructions::{SelectedBlockId, SelectedTerminator};
    let continuation = builder.open_block.unwrap();
    let mut registers = vec![0; builder.registers.len()];
    let mut current = SelectedBlockId(0);
    let mut visits = 0;
    while current != continuation.id {
        visits += 1;
        assert!(visits < 100_000, "operation did not terminate");
        let block = builder
            .implementation_blocks
            .iter()
            .find(|block| block.id == current)
            .unwrap();
        let compared = execute_with_frame(&block.instructions, &mut registers, bytes, frame);
        current = match &block.terminator {
            SelectedTerminator::Jump { successor, .. } => successor.block,
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => {
                if compared.0 != compared.1 {
                    when_nonzero.block
                } else {
                    when_zero.block
                }
            }
            SelectedTerminator::OperationTrap { .. } => return None,
            unexpected => panic!("unexpected operation control {unexpected:?}"),
        };
    }
    execute_with_frame(
        &builder.instructions[continuation.start..],
        &mut registers,
        bytes,
        frame,
    );
    Some(registers)
}

#[test]
fn failed_operation_assertion_traps_without_a_source_edge_or_continuation() {
    use super::super::super::operation_control::{self, OpenBlock};
    use selected_instructions::{SelectedBlockId, SelectedBlockOrigin, SelectedTerminator};
    use semantic_vocabulary::BlockId;
    for target in targets() {
        for input in [0, 1, 7] {
            with_builder(target, |builder, row| {
                builder.open_block = Some(OpenBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(BlockId::new(7).unwrap()),
                    start: 0,
                });
                builder.next_block = 1;
                let place = PlaceId::new(19).unwrap();
                let pointer = fragmented_scalar::constant(builder, row, place, 0).unwrap();
                let predicate = fragmented_scalar::constant(builder, row, place, input).unwrap();
                operation_control::require_nonzero(builder, row, predicate).unwrap();
                let one = fragmented_scalar::constant(builder, row, place, 1).unwrap();
                ScalarImageCodec::Bytes {
                    byte_offset: 0,
                    width: 8,
                    signed: false,
                }
                .write(
                    builder,
                    row,
                    ImageHome {
                        storage: fragmented_scalar::StorageAccess::Place(&[]),
                        place,
                        pointer,
                        byte_offset: 0,
                    },
                    one,
                )
                .unwrap();
                let continuation = builder.open_block.unwrap();
                let mut registers = vec![0; builder.registers.len()];
                let mut bytes = [0xa5; 8];
                let mut current = SelectedBlockId(0);
                let mut trapped = false;
                for _ in 0..4 {
                    if current == continuation.id {
                        execute(
                            &builder.instructions[continuation.start..],
                            &mut registers,
                            &mut bytes,
                        );
                        break;
                    }
                    let block = builder
                        .implementation_blocks
                        .iter()
                        .find(|block| block.id == current)
                        .unwrap();
                    let compared = execute(&block.instructions, &mut registers, &mut bytes);
                    match &block.terminator {
                        SelectedTerminator::ConditionalBranch {
                            when_nonzero,
                            when_zero,
                            ..
                        } => {
                            current = if compared.0 != compared.1 {
                                when_nonzero.block
                            } else {
                                when_zero.block
                            };
                        }
                        SelectedTerminator::OperationTrap {
                            operation,
                            instruction,
                        } => {
                            assert_eq!(*operation, row.operation);
                            assert_eq!(instruction.kind, SelectedInstructionKind::Crash);
                            assert!(
                                instruction.provenance.edges.is_empty()
                                    && instruction.provenance.fuel.is_empty()
                            );
                            assert!(
                                selected_instructions::terminator_successors(&block.terminator)
                                    .is_empty()
                            );
                            trapped = true;
                            break;
                        }
                        unexpected => panic!("unexpected assertion control {unexpected:?}"),
                    }
                }
                assert_eq!(trapped, input == 0);
                assert_eq!(
                    bytes,
                    if trapped {
                        [0xa5; 8]
                    } else {
                        1_u64.to_le_bytes()
                    }
                );
            });
        }
    }
}

#[test]
fn operation_owned_nested_loops_execute_without_semantic_edges() {
    use super::super::super::operation_control::{self, OpenBlock};
    use selected_instructions::{
        SelectedBlockId, SelectedBlockOrigin, SelectedSuccessorRole, SelectedTerminator,
    };
    use semantic_vocabulary::BlockId;
    for target in targets() {
        for count in [0_u64, 1, 32] {
            with_builder(target, |builder, row| {
                let source = BlockId::new(7).unwrap();
                builder.open_block = Some(OpenBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(source),
                    start: 0,
                });
                builder.next_block = 1;
                let place = PlaceId::new(19).unwrap();
                let pointer = fragmented_scalar::constant(builder, row, place, 0).unwrap();
                let home = ImageHome {
                    storage: fragmented_scalar::StorageAccess::Place(&[]),
                    place,
                    pointer,
                    byte_offset: 0,
                };
                let codec = |byte_offset| ScalarImageCodec::Bytes {
                    byte_offset,
                    width: 8,
                    signed: false,
                };
                let decrement = |builder: &mut Builder<'_>, byte_offset| {
                    let current = codec(byte_offset).read(builder, row, home)?;
                    let one = fragmented_scalar::constant(builder, row, place, 1)?;
                    let next = fragmented_scalar::binary(
                        builder,
                        row,
                        place,
                        SelectedInstructionKind::WrappingSubtractI64,
                        current,
                        one,
                    )?;
                    codec(byte_offset).write(builder, row, home, next)
                };
                let preceding = OperationId::new(70).unwrap();
                builder.pending_provenance = SelectedInstructionProvenance {
                    operations: vec![preceding],
                    fuel: vec![optimization_unit::FuelSettlement {
                        site: optimization_unit::PsiProvenance::Operation(preceding),
                        units: 3,
                    }],
                    ..Default::default()
                };
                operation_control::while_nonzero(
                    builder,
                    row,
                    |builder| codec(0).read(builder, row, home),
                    |builder| {
                        let two = fragmented_scalar::constant(builder, row, place, 2)?;
                        codec(8).write(builder, row, home, two)?;
                        operation_control::while_nonzero(
                            builder,
                            row,
                            |builder| codec(8).read(builder, row, home),
                            |builder| {
                                let accumulated = codec(16).read(builder, row, home)?;
                                let one = fragmented_scalar::constant(builder, row, place, 1)?;
                                let next = fragmented_scalar::binary(
                                    builder,
                                    row,
                                    place,
                                    SelectedInstructionKind::WrappingAddI64,
                                    accumulated,
                                    one,
                                )?;
                                codec(16).write(builder, row, home, next)?;
                                decrement(builder, 8)
                            },
                        )?;
                        decrement(builder, 0)
                    },
                )
                .unwrap();
                let one = fragmented_scalar::constant(builder, row, place, 1).unwrap();
                codec(24).write(builder, row, home, one).unwrap();
                let continuation = builder.open_block.unwrap();
                assert_eq!(builder.implementation_blocks.len(), 6);
                let mut registers = vec![0; builder.registers.len()];
                let mut bytes = vec![0; 40];
                bytes[..8].copy_from_slice(&count.to_le_bytes());
                bytes[32..].fill(0xa7);
                let mut current = SelectedBlockId(0);
                let mut visits = 0;
                let mut charged = 0;
                while current != continuation.id {
                    visits += 1;
                    assert!(visits <= 400, "operation loop did not terminate");
                    let block = builder
                        .implementation_blocks
                        .iter()
                        .find(|block| block.id == current)
                        .unwrap();
                    let compared = execute(&block.instructions, &mut registers, &mut bytes);
                    charged += block
                        .instructions
                        .iter()
                        .flat_map(|instruction| &instruction.provenance.fuel)
                        .map(|settlement| settlement.units)
                        .sum::<u64>();
                    assert!(
                        selected_instructions::terminator_instruction(&block.terminator)
                            .provenance
                            .fuel
                            .is_empty()
                    );
                    for successor in selected_instructions::terminator_successors(&block.terminator)
                    {
                        assert_eq!(
                            successor.role,
                            SelectedSuccessorRole::OperationContinuation {
                                operation: row.operation
                            }
                        );
                        assert_eq!(successor.psi_edge, None);
                        assert!(
                            successor.fuel.is_empty()
                                && successor.bindings.is_empty()
                                && successor.structural_bindings.is_empty()
                        );
                    }
                    current = match &block.terminator {
                        SelectedTerminator::Jump { successor, .. } => successor.block,
                        SelectedTerminator::ConditionalBranch {
                            when_nonzero,
                            when_zero,
                            ..
                        } => {
                            if compared.0 != compared.1 {
                                when_nonzero.block
                            } else {
                                when_zero.block
                            }
                        }
                        unexpected => panic!("unexpected operation control {unexpected:?}"),
                    };
                }
                execute(
                    &builder.instructions[continuation.start..],
                    &mut registers,
                    &mut bytes,
                );
                assert_eq!(u64::from_le_bytes(bytes[0..8].try_into().unwrap()), 0);
                assert_eq!(
                    u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
                    count * 2
                );
                assert_eq!(u64::from_le_bytes(bytes[24..32].try_into().unwrap()), 1);
                assert_eq!(&bytes[32..], &[0xa7; 8]);
                assert_eq!(
                    charged, 3,
                    "preceding source fuel executes once, not once per native iteration"
                );
            });
        }
    }
}

#[test]
fn resident_scalar_decode_uses_actual_bytes_and_authored_extension() {
    let cases = [
        (
            LayoutPlacementReport::At { offset: 3 },
            0x0123_4567_89ab_cdef_u64,
            8,
        ),
        (
            LayoutPlacementReport::IntegerAt {
                offset: 3,
                stored_width: 24,
                interpretation: IntegerInterpretation::Unsigned,
            },
            0x00ff_ffff,
            8,
        ),
        (
            LayoutPlacementReport::IntegerAt {
                offset: 3,
                stored_width: 24,
                interpretation: IntegerInterpretation::Signed,
            },
            0xffff_ffff_ff80_0000,
            8,
        ),
        (
            LayoutPlacementReport::IntegerAt {
                offset: 3,
                stored_width: 8,
                interpretation: IntegerInterpretation::Signed,
            },
            u64::MAX,
            8,
        ),
    ];
    for target in targets() {
        for (placement, expected, width) in cases {
            with_builder(target, |builder, row| {
                let owner = PlaceId::new(9).unwrap();
                let backing = fragmented_scalar::constant(builder, row, owner, 11).unwrap();
                let decoded =
                    decode_scalar(builder, row, owner, backing, width, &[placement]).unwrap();
                let mut bytes = [0xa5; 40];
                bytes[14..22].copy_from_slice(&expected.to_le_bytes());
                let original = bytes;
                let mut registers = vec![0; builder.registers.len()];
                execute(&builder.instructions, &mut registers, &mut bytes);
                assert_eq!(registers[decoded.0 as usize], expected);
                assert_eq!(bytes, original, "decoding must not mutate the resident");
                assert!(
                    builder
                        .transport
                        .memory
                        .iter()
                        .all(|access| access.place == owner
                            && access.origin
                                == selected_instructions::SelectedMemoryAccessOrigin::Operation(
                                    row.operation
                                ))
                );
            });
        }
    }
}

#[test]
fn resident_fragment_transfer_preserves_unrelated_output_bits() {
    for target in targets() {
        with_builder(target, |builder, row| {
            let owner = PlaceId::new(9).unwrap();
            let destination = PlaceId::new(10).unwrap();
            let backing = fragmented_scalar::constant(builder, row, owner, 0).unwrap();
            let placements = [
                LayoutPlacementReport::Bits {
                    container: 1,
                    container_width: 16,
                    destination_lsb: 5,
                    source_lsb: 0,
                    width: 5,
                },
                LayoutPlacementReport::Bits {
                    container: 5,
                    container_width: 8,
                    destination_lsb: 2,
                    source_lsb: 5,
                    width: 3,
                },
            ];
            let decoded = decode_scalar(builder, row, owner, backing, 1, &placements).unwrap();
            let pointer = fragmented_scalar::constant(builder, row, destination, 16).unwrap();
            let output = AuthoredBitFragments::new([AuthoredBitFragment {
                container_byte_offset: 2,
                container_width_bits: 16,
                destination_lsb: 3,
                source_lsb: 0,
                width: 8,
            }]);
            fragmented_scalar::write_register(
                builder,
                row,
                destination,
                pointer,
                2,
                &output,
                fragmented_scalar::StorageAccess::Place(&[]),
                decoded,
                None,
            )
            .unwrap();
            let mut bytes = [0xa5; 32];
            bytes[1..3].copy_from_slice(&(0b10101_u16 << 5).to_le_bytes());
            bytes[5] = 0b110 << 2;
            let original = bytes;
            let mut registers = vec![0; builder.registers.len()];
            execute(&builder.instructions, &mut registers, &mut bytes);
            assert_eq!(registers[decoded.0 as usize], 0b110_10101);
            let previous = u16::from_le_bytes([original[18], original[19]]);
            let actual = u16::from_le_bytes([bytes[18], bytes[19]]);
            assert_eq!(actual, (previous & !(0xff << 3)) | (0b110_10101 << 3));
            assert_eq!(&bytes[..18], &original[..18]);
            assert_eq!(&bytes[20..], &original[20..]);
        });
    }
}

#[test]
fn resident_scalar_decode_refuses_invalid_carriers_and_mixed_placements() {
    with_builder(NativeTarget::linux_x64(), |builder, row| {
        let owner = PlaceId::new(9).unwrap();
        let backing = fragmented_scalar::constant(builder, row, owner, 0).unwrap();
        assert!(
            decode_scalar(
                builder,
                row,
                owner,
                backing,
                9,
                &[LayoutPlacementReport::At { offset: 0 }]
            )
            .is_err()
        );
        assert!(
            decode_scalar(
                builder,
                row,
                owner,
                backing,
                8,
                &[
                    LayoutPlacementReport::At { offset: 0 },
                    LayoutPlacementReport::IntegerAt {
                        offset: 8,
                        stored_width: 8,
                        interpretation: IntegerInterpretation::Unsigned
                    },
                ]
            )
            .is_err()
        );
        assert!(
            decode_scalar(
                builder,
                row,
                owner,
                backing,
                1,
                &[LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 8,
                    destination_lsb: 7,
                    source_lsb: 0,
                    width: 2
                },]
            )
            .is_err()
        );
    });
}

#[test]
fn resident_array_transfer_keeps_source_and_destination_strides_independent() {
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralFieldId};
    use terminal_psi::{BindingRelevance, StructuralFieldType, TerminalIdentityIndex};
    let scalar = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let declaration = |id, shape| StructuralTypeDeclaration {
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex(0),
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        copyable: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape,
    };
    let types = vec![
        declaration(
            scalar,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            )),
        ),
        declaration(
            array,
            StructuralTypeShape::FixedArray {
                element: scalar,
                length: 2,
            },
        ),
    ];
    let field = StructuralFieldDeclaration {
        id: StructuralFieldId::new(1).unwrap(),
        identity: TerminalIdentityIndex(1),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(array),
        storage: Some(AuthoredFieldStorage::Repeated {
            byte_offset: 16,
            element_stride: 12,
        }),
    };
    let scattered = aggregate_chunks(
        &types,
        &field,
        8,
        &[
            LayoutPlacementReport::At { offset: 40 },
            LayoutPlacementReport::At { offset: 20 },
        ],
    )
    .unwrap();
    assert_eq!(
        scattered,
        vec![
            ImageChunk {
                source_offset: 20,
                destination_offset: 0,
                byte_count: 4
            },
            ImageChunk {
                source_offset: 40,
                destination_offset: 12,
                byte_count: 4
            },
        ]
    );
    let contiguous = aggregate_chunks(
        &types,
        &field,
        8,
        &[LayoutPlacementReport::At { offset: 20 }],
    )
    .unwrap();
    assert_eq!(
        contiguous,
        vec![
            ImageChunk {
                source_offset: 20,
                destination_offset: 0,
                byte_count: 4
            },
            ImageChunk {
                source_offset: 24,
                destination_offset: 12,
                byte_count: 4
            },
        ]
    );
    assert!(
        aggregate_chunks(
            &types,
            &field,
            12,
            &[LayoutPlacementReport::At { offset: 20 }]
        )
        .is_err()
    );
    assert!(aggregate_chunks(&types, &field, 8, &[]).is_err());
}

fn image_fixture() -> (
    Vec<StructuralTypeDeclaration>,
    TerminalPlacementLifecyclePlan,
) {
    use access_plans::{AccessPlan, BoundaryReach, PlacementPlan, validate_placement_plan};
    use layout_plans::{LayoutFieldEntryReport, LayoutPlanReport};
    use semantic_vocabulary::{
        BoundaryMachineId, IntegerSign, IntegerType, ScalarType, StructuralDomainId,
        StructuralFieldId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralFieldType, StructuralRecordStorage, TerminalIdentityIndex,
        TerminalPlacementLifecycleExecution, TerminalPlacementLifecycleOperation,
        TerminalPlacementStorage,
    };

    let schema = StructuralTypeId::new(1).unwrap();
    let element = StructuralTypeId::new(2).unwrap();
    let array = StructuralTypeId::new(3).unwrap();
    let scalar = |sign, width| ScalarType::Integer(IntegerType::new(sign, width).unwrap());
    let field = |number, field_type| StructuralFieldDeclaration {
        id: StructuralFieldId::new(number).unwrap(),
        identity: TerminalIdentityIndex(u32::try_from(number).unwrap()),
        relevance: BindingRelevance::Relevant,
        field_type,
        storage: None,
    };
    let declaration = |id, shape| StructuralTypeDeclaration {
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex(0),
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        copyable: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape,
    };
    let types = vec![
        declaration(
            schema,
            StructuralTypeShape::Record {
                fields: vec![
                    field(
                        1,
                        StructuralFieldType::Scalar(scalar(IntegerSign::Unsigned, 64)),
                    ),
                    field(
                        2,
                        StructuralFieldType::Scalar(scalar(IntegerSign::Unsigned, 8)),
                    ),
                    field(
                        3,
                        StructuralFieldType::Scalar(scalar(IntegerSign::Signed, 64)),
                    ),
                    field(4, StructuralFieldType::Structural(array)),
                ],
                storage: StructuralRecordStorage::Packed,
            },
        ),
        declaration(
            element,
            StructuralTypeShape::PrimitiveScalar(scalar(IntegerSign::Unsigned, 16)),
        ),
        declaration(
            array,
            StructuralTypeShape::FixedArray { element, length: 2 },
        ),
    ];
    let entry = |field: &str, placement| LayoutFieldEntryReport {
        field: field.into(),
        member_identity: None,
        placement,
    };
    // Deliberately neither declaration order nor byte order. Every accessor
    // is inaccessible; complete initialization must still encode its field.
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 0,
        offsets: None,
        size: Some(32),
        align: 1,
        entries: vec![
            entry(
                "signed",
                LayoutPlacementReport::IntegerAt {
                    offset: 4,
                    stored_width: 24,
                    interpretation: IntegerInterpretation::Signed,
                },
            ),
            entry("plain", LayoutPlacementReport::At { offset: 20 }),
            entry("array", LayoutPlacementReport::At { offset: 16 }),
            entry(
                "small",
                LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 8,
                    destination_lsb: 2,
                    source_lsb: 0,
                    width: 3,
                },
            ),
            entry("array", LayoutPlacementReport::At { offset: 12 }),
            entry(
                "small",
                LayoutPlacementReport::Bits {
                    container: 3,
                    container_width: 8,
                    destination_lsb: 1,
                    source_lsb: 3,
                    width: 5,
                },
            ),
        ],
    };
    let access = AccessPlan::inaccessible(&layout).unwrap();
    let placement = validate_placement_plan(PlacementPlan {
        layout,
        access,
        reach: BoundaryReach::default(),
    })
    .unwrap();
    let image_fields = placement
        .access()
        .plan()
        .entries()
        .iter()
        .map(|entry| {
            StructuralFieldId::new(match entry.field() {
                "plain" => 1,
                "small" => 2,
                "signed" => 3,
                "array" => 4,
                name => panic!("unexpected fixture slot {name}"),
            })
            .unwrap()
        })
        .collect();
    let plan = TerminalPlacementLifecyclePlan {
        boundary: BoundaryMachineId::new(1).unwrap(),
        operation: TerminalPlacementLifecycleOperation::Initialize,
        execution: TerminalPlacementLifecycleExecution::CompilerPrimitive {
            profile_type: None,
            profile_requirement: None,
        },
        // These identities are deliberately inert here: a codec test grants
        // neither an extent nor a selected-provider settlement.
        storage: TerminalPlacementStorage {
            carrier: StructuralTypeId::new(4).unwrap(),
            base: StructuralFieldId::new(5).unwrap(),
            length: StructuralFieldId::new(6).unwrap(),
            granted: StructuralDomainId::new(1).unwrap(),
            vacant: Some(StructuralDomainId::new(2).unwrap()),
            resident: Some(StructuralDomainId::new(3).unwrap()),
        },
        schema,
        image_fields,
        policy_identity: TerminalIdentityIndex(10),
        policy_plan_machine_identity: TerminalIdentityIndex(11),
        schema_identity: TerminalIdentityIndex(12),
        placement,
    };
    (types, plan)
}

#[test]
fn resident_complete_image_stages_commits_and_decodes_without_touching_rejected_backing() {
    let (types, plan) = image_fixture();
    for target in targets() {
        for supplied in [0, 1] {
            with_builder(target, |builder, row| {
                let owner = PlaceId::new(9).unwrap();
                let scratch = owner;
                let output = PlaceId::new(11).unwrap();
                let value_pointer = fragmented_scalar::constant(builder, row, owner, 0).unwrap();
                let value = ImageHome {
                    storage: fragmented_scalar::StorageAccess::Place(&[]),
                    place: owner,
                    pointer: value_pointer,
                    byte_offset: 5,
                };
                let mut staged = ImageHome::scratch(builder, row, scratch, 35).unwrap();
                staged.byte_offset = 3;
                initialization::stage_image(&types, builder, row, &plan, value, staged).unwrap();
                let backing = fragmented_scalar::constant(builder, row, owner, 128).unwrap();
                let available = fragmented_scalar::constant(builder, row, owner, 48).unwrap();
                let supply = fragmented_scalar::constant(builder, row, owner, supplied).unwrap();
                let decision =
                    decide_constants(builder, row, owner, backing, available, 32, 1, 0, supply)
                        .unwrap();
                let staged_start = fragmented_scalar::constant(builder, row, scratch, 67).unwrap();
                initialization::commit(builder, row, owner, staged_start, backing, &decision)
                    .unwrap();
                if supplied == 1 {
                    let output_pointer =
                        fragmented_scalar::constant(builder, row, output, 192).unwrap();
                    transfer_image(
                        &types,
                        row,
                        builder,
                        &plan,
                        ImageHome {
                            storage: fragmented_scalar::StorageAccess::Place(&[]),
                            place: owner,
                            pointer: backing,
                            byte_offset: 0,
                        },
                        ImageHome {
                            storage: fragmented_scalar::StorageAccess::Place(&[]),
                            place: output,
                            pointer: output_pointer,
                            byte_offset: 0,
                        },
                        ImageDirection::Decode,
                    )
                    .unwrap();
                }
                let mut bytes = [0xa5; 240];
                bytes[5..13].copy_from_slice(&0x0123_4567_89ab_cdef_u64.to_le_bytes());
                bytes[13] = 0b1101_0101;
                // Canonical geometry aligns fields; declaration adjacency is
                // not byte adjacency, even in StructuralRecordStorage::Packed.
                bytes[21..29].copy_from_slice(&(-128_i64).to_le_bytes());
                bytes[29..33].copy_from_slice(&[0x34, 0x12, 0x78, 0x56]);
                let original = bytes;
                let mut registers = vec![0; builder.registers.len()];
                let frame = [(
                    selected_instructions::FrameStorageSlotId::Local(
                        selected_instructions::LocalStorageSlotId::Boundary {
                            operation: row.operation,
                        },
                    ),
                    64,
                )];
                execute_with_frame(&builder.instructions, &mut registers, &mut bytes, &frame);
                let mut expected_image = [0; 32];
                expected_image[0] = (0b1101_0101 & 7) << 2;
                expected_image[3] = (0b1101_0101 >> 3) << 1;
                expected_image[4..7].copy_from_slice(&[0x80, 0xff, 0xff]);
                expected_image[12..14].copy_from_slice(&original[29..31]);
                expected_image[16..18].copy_from_slice(&original[31..33]);
                expected_image[20..28].copy_from_slice(&original[5..13]);
                assert_eq!(&bytes[67..99], &expected_image);
                assert_eq!(&bytes[..67], &original[..67], "input must remain unchanged");
                assert_eq!(&bytes[99..128], &original[99..128]);
                assert_eq!(
                    &bytes[160..192],
                    &original[160..192],
                    "allocation tail must remain unchanged"
                );
                if supplied == 1 {
                    assert_eq!(&bytes[128..160], &expected_image);
                    assert_eq!(&bytes[192..200], &original[5..13]);
                    assert_eq!(bytes[200], original[13]);
                    assert_eq!(&bytes[201..208], &original[201..208]);
                    assert_eq!(&bytes[208..220], &original[21..33]);
                    assert_eq!(&bytes[220..], &original[220..]);
                } else {
                    assert_eq!(&bytes[192..], &original[192..]);
                    assert_eq!(&bytes[128..160], &original[128..160]);
                }
            });
        }
    }
}

#[test]
fn resident_authored_value_codec_keeps_signed_fields_fragments_and_array_strides() {
    use semantic_vocabulary::{AuthoredRecordStorage, IntegerSign, IntegerType, ScalarType};
    use terminal_psi::StructuralRecordStorage;
    let (mut types, plan) = image_fixture();
    let StructuralTypeShape::Record { fields, storage } = &mut types[0].shape else {
        panic!("fixture record");
    };
    *storage = StructuralRecordStorage::Authored(AuthoredRecordStorage {
        byte_size: 40,
        byte_alignment: 1,
    });
    fields[0].storage = Some(AuthoredFieldStorage::Direct { byte_offset: 0 });
    fields[1].storage = Some(AuthoredFieldStorage::Fragmented(AuthoredBitFragments::new(
        [AuthoredBitFragment {
            container_byte_offset: 8,
            container_width_bits: 16,
            destination_lsb: 3,
            source_lsb: 0,
            width: 8,
        }],
    )));
    fields[2].storage = Some(AuthoredFieldStorage::Scalar {
        byte_offset: 12,
        stored_scalar: Some(ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        )),
    });
    fields[3].storage = Some(AuthoredFieldStorage::Repeated {
        byte_offset: 16,
        element_stride: 8,
    });
    for target in targets() {
        with_builder(target, |builder, row| {
            let owner = PlaceId::new(9).unwrap();
            let scratch = owner;
            let result = PlaceId::new(11).unwrap();
            let value = ImageHome {
                storage: fragmented_scalar::StorageAccess::Place(&[]),
                place: owner,
                pointer: fragmented_scalar::constant(builder, row, owner, 0).unwrap(),
                byte_offset: 0,
            };
            let staged = ImageHome::scratch(builder, row, scratch, 32).unwrap();
            initialization::stage_image(&types, builder, row, &plan, value, staged).unwrap();
            let staged_end = builder.instructions.len();
            let restored = ImageHome {
                storage: fragmented_scalar::StorageAccess::Place(&[]),
                place: result,
                pointer: fragmented_scalar::constant(builder, row, result, 128).unwrap(),
                byte_offset: 0,
            };
            transfer_image(
                &types,
                row,
                builder,
                &plan,
                staged,
                restored,
                ImageDirection::Decode,
            )
            .unwrap();
            let mut bytes = [0xa5; 192];
            bytes[..8].copy_from_slice(&0x0123_4567_89ab_cdef_u64.to_le_bytes());
            let original_container = 0xa5a5_u16;
            let input_container = (original_container & !(0xff << 3)) | (0xd5 << 3);
            bytes[8..10].copy_from_slice(&input_container.to_le_bytes());
            bytes[12] = 0x80;
            bytes[16..18].copy_from_slice(&[0x34, 0x12]);
            bytes[24..26].copy_from_slice(&[0x78, 0x56]);
            let original = bytes;
            let mut registers = vec![0; builder.registers.len()];
            let frame = [(
                selected_instructions::FrameStorageSlotId::Local(
                    selected_instructions::LocalStorageSlotId::Boundary {
                        operation: row.operation,
                    },
                ),
                64,
            )];
            execute_with_frame(
                &builder.instructions[..staged_end],
                &mut registers,
                &mut bytes,
                &frame,
            );
            assert_eq!(
                &bytes[68..71],
                &[0x80, 0xff, 0xff],
                "signed input must extend before encoding"
            );
            assert_eq!(&bytes[76..78], &[0x34, 0x12]);
            assert_eq!(&bytes[80..82], &[0x78, 0x56]);
            assert_eq!(&bytes[..64], &original[..64]);
            assert_eq!(&bytes[96..], &original[96..]);
            // Extraction observes the current image, not the original value.
            bytes[68] = 0x81;
            execute(
                &builder.instructions[staged_end..],
                &mut registers,
                &mut bytes,
            );
            assert_eq!(&bytes[128..136], &original[..8]);
            assert_eq!(&bytes[136..138], &input_container.to_le_bytes());
            assert_eq!(bytes[140], 0x81);
            assert_eq!(&bytes[144..146], &original[16..18]);
            assert_eq!(&bytes[152..154], &original[24..26]);
            for position in [
                138, 139, 141, 142, 143, 146, 147, 148, 149, 150, 151, 154, 167, 168, 191,
            ] {
                assert_eq!(
                    bytes[position], original[position],
                    "unrelated byte {position}"
                );
            }
        });
    }
}

#[test]
fn resident_initialization_rejects_before_copy_and_commits_only_the_complete_image() {
    // base, length, supply predicate, outcome tag, failure tag. Rejection
    // precedence is intentional: range wins over supply, then alignment.
    let cases = [
        (16, 16, 1, 0, 1),
        (17, 16, 1, 1, 1),
        (16, 16, 0, 1, 2),
        (17, 16, 0, 1, 2),
        (u64::MAX, 0, 0, 1, 0),
        (16, 7, 1, 1, 0),
        (16, 16, 2, 1, 2),
    ];
    for target in targets() {
        for (base, length, supply, outcome, reason) in cases {
            with_builder(target, |builder, row| {
                let owner = PlaceId::new(9).unwrap();
                let staged = fragmented_scalar::constant(builder, row, owner, 0).unwrap();
                let backing = fragmented_scalar::constant(builder, row, owner, base).unwrap();
                let available = fragmented_scalar::constant(builder, row, owner, length).unwrap();
                let supply = fragmented_scalar::constant(builder, row, owner, supply).unwrap();
                let decision =
                    decide_constants(builder, row, owner, backing, available, 8, 8, 0, supply)
                        .unwrap();
                initialization::commit(builder, row, owner, staged, backing, &decision).unwrap();
                let mut bytes = [0xa5; 40];
                bytes[..8].copy_from_slice(&0x0123_4567_89ab_cdef_u64.to_le_bytes());
                let original = bytes;
                let mut registers = vec![0; builder.registers.len()];
                execute(&builder.instructions, &mut registers, &mut bytes);
                assert_eq!(registers[decision.outcome_tag.0 as usize], outcome);
                assert_eq!(registers[decision.failure_tag.0 as usize], reason);
                assert_eq!(
                    registers[decision.ready.0 as usize],
                    u64::from(outcome == 0)
                );
                if outcome == 0 {
                    assert_eq!(&bytes[16..24], &original[..8]);
                    assert_eq!(&bytes[..16], &original[..16]);
                    assert_eq!(
                        &bytes[24..],
                        &original[24..],
                        "allocation tail must remain untouched"
                    );
                    assert_eq!(registers[decision.copy_length.0 as usize], 8);
                } else {
                    assert_eq!(
                        bytes, original,
                        "rejection must preserve both input and backing"
                    );
                    assert_eq!(registers[decision.copy_length.0 as usize], 0);
                }
            });
        }
    }
}

#[test]
fn resident_initialization_geometry_handles_high_addresses_and_nonzero_residue() {
    for (base, modulus, residue) in [(u64::MAX - 7, 8, 0), (u64::MAX - 3, 8, 4), (u64::MAX, 1, 0)] {
        with_builder(NativeTarget::macos_arm64(), |builder, row| {
            let owner = PlaceId::new(9).unwrap();
            let base = fragmented_scalar::constant(builder, row, owner, base).unwrap();
            let available = fragmented_scalar::constant(builder, row, owner, u64::MAX).unwrap();
            let supply = fragmented_scalar::constant(builder, row, owner, 1).unwrap();
            let decision = decide_constants(
                builder, row, owner, base, available, 8, modulus, residue, supply,
            )
            .unwrap();
            let mut registers = vec![0; builder.registers.len()];
            execute(&builder.instructions, &mut registers, &mut []);
            assert_eq!(registers[decision.ready.0 as usize], 1);
            assert_eq!(registers[decision.copy_length.0 as usize], 8);
            for (modulus, residue) in [(0, 0), (3, 0), (8, 8)] {
                let decision = decide_constants(
                    builder, row, owner, base, available, 8, modulus, residue, supply,
                )
                .unwrap();
                let mut registers = vec![0; builder.registers.len()];
                execute(&builder.instructions, &mut registers, &mut []);
                assert_eq!(registers[decision.ready.0 as usize], 0);
                assert_eq!(registers[decision.copy_length.0 as usize], 0);
            }
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn decide_constants(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    owner: PlaceId,
    base: VirtualRegisterId,
    available: VirtualRegisterId,
    image_length: u64,
    modulus: u64,
    residue: u64,
    supply: VirtualRegisterId,
) -> Result<outcome::PlacementDecision, SelectedInstructionError> {
    let modulus = fragmented_scalar::constant(builder, row, owner, modulus)?;
    let residue = fragmented_scalar::constant(builder, row, owner, residue)?;
    outcome::decide(
        builder,
        row,
        owner,
        base,
        available,
        image_length,
        modulus,
        residue,
        supply,
    )
}
