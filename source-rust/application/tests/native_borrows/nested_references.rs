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

#[test]
fn borrowing_a_fresh_reference_keeps_both_layers_until_the_last_use() {
    for formation in ["@@owned", "@share(@owned)"] {
        let program = Program::new(
            &format!(
                r#"
machine share<'a>(source: @'a u8) -> @'a u8 {{ source }}
machine main() crashes Trap {{
    let owned: u8 = 37u8;
    let outer: @@u8 = {formation};
    let inner: @u8 = $outer;
    transition $inner == 37u8 {{ true -> done(), _ -> wrong() }}
    machine done() {{}}
    machine wrong() {{ crash Trap; }}
}}"#
            ),
            &[],
        );
        program.assert_native_returns();
        program.assert_psi_returns();
    }
}

#[test]
fn a_reference_to_a_temporary_descriptor_cannot_escape() {
    let program = Program::new(
        r#"
machine escape<'a>(source: @'a u8) -> @'a @'a u8 { @@$source }
machine main() crashes Trap {
    let owned: u8 = 37u8;
    let outer: @@u8 = escape(@owned);
    let inner: @u8 = $outer;
    transition $inner == 37u8 { true -> done(), _ -> wrong() }
    machine done() {}
    machine wrong() { crash Trap; }
}"#,
        &[],
    );
    let output = program.check();
    let message = super::report(&output);
    assert!(
        !output.status.success() && message.contains("local storage cannot establish"),
        "{message}"
    );
}

#[test]
fn an_explicit_borrow_keeps_its_layer_at_a_named_call() {
    const BODY: &str = r#"
data Pair { value: u8 }
machine read(pair: @Pair) -> u8 { pair.value }
machine relay(pair: @Pair) -> u8 { read(ARGUMENT) }
machine main() crashes Trap {
    let owned: Pair = Pair { value: 43u8 };
    transition relay(@owned) == 43u8 { true -> {}, _ -> wrong() }
    machine wrong() { crash Trap; }
}"#;
    let refused = Program::new(&BODY.replace("ARGUMENT", "@pair"), &[]).check();
    let diagnostic = super::report(&refused);
    assert!(
        !refused.status.success()
            && diagnostic.contains("adds a reference layer beyond the destination type")
            && !diagnostic.contains("not supported yet"),
        "{diagnostic}"
    );
    for expression in ["pair", "@$pair"] {
        let program = Program::new(&BODY.replace("ARGUMENT", expression), &[]);
        program.assert_psi_returns();
        program.assert_native_returns();
    }
}

#[test]
fn a_nested_reference_argument_keeps_each_written_layer() {
    let body = r#"
machine read(value: @@u8) -> u8 { $$value }
machine main() crashes Trap {
    let owned: u8 = 53u8;
    let slot: @u8 = @owned;
    transition read(ARGUMENT) == 53u8 { true -> {}, _ -> wrong() }
    machine wrong() { crash Trap; }
}"#;
    let refused = Program::new(&body.replace("ARGUMENT", "@@slot"), &[]).check();
    let message = super::report(&refused);
    assert!(
        !refused.status.success()
            && message.contains("adds a reference layer beyond the destination type"),
        "{message}"
    );
    for argument in ["@slot", "@@owned"] {
        let program = Program::new(&body.replace("ARGUMENT", argument), &[]);
        program.assert_psi_returns();
        program.assert_native_returns();
    }
}

#[test]
fn a_binding_field_or_return_checks_the_same_written_borrow_as_a_call() {
    let source = r#"
data Pair { value: u8 }
data Holder<'a> { pair: @'a Pair }
machine read(pair: @Pair) -> u8 { pair.value }
DECLARATION
machine main() crashes Trap {
    let owned: Pair = Pair { value: 39u8 };
    transition RESULT == 39u8 { true -> {}, false -> wrong() }
    machine wrong() { crash Trap; }
}"#;
    for (declaration, result) in [
        (
            "machine relay(pair: @Pair) -> u8 { let stored: @Pair = FORM; read(stored) }",
            "relay(@owned)",
        ),
        (
            "machine relay<'a>(pair: @'a Pair) -> u8 { let holder: Holder<'a> = Holder { pair: FORM }; read(holder.pair) }",
            "relay(@owned)",
        ),
        (
            "machine relay<'a>(pair: @'a Pair) -> @'a Pair { FORM }",
            "read(relay(@owned))",
        ),
    ] {
        let body = source
            .replace("DECLARATION", declaration)
            .replace("RESULT", result);
        let refused = Program::new(&body.replace("FORM", "@pair"), &[]).check();
        let diagnostic = super::report(&refused);
        assert!(
            !refused.status.success()
                && diagnostic.contains("adds a reference layer beyond the destination type"),
            "{declaration}: {diagnostic}"
        );
        let valid = Program::new(&body.replace("FORM", "@$pair"), &[]);
        valid.assert_psi_returns();
        valid.assert_native_returns();
    }
}
