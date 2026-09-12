use super::*;

const ORDERED_SOURCE: &str = r#"
    data Pair { left: u16; right: u16; }
    machine observe() {}
    machine Pair::ordered(&mut self, replacement: u16) {
        self.left = 1;
        observe();
        self.right = replacement;
        observe();
        self.left = 2;
    }
"#;

fn checked() -> checked_trees::CheckedTrees {
    typed_trees_to_checked_trees::lower_typed_trees(typed_from_source(ORDERED_SOURCE)).unwrap()
}

fn plan_mut(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut checked_trees::CheckedUnitEffectMachinePlan {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Pair::ordered")
        .unwrap()
        .symbol;
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .unwrap()
}

#[test]
fn ordered_store_source_custody_rejects_omission_reordering_and_substitution() {
    let _artifact =
        terminal_production::TerminalProductionRequest::new(&checked(), "Pair::ordered")
            .produce_artifact()
            .expect("unmodified authored store sequence publishes");
    for mutation in 0..11 {
        let mut checked = checked();
        let plan = plan_mut(&mut checked);
        match mutation {
            0 => plan.operations.retain(|operation| {
                !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                )
            }),
            1 => {
                plan.operations.remove(2);
            }
            2 => {
                plan.operations.swap(0, 4);
            }
            3 => {
                let duplicate = plan.operations[2].clone();
                plan.operations.insert(2, duplicate);
            }
            4 => {
                let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) =
                    &mut plan.operations[0]
                else {
                    panic!("first store");
                };
                store.field_identity = "right".into();
            }
            5 => {
                let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(last) =
                    &plan.operations[4]
                else {
                    panic!("last store");
                };
                let replacement = last.value.clone();
                let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(first) =
                    &mut plan.operations[0]
                else {
                    panic!("first store");
                };
                first.value = replacement;
            }
            6 => {
                let CheckedUnitEffectOperationPlan::Complete {
                    statement_index, ..
                } = plan.operations.last_mut().unwrap()
                else {
                    panic!("return");
                };
                *statement_index += 1;
            }
            7 => {
                let state = plan.state;
                let plans = &mut checked.facts.values.scalar_expressions;
                let replacement = plans
                    .source_bindings
                    .iter()
                    .find(|(_, binding)| binding.state == state && binding.statement_ordinal == 4)
                    .unwrap()
                    .1
                    .expression;
                let binding = plans
                    .source_bindings
                    .iter()
                    .find(|(_, binding)| binding.state == state && binding.statement_ordinal == 0)
                    .unwrap()
                    .0;
                plans.source_bindings.get_mut(binding).expression = replacement;
            }
            8 => {
                plan.operations.remove(1);
            }
            9 => {
                let CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. } =
                    &mut plan.operations[1]
                else {
                    panic!("ordinary call");
                };
                coordinate.statement_index = 2;
            }
            10 => {
                let CheckedUnitEffectOperationPlan::CallUnit { target_state, .. } =
                    &mut plan.operations[1]
                else {
                    panic!("ordinary call");
                };
                *target_state = symbols::SymbolHandle::invalid();
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&checked, "Pair::ordered")
                .produce_artifact()
                .is_err(),
            "store custody mutation {mutation} must reject"
        );
    }
}

#[test]
fn unrelated_scalar_local_cannot_hide_an_omitted_call_between_stores() {
    let source = ORDERED_SOURCE.replace("self.left = 1;", "let unrelated: u16 = 7; self.left = 1;");
    let mut checked =
        typed_trees_to_checked_trees::lower_typed_trees(typed_from_source(&source)).unwrap();
    let _artifact = terminal_production::TerminalProductionRequest::new(&checked, "Pair::ordered")
        .produce_artifact()
        .expect("authored local and ordered stores publish together");
    let plan = plan_mut(&mut checked);
    let position = plan
        .operations
        .iter()
        .position(|operation| matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. }))
        .unwrap();
    plan.operations.remove(position);
    assert!(
        terminal_production::TerminalProductionRequest::new(&checked, "Pair::ordered")
            .produce_artifact()
            .is_err()
    );
}
