//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

mod case_where_generic_instance;
mod constant_carriers;
mod data_where_identity;
mod module_namespaces;
mod trait_machine_identity;
