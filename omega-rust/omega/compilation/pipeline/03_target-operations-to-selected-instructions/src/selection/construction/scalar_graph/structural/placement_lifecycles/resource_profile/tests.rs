//! Execute emitted bounded validation and compare with the canonical normalizer.
//! These checks intentionally do not claim native placement or backing admission.

use super::super::tests::{execute_operation, targets, with_builder};
use super::*;
use access_plans::{
    AtomicCapability, AtomicPermissions, AtomicTransferRule, BoundaryReach, BoundaryServiceReachId,
    ExternalCapability, ExternalReadBehavior, PeerWritability, ResourceProfile, ResourceRegion,
    StableCapability, TransferRule,
};
use selected_instructions::{SelectedBlockId, SelectedBlockOrigin};
use semantic_vocabulary::{
    BlockId, IntegerSign, IntegerType, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId,
    StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
};

mod initialization;

#[derive(Default)]
struct Types(Vec<StructuralTypeDeclaration>);

impl Types {
    fn expose_scalar_fields(&mut self) {
        let scalars: Vec<_> = self
            .0
            .iter()
            .filter_map(|declaration| match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => Some((declaration.id, scalar)),
                _ => None,
            })
            .collect();
        let lower = |field: &mut StructuralFieldDeclaration| {
            if let Some((_, scalar)) = scalars
                .iter()
                .find(|(identity, _)| field.field_type.as_structural() == Some(*identity))
            {
                field.field_type = StructuralFieldType::Scalar(*scalar);
            }
        };
        for declaration in &mut self.0 {
            match &mut declaration.shape {
                StructuralTypeShape::Record { fields, .. } => fields.iter_mut().for_each(lower),
                StructuralTypeShape::Sum { cases } => cases
                    .iter_mut()
                    .flat_map(|case| &mut case.fields)
                    .for_each(lower),
                _ => {}
            }
        }
    }
    fn add(&mut self, shape: StructuralTypeShape) -> StructuralTypeId {
        let id = StructuralTypeId::new((self.0.len() + 1).try_into().unwrap()).unwrap();
        self.0.push(StructuralTypeDeclaration {
            quotient_carrier: None,
            id,
            identity: TerminalIdentityIndex(id.get().try_into().unwrap()),
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape,
        });
        id
    }

    fn fields(types: &[StructuralTypeId]) -> Vec<StructuralFieldDeclaration> {
        types
            .iter()
            .enumerate()
            .map(|(position, field_type)| StructuralFieldDeclaration {
                id: StructuralFieldId::new((position + 1).try_into().unwrap()).unwrap(),
                identity: TerminalIdentityIndex((position + 1).try_into().unwrap()),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(*field_type),
                storage: None,
            })
            .collect()
    }

    fn record(&mut self, types: &[StructuralTypeId]) -> StructuralTypeId {
        self.add(StructuralTypeShape::Record {
            fields: Self::fields(types),
            storage: StructuralRecordStorage::Packed,
        })
    }

    fn sum(&mut self, payloads: &[&[StructuralTypeId]]) -> StructuralTypeId {
        self.add(StructuralTypeShape::Sum {
            cases: payloads
                .iter()
                .enumerate()
                .map(|(position, payload)| StructuralCaseDeclaration {
                    id: StructuralCaseId::new((position + 1).try_into().unwrap()).unwrap(),
                    identity: TerminalIdentityIndex((position + 1).try_into().unwrap()),
                    fields: Self::fields(payload),
                })
                .collect(),
        })
    }

    fn counted(
        &mut self,
        element: StructuralTypeId,
        count: StructuralTypeId,
        capacity: u64,
    ) -> StructuralTypeId {
        let array = self.add(StructuralTypeShape::FixedArray {
            element,
            length: capacity,
        });
        self.record(&[array, count])
    }

    fn profile(&mut self) -> StructuralTypeId {
        let word = self.add(StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        )));
        let boolean = self.add(StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean));
        let range = self.record(&[word, word]);
        let peer = self.sum(&[&[], &[]]);
        let stable = self.sum(&[&[], &[], &[], &[]]);
        let read = self.sum(&[&[], &[], &[]]);
        let transfer = self.record(&[word, word]);
        let rules = self.counted(transfer, word, 16);
        let external = self.sum(&[&[], &[read, boolean, rules]]);
        let operations = self.record(&[boolean; 12]);
        let atomic_transfer = self.record(&[transfer, operations]);
        let atomic_rules = self.counted(atomic_transfer, word, 16);
        let atomic = self.sum(&[&[], &[atomic_rules]]);
        let reach = self.counted(word, word, 32);
        let region = self.record(&[range, peer, stable, external, atomic, reach]);
        self.counted(region, word, 32)
    }
}

