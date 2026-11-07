//! Initialization writes the complete image even when its accessors cannot.
//! These are compatibility tests, not evidence of vacancy or an executed write.

use crate::{
    AccessExposure, AccessFields, AccessPlan, AtomicCapability, BoundaryReach, ExternalCapability,
    PeerWritability, ResourceProfile, ResourceRegion, StableCapability, ValidatedAccessPlan,
    validate_access_plan, validate_access_resources, validate_initialization_resources,
    validate_resource_profile,
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
    let empty = validate_resource_profile(ResourceProfile::default(), 0).expect("empty profile");
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
    let error =
        validate_initialization_resources(&plan, &profile).expect_err("initialization must write");
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
