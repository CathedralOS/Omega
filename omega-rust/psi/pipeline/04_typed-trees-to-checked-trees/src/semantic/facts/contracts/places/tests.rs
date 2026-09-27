use crate::semantic::facts::contracts::places::contract_owner_self_symbol;
use crate::tests::front_end::typed_program;
use checked_trees::ContractProofFactOwner;

#[test]
fn machine_contract_cannot_borrow_a_sibling_states_self_parameter() {
    let program = typed_program(
        r#"
        data Owner {}
        machine Owner::run() {
            transition { _ -> next() }
            state next(&self) {}
        }
    "#,
    );
    let machine = &program.machines()[0];
    assert!(
        program
            .state_parameters(&program.machine_states(machine)[0])
            .is_empty()
    );
    assert_eq!(
        contract_owner_self_symbol(
            &program,
            ContractProofFactOwner::Machine {
                machine_symbol: machine.symbol
            }
        ),
        None
    );
    assert!(
        contract_owner_self_symbol(
            &program,
            ContractProofFactOwner::MachineState {
                machine_symbol: machine.symbol,
                state_symbol: program.machine_states(machine)[1].symbol
            }
        )
        .is_some()
    );
}
