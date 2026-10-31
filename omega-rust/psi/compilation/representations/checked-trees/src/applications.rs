//! Read the checked invocation graph without introducing executable roots.
//! Provider edges are borrowed from the consumer's exact selection or conservative
//! candidate domain; metadata references never donate another machine body.

use crate::CheckedTrees;
use symbols::SymbolHandle;

/// Compiler-supplied invocation roots for one checked Build target application.
#[derive(Clone)]
pub struct TargetApplicationRoots {
    pub target: String,
    pub machines: Vec<SymbolHandle>,
}

/// Read actual invocation demand through the same source-independent graph
/// used by Source preparation and Terminal proof scope. Target selection already
/// supplied these exact roots and provider edges; it never selects source bodies.
pub fn application_machines(
    checked: &CheckedTrees,
    roots: &[SymbolHandle],
    provider_edges: &[(SymbolHandle, SymbolHandle)],
) -> Vec<SymbolHandle> {
    crate::ProofApplications::new(
        &checked.facts.proof_applications,
        &checked.facts.operational,
    )
    .reachability(provider_edges, Some(roots))
    .machines
}
