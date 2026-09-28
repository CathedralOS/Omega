//! Optimizer module role: accounting leaf. The node custody a plan declares.
//!
//! A published candidate names the exact source custody each folded node
//! retains. The rule derives it from the input function here; the
//! independent validator recomputes the same rows from the candidate's
//! memberships and rejects a declaration that disagrees.

use super::{
    CaseMembershipSpecializationRewrite, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationFunction, PsiRealizationSite,
};

/// The exact node custody a plan's folded observations carry: each folded
/// site retains the membership's own provenance and fuel settlement, realized
/// at the same node. `None` when a row's site does not name a node of
/// `input_function`.
pub(crate) fn provenance_rows(
    input_function: &PsiOptimizationFunction,
    plan: &CaseMembershipSpecializationRewrite,
) -> Option<Vec<ProvenanceRewrite>> {
    let mut rows = Vec::new();
    for row in &plan.memberships {
        let index = usize::try_from(row.site.node).ok()?;
        let node = input_function
            .blocks
            .iter()
            .find(|block| block.id == row.site.block)
            .and_then(|block| block.nodes.get(index))?;
        let site = PsiRealizationSite::Node(row.site);
        rows.push(ProvenanceRewrite {
            input: site,
            disposition: ProvenanceDisposition::RealizedAt(site),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
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
