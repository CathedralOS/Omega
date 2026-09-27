//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

mod arithmetic_import;
mod artifact;
mod bounded_certificate;
mod canonical;
mod debug_map;
mod disjunction_elimination;
mod float_ranges;
mod integer_order_discreteness;
mod integer_order_substitution;
mod integer_step_orders;
mod integer_strict_order_transitivity;
mod integer_subtract_order;
mod ledger_spike;
mod mathematical_certificate;
mod publication;
mod quotient_correspondence;
mod reborrow_restored_call_use;
mod theorem_certificate;
mod value_equality_transport;
