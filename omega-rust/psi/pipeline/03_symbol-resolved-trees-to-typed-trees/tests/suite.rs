//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

// The front-end pipeline these tests run, shared with the crate's unit tests
// through `src/lib.rs`; see its module documentation.
#[path = "support/front_end.rs"]
mod front_end;

mod indexed_member_identity;
mod service_reach_source_spans;
mod transparent_refinement_reaches;
