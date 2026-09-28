//! Fresh construction and selected value joins share the ordinary result
//! namespace. Case and record identities remain structural; field and dispatch
//! operands share scalar evaluation. Each selected owner transfers to one continuation place,
//! which later statements consume just like a completed call result.
//! A whole record operand uses the same result route, with an owned block edge
//! materializing independent unrestricted storage or transferring affine storage.
//! Merely relabeling its source place would alias copies and reuse a declaration.

use super::CheckedTrees;
use crate::expression_preparation::source_custody::structural::plain_record;
mod emission;
mod locals;
pub(crate) mod record;
mod structural_case;

pub(crate) use emission::emit;
pub(super) use locals::bind_local;
