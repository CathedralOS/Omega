use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::CheckedUnitEffectOperationPlan;
use super::super::CheckedTrees;
use super::{CheckedComposedUnitControlMachinePlan, admission};
mod case_payload_edges;
mod closed_cases;
mod composite_arguments;
mod displaced_field_replacement;
mod local_operand_edges;
mod nested_local_construction;
mod projected_loans;
mod qualified_invocation;
mod receiver_subslice_edges;
mod service_edges;

#[test]
fn scalar_returns_rejoin_local_cleanup_evidence() {
    use language_semantics::{PermissionEventKind, PermissionEventSource};
    let checked = crate::front_end::checked_program(
        "data Token { value: i32; }
         machine make() -> Token { Token { value: 42 } }
         machine route(accepted: bool) -> bool {
             let token: Token = make();
             transition accepted {
                 true -> observe(token)
                 false -> (false)
             }
             state observe(token: Token) -> bool {
                 let comparison: Token = make();
                 token.value == comparison.value
             }
         }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "route")
        .expect("source machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("scalar completion graph");
    admission::admit(&checked, plan).expect("conditional and direct scalar cleanup");
    for state in &plan.states {
        let source = checked
            .machines()
            .iter()
            .find(|source| source.symbol == machine)
            .unwrap();
        let source = checked
            .machine_states(source)
            .iter()
            .find(|source| source.symbol == state.state)
            .unwrap();
        let local = checked
            .statement_table
            .statements(source.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                checked_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
                _ => None,
            })
            .expect("each return state owns a local");
        let (drop_handle, original) = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .find(|(_, event)| {
                event.machine_symbol == machine
                    && event.state_symbol == state.state
                    && event.root == facts::PlaceRoot::Symbol(local)
                    && event.source == PermissionEventSource::StateExit
                    && event.kind == PermissionEventKind::AffineDrop
            })
            .expect("owned cleanup at each returning state");
        for corruption in 0..3 {
            let mut changed = checked.clone();
            let permissions = &mut changed.facts.flow.ownership.permissions;
            match corruption {
                0 => permissions.get_mut(drop_handle).source = PermissionEventSource::StateEntry,
                1 => permissions.get_mut(drop_handle).root = facts::PlaceRoot::Unknown,
                2 => {
                    permissions.insert(original.clone());
                }
                _ => unreachable!(),
            }
            assert!(
                admission::admit(&changed, plan).is_err(),
                "missing, substituted or duplicate cleanup rejects: {corruption}"
            );
        }
    }
}

#[test]
fn local_field_loan_preserves_owned_home_custody() {
    use language_semantics::{PermissionAccess, PermissionEventKind};
    let checked = crate::front_end::checked_program(
        "data Root {}
         data Storage { raw: u8; }
         machine Root::route(context: &mut Root) {
             let mut storage: Storage;
             storage.raw = 1;
             let seen: &mut u8 = &mut storage.raw;
             seen = 0;
             transition storage.raw == 0 {
                 true -> good(context)
                 _ -> bad(context)
             }
             state good(context: &mut Root) {}
             state bad(context: &mut Root) {}
         }",
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.states.len() == 3)
        .expect("local loan graph");
    admission::admit(&checked, plan).expect("loan access is not owned-home disposition");
    let permissions = &checked.facts.flow.ownership.permissions;
    let (establishment, original) = permissions
        .iter()
        .find(|(_, event)| {
            event.machine_symbol == plan.machine
                && event.state_symbol == plan.states[0].state
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
        })
        .expect("owned establishment");
    let (loan, _) = permissions
        .iter()
        .find(|(_, event)| {
            event.machine_symbol == plan.machine
                && event.state_symbol == plan.states[0].state
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Exclusive
        })
        .expect("exclusive loan establishment");
    for corruption in 0..6 {
        let mut changed = checked.clone();
        let permissions = &mut changed.facts.flow.ownership.permissions;
        match corruption {
            0 => permissions.get_mut(establishment).access = PermissionAccess::Shared,
            1 => {
                permissions.insert(original.clone());
            }
            2 => permissions.get_mut(establishment).obligation_live = true,
            3 => {
                permissions.get_mut(loan).multiplicity =
                    language_semantics::Multiplicity::Unrestricted
            }
            4 => permissions.get_mut(loan).access = PermissionAccess::Shared,
            5 => {
                permissions.get_mut(loan).provenance =
                    language_semantics::PermissionProvenance::Unknown
            }
            _ => unreachable!(),
        }
        assert!(
            admission::admit(&changed, plan).is_err(),
            "ownership and exact loan custody must reject tampering: {corruption}"
        );
    }
}

