use access_admission::test_support::*;

mod field_regions {
    use access_admission::AccessFields;
    use access_admission::ValidatedPlacement;
    use access_admission::validate_access_resources;
    use access_admission::validate_placement;
    use access_admission::validate_resource_profile;
    use access_vocabulary::AccessExposure;
    use access_vocabulary::AccessOperation;
    use access_vocabulary::AtomicCapability;
    use access_vocabulary::BorrowPolarity;
    use access_vocabulary::BoundaryReach;
    use access_vocabulary::ExternalCapability;
    use access_vocabulary::FieldAccess;
    use access_vocabulary::PeerWritability;
    use access_vocabulary::ResourceProfile;
    use access_vocabulary::ResourceRegion;
    use access_vocabulary::StableCapability;
    use layout_plans::LayoutPlacementReport;
    use layout_plans::test_support::{layout_field_entry, layout_report};
    use storage_access::AccessAuthorization;

    fn page_plan(read: bool, write: bool, byte_length: u64) -> ValidatedPlacement {
        region_plan(FieldAccess::Region {
            byte_length,
            byte_alignment: 8,
            transfer_width_bits: 64,
            observation: access_vocabulary::ObservationModel::Stable,
            permissions: access_vocabulary::AccessPermissions {
                read,
                write,
                ..Default::default()
            },
            exposure: AccessExposure::Exported,
        })
    }

    fn region_plan(demand: FieldAccess) -> ValidatedPlacement {
        let layout = layout_report(
            vec![layout_field_entry(
                "words",
                LayoutPlacementReport::At { offset: 8 },
            )],
            None,
            Some(4104),
            8,
        );
        let mut access = AccessFields::inaccessible(&layout).expect("schema slots");
        let key = access.key_at(0).expect("one field");
        access.set(key, demand).expect("exact field key");
        validate_placement(layout, access, BoundaryReach::default())
            .expect("structural page region")
    }

    #[test]
    fn external_region_requires_complete_exact_leaf_supply() {
        use access_vocabulary::AccessPermissions;
        use access_vocabulary::ExternalReadBehavior;
        use access_vocabulary::ObservationModel;
        use access_vocabulary::TransferRule;
        let demand = FieldAccess::Region {
            byte_length: 4096,
            byte_alignment: 8,
            transfer_width_bits: 64,
            observation: ObservationModel::External,
            permissions: AccessPermissions {
                read: true,
                ..Default::default()
            },
            exposure: AccessExposure::Exported,
        };
        let plan = region_plan(demand.clone());
        let external = |offset, length, width_bits, alignment_bytes, read, write| ResourceRegion {
            external: ExternalCapability::Access {
                read,
                write,
                transfers: vec![TransferRule {
                    width_bits,
                    alignment_bytes,
                }],
            },
            ..region(offset, length, StableCapability::None)
        };
        let read = ExternalReadBehavior::Repeatable;
        let cases = [
            (
                "external-only",
                true,
                vec![external(8, 4096, 64, 8, read, false)],
            ),
            (
                "different aligned rows",
                true,
                vec![
                    external(8, 2048, 64, 8, read, false),
                    external(2056, 2048, 64, 8, read, true),
                ],
            ),
            (
                "equivalent unaligned raw split",
                true,
                vec![
                    external(8, 2044, 64, 8, read, false),
                    external(2052, 2052, 64, 8, read, false),
                ],
            ),
            (
                "transfer crosses different rows",
                false,
                vec![
                    external(8, 2044, 64, 8, read, false),
                    external(2052, 2052, 64, 8, read, true),
                ],
            ),
            (
                "gap",
                false,
                vec![
                    external(8, 2048, 64, 8, read, false),
                    external(2064, 2040, 64, 8, read, false),
                ],
            ),
            (
                "last leaf missing",
                false,
                vec![external(8, 4088, 64, 8, read, false)],
            ),
            (
                "wrong width",
                false,
                vec![external(8, 4096, 32, 4, read, false)],
            ),
            (
                "wrong read behavior",
                false,
                vec![external(
                    8,
                    4096,
                    64,
                    8,
                    ExternalReadBehavior::Destructive,
                    false,
                )],
            ),
            (
                "stride cannot meet alignment",
                false,
                vec![external(8, 4096, 64, 16, read, false)],
            ),
        ];
        for (name, accepted, regions) in cases {
            let profile = validate_resource_profile(ResourceProfile { regions }, 4104).expect(name);
            let result = validate_access_resources(&plan, &profile);
            assert_eq!(result.is_ok(), accepted, "{name}: {result:?}");
            if let Ok(compatible) = result {
                assert!(compatible.base_congruence().admits(4096), "{name}");
                assert!(!compatible.base_congruence().admits(4097), "{name}");
            }
        }
        let key = plan.access().plan().key_at(0).unwrap();
        let leaf = plan
            .access()
            .authorize_region_projection(
                key,
                8,
                64,
                BorrowPolarity::Shared,
                BorrowPolarity::Shared,
                AccessOperation::Read,
            )
            .unwrap();
        assert_eq!(leaf.descriptor().observation(), ObservationModel::External);
        assert!(
            plan.access()
                .authorize_region_projection(
                    key,
                    8,
                    32,
                    BorrowPolarity::Shared,
                    BorrowPolarity::Shared,
                    AccessOperation::Read
                )
                .is_err()
        );
        assert_ne!(
            plan.content_interpretation(),
            page_plan(true, false, 4096).content_interpretation()
        );
        let FieldAccess::Region {
            byte_length,
            byte_alignment,
            observation,
            permissions,
            exposure,
            ..
        } = demand
        else {
            unreachable!()
        };
        let narrow = region_plan(FieldAccess::Region {
            byte_length,
            byte_alignment,
            observation,
            permissions,
            exposure,
            transfer_width_bits: 32,
        });
        assert_ne!(
            plan.content_interpretation(),
            narrow.content_interpretation()
        );
    }

