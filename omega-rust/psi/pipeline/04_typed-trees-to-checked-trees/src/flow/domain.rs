//! Domain dependencies and fact invalidation. `build_domain_facts` records each
//! domain's dependency paths before flow. `filter_contexts_after_place_mutations`
//! drops each context holding a fact that a written place may overlap, directly or
//! through a domain dependency, and records a `FlowInvalidationFact` for it.
mod dependencies;
mod invalidation;

pub(crate) use dependencies::build_domain_facts;
pub(crate) use dependencies::relative_place_segments_from_expression;
pub(crate) use invalidation::filter_contexts_after_place_mutations;
