mod calls;
mod static_namespaces;
mod transitions;

pub(super) use calls::{
    SOURCE_CALL_KINDS, assign_call_machine_argument_symbols, assign_static_argument_symbols,
    proof_static_call_target, resolve_call_target_symbol, resolve_free_call_target_symbol,
    resolve_free_machine_entry_state_symbol,
};
pub(super) use transitions::assign_transition_target_symbols;

pub(crate) use static_namespaces::normalize_static_module_calls;
