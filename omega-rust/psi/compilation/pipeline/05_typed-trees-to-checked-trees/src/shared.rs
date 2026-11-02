//! Helpers every checking step reads: `collections` (ordered first-occurrence
//! rosters), `call_formals` (aligning a call's operands with its formals),
//! `operators` (checked operator selection) and `authored_selections`
//! (authored declaration custody retained through specialization).

pub(crate) mod authored_selections;
pub(crate) mod call_formals;
pub(crate) mod collections;
pub(crate) mod operators;
