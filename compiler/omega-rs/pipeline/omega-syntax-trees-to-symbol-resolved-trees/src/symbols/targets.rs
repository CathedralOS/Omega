mod calls;
mod transitions;

pub(super) use calls::{resolve_call_target_symbol, resolve_static_machine_argument_symbol};
pub(super) use transitions::assign_transition_target_symbols;
