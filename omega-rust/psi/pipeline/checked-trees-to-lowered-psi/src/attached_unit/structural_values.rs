//! Fresh construction and selected value joins share the ordinary result
//! namespace. Case identities remain structural; only dispatch operands use
//! scalar evaluation. Each selected owner transfers to one continuation place,
//! which later statements consume just like a completed call result.

use super::*;

mod emission;
pub(crate) mod source_custody;

pub(crate) use emission::emit;
