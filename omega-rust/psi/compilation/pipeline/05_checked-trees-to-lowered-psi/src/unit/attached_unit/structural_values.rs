//! Fresh construction and selected value joins share the ordinary result
//! namespace. Case and record identities remain structural; field and dispatch
//! operands share scalar evaluation. Each selected owner transfers to one continuation place,
//! which later statements consume just like a completed call result.
//! A whole record operand uses the same result route, with an owned block edge
//! materializing independent unrestricted storage or transferring affine storage.
//! Merely relabeling its source place would alias copies and reuse a declaration.
//! Each nested constructor publishes its own claim frontier before its parent
//! consumes it. The enclosing frontier is projected through the exact child
//! path; attaching claims only at the outer result loses the child's transfer.
//! `completed_aggregate` publishes record, case and array literals through the
//! same operation: every structural member transports its memberships under
//! its declared path, and one establishing operation commits the result.

use super::CheckedTrees;
mod completed_aggregate;
mod emission;
pub(crate) mod field_carrier;
pub(crate) mod fixed_array;
mod locals;
pub(super) mod member_memberships;
mod operand_claims;
pub(crate) mod record;
pub(crate) mod structural_case;

pub(crate) use emission::{ValueDestination, ValueProduction, emit};
pub(super) use locals::local_binding;

/// An evaluation may leave the caller suspended in a checked invocation.
/// That outcome carries no value and grants no normal-completion authority.
pub(crate) enum ValueOutcome<T> {
    Normal(T),
    NoNormal,
}
