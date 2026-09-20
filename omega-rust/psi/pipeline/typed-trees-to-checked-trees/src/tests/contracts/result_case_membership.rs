use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;
use crate::tests::parse_typed_trees_with_core_service;

const DATA: &str = "data Message { case Empty; case Data(value: u8); }";
const PUB_DATA: &str = "pub data Message { case Empty; case Data(value: u8); }";

fn check(machine: &str, accepted: bool) {
    let source = format!("{DATA}\n{machine}");
    match lower_typed_trees(parse_typed_trees(&source)) {
        Ok(_) => assert!(accepted, "unproved result membership accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
                "{diagnostics:#?}"
            );
        }
    }
}

/// `Service<R>` fixtures lower against the toolchain carrier with the same
/// fused-service erasure authorizations `settle_checked_providers` binds in
/// real builds; without them the carrier field stays unshaped and the
/// machine's unit plan fails closed.
fn lower_service_source(
    source: &str,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let mut typed = parse_typed_trees_with_core_service(source);
    crate::tests::bind_fixture_fused_service_erasures(&mut typed);
    lower_typed_trees(typed)
}

#[test]
fn result_case_membership_observes_the_returned_tag_not_payload_equality() {
    for (condition, accepted) in [
        ("result in Message::Data", true),
        ("result in Message::Empty", false),
        ("!(result in Message::Empty)", true),
        ("!(result in Message::Data)", false),
        (
            "(result in Message::Data) && !(result in Message::Empty)",
            true,
        ),
        (
            "(result in Message::Empty) || (result in Message::Data)",
            true,
        ),
        (
            "(result in Message::Empty) && (result in Message::Data)",
            false,
        ),
    ] {
        check(
            &format!(
                "machine make(value: u8) -> Message ensures {condition}; {{ let before: u8 = value; Message::Data {{ value: before }} }}"
            ),
            accepted,
        );
    }
    check(
        "machine empty() -> Message ensures result in Message::Empty; { Message::Empty }",
        true,
    );
}

#[test]
fn result_case_membership_consumes_live_parameter_and_projection_predicates() {
    check(
        "machine forward(value: Message) -> Message requires value in Message::Data; ensures result in Message::Data; { value }",
        true,
    );
    check(
        "data Wrapper { message: Message; } machine forward(value: Wrapper) -> Message requires value.message in Message::Data; ensures result in Message::Data; { value.message }",
        true,
    );
    check(
        "machine forward(value: Message) -> Message requires value in Message::Empty; ensures result in Message::Data; { value }",
        false,
    );
    check(
        "machine forward(value: Message) -> Message ensures !(result in Message::Data); { value }",
        false,
    );
    check(
        "machine forward(mut value: Message) -> Message requires value in Message::Data; ensures result in Message::Data; { value = Message::Empty; value }",
        false,
    );
}

#[test]
fn result_case_membership_respects_shadowing_and_each_normal_exit() {
    check(
        "machine make(result: Message) -> Message requires result in Message::Empty; ensures result in Message::Data; { Message::Data { value: 1 } }",
        false,
    );
    check(
        "machine make(flag: bool) -> Message ensures result in Message::Data; { transition flag { true -> (Message::Data { value: 1 }) false -> (Message::Data { value: 2 }) } }",
        true,
    );
    check(
        "machine make(flag: bool) -> Message ensures result in Message::Data; { transition flag { true -> (Message::Data { value: 1 }) false -> (Message::Empty) } }",
        false,
    );
}

