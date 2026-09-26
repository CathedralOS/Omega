use crate::tests::front_end::checked_program_result;

#[test]
fn named_static_binder_tail_checks_signature_arguments() {
    for argument in ["0", "", "true"] {
        let source = format!(
            "machine invoke<machine Work>() -> u64\n\
             where machine Work(value: u64) -> u64;\n\
             {{ transition {{ _ -> Work({argument}) }} }}",
        );
        let result = checked_program_result(&source);
        if argument == "0" {
            result.expect("the matching binder argument should check");
        } else {
            let diagnostics =
                result.expect_err("a named binder tail must check its signature arguments");
            if argument.is_empty() {
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("expects 1 argument(s), got 0")),
                    "{diagnostics:?}"
                );
            }
        }
    }
}

#[test]
fn named_tail_call_does_not_invent_suspension_acknowledgement() {
    for target in [
        "Root::wait()",
        "(Root::wait())",
        "Root::target(suspend Root::wait())",
    ] {
        let source = r#"
        pub data Root {}
        pub machine Root::wait() -> u64 suspends; { 0 }
        machine Root::target(value: u64) -> u64 { value }
        pub machine Root::entry() -> u64 suspends; {
            transition { _ -> TAIL }
        }
    "#
        .replace("TAIL", target);
        let diagnostics = checked_program_result(&source)
            .expect_err("an unmarked named transfer cannot acknowledge a suspending call");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("suspend")),
            "{target}: {diagnostics:?}"
        );
    }
}
