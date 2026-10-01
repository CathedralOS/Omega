//! A terminal machine call completes its caller only with a result of the
//! caller's own declared type: carrier, arithmetic policy and Unit-ness.

/// Whether checking `source` reports a terminal-call result mismatch.
fn rejects_terminal_result(source: &str) -> bool {
    let typed = crate::front_end::typed_program(source);
    match crate::validate_program(&typed) {
        Ok(_) => false,
        Err(diagnostics) => diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("terminal call to `callee`")
                && diagnostic
                    .message
                    .contains("a terminal machine call's normal result is its caller's result")
        }),
    }
}

/// A caller that tail-calls `callee` on a guarded arm beside a value arm, or
/// on its one unconditional arm when the caller returns Unit.
fn program(caller_result: &str, callee_result: &str) -> String {
    let callee_body = if callee_result.is_empty() {
        "{}".to_owned()
    } else {
        "{ transition { _ -> (7) } }".to_owned()
    };
    let caller_body = if caller_result.is_empty() {
        "{ transition { _ -> callee(value) } }".to_owned()
    } else {
        "{ transition value > 0 { true -> callee(value) false -> (0) } }".to_owned()
    };
    let arrow = |result: &str| {
        if result.is_empty() {
            String::new()
        } else {
            format!(" -> {result}")
        }
    };
    format!(
        "machine callee(value: i32){} {callee_body}\n\
         machine caller(value: i32){} {caller_body}",
        arrow(callee_result),
        arrow(caller_result),
    )
}

#[test]
fn terminal_call_result_must_be_the_caller_result() {
    for (caller, callee, rejected) in [
        ("i32", "i32", false),
        ("u64", "u64", false),
        ("", "", false),
        ("i32", "u64", true),
        ("u32 in Wrapping", "u32", true),
        ("i32", "", true),
        ("", "u64", true),
    ] {
        let source = program(caller, callee);
        assert_eq!(
            rejects_terminal_result(&source),
            rejected,
            "caller `{caller}`, callee `{callee}`:\n{source}"
        );
    }
}

#[test]
fn terminal_call_result_compares_data_types_by_declaration() {
    let source = |callee: &str| {
        format!(
            "data Left {{ value: i32; }}\n\
             data Right {{ value: i32; }}\n\
             machine callee(value: i32) -> {callee} {{ {callee} {{ value: value }} }}\n\
             machine caller(value: i32) -> Left {{ transition {{ _ -> callee(value) }} }}"
        )
    };
    assert!(!rejects_terminal_result(&source("Left")));
    assert!(rejects_terminal_result(&source("Right")));
}
