//! Fixtures shared by the proof and float suite canaries.

#[path = "../fixture_rosters/proof_and_float_suites.rs"]
pub(super) mod fixture_roster;
#[path = "proof_and_float_suites/float_semantic_twins.rs"]
mod float_semantic_twins;
#[path = "proof_and_float_suites/proof_and_domain_canaries.rs"]
mod proof_and_domain_canaries;
#[path = "proof_and_float_suites/runtime_float_and_index_canaries.rs"]
mod runtime_float_and_index_canaries;
