//! Exact source-constant evaluation used by declaration retention and synthesis.

use super::*;

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
pub(super) use fact_values::*;
pub(super) use facts::*;
use remainder::validate_anonymous_remainder;
pub(super) use templates::*;
pub(super) use values::*;
