//! Exercise the production lifecycle emitter, including original operand
//! projection and complete outcome storage. This bounded selected-instruction
//! test does not replace provider admission or native machine execution.

use super::*;
use abstract_operations::{PlacementLifecycleOperands, StructuralTypeCatalog};
use calling_conventions::{CallPlan, CallingPolicy, EntryControl, RegisterSet, ValuePlacement};
use legalized_operations::{
    LegalizedCallUnitParameter, LegalizedScalarFunction, LegalizedScalarInstructionKind,
    LegalizedStructuralContract,
};
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};
use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralDomainId};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, TerminalPlacementLifecycleExecution,
    TerminalPlacementLifecycleOperation, TerminalPlacementLifecyclePlan, TerminalPlacementStorage,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputProjection {
    Root,
    Field,
    Indexed,
}

pub(super) fn call_plan() -> CallPlan {
    CallPlan {
        policy: CallingPolicy::MicrosoftX64,
        parameters: vec![],
        result: None,
        callback_materializations: vec![],
        ordinary_clobbers: RegisterSet::new([]),
        stack_alignment: 16,
        shadow_bytes: 0,
        entry_control: EntryControl::CallReturn,
    }
}

fn aligned_placement() -> access_plans::ValidatedPlacementPlan {
    use access_plans::{
        AccessExposure, AccessPlan, FieldAccess, PlacementPlan, validate_placement_plan,
    };
    let mut layout = placement(
        0,
        FieldAccess::Stable {
            transfer_width_bits: 64,
            read: true,
            write: true,
            exposure: AccessExposure::Exported,
        },
    )
    .layout()
    .clone();
    layout.align = 8;
    let mut access = AccessPlan::inaccessible(&layout).unwrap();
    access
        .set(
            access.entries()[0].key(),
            FieldAccess::Stable {
                transfer_width_bits: 64,
                read: true,
                write: true,
                exposure: AccessExposure::Exported,
            },
        )
        .unwrap();
    validate_placement_plan(PlacementPlan {
        layout,
        access,
        reach: BoundaryReach::default(),
    })
    .unwrap()
}

