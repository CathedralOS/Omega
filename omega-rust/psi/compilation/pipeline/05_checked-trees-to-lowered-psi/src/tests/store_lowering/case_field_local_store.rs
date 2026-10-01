//! Case-field assignment remains in authored order among ordinary calls.

#[test]
fn case_literal_field_assignment_between_calls_retains_its_statement() {
    let checked = crate::front_end::checked_program(
        r#"
        data Mode [copy] { case Empty; case Exploring; case Counter(step: u64); }
        data Controller { mode: Mode; count: u64; }
        machine Controller::before(&mut self) { self.count = 1; }
        machine Controller::after(&mut self) { self.count = 2; }
        machine Controller::initialize(&mut self, seed: u64) {
            self.before();
            self.mode = Mode::Exploring;
            self.mode = Mode::Counter { step: seed };
            self.after();
        }
    "#,
    );
    let selection = crate::select_terminal_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Controller::initialize"),
    )
    .unwrap();
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == selection.machine)
        .expect("the case literal store composes with its surrounding calls");
    assert!(matches!(
        plan.operations.last(),
        Some(checked_trees::CheckedUnitEffectOperationPlan::Complete { .. })
    ));
    // Payload construction also emits its value and borrowed-window events;
    // they do not change the order of the two calls and committed stores.
    let statements = plan
        .operations
        .iter()
        .filter_map(|operation| {
            use checked_trees::CheckedUnitEffectOperationPlan as Operation;
            match operation {
                Operation::Call(call) => Some(call.coordinate.statement_index),
                Operation::StructuralCaseLiteralFieldStore(store) => {
                    assert_eq!(
                        operation.store_statement_index(),
                        Some(store.statement_index)
                    );
                    Some(store.statement_index)
                }
                Operation::StoreStructuralField {
                    statement_index, ..
                } => Some(*statement_index),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(statements, [0, 1, 2, 3]);
    let lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Controller::initialize"),
    )
    .expect("ordered call/store/call lowers without losing a pending store");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the store's source occurrence and custody independently replay");
}
