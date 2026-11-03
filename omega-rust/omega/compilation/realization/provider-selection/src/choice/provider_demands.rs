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
        || choices
            .iter()
            .any(|choice| choice.subject.selects_plan(schema, plan))
}