fn store(bytes: &mut [u8], base: usize, scalar: Scalar, value: u64) {
    let start = base + scalar.offset as usize;
    bytes[start..start + scalar.width as usize]
        .copy_from_slice(&value.to_le_bytes()[..scalar.width as usize]);
}

fn entry(base: usize, array: Counted, position: usize) -> usize {
    base + array.offset as usize + position * array.stride as usize
}

fn encode(layout: &ProfileLayout, profile: &ResourceProfile) -> Vec<u8> {
    // Inactive slots contain poison. Validation must never read them as active
    // payloads, or require their padding/inactive cases to be zero.
    let mut bytes = vec![0xd7; layout.bytes as usize + VALIDATION_SCRATCH_BYTES as usize + 16];
    store(
        &mut bytes,
        0,
        layout.regions.count,
        profile.regions.len() as u64,
    );
    for (position, region) in profile.regions.iter().enumerate() {
        let base = entry(0, layout.regions, position);
        store(&mut bytes, base, layout.start, region.offset);
        store(&mut bytes, base, layout.length, region.length);
        store(
            &mut bytes,
            base,
            layout.peer,
            match region.peer {
                PeerWritability::Exclusive => 0,
                PeerWritability::HostileShared => 1,
            },
        );
        store(
            &mut bytes,
            base,
            layout.stable,
            match region.stable {
                StableCapability::None => 0,
                StableCapability::Read => 1,
                StableCapability::Write => 2,
                StableCapability::ReadWrite => 3,
            },
        );
        match &region.external {
            ExternalCapability::None => store(&mut bytes, base, layout.external, 0),
            ExternalCapability::Access {
                read,
                write,
                transfers,
            } => {
                store(&mut bytes, base, layout.external, 1);
                store(
                    &mut bytes,
                    base,
                    layout.external_read,
                    match read {
                        ExternalReadBehavior::None => 0,
                        ExternalReadBehavior::Repeatable => 1,
                        ExternalReadBehavior::Destructive => 2,
                    },
                );
                store(&mut bytes, base, layout.external_write, u64::from(*write));
                store(
                    &mut bytes,
                    base,
                    layout.external_rules.count,
                    transfers.len() as u64,
                );
                for (position, rule) in transfers.iter().enumerate() {
                    let base = entry(base, layout.external_rules, position);
                    store(
                        &mut bytes,
                        base,
                        layout.external_transfer.width,
                        u64::from(rule.width_bits),
                    );
                    store(
                        &mut bytes,
                        base,
                        layout.external_transfer.alignment,
                        rule.alignment_bytes,
                    );
                }
            }
        }
        match &region.atomic {
            AtomicCapability::None => store(&mut bytes, base, layout.atomic, 0),
            AtomicCapability::Access { transfers } => {
                store(&mut bytes, base, layout.atomic, 1);
                store(
                    &mut bytes,
                    base,
                    layout.atomic_rules.count,
                    transfers.len() as u64,
                );
                for (position, rule) in transfers.iter().enumerate() {
                    let base = entry(base, layout.atomic_rules, position);
                    store(
                        &mut bytes,
                        base,
                        layout.atomic_transfer.width,
                        u64::from(rule.transfer.width_bits),
                    );
                    store(
                        &mut bytes,
                        base,
                        layout.atomic_transfer.alignment,
                        rule.transfer.alignment_bytes,
                    );
                    let operations = rule.operations;
                    for (field, value) in
                        layout.atomic_transfer.operations.unwrap().into_iter().zip([
                            operations.load,
                            operations.store,
                            operations.fetch_add,
                            operations.fetch_sub,
                            operations.fetch_xor,
                            operations.fetch_or,
                            operations.fetch_and,
                            operations.swap,
                            operations.compare_exchange,
                            operations.compare_exchange_once,
                            operations.try_exchange,
                            operations.try_exchange_once,
                        ])
                    {
                        store(&mut bytes, base, field, u64::from(value));
                    }
                }
            }
        }
        store(
            &mut bytes,
            base,
            layout.reach.count,
            region.reach.services().len() as u64,
        );
        for (position, service) in region.reach.services().enumerate() {
            store(
                &mut bytes,
                entry(base, layout.reach, position),
                layout.service,
                service.normalized_identity(),
            );
        }
    }
    bytes
}

