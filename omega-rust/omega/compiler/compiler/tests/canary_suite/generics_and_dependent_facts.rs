//! Fixtures shared by the generic and dependent fact canaries.

#[path = "../fixture_rosters/generics_and_dependent_facts.rs"]
pub(super) mod fixture_roster;
#[path = "generics_and_dependent_facts/generic_specializations.rs"]
mod generic_specializations;
#[path = "generics_and_dependent_facts/local_instantiations_and_requires.rs"]
mod local_instantiations_and_requires;
#[path = "generics_and_dependent_facts/range_inference_and_measures.rs"]
mod range_inference_and_measures;
