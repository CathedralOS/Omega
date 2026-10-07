//! Rewrite selected token and shorthand uses to ordinary typed calls.
//! Readonly use discovery belongs to validation; checked contract publication
//! belongs to the following stage.

mod call_binding;
mod float_shorthand_calls;

pub(crate) use call_binding::bind_selected_operator_calls;
