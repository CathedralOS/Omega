//! Compile-time wire compatibility: probe generated codecs, compare published
//! schemas against the requested directional facts, and reject unmet demands.
//!
//! These are target-neutral queries over typed source and interpreted codec
//! behavior. The build supplies demands; it does not implement their meaning.
//! Schema projection, directional comparisons and migration/retirement checking
//! have separate owners below. Finite codec probes never establish derived trust.

use checked_interpreter::WireCodecVerification;
use diagnostics::Diagnostic;
use std::collections::BTreeMap;
use typed_trees::TypedTrees;

mod demands;
mod migration;
mod observations;
mod schemas;

use language_semantics::wire::WireCompatibilityDemand;
use observations::{WireCase, WireDemandVerdict, WireFact, WireField, WireShape};
use schemas::{check_wire_demands, qualified_schema_path};

/// Reject codec divergences and unmet directional facts.
/// Codec divergences and unmet demands retain their original diagnostic order.
pub(crate) fn validate_wire_protocol(
    typed: &TypedTrees,
    compatibility_demands: &[WireCompatibilityDemand],
) -> Result<(), Vec<Diagnostic>> {
    // Independent verification of every generated codec FIRST: a codec that
    // provably diverges from the public compact_binary requirement is a
    // compile-time diagnostic, not an admission.
    let verifications: BTreeMap<String, Result<WireCodecVerification, String>> = typed
        .wire_schemas()
        .iter()
        .map(|schema| {
            (
                qualified_schema_path(typed, schema.symbol, schema.name.as_str()),
                checked_interpreter::verify_wire_schema_codec(typed, schema),
            )
        })
        .collect();
    let demands = check_wire_demands(typed, compatibility_demands);
    let refusal_diagnostics = verification_diagnostics(&verifications);
    match (validate_wire_demands(&demands), refusal_diagnostics) {
        (Ok(()), empty) if empty.is_empty() => Ok(()),
        (Ok(()), diagnostics) => Err(diagnostics),
        (Err(mut demand_diagnostics), mut verification_diagnostics) => {
            demand_diagnostics.append(&mut verification_diagnostics);
            Err(demand_diagnostics)
        }
    }
}

/// The consumer-facing refusal of a generated codec whose independent
/// verification PROVED a divergence: refused trust is a compile-time
/// diagnostic, never a report-only downgrade.
fn verification_diagnostics(
    verifications: &BTreeMap<String, Result<WireCodecVerification, String>>,
) -> Vec<Diagnostic> {
    verifications
        .iter()
        .filter_map(|(path, result)| {
            result.as_ref().err().map(|divergence| {
                Diagnostic::error(format!(
                    "generated wire codec `{path}` failed independent verification against the \
                     public compact_binary requirement: {divergence}"
                ))
            })
        })
        .collect()
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
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
