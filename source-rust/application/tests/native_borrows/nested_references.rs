//! The outer operation borrows a slot holding a reference. Loading that slot
//! recovers the inner reference and its own backing.
use super::{Program, READ, UPGRADE};

fn inspect(formation: &str, grants: &[&str]) -> Program {
    let source = format!(
        r#"
machine inspect<'a>(input: @'a u8) -> u8 {{
    let stored: @'a u8 = input;
    {formation}
    let inner: @'a u8 = $nested;
    $inner
}}
machine main() crashes Trap {{
    let value: u8 = 19u8;
    transition inspect(@value) == 19u8 {{ true -> done(), _ -> wrong() }}
    machine done() {{}}
    machine wrong() {{ crash Trap; }}
}}"#
    );
    Program::new(&source, grants)
}

#[test]
fn an_explicit_borrow_can_lend_a_slot_holding_a_reference() {
    let program = inspect("let nested: @@'a u8 = @stored;", &[]);
    program.assert_psi_returns();
    program.assert_native_returns();
}

#[test]
fn upgrading_the_outer_borrow_preserves_the_inner_reference() {
    let program = inspect(
        "let nested: @mutable @'a u8 = |native|::upgrade_mutability::<@'a u8>(@stored);",
        &[UPGRADE],
    );
    program.assert_psi_returns();
    program.assert_native_returns();
}

#[test]
fn a_raw_read_from_a_reference_slot_recovers_its_stored_reference() {
    for address in [
        "let location: address = address_of(@stored);",
        "let outer: @@'a u8 = @stored; let location: address = address_of(outer);",
    ] {
        let formation =
            format!("{address} let nested: @@'a u8 = |native|::borrow_at::<@'a u8>(location);");
        inspect(&formation, &[READ]).assert_native_returns();
    }
}

fn replacement(action: &str, formation: &str, grants: &[&str]) -> Program {
    let code = format!(
        r#"
machine inspect<'a>(left: @'a u8, right: @'a u8) -> u8 {{
    let mutable stored: @'a u8 = left;
    {formation}
    {action}
    $stored
}}
machine main() crashes Trap {{
    let first: u8 = 19u8;
    let second: u8 = 31u8;
    let result: u8 = inspect(@first, @second);
    transition result == 31u8 && first == 19u8 && second == 31u8 {{
        true -> done(), _ -> wrong()
    }}
    machine done() {{}}
    machine wrong() {{ crash Trap; }}
}}"#
    );
    Program::new(&code, grants)
}

const UPGRADE_SLOT: &str =
    "let edit: @mutable @'a u8 = |native|::upgrade_mutability::<@'a u8>(@stored);";

#[test]
fn replacing_a_reference_through_an_upgrade_changes_only_the_slot() {
    let program = replacement("$edit = right;", UPGRADE_SLOT, &[UPGRADE]);
    program.assert_native_returns();
    program.assert_psi_returns();
}

#[test]
fn an_exclusive_slot_cannot_write_through_its_shared_inner_reference() {
    let program = replacement(
        "let inner: @'a u8 = $edit;\n    $inner = 31u8;",
        UPGRADE_SLOT,
        &[UPGRADE],
    );
    let output = program.check();
    assert!(!output.status.success());
    let message = super::report(&output);
    assert!(message.contains("grants shared access only"), "{message}");
}

#[test]
fn replacing_a_reference_through_its_native_address_changes_only_the_slot() {
    let formation = "let location: address = address_of(@stored);\n    let edit: @mutable @'a u8 = |native|::borrow_mutable_at::<@'a u8>(location);";
    replacement("$edit = right;", formation, &[READ, super::WRITE]).assert_native_returns();
}

#[test]
fn the_inner_lifetime_cannot_extend_a_native_borrow_of_the_local_slot() {
    let code = r#"
machine leave<'a>(arg: @'a u8) -> @'a @'a u8 {
    let stored: @'a u8 = arg;
    |native|::borrow_at::<@'a u8>(address_of(@stored))
}
machine main() crashes Trap {
    let value: u8 = 19u8;
    let outer: @@u8 = leave(@value);
    let inner: @u8 = $outer;
    transition $inner == 19u8 { true -> done(), _ -> wrong() }
    machine done() {}
    machine wrong() { crash Trap; }
}"#;
    let output = Program::new(code, &[READ]).check();
    assert!(!output.status.success());
    let message = super::report(&output);
    assert!(
        message.contains("outlive its local backing storage"),
        "{message}"
    );
}
