//! Statement grammar: `parse_statement` dispatches one statement to the
//! ordinary and atomic `let` forms, destructuring and proof-output bindings,
//! discards, local data and inline assembly; `statement_tables` copies the
//! parsed handles into the statement tables.

pub(crate) mod atomic_lets;
pub(crate) mod destructure_and_proof_output;
pub(crate) mod discard_and_local_data;
pub(crate) mod inline_assembly;
pub(crate) mod parse_statement;
pub(crate) mod statement_tables;
