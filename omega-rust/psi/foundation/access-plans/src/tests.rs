//! Tests the validators consumed by compiler and runtime access paths.

use crate::{
    AccessExposure, AccessFieldEntry, AccessFieldKey, AccessFields, AccessPlan, AtomicCapability,
    BoundaryReach, BoundaryServiceReachId, ExternalCapability, ExternalRead, ExternalReadBehavior,
    FieldAccess, PeerWritability, ResourceProfile, ResourceRegion, StableCapability, TransferRule,
    ValidatedAccessFields, ValidatedAccessPlan, validate_access_fields, validate_access_plan,
};
use extents::{AddressSpaceId, ExtentProvenanceId, ExtentRights};
use layout_plans::test_support::layout_field_entry;
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};

fn reach() -> BoundaryServiceReachId {
    BoundaryServiceReachId::from_normalized_identity(7).expect("normalized reach")
}

fn word32_transfers() -> Vec<TransferRule> {
    vec![TransferRule {
        width_bits: 32,
        alignment_bytes: 4,
    }]
}

fn word32_access(read: ExternalReadBehavior, write: bool) -> ExternalCapability {
    ExternalCapability::Access {
        read,
        write,
        transfers: word32_transfers(),
    }
}

fn stable_word32_read(exposure: AccessExposure) -> FieldAccess {
    FieldAccess::Stable {
        transfer_width_bits: 32,
        read: true,
        write: false,
        exposure,
    }
}

fn uart_reach() -> BoundaryReach {
    BoundaryReach::from_services([reach()])
}

fn uart_layout() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            layout_field_entry("status", LayoutPlacementReport::At { offset: 0 }),
            layout_field_entry("transmit", LayoutPlacementReport::At { offset: 4 }),
            LayoutFieldEntryReport {
                field: "control".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 8,
                    container_width: 32,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 8,
                },
            },
        ],
        offsets: None,
        size: Some(12),
        align: 4,
    }
}

fn access_plan(layout: &LayoutPlanReport, decisions: &[(&str, FieldAccess)]) -> AccessFields {
    let mut plan = AccessFields::inaccessible(layout).expect("inaccessible seed");
    for (field, access) in decisions {
        let key = plan
            .entries()
            .iter()
            .find(|entry| entry.field() == *field)
            .map(AccessFieldEntry::key)
            .expect("schema field key");
        plan.set(key, access.clone())
            .expect("replace field decision");
    }
    plan
}

fn field_key(plan: &ValidatedAccessFields, field: &str) -> AccessFieldKey {
    plan.plan()
        .entries()
        .iter()
        .find(|entry| entry.field() == field)
        .map(AccessFieldEntry::key)
        .expect("validated schema field key")
}

fn uart_access_source(layout: &LayoutPlanReport) -> AccessFields {
    access_plan(
        layout,
        &[
            (
                "status",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Read,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            ),
            (
                "transmit",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::None,
                    write: true,
                    exposure: AccessExposure::Exported,
                },
            ),
            (
                "control",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Read,
                    write: false,
                    exposure: AccessExposure::BindingPrivate,
                },
            ),
        ],
    )
}

fn uart_access_fields() -> ValidatedAccessFields {
    let layout = uart_layout();
    let plan = uart_access_source(&layout);
    validate_access_fields(plan, &layout).expect("UART plan")
}

fn uart_access_plan() -> ValidatedAccessPlan {
    let layout = uart_layout();
    validate_access_plan(AccessPlan {
        access: uart_access_source(&layout),
        layout,
        reach: uart_reach(),
    })
    .expect("UART access plan")
}

fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, extents::ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn provider_issuance(seed: u64) -> extents::ExtentProviderIssuance {
    let base = seed * 16;
    extents::ExtentProviderIssuance::from_normalized_identities(std::array::from_fn(
        |identity_index| base + (identity_index as u64 + 1),
    ))
    .expect("normalized provider issuance")
}

fn extent_rights(identities: &[u64]) -> ExtentRights {
    ExtentRights::from_normalized_identities(
        identities
            .iter()
            .copied()
            .map(|identity| extent_id(identity, extents::ExtentRightId::from_normalized_identity)),
    )
}

fn uart_extent(base: u64, length: u64) -> extents::Extent {
    uart_extent_with_lineage(base, length, 1)
}

fn uart_extent_with_lineage(base: u64, length: u64, lineage: u64) -> extents::Extent {
    uart_extent_with_root(base, length, 1, lineage)
}

fn uart_extent_with_root(base: u64, length: u64, provider: u64, lineage: u64) -> extents::Extent {
    uart_root_grant(provider, lineage)
        .mint(base, length)
        .expect("UART extent")
}

fn uart_root_grant(provider: u64, lineage: u64) -> extents::ExtentRootGrant {
    uart_root_grant_with_mapping(provider, lineage, 5, 6)
}

fn uart_root_grant_with_mapping(
    provider: u64,
    lineage: u64,
    provenance: u64,
    era: u64,
) -> extents::ExtentRootGrant {
    extents::ExtentRootGrant::from_admitted_provider(
        provider_issuance(provider),
        extent_id(lineage, extents::ExtentLineageId::from_normalized_identity),
        extent_id(2, AddressSpaceId::from_normalized_identity),
        extent_rights(&[3, 4]),
        extent_id(provenance, ExtentProvenanceId::from_normalized_identity),
        extent_id(era, extents::MappingEraId::from_normalized_identity),
    )
}

fn uart_resource_profile_data(length: u64, reach: &BoundaryReach) -> ResourceProfile {
    ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length,
            peer: PeerWritability::Exclusive,
            stable: StableCapability::None,
            external: word32_access(ExternalReadBehavior::Repeatable, true),
            atomic: AtomicCapability::None,
            reach: reach.clone(),
        }],
    }
}

