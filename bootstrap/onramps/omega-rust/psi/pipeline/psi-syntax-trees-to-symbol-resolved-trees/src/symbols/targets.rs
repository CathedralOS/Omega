mod calls;
mod transitions;

pub(super) use calls::{
    assign_provider_selection_argument_symbol, assign_static_argument_symbols,
    resolve_call_target_symbol, resolve_free_machine_entry_state_symbol,
    resolve_proposition_binder_argument_symbol,
};
pub(super) use transitions::assign_transition_target_symbols;
