mod calls;
mod receivers;
mod symbols;

pub(crate) use receivers::{projected_statement_receiver_place, resolve_projected_receiver_calls};

use crate::context::*;
pub(crate) use calls::{
    call_receiver_parts, receiver_can_dispatch_to_machine, resolve_name_path_member_symbol,
    resolve_state_call_target, statement_call_can_dispatch_to_machine,
    statement_call_receiver_members, statement_call_receiver_path,
};
pub(crate) use symbols::{
    expression_root_symbol, first_valid_name_path_symbol, machine_by_symbol, machine_state_count,
    machine_symbol_from_type_reference_handle,
};