#[test]
fn unreachable_alternatives_do_not_replace_the_selected_edge() {
    let checked = crate::front_end::checked_program(
        "data Root { count: u32 in Wrapping; }
         machine Root::choose(&mut self) {
             self.count = self.count + 1;
             transition { _ -> selected() _ -> skipped() }
             state selected(&mut self) { self.count = self.count + 1; }
             state skipped(&mut self) { self.count = self.count + 10; }
         }",
    );
    let original = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.states.len() == 3)
        .expect("ordered transition plan");
    admission::admit(&checked, original).expect("only the first arm can execute");
    for change_destination in [false, true] {
        let mut changed = original.clone();
        let skipped = changed.states[2].state;
        let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
            &mut changed.states[0].terminator
        else {
            panic!("unconditional edge");
        };
        if change_destination {
            successor.target_state = skipped;
        } else {
            successor.statement_ordinal += 1;
        }
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "an unreachable source arm cannot authorize a replacement edge"
        );
    }
    let mut changed = checked.clone();
    let source = changed.machine_states(&changed.machines()[0])[0].statement_nodes;
    let condition = changed
        .typed
        .expression_table
        .insert(checked_trees::expression::ExpressionNode::Boolean(false));
    let checked_trees::statement::StatementNode::Transition(transition) =
        &mut changed.typed.statement_table.statements_mut(source)[1]
    else {
        panic!("first arm");
    };
    transition.guard = checked_trees::statement::TransitionGuardNode::When(condition);
    assert!(
        admission::admit(&changed, original).is_err(),
        "a conditional arm cannot justify dropping its fallback"
    );
}

fn fixture() -> CheckedTrees {
    let source = r#"
        data Main { counter: u64 in Wrapping; total: u64 in Wrapping; }
        machine Main::main(&mut self) {
            self.counter = 0;
            self.total = 0;
            transition { _ -> work() }
            state work(&mut self) {
                transition self.counter < 4 { true -> step() _ -> done() }
            }
            state step(&mut self) {
                self.record();
                self.counter = self.counter + 1;
                transition { _ -> work() }
            }
            state done(&mut self) {}
        }
        machine Main::record(&mut self) { self.total = self.total + self.counter; }
    "#;
    crate::front_end::checked_program(source)
}

fn plan(checked: &CheckedTrees) -> CheckedComposedUnitControlMachinePlan {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.states.len() == 4)
        .expect("source retains its cyclic graph")
        .clone()
}

#[test]
fn receiver_graph_accepts_shared_state_view_without_gaining_authority() {
    let checked = crate::front_end::checked_program(
        r#"
        data Main { counter: u64; }
        machine Main::main(&mut self) {
            self.counter = 7;
            transition { _ -> observe() }
            state observe(&self) {
                let observed: u64 = self.counter;
                transition observed == 7 { true -> done() false -> done() }
            }
            state done(&self) {}
        }
    "#,
    );
    let original = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.states.len() == 3)
        .expect("a mutable invocation may continue through shared receiver states");
    admission::admit(&checked, original).expect("source-exact shared view");
    assert!(!super::parameters::receiver_transfer_matches(
        &original.states[1].structural_parameters[0],
        &original.states[0].structural_parameters[0],
    ));

    let mut changed = original.clone();
    changed.states[1].structural_parameters[0].access =
        checked_trees::CheckedStructuralAccess::MutableBorrow;
    assert!(admission::admit(&checked, &changed).is_err());
    let mut changed = original.clone();
    changed.states[1].structural_parameters[0].type_identity = "DifferentReceiver".into();
    assert!(admission::admit(&checked, &changed).is_err());

    let mut changed = original.clone();
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut changed.states[0].terminator
    else {
        panic!("entry jumps to observe");
    };
    successor.transfers[0].source =
        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 };
    assert!(admission::admit(&checked, &changed).is_err());
}

#[test]
fn receiver_graph_rejects_missing_and_reordered_effects() {
    let checked = fixture();
    let original = plan(&checked);
    admission::admit(&checked, &original).expect("unmodified control");
    for (state, operation) in [(0, 0), (2, 0), (2, 1)] {
        let mut changed = original.clone();
        changed.states[state].operations.remove(operation);
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "removed effect {state}/{operation}"
        );
    }
    for state in [0, 2] {
        let mut changed = original.clone();
        changed.states[state].operations.swap(0, 1);
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "reordered effects in state {state}"
        );
    }
}

#[test]
fn receiver_graph_rejects_stale_receiver_transfer() {
    let checked = fixture();
    let mut changed = plan(&checked);
    admission::admit(&checked, &changed).expect("unmodified control");
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut changed.states[2].terminator
    else {
        panic!("step returns to work");
    };
    successor.transfers[0].source =
        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 };
    assert!(admission::admit(&checked, &changed).is_err());
}

#[test]
fn receiver_graph_rejects_substituted_store_value() {
    let checked = fixture();
    let mut changed = plan(&checked);
    admission::admit(&checked, &changed).expect("unmodified control");
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(initial) =
        &changed.states[0].operations[0]
    else {
        panic!("initial store");
    };
    let replacement = initial.value.clone();
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) =
        &mut changed.states[2].operations[1]
    else {
        panic!("counter increment");
    };
    store.value = replacement;
    assert!(admission::admit(&checked, &changed).is_err());
}
