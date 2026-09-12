//! Fresh construction and selected value joins share the ordinary result
//! namespace. Case and record identities remain structural; field and dispatch
//! operands share scalar evaluation. Each selected owner transfers to one continuation place,
//! which later statements consume just like a completed call result.

use super::*;

mod emission;
mod record;
pub(crate) mod source_custody;

pub(crate) use emission::emit;
pub(crate) use record::emit_record;
