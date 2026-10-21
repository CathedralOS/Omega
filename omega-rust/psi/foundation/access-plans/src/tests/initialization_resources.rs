//! Initialization writes the complete image even when its accessors cannot.
//! These are compatibility tests, not evidence of vacancy or an executed write.

use crate::{
    AccessExposure, AccessPlan, AtomicCapability, BoundaryReach, ExternalCapability,
    PeerWritability, PlacementPlan, ResourceProfile, ResourceRegion, StableCapability,
    ValidatedPlacementPlan, validate_initialization_resources, validate_placement_plan,
    validate_placement_resources, validate_resource_profile,
};

fn plan(expose_read: bool) -> ValidatedPlacementPlan {
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
        AccessPlan::inaccessible(&layout).expect("inaccessible access")
    };
    validate_placement_plan(PlacementPlan {
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
fn read_only_view_supply_does_not_authorize_initialization() {
    let plan = plan(true);
    let profile = validate_resource_profile(
        ResourceProfile {
            regions: vec![region(0, 12, StableCapability::Read)],
        },
        12,
    )
    .expect("readable profile");
    validate_placement_resources(&plan, &profile).expect("view can read");
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
        validate_placement_resources(&plan, &profile).expect("no accessor demands");
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
