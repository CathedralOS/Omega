//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

mod borrowed_storage_windows;
mod calls;
mod crash_site_truth;
mod dynamic_dispatch;
mod entry_requirement_crash_coverage;
mod float_ranges;
mod operation_crash_contracts;
mod ranked_scc;
mod reborrow_restored_call_use;
mod straight_line;
mod structural_crash_site_truth;
mod structural_scalar_fields;
mod structural_unit;
mod suspension_call_plan;
mod trusted_surface;
