//! Compile-time wire compatibility: compare published schemas against the
//! requested directional facts and reject unmet demands.
//!
//! These are target-neutral queries over typed source. The build supplies
//! demands; it does not implement their meaning.
//! Schema projection, directional comparisons and migration/retirement checking
//! have separate owners below.

use diagnostics::Diagnostic;

mod demands;
mod migration;
mod observations;
mod schemas;

use language_semantics::wire::WireCompatibilityDemand;
use observations::{WireCase, WireDemandVerdict, WireFact, WireField, WireShape};
use schemas::check_wire_demands;

/// Reject unmet directional compatibility facts.
pub fn validate_wire_protocol(
    typed: &dyn crate::CheckingInput,
    compatibility_demands: &[WireCompatibilityDemand],
) -> Result<(), Vec<Diagnostic>> {
    validate_wire_demands(&check_wire_demands(typed, compatibility_demands))
}

fn validate_wire_demands(demands: &[WireDemandVerdict]) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = demands
        .iter()
        .filter(|demand| !demand.satisfied)
        .map(|demand| {
            let failed = [
                ("readability", &demand.readability),
                ("writability", &demand.writability),
                ("unknown preservation", &demand.unknown_preservation),
                ("canonicality", &demand.canonicality),
                ("migration coverage", &demand.migration_coverage),
            ]
            .into_iter()
            .filter(|(_, fact)| fact.required && !fact.satisfied)
            .map(|(name, fact)| format!("{name} ({})", fact.detail))
            .collect::<Vec<_>>()
            .join(", ");
            Diagnostic::error(format!(
                "wire compatibility demand `{}` is unsatisfied for local schema `{}` and peer \
                 schema `{}`: {}",
                demand.edge, demand.local_schema, demand.peer_schema, failed
            ))
        })
        .collect::<Vec<_>>();
    crate::program_validation::finish_strict_diagnostics(diagnostics)
}
