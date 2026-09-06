mod calls;
mod projected_receivers;
mod receivers;
mod stamping;

pub(super) use calls::resolve_expression_table_call_target_symbol;
pub(super) use projected_receivers::needs_declared_projection;
pub(super) use receivers::{
    resolve_expression_table_member_symbol, resolve_expression_table_receiver_path_symbols,
};
pub(super) use stamping::stamp_receiver_path_symbols_in_table;
