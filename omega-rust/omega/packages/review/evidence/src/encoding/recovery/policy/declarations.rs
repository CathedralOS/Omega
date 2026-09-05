//! Full public declaration vocabulary under the enclosing policy reader.

mod data;
mod domains;
mod traits;
mod values;

#[cfg(test)]
mod tests;

pub(super) use data::data_member;
pub(super) use domains::alias_atom;
#[cfg(test)]
pub(super) use traits::conformance_shape;
pub(super) use traits::trait_parent;
pub(super) use values::{const_shape, operator_spelling, proposition_shape};

#[cfg(test)]
pub(super) use data::data_shape;
#[cfg(test)]
pub(super) use domains::domain_shape;
#[cfg(test)]
pub(super) use traits::trait_shape;
#[cfg(test)]
pub(super) use values::operator_shape;
