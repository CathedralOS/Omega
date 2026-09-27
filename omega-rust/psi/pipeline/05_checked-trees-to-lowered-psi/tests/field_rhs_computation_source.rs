//! Call-bearing receiver assignments through source-produced Terminal artifacts.

use checked_trees_to_lowered_psi::TerminalMachineSelection;

#[test]
fn field_rhs_computation_custody_rejects_destination_root_and_call_substitution() {
    use checked_trees::{
        CheckedScalarComputationKind, CheckedScalarExpressionRole,
        CheckedStructuralScalarFieldStoreValue, CheckedUnitEffectOperationPlan,
    };
    let checked = crate::front_end::checked_program(
        r#"
        data Main { value: u16; other: u16; }
        machine identity(value: u16) -> u16 { value }
        machine Main::main(&mut self) {
            self.value = 17;
            self.other = 19;
            self.value = identity(self.value);
            self.other = identity(self.other);
        }
    "#,
    );
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("unmodified source computation custody is valid");
    for mutation in 0..4 {
        let mut changed = checked.clone();
        let stores: Vec<_> = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .flat_map(|machine| &machine.operations)
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                    match store.value {
                        CheckedStructuralScalarFieldStoreValue::Computation(handle) => Some(handle),
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect();
        assert_eq!(stores.len(), 2);
        match mutation {
            0 | 1 => {
                let store = changed.facts.flow.terminal_unit_effects.machines.iter_mut()
                    .flat_map(|machine| &mut machine.operations)
                    .find_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store)
                            if matches!(store.value, CheckedStructuralScalarFieldStoreValue::Computation(handle) if handle == stores[0]) => Some(store),
                        _ => None,
                    }).unwrap();
                if mutation == 0 {
                    store.field_identity = "other".into();
                } else {
                    store.value = CheckedStructuralScalarFieldStoreValue::Computation(stores[1]);
                }
            }
            2 => {
                let plans = &mut changed.facts.values.scalar_computations;
                let root = plans
                    .roots
                    .iter()
                    .find(|(_, root)| root.root == stores[0])
                    .unwrap()
                    .0;
                plans.roots.get_mut(root).role = CheckedScalarExpressionRole::Return;
            }
            3 => {
                let node = changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(stores[0]);
                let CheckedScalarComputationKind::Call { call_ordinal, .. } = &mut node.kind else {
                    panic!("direct call is the assignment computation root")
                };
                *call_ordinal += 1;
            }
            _ => unreachable!(),
        }
        let result = checked_trees_to_lowered_psi::lower_machine(
            &changed,
            TerminalMachineSelection::Name("Main::main"),
        )
        .map(|_| ());
        assert!(
            result.is_err(),
            "field RHS custody mutation {mutation} must reject: {result:?}"
        );
    }
}
