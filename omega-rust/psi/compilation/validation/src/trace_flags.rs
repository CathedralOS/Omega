//! Diagnostic trace switches, each read from the environment once per process.
//!
//! Effect planning and checking test these switches inside per-operation loops.
//! On Windows every environment read takes the process environment lock, and an
//! unconditional read in `captured_reference_result` alone cost about 0.75 CPU
//! seconds of a one-line `omega --check`. The switches only add `eprintln!`
//! output, so reading each once cannot change what the compiler produces.

use std::sync::OnceLock;

fn flag(cell: &'static OnceLock<bool>, name: &str) -> bool {
    *cell.get_or_init(|| std::env::var_os(name).is_some())
}

/// `OMEGA_COMPILER_CHECK_SITES`: report each refusal site as it is reached.
pub fn check_sites() -> bool {
    static CELL: OnceLock<bool> = OnceLock::new();
    flag(&CELL, "OMEGA_COMPILER_CHECK_SITES")
}

/// `OMEGA_COMPILER_TRACE_COMPLETION`: report missing completion rows.
pub fn trace_completion() -> bool {
    static CELL: OnceLock<bool> = OnceLock::new();
    flag(&CELL, "OMEGA_COMPILER_TRACE_COMPLETION")
}

/// `OMEGA_STRUCT_TRACE`: report structural carrier decisions.
pub fn struct_trace() -> bool {
    static CELL: OnceLock<bool> = OnceLock::new();
    flag(&CELL, "OMEGA_STRUCT_TRACE")
}

/// `OMEGA_CALL_WRITE_TRACE`: report call write summaries.
pub fn call_write_trace() -> bool {
    static CELL: OnceLock<bool> = OnceLock::new();
    flag(&CELL, "OMEGA_CALL_WRITE_TRACE")
}

/// `OMEGA_REFERENCE_SLOT_TRACE`: report reference slot decisions.
pub fn reference_slot_trace() -> bool {
    static CELL: OnceLock<bool> = OnceLock::new();
    flag(&CELL, "OMEGA_REFERENCE_SLOT_TRACE")
}

/// `OMEGA_STORAGE_QUERY_TRACE`: report completed-storage queries.
pub fn storage_query_trace() -> bool {
    static CELL: OnceLock<bool> = OnceLock::new();
    flag(&CELL, "OMEGA_STORAGE_QUERY_TRACE")
}

/// `OMEGA_STATE_REFERENCE_DIAGNOSTICS`: report state reference decisions.
pub fn state_reference_diagnostics() -> bool {
    static CELL: OnceLock<bool> = OnceLock::new();
    flag(&CELL, "OMEGA_STATE_REFERENCE_DIAGNOSTICS")
}

/// Whether `OMEGA_PHASE_PROFILE` is set at all, which some refusal sites
/// also treat as a request for their trace lines.
pub fn phase_profile_requested() -> bool {
    static CELL: OnceLock<bool> = OnceLock::new();
    flag(&CELL, "OMEGA_PHASE_PROFILE")
}