    fn region(offset: u64, length: u64, stable: StableCapability) -> ResourceRegion {
        ResourceRegion {
            offset,
            length,
            peer: PeerWritability::Exclusive,
            stable,
            external: ExternalCapability::None,
            atomic: AtomicCapability::None,
            reach: BoundaryReach::default(),
        }
    }

    #[test]
    fn page_region_retains_one_key_but_no_primitive_transfer() {
        let plan = page_plan(true, true, 4096);
        let key = plan.access().plan().key_at(0).expect("one field");
        assert_eq!(plan.access().plan().entries().len(), 1);
        let region = plan.access().field_region(key).expect("region descriptor");
        assert_eq!(
            (
                region.byte_offset(),
                region.byte_length(),
                region.byte_alignment()
            ),
            (8, 4096, 8)
        );
        assert!(plan.access().field_descriptor(key).is_none());
        for operation in [AccessOperation::Read, AccessOperation::Write] {
            let error = plan
                .access()
                .authorize(
                    key,
                    BorrowPolarity::Exclusive,
                    BorrowPolarity::Exclusive,
                    operation,
                )
                .expect_err("a structural region is not a primitive event");
            assert!(error.0.contains("checked structural projection"));
        }
    }

    #[test]
    fn field_region_supply_covers_all_bytes_and_requested_operations() {
        let plan = page_plan(true, false, 4096);
        let profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![
                    region(0, 2048, StableCapability::Read),
                    region(2048, 2056, StableCapability::ReadWrite),
                ],
            },
            4104,
        )
        .expect("adjacent different supply");
        let compatible = validate_access_resources(&plan, &profile).expect("complete read supply");
        assert!(compatible.base_congruence().admits(4096));
        assert!(!compatible.base_congruence().admits(4097));
        assert!(validate_access_resources(&page_plan(true, true, 4096), &profile).is_err());
        for regions in [
            vec![region(8, 4095, StableCapability::Read)],
            vec![
                region(8, 2040, StableCapability::Read),
                region(2049, 2055, StableCapability::Read),
            ],
            vec![region(8, 4096, StableCapability::Write)],
        ] {
            let profile = validate_resource_profile(ResourceProfile { regions }, 4104)
                .expect("valid incomplete supply");
            assert!(validate_access_resources(&plan, &profile).is_err());
        }
    }

    #[test]
    fn structural_region_identity_binds_extent_and_permissions() {
        let plan = page_plan(true, true, 4096);
        for different in [page_plan(true, false, 4096), page_plan(true, true, 2048)] {
            assert_ne!(
                plan.content_interpretation(),
                different.content_interpretation()
            );
        }
    }

    #[test]
    fn structural_permission_does_not_relax_scalar_transfer_limits() {
        let plan = page_plan(true, true, 4096);
        let mut scalar = plan.access().plan().clone();
        let key = scalar.key_at(0).expect("field");
        scalar
            .set(
                key,
                FieldAccess::Stable {
                    transfer_width_bits: 32768,
                    read: true,
                    write: true,
                    exposure: AccessExposure::Exported,
                },
            )
            .expect("exact field");
        assert!(access_admission::validate_access_fields(scalar, plan.layout()).is_err());
    }

    #[test]
    fn projected_transfer_keeps_parent_range_permissions_and_source_borrow() {
        let plan = page_plan(true, true, 4096);
        let key = plan.access().plan().key_at(0).expect("parent field");
        for offset in [8, 4096] {
            let authorized = plan
                .access()
                .authorize_region_projection(
                    key,
                    offset,
                    64,
                    BorrowPolarity::Exclusive,
                    BorrowPolarity::Exclusive,
                    AccessOperation::Write,
                )
                .expect("first and last whole leaf");
            assert_eq!(authorized.descriptor().key(), key);
            assert_eq!(authorized.descriptor().container_byte_offset(), offset);
            assert_eq!(authorized.descriptor().effect_footprint().length_bytes(), 8);
        }
        for (offset, width) in [(0, 64), (4104, 64), (9, 64), (8, 32768), (8, 0), (8, 12)] {
            assert!(
                plan.access()
                    .authorize_region_projection(
                        key,
                        offset,
                        width,
                        BorrowPolarity::Exclusive,
                        BorrowPolarity::Exclusive,
                        AccessOperation::Write,
                    )
                    .is_err()
            );
        }
        assert!(
            plan.access()
                .authorize_region_projection(
                    key,
                    8,
                    64,
                    BorrowPolarity::Exclusive,
                    BorrowPolarity::Shared,
                    AccessOperation::Write,
                )
                .is_err()
        );
        assert!(
            plan.access()
                .authorize_region_projection(
                    key,
                    8,
                    64,
                    BorrowPolarity::Shared,
                    BorrowPolarity::Exclusive,
                    AccessOperation::Write,
                )
                .is_err()
        );
        let read_only = page_plan(true, false, 4096);
        let read_key = read_only
            .access()
            .plan()
            .key_at(0)
            .expect("read-only field");
        assert!(
            read_only
                .access()
                .authorize_region_projection(
                    read_key,
                    8,
                    64,
                    BorrowPolarity::Exclusive,
                    BorrowPolarity::Exclusive,
                    AccessOperation::Write,
                )
                .is_err()
        );
        assert!(
            read_only
                .access()
                .authorize_region_projection(
                    read_key,
                    8,
                    64,
                    BorrowPolarity::Shared,
                    BorrowPolarity::Shared,
                    AccessOperation::Read,
                )
                .is_ok()
        );
    }

    #[test]
    fn destructive_region_cannot_overlap_another_accessible_field() {
        for other_offset in [8, 16] {
            let layout = layout_report(
                vec![
                    layout_field_entry("consumed", LayoutPlacementReport::At { offset: 0 }),
                    layout_field_entry(
                        "observed",
                        LayoutPlacementReport::At {
                            offset: other_offset,
                        },
                    ),
                ],
                None,
                Some(32),
                8,
            );
            let mut access = AccessFields::inaccessible(&layout).unwrap();
            access
                .set(
                    access.key_at(0).unwrap(),
                    FieldAccess::Region {
                        byte_length: 16,
                        byte_alignment: 8,
                        transfer_width_bits: 64,
                        observation: access_vocabulary::ObservationModel::External,
                        permissions: access_vocabulary::AccessPermissions {
                            take: true,
                            ..Default::default()
                        },
                        exposure: AccessExposure::Exported,
                    },
                )
                .unwrap();
            for other in [
                FieldAccess::External {
                    transfer_width_bits: 64,
                    read: access_vocabulary::ExternalRead::Read,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
                FieldAccess::Region {
                    byte_length: 16,
                    byte_alignment: 8,
                    transfer_width_bits: 64,
                    observation: access_vocabulary::ObservationModel::External,
                    permissions: access_vocabulary::AccessPermissions {
                        read: true,
                        ..Default::default()
                    },
                    exposure: AccessExposure::Exported,
                },
            ] {
                access.set(access.key_at(1).unwrap(), other).unwrap();
                let result =
                    validate_placement(layout.clone(), access.clone(), BoundaryReach::default());
                assert_eq!(result.is_ok(), other_offset == 16, "{result:?}");
                if let Err(error) = result {
                    assert!(error.0.contains("overlaps"), "{error}");
                }
            }
        }
    }
}

