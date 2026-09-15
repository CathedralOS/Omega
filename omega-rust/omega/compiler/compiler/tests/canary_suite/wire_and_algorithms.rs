//! Fixtures shared by the wire codec and algorithm canaries.

#[path = "wire_and_algorithms/arithmetic_casts_and_floats.rs"]
mod arithmetic_casts_and_floats;
#[path = "wire_and_algorithms/classic_algorithms.rs"]
mod classic_algorithms;
#[path = "../fixture_rosters/wire_and_algorithms.rs"]
pub(super) mod fixture_roster;
#[path = "wire_and_algorithms/indexed_structures_and_guards.rs"]
mod indexed_structures_and_guards;
#[path = "wire_and_algorithms/wire_codecs_and_views.rs"]
mod wire_codecs_and_views;