mod field_regions {
    use crate::{
        AccessExposure, AccessFields, AccessOperation, AccessPlan, AtomicCapability,
        BorrowPolarity, BoundaryReach, ExternalCapability, FieldAccess, PeerWritability,
        ResourceProfile, ResourceRegion, StableCapability, ValidatedAccessPlan,
        validate_access_plan, validate_access_resources, validate_resource_profile,
    };
    use layout_plans::LayoutPlacementReport;
    use layout_plans::test_support::{layout_field_entry, layout_report};

    fn page_plan(read: bool, write: bool, byte_length: u64) -> ValidatedAccessPlan {
        region_plan(FieldAccess::Region {
            byte_length,
            byte_alignment: 8,
            transfer_width_bits: 64,
            observation: crate::ObservationModel::Stable,
            permissions: crate::AccessPermissions {
                read,
                write,
                ..Default::default()
            },
            exposure: AccessExposure::Exported,
        })
    }

    fn region_plan(demand: FieldAccess) -> ValidatedAccessPlan {
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
        validate_access_plan(AccessPlan {
            layout,
            access,
            reach: BoundaryReach::default(),
        })
        .expect("structural page region")
    }

    #[test]
    fn external_region_requires_complete_exact_leaf_supply() {
        use crate::{AccessPermissions, ExternalReadBehavior, ObservationModel, TransferRule};
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
        assert!(crate::validate_access_fields(scalar, plan.layout()).is_err());
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
                        observation: crate::ObservationModel::External,
                        permissions: crate::AccessPermissions {
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
                    read: crate::ExternalRead::Read,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
                FieldAccess::Region {
                    byte_length: 16,
                    byte_alignment: 8,
                    transfer_width_bits: 64,
                    observation: crate::ObservationModel::External,
                    permissions: crate::AccessPermissions {
                        read: true,
                        ..Default::default()
                    },
                    exposure: AccessExposure::Exported,
                },
            ] {
                access.set(access.key_at(1).unwrap(), other).unwrap();
                let result = validate_access_plan(AccessPlan {
                    layout: layout.clone(),
                    access: access.clone(),
                    reach: BoundaryReach::default(),
                });
                assert_eq!(result.is_ok(), other_offset == 16, "{result:?}");
                if let Err(error) = result {
                    assert!(error.0.contains("overlaps"), "{error}");
                }
            }
        }
    }
}

mod initialization_resources {
    use crate::{
        AccessExposure, AccessFields, AccessPlan, AtomicCapability, BoundaryReach,
        ExternalCapability, PeerWritability, ResourceProfile, ResourceRegion, StableCapability,
        ValidatedAccessPlan, validate_access_plan, validate_access_resources,
        validate_initialization_resources, validate_resource_profile,
    };

    fn plan(expose_read: bool) -> ValidatedAccessPlan {
        let layout = super::uart_layout();
        let access = if expose_read {
            super::access_plan(
                &layout,
                &[(
                    "control",
                    super::stable_word32_read(AccessExposure::Exported),
                )],
            )
        } else {
            AccessFields::inaccessible(&layout).expect("inaccessible access")
        };
        validate_access_plan(AccessPlan {
            layout,
            access,
            reach: super::uart_reach(),
        })
        .expect("placement")
    }

    fn region(offset: u64, length: u64, stable: StableCapability) -> ResourceRegion {
        ResourceRegion {
            offset,
            length,
            peer: PeerWritability::Exclusive,
            stable,
            external: ExternalCapability::None,
            atomic: AtomicCapability::None,
            reach: super::uart_reach(),
        }
    }

    #[test]
    fn relocation_checks_later_cells_without_enumerating_the_run() {
        let plan = plan(true);
        let count = 1_000_000_000;
        let occupied = count * 12;
        let source = validate_resource_profile(
            ResourceProfile {
                regions: vec![region(0, occupied, StableCapability::Read)],
            },
            occupied,
        )
        .expect("source");
        let destination = validate_resource_profile(
            ResourceProfile {
                regions: vec![region(0, occupied, StableCapability::ReadWrite)],
            },
            occupied,
        )
        .expect("destination");
        crate::validate_run_relocation_resources(&plan, count, 4096, &source, &destination)
            .expect("a billion cells with one uniform supply region");

        let late_write_only = validate_resource_profile(
            ResourceProfile {
                regions: vec![
                    region(0, occupied - 12, StableCapability::ReadWrite),
                    region(occupied - 12, 12, StableCapability::Write),
                ],
            },
            occupied,
        )
        .expect("late read permission removed");
        let error =
            crate::validate_run_relocation_resources(&plan, count, 4096, &source, &late_write_only)
                .expect_err("the last cell still owes the declared read access");
        assert!(error.0.contains("Stable read=true"));
    }

    #[test]
    fn relocation_requires_exact_windows_and_complete_source_reads() {
        let plan = plan(false);
        let readable = validate_resource_profile(
            ResourceProfile {
                regions: vec![region(0, 24, StableCapability::Read)],
            },
            24,
        )
        .expect("source");
        let writable = validate_resource_profile(
            ResourceProfile {
                regions: vec![region(0, 24, StableCapability::Write)],
            },
            24,
        )
        .expect("destination");
        crate::validate_run_relocation_resources(&plan, 2, 4096, &readable, &writable)
            .expect("write-only destination is sufficient for inaccessible cells");
        assert!(
            crate::validate_run_relocation_resources(&plan, 1, 4096, &readable, &writable)
                .expect_err("capacity is not occupied length")
                .0
                .contains("exact occupied")
        );
        assert!(
            crate::validate_run_relocation_resources(&plan, 2, 4097, &readable, &writable)
                .expect_err("alignment still matters")
                .0
                .contains("alignment")
        );
        assert!(
            crate::validate_run_relocation_resources(&plan, 2, 4096, &writable, &writable)
                .expect_err("source needs reads")
                .0
                .contains("Stable read")
        );
        assert!(
            crate::validate_run_relocation_resources(&plan, u64::MAX, 0, &readable, &writable)
                .expect_err("count arithmetic is checked")
                .0
                .contains("overflows")
        );
    }