#[test]
fn initializer_emits_original_inputs_complete_image_and_typed_outcomes() {
    let mut types = Types::default();
    let profile_type = types.profile();
    let word = types.add(StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    )));
    let owner = types.record(&[word, word, word]);
    let schema = types.record(&[word]);
    let returned = types.record(&[owner, schema]);
    let reason = types.sum(&[&[word, word], &[word, word], &[]]);
    let result_type = types.sum(&[&[owner], &[returned, reason]]);
    let owner_wrapper = types.record(&[word, owner]);
    let value_wrapper = types.record(&[word, schema]);
    let owner_array = types.add(StructuralTypeShape::FixedArray {
        element: owner,
        length: 2,
    });
    let value_array = types.add(StructuralTypeShape::FixedArray {
        element: schema,
        length: 3,
    });
    let borrowed_profile = types.add(StructuralTypeShape::Reference {
        referent: profile_type,
        access: StructuralAccess::SharedBorrow,
    });
    types.expose_scalar_fields();
    let profile_layout = ProfileLayout::derive(&types.0, profile_type).unwrap();
    let boundary = BoundaryMachineId::new(8).unwrap();
    let plan = TerminalPlacementLifecyclePlan {
        boundary,
        operation: TerminalPlacementLifecycleOperation::Initialize,
        execution: TerminalPlacementLifecycleExecution::CompilerPrimitive {
            profile_type: Some(profile_type),
            profile_requirement: None,
        },
        storage: TerminalPlacementStorage {
            carrier: owner,
            base: StructuralFieldId::new(3).unwrap(),
            length: StructuralFieldId::new(2).unwrap(),
            granted: StructuralDomainId::new(1).unwrap(),
            vacant: Some(StructuralDomainId::new(2).unwrap()),
            resident: Some(StructuralDomainId::new(3).unwrap()),
        },
        schema,
        image_fields: vec![StructuralFieldId::new(1).unwrap()],
        policy_identity: TerminalIdentityIndex(10),
        policy_plan_machine_identity: TerminalIdentityIndex(11),
        schema_identity: TerminalIdentityIndex(12),
        placement: aligned_placement(),
    };
    for target in targets() {
        for projection in [
            InputProjection::Root,
            InputProjection::Field,
            InputProjection::Indexed,
        ] {
            with_builder(target, |builder, original| {
                let projected = projection != InputProjection::Root;
                let owner_place = PlaceId::new(1).unwrap();
                let value_place = PlaceId::new(2).unwrap();
                let profile_place = PlaceId::new(3).unwrap();
                let result_place = PlaceId::new(4).unwrap();
                let mut row = original.clone();
                let argument = |place, access, path| StructuralArgument {
                    place,
                    access,
                    path,
                };
                let field_path = if projection == InputProjection::Field {
                    vec![StructuralPathSegment::Field(TerminalIdentityIndex(2))]
                } else {
                    vec![]
                };
                let index =
                    |number, stride, extent| legalized_operations::LegalizedRuntimeIndexOperand {
                        operand: abstract_operations::AbstractResult {
                            value: semantic_vocabulary::ValueId::new(number).unwrap(),
                            scalar_type: ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                            ),
                        },
                        stride,
                        extent: Some(extent),
                        obligation: semantic_vocabulary::ObligationId::new(number).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                                b"initializer-index",
                            ),
                    };
                let selectors = if projection == InputProjection::Indexed {
                    vec![index(7, 24, 2), index(8, 8, 3)]
                } else {
                    vec![]
                };
                let path = |position: usize| {
                    selectors.get(position).map_or_else(
                        || field_path.clone(),
                        |selector| {
                            vec![StructuralPathSegment::RuntimeIndex {
                                index: selector.operand.value,
                                obligation: selector.obligation,
                            }]
                        },
                    )
                };
                row.kind = LegalizedScalarInstructionKind::PlacementLifecycle {
                    boundary,
                    operands: PlacementLifecycleOperands::Initialize {
                        storage: argument(owner_place, StructuralAccess::Owned, path(0)),
                        value: argument(value_place, StructuralAccess::Owned, path(1)),
                        profile: argument(
                            profile_place,
                            StructuralAccess::SharedBorrow,
                            if projected {
                                vec![StructuralPathSegment::Referent]
                            } else {
                                vec![]
                            },
                        ),
                    },
                    indices: selectors.clone(),
                    result: StructuralOperationResult {
                        place: result_place,
                        structural_type: result_type,
                        multiplicity: StructuralMultiplicity::Linear,
                        qualifications: vec![],
                        projected_qualifications: vec![],
                        qualification_establishments: vec![],
                        claims: vec![],
                    },
                };
                let parameter_types = [
                    match projection {
                        InputProjection::Root => owner,
                        InputProjection::Field => owner_wrapper,
                        InputProjection::Indexed => owner_array,
                    },
                    match projection {
                        InputProjection::Root => schema,
                        InputProjection::Field => value_wrapper,
                        InputProjection::Indexed => value_array,
                    },
                    if projected {
                        borrowed_profile
                    } else {
                        profile_type
                    },
                ];
                let parameters = [owner_place, value_place, profile_place]
                    .into_iter()
                    .zip(parameter_types)
                    .enumerate()
                    .map(|(position, (place, structural_type))| {
                        let shape = crate::structural_inputs::structural_reference_input::shape(
                            structural_type,
                            &types.0,
                        )
                        .unwrap();
                        let semantic = StructuralParameterDeclaration {
                            place,
                            position: position as u32,
                            structural_type,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            access: StructuralAccess::Owned,
                            qualifications: vec![],
                            projected_qualifications: vec![],
                        };
                        LegalizedCallUnitParameter {
                            target: target_operations::TargetStructuralParameter {
                                place,
                                structural_type,
                                multiplicity: semantic.multiplicity,
                                access: semantic.access,
                                projected_qualifications: vec![],
                                shape,
                                placement: ValuePlacement {
                                    shape,
                                    locations: vec![],
                                },
                            },
                            semantic,
                        }
                    })
                    .collect();
                let source = LegalizedScalarFunction {
                    machine: MachineId::new(1).unwrap(),
                    attachment: None,
                    nominal_cleanup_subject: None,
                    provenance: target_operations::TerminalPsiProvenance {
                        operations: vec![row.operation],
                        edges: vec![],
                    },
                    call_plan: call_plan(),
                    parameters: vec![],
                    structural: Some(LegalizedStructuralContract {
                        result: None,
                        structural_types: StructuralTypeCatalog::from(types.0.clone())
                            .with_placement_lifecycles(vec![plan.clone()])
                            .unwrap(),
                        parameters,
                        structural_places: vec![],
                        entry_claims: vec![],
                        published_service_ceiling: vec![],
                    }),
                    entry_block: BlockId::new(7).unwrap(),
                    blocks: vec![],
                    ownership_frontier_facts: vec![],
                };
                builder.open_block = Some(operation_control::OpenBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(source.entry_block),
                    start: 0,
                });
                builder.next_block = 1;
                for (position, selector) in selectors.iter().enumerate() {
                    let register = fragmented_scalar::constant(
                        builder,
                        &row,
                        value_place,
                        position as u64 + 1,
                    )
                    .unwrap();
                    builder.definitions.push((
                        selector.operand.value,
                        register,
                        optimization_unit::ValueDefinitionSite::FunctionParameter(position as u32),
                        selector.operand.scalar_type,
                    ));
                }
                let arena = (u64::from(profile_layout.bytes) + 255) & !255;
                for (place, address) in [
                    (owner_place, arena),
                    (value_place, arena + 64),
                    (profile_place, if projected { arena + 128 } else { 0 }),
                ] {
                    let pointer =
                        fragmented_scalar::constant(builder, &row, place, address).unwrap();
                    builder.transport.pointers.push((place, pointer));
                }
                super::super::super::emit(&source, &row, builder).unwrap();
                let backing_writes = builder
                    .transport
                    .memory
                    .iter()
                    .filter(|access| {
                        access.role == selected_instructions::SelectedMemoryAccessRole::WriteBacking
                    })
                    .collect::<Vec<_>>();
                let [write] = backing_writes.as_slice() else {
                    panic!("initialization must declare one complete backing write");
                };
                assert_eq!(
                    (write.place, write.byte_offset, write.byte_count),
                    (owner_place, 0, 16)
                );
                assert!(builder.transport.memory.iter().any(|read| {
                    read.instruction == write.instruction
                        && read.role
                            == selected_instructions::SelectedMemoryAccessRole::ReadLocal {
                                slot: LocalStorageSlotId::Boundary {
                                    operation: row.operation,
                                },
                            }
                        && read.byte_offset
                            == super::super::compatibility::COMPATIBILITY_SCRATCH_BYTES
                        && read.byte_count == write.byte_count
                }));
                let frame = [
                    (
                        FrameStorageSlotId::Local(LocalStorageSlotId::Structural {
                            operation: row.operation,
                            place: result_place,
                        }),
                        arena + 256,
                    ),
                    (
                        FrameStorageSlotId::Local(LocalStorageSlotId::Boundary {
                            operation: row.operation,
                        }),
                        arena + 512,
                    ),
                ];
                assert_eq!(builder.transport.local_slots.len(), 2);
                let result_pointer = builder
                    .transport
                    .pointers
                    .iter()
                    .find(|(place, _)| *place == result_place)
                    .unwrap()
                    .1;
                for outcome in 0..5 {
                    let available = if outcome == 1 { 8 } else { 64 };
                    let mut supplied = region(0, available);
                    if outcome == 2 {
                        supplied.stable = StableCapability::Read;
                    }
                    let mut bytes = encode(
                        &profile_layout,
                        &ResourceProfile {
                            regions: vec![supplied],
                        },
                    );
                    bytes.resize((arena + 2048) as usize, 0xa5);
                    if outcome == 4 {
                        store(&mut bytes, 0, profile_layout.regions.count, 33);
                    }
                    let owner_start = arena as usize
                        + match projection {
                            InputProjection::Root => 0,
                            InputProjection::Field => 8,
                            InputProjection::Indexed => 24,
                        };
                    let value_start = arena as usize
                        + 64
                        + match projection {
                            InputProjection::Root => 0,
                            InputProjection::Field => 8,
                            InputProjection::Indexed => 16,
                        };
                    let backing = arena + 1024 + u64::from(outcome == 3);
                    bytes[owner_start..owner_start + 8].copy_from_slice(&0xfeed_u64.to_le_bytes());
                    bytes[owner_start + 8..owner_start + 16]
                        .copy_from_slice(&available.to_le_bytes());
                    bytes[owner_start + 16..owner_start + 24]
                        .copy_from_slice(&backing.to_le_bytes());
                    bytes[value_start..value_start + 8].copy_from_slice(&17_u64.to_le_bytes());
                    bytes[arena as usize + 128..arena as usize + 136]
                        .copy_from_slice(&0_u64.to_le_bytes());
                    let original = bytes.clone();
                    let registers = execute_operation(builder, &mut bytes, &frame);
                    let backing = backing as usize;
                    assert_eq!(
                        &bytes[..arena as usize + 256],
                        &original[..arena as usize + 256]
                    );
                    assert_eq!(&bytes[backing + 16..], &original[backing + 16..]);
                    if outcome == 4 {
                        assert!(
                            registers.is_none(),
                            "malformed supply must not return Rejected"
                        );
                        assert_eq!(
                            &bytes[backing..backing + 16],
                            &original[backing..backing + 16]
                        );
                        continue;
                    }
                    let registers = registers.unwrap();
                    let result_start = registers[result_pointer.0 as usize] as usize;
                    let read = |offset| {
                        u64::from_le_bytes(
                            bytes[result_start + offset..result_start + offset + 8]
                                .try_into()
                                .unwrap(),
                        )
                    };
                    assert_eq!(read(0) & 0xffff_ffff, u64::from(outcome != 0));
                    assert_eq!(
                        &bytes[result_start + 8..result_start + 32],
                        &original[owner_start..owner_start + 24]
                    );
                    if outcome == 0 {
                        assert_eq!(&bytes[backing..backing + 8], &17_u64.to_le_bytes());
                        assert_eq!(&bytes[backing + 8..backing + 16], &[0; 8]);
                    } else {
                        assert_eq!(read(32), 17);
                        assert_eq!(
                            read(40) & 0xffff_ffff,
                            match outcome {
                                1 => 0,
                                2 => 2,
                                _ => 1,
                            }
                        );
                        if outcome == 1 {
                            assert_eq!((read(48), read(56)), (16, available));
                        }
                        if outcome == 3 {
                            assert_eq!((read(48), read(56)), (8, 0));
                        }
                        assert_eq!(
                            &bytes[backing..backing + 16],
                            &original[backing..backing + 16]
                        );
                    }
                }
                if projection == InputProjection::Indexed {
                    for missing in [false, true] {
                        let mut invalid = row.clone();
                        let LegalizedScalarInstructionKind::PlacementLifecycle { indices, .. } =
                            &mut invalid.kind
                        else {
                            unreachable!()
                        };
                        if missing {
                            indices.pop();
                        } else {
                            indices.swap(0, 1);
                        }
                        with_builder(target, |builder, _| {
                            let error =
                                super::super::super::emit(&source, &invalid, builder).unwrap_err();
                            assert!(format!("{error:?}").contains("selector"));
                        });
                    }
                }
            });
        }
    }
}
