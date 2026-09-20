//! Exact source-constant evaluation used by declaration retention and synthesis.

mod anonymous;
mod arguments;
mod domains;
mod fact_values;
mod facts;
mod remainder;
mod templates;
mod values;

pub(super) use arguments::*;
pub(super) use domains::*;
pub(crate) use fact_values::*;
pub(crate) use facts::*;
use remainder::validate_anonymous_remainder;
pub(in crate::preparation) use templates::collect_type_positions;
pub(super) use templates::*;
pub(super) use values::*;
