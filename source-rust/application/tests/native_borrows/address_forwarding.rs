//! A machine returning an address can carry its caller's storage into a raw
//! borrow. The returned integer alone owns no backing.
use super::{Program, READ, report};

const FORWARDERS: &str = r#"
machine forward(value: address) -> address { value }
machine again(value: address) -> address { forward(forward(value)) }
machine second(ignored: address, answer: address) -> address { forward(answer) }
machine choose(flag: bool, left: address, right: address) -> address {
    transition flag { true -> left, false -> right }
}
"#;

fn source_result(formation: &str) -> Program {
    Program::new(
        &format!(
            r#"
{FORWARDERS}
machine expose<'a>(incoming: address, flag: bool) -> @'a u8 {{
    let local: u8 = 3u8;
    let address: address = {formation};
    |native|::borrow_at::<u8>(address)
}}
machine main() crashes Trap {{
    let owned: u8 = 17u8;
    let value: @u8 = expose(address_of(@owned), false);
    transition $value == 17u8 {{ true -> done(), _ -> wrong() }}
    machine done() {{}}
    machine wrong() {{ crash Trap; }}
}}"#
        ),
        &[READ],
    )
}

#[test]
fn an_address_returning_helper_preserves_the_callers_backing_lifetime() {
    for formation in [
        "forward(address_of(@local))",
        "again(address_of(@local))",
        "choose(flag, again(incoming), forward(address_of(@local)))",
    ] {
        let refused = source_result(formation).check();
        let message = report(&refused);
        assert!(
            !refused.status.success() && message.contains("outlive its local backing"),
            "{formation}: {message}"
        );
    }
    source_result("second(address_of(@local), again(incoming))").assert_native_returns();
}

#[test]
fn a_helper_returning_a_reference_slots_address_preserves_its_exact_referent() {
    let program = Program::new(
        &format!(
            r#"
{FORWARDERS}
machine read<'a>(input: @'a u8) -> u8 {{
    let stored: @'a u8 = input;
    let address: address = again(address_of(@stored));
    let outer: @@'a u8 = |native|::borrow_at::<@'a u8>(address);
    let inner: @'a u8 = $outer;
    $inner
}}
machine main() crashes Trap {{
    let owned: u8 = 19u8;
    transition read(@owned) == 19u8 {{ true -> done(), _ -> wrong() }}
    machine done() {{}}
    machine wrong() {{ crash Trap; }}
}}"#
        ),
        &[READ],
    );
    program.assert_native_returns();
}

#[test]
fn a_returned_field_address_borrows_the_structural_argument_that_supplied_it() {
    let body = r#"
data Pair { left: u8; right: u8 }
machine locate(pair: @Pair) -> address { address_of(@pair.right) }
machine again(pair: @Pair) -> address { locate(pair) }
machine choose<'a>(incoming: @'a Pair) -> @'a u8 {
    let local: Pair = Pair { left: 3u8, right: 4u8 };
    |native|::borrow_at::<u8>(again(ARG))
}
machine main() crashes Trap {
    let caller: Pair = Pair { left: 13u8, right: 17u8 };
    let result: @u8 = choose(@caller);
    transition $result == 17u8 && caller.left == 13u8 { true -> {}, _ -> wrong() }
    machine wrong() { crash Trap; }
}"#;
    let refused = Program::new(&body.replace("ARG", "@local"), &[READ]).check();
    let diagnostic = report(&refused);
    assert!(
        !refused.status.success() && diagnostic.contains("outlive its local backing"),
        "{diagnostic}"
    );
    Program::new(&body.replace("ARG", "incoming"), &[READ]).assert_native_returns();
}

#[test]
fn a_native_borrow_cannot_be_formed_after_its_address_owners_return() {
    let program = Program::new(
        r#"
machine local_address() -> address {
    let local: u8 = 13u8;
    address_of(@local)
}
machine forward() -> address { local_address() }
machine main() {
    let local: @u8 = |native|::borrow_at::<u8>(forward());
}
"#,
        &[READ],
    );
    let refused = program.check();
    let diagnostic = report(&refused);
    assert!(
        !refused.status.success()
            && diagnostic.contains("already returned")
            && !diagnostic.contains("not supported yet"),
        "{diagnostic}"
    );
}

#[test]
fn a_caller_must_supply_live_storage_to_the_parameter_a_helper_borrows() {
    let source = r#"
machine local_address() -> address {
    let local: u8 = 7u8;
    address_of(@local)
}
machine read(location: address) -> u8 {
    let byte: @u8 = |native|::borrow_at::<u8>(location);
    $byte
}
machine relay(ignored: address, selected: address) -> u8 { read(selected) }
machine optional(needed: bool, location: address) -> u8 {
    transition needed { true -> read(location), false -> 31u8 }
}
machine main() crashes Trap {
    let live: u8 = 31u8;
    let past: address = local_address();
    let left: u8 = relay(ARGUMENTS);
    let right: u8 = optional(false, past);
    transition left == 31u8 && right == 31u8 { true -> {}, _ -> wrong() }
    machine wrong() { crash Trap; }
}"#;
    let refused = Program::new(
        &source.replace("ARGUMENTS", "address_of(@live), past"),
        &[READ],
    )
    .check();
    let diagnostic = report(&refused);
    assert!(
        !refused.status.success()
            && diagnostic.contains("already returned")
            && !diagnostic.contains("not supported yet"),
        "{diagnostic}"
    );
    Program::new(
        &source.replace("ARGUMENTS", "past, address_of(@live)"),
        &[READ],
    )
    .assert_native_returns();
}

#[test]
fn an_address_into_an_inline_array_keeps_its_owners_lifetime() {
    let source = r#"
machine find(input: @[u8; 2], i: u64) -> address requires i < 2u64 {
    address_of(@input[i])
}
machine locate(input: @[u8; 2], i: u64) -> address requires i < 2u64 {
    let owned: [u8; 2] = [4u8, 7u8];
    LOCATION
}
machine main() crashes Trap {
    let caller: [u8; 2] = [19u8, 37u8];
    let cell: @u8 = |native|::borrow_at::<u8>(locate(@caller, 1u64));
    transition $cell == 37u8 { true -> {}, false -> bad() }
    machine bad() { crash Trap; }
}"#;
    for location in [
        "address_of(@owned[1u64])",
        "address_of(@owned[i])",
        "find(@owned, i)",
    ] {
        let refused = Program::new(&source.replace("LOCATION", location), &[READ]).check();
        let diagnostic = report(&refused);
        assert!(
            !refused.status.success() && diagnostic.contains("already returned"),
            "{location}: {diagnostic}"
        );
    }
    Program::new(&source.replace("LOCATION", "find(input, i)"), &[READ]).assert_native_returns();
}
