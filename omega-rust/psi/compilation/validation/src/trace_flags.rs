/*
Diagnostic trace switches, each read from the environment once per process.

Checking and effect planning test these switches inside per-operation loops.
On Windows every environment read takes the process environment lock, and an
unconditional read in `captured_reference_result` alone cost about 0.75 CPU
seconds of a one-line `omega --check`. The switches only add `eprintln!`
output and nothing sets them mid-process, so reading each once cannot change
what the compiler produces. Most switches test presence. The structural
switch also supports a selected contract; all readers use the same captured
value so unrelated traces stay disabled for that mode.
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

fn struct_trace_value() -> Option<&'static std::ffi::OsStr> {
    static VALUE: OnceLock<Option<std::ffi::OsString>> = OnceLock::new();
    VALUE
        .get_or_init(|| std::env::var_os("OMEGA_STRUCT_TRACE"))
        .as_deref()
}

pub fn struct_trace() -> bool {
    let value = struct_trace_value();
    full_struct_trace(value.and_then(std::ffi::OsStr::to_str), value.is_some())
}

fn full_struct_trace(value: Option<&str>, present: bool) -> bool {
    present && !value.is_some_and(|value| value.starts_with("contract="))
}

/// Limit existing contract diagnostics to one machine's declaration name.
/// Other values keep the ordinary presence-only structural trace behavior.
pub fn struct_contract_selection() -> Option<&'static str> {
    selected_contract(struct_trace_value()?.to_str()?)
}

fn selected_contract(value: &str) -> Option<&str> {
    value
        .strip_prefix("contract=")
        .filter(|name| !name.is_empty())
}

/// Select one whole-call consumer without changing the existing presence switch.
/// The optional value is `caller->callee@root_index:root_generation`.
/// Names identify declaration machines; root numbers identify retained storage.
pub fn struct_call_consumer_selection() -> Option<(&'static str, &'static str, u32, u32)> {
    static SELECTION: OnceLock<Option<(&'static str, &'static str, u32, u32)>> = OnceLock::new();
    *SELECTION.get_or_init(|| parse_struct_call_consumer_selection(struct_trace_value()?.to_str()?))
}

fn parse_struct_call_consumer_selection(value: &str) -> Option<(&str, &str, u32, u32)> {
    let (caller, selected) = value.split_once("->")?;
    let (callee, root) = selected.split_once('@')?;
    let (index, generation) = root.split_once(':')?;
    if caller.is_empty() || callee.is_empty() || caller.contains('@') || callee.contains("->") {
        return None;
    }
    let index = index.parse::<u32>().ok()?;
    let generation = generation.parse::<u32>().ok()?;
    (generation != 0).then_some((caller, callee, index, generation))
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

#[cfg(test)]
mod struct_call_consumer_tests {
    use super::parse_struct_call_consumer_selection;

    #[test]
    fn selects_exact_declaration_paths_and_root_generation() {
        assert_eq!(
            parse_struct_call_consumer_selection("outer::read->outer::consume@17:1"),
            Some(("outer::read", "outer::consume", 17, 1)),
        );
        assert_eq!(
            parse_struct_call_consumer_selection("read->consume@0:2"),
            Some(("read", "consume", 0, 2)),
        );
    }

    #[test]
    fn ordinary_trace_values_do_not_select_a_call() {
        for value in ["", "1", "true", "outer::read"] {
            assert_eq!(parse_struct_call_consumer_selection(value), None);
        }
    }

    #[test]
    fn refuses_incomplete_or_ambiguous_selectors() {
        for value in [
            "->consume@1:1",
            "read->@1:1",
            "read->consume",
            "read->consume@1",
            "read->consume@1:0",
            "read->consume@4294967296:1",
            "read->consume@1:4294967296",
            "read->consume@1:1:1",
            "read->other->consume@1:1",
            "read@other->consume@1:1",
        ] {
            assert_eq!(parse_struct_call_consumer_selection(value), None, "{value}");
        }
    }
}

#[cfg(test)]
mod contract_trace_tests {
    use super::{full_struct_trace, selected_contract};

    #[test]
    fn contract_selection_does_not_enable_the_global_trace() {
        assert_eq!(
            selected_contract("contract=cauchy_at_reflexive"),
            Some("cauchy_at_reflexive")
        );
        assert!(!full_struct_trace(
            Some("contract=cauchy_at_reflexive"),
            true
        ));
        assert_eq!(selected_contract("contract="), None);
        assert!(!full_struct_trace(Some("contract="), true));
    }

    #[test]
    fn existing_presence_values_keep_their_meaning() {
        assert!(!full_struct_trace(None, false));
        assert!(full_struct_trace(None, true));
        for value in ["", "1", "true", "outer::read->outer::consume@17:1"] {
            assert!(full_struct_trace(Some(value), true));
            assert_eq!(selected_contract(value), None);
        }
    }
}