    #[test]
    fn relocation_distinguishes_no_cells_from_zero_footprint_cells() {
        let empty =
            validate_resource_profile(ResourceProfile::default(), 0).expect("empty profile");
        assert!(
            crate::validate_run_relocation_resources(&plan(true), 0, 4096, &empty, &empty)
                .expect("an empty run needs no view of a cell")
                .is_none()
        );
        let layout = layout_plans::LayoutPlanReport {
            schema_report_fingerprint: 71,
            entries: Vec::new(),
            offsets: None,
            size: Some(0),
            align: 8,
        };
        let access = AccessFields::inaccessible(&layout).expect("zero-footprint fields");
        let zero_sized = validate_access_plan(AccessPlan {
            layout,
            access,
            reach: BoundaryReach::default(),
        })
        .expect("zero-footprint placement");
        assert!(
            crate::validate_run_relocation_resources(&zero_sized, u64::MAX, 4096, &empty, &empty)
                .expect("logical count does not enumerate zero-sized cells")
                .is_some()
        );
        assert!(
            crate::validate_run_relocation_resources(&zero_sized, 1, 4097, &empty, &empty)
                .expect_err("zero bytes do not erase cell alignment")
                .0
                .contains("alignment")
        );
    }

    #[test]
    fn read_only_view_supply_does_not_authorize_initialization() {
        let plan = plan(true);
        let profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![region(0, 12, StableCapability::Read)],
            },
            12,
        )
        .expect("readable profile");
        validate_access_resources(&plan, &profile).expect("view can read");
        let error = validate_initialization_resources(&plan, &profile)
            .expect_err("initialization must write");
        assert!(error.0.contains("Stable write supply at byte 0"));
    }

    #[test]
    fn initialization_covers_inaccessible_fields_and_padding() {
        let plan = plan(false);
        for regions in [
            vec![
                region(0, 4, StableCapability::Write),
                region(8, 4, StableCapability::Write),
            ],
            vec![region(0, 8, StableCapability::Write)],
            vec![region(4, 8, StableCapability::Write)],
            Vec::new(),
        ] {
            let profile = validate_resource_profile(ResourceProfile { regions }, 12)
                .expect("uncovered bytes supply nothing");
            validate_access_resources(&plan, &profile).expect("no accessor demands");
            let error = validate_initialization_resources(&plan, &profile)
                .expect_err("whole image needs coverage");
            assert!(error.0.contains("no resource supply at byte"));
        }
    }

    #[test]
    fn adjacent_different_supply_rows_cover_one_initial_image() {
        let plan = plan(false);
        let profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![
                    region(4, 8, StableCapability::ReadWrite),
                    region(0, 4, StableCapability::Write),
                ],
            },
            12,
        )
        .expect("unordered disjoint rows normalize");
        assert_eq!(profile.regions().len(), 2);
        let compatible = validate_initialization_resources(&plan, &profile)
            .expect("different additional capabilities do not forbid initialization");
        assert_eq!(compatible.byte_length(), 12);
        assert!(compatible.placement().base_congruence().admits(4096));
        assert!(!compatible.placement().base_congruence().admits(4097));
    }

    #[test]
    fn initialization_requires_reach_for_bytes_without_accessors() {
        let plan = plan(false);
        let profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    reach: BoundaryReach::default(),
                    ..region(0, 12, StableCapability::Write)
                }],
            },
            12,
        )
        .expect("write supply without required service");
        let error = validate_initialization_resources(&plan, &profile)
            .expect_err("inaccessible fields do not erase initialization reach");
        assert!(error.0.contains("complete boundary reach at byte 0"));
    }

    #[test]
    fn device_write_supply_does_not_turn_initialization_into_device_programming() {
        let plan = plan(false);
        let profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    external: super::word32_access(crate::ExternalReadBehavior::None, true),
                    ..region(0, 12, StableCapability::None)
                }],
            },
            12,
        )
        .expect("valid external write supply");
        let error = validate_initialization_resources(&plan, &profile)
            .expect_err("initialization requires Stable storage");
        assert!(error.0.contains("Stable write supply"));
    }

    #[test]
    fn initialization_preserves_subsequent_access_demands_and_stops_at_image_end() {
        let writable = validate_resource_profile(
            ResourceProfile {
                regions: vec![
                    region(0, 12, StableCapability::Write),
                    region(12, 4, StableCapability::Read),
                ],
            },
            16,
        )
        .expect("write supply followed by read-only storage");
        validate_initialization_resources(&plan(false), &writable)
            .expect("bytes beyond the image are not written");
        let error = validate_initialization_resources(&plan(true), &writable)
            .expect_err("the resulting view still needs read permission");
        assert!(error.0.contains("requests Stable read=true"));
    }

    fn admitted(extent: &extents::Extent) -> crate::AdmittedResourceProfile {
        crate::ResourceProfileGrant::from_admitted_provider(
            crate::ResourceProfileReceiptId::from_normalized_identity(41).expect("receipt"),
            extent,
            super::extent_rights(&[3]),
            super::uart_reach(),
        )
        .expect("selected provider binding")
        .admit(ResourceProfile {
            regions: vec![region(0, extent.length(), StableCapability::ReadWrite)],
        })
        .expect("admitted complete profile")
    }

    #[test]
    fn initialization_joins_exclusive_loan_and_concrete_alignment() {
        let mut extent = super::uart_extent(4096, 32);
        let profile = admitted(&extent);
        {
            let shared = extent.loan(0, 12).expect("shared loan");
            assert!(
                profile
                    .initialization_compatibility(&shared, &plan(false))
                    .expect_err("shared access cannot initialize")
                    .0
                    .contains("exclusive extent loan")
            );
        }
        {
            let unaligned = extent.loan_mut(1, 12).expect("exclusive unaligned loan");
            assert!(
                profile
                    .initialization_compatibility(&unaligned, &plan(false))
                    .expect_err("concrete base must satisfy the plan")
                    .0
                    .contains("base mod 4 = 0")
            );
        }
        {
            let aligned = extent.loan_mut(4, 12).expect("exclusive aligned subrange");
            assert_eq!(
                profile
                    .initialization_compatibility(&aligned, &plan(false))
                    .expect("the bound parent profile restricts to this loan")
                    .byte_length(),
                12
            );
        }
        assert_eq!((extent.base(), extent.length()), (4096, 32));
    }

    #[test]
    fn initialization_rejects_coincident_foreign_roots_and_mapping_epochs() {
        let extent = super::uart_extent(4096, 32);
        let profile = admitted(&extent);
        for (mut foreign, fragment) in [
            (super::uart_extent_with_root(4096, 32, 2, 1), "root origin"),
            (super::uart_extent_with_lineage(4096, 32, 2), "root lineage"),
            (
                super::uart_root_grant_with_mapping(1, 1, 5, 7)
                    .mint(4096, 32)
                    .expect("new mapping epoch"),
                "mapping era",
            ),
            (
                super::uart_root_grant_with_mapping(1, 1, 8, 6)
                    .mint(4096, 32)
                    .expect("different provenance"),
                "provenance",
            ),
        ] {
            let loan = foreign.loan_mut(0, 12).expect("coincident foreign loan");
            assert!(
                profile
                    .initialization_compatibility(&loan, &plan(false))
                    .expect_err("equal geometry is not this profile's qualification")
                    .0
                    .contains(fragment)
            );
        }
    }
}

