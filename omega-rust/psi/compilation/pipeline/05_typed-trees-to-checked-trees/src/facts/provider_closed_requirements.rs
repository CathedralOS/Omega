//! Publish admitted provider applications after source checking completes.
//!
//! Typed association formation is shared with immutable body-origin readers;
//! it supplies no checked authority until the pipeline reaches this stage.

use checked_trees::{CheckedProviderClosedRequirement, CheckedProviderClosedRequirements};

pub(crate) fn record_provider_closed_requirements(
    program: &dyn validation::CheckingInput,
) -> CheckedProviderClosedRequirements {
    let mut recorded = CheckedProviderClosedRequirements::default();
    for association in validation::provider_application_associations(program).iter() {
        let row = &association.identity;
        recorded.insert(
            CheckedProviderClosedRequirement {
                provider: row.provider,
                selecting_declaration: row.selecting_declaration,
                template: row.template,
                closed_requirement: row.closed_requirement,
                receiving_application: row.receiving_application,
                receiving_regions: arena::HandleSpan::empty(),
            },
            association.receiving_regions.iter().copied(),
        );
    }
    recorded
}
