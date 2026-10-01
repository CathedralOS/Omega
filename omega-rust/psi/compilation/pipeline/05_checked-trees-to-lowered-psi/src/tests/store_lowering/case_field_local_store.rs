//! Case-field assignment remains in authored order among ordinary calls.

const COMPOSED_CASE_STORES: &str = r#"
    data Mode [copy] { case Empty; case Exploring; case Counter(step: u64); }
    data Controller { mode: Mode; other: Mode; count: u64; }
    machine Controller::before(&mut self) { self.count = 1; }
    machine Controller::after(&mut self) { self.count = 2; }
    machine Controller::initialize(&mut self, seed: u64) {
        self.before();
        self.mode = Mode::Exploring;
        self.mode = Mode::Counter { step: seed };
        transition { _ -> finish() }
        state finish(&mut self) { self.after(); }
    }
"#;

#[test]
fn composed_case_literal_field_stores_replay_across_state_edges() {
    let checked = crate::front_end::checked_program(COMPOSED_CASE_STORES);
    let lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Controller::initialize"),
    )
    .expect("case stores compose with calls and a following state transition");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("case stores retain exact construction and move/restore custody");
}

fn composed_operations(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut Vec<checked_trees::CheckedUnitEffectOperationPlan> {
    let machine = crate::select_terminal_machine(
        checked,
        crate::TerminalMachineSelection::Name("Controller::initialize"),
    )
    .unwrap()
    .machine;
    &mut checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("the authored multi-state machine retains its composed plan")
        .states[0]
        .operations
}

#[test]
fn composed_case_literal_store_rejects_substituted_destination_case_and_occurrence() {
    let source = COMPOSED_CASE_STORES.replace(
        "self.mode = Mode::Exploring;",
        "self.mode = Mode::Exploring; self.other = Mode::Empty;",
    );
    let base = crate::front_end::checked_program(&source);
    crate::lower_machine(
        &base,
        crate::TerminalMachineSelection::Name("Controller::initialize"),
    )
    .expect("the unmodified source must lower before testing substitution");
    for mutation in 0..4 {
        let mut changed = base.clone();
        let mut stores = composed_operations(&mut changed)
            .iter_mut()
            .filter_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(
                    store,
                ) => Some(store),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [first, second] = stores.as_mut_slice() else {
            panic!("two payload-free literal assignments retain distinct stores");
        };
        match mutation {
            0 => first.field_identity = second.field_identity.clone(),
            1 => first.case_identity = second.case_identity.clone(),
            2 => first.literal = second.literal,
            3 => first.statement_index = second.statement_index,
            _ => unreachable!(),
        }
        crate::lower_machine(
            &changed,
            crate::TerminalMachineSelection::Name("Controller::initialize"),
        )
        .expect_err("a type-correct different field, case or occurrence supplies no authority");
    }
}

const COMPOSED_PARAMETER_STORE: &str = r#"
    data Mode [copy] { case Empty; case Exploring; }
    data Controller { mode: Mode; other: Mode; }
    machine Controller::initialize(&mut self, selected: Mode, alternate: Mode) {
        self.mode = selected;
        transition { _ -> finish() }
        state finish(&mut self) {}
    }
"#;

#[test]
fn composed_case_stores_preserve_owned_local_and_projected_parameter_roots() {
    let local = r#"
        data Mode [copy] { case Empty; case Exploring; }
        data Controller { mode: Mode; }
        machine Controller::initialize(selected: Mode) {
            let local: Controller;
            local.mode = Mode::Exploring;
            local.mode = selected;
            transition { _ -> finish(local) }
            state finish(local: Controller) {}
        }
    "#;
    let projected = COMPOSED_PARAMETER_STORE
        .replace(
            "data Controller",
            "data Palette [copy] { mode: Mode; } data Controller",
        )
        .replace(
            "selected: Mode, alternate: Mode",
            "selected: &Palette, alternate: &Palette",
        )
        .replace("self.mode = selected;", "self.mode = selected.mode;");
    for source in [local, projected.as_str()] {
        let lowered = crate::lower_machine(
            &crate::front_end::checked_program(source),
            crate::TerminalMachineSelection::Name("Controller::initialize"),
        )
        .expect("case stores compose for owned locals and shared parameter projections");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("live roots, exact paths and restoration independently replay");
    }
}

#[test]
fn ordinary_case_literal_store_rejects_retargeting_and_orphan_occurrences() {
    let source = COMPOSED_CASE_STORES
        .replace("transition { _ -> finish() }", "")
        .replace("state finish(&mut self) { self.after(); }", "self.after();");
    let base = crate::front_end::checked_program(&source);
    crate::lower_machine(
        &base,
        crate::TerminalMachineSelection::Name("Controller::initialize"),
    )
    .expect("ordinary case replacement must lower before mutation");
    for mutation in 0..3 {
        let mut changed = base.clone();
        let machine = crate::select_terminal_machine(
            &changed,
            crate::TerminalMachineSelection::Name("Controller::initialize"),
        )
        .unwrap()
        .machine;
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == machine)
            .unwrap();
        let store = plan
            .operations
            .iter_mut()
            .find_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(
                    store,
                ) => Some(store),
                _ => None,
            })
            .unwrap();
        match mutation {
            0 => store.field_identity = "other".into(),
            1 => store.statement_index = u32::MAX,
            2 => store.statement_index = 0,
            _ => unreachable!(),
        }
        crate::lower_machine(
            &changed,
            crate::TerminalMachineSelection::Name("Controller::initialize"),
        )
        .expect_err("an ordinary case store must name its exact authored assignment");
    }
}

#[test]
fn composed_case_parameter_store_replays_exact_source_and_destination() {
    let base = crate::front_end::checked_program(COMPOSED_PARAMETER_STORE);
    let lowered = crate::lower_machine(
        &base,
        crate::TerminalMachineSelection::Name("Controller::initialize"),
    )
    .expect("a copied case parameter composes with state transitions");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the parameter copy repairs exactly the selected destination");
    for change_source in [false, true] {
        let mut changed = base.clone();
        let store = composed_operations(&mut changed)
            .iter_mut()
            .find_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(store) => {
                    Some(store)
                }
                _ => None,
            })
            .expect("the case-parameter assignment retains its store");
        if change_source {
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } = &mut store.value.source
            else {
                panic!("authored value is a parameter");
            };
            *parameter_index += 1;
        } else {
            assert_eq!(store.field_identity, "mode");
            store.field_identity = "other".into();
        }
        crate::lower_machine(
            &changed,
            crate::TerminalMachineSelection::Name("Controller::initialize"),
        )
        .expect_err("same-typed parameter and destination substitutions must reject");
    }
}

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
