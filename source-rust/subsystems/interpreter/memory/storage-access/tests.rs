use access_admission::test_support::*;
use extent_contracts::{AddressSpaceId, ExtentProvenanceId, ExtentRights};

fn extent_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, extent_contracts::ExtentDiagnostic>,
) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn provider_issuance(seed: u64) -> crate::extents::ExtentProviderIssuance {
    let base = seed * 16;
    crate::extents::ExtentProviderIssuance::from_normalized_identities(std::array::from_fn(
        |identity_index| base + (identity_index as u64 + 1),
    ))
    .expect("normalized provider issuance")
}

fn extent_rights(identities: &[u64]) -> ExtentRights {
    ExtentRights::from_normalized_identities(identities.iter().copied().map(|identity| {
        extent_id(
            identity,
            extent_contracts::ExtentRightId::from_normalized_identity,
        )
    }))
}

fn uart_extent(base: u64, length: u64) -> crate::extents::Extent {
    uart_extent_with_lineage(base, length, 1)
}

fn uart_extent_with_lineage(base: u64, length: u64, lineage: u64) -> crate::extents::Extent {
    uart_extent_with_root(base, length, 1, lineage)
}

fn uart_extent_with_root(
    base: u64,
    length: u64,
    provider: u64,
    lineage: u64,
) -> crate::extents::Extent {
    uart_root_grant(provider, lineage)
        .mint(base, length)
        .expect("UART extent")
}

fn uart_root_grant(provider: u64, lineage: u64) -> crate::extents::ExtentRootGrant {
    uart_root_grant_with_mapping(provider, lineage, 5, 6)
}

fn uart_root_grant_with_mapping(
    provider: u64,
    lineage: u64,
    provenance: u64,
    era: u64,
) -> crate::extents::ExtentRootGrant {
    crate::extents::ExtentRootGrant::from_admitted_provider(
        provider_issuance(provider),
        extent_id(
            lineage,
            extent_contracts::ExtentLineageId::from_normalized_identity,
        ),
        extent_id(2, AddressSpaceId::from_normalized_identity),
        extent_rights(&[3, 4]),
        extent_id(provenance, ExtentProvenanceId::from_normalized_identity),
        extent_id(
            era,
            extent_contracts::MappingEraId::from_normalized_identity,
        ),
    )
}

fn stable_region(
    offset: u64,
    length: u64,
    stable: access_vocabulary::StableCapability,
    reach: access_vocabulary::BoundaryReach,
) -> access_vocabulary::ResourceRegion {
    access_vocabulary::ResourceRegion {
        offset,
        length,
        peer: access_vocabulary::PeerWritability::Exclusive,
        stable,
        external: access_vocabulary::ExternalCapability::None,
        atomic: access_vocabulary::AtomicCapability::None,
        reach,
    }
}

fn external_region(
    offset: u64,
    length: u64,
    external: access_vocabulary::ExternalCapability,
    reach: access_vocabulary::BoundaryReach,
) -> access_vocabulary::ResourceRegion {
    access_vocabulary::ResourceRegion {
        external,
        ..stable_region(
            offset,
            length,
            access_vocabulary::StableCapability::None,
            reach,
        )
    }
}

fn validated_profile(
    regions: Vec<access_vocabulary::ResourceRegion>,
    length: u64,
) -> access_admission::ValidatedResourceProfile {
    access_admission::validate_resource_profile(
        access_vocabulary::ResourceProfile { regions },
        length,
    )
    .expect("valid resource profile")
}

fn admit_extent_profile(
    receipt: u64,
    extent: &crate::extents::Extent,
    required_rights: &[u64],
    permitted_reach: access_vocabulary::BoundaryReach,
    profile: access_vocabulary::ResourceProfile,
) -> crate::storage_access::AdmittedResourceProfile {
    crate::storage_access::ResourceProfileGrant::from_admitted_provider(
        access_vocabulary::ResourceProfileReceiptId::from_normalized_identity(receipt)
            .expect("profile receipt"),
        extent,
        extent_rights(required_rights),
        permitted_reach,
    )
    .expect("profile grant")
    .admit(profile)
    .expect("admitted profile")
}