fn run_values(builder: &Builder<'_>, bytes: &mut [u8], scratch_base: u64) -> Vec<u64> {
    let [slot] = builder.transport.local_slots.as_slice() else {
        panic!("profile assessment must own one real scratch allocation");
    };
    let frame = [(
        selected_instructions::FrameStorageSlotId::Local(slot.id),
        scratch_base,
    )];
    assert!(scratch_base + u64::from(slot.byte_size) <= bytes.len() as u64);
    let mut reads = 0;
    let mut writes = 0;
    for access in &builder.transport.memory {
        use selected_instructions::SelectedMemoryAccessRole;
        match access.role {
            SelectedMemoryAccessRole::ReadLocal { slot: owner }
            | SelectedMemoryAccessRole::WriteLocal { slot: owner }
            | SelectedMemoryAccessRole::AddressLocal { slot: owner } => {
                assert_eq!(owner, slot.id);
                assert!(access.byte_offset + access.byte_count <= slot.byte_size);
                reads += usize::from(matches!(
                    access.role,
                    SelectedMemoryAccessRole::ReadLocal { .. }
                ));
                writes += usize::from(matches!(
                    access.role,
                    SelectedMemoryAccessRole::WriteLocal { .. }
                ));
            }
            SelectedMemoryAccessRole::ReadPlace => {}
            unexpected => panic!("profile validation invented a source access: {unexpected:?}"),
        }
    }
    assert!(reads > 0 && writes > 0);
    execute_operation(builder, bytes, &frame).expect("assessment itself does not trap")
}

fn run(
    builder: &Builder<'_>,
    result: VirtualRegisterId,
    bytes: &mut [u8],
    scratch_base: u64,
) -> bool {
    let registers = run_values(builder, bytes, scratch_base);
    assert!(registers[result.0 as usize] <= 1);
    registers[result.0 as usize] == 1
}

fn with_validator(available: u64, test: impl Fn(&Builder<'_>, VirtualRegisterId, &ProfileLayout)) {
    let mut declarations = Types::default();
    let profile_type = declarations.profile();
    declarations.expose_scalar_fields();
    let layout = ProfileLayout::derive(&declarations.0, profile_type).unwrap();
    for target in targets() {
        with_builder(target, |builder, row| {
            builder.open_block = Some(operation_control::OpenBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(7).unwrap()),
                start: 0,
            });
            builder.next_block = 1;
            let place = PlaceId::new(19).unwrap();
            let pointer = fragmented_scalar::constant(builder, row, place, 0).unwrap();
            let profile = ImageHome {
                storage: fragmented_scalar::StorageAccess::Place(&[]),
                place,
                pointer,
                byte_offset: 0,
            };
            let scratch =
                ImageHome::scratch(builder, row, place, VALIDATION_SCRATCH_BYTES).unwrap();
            let available = fragmented_scalar::constant(builder, row, place, available).unwrap();
            let result = validate(
                &declarations.0,
                profile_type,
                builder,
                row,
                profile,
                scratch,
                available,
            )
            .unwrap();
            test(builder, result, &layout);
        });
    }
}

fn region(offset: u64, length: u64) -> ResourceRegion {
    ResourceRegion {
        offset,
        length,
        peer: PeerWritability::Exclusive,
        stable: StableCapability::ReadWrite,
        external: ExternalCapability::None,
        atomic: AtomicCapability::None,
        reach: BoundaryReach::from_services([
            BoundaryServiceReachId::from_normalized_identity(17).unwrap(),
            BoundaryServiceReachId::from_normalized_identity(29).unwrap(),
        ]),
    }
}

