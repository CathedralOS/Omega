//! Validating proof contracts: requires-to-ensures entailment, proof facts
//! and embeddings, the proof-only fence, properties, rankings, quotients,
//! float projections and the arithmetic, default and weakened domains.

pub(crate) mod arithmetic_domains;
pub(crate) mod bound_expression_meaning;
pub(crate) mod contract_entailment;
pub(crate) mod contract_results;
pub(crate) mod default_domains;
pub(crate) mod domain_weakening;
pub(crate) mod domains;
pub(crate) mod float_projection_bindings;
pub(crate) mod float_projection_invocations;
pub(crate) mod immutable_integer_bounds;
pub(crate) mod parameter_expression_meaning;
pub(crate) mod proof_embeddings;
pub(crate) mod proof_facts;
pub(crate) mod proof_only_faces;
pub(crate) mod properties;
pub(crate) mod proposition_entailment;
pub(crate) mod qualification_evidence;
pub(crate) mod quotients;
pub(crate) mod relevance;
pub(crate) mod slice_ranking;
