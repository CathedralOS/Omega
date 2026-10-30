//! Retain candidate order while judging prepared demand and authored choices.

use crate::ProviderDeclaration;
use effects::provider_plan::ProviderPlan;
use std::collections::HashSet;
use std::hash::Hash;

pub fn is_candidate_demanded<Declaration: Eq + Hash, Origin>(
    plan: &ProviderPlan,
    schema: &Result<Declaration, String>,
    requirements: &[Declaration],
    uses: &HashSet<Declaration>,
    choices: &[ProviderDeclaration<Declaration, Origin>],
) -> bool {
    requirements
        .iter()
        .any(|requirement| uses.contains(requirement))
        || choices.iter().any(|choice| {
            choice
                .target
                .is_none_or(|target| target.canonical_name() == plan.target)
                && choice.subject.selects_plan(schema, plan)
        })
}
