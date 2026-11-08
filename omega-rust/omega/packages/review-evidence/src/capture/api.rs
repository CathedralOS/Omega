// Public API projection for declared conformances, constants, data types,
// domains, operators, propositions, and traits, each captured from its checked
// declaration owner and assembled by `policy`.

pub(super) mod conformances;
pub(super) mod constants;
pub(super) mod data;
pub(super) mod domains;
pub(super) mod operators;
pub(crate) mod policy;
pub(super) mod propositions;
mod slot_metadata;
pub(super) mod traits;