mod plan_validation {
    use super::{
        access_plan, field_key, reach, uart_access_fields, uart_access_source, uart_layout,
    };
    use crate::plan_policy::access_fields_validation::validate_entry_geometry;
    use crate::{
        AccessExposure, AccessFieldEntry, AccessFieldKey, AccessFields, AccessOperation,
        AccessPlan, AtomicPermissions, BorrowPolarity, BoundaryReach, BoundaryServiceReachId,
        ExternalRead, FieldAccess, LogicalFieldFragment, ObservationModel, RelativeEffectFootprint,
        validate_access_fields, validate_access_plan,
    };
    use layout_plans::test_support::{layout_field_entry, layout_report};
    use layout_plans::{
        IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
    };

    #[test]
    fn compact_fnv_plan_inventory_is_explicitly_non_authoritative() {
        let source = include_str!("plan_policy.rs");
        assert_eq!(
            source.matches("semantic_vocabulary::Fnv1a::new()").count(),
            3,
            "every compact FNV plan family must remain in the reviewed local inventory"
        );
        for name in [
            "non_authoritative_access_plan_compatibility_fingerprint",
            "non_authoritative_access_compatibility_fingerprint",
            "non_authoritative_resource_profile_compatibility_fingerprint",
        ] {
            assert!(
                source.contains(name),
                "compact plan fingerprints must advertise their non-authoritative role: {name}"
            );
        }
        assert!(
            source.contains("omega.placement-plan.authoritative.v1\\0")
                && include_str!("../../optimization/src/lib.rs").contains("DomainDigestEncoder"),
            "provider-content interpretation must retain a domain-separated strong commitment"
        );
        assert!(
            source.contains("omega.access-field-layout.authoritative.v1\\0")
                && include_str!("access_fields.rs")
                    .contains("layout_commitment: AccessLayoutCommitment"),
            "access field keys must retain a domain-separated exact-layout commitment"
        );
    }

