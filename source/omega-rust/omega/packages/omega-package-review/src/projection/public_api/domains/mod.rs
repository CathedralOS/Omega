mod aliases;
mod facts;
mod projection;

pub(crate) use aliases::{project_domain_alias_expansion, project_domain_establishment_route};
pub(crate) use facts::{
    project_definition_contract_fact, project_domain_predicate_facts,
    require_exact_checked_domain_fact, semantic_fact_matches_definition_fact,
};
pub(crate) use projection::{project_domain_semantic_roles, project_public_domains};
