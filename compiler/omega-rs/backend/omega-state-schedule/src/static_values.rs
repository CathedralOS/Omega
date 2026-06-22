mod aliases;
mod assignments;
mod evaluation;

use omega_core::symbols::SymbolHandle;
use std::sync::Arc;

pub(super) use aliases::{PlaceKey, argument_binding_place_key};
pub(super) use assignments::{apply_static_operations, set_static_value};
pub(super) use evaluation::{resolve_static_value, select_transition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum StaticValue {
    Boolean(bool),
    Integer(i64),
    String(Arc<str>),
    Symbol(SymbolHandle),
}