#[test]
fn result_case_membership_composes_with_live_scalar_predicates() {
    check(
        "machine make(mut value: u8) -> Message requires value > 0; ensures (result in Message::Data) && value > 0; { value = 0; Message::Data { value: value } }",
        false,
    );
    for (condition, accepted) in [
        ("(result in Message::Data) && value > 0", true),
        ("value > 0 && (result in Message::Data)", true),
        ("(result in Message::Data) && value == 0", false),
        ("(result in Message::Data) && !(value > 0)", false),
    ] {
        check(
            &format!(
                "machine make(value: u8) -> Message requires value > 0; ensures {condition}; {{ Message::Data {{ value: value }} }}"
            ),
            accepted,
        );
    }
    for (condition, accepted) in [
        ("(result in Message::Data) && value > 0", true),
        ("value > 0 && (result in Message::Data)", true),
        ("(result in Message::Data) && value == 0", false),
        ("(result in Message::Empty) || value > 0", true),
        ("(result in Message::Empty) || value == 0", false),
    ] {
        check(
            &format!(
                "machine make(mut value: u8) -> Message ensures {condition}; {{ value = 1; Message::Data {{ value: value }} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn result_case_membership_rejects_a_foreign_nominal_classifier() {
    let source = format!(
        "{DATA} data Other {{ case Empty; case Data(value: u8); }} machine make() -> Message ensures result in Other::Data; {{ Message::Data {{ value: 1 }} }}"
    );
    assert!(lower_typed_trees(parse_typed_trees(&source)).is_err());
}

#[test]
fn boundary_call_results_carry_their_declaring_case_owner() {
    // A boundary requirement call resolves to the trait's declared signature
    // symbol, not a machine-body state. The declared result still supplies the
    // exact case owner, so transition patterns and `in` membership observe it.
    let source = format!(
        "{PUB_DATA}
        pub boundary trait Input {{ machine read(buffer: &mut [u8]) -> Message reaches Input; }}
        data Probe {{ input: Service<Input>; buffer: [u8; 4]; }}
        machine Probe::run(&mut self) reaches Input {{
            transition self.input.read(&mut self.buffer) {{
                Message::Empty -> done()
                Message::Data {{ value }} -> got(value)
                _ -> done()
            }}
            state done(&mut self) {{ }}
            state got(&mut self, value: u8) {{ }}
        }}
        machine Probe::peek(&mut self) -> bool reaches Input {{
            self.input.read(&mut self.buffer) in Message::Empty
        }}"
    );
    lower_service_source(&source)
        .expect("boundary call results keep their exact declaring data type");
}

#[test]
fn boundary_call_results_still_reject_a_foreign_case_owner() {
    let source = format!(
        "{PUB_DATA}
        data Other {{ case Empty; }}
        pub boundary trait Input {{ machine read(buffer: &mut [u8]) -> Message reaches Input; }}
        data Probe {{ input: Service<Input>; buffer: [u8; 4]; }}
        machine Probe::peek(&mut self) -> bool reaches Input {{
            self.input.read(&mut self.buffer) in Other::Empty
        }}"
    );
    let diagnostics = lower_service_source(&source)
        .expect_err("a foreign case owner is not evidence about the call result");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("case membership must test a value of the exact declaring data type")),
        "{diagnostics:#?}"
    );
}

#[test]
fn result_field_membership_observes_nested_and_distinct_fields() {
    for (condition, accepted) in [
        ("result.inner.message in Message::Data", true),
        ("result.inner.message in Message::Empty", false),
        ("!(result.inner.message in Message::Empty)", true),
    ] {
        check(&format!(
            "data Inner {{ message: Message; }} data Wrapper {{ inner: Inner; }}
            machine make(value: u8) -> Wrapper ensures {condition};
            {{ let before: u8 = value; Wrapper {{ inner: Inner {{ message: Message::Data {{ value: before }} }} }} }}"
        ), accepted);
    }
    for (condition, accepted) in [
        (
            "(result.left in Message::Data) && (result.right in Message::Empty)",
            true,
        ),
        ("result.right in Message::Data", false),
    ] {
        check(
            &format!(
                "data Pair {{ left: Message; right: Message; }}
            machine make() -> Pair ensures {condition};
            {{ Pair {{ left: Message::Data {{ value: 1 }}, right: Message::Empty }} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn result_field_membership_retains_live_subject_and_exit_custody() {
    check(
        "data Wrapper { message: Message; }
        machine forward(value: Wrapper) -> Wrapper
        requires value.message in Message::Data;
        ensures result.message in Message::Data; { value }",
        true,
    );
    check(
        "data Wrapper { message: Message; }
        machine forward(value: Message) -> Wrapper
        requires value in Message::Data;
        ensures result.message in Message::Data; { Wrapper { message: value } }",
        true,
    );
    check(
        "data Wrapper { message: Message; }
        machine forward(value: Wrapper) -> Wrapper
        ensures !(result.message in Message::Data); { value }",
        false,
    );
    check(
        "data Wrapper { message: Message; }
        machine forward(mut value: Wrapper) -> Wrapper
        requires value.message in Message::Data;
        ensures result.message in Message::Data; { value.message = Message::Empty; value }",
        false,
    );
    for (other, accepted) in [
        ("Message::Data { value: 2 }", true),
        ("Message::Empty", false),
    ] {
        check(
            &format!(
                "data Wrapper {{ message: Message; }}
            machine make(flag: bool) -> Wrapper ensures result.message in Message::Data;
            {{ transition flag {{
                true -> (Wrapper {{ message: Message::Data {{ value: 1 }} }})
                false -> (Wrapper {{ message: {other} }})
            }} }}"
            ),
            accepted,
        );
    }
}
