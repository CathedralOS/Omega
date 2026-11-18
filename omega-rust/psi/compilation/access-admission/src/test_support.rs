//! Tests the validators consumed by compiler and runtime access paths.

use crate::{
    AccessExposure, AccessFieldEntry, AccessFieldKey, AccessFields, AtomicCapability,
    BoundaryReach, BoundaryServiceReachId, ExternalCapability, ExternalRead, ExternalReadBehavior,
    FieldAccess, PeerWritability, ResourceProfile, ResourceRegion, StableCapability, TransferRule,
    ValidatedAccessFields, ValidatedPlacement, validate_access_fields, validate_placement,
};
use layout_plans::test_support::layout_field_entry;
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};

pub fn reach() -> BoundaryServiceReachId {
    BoundaryServiceReachId::from_normalized_identity(7).expect("normalized reach")
}

pub fn word32_transfers() -> Vec<TransferRule> {
    vec![TransferRule {
        width_bits: 32,
        alignment_bytes: 4,
    }]
}

pub fn word32_access(read: ExternalReadBehavior, write: bool) -> ExternalCapability {
    ExternalCapability::Access {
        read,
        write,
        transfers: word32_transfers(),
    }
}

pub fn stable_word32_read(exposure: AccessExposure) -> FieldAccess {
    FieldAccess::Stable {
        transfer_width_bits: 32,
        read: true,
        write: false,
        exposure,
    }
}

pub fn uart_reach() -> BoundaryReach {
    BoundaryReach::from_services([reach()])
}

pub fn uart_layout() -> LayoutPlanReport {
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

pub fn access_plan(layout: &LayoutPlanReport, decisions: &[(&str, FieldAccess)]) -> AccessFields {
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

pub fn field_key(plan: &ValidatedAccessFields, field: &str) -> AccessFieldKey {
    plan.plan()
        .entries()
        .iter()
        .find(|entry| entry.field() == field)
        .map(AccessFieldEntry::key)
        .expect("validated schema field key")
}

pub fn uart_access_source(layout: &LayoutPlanReport) -> AccessFields {
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

pub fn uart_access_fields() -> ValidatedAccessFields {
    let layout = uart_layout();
    let plan = uart_access_source(&layout);
    validate_access_fields(plan, &layout).expect("UART plan")
}

pub fn uart_access_plan() -> ValidatedPlacement {
    let layout = uart_layout();
    let placement_fields = uart_access_source(&layout);
    validate_placement(layout, placement_fields, uart_reach()).expect("UART access plan")
}

pub fn uart_resource_profile_data(length: u64, reach: &BoundaryReach) -> ResourceProfile {
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
