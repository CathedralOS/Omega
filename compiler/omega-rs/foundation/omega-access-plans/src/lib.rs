//! Normalized access policy for placed views.
//!
//! `LayoutPlan` owns geometry. `AccessPlan` owns observation and the exact
//! primitive operations permitted over that geometry. Keeping them separate
//! prevents wire layouts from acquiring MMIO vocabulary and prevents an
//! arbitrary-offset volatile escape hatch from bypassing plan validation.

use std::collections::BTreeSet;

use omega_layout_plans::{LayoutPlacementReport, LayoutPlanReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryServiceReachId(u64);

impl BoundaryServiceReachId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
        if identity == 0 {
            return Err(AccessPlanDiagnostic(
                "boundary-service reach identity cannot be zero".into(),
            ));
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// How repeated observations of the placed field relate to one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObservationModel {
    /// Ordinary owned or immutably borrowed storage. The compiler may use its
    /// ordinary load/store rules.
    Stable,
    /// Another agent may change storage. Every authorized read/write is one
    /// exact-width external event; device ordering still requires fences.
    External,
    /// Shared mutation is legal only through the declared atomic operations.
    Atomic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccessExposure {
    Exported,
    ProviderPrivate,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AtomicPermissions {
    pub load: bool,
    pub store: bool,
    pub compare_exchange: bool,
    pub read_modify_write: bool,
}

impl AtomicPermissions {
    pub const fn any(self) -> bool {
        self.load || self.store || self.compare_exchange || self.read_modify_write
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessPermissions {
    pub read: bool,
    pub write: bool,
    /// Explicit permission for a read-followed-by-write primitive. This is not
    /// inferred merely because both read and write are present.
    pub read_modify_write: bool,
    pub atomic: AtomicPermissions,
}

impl AccessPermissions {
    pub const fn any(self) -> bool {
        self.read || self.write || self.read_modify_write || self.atomic.any()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessFieldEntry {
    pub field: String,
    /// Width of the actual memory transaction, not the logical bit fragment.
    pub transfer_width_bits: u16,
    pub observation: ObservationModel,
    pub permissions: AccessPermissions,
    pub exposure: AccessExposure,
    /// Statically normalized service reach contributed by every primitive
    /// access. Runtime extent provenance cannot rewrite this value.
    pub service_reach: Option<BoundaryServiceReachId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessPlan {
    pub entries: Vec<AccessFieldEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAccessPlan(AccessPlan);

impl ValidatedAccessPlan {
    pub const fn plan(&self) -> &AccessPlan {
        &self.0
    }

    pub fn field(&self, field: &str) -> Option<&AccessFieldEntry> {
        self.0.entries.iter().find(|entry| entry.field == field)
    }

    pub fn authorize(
        &self,
        field: &str,
        borrow: BorrowPolarity,
        operation: AccessOperation,
    ) -> Result<(), AccessPlanDiagnostic> {
        let entry = self.field(field).ok_or_else(|| {
            AccessPlanDiagnostic(format!(
                "field `{field}` has no access entry in the validated plan"
            ))
        })?;
        authorize_entry(entry, borrow, operation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BorrowPolarity {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccessOperation {
    Read,
    Write,
    ReadModifyWrite,
    AtomicLoad,
    AtomicStore,
    AtomicCompareExchange,
    AtomicReadModifyWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPlanDiagnostic(pub String);

impl std::fmt::Display for AccessPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AccessPlanDiagnostic {}

pub fn validate_access_plan(
    plan: AccessPlan,
    layout: &LayoutPlanReport,
) -> Result<ValidatedAccessPlan, AccessPlanDiagnostic> {
    let layout_size = layout.size.ok_or_else(|| {
        AccessPlanDiagnostic("placed access requires a fixed-size layout plan".into())
    })?;
    let layout_size = u64::try_from(layout_size).map_err(|_| {
        AccessPlanDiagnostic(format!(
            "placed access cannot use negative layout size {layout_size}"
        ))
    })?;

    let mut fields = BTreeSet::new();
    for entry in &plan.entries {
        if entry.field.is_empty() {
            return Err(AccessPlanDiagnostic(
                "access entry field name cannot be empty".into(),
            ));
        }
        if !fields.insert(entry.field.as_str()) {
            return Err(AccessPlanDiagnostic(format!(
                "field `{}` has more than one access entry",
                entry.field
            )));
        }
    }
    for entry in &plan.entries {
        validate_entry_policy(entry)?;
        validate_entry_geometry(entry, layout, layout_size)?;
    }

    Ok(ValidatedAccessPlan(plan))
}

fn validate_entry_policy(entry: &AccessFieldEntry) -> Result<(), AccessPlanDiagnostic> {
    if entry.transfer_width_bits == 0
        || entry.transfer_width_bits > 128
        || !entry.transfer_width_bits.is_multiple_of(8)
    {
        return Err(AccessPlanDiagnostic(format!(
            "field `{}` transfer width {} is not a supported whole-byte width in 8..=128",
            entry.field, entry.transfer_width_bits
        )));
    }
    if !entry.permissions.any() {
        return Err(AccessPlanDiagnostic(format!(
            "field `{}` exposes no primitive operation",
            entry.field
        )));
    }

    match entry.observation {
        ObservationModel::Atomic => {
            if entry.permissions.read
                || entry.permissions.write
                || entry.permissions.read_modify_write
            {
                return Err(AccessPlanDiagnostic(format!(
                    "atomic field `{}` cannot also expose ordinary read/write/RMW",
                    entry.field
                )));
            }
            if !entry.permissions.atomic.any() {
                return Err(AccessPlanDiagnostic(format!(
                    "atomic field `{}` exposes no atomic operation",
                    entry.field
                )));
            }
        }
        ObservationModel::Stable | ObservationModel::External => {
            if entry.permissions.atomic.any() {
                return Err(AccessPlanDiagnostic(format!(
                    "non-atomic field `{}` cannot expose atomic operations",
                    entry.field
                )));
            }
            if entry.permissions.read_modify_write
                && !(entry.permissions.read && entry.permissions.write)
            {
                return Err(AccessPlanDiagnostic(format!(
                    "field `{}` RMW requires both read and write permission",
                    entry.field
                )));
            }
        }
    }

    if entry.observation == ObservationModel::External && entry.service_reach.is_none() {
        return Err(AccessPlanDiagnostic(format!(
            "external field `{}` must pin boundary-service reach",
            entry.field
        )));
    }
    if entry.observation == ObservationModel::Stable && entry.service_reach.is_some() {
        return Err(AccessPlanDiagnostic(format!(
            "stable field `{}` cannot disguise a boundary event as ordinary access",
            entry.field
        )));
    }
    if entry.observation == ObservationModel::External
        && entry.permissions.read_modify_write
        && entry.exposure != AccessExposure::ProviderPrivate
    {
        return Err(AccessPlanDiagnostic(format!(
            "external field `{}` may expose RMW only through a provider-private primitive",
            entry.field
        )));
    }
    Ok(())
}

fn validate_entry_geometry(
    access: &AccessFieldEntry,
    layout: &LayoutPlanReport,
    layout_size: u64,
) -> Result<(), AccessPlanDiagnostic> {
    let placements = layout
        .entries
        .iter()
        .filter(|entry| entry.field == access.field)
        .map(|entry| entry.placement)
        .collect::<Vec<_>>();
    if placements.is_empty() {
        return Err(AccessPlanDiagnostic(format!(
            "access field `{}` does not exist in the layout plan",
            access.field
        )));
    }

    let transfer_bytes = u64::from(access.transfer_width_bits / 8);
    match placements.as_slice() {
        [LayoutPlacementReport::At { offset }] => {
            let offset = u64::try_from(*offset).map_err(|_| {
                AccessPlanDiagnostic(format!(
                    "access field `{}` has negative layout offset {offset}",
                    access.field
                ))
            })?;
            validate_transfer_range(access, offset, transfer_bytes, layout_size)
        }
        placements => {
            let mut container = None;
            for placement in placements {
                let LayoutPlacementReport::Bits {
                    container: candidate,
                    container_width,
                    ..
                } = placement
                else {
                    return Err(AccessPlanDiagnostic(format!(
                        "access field `{}` mixes whole and fragmented placement",
                        access.field
                    )));
                };
                if *container_width != i64::from(access.transfer_width_bits) {
                    return Err(AccessPlanDiagnostic(format!(
                        "access field `{}` requests a {}-bit transfer over a {container_width}-bit container",
                        access.field, access.transfer_width_bits
                    )));
                }
                if container
                    .replace(*candidate)
                    .is_some_and(|prior| prior != *candidate)
                {
                    return Err(AccessPlanDiagnostic(format!(
                        "fragmented field `{}` spans multiple containers and cannot be projected through one exact access",
                        access.field
                    )));
                }
            }
            let container = container.expect("nonempty placements");
            let container = u64::try_from(container).map_err(|_| {
                AccessPlanDiagnostic(format!(
                    "access field `{}` has negative container offset {container}",
                    access.field
                ))
            })?;
            validate_transfer_range(access, container, transfer_bytes, layout_size)
        }
    }
}

fn validate_transfer_range(
    access: &AccessFieldEntry,
    offset: u64,
    transfer_bytes: u64,
    layout_size: u64,
) -> Result<(), AccessPlanDiagnostic> {
    let end = offset.checked_add(transfer_bytes).ok_or_else(|| {
        AccessPlanDiagnostic(format!(
            "access field `{}` transfer byte range overflows",
            access.field
        ))
    })?;
    if end > layout_size {
        return Err(AccessPlanDiagnostic(format!(
            "access field `{}` transfer at {offset}..{end} exceeds {layout_size}-byte layout",
            access.field
        )));
    }
    Ok(())
}

fn authorize_entry(
    entry: &AccessFieldEntry,
    borrow: BorrowPolarity,
    operation: AccessOperation,
) -> Result<(), AccessPlanDiagnostic> {
    let permitted = match operation {
        AccessOperation::Read => entry.permissions.read,
        AccessOperation::Write => entry.permissions.write && borrow == BorrowPolarity::Exclusive,
        AccessOperation::ReadModifyWrite => {
            entry.permissions.read_modify_write && borrow == BorrowPolarity::Exclusive
        }
        AccessOperation::AtomicLoad => entry.permissions.atomic.load,
        AccessOperation::AtomicStore => entry.permissions.atomic.store,
        AccessOperation::AtomicCompareExchange => entry.permissions.atomic.compare_exchange,
        AccessOperation::AtomicReadModifyWrite => entry.permissions.atomic.read_modify_write,
    };
    if permitted {
        Ok(())
    } else {
        Err(AccessPlanDiagnostic(format!(
            "field `{}` does not permit {operation:?} through a {borrow:?} borrow",
            entry.field
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};

    fn reach() -> BoundaryServiceReachId {
        BoundaryServiceReachId::from_normalized_identity(7).expect("normalized reach")
    }

    fn uart_layout() -> LayoutPlanReport {
        LayoutPlanReport {
            entries: vec![
                LayoutFieldEntryReport {
                    field: "status".into(),
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "transmit".into(),
                    placement: LayoutPlacementReport::At { offset: 4 },
                },
                LayoutFieldEntryReport {
                    field: "control".into(),
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

    fn ordinary(read: bool, write: bool, rmw: bool) -> AccessPermissions {
        AccessPermissions {
            read,
            write,
            read_modify_write: rmw,
            atomic: AtomicPermissions::default(),
        }
    }

    #[test]
    fn uart_access_plan_validates_geometry_reach_and_borrow_polarity() {
        let plan = validate_access_plan(
            AccessPlan {
                entries: vec![
                    AccessFieldEntry {
                        field: "status".into(),
                        transfer_width_bits: 32,
                        observation: ObservationModel::External,
                        permissions: ordinary(true, false, false),
                        exposure: AccessExposure::Exported,
                        service_reach: Some(reach()),
                    },
                    AccessFieldEntry {
                        field: "transmit".into(),
                        transfer_width_bits: 32,
                        observation: ObservationModel::External,
                        permissions: ordinary(false, true, false),
                        exposure: AccessExposure::Exported,
                        service_reach: Some(reach()),
                    },
                    AccessFieldEntry {
                        field: "control".into(),
                        transfer_width_bits: 32,
                        observation: ObservationModel::External,
                        permissions: ordinary(true, true, true),
                        exposure: AccessExposure::ProviderPrivate,
                        service_reach: Some(reach()),
                    },
                ],
            },
            &uart_layout(),
        )
        .expect("UART plan");

        plan.authorize("status", BorrowPolarity::Shared, AccessOperation::Read)
            .expect("shared snapshot read");
        assert!(
            plan.authorize("transmit", BorrowPolarity::Shared, AccessOperation::Write)
                .is_err()
        );
        plan.authorize(
            "transmit",
            BorrowPolarity::Exclusive,
            AccessOperation::Write,
        )
        .expect("exclusive whole write");
        plan.authorize(
            "control",
            BorrowPolarity::Exclusive,
            AccessOperation::ReadModifyWrite,
        )
        .expect("provider-private explicit RMW");
    }

    #[test]
    fn exported_external_rmw_and_missing_reach_reject() {
        let mut entry = AccessFieldEntry {
            field: "status".into(),
            transfer_width_bits: 32,
            observation: ObservationModel::External,
            permissions: ordinary(true, true, true),
            exposure: AccessExposure::Exported,
            service_reach: Some(reach()),
        };
        let error = validate_access_plan(
            AccessPlan {
                entries: vec![entry.clone()],
            },
            &uart_layout(),
        )
        .expect_err("public MMIO RMW must not be derived");
        assert!(error.0.contains("provider-private"));

        entry.permissions.read_modify_write = false;
        entry.service_reach = None;
        let error = validate_access_plan(
            AccessPlan {
                entries: vec![entry],
            },
            &uart_layout(),
        )
        .expect_err("external access cannot launder service reach");
        assert!(error.0.contains("reach"));
    }

    #[test]
    fn atomic_shared_page_exposes_only_atomic_mutation() {
        let layout = LayoutPlanReport {
            entries: vec![LayoutFieldEntryReport {
                field: "head".into(),
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        let plan = validate_access_plan(
            AccessPlan {
                entries: vec![AccessFieldEntry {
                    field: "head".into(),
                    transfer_width_bits: 32,
                    observation: ObservationModel::Atomic,
                    permissions: AccessPermissions {
                        atomic: AtomicPermissions {
                            load: true,
                            store: true,
                            compare_exchange: true,
                            read_modify_write: true,
                        },
                        ..AccessPermissions::default()
                    },
                    exposure: AccessExposure::Exported,
                    service_reach: None,
                }],
            },
            &layout,
        )
        .expect("atomic IPC plan");

        plan.authorize("head", BorrowPolarity::Shared, AccessOperation::AtomicStore)
            .expect("shared mutation is explicitly atomic");
        assert!(
            plan.authorize("head", BorrowPolarity::Exclusive, AccessOperation::Write)
                .is_err()
        );
    }

    #[test]
    fn multi_container_fragments_are_not_one_access() {
        let layout = LayoutPlanReport {
            entries: vec![
                LayoutFieldEntryReport {
                    field: "entry".into(),
                    placement: LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 32,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 16,
                    },
                },
                LayoutFieldEntryReport {
                    field: "entry".into(),
                    placement: LayoutPlacementReport::Bits {
                        container: 4,
                        container_width: 32,
                        destination_lsb: 0,
                        source_lsb: 16,
                        width: 16,
                    },
                },
            ],
            offsets: None,
            size: Some(8),
            align: 4,
        };
        let error = validate_access_plan(
            AccessPlan {
                entries: vec![AccessFieldEntry {
                    field: "entry".into(),
                    transfer_width_bits: 32,
                    observation: ObservationModel::Stable,
                    permissions: ordinary(true, false, false),
                    exposure: AccessExposure::Exported,
                    service_reach: None,
                }],
            },
            &layout,
        )
        .expect_err("one token cannot hide two primitive accesses");
        assert!(error.0.contains("multiple containers"));
    }

    #[test]
    fn duplicate_and_unknown_access_fields_reject() {
        let entry = AccessFieldEntry {
            field: "missing".into(),
            transfer_width_bits: 32,
            observation: ObservationModel::Stable,
            permissions: ordinary(true, false, false),
            exposure: AccessExposure::Exported,
            service_reach: None,
        };
        let error = validate_access_plan(
            AccessPlan {
                entries: vec![entry.clone(), entry],
            },
            &uart_layout(),
        )
        .expect_err("duplicate fields reject before geometry");
        assert!(error.0.contains("more than one"));

        let error = validate_access_plan(
            AccessPlan {
                entries: vec![AccessFieldEntry {
                    field: "missing".into(),
                    transfer_width_bits: 32,
                    observation: ObservationModel::Stable,
                    permissions: ordinary(true, false, false),
                    exposure: AccessExposure::Exported,
                    service_reach: None,
                }],
            },
            &uart_layout(),
        )
        .expect_err("unknown field rejects");
        assert!(error.0.contains("does not exist"));
    }
}
