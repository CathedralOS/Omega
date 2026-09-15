//! Fixtures shared by the arithmetic and data canaries.

#[path = "arithmetic_and_data/arithmetic_domains.rs"]
mod arithmetic_domains;
#[path = "arithmetic_and_data/casts_and_narrowing.rs"]
mod casts_and_narrowing;
#[path = "arithmetic_and_data/enum_and_comparison_canaries.rs"]
mod enum_and_comparison_canaries;
#[path = "../fixture_rosters/arithmetic_and_data.rs"]
pub(super) mod fixture_roster;
