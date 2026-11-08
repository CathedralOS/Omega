// Authored source custody, compiler-private row pairings (`ProjectedReviewRow`),
// family interfaces, invocations, source locations, service reach, and suspension.

pub(super) mod family_interfaces;
pub(super) mod invocations;
pub(super) mod locations;
mod rows;
pub(super) mod service_reach;
pub(super) mod suspension;

pub(crate) use rows::ProjectedReviewRow;
