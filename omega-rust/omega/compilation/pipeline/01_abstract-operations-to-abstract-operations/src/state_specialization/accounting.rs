//! Optimizer module role: accounting leaf. The edge custody a plan declares.
//!
//! A published candidate names the exact source custody each fused edge
//! carries and the surviving dispatch occurrence of each resolved arm. The
//! rule derives both from the input function here; the independent validator
//! recomputes the same rows from the candidate's edges and rejects a
//! declaration that disagrees.

use super::{
    OptimizationEdge, ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationFunction,
    PsiRealizationSite, StateArgumentSpecializationRewrite,
};
use semantic_vocabulary::EdgeId;
use std::collections::BTreeSet;

/// The accepted custody ledger for one specialization plan: each fused edge
/// records the incoming edge's retained occurrence and the resolved arm
/// edge's fan-out onto the fused edge, and each resolved arm edge records its
/// surviving dispatch occurrence once — two incoming edges may legitimately
/// resolve to the same dispatch arm, so that occurrence is a per-edge row,
/// not a per-specialization row. `None` when a row names an edge absent from
/// `input_function`.
pub(crate) fn provenance_rows(
    input_function: &PsiOptimizationFunction,
    plan: &StateArgumentSpecializationRewrite,
) -> Option<Vec<ProvenanceRewrite>> {
    let machine = plan.machine;
    let mut rows = Vec::new();
    let mut resolved_edges = BTreeSet::new();
    for row in &plan.edges {
        let incoming = find_edge(input_function, row.incoming_edge)?;
        let resolved = find_edge(input_function, row.taken_edge)?;
        let incoming_site = PsiRealizationSite::Edge {
            machine,
            edge: row.incoming_edge,
        };
        let resolved_site = PsiRealizationSite::Edge {
            machine,
            edge: row.taken_edge,
        };
        rows.push(ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(incoming_site),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        });
        rows.push(ProvenanceRewrite {
            input: resolved_site,
            disposition: ProvenanceDisposition::RealizedAt(incoming_site),
            sources: resolved.provenance.clone(),
            fuel: resolved.fuel.clone(),
        });
        resolved_edges.insert(row.taken_edge);
    }
    for taken_edge in resolved_edges {
        let resolved = find_edge(input_function, taken_edge)?;
        let resolved_site = PsiRealizationSite::Edge {
            machine,
            edge: taken_edge,
        };
        rows.push(ProvenanceRewrite {
            input: resolved_site,
            disposition: ProvenanceDisposition::RealizedAt(resolved_site),
            sources: resolved.provenance.clone(),
            fuel: resolved.fuel.clone(),
        });
    }
    rows.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some(rows)
}

fn find_edge(function: &PsiOptimizationFunction, edge: EdgeId) -> Option<&OptimizationEdge> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .find(|candidate| candidate.psi_edge == edge)
}