fn all_operations() -> AtomicPermissions {
    AtomicPermissions {
        load: true,
        store: true,
        fetch_add: true,
        fetch_sub: true,
        fetch_xor: true,
        fetch_or: true,
        fetch_and: true,
        swap: true,
        compare_exchange: true,
        compare_exchange_once: true,
        try_exchange: true,
        try_exchange_once: true,
    }
}

#[test]
fn runtime_profile_validation_matches_complete_normalizer() {
    let rules: Vec<_> = (1..=16)
        .rev()
        .map(|width| TransferRule {
            width_bits: width * 8,
            alignment_bytes: 1_u64 << (width % 8),
        })
        .collect();
    let mut rich = region(0, 256);
    rich.external = ExternalCapability::Access {
        read: ExternalReadBehavior::Repeatable,
        write: true,
        transfers: rules.clone(),
    };
    rich.atomic = AtomicCapability::Access {
        transfers: rules
            .iter()
            .map(|transfer| AtomicTransferRule {
                transfer: *transfer,
                operations: all_operations(),
            })
            .collect(),
    };
    let mut cases = vec![
        ResourceProfile::default(),
        ResourceProfile {
            regions: vec![rich.clone()],
        },
        ResourceProfile {
            regions: vec![region(128, 128), region(0, 128)],
        },
        ResourceProfile {
            regions: (0..32)
                .rev()
                .map(|position| region(position * 8, 8))
                .collect(),
        },
    ];
    for mutation in 0..15 {
        let mut changed = rich.clone();
        match mutation {
            0 => changed.length = 0,
            1 => changed.offset = u64::MAX,
            2 => changed.length = 257,
            3 => changed.peer = PeerWritability::HostileShared,
            4 => {
                changed.external = ExternalCapability::Access {
                    read: ExternalReadBehavior::None,
                    write: false,
                    transfers: rules.clone(),
                }
            }
            5 => {
                changed.external = ExternalCapability::Access {
                    read: ExternalReadBehavior::Destructive,
                    write: false,
                    transfers: vec![],
                }
            }
            6..=10 => {
                let mut transfers = rules.clone();
                match mutation {
                    6 => transfers[1] = transfers[0],
                    7 => transfers[0].width_bits = 7,
                    8 => transfers[0].width_bits = 136,
                    9 => transfers[0].alignment_bytes = 0,
                    _ => transfers[0].alignment_bytes = 3,
                }
                changed.external = ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: true,
                    transfers,
                };
            }
            11 => changed.atomic = AtomicCapability::Access { transfers: vec![] },
            12 => {
                changed.atomic = AtomicCapability::Access {
                    transfers: vec![AtomicTransferRule {
                        transfer: rules[0],
                        operations: AtomicPermissions::default(),
                    }],
                }
            }
            13 => {
                changed.stable = StableCapability::None;
                changed.external = ExternalCapability::None;
                changed.atomic = AtomicCapability::None;
            }
            _ => {
                changed.peer = PeerWritability::HostileShared;
                changed.stable = StableCapability::None;
            }
        }
        cases.push(ResourceProfile {
            regions: vec![changed],
        });
    }
    cases.push(ResourceProfile {
        regions: vec![region(0, 129), region(128, 128)],
    });
    with_validator(256, |builder, result, layout| {
        for (position, profile) in cases.iter().enumerate() {
            let expected = access_plans::validate_resource_profile(profile.clone(), 256).is_ok();
            let mut bytes = encode(layout, profile);
            let before = bytes[..layout.bytes as usize + 8].to_vec();
            assert_eq!(
                run(builder, result, &mut bytes, u64::from(layout.bytes) + 8),
                expected,
                "profile {position}"
            );
            assert_eq!(
                &bytes[..before.len()],
                before,
                "validation mutated its immutable input"
            );
            assert_eq!(&bytes[bytes.len() - 8..], &[0xd7; 8]);
        }
    });
}

