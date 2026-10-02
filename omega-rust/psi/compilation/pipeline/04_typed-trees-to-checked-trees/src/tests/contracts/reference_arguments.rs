use super::parse_typed_trees;
use crate::CheckingRequest;
use crate::lower_typed_trees;

const ASCII_DOMAIN: &str = "domain [u8]::Ascii(bytes) requires ascii_only(bytes);";

fn check_rejection(source: &str, expected_message: &str) {
    let typed = parse_typed_trees(source);
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("reference compatibility must not supply missing evidence or authority");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected_message)),
        "expected {expected_message:?}: {diagnostics:#?}\n{source}"
    );
}

#[test]
fn named_shared_reference_arguments_preserve_derived_domain_facts() {
    for body in [
        "let local: &[u8] = bytes[0..0]; consume(local);",
        "let local: &[u8] = bytes; consume(local);",
        "let prefix: &[u8] = bytes[0..0]; let local: &[u8] = prefix; consume(local);",
    ] {
        let source = format!(
            r#"
            {ASCII_DOMAIN}
            machine consume(bytes: &[u8] in Ascii) {{}}
            machine relay(bytes: &[u8] in Ascii) {{ {body} }}
            "#
        );
        lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics:#?}"));
    }
}

#[test]
fn named_record_reference_arguments_preserve_live_domain_facts() {
    let source = r#"
        data Token { value: u64; }
        domain Token::Valid(token) requires token.value > 0;
        machine consume(token: &Token in Valid) {}
        machine relay(token: &Token in Valid) {
            let local: &Token = token;
            consume(local);
        }
        "#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("a shared record alias retains its referent's live domain facts");
    check_rejection(
        &source.replace("relay(token: &Token in Valid)", "relay(token: &Token)"),
        "cannot prove requires contract for call consume from relay",
    );
}

#[test]
fn stored_reference_arguments_require_facts_for_the_selected_field() {
    let source = r#"
        data Token { value: u64; }
        data Holder { token: &Token; }
        domain Token::Valid(token) requires token.value > 0;
        machine consume(token: &Token in Valid) {}
        machine relay(holder: &Holder)
        requires holder.token in Valid {
            consume(holder.token);
        }
        "#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("the stored reference uses its exact field's live domain facts");
    check_rejection(
        &source.replace("requires holder.token in Valid", ""),
        "cannot prove requires contract for call consume from relay",
    );
}

#[test]
fn named_shared_reference_arguments_require_domain_evidence() {
    check_rejection(
        &format!(
            r#"
            {ASCII_DOMAIN}
            machine consume(bytes: &[u8] in Ascii) {{}}
            machine relay(bytes: &[u8]) {{
                let local: &[u8] = bytes;
                consume(local);
            }}
            "#
        ),
        "cannot prove requires contract for call consume from relay",
    );
}

#[test]
fn reassigned_reference_arguments_do_not_reuse_initializer_facts() {
    check_rejection(
        &format!(
            r#"
            {ASCII_DOMAIN}
            machine consume(bytes: &[u8] in Ascii) {{}}
            machine relay(bytes: &[u8] in Ascii, replacement: &[u8]) {{
                let mut local: &[u8] = bytes[0..0];
                local = replacement;
                consume(local);
            }}
            "#
        ),
        "cannot prove requires contract for call consume from relay",
    );
}

#[test]
fn reference_arguments_keep_exact_element_and_referee_types() {
    for source in [
        format!(
            r#"
            {ASCII_DOMAIN}
            machine consume(bytes: &[u8] in Ascii) {{}}
            machine relay(values: &[i32]) {{
                let local: &[i32] = values;
                consume(local);
            }}
            "#
        ),
        r#"
        data Token { value: u64; }
        data Other { value: u64; }
        domain Token::Valid(token) requires token.value > 0;
        machine consume(token: &Token in Valid) {}
        machine relay(value: &Other) {
            let local: &Other = value;
            consume(local);
        }
        "#
        .to_owned(),
    ] {
        check_rejection(&source, "for state `entry` expects");
    }
}

#[test]
fn qualified_shared_reference_arguments_cannot_supply_mutable_access() {
    check_rejection(
        &format!(
            r#"
            {ASCII_DOMAIN}
            machine consume(bytes: &mut [u8] in Ascii) {{}}
            machine relay(bytes: &[u8] in Ascii) {{
                let mut local: &[u8] = bytes[0..0];
                consume(local);
            }}
            "#
        ),
        "caller lends only immutable access",
    );
}

#[test]
fn named_reference_arguments_do_not_mint_routed_membership() {
    check_rejection(
        r#"
        data Token { value: u64; }
        domain Token::Issued established by Issuer::issue;
        boundary trait Issuer { machine issue() -> Token in Issued; }
        machine consume(token: &Token in Issued) {}
        machine relay(token: &Token) {
            let local: &Token = token;
            consume(local);
        }
        "#,
        "cannot prove requires contract for call consume from relay",
    );
}
