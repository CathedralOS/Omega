//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

mod equality_symmetry;
mod indexed_derivation;
mod indexed_levels;
mod indexed_mutual;
mod indexed_nested;
mod indexed_relations;
mod indexed_vector;
mod integer_order_weakening;
mod predicate_conversion;
mod predicate_denotation;
mod theorem_certificate_admission;
mod value_equality_transport;