fn fields_layout(
    fingerprint: u64,
    fields: &[(&str, u64)],
    size: u64,
    align: u64,
) -> layout_plans::LayoutPlanReport {
    layout_plans::LayoutPlanReport {
        schema_report_fingerprint: fingerprint,
        entries: fields
            .iter()
            .map(|&(name, offset)| {
                layout_plans::test_support::layout_field_entry(
                    name,
                    layout_plans::LayoutPlacementReport::At { offset },
                )
            })
            .collect(),
        offsets: Some(fields.iter().map(|&(_, offset)| offset).collect()),
        size: Some(size),
        align,
    }
}

mod initialization_resources {
    use super::{admit_extent_profile, stable_region, validated_profile};
    use access_admission::{
        AccessFields, ValidatedPlacement, validate_access_resources,
        validate_initialization_resources, validate_placement, validate_resource_profile,
        validate_run_relocation_resources,
    };
    use access_vocabulary::{
        AccessExposure, BoundaryReach, ExternalReadBehavior, ResourceProfile, ResourceRegion,
        StableCapability,
    };

    fn plan(expose_read: bool) -> ValidatedPlacement {
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
        validate_placement(layout, access, super::uart_reach()).expect("placement")
    }

    fn region(offset: u64, length: u64, stable: StableCapability) -> ResourceRegion {
        stable_region(offset, length, stable, super::uart_reach())
    }

    #[test]
    fn relocation_checks_later_cells_without_enumerating_the_run() {
        let plan = plan(true);
        let count = 1_000_000_000;
        let occupied = count * 12;
        let source = validated_profile(vec![region(0, occupied, StableCapability::Read)], occupied);
        let destination = validated_profile(
            vec![region(0, occupied, StableCapability::ReadWrite)],
            occupied,
        );
        validate_run_relocation_resources(&plan, count, 4096, &source, &destination)
            .expect("a billion cells with one uniform supply region");

        let late_write_only = validated_profile(
            vec![
                region(0, occupied - 12, StableCapability::ReadWrite),
                region(occupied - 12, 12, StableCapability::Write),
            ],
            occupied,
        );
        let error =
            validate_run_relocation_resources(&plan, count, 4096, &source, &late_write_only)
                .expect_err("the last cell still owes the declared read access");
        assert!(error.0.contains("Stable read=true"));
    }

    #[test]
    fn relocation_requires_exact_windows_and_complete_source_reads() {
        let plan = plan(false);
        let readable = validated_profile(vec![region(0, 24, StableCapability::Read)], 24);
        let writable = validated_profile(vec![region(0, 24, StableCapability::Write)], 24);
        validate_run_relocation_resources(&plan, 2, 4096, &readable, &writable)
            .expect("write-only destination is sufficient for inaccessible cells");
        for (count, base, src, expected) in [
            (1, 4096, &readable, "exact occupied"),
            (2, 4097, &readable, "alignment"),
            (2, 4096, &writable, "Stable read"),
            (u64::MAX, 0, &readable, "overflows"),
        ] {
            assert!(
                validate_run_relocation_resources(&plan, count, base, src, &writable)
                    .expect_err(expected)
                    .0
                    .contains(expected)
            );
        }
    }

    #[test]
    fn relocation_distinguishes_no_cells_from_zero_footprint_cells() {
        let empty = validate_resource_profile(ResourceProfile::default(), 0).expect("empty");
        assert!(
            validate_run_relocation_resources(&plan(true), 0, 4096, &empty, &empty)
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
        let zero_sized = validate_placement(layout, access, BoundaryReach::default())
            .expect("zero-footprint placement");
        assert!(
            validate_run_relocation_resources(&zero_sized, u64::MAX, 4096, &empty, &empty)
                .expect("logical count does not enumerate zero-sized cells")
                .is_some()
        );
        assert!(
            validate_run_relocation_resources(&zero_sized, 1, 4097, &empty, &empty)
                .expect_err("zero bytes do not erase cell alignment")
                .0
                .contains("alignment")
        );
    }

    #[test]
    fn read_only_view_supply_does_not_authorize_initialization() {
        let plan = plan(true);
        let profile = validated_profile(vec![region(0, 12, StableCapability::Read)], 12);
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
            let profile = validated_profile(regions, 12);
            validate_access_resources(&plan, &profile).expect("no accessor demands");
            let error = validate_initialization_resources(&plan, &profile)
                .expect_err("whole image needs coverage");
            assert!(error.0.contains("no resource supply at byte"));
        }
    }

