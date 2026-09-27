//! Boundary `ensures result in <domain>` clauses on an invoked requirement:
//! a clause authorized by the domain's `established by` route is admitted
//! through the exact-qualification gate, folds the domain onto the structural
//! result qualification at lowering, and survives codec + independent
//! verification; a clause naming a domain that does not authorize this
//! requirement is still refused.

use checked_trees_to_lowered_psi::TerminalMachineSelection;

fn program(domain_declaration: &str) -> String {
    format!(
        r#"
        machine identity16(value: u16) -> u16
        requires 0u16 == 0u16
        ensures result == value
        {{ value }}
        pub data Token {{ flag: bool; }}
        {domain_declaration}
        boundary trait Factory {{
            machine create(first: u16, second: u16) -> Token reaches Factory
            ensures result in Token::Held;
        }}
        boundary trait Host {{ machine finish(value: Token) reaches Host; }}
        data Main {{}}
        machine Main::main() reaches Factory + Host {{
            let result: Token = Factory::create(identity16(5u16), 7u16);
            Host::finish(result);
        }}
        "#
    )
}

#[test]
fn unauthorized_result_domain_still_rejects_the_call() {
    let checked = crate::front_end::checked_program(&program("domain Token::Held;"));
    assert!(matches!(
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Main::main")
        ),
        Err(checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan { .. })
    ));
}