#[test]
fn malformed_runtime_counts_tags_and_boolean_bits_never_enable_profile() {
    with_validator(256, |builder, result, layout| {
        let mut profile = region(0, 256);
        profile.external = ExternalCapability::Access {
            read: ExternalReadBehavior::Repeatable,
            write: true,
            transfers: vec![TransferRule {
                width_bits: 64,
                alignment_bytes: 8,
            }],
        };
        profile.atomic = AtomicCapability::Access {
            transfers: vec![AtomicTransferRule {
                transfer: TransferRule {
                    width_bits: 64,
                    alignment_bytes: 8,
                },
                operations: all_operations(),
            }],
        };
        let clean = encode(
            layout,
            &ResourceProfile {
                regions: vec![profile],
            },
        );
        let base = entry(0, layout.regions, 0);
        let atomic = entry(base, layout.atomic_rules, 0);
        let service = entry(base, layout.reach, 0);
        let mut faults = vec![
            (0, layout.regions.count, 33),
            (0, layout.regions.count, u64::MAX),
            (base, layout.peer, 2),
            (base, layout.stable, 4),
            (base, layout.external, 2),
            (base, layout.atomic, 2),
            (base, layout.external_read, 3),
            (base, layout.external_write, 2),
            (base, layout.external_rules.count, 17),
            (base, layout.atomic_rules.count, u64::MAX),
            (base, layout.reach.count, 33),
            (service, layout.service, 0),
        ];
        faults.extend(
            layout
                .atomic_transfer
                .operations
                .unwrap()
                .map(|field| (atomic, field, 2)),
        );
        for (base, scalar, value) in faults {
            let mut bytes = clean.clone();
            store(&mut bytes, base, scalar, value);
            assert!(!run(
                builder,
                result,
                &mut bytes,
                u64::from(layout.bytes) + 8
            ));
        }
    });
    with_validator(0, |builder, result, layout| {
        let mut bytes = encode(layout, &ResourceProfile::default());
        assert!(!run(
            builder,
            result,
            &mut bytes,
            u64::from(layout.bytes) + 8
        ));
    });
}

fn placement(
    offset: u64,
    access: access_plans::FieldAccess,
) -> access_plans::ValidatedPlacementPlan {
    use access_plans::{AccessPlan, PlacementPlan, validate_placement_plan};
    use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 0,
        offsets: None,
        size: Some(16),
        align: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "value".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset },
        }],
    };
    let mut plan = AccessPlan::inaccessible(&layout).unwrap();
    let key = plan.entries()[0].key();
    plan.set(key, access).unwrap();
    validate_placement_plan(PlacementPlan {
        layout,
        access: plan,
        reach: BoundaryReach::from_services([
            BoundaryServiceReachId::from_normalized_identity(17).unwrap()
        ]),
    })
    .unwrap()
}

