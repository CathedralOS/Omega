//! Structural Stable permissions cover storage, never authorize a giant scalar
//! operation. These pure judgments are prerequisites, not executed access.
use crate::{
    AccessExposure, AccessOperation, AccessPlan, AtomicCapability, BorrowPolarity, BoundaryReach,
    ExternalCapability, FieldAccess, PeerWritability, PlacementPlan, ResourceProfile,
    ResourceRegion, StableCapability, ValidatedPlacementPlan, validate_placement_plan,
    validate_placement_resources, validate_resource_profile,
};
use layout_plans::LayoutPlacementReport;
use layout_plans::test_support::{layout_field_entry, layout_report};

fn page_plan(read: bool, write: bool, byte_length: u64) -> ValidatedPlacementPlan {
    let layout = layout_report(
        vec![layout_field_entry(
            "words".into(),
            LayoutPlacementReport::At { offset: 8 },
        )],
        None,
        Some(4104),
        8,
    );
    let mut access = AccessPlan::inaccessible(&layout).expect("schema slots");
    let key = access.key_at(0).expect("one field");
    access
        .set(
            key,
            FieldAccess::StableRegion {
                byte_length,
                byte_alignment: 8,
                read,
                write,
                exposure: AccessExposure::Exported,
            },
        )
        .expect("exact field key");
    validate_placement_plan(PlacementPlan {
        layout,
        access,
        reach: BoundaryReach::default(),
    })
    .expect("structural page region")
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
    let region = plan.access().stable_region(key).expect("region descriptor");
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
fn stable_region_supply_covers_all_bytes_and_requested_operations() {
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
    let compatible = validate_placement_resources(&plan, &profile).expect("complete read supply");
    assert!(compatible.base_congruence().admits(4096));
    assert!(!compatible.base_congruence().admits(4097));
    assert!(validate_placement_resources(&page_plan(true, true, 4096), &profile).is_err());
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
        assert!(validate_placement_resources(&plan, &profile).is_err());
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
    assert!(crate::validate_access_plan(scalar, plan.layout()).is_err());
}

#[test]
fn projected_transfer_keeps_parent_range_permissions_and_source_borrow() {
    let plan = page_plan(true, true, 4096);
    let key = plan.access().plan().key_at(0).expect("parent field");
    for offset in [8, 4096] {
        let authorized = plan
            .access()
            .authorize_stable_projection(
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
                .authorize_stable_projection(
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
            .authorize_stable_projection(
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
            .authorize_stable_projection(
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
            .authorize_stable_projection(
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
            .authorize_stable_projection(
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
