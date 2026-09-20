//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

mod authored_declaration_selection_ledger;
mod indexed_member_identity;
mod invocation_source_spans;
mod ordinary_member_identity;
mod quotient_theorem_selection;
mod service_reach_source_spans;
mod trait_machine_identity;
mod transparent_refinement_reaches;
