//! Optimizer module role: policy table. Canonical settled `TimeHost` cohort
//! dispositions.
//!
//! The canonical `TimeHost` raw boundary declared in
//! `source/library/std/time_host.omg` is a clock read and sleep seam: no
//! closed `TerminalAuthorityClass` covers a monotonic or wall-clock read, a
//! ticks/units/epoch constant, or a millisecond sleep, so every canonical
//! requirement settles to the explicit empty classification. That empty set
//! is itself the reviewed disposition — service reach and exact review
//! identity are retained while no closed class is exercised.
//!
//! Requirement names only locate rows inside the canonical checked schema; the
//! emitted permission and classification rows are still keyed by the exact
//! schema commitment, the complete normalized requirement identity, and the
//! exact role-tagged mechanism identity. A name outside the canonical cohort
//! is not a disposition: lookup fails closed. Demand completeness never
//! fabricates a broad union.

use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityDisposition, TerminalMechanismIdentity,
    provider_plan::{ServiceMethod, ServiceSchema, ServiceSchemaDigest},
};

use super::TerminalAuthorityPolicyRow;

/// The named requirement has no settled canonical `TimeHost` disposition
/// usable for the requested row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsettledTimeHostRequirement {
    /// The name is not a canonical `TimeHost` cohort member. No disposition
    /// is inferred from a readable name.
    UnknownRequirement,
}

/// Whether the named requirement belongs to the canonical `TimeHost` cohort.
/// Every member shares the explicit empty disposition: no closed authority
/// class covers a clock read, a clock constant, or a sleep.
pub fn settled_time_host_cohort(name: &str) -> bool {
    matches!(
        name,
        "monotonic_ticks"
            | "monotonic_ticks_per_second"
            | "wall_clock_raw"
            | "wall_clock_units_per_second"
            | "wall_clock_epoch_offset_seconds"
            | "sleep"
    )
}

/// Emit the exact consumer permission row for one `TimeHost` schema method.
/// `method.name` locates the settled cohort; the emitted row is keyed by the
/// supplied schema commitment and the method's complete normalized
/// requirement identity, and grants the explicit empty disposition.
pub fn time_host_permission_row(
    service_schema: ServiceSchemaDigest,
    method: &ServiceMethod,
) -> Result<ServiceTerminalAuthorityPermission, UnsettledTimeHostRequirement> {
    if !settled_time_host_cohort(&method.name) {
        return Err(UnsettledTimeHostRequirement::UnknownRequirement);
    }
    Ok(ServiceTerminalAuthorityPermission::new(
        service_schema,
        method.requirement_identity.clone(),
        TerminalAuthorityDisposition::from_classes([]),
    ))
}

/// Emit the complete consumer permission table for one canonical `TimeHost`
/// schema. Every method must resolve to the settled cohort; unknown members
/// fail the whole table rather than emitting a partial one.
pub fn time_host_permission_rows(
    schema: &ServiceSchema,
) -> Result<Vec<ServiceTerminalAuthorityPermission>, UnsettledTimeHostRequirement> {
    let digest = schema.identity_digest();
    schema
        .methods
        .iter()
        .map(|method| time_host_permission_row(digest, method))
        .collect()
}

/// Emit one exact receiving-policy row classifying the mechanism realization
/// admitted for the named `TimeHost` requirement. The row is keyed by the
/// exact mechanism identity; `method.name` only selects the settled cohort's
/// explicit empty disposition.
pub fn time_host_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    method: &ServiceMethod,
) -> Result<TerminalAuthorityPolicyRow, UnsettledTimeHostRequirement> {
    if !settled_time_host_cohort(&method.name) {
        return Err(UnsettledTimeHostRequirement::UnknownRequirement);
    }
    Ok(TerminalAuthorityPolicyRow::new(
        mechanism,
        TerminalAuthorityDisposition::from_classes([]),
    ))
}
