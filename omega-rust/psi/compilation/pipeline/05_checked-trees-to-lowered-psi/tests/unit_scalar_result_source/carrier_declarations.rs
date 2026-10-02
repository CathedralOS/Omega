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
        let position = operations
            .iter()
            .position(|operation| {
                if remove_call {
                    // choose(seed) is the retained computation of the first
                    // immutable local, not a top-level Unit Call operation.
                    matches!(operation,
                        CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                            result,
                            value: checked_trees::CheckedCallScalarArgument::Computation(_),
                        } if result.statement_index == 0
                    )
                } else {
                    matches!(operation,
                        CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. }
                            if result.statement_index == 1
                    )
                }
            })
            .expect("the exact initializer or view producer is retained");
        operations.remove(position);
        assert!(
            publish(&changed).is_err(),
            "a carrier cannot cover the deleted producer"
        );
    }
}

#[test]
fn returned_window_alias_lifetime_does_not_include_unrelated_completion_values() {
    // Different final expressions share the same alias lifetime: the outside
    // carrier was last used by its store, not by completion evaluation.
    for tail in ["held.len", "2", "held.len + 0"] {
        let source = RETURNED_WINDOW.replace("    held.len\n", &format!("    {tail}\n"));
        let original = crate::front_end::checked_program(&source);
        publish(&original).unwrap_or_else(|error| panic!("{tail}: {error}"));
        let (handle, last_use) = original
            .facts
            .borrow
            .loans
            .iter()
            .find(|(_, loan)| original.symbols.name(loan.owner_symbol) == "outside")
            .map(|(handle, loan)| (handle, loan.last_use_statement_index))
            .expect("outside loan");
        let mut changed = original.clone();
        changed
            .facts
            .borrow
            .loans
            .get_mut(handle)
            .last_use_statement_index = last_use + 1;
        assert!(
            publish(&changed).is_err(),
            "unrelated completion cannot extend the alias lifetime"
        );
    }
}
