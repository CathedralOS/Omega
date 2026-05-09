mod aliases;
mod assignments;
mod evaluation;

pub(super) use aliases::{PlaceKey, argument_binding_place_key};
pub(super) use assignments::{apply_static_operations, set_static_value};
pub(super) use evaluation::{resolve_static_value, select_transition};
