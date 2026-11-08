/*
Diagnostic trace switches, each read from the environment once per process.

Checking and effect planning test these switches inside per-operation loops.
On Windows every environment read takes the process environment lock, and an
unconditional read in `captured_reference_result` alone cost about 0.75 CPU
seconds of a one-line `omega --check`. The switches only add `eprintln!`
output and nothing sets them mid-process, so reading each once cannot change
what the compiler produces. Each function answers whether its switch is set at
all; string-valued filters keep their exact matching behind that test.
*/

use std::sync::OnceLock;

fn switch_is_set(cell: &'static OnceLock<bool>, name: &str) -> bool {
    *cell.get_or_init(|| std::env::var_os(name).is_some())
}

pub fn check_sites() -> bool {
    static SWITCH: OnceLock<bool> = OnceLock::new();
    switch_is_set(&SWITCH, "OMEGA_COMPILER_CHECK_SITES")
}

pub fn trace_completion() -> bool {
    static SWITCH: OnceLock<bool> = OnceLock::new();
    switch_is_set(&SWITCH, "OMEGA_COMPILER_TRACE_COMPLETION")
}

pub fn trace_installed_context() -> bool {
    static SWITCH: OnceLock<bool> = OnceLock::new();
    switch_is_set(&SWITCH, "OMEGA_COMPILER_TRACE_INSTALLED_CONTEXT")
}

pub fn struct_trace() -> bool {
    static SWITCH: OnceLock<bool> = OnceLock::new();
    switch_is_set(&SWITCH, "OMEGA_STRUCT_TRACE")
}

pub fn call_write_trace() -> bool {
    static SWITCH: OnceLock<bool> = OnceLock::new();
    switch_is_set(&SWITCH, "OMEGA_CALL_WRITE_TRACE")
}

pub fn reference_slot_trace() -> bool {
    static SWITCH: OnceLock<bool> = OnceLock::new();
    switch_is_set(&SWITCH, "OMEGA_REFERENCE_SLOT_TRACE")
}

pub fn storage_query_trace() -> bool {
    static SWITCH: OnceLock<bool> = OnceLock::new();
    switch_is_set(&SWITCH, "OMEGA_STORAGE_QUERY_TRACE")
}

pub fn state_reference_diagnostics() -> bool {
    static SWITCH: OnceLock<bool> = OnceLock::new();
    switch_is_set(&SWITCH, "OMEGA_STATE_REFERENCE_DIAGNOSTICS")
}