mod plan_validation {
    use super::{
        access_plan, field_key, reach, uart_access_fields, uart_access_source, uart_layout,
    };
    use access_admission::AccessFieldEntry;
    use access_admission::AccessFields;
    use access_admission::validate_access_fields;
    use access_admission::validate_placement;
    use access_vocabulary::AccessExposure;
    use access_vocabulary::AccessOperation;
    use access_vocabulary::AtomicPermissions;
    use access_vocabulary::BorrowPolarity;
    use access_vocabulary::BoundaryReach;
    use access_vocabulary::BoundaryServiceReachId;
    use access_vocabulary::ExternalRead;
    use access_vocabulary::FieldAccess;
    use access_vocabulary::ObservationModel;
    use layout_plans::test_support::{layout_field_entry, layout_report};
    use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
    use storage_access::AccessAuthorization;

    #[test]
    fn inaccessible_seed_has_exact_canonical_schema_cardinality() {
        let layout = uart_layout();
        let plan = AccessFields::inaccessible(&layout).expect("inaccessible plan");
        assert_eq!(plan.entries().len(), 3);
        assert_eq!(
            plan.entries()
                .iter()
                .map(AccessFieldEntry::field)
                .collect::<Vec<_>>(),
            vec!["control", "status", "transmit"]
        );
        assert!(
            plan.entries()
                .iter()
                .all(|entry| entry.access() == &FieldAccess::Inaccessible)
        );
        let validated = validate_access_fields(plan, &layout).expect("all-inaccessible plan");
        assert!(validated.field_descriptors().is_empty());
        assert!(
            validated
                .authorize(
                    field_key(&validated, "status"),
                    BorrowPolarity::Shared,
                    BorrowPolarity::Shared,
                    AccessOperation::Read,
                )
                .is_err()
        );
    }

