//! Carrier declarations erase bindings, never initializer effects or producers.

use checked_trees::{BorrowAccessKind, CheckedTrees, CheckedUnitEffectOperationPlan};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

const RETURNED_WINDOW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tests/omega/pass/borrows/borrow_returned_window_write_and_call/main.omg"
));

fn publish(checked: &CheckedTrees) -> Result<(), String> {
    terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name("write_outside_window"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .map(|_| ())
    .map_err(|error| format!("{error:?}"))
}

#[test]
fn returned_window_carrier_declarations_replay_exact_loan_custody() {
    let original = crate::front_end::checked_program(RETURNED_WINDOW);
    publish(&original).expect("the returned window accounts for its erased outside alias");
    let (loan_handle, _) = original
        .facts
        .borrow
        .loans
        .iter()
        .find(|(_, loan)| original.symbols.name(loan.owner_symbol) == "outside")
        .expect("the outside carrier has a loan");
    for mutation in 0..4 {
        let mut changed = original.clone();
        let loan = changed.facts.borrow.loans.get_mut(loan_handle);
        match mutation {
            0 => loan.root_symbol = symbols::SymbolHandle::invalid(),
            1 => loan.kind = BorrowAccessKind::Read,
            2 => loan.statement_index += 1,
            3 => loan.segments = arena::HandleSpan::empty(),
            _ => unreachable!(),
        }
        assert!(
            publish(&changed).is_err(),
            "changed carrier custody {mutation}"
        );
    }
}

#[test]
fn returned_window_carrier_coverage_keeps_initializer_calls_and_view_establishments() {
    let original = crate::front_end::checked_program(RETURNED_WINDOW);
    publish(&original).expect("the original returned-window body publishes");
    for remove_call in [true, false] {
        let mut changed = original.clone();
        let machine = changed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "write_outside_window")
            .expect("window machine")
            .symbol;
        let operations = &mut changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == machine)
            .expect("window operation plan")
            .operations;
        let before = operations.len();
        operations.retain(|operation| {
            if remove_call {
                !operation
                    .call()
                    .is_some_and(|call| call.coordinate.statement_index == 0)
            } else {
                !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                )
            }
        });
        assert!(
            operations.len() < before,
            "the mutation must remove a real producer"
        );
        assert!(
            publish(&changed).is_err(),
            "a carrier cannot cover the deleted producer"
        );
    }
}
