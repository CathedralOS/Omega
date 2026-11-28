//! A native borrow keeps the domain memberships its source can supply.
use super::{Program, READ, UPGRADE, WRITE, report};

const ISSUED_TOKEN: &str = r#"
use {omega}/language/core/layout.omg as layout;
use {omega}/language/core/reflect/reflect.omg as reflect;
use {omega}/language/core/access/access.omg as access;

data Token [copy] { value: u32; }
domain Token::Issued access TokenAccess;
TokenAccess satisfies |layout|::AccessPolicy {
    machine plan(schema: |reflect|::TypeSchema) -> |layout|::AccessPlan
        reaches none crashes none terminates {
        |access|::new_plan(@schema,
            |layout|::Issuance::Only { routes: [|reflect|::callable(issue)] })
    }
}
machine issue(token: Token) -> Token in Token::Issued { token as +Token::Issued }
machine consume(token: Token in Token::Issued) -> u32 { token.value }
"#;

#[test]
fn upgrading_an_issued_value_keeps_its_domain() {
    let main = r#"
machine main() crashes Trap {
    let mutable original: Token in Token::Issued = issue(Token { value: 19u32 });
    let borrow: @mutable Token in Token::Issued =
        |native|::upgrade_mutability::<Token in Token::Issued>(@original);
    let observed: u32 = consume($borrow);
    transition observed == 19u32 { true -> done(), false -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    let body = format!("{ISSUED_TOKEN}{main}");
    let issued = Program::new(&body, &[UPGRADE]);
    issued.assert_psi_returns();
    issued.assert_native_returns();

    let unissued = body.replace(
        "Token in Token::Issued = issue(Token { value: 19u32 })",
        "Token = Token { value: 19u32 }",
    );
    let declined = Program::new(&unissued, &[UPGRADE]).check();
    let detail = report(&declined);
    assert!(
        !declined.status.success()
            && detail.contains("type carries no domain")
            && detail.contains("the borrow's referent type declares `Token::Issued`"),
        "{detail}"
    );
}

#[test]
fn granting_raw_address_access_cannot_issue_a_domain() {
    for (access, operation, grants) in [
        ("@", "borrow_at", &[READ][..]),
        ("@mutable ", "borrow_mutable_at", &[READ, WRITE][..]),
    ] {
        let body = format!(
            r#"{ISSUED_TOKEN}
machine main() {{
    let mutable original: Token = Token {{ value: 19u32 }};
    let borrow: {access}Token in Token::Issued =
        |native|::{operation}::<Token in Token::Issued>(address_of(@original));
    _ = consume($borrow);
}}"#
        );
        let refused = Program::new(&body, grants).check();
        let detail = report(&refused);
        assert!(
            !refused.status.success()
                && detail.contains("a native address carries no domain membership")
                && detail.contains("main.omg:"),
            "{detail}"
        );
    }
}

#[test]
fn upgrading_a_record_keeps_the_domain_of_its_issued_field() {
    let body = format!(
        r#"{ISSUED_TOKEN}
data Envelope [copy] {{ token: Token in Token::Issued; }}
machine main() crashes Trap {{
    let mutable packet: Envelope = Envelope {{ token: issue(Token {{ value: 19u32 }}) }};
    let borrow: @mutable Envelope = |native|::upgrade_mutability::<Envelope>(@packet);
    transition consume(($borrow).token) == 19u32 {{ true -> done(), false -> bad() }}
    machine done() {{}}
    machine bad() {{ crash Trap; }}
}}"#
    );
    let program = Program::new(&body, &[UPGRADE]);
    program.assert_psi_returns();
    program.assert_native_returns();
}

#[test]
fn changing_an_issued_value_through_an_upgrade_requires_its_establishment_route() {
    let main = r#"
machine change(token: @mutable Token in Token::Issued) { token.value = 23u32; }
machine main() crashes Trap {
    let mutable original: Token in Token::Issued = issue(Token { value: 19u32 });
    let writable: @mutable Token in Token::Issued =
        |native|::upgrade_mutability::<Token in Token::Issued>(@original);
    change(writable);
    transition consume(original) == 23u32 { true -> done(), false -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    let source = format!("{ISSUED_TOKEN}{main}");
    let denied = Program::new(&source, &[UPGRADE]).check();
    let detail = report(&denied);
    assert!(
        !denied.status.success()
            && detail.contains("establishment routes")
            && detail.contains("Token::Issued"),
        "{detail}"
    );

    let direct_write = source
        .replace("    change(writable);", "    writable.value = 23u32;")
        .replace(
            "machine change(token: @mutable Token in Token::Issued) { token.value = 23u32; }",
            "",
        );
    let denied = Program::new(&direct_write, &[UPGRADE]).check();
    let detail = report(&denied);
    assert!(
        !denied.status.success()
            && detail.contains("establishment routes")
            && detail.contains("Token::Issued"),
        "{detail}"
    );

    let delegated = source.replace(
        "[|reflect|::callable(issue)]",
        "[|reflect|::callable(issue), |reflect|::callable(change)]",
    );
    let allowed = Program::new(&delegated, &[UPGRADE]);
    allowed.assert_psi_returns();
    allowed.assert_native_returns();
}

#[test]
fn upgrading_a_slice_keeps_its_issued_elements_and_offset() {
    let main = r#"
machine inspect(view: @mutable [Token in Token::Issued]) -> u32 crashes Trap {
    transition view.length == 2u64 { true -> first(view), _ -> bad() }
    machine first(view: @mutable [Token in Token::Issued]) -> u32
        requires view.length == 2u64 { consume(view[0u64]) }
    machine bad() -> u32 { crash Trap; }
}
machine main() crashes Trap {
    let mutable tokens: [Token in Token::Issued; 3] = [
        issue(Token { value: 11u32 }), issue(Token { value: 19u32 }), issue(Token { value: 31u32 })
    ];
    let view: @[Token in Token::Issued] = @tokens[1..3];
    let writable: @mutable [Token in Token::Issued] =
        |native|::upgrade_mutability::<[Token in Token::Issued]>(view);
    transition inspect(writable) == 19u32 { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    let program = Program::new(&format!("{ISSUED_TOKEN}{main}"), &[UPGRADE]);
    program.assert_psi_returns();
    program.assert_native_returns();
}