    #[test]
    fn stored_integer_geometry_uses_the_exact_encoded_width() {
        let layout = layout_report(
            vec![layout_field_entry(
                "value",
                LayoutPlacementReport::IntegerAt {
                    offset: 3,
                    stored_width: 16,
                    interpretation: IntegerInterpretation::Signed,
                },
            )],
            None,
            Some(8),
            1,
        );
        let (offset, logical, effect) = validate_entry_geometry("value", 16, &layout, 8)
            .expect("the access transfer matches the stored integer width");
        assert_eq!(offset, 3);
        assert_eq!(logical.fragments[0].layout_bit_offset, 24);
        assert_eq!(logical.fragments[0].width_bits, 16);
        assert_eq!(effect.length_bytes, 2);
        let error = validate_entry_geometry("value", 32, &layout, 8)
            .expect_err("semantic carrier width is not the stored transfer width");
        assert!(
            error
                .0
                .contains("32-bit transfer over a 16-bit stored integer")
        );
    }

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
    fn access_validation_replays_retained_layout_structure_not_only_fingerprint() {
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 0x46,
            entries: vec![LayoutFieldEntryReport {
                field: "word".into(),
                member_identity: Some(7),
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(8),
            align: 8,
        };
        let mut plan = AccessFields::inaccessible(&layout).expect("canonical access seed");
        let compact_identity = plan.layout_report_fingerprint;
        plan.retained_layout.entries[0].placement = LayoutPlacementReport::At { offset: 4 };
        assert_eq!(
            plan.layout_report_fingerprint, compact_identity,
            "the simulated carrier drift deliberately leaves its compact identity unchanged"
        );

        let error = validate_access_fields(plan, &layout)
            .expect_err("structural carrier drift must reject before access-plan sealing");
        assert!(
            error.0.contains("different validated layout"),
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
            validate_access_plan(AccessPlan {
                access: access_plan(&layout, &[("word", access)]),
                layout: layout.clone(),
                reach: BoundaryReach::default(),
            })
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
        let uart = validate_access_plan(AccessPlan {
            layout: layout.clone(),
            access: access.clone(),
            reach: BoundaryReach::from_services([reach(), reach()]),
        })
        .expect("UART placement");
        let alternate_reach =
            BoundaryServiceReachId::from_normalized_identity(8).expect("alternate reach");
        let alternate = validate_access_plan(AccessPlan {
            layout,
            access,
            reach: BoundaryReach::from_services([alternate_reach]),
        })
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
            control.logical_extent().fragments(),
            &[LogicalFieldFragment {
                layout_bit_offset: 64,
                source_bit_offset: 0,
                width_bits: 8,
            }]
        );
        assert_eq!(
            control.effect_footprint(),
            RelativeEffectFootprint {
                byte_offset: 8,
                length_bytes: 4,
            },
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

    #[test]
    fn field_keys_reject_cross_layout_and_out_of_cardinality_use() {
        let layout = uart_layout();
        let mut plan = AccessFields::inaccessible(&layout).expect("UART seed");
        let mut alternate_layout = layout.clone();
        alternate_layout.schema_report_fingerprint = 2;
        let mut alternate =
            AccessFields::inaccessible(&alternate_layout).expect("alternate schema seed");
        alternate.layout_report_fingerprint = plan.layout_report_fingerprint;
        for entry in &mut alternate.entries {
            entry.key.layout_report_fingerprint = plan.layout_report_fingerprint;
        }
        assert_eq!(
            plan.layout_report_fingerprint, alternate.layout_report_fingerprint,
            "the adversarial key substitution holds the compact report coordinate equal"
        );
        assert_ne!(plan.layout_commitment, alternate.layout_commitment);
        let error = plan
            .set(
                alternate.key_at(0).expect("alternate key"),
                super::stable_word32_read(AccessExposure::Exported),
            )
            .expect_err("cross-layout key must reject");
        assert!(error.0.contains("different validated layout"));

        let error = plan
            .set(
                AccessFieldKey {
                    layout_report_fingerprint: plan.layout_report_fingerprint(),
                    layout_commitment: plan.layout_commitment,
                    slot: u32::MAX,
                },
                super::stable_word32_read(AccessExposure::Exported),
            )
            .expect_err("out-of-cardinality key must reject");
        assert!(error.0.contains("outside the schema cardinality"));
    }
}

mod resource_profiles {
    use super::{
        access_plan, extent_rights, reach, uart_access_plan, uart_extent, uart_extent_with_lineage,
        uart_extent_with_root, uart_layout, uart_reach, uart_resource_profile_data,
    };
    use crate::{
        AccessExposure, AccessPlan, AtomicCapability, AtomicPermissions, AtomicTransferRule,
        BoundaryReach, BoundaryServiceReachId, EffectiveSupplyKind, ExternalCapability,
        ExternalRead, ExternalReadBehavior, FieldAccess, PeerWritability, ResourceProfile,
        ResourceProfileGrant, ResourceProfileReceiptId, ResourceRegion, StableCapability,
        TransferRule, validate_access_plan, validate_access_resources, validate_resource_profile,
    };
    use layout_plans::test_support::layout_field_entry;
    use layout_plans::{LayoutPlacementReport, LayoutPlanReport};

    #[test]
    fn empty_profile_restriction_supplies_no_bytes_and_keeps_admission_identity() {
        let extent = uart_extent(0x4000, 16);
        let loan = extent.loan(16, 0).expect("empty loan at the end");
        let admitted = ResourceProfileGrant::from_admitted_provider_loan(
            ResourceProfileReceiptId::from_normalized_identity(41).expect("receipt"),
            &loan,
            extent_rights(&[3]),
            BoundaryReach::default(),
        )
        .expect("empty loan still has exact backing authority")
        .admit(ResourceProfile {
            regions: Vec::new(),
        })
        .expect("no operations over an empty range");
        let profile = admitted.restrict_to_loan(&loan).expect("same empty loan");
        assert_eq!(profile.length(), 0);
        assert!(profile.regions().is_empty());
        assert!(profile.restrict(0, 1, &BoundaryReach::default()).is_err());
        assert!(profile.restrict(1, 0, &BoundaryReach::default()).is_err());

        let other = uart_extent_with_lineage(0x4000, 16, 63);
        let other_loan = other.loan(16, 0).expect("same empty coordinates");
        assert!(admitted.restrict_to_loan(&other_loan).is_err());
        let nonempty = extent.loan(15, 1).expect("one real byte");
        assert!(admitted.restrict_to_loan(&nonempty).is_err());

        let parent = validate_resource_profile(
            uart_resource_profile_data(16, &BoundaryReach::default()),
            16,
        )
        .expect("parent profile");
        for offset in [0, 8, 16] {
            let empty = parent
                .restrict(offset, 0, &BoundaryReach::default())
                .expect("contained empty restriction");
            assert_eq!(empty.length(), 0);
            assert!(empty.regions().is_empty());
        }
        assert!(
            validate_resource_profile(uart_resource_profile_data(16, &BoundaryReach::default()), 0)
                .is_err()
        );
    }

    #[test]
    fn resource_profiles_normalize_disjoint_regions_and_restrict_subranges() {
        let alternate_reach =
            BoundaryServiceReachId::from_normalized_identity(8).expect("alternate reach");
        let broad_reach = BoundaryReach::from_services([reach(), alternate_reach]);
        let stable = ResourceRegion {
            offset: 0,
            length: 4,
            peer: PeerWritability::Exclusive,
            stable: StableCapability::ReadWrite,
            external: ExternalCapability::None,
            atomic: AtomicCapability::None,
            reach: broad_reach.clone(),
        };
        let profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![
                    ResourceRegion {
                        offset: 4,
                        ..stable.clone()
                    },
                    stable.clone(),
                    ResourceRegion {
                        offset: 8,
                        length: 8,
                        peer: PeerWritability::Exclusive,
                        stable: StableCapability::None,
                        external: super::word32_access(ExternalReadBehavior::Repeatable, false),
                        atomic: AtomicCapability::None,
                        reach: broad_reach,
                    },
                ],
            },
            16,
        )
        .expect("disjoint profile");
        assert_eq!(
            profile.regions().len(),
            2,
            "adjacent identical regions normalize into one interval"
        );
        assert_eq!(profile.regions()[0].offset, 0);
        assert_eq!(profile.regions()[0].length, 8);

        let child = profile
            .restrict(4, 8, &uart_reach())
            .expect("subrange restriction");
        assert_eq!(child.length(), 8);
        assert_eq!(child.regions().len(), 2);
        assert_eq!(
            (child.regions()[0].offset, child.regions()[0].length),
            (0, 4)
        );
        assert_eq!(
            (child.regions()[1].offset, child.regions()[1].length),
            (4, 4)
        );
        assert!(child.regions().iter().all(|region| {
            region.reach.services().len() == 1 && region.reach.contains(reach())
        }));

        let overlap = validate_resource_profile(
            ResourceProfile {
                regions: vec![
                    stable,
                    ResourceRegion {
                        offset: 2,
                        length: 4,
                        peer: PeerWritability::Exclusive,
                        stable: StableCapability::Read,
                        external: ExternalCapability::None,
                        atomic: AtomicCapability::None,
                        reach: BoundaryReach::default(),
                    },
                ],
            },
            8,
        )
        .expect_err("overlapping resource regions must reject");
        assert!(overlap.0.contains("overlap"));
    }

    #[test]
    fn resource_compatibility_joins_observation_operations_width_and_reach() {
        let plan = uart_access_plan();
        let stable_profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 12,
                    peer: PeerWritability::Exclusive,
                    stable: StableCapability::ReadWrite,
                    external: ExternalCapability::None,
                    atomic: AtomicCapability::None,
                    reach: uart_reach(),
                }],
            },
            12,
        )
        .expect("stable profile");
        let compatibility = validate_access_resources(&plan, &stable_profile)
            .expect("stable supply may conservatively satisfy external demand");
        assert!(
            compatibility
                .fields()
                .iter()
                .all(|field| field.kind() == EffectiveSupplyKind::Stable)
        );
        assert_eq!(compatibility.base_congruence().modulus(), 4);
        assert_eq!(compatibility.base_congruence().residue(), 0);

        let read_only_external = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 12,
                    peer: PeerWritability::Exclusive,
                    stable: StableCapability::None,
                    external: super::word32_access(ExternalReadBehavior::Repeatable, false),
                    atomic: AtomicCapability::None,
                    reach: uart_reach(),
                }],
            },
            12,
        )
        .expect("read-only external profile");
        let error = validate_access_resources(&plan, &read_only_external)
            .expect_err("read-only external supply cannot satisfy UART writes");
        assert!(
            error.0.contains("transmit") && error.0.contains("incompatible External"),
            "canonical field order reports the first unsupported UART write: {error}"
        );

        let wrong_width = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 12,
                    peer: PeerWritability::Exclusive,
                    stable: StableCapability::None,
                    external: ExternalCapability::Access {
                        read: ExternalReadBehavior::Repeatable,
                        write: true,
                        transfers: vec![TransferRule {
                            width_bits: 64,
                            alignment_bytes: 8,
                        }],
                    },
                    atomic: AtomicCapability::None,
                    reach: uart_reach(),
                }],
            },
            12,
        )
        .expect("wrong-width profile remains structurally valid");
        let error = validate_access_resources(&plan, &wrong_width)
            .expect_err("transfer width must match exactly");
        assert!(
            error.0.contains("control") && error.0.contains("32-bit"),
            "canonical field order reports the first width mismatch: {error}"
        );

        let stable_demand = validate_access_plan(AccessPlan {
            layout: uart_layout(),
            access: access_plan(
                &uart_layout(),
                &[(
                    "status",
                    super::stable_word32_read(AccessExposure::Exported),
                )],
            ),
            reach: uart_reach(),
        })
        .expect("stable demand");
        let error = validate_access_resources(&stable_demand, &read_only_external)
            .expect_err("external supply cannot satisfy Stable demand");
        assert!(error.0.contains("requests Stable"));

        let destructive_demand = validate_access_plan(AccessPlan {
            layout: uart_layout(),
            access: access_plan(
                &uart_layout(),
                &[(
                    "status",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Take,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            reach: uart_reach(),
        })
        .expect("destructive external demand");
        let error = validate_access_resources(&destructive_demand, &read_only_external)
            .expect_err("repeatable reads cannot satisfy destructive observation");
        assert!(
            error.0.contains("status") && error.0.contains("Take"),
            "observation mismatch must name the destructive demand: {error}"
        );

        let atomic_layout = LayoutPlanReport {
            schema_report_fingerprint: 93,
            entries: vec![layout_field_entry(
                "head",
                LayoutPlacementReport::At { offset: 0 },
            )],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        let atomic_demand = validate_access_plan(AccessPlan {
            access: access_plan(
                &atomic_layout,
                &[(
                    "head",
                    FieldAccess::Atomic {
                        transfer_width_bits: 32,
                        operations: AtomicPermissions {
                            load: true,
                            fetch_add: true,
                            ..AtomicPermissions::default()
                        },
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            layout: atomic_layout,
            reach: BoundaryReach::default(),
        })
        .expect("atomic demand");
        let load_only = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 4,
                    peer: PeerWritability::Exclusive,
                    stable: StableCapability::None,
                    external: ExternalCapability::None,
                    atomic: AtomicCapability::Access {
                        transfers: vec![AtomicTransferRule {
                            transfer: TransferRule {
                                width_bits: 32,
                                alignment_bytes: 4,
                            },
                            operations: AtomicPermissions {
                                load: true,
                                ..AtomicPermissions::default()
                            },
                        }],
                    },
                    reach: BoundaryReach::default(),
                }],
            },
            4,
        )
        .expect("load-only atomic profile");
        let error = validate_access_resources(&atomic_demand, &load_only)
            .expect_err("atomic operation demand must be an exact supply subset");
        assert!(
            error.0.contains("head") && error.0.contains("operation families"),
            "atomic mismatch must name the field and operation family: {error}"
        );
    }

    #[test]
    fn subrange_loan_rebases_profile_and_preserves_denied_bytes() {
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 94,
            entries: vec![layout_field_entry(
                "word",
                LayoutPlacementReport::At { offset: 0 },
            )],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        let placement = validate_access_plan(AccessPlan {
            access: access_plan(
                &layout,
                &[(
                    "word",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            layout,
            reach: BoundaryReach::default(),
        })
        .expect("subrange placement");
        let extent = uart_extent(0x4000, 16);
        let profile = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(41).expect("profile receipt"),
            &extent,
            extent_rights(&[3]),
            BoundaryReach::default(),
        )
        .expect("profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 4,
                length: 4,
                peer: PeerWritability::Exclusive,
                stable: StableCapability::None,
                external: super::word32_access(ExternalReadBehavior::Repeatable, false),
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            }],
        })
        .expect("sparse admitted profile");

        {
            let loan = extent.loan(4, 4).expect("covered subrange loan");
            let restricted = profile
                .restrict_to_loan(&loan)
                .expect("resource region must rebase to the subrange loan");
            let compatibility = validate_access_resources(&placement, &restricted)
                .expect("covered supply satisfies demand");
            assert_eq!(compatibility.fields()[0].offset(), 0);
            assert_eq!(loan.base(), 0x4004);
        }

        let loan = extent.loan(0, 4).expect("uncovered subrange loan");
        let restricted = profile.restrict_to_loan(&loan).expect("same backing");
        let rejection = validate_access_resources(&placement, &restricted)
            .expect_err("profile restriction must not fill uncovered parent bytes");
        assert!(
            rejection.0.contains("not covered"),
            "uncovered subrange rejection must report missing supply"
        );
        drop(rejection);

        let partition = extent
            .partition_owned(4, 4)
            .expect("owned subrange partition");
        {
            let loan = partition
                .selected()
                .loan(0, 4)
                .expect("selected split loan");
            let restricted = profile
                .restrict_to_loan(&loan)
                .expect("a conserved split must retain its root profile binding");
            validate_access_resources(&placement, &restricted).expect("split supply");
            assert_eq!(loan.base(), 0x4004);
        }
        let restored = partition.rejoin();
        assert_eq!(restored.base(), 0x4000);
        assert_eq!(restored.length(), 16);
    }

    #[test]
    fn admitted_profile_rejects_coincident_independent_extent_root() {
        let admitted_root = uart_extent_with_lineage(0x6000, 12, 61);
        let profile = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(62).expect("profile receipt"),
            &admitted_root,
            extent_rights(&[3]),
            uart_reach(),
        )
        .expect("root-bound profile grant")
        .admit(uart_resource_profile_data(12, &uart_reach()))
        .expect("admitted root-bound profile");

        let foreign_origin = uart_extent_with_root(0x6000, 12, 2, 61);
        assert_ne!(foreign_origin.origin(), admitted_root.origin());
        assert_eq!(foreign_origin.lineage_root(), admitted_root.lineage_root());
        let loan = foreign_origin
            .loan(0, 12)
            .expect("coincident foreign-origin loan");
        let rejection = profile
            .restrict_to_loan(&loan)
            .expect_err("coincident geometry and lineage must not replay another origin's profile");
        assert!(
            rejection.0.contains("sealed root origin"),
            "cross-origin replay must identify the sealed-origin mismatch"
        );

        let coincident_root = uart_extent_with_lineage(0x6000, 12, 63);
        assert_eq!(coincident_root.origin(), admitted_root.origin());
        assert_ne!(coincident_root.lineage_root(), admitted_root.lineage_root());
        let loan = coincident_root
            .loan(0, 12)
            .expect("coincident independent loan");
        let rejection = profile.restrict_to_loan(&loan).expect_err(
            "coincident geometry and provenance must not replay another root's profile",
        );
        assert!(
            rejection.0.contains("root lineage"),
            "cross-root replay must identify the root-lineage mismatch"
        );
    }

    #[test]
    fn transfer_alignment_derives_build_time_and_runtime_base_checks() {
        let conflicting_layout = LayoutPlanReport {
            schema_report_fingerprint: 91,
            entries: vec![
                layout_field_entry("left", LayoutPlacementReport::At { offset: 0 }),
                layout_field_entry("right", LayoutPlacementReport::At { offset: 2 }),
            ],
            offsets: Some(vec![0, 2]),
            size: Some(8),
            align: 2,
        };
        let conflicting = validate_access_plan(AccessPlan {
            access: access_plan(
                &conflicting_layout,
                &[
                    (
                        "left",
                        FieldAccess::External {
                            transfer_width_bits: 32,
                            read: ExternalRead::Read,
                            write: false,
                            exposure: AccessExposure::Exported,
                        },
                    ),
                    (
                        "right",
                        FieldAccess::External {
                            transfer_width_bits: 32,
                            read: ExternalRead::Read,
                            write: false,
                            exposure: AccessExposure::Exported,
                        },
                    ),
                ],
            ),
            layout: conflicting_layout,
            reach: BoundaryReach::default(),
        })
        .expect("relative geometry is structurally valid");
        let profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 8,
                    peer: PeerWritability::Exclusive,
                    stable: StableCapability::None,
                    external: super::word32_access(ExternalReadBehavior::Repeatable, false),
                    atomic: AtomicCapability::None,
                    reach: BoundaryReach::default(),
                }],
            },
            8,
        )
        .expect("alignment profile");
        let error = validate_access_resources(&conflicting, &profile)
            .expect_err("inconsistent field congruences must reject before admission");
        assert!(
            error.0.contains("right")
                && error.0.contains("offset 2")
                && error.0.contains("conflicts")
        );

        let layout = LayoutPlanReport {
            schema_report_fingerprint: 92,
            entries: vec![layout_field_entry(
                "word",
                LayoutPlacementReport::At { offset: 0 },
            )],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 1,
        };
        let placement = validate_access_plan(AccessPlan {
            access: access_plan(
                &layout,
                &[(
                    "word",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            layout,
            reach: BoundaryReach::default(),
        })
        .expect("single-field placement");
        let extent = uart_extent(0x1002, 4);
        let loan = extent.loan(0, 4).expect("misaligned loan");
        let resources = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(22).expect("profile receipt"),
            &extent,
            extent_rights(&[3]),
            BoundaryReach::default(),
        )
        .expect("profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 4,
                peer: PeerWritability::Exclusive,
                stable: StableCapability::None,
                external: super::word32_access(ExternalReadBehavior::Repeatable, false),
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            }],
        })
        .expect("admitted profile");
        let restricted = resources.restrict_to_loan(&loan).expect("same backing");
        let compatibility = validate_access_resources(&placement, &restricted)
            .expect("relative geometry derives the base constraint");
        assert_eq!(compatibility.base_congruence().modulus(), 4);
        assert_eq!(compatibility.base_congruence().residue(), 0);
        assert!(
            !compatibility.base_congruence().admits(loan.base()),
            "actual base must discharge the derived congruence"
        );
    }

    #[test]
    fn admitted_profile_restriction_preserves_rights_and_rejected_loan() {
        let extent = uart_extent(0x3000, 12);
        let profile = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(31).expect("profile receipt"),
            &extent,
            extent_rights(&[4]),
            uart_reach(),
        )
        .expect("profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 12,
                peer: PeerWritability::Exclusive,
                stable: StableCapability::None,
                external: super::word32_access(ExternalReadBehavior::Repeatable, true),
                atomic: AtomicCapability::None,
                reach: uart_reach(),
            }],
        })
        .expect("admitted profile");
        let extent = extent
            .attenuate(extent_rights(&[3]))
            .expect("attenuated extent");
        let loan = extent.loan(0, 12).expect("UART loan");
        let rejection = profile
            .restrict_to_loan(&loan)
            .expect_err("attenuated loan cannot recover profile-bound rights");
        assert!(rejection.0.contains("lacks rights"));
        assert_eq!(loan.base(), 0x3000);
        assert_eq!(loan.length(), 12);
    }

    #[test]
    fn hostile_shared_regions_never_supply_stable() {
        let hostile_stable = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 8,
                    peer: PeerWritability::HostileShared,
                    stable: StableCapability::Read,
                    external: ExternalCapability::None,
                    atomic: AtomicCapability::None,
                    reach: BoundaryReach::default(),
                }],
            },
            8,
        )
        .expect_err("stable supply over hostile-shared memory is incoherent");
        assert!(
            hostile_stable.0.contains("hostile-shared"),
            "the rejection names the violated rule: {hostile_stable}"
        );

        let transfers = super::word32_transfers();
        let hostile_external = validate_resource_profile(
            ResourceProfile {
                regions: vec![
                    ResourceRegion {
                        offset: 0,
                        length: 4,
                        peer: PeerWritability::HostileShared,
                        stable: StableCapability::None,
                        external: ExternalCapability::Access {
                            read: ExternalReadBehavior::Repeatable,
                            write: true,
                            transfers: transfers.clone(),
                        },
                        atomic: AtomicCapability::None,
                        reach: uart_reach(),
                    },
                    ResourceRegion {
                        offset: 4,
                        length: 4,
                        peer: PeerWritability::Exclusive,
                        stable: StableCapability::Read,
                        external: ExternalCapability::None,
                        atomic: AtomicCapability::None,
                        reach: uart_reach(),
                    },
                ],
            },
            8,
        )
        .expect("hostile external supply is coherent beside exclusive stable");
        assert_eq!(
            hostile_external.regions().len(),
            2,
            "adjacent regions never merge across a peer boundary"
        );

        let child = hostile_external
            .restrict(0, 4, &uart_reach())
            .expect("restrict retains the hostile peer claim");
        assert_eq!(child.regions()[0].peer, PeerWritability::HostileShared);

        let profile_with = |peer: PeerWritability| {
            validate_resource_profile(
                ResourceProfile {
                    regions: vec![ResourceRegion {
                        offset: 0,
                        length: 4,
                        peer,
                        stable: StableCapability::None,
                        external: ExternalCapability::Access {
                            read: ExternalReadBehavior::Repeatable,
                            write: true,
                            transfers: transfers.clone(),
                        },
                        atomic: AtomicCapability::None,
                        reach: uart_reach(),
                    }],
                },
                4,
            )
            .expect("peer claim alone cannot invalidate a profile")
        };
        assert_ne!(
            profile_with(PeerWritability::Exclusive).identity(),
            profile_with(PeerWritability::HostileShared).identity(),
            "the peer claim participates in normalized identity"
        );
    }
}