    #[test]
    fn adjacent_different_supply_rows_cover_one_initial_image() {
        let plan = plan(false);
        let profile = validated_profile(
            vec![
                region(4, 8, StableCapability::ReadWrite),
                region(0, 4, StableCapability::Write),
            ],
            12,
        );
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
        let profile = validated_profile(
            vec![stable_region(
                0,
                12,
                StableCapability::Write,
                BoundaryReach::default(),
            )],
            12,
        );
        let error = validate_initialization_resources(&plan, &profile)
            .expect_err("inaccessible fields do not erase initialization reach");
        assert!(error.0.contains("complete boundary reach at byte 0"));
    }

    #[test]
    fn device_write_supply_does_not_turn_initialization_into_device_programming() {
        let plan = plan(false);
        let profile = validated_profile(
            vec![super::external_region(
                0,
                12,
                super::word32_access(ExternalReadBehavior::None, true),
                super::uart_reach(),
            )],
            12,
        );
        let error = validate_initialization_resources(&plan, &profile)
            .expect_err("initialization requires Stable storage");
        assert!(error.0.contains("Stable write supply"));
    }

    #[test]
    fn initialization_preserves_subsequent_access_demands_and_stops_at_image_end() {
        let writable = validated_profile(
            vec![
                region(0, 12, StableCapability::Write),
                region(12, 4, StableCapability::Read),
            ],
            16,
        );
        validate_initialization_resources(&plan(false), &writable)
            .expect("bytes beyond the image are not written");
        let error = validate_initialization_resources(&plan(true), &writable)
            .expect_err("the resulting view still needs read permission");
        assert!(error.0.contains("requests Stable read=true"));
    }