    #[test]
    fn numbered_field_rename_does_not_change_access_identity() {
        let mut original_layout = LayoutPlanReport {
            schema_report_fingerprint: 0x44,
            entries: vec![LayoutFieldEntryReport {
                field: "word".into(),
                member_identity: Some(7),
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        let original = validate_access_fields(
            access_plan(
                &original_layout,
                &[("word", super::stable_word32_read(AccessExposure::Exported))],
            ),
            &original_layout,
        )
        .expect("original numbered plan");

        original_layout.entries[0].field = "renamed_word".into();
        let renamed = validate_access_fields(
            access_plan(
                &original_layout,
                &[(
                    "renamed_word",
                    super::stable_word32_read(AccessExposure::Exported),
                )],
            ),
            &original_layout,
        )
        .expect("renamed numbered plan");
        assert_eq!(original.identity(), renamed.identity());
    }

    #[test]
    fn inaccessible_plan_rejects_one_name_for_multiple_field_identities() {
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 0x45,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "word".into(),
                    member_identity: Some(7),
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "word".into(),
                    member_identity: Some(8),
                    placement: LayoutPlacementReport::At { offset: 4 },
                },
            ],
            offsets: Some(vec![0, 4]),
            size: Some(8),
            align: 4,
        };

        let error = AccessFields::inaccessible(&layout)
            .expect_err("one presentation name cannot select two stable field identities");
        assert!(
            error.0.contains(
                "layout field `word` identifies both stable member identity #7 and stable member identity #8"
            ),
            "{}",
            error.0
        );
    }