#[test]
fn runtime_profile_compatibility_matches_normalized_coverage_and_congruence() {
    use access_plans::{AccessExposure, ExternalRead, FieldAccess};
    let exposure = AccessExposure::Exported;
    let placements = [
        placement(
            0,
            FieldAccess::Stable {
                transfer_width_bits: 64,
                read: true,
                write: true,
                exposure,
            },
        ),
        placement(
            3,
            FieldAccess::Stable {
                transfer_width_bits: 32,
                read: true,
                write: false,
                exposure,
            },
        ),
        placement(
            0,
            FieldAccess::External {
                transfer_width_bits: 64,
                read: ExternalRead::Read,
                write: true,
                exposure,
            },
        ),
        placement(
            0,
            FieldAccess::External {
                transfer_width_bits: 64,
                read: ExternalRead::Take,
                write: false,
                exposure,
            },
        ),
        placement(
            0,
            FieldAccess::Atomic {
                transfer_width_bits: 64,
                operations: AtomicPermissions {
                    load: true,
                    compare_exchange_once: true,
                    ..Default::default()
                },
                exposure,
            },
        ),
        placement(0, FieldAccess::Inaccessible),
    ];
    let rules = vec![
        TransferRule {
            width_bits: 64,
            alignment_bytes: 32,
        },
        TransferRule {
            width_bits: 32,
            alignment_bytes: 4,
        },
    ];
    let mut rich = region(0, 256);
    rich.external = ExternalCapability::Access {
        read: ExternalReadBehavior::Repeatable,
        write: true,
        transfers: rules.clone(),
    };
    rich.atomic = AtomicCapability::Access {
        transfers: rules
            .iter()
            .map(|rule| AtomicTransferRule {
                transfer: *rule,
                operations: all_operations(),
            })
            .collect(),
    };
    let split = |first: ResourceRegion, second: ResourceRegion| ResourceProfile {
        regions: vec![
            ResourceRegion {
                offset: 4,
                length: 252,
                ..second
            },
            ResourceRegion {
                offset: 0,
                length: 4,
                ..first
            },
        ],
    };
    let mut reversed = rich.clone();
    if let ExternalCapability::Access { transfers, .. } = &mut reversed.external {
        transfers.reverse();
    }
    if let AtomicCapability::Access { transfers } = &mut reversed.atomic {
        transfers.reverse();
    }
    let mut extra = rich.clone();
    extra.reach =
        BoundaryReach::from_services([
            BoundaryServiceReachId::from_normalized_identity(17).unwrap()
        ]);
    let mut destructive = rich.clone();
    if let ExternalCapability::Access { read, .. } = &mut destructive.external {
        *read = ExternalReadBehavior::Destructive;
    }
    let mut missing_write = rich.clone();
    missing_write.stable = StableCapability::Read;
    let mut missing_reach = rich.clone();
    missing_reach.reach = BoundaryReach::default();
    let mut malformed = rich.clone();
    malformed.length = 0;
    let mut missing_atomic_operation = rich.clone();
    if let AtomicCapability::Access { transfers } = &mut missing_atomic_operation.atomic {
        transfers[0].operations.compare_exchange_once = false;
    }
    let mut missing_atomic_width = rich.clone();
    if let AtomicCapability::Access { transfers } = &mut missing_atomic_width.atomic {
        transfers.remove(0);
    }
    let profiles = vec![
        ResourceProfile::default(),
        ResourceProfile {
            regions: vec![region(0, 256)],
        },
        ResourceProfile {
            regions: vec![rich.clone()],
        },
        split(rich.clone(), reversed),
        split(rich.clone(), extra),
        split(rich.clone(), destructive.clone()),
        ResourceProfile {
            regions: vec![destructive],
        },
        split(rich.clone(), missing_write),
        split(rich.clone(), missing_reach),
        ResourceProfile {
            regions: vec![malformed],
        },
        ResourceProfile {
            regions: vec![region(0, 4), region(5, 251)],
        },
        ResourceProfile {
            regions: (0..32)
                .rev()
                .map(|position| region(position * 8, 8))
                .collect(),
        },
        ResourceProfile {
            regions: vec![missing_atomic_operation.clone()],
        },
        ResourceProfile {
            regions: vec![missing_atomic_width],
        },
        split(rich.clone(), missing_atomic_operation),
    ];
    let mut declarations = Types::default();
    let profile_type = declarations.profile();
    declarations.expose_scalar_fields();
    let layout = ProfileLayout::derive(&declarations.0, profile_type).unwrap();
    for target in targets() {
        for (placement_position, placement) in placements.iter().enumerate() {
            with_builder(target, |builder, row| {
                builder.open_block = Some(operation_control::OpenBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(BlockId::new(7).unwrap()),
                    start: 0,
                });
                builder.next_block = 1;
                let place = PlaceId::new(19).unwrap();
                let pointer = fragmented_scalar::constant(builder, row, place, 0).unwrap();
                let profile = ImageHome {
                    storage: fragmented_scalar::StorageAccess::Place(&[]),
                    place,
                    pointer,
                    byte_offset: 0,
                };
                let scratch = ImageHome::scratch(
                    builder,
                    row,
                    place,
                    compatibility::COMPATIBILITY_SCRATCH_BYTES,
                )
                .unwrap();
                let available = fragmented_scalar::constant(builder, row, place, 256).unwrap();
                let assessment = compatibility::assess(
                    &declarations.0,
                    profile_type,
                    builder,
                    row,
                    profile,
                    scratch,
                    available,
                    placement,
                    terminal_psi::TerminalPlacementLifecycleOperation::Initialize,
                )
                .unwrap();
                let first_backing = (u64::from(layout.bytes)
                    + u64::from(compatibility::COMPATIBILITY_SCRATCH_BYTES)
                    + 47)
                    & !31;
                let backing_bases = [first_backing, first_backing + 257];
                let staged_base = first_backing + 544;
                let staged = fragmented_scalar::constant(builder, row, place, staged_base).unwrap();
                let decisions: Vec<_> = backing_bases
                    .into_iter()
                    .map(|base| {
                        let backing =
                            fragmented_scalar::constant(builder, row, place, base).unwrap();
                        let decision = super::super::outcome::decide(
                            builder,
                            row,
                            place,
                            backing,
                            available,
                            16,
                            assessment.modulus,
                            assessment.residue,
                            assessment.compatible,
                        )
                        .unwrap();
                        super::super::initialization::commit(
                            builder, row, place, staged, backing, &decision,
                        )
                        .unwrap();
                        decision
                    })
                    .collect();
                for (profile_position, profile) in profiles.iter().enumerate() {
                    let normalized = access_plans::validate_resource_profile(profile.clone(), 256);
                    let expected = normalized.as_ref().ok().and_then(|profile| {
                        access_plans::validate_initialization_resources(placement, profile).ok()
                    });
                    let mut bytes = encode(&layout, profile);
                    bytes.resize(staged_base as usize + 24, 0xd7);
                    bytes[staged_base as usize..staged_base as usize + 16].fill(0x69);
                    let before = bytes[..layout.bytes as usize + 8].to_vec();
                    let registers = run_values(builder, &mut bytes, u64::from(layout.bytes) + 8);
                    assert_eq!(
                        registers[assessment.well_formed.0 as usize],
                        u64::from(normalized.is_ok())
                    );
                    assert_eq!(
                        registers[assessment.compatible.0 as usize],
                        u64::from(expected.is_some()),
                        "placement {placement_position}, profile {profile_position}"
                    );
                    if let Some(expected) = &expected {
                        let congruence = expected.placement().base_congruence();
                        assert_eq!(
                            registers[assessment.modulus.0 as usize],
                            congruence.modulus(),
                            "placement {placement_position}, profile {profile_position}"
                        );
                        assert_eq!(
                            registers[assessment.residue.0 as usize],
                            congruence.residue(),
                            "placement {placement_position}, profile {profile_position}"
                        );
                    }
                    for (base, decision) in backing_bases.into_iter().zip(&decisions) {
                        let ready = expected.as_ref().is_some_and(|expected| {
                            expected.placement().base_congruence().admits(base)
                        });
                        assert_eq!(registers[decision.ready.0 as usize], u64::from(ready));
                        assert_eq!(
                            registers[decision.copy_length.0 as usize],
                            if ready { 16 } else { 0 }
                        );
                        assert_eq!(
                            &bytes[base as usize..base as usize + 16],
                            &[if ready { 0x69 } else { 0xd7 }; 16]
                        );
                        assert!(
                            bytes[base as usize + 16..base as usize + 256]
                                .iter()
                                .all(|byte| *byte == 0xd7),
                            "initialization touched the allocation tail"
                        );
                    }
                    assert_eq!(
                        &bytes[..before.len()],
                        before,
                        "compatibility mutated its immutable input"
                    );
                    assert_eq!(&bytes[bytes.len() - 8..], &[0xd7; 8]);
                    if profile_position == 3 {
                        // Raw reach is a set, not an ordered list or a count
                        // identity. Add a duplicate in only one adjacent row
                        // and reorder its services; coalescing is unchanged.
                        let region = entry(0, layout.regions, 0);
                        store(&mut bytes, region, layout.reach.count, 3);
                        for (position, service) in [29, 17, 29].into_iter().enumerate() {
                            store(
                                &mut bytes,
                                entry(region, layout.reach, position),
                                layout.service,
                                service,
                            );
                        }
                        let registers =
                            run_values(builder, &mut bytes, u64::from(layout.bytes) + 8);
                        assert_eq!(registers[assessment.well_formed.0 as usize], 1);
                        assert_eq!(
                            registers[assessment.compatible.0 as usize],
                            u64::from(expected.is_some())
                        );
                    }
                }
            });
        }
    }
}