    fn admitted(extent: &crate::extents::Extent) -> crate::storage_access::AdmittedResourceProfile {
        admit_extent_profile(
            41,
            extent,
            &[3],
            super::uart_reach(),
            ResourceProfile {
                regions: vec![region(0, extent.length(), StableCapability::ReadWrite)],
            },
        )
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

mod resource_profiles {
    use super::{
        access_plan, admit_extent_profile, extent_rights, external_region, fields_layout, reach,
        stable_region, uart_access_plan, uart_extent, uart_extent_with_lineage,
        uart_extent_with_root, uart_layout, uart_reach, uart_resource_profile_data,
        validated_profile,
    };
    use crate::storage_access::ResourceProfileGrant;
    use access_admission::{
        EffectiveSupplyKind, validate_access_resources, validate_placement,
        validate_resource_profile,
    };
    use access_vocabulary::{
        AccessExposure, AtomicCapability, AtomicPermissions, AtomicTransferRule, BoundaryReach,
        BoundaryServiceReachId, ExternalCapability, ExternalRead, ExternalReadBehavior,
        FieldAccess, PeerWritability, ResourceProfile, ResourceProfileReceiptId, ResourceRegion,
        StableCapability, TransferRule,
    };

    fn external_word32_read() -> FieldAccess {
        FieldAccess::External {
            transfer_width_bits: 32,
            read: ExternalRead::Read,
            write: false,
            exposure: AccessExposure::Exported,
        }
    }

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
        let stable = stable_region(0, 4, StableCapability::ReadWrite, broad_reach.clone());
        let profile = validated_profile(
            vec![
                ResourceRegion {
                    offset: 4,
                    ..stable.clone()
                },
                stable.clone(),
                external_region(
                    8,
                    8,
                    super::word32_access(ExternalReadBehavior::Repeatable, false),
                    broad_reach,
                ),
            ],
            16,
        );
        assert_eq!(
            profile.regions().len(),
            2,
            "adjacent identical regions normalize into one interval"
        );
        assert_eq!(
            (profile.regions()[0].offset, profile.regions()[0].length),
            (0, 8)
        );

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
                    stable_region(2, 4, StableCapability::Read, BoundaryReach::default()),
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
        let stable_profile = validated_profile(
            vec![stable_region(
                0,
                12,
                StableCapability::ReadWrite,
                uart_reach(),
            )],
            12,
        );
        let compatibility = validate_access_resources(&plan, &stable_profile)
            .expect("stable supply may conservatively satisfy external demand");
        assert!(
            compatibility
                .fields()
                .iter()
                .all(|field| field.kind() == EffectiveSupplyKind::Stable)
        );
        assert_eq!(
            (
                compatibility.base_congruence().modulus(),
                compatibility.base_congruence().residue()
            ),
            (4, 0)
        );

        let read_only_external = validated_profile(
            vec![external_region(
                0,
                12,
                super::word32_access(ExternalReadBehavior::Repeatable, false),
                uart_reach(),
            )],
            12,
        );
        let error = validate_access_resources(&plan, &read_only_external)
            .expect_err("read-only external supply cannot satisfy UART writes");
        assert!(
            error.0.contains("transmit") && error.0.contains("incompatible External"),
            "canonical field order reports the first unsupported UART write: {error}"
        );

        let wrong_width = validated_profile(
            vec![external_region(
                0,
                12,
                ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: true,
                    transfers: vec![TransferRule {
                        width_bits: 64,
                        alignment_bytes: 8,
                    }],
                },
                uart_reach(),
            )],
            12,
        );
        let error = validate_access_resources(&plan, &wrong_width)
            .expect_err("transfer width must match exactly");
        assert!(
            error.0.contains("control") && error.0.contains("32-bit"),
            "canonical field order reports the first width mismatch: {error}"
        );

        let stable_demand = validate_placement(
            uart_layout(),
            access_plan(
                &uart_layout(),
                &[(
                    "status",
                    super::stable_word32_read(AccessExposure::Exported),
                )],
            ),
            uart_reach(),
        )
        .expect("stable demand");
        let error = validate_access_resources(&stable_demand, &read_only_external)
            .expect_err("external supply cannot satisfy Stable demand");
        assert!(error.0.contains("requests Stable"));

        let destructive_demand = validate_placement(
            uart_layout(),
            access_plan(
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
            uart_reach(),
        )
        .expect("destructive external demand");
        let error = validate_access_resources(&destructive_demand, &read_only_external)
            .expect_err("repeatable reads cannot satisfy destructive observation");
        assert!(
            error.0.contains("status") && error.0.contains("Take"),
            "observation mismatch must name the destructive demand: {error}"
        );

        let atomic_layout = fields_layout(93, &[("head", 0)], 4, 4);
        let placement_fields = access_plan(
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
        );
        let atomic_demand =
            validate_placement(atomic_layout, placement_fields, BoundaryReach::default())
                .expect("atomic demand");
        let load_only = validated_profile(
            vec![ResourceRegion {
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
                ..stable_region(0, 4, StableCapability::None, BoundaryReach::default())
            }],
            4,
        );
        let error = validate_access_resources(&atomic_demand, &load_only)
            .expect_err("atomic operation demand must be an exact supply subset");
        assert!(
            error.0.contains("head") && error.0.contains("operation families"),
            "atomic mismatch must name the field and operation family: {error}"
        );
    }

    #[test]
    fn subrange_loan_rebases_profile_and_preserves_denied_bytes() {
        let layout = fields_layout(94, &[("word", 0)], 4, 4);
        let placement_fields = access_plan(&layout, &[("word", external_word32_read())]);
        let placement = validate_placement(layout, placement_fields, BoundaryReach::default())
            .expect("subrange placement");
        let extent = uart_extent(0x4000, 16);
        let profile = admit_extent_profile(
            41,
            &extent,
            &[3],
            BoundaryReach::default(),
            ResourceProfile {
                regions: vec![external_region(
                    4,
                    4,
                    super::word32_access(ExternalReadBehavior::Repeatable, false),
                    BoundaryReach::default(),
                )],
            },
        );

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
        assert_eq!((restored.base(), restored.length()), (0x4000, 16));
    }

    #[test]
    fn admitted_profile_rejects_coincident_independent_extent_root() {
        let admitted_root = uart_extent_with_lineage(0x6000, 12, 61);
        let profile = admit_extent_profile(
            62,
            &admitted_root,
            &[3],
            uart_reach(),
            uart_resource_profile_data(12, &uart_reach()),
        );

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
        let conflicting_layout = fields_layout(91, &[("left", 0), ("right", 2)], 8, 2);
        let placement_fields = access_plan(
            &conflicting_layout,
            &[
                ("left", external_word32_read()),
                ("right", external_word32_read()),
            ],
        );
        let conflicting = validate_placement(
            conflicting_layout,
            placement_fields,
            BoundaryReach::default(),
        )
        .expect("relative geometry is structurally valid");
        let profile = validated_profile(
            vec![external_region(
                0,
                8,
                super::word32_access(ExternalReadBehavior::Repeatable, false),
                BoundaryReach::default(),
            )],
            8,
        );
        let error = validate_access_resources(&conflicting, &profile)
            .expect_err("inconsistent field congruences must reject before admission");
        assert!(
            error.0.contains("right")
                && error.0.contains("offset 2")
                && error.0.contains("conflicts")
        );

        let layout = fields_layout(92, &[("word", 0)], 4, 1);
        let placement_fields = access_plan(&layout, &[("word", external_word32_read())]);
        let placement = validate_placement(layout, placement_fields, BoundaryReach::default())
            .expect("single-field placement");
        let extent = uart_extent(0x1002, 4);
        let loan = extent.loan(0, 4).expect("misaligned loan");
        let resources = admit_extent_profile(
            22,
            &extent,
            &[3],
            BoundaryReach::default(),
            ResourceProfile {
                regions: vec![external_region(
                    0,
                    4,
                    super::word32_access(ExternalReadBehavior::Repeatable, false),
                    BoundaryReach::default(),
                )],
            },
        );
        let restricted = resources.restrict_to_loan(&loan).expect("same backing");
        let compatibility = validate_access_resources(&placement, &restricted)
            .expect("relative geometry derives the base constraint");
        assert_eq!(
            (
                compatibility.base_congruence().modulus(),
                compatibility.base_congruence().residue()
            ),
            (4, 0)
        );
        assert!(
            !compatibility.base_congruence().admits(loan.base()),
            "actual base must discharge the derived congruence"
        );
    }

    #[test]
    fn admitted_profile_restriction_preserves_rights_and_rejected_loan() {
        let extent = uart_extent(0x3000, 12);
        let profile = admit_extent_profile(
            31,
            &extent,
            &[4],
            uart_reach(),
            ResourceProfile {
                regions: vec![external_region(
                    0,
                    12,
                    super::word32_access(ExternalReadBehavior::Repeatable, true),
                    uart_reach(),
                )],
            },
        );
        let extent = extent
            .attenuate(extent_rights(&[3]))
            .expect("attenuated extent");
        let loan = extent.loan(0, 12).expect("UART loan");
        let rejection = profile
            .restrict_to_loan(&loan)
            .expect_err("attenuated loan cannot recover profile-bound rights");
        assert!(rejection.0.contains("lacks rights"));
        assert_eq!((loan.base(), loan.length()), (0x3000, 12));
    }

    #[test]
    fn hostile_shared_regions_never_supply_stable() {
        let hostile_stable = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    peer: PeerWritability::HostileShared,
                    ..stable_region(0, 8, StableCapability::Read, BoundaryReach::default())
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
        let word32_ext = |peer: PeerWritability| ResourceRegion {
            peer,
            ..external_region(
                0,
                4,
                ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: true,
                    transfers: transfers.clone(),
                },
                uart_reach(),
            )
        };
        let hostile_external = validated_profile(
            vec![
                word32_ext(PeerWritability::HostileShared),
                stable_region(4, 4, StableCapability::Read, uart_reach()),
            ],
            8,
        );
        assert_eq!(
            hostile_external.regions().len(),
            2,
            "adjacent regions never merge across a peer boundary"
        );

        let child = hostile_external
            .restrict(0, 4, &uart_reach())
            .expect("restrict retains the hostile peer claim");
        assert_eq!(child.regions()[0].peer, PeerWritability::HostileShared);

        let profile_with = |peer: PeerWritability| validated_profile(vec![word32_ext(peer)], 4);
        assert_ne!(
            profile_with(PeerWritability::Exclusive).identity(),
            profile_with(PeerWritability::HostileShared).identity(),
            "the peer claim participates in normalized identity"
        );
    }
}