    #[test]
    fn access_identity_covers_operation_width_and_exposure() {
        let layout = layout_report(
            vec![layout_field_entry(
                "word",
                LayoutPlacementReport::At { offset: 0 },
            )],
            Some(vec![0]),
            Some(8),
            8,
        );
        let validate = |access: FieldAccess| {
            validate_access_fields(access_plan(&layout, &[("word", access)]), &layout)
                .expect("identity test plan")
                .identity()
        };
        let stable_read = super::stable_word32_read(AccessExposure::Exported);
        let mut stable_write = stable_read.clone();
        let FieldAccess::Stable { read, write, .. } = &mut stable_write else {
            unreachable!()
        };
        *read = false;
        *write = true;
        let mut wider = stable_read.clone();
        let FieldAccess::Stable {
            transfer_width_bits,
            ..
        } = &mut wider
        else {
            unreachable!()
        };
        *transfer_width_bits = 64;
        let mut private = stable_read.clone();
        let FieldAccess::Stable { exposure, .. } = &mut private else {
            unreachable!()
        };
        *exposure = AccessExposure::BindingPrivate;
        let external = FieldAccess::External {
            transfer_width_bits: 32,
            read: ExternalRead::Read,
            write: false,
            exposure: AccessExposure::Exported,
        };

        let commitment = |access: FieldAccess| {
            validate_placement(
                layout.clone(),
                access_plan(&layout, &[("word", access)]),
                BoundaryReach::default(),
            )
            .expect("interpretation test placement")
            .content_interpretation()
            .commitment()
        };
        let policies = [stable_read, stable_write, wider, private, external];
        let identities = policies.clone().map(validate);
        let interpretations = policies.map(commitment);
        for (index, identity) in identities.iter().enumerate() {
            assert!(
                identities[index + 1..]
                    .iter()
                    .all(|other| other != identity),
                "every semantic policy change must alter the compact compatibility identity"
            );
            assert!(
                interpretations[index + 1..]
                    .iter()
                    .all(|other| *other != interpretations[index]),
                "every semantic policy change must alter the authoritative interpretation commitment"
            );
        }
    }

    #[test]
    fn placement_identity_owns_normalized_reach() {
        let layout = uart_layout();
        let access = uart_access_source(&layout);
        let uart = validate_placement(
            layout.clone(),
            access.clone(),
            BoundaryReach::from_services([reach(), reach()]),
        )
        .expect("UART placement");
        let alternate_reach =
            BoundaryServiceReachId::from_normalized_identity(8).expect("alternate reach");
        let alternate = validate_placement(
            layout,
            access,
            BoundaryReach::from_services([alternate_reach]),
        )
        .expect("alternate placement reach");
        assert_eq!(
            uart.reach().services().len(),
            1,
            "reach is a normalized set"
        );
        assert_eq!(uart.access().identity(), alternate.access().identity());
        assert_ne!(uart.identity(), alternate.identity());
    }

