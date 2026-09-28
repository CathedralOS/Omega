//! Public API projection: the declared conformances, constants, data,
//! domains, operators, propositions and traits a package exposes, each
//! captured from its exact checked declaration owner and policed by `policy`.

pub(super) mod conformances;
pub(super) mod constants;
pub(super) mod data;
pub(super) mod domains;
pub(super) mod operators;
pub(crate) mod policy;
pub(super) mod propositions;
pub(super) mod traits;
