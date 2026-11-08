//! Full public declaration vocabulary under the enclosing policy reader.

pub(in crate::encoding::recovery) mod data;
pub(in crate::encoding::recovery) mod domains;
pub(in crate::encoding::recovery) mod traits;
pub(in crate::encoding::recovery) mod values;

pub(in crate::encoding::recovery) use data::data_member;
pub(in crate::encoding::recovery) use domains::{alias_atom, establishment_route};
pub(in crate::encoding::recovery) use traits::trait_parent;
pub(in crate::encoding::recovery) use values::{const_shape, operator_spelling, proposition_shape};
