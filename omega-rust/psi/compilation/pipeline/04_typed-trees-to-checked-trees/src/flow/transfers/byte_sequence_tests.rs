//! Regression coverage for the numeric evidence indexed text writes consume:
//! effectful call arguments, receiver-call field reads resolved against the
//! caller's storage, and conversion policies the selected scalar plan has no
//! node for (today `Saturating`). A case that cannot carry evidence must keep
//! rejecting — nothing here invents a value or a tighter bound.
use crate::tests::front_end::checked_program_result;
use checked_trees::CheckedTrees;

fn check(source: &str) -> Result<CheckedTrees, Vec<String>> {
    checked_program_result(source).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect()
    })
}

fn write_machine(signature: &str, statement: &str) -> String {
    format!(
        "domain [u8; 4]::Ascii requires ascii_only(self); {signature} \
         requires output in Ascii ensures output in Ascii {{ {statement} }}"
    )
}

/// Cast domains nested inside call arguments ride the callee's declared
/// parameter domain: `Saturating` lowers to a plan the captured evaluation
/// can replay, while `Trapping` binary arithmetic has no selected scalar
/// form and fails closed at the ensures contract.
#[test]
fn argument_cast_policies_follow_the_callee_parameter_domain() {
    for (domain, expected) in [("Saturating", true), ("Trapping", false)] {
        let source = format!(
            "machine digit(value: u64 in {domain}) -> u8 {{ ((value % 10 + 48) as u8 in Wrapping) as u8 }} {}",
            write_machine(
                "machine write(output: &mut [u8; 4], position: u64 [0..=3], unknown: u64)",
                &format!("output[position] = digit((unknown as u64 in {domain}));"),
            )
        );
        match (check(&source), expected) {
            (Ok(_), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove ensures")),
                "{domain}: {messages:?}"
            ),
            (result, _) => panic!("{domain}: {result:?}"),
        }
    }
}

/// A receiver call's `self` field reads resolve against the caller's receiver
/// place: live snapshot first, then declared/carrier bounds. The returned
/// digit then satisfies the element predicate.
#[test]
fn receiver_calls_resolve_self_fields_against_the_caller_place() {
    let source = format!(
        "data Pad {{ cell: u64 [0..=9]; }} \
         machine Pad::digit(&mut self) -> u8 {{ ((self.cell % 10 + 48) as u8 in Wrapping) as u8 }} {}",
        write_machine(
            "machine write(output: &mut [u8; 4], pad: &mut Pad, position: u64 [0..=3])",
            "output[position] = pad.digit();",
        )
    );
    check(&source).map(|_| ()).unwrap_or_else(|messages| {
        panic!("receiver self-field bounds feed the call result: {messages:?}")
    });
}

#[test]
fn subslice_bindings_materialize_preserved_byte_predicates() {
    for predicate in ["no_nul", "ascii_only"] {
        let source = format!(
            "domain [u8]::Text(bytes) requires {predicate}(bytes);
             machine inspect(bytes: &[u8] in Text) {{}}
             machine relay(bytes: &[u8] in Text) {{
                 let prefix: &[u8] = bytes[0..0];
                 let alias: &[u8] = prefix;
                 let suffix: &[u8] = alias[0..0];
                 inspect(suffix[..]);
             }}"
        );
        check(&source).map(|_| ()).unwrap_or_else(|messages| {
            panic!("{predicate} follows the exact captured descriptors: {messages:?}")
        });
    }
}

#[test]
fn replacing_a_subslice_binding_retires_its_byte_predicates() {
    let source = "domain [u8]::Text(bytes) requires no_nul(bytes);
        machine inspect(bytes: &[u8] in Text) {}
        machine relay(bytes: &[u8] in Text, other: &[u8]) {
            let mut prefix: &[u8] = bytes[0..0];
            prefix = other;
            inspect(prefix[0..0]);
        }";
    let messages = check(source).expect_err("the replacement has no byte-class premise");
    assert!(
        messages
            .iter()
            .any(|message| message.contains("cannot prove requires contract")),
        "{messages:?}"
    );
}

#[test]
fn subslice_bindings_do_not_materialize_nonpreserving_predicates() {
    for (predicate, range) in [("valid_utf8", "0..1"), ("non_empty", "0..0")] {
        let source = format!(
            "domain [u8]::Text(bytes) requires {predicate}(bytes);
             machine inspect(bytes: &[u8] in Text) {{}}
             machine relay(bytes: &[u8] in Text) requires bytes.len > 0 {{
                 let prefix: &[u8] = bytes[{range}];
                 inspect(prefix[..]);
             }}"
        );
        let messages = check(&source).expect_err("the byte cut does not preserve this predicate");
        assert!(
            messages
                .iter()
                .any(|message| message.contains("cannot prove requires contract")),
            "{predicate}: {messages:?}"
        );
    }
}
