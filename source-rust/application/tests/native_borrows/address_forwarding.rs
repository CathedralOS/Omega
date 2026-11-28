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