    #[test]
    fn uart_access_plan_validates_geometry_and_borrow_polarity() {
        let plan = uart_access_fields();

        let status = plan
            .authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Shared,
                BorrowPolarity::Shared,
                AccessOperation::Read,
            )
            .expect("shared snapshot read");
        assert_eq!(status.descriptor().field(), "status");
        assert_eq!(status.descriptor().container_byte_offset(), 0);
        assert_eq!(status.descriptor().transfer_width_bits(), 32);
        assert_eq!(
            status.descriptor().observation(),
            ObservationModel::External
        );
        assert_eq!(status.current_borrow(), BorrowPolarity::Shared);
        assert_eq!(status.source_loan(), BorrowPolarity::Shared);
        assert_eq!(status.operation(), AccessOperation::Read);
        assert_eq!(plan.field_descriptors().len(), 3);
        let control = plan
            .field_descriptor(field_key(&plan, "control"))
            .expect("control descriptor");
        assert_eq!(control.container_byte_offset(), 8);
        assert_eq!(
            control
                .logical_extent()
                .fragments()
                .iter()
                .map(|fragment| (
                    fragment.layout_bit_offset(),
                    fragment.source_bit_offset(),
                    fragment.width_bits()
                ))
                .collect::<Vec<_>>(),
            [(64, 0, 8)]
        );
        assert_eq!(
            (
                control.effect_footprint().byte_offset(),
                control.effect_footprint().length_bytes()
            ),
            (8, 4),
            "a narrow logical bitfield retains its whole transfer container"
        );
        assert!(
            plan.authorize(
                field_key(&plan, "transmit"),
                BorrowPolarity::Shared,
                BorrowPolarity::Exclusive,
                AccessOperation::Write,
            )
            .is_err()
        );
        plan.authorize(
            field_key(&plan, "transmit"),
            BorrowPolarity::Exclusive,
            BorrowPolarity::Exclusive,
            AccessOperation::Write,
        )
        .expect("exclusive whole write");
        assert!(
            plan.authorize(
                field_key(&plan, "control"),
                BorrowPolarity::Exclusive,
                BorrowPolarity::Exclusive,
                AccessOperation::CompoundMutation,
            )
            .is_err(),
            "external storage never derives compound mutation"
        );
    }

    #[test]
    fn stable_compound_mutation_is_derived_from_permissions_and_borrow() {
        let layout = uart_layout();
        let plan = validate_access_fields(
            access_plan(
                &layout,
                &[(
                    "status",
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: true,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            &layout,
        )
        .expect("stable read-write plan");
        plan.authorize(
            field_key(&plan, "status"),
            BorrowPolarity::Exclusive,
            BorrowPolarity::Exclusive,
            AccessOperation::CompoundMutation,
        )
        .expect("exclusive stable read-write access derives compound mutation");
        assert!(
            plan.authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Shared,
                BorrowPolarity::Exclusive,
                AccessOperation::CompoundMutation,
            )
            .is_err()
        );
        assert!(
            plan.authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Exclusive,
                BorrowPolarity::Shared,
                AccessOperation::CompoundMutation,
            )
            .is_err(),
            "an exclusive current borrow cannot upgrade a shared source loan"
        );

        let plan = validate_access_fields(
            access_plan(
                &layout,
                &[(
                    "status",
                    super::stable_word32_read(AccessExposure::Exported),
                )],
            ),
            &layout,
        )
        .expect("stable read-only plan");
        assert!(
            plan.authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Exclusive,
                BorrowPolarity::Exclusive,
                AccessOperation::CompoundMutation,
            )
            .is_err()
        );
    }

    #[test]
    fn narrow_external_write_rejects_before_admission() {
        let layout = uart_layout();
        let error = validate_access_fields(
            access_plan(
                &layout,
                &[(
                    "control",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: true,
                        exposure: AccessExposure::BindingPrivate,
                    },
                )],
            ),
            &layout,
        )
        .expect_err("a narrow External write would require a generic RMW");
        assert!(
            error.0.contains("complete admitted container"),
            "diagnostic must explain the whole-transfer requirement: {error}"
        );
    }

    #[test]
    fn destructive_access_requires_one_whole_snapshot_accessor() {
        let layout = uart_layout();
        let error = validate_access_fields(
            access_plan(
                &layout,
                &[(
                    "control",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Take,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            &layout,
        )
        .expect_err("a narrow field cannot independently consume its container");
        assert!(
            error
                .0
                .contains("only part of its 4-byte transfer container")
        );

        let aliased_layout = LayoutPlanReport {
            schema_report_fingerprint: 0xdead,
            entries: vec![
                layout_field_entry("snapshot", LayoutPlacementReport::At { offset: 0 }),
                layout_field_entry("status", LayoutPlacementReport::At { offset: 0 }),
            ],
            offsets: Some(vec![0, 0]),
            size: Some(4),
            align: 4,
        };
        let error = validate_access_fields(
            access_plan(
                &aliased_layout,
                &[
                    (
                        "snapshot",
                        FieldAccess::External {
                            transfer_width_bits: 32,
                            read: ExternalRead::Take,
                            write: false,
                            exposure: AccessExposure::Exported,
                        },
                    ),
                    (
                        "status",
                        super::stable_word32_read(AccessExposure::Exported),
                    ),
                ],
            ),
            &aliased_layout,
        )
        .expect_err("one destructive unit cannot expose a second field accessor");
        assert!(error.0.contains("one whole-snapshot take"));
    }

    #[test]
    fn empty_access_cases_reject_in_favor_of_inaccessible() {
        let layout = uart_layout();
        for access in [
            FieldAccess::Stable {
                transfer_width_bits: 32,
                read: false,
                write: false,
                exposure: AccessExposure::Exported,
            },
            FieldAccess::External {
                transfer_width_bits: 32,
                read: ExternalRead::None,
                write: false,
                exposure: AccessExposure::Exported,
            },
            FieldAccess::Atomic {
                transfer_width_bits: 32,
                operations: AtomicPermissions::default(),
                exposure: AccessExposure::Exported,
            },
        ] {
            let error =
                validate_access_fields(access_plan(&layout, &[("status", access)]), &layout)
                    .expect_err("empty access case must reject");
            assert!(error.0.contains("Inaccessible"));
        }
    }

    #[test]
    fn overlapping_atomic_fields_cannot_select_mixed_widths() {
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 0xa70,
            entries: vec![
                layout_field_entry("wide", LayoutPlacementReport::At { offset: 0 }),
                layout_field_entry("upper", LayoutPlacementReport::At { offset: 4 }),
            ],
            offsets: Some(vec![0, 4]),
            size: Some(8),
            align: 8,
        };
        let atomic_load = |transfer_width_bits| FieldAccess::Atomic {
            transfer_width_bits,
            operations: AtomicPermissions {
                load: true,
                ..AtomicPermissions::default()
            },
            exposure: AccessExposure::Exported,
        };
        let error = validate_access_fields(
            access_plan(
                &layout,
                &[("wide", atomic_load(64)), ("upper", atomic_load(32))],
            ),
            &layout,
        )
        .expect_err("one active placement cannot mix overlapping atomic widths");
        assert!(
            error.0.contains("overlapping transfer containers") && error.0.contains("mix widths"),
            "diagnostic must identify both the overlap and granularity conflict: {error}"
        );
    }

    #[test]
    fn multi_container_fragments_are_not_one_access() {
        let layout = layout_report(
            vec![
                layout_field_entry(
                    "entry",
                    LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 32,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 16,
                    },
                ),
                layout_field_entry(
                    "entry",
                    LayoutPlacementReport::Bits {
                        container: 4,
                        container_width: 32,
                        destination_lsb: 0,
                        source_lsb: 16,
                        width: 16,
                    },
                ),
            ],
            None,
            Some(8),
            4,
        );
        let error = validate_access_fields(
            access_plan(
                &layout,
                &[("entry", super::stable_word32_read(AccessExposure::Exported))],
            ),
            &layout,
        )
        .expect_err("one token cannot hide two primitive accesses");
        assert!(error.0.contains("multiple containers"));
    }
}
