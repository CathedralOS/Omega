use crate::tests::front_end::checked_program_result;

fn check(source: &str, accepted: bool) {
    match checked_program_result(source) {
        Ok(_) => assert!(accepted, "unproved contract accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("requires")
                        || diagnostic.message.contains("ensures")
                        || diagnostic.message.contains("implicit domain weakening")
                        || diagnostic.message.contains("implicit domain strengthening")
                        || diagnostic.message.contains("distinct normalized instances")
                        || diagnostic.message.contains("violates required fact")),
                "{diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn qualified_result_membership_composes_before_call_requires() {
    for argument in [
        "mark(value)",
        "relay(mark(value))",
        "value as i64 in Km",
        "match flag { true -> mark(value), false -> value as i64 in Km }",
        "match flag { true -> mark(value), false -> match flag { true -> mark(value), false -> mark(value) } }",
    ] {
        check(
            &format!(
                r#"
            domain i64::Km;
            machine mark(value: i64) -> i64 in Km {{ value as i64 in Km }}
            machine relay(value: i64 in Km) -> i64 in Km {{ value }}
            machine run(flag: bool, value: i64) -> i64 in Km {{ relay({argument}) }}
        "#
            ),
            true,
        );
    }
}

#[test]
fn result_membership_does_not_come_from_the_recipient_type() {
    for argument in [
        "value",
        "bare(value)",
        "match flag { true -> bare(value), false -> bare(value) }",
        "value as i64 in Seconds",
        "other(value)",
        "match flag { true -> other(value), false -> other(value) }",
    ] {
        check(
            &format!(
                r#"
            domain i64::Km;
            domain i64::Seconds;
            machine bare(value: i64) -> i64 {{ value }}
            machine other(value: i64) -> i64 in Seconds {{ value as i64 in Seconds }}
            machine relay(value: i64 in Km) -> i64 in Km {{ value }}
            machine run(flag: bool, value: i64) -> i64 in Km {{ relay({argument}) }}
        "#
            ),
            false,
        );
    }
}

#[test]
fn storage_dependencies_never_assert_their_boolean_operands() {
    check(
        r#"
        machine establish(first: &mut bool, second: &mut bool)
        ensures first == second
        { first = false; second = false; }
        machine consume(flag: bool) requires flag {}
        machine run() {
            let mut first: bool = false;
            let mut second: bool = false;
            establish(&mut first, &mut second);
            consume(first);
        }
        "#,
        false,
    );
}

#[test]
fn unknown_short_circuit_does_not_publish_conditional_guarantees() {
    check(
        r#"
        machine establish(flag: &mut bool) -> bool
        ensures flag == true
        { flag = true; true }
        machine consume(flag: bool)
        requires flag == true
        {}
        machine run(gate: bool, flag: &mut bool) {
            let ignored: bool = gate && establish(&mut flag);
            consume(flag);
        }
    "#,
        false,
    );
}

#[test]
fn unknown_short_circuit_invalidates_only_conditional_write_subject() {
    for (returned, accepted) in [("flag", false), ("other", true)] {
        check(
            &format!(
                r#"
            machine clear(flag: &mut bool) -> bool {{ flag = false; true }}
            machine run(gate: bool) -> bool
            ensures result == true
            {{
                let mut flag: bool = true;
                let other: bool = true;
                let ignored: bool = gate && clear(&mut flag);
                {returned}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn named_state_arguments_transport_established_memberships() {
    // The membership rides on the argument's denoted place, so a qualified
    // call result, a staged local, a selected field, and a field below a
    // passed aggregate all feed a bare destination parameter that a later
    // `requires` contract consumes. No value is copied merely to carry the
    // claim.
    let prelude = r#"
        data Token { value: u64; }
        data Holder { token: Token; }
        domain Token::Issued established by TokenIssuer::issue;
        trait TokenIssuer { machine issue(value: u64) -> Token ensures result in Token::Issued; }
        machine Token::issue(value: u64) -> Token satisfies TokenIssuer::issue ensures result in Token::Issued { Token { value: value } }
        machine consume(token: Token in Issued) {}
    "#;
    for body in [
        // Qualified call result passed straight into a bare parameter.
        r#"
        machine run(value: u64) {
            transition { _ -> done(Token::issue(value)) }
            state done(token: Token) { consume(token); }
        }
    "#,
        // Membership staged through a local binding.
        r#"
        machine run(value: u64) {
            let issued: Token = Token::issue(value);
            transition { _ -> done(issued) }
            state done(token: Token) { consume(token); }
        }
    "#,
        // A selected field of a passed aggregate's own value.
        r#"
        machine run(value: u64) {
            let mut holder: Holder = Holder { token: Token { value: 0 } };
            holder.token = Token::issue(value);
            transition { _ -> done(holder.token) }
            state done(token: Token) { consume(token); }
        }
    "#,
        // The claim lives below the argument's place and follows it.
        r#"
        machine run(value: u64) {
            let mut holder: Holder = Holder { token: Token { value: 0 } };
            holder.token = Token::issue(value);
            transition { _ -> done(holder) }
            state done(holder: Holder) { consume(holder.token); }
        }
    "#,
        // A renamed parameter still carries its declared membership.
        r#"
        machine run(token: Token in Issued) {
            transition { _ -> hold(token) }
            state hold(token: Token in Issued) {
                transition { _ -> done(token) }
            }
            state done(token: Token in Issued) { consume(token); }
        }
    "#,
    ] {
        check(&format!("{prelude}\n{body}"), true);
    }
}
