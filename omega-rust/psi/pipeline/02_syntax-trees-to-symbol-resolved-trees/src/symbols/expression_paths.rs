//! Symbol lookup for receiver paths, member accesses and call targets inside
//! an expression table.
//!
//! There is no entry of its own; callers run these lookups in their own
//! order. `expressions::references` uses all of them: `assign_call_symbol`
//! resolves the receiver path with
//! `resolve_expression_table_receiver_path_symbols`, writes the result onto
//! the receiver with `stamp_receiver_path_symbols_in_table`, then selects the
//! call target with `resolve_expression_table_call_target_symbol`;
//! `assign_member_symbol` uses `resolve_expression_table_member_symbol`.
//! `contracts` also calls `resolve_expression_table_call_target_symbol`.
//!
//! Children: `receivers` resolves receiver paths and members, `calls` selects
//! a call target, `stamping` writes resolved symbols back into the table, and
//! `projected_receivers` selects call targets for receivers reached through an
//! index or a case payload (`needs_declared_projection` detects those).
//! `scope` also reads `projected_receivers` directly.

mod calls;
pub(in crate::symbols) mod projected_receivers;
mod receivers;
mod stamping;

pub(super) use calls::resolve_expression_table_call_target_symbol;
pub(super) use projected_receivers::needs_declared_projection;
pub(super) use receivers::{
    resolve_expression_table_member_symbol, resolve_expression_table_receiver_path_symbols,
};
pub(super) use stamping::stamp_receiver_path_symbols_in_table;
