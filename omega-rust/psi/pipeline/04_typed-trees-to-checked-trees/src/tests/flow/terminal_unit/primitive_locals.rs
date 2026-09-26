use super::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
    PrimitiveType,
};
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::CheckedPrimitiveStoreDestination;

const SOURCE: &str = r#"
    machine reset(value: &mut u64) -> u64 { value = 0; 7 }
    machine enter(value: &mut u64) {
        let mut scratch: u64 = 91;
        let returned: u64 = reset(&mut scratch);
        value = scratch;
    }
"#;

#[test]
fn primitive_local_rejects_stale_scalar_facts_and_source_custody() {
    let original = checked(SOURCE);
    let caller = machine_named(&original, "enter");
    let plan = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(caller)
        .expect("local plan");
    for mutation in 0..8 {
        let mut changed = original.facts.clone();
        let expressions = &mut changed.values.scalar_expressions;
        let initializer = expressions
            .expressions
            .iter()
            .find(|expression| {
                expression.state == plan.state
                    && expression.role == CheckedScalarExpressionRole::StorageInitializer
            })
            .unwrap()
            .expression
            .clone();
        let (binding_handle, binding) = expressions
            .source_bindings
            .iter()
            .find(|(_, binding)| {
                binding.state == plan.state
                    && binding.statement_ordinal == 2
                    && binding.role == CheckedScalarExpressionRole::AssignmentValue
            })
            .unwrap();
        let binding = binding.clone();
        match mutation {
            0..=2 => {
                let assignment = expressions
                    .expressions
                    .iter_mut()
                    .find(|expression| {
                        expression.state == plan.state
                            && expression.statement_ordinal == 2
                            && expression.role == CheckedScalarExpressionRole::AssignmentValue
                    })
                    .unwrap();
                assignment.expression = match mutation {
                    0 => initializer,
                    1 => CheckedScalarExpression::Local {
                        position: 0,
                        primitive_type: PrimitiveType::U64,
                    },
                    _ => CheckedScalarExpression::StorageRead {
                        symbol: binding.destination,
                        primitive_type: PrimitiveType::U64,
                    },
                };
            }
            3 => {
                expressions
                    .source_bindings
                    .get_mut(binding_handle)
                    .expression = arena::Handle::invalid()
            }
            4 => {
                expressions
                    .source_bindings
                    .get_mut(binding_handle)
                    .destination = arena::Handle::invalid()
            }
            5 => {
                expressions.source_bindings.append(binding);
            }
            6 => {
                expressions.source_bindings.get_mut(binding_handle).symbols =
                    arena::HandleSpan::empty()
            }
            _ => {
                let initializer_handle = expressions
                    .source_bindings
                    .iter()
                    .find(|(_, binding)| {
                        binding.state == plan.state
                            && binding.role == CheckedScalarExpressionRole::StorageInitializer
                    })
                    .unwrap()
                    .0;
                expressions
                    .source_bindings
                    .get_mut(initializer_handle)
                    .destination = arena::Handle::invalid();
            }
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &original.typed,
            &changed,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(caller).is_none(),
            "source custody mutation {mutation}"
        );
    }
}

#[test]
fn primitive_local_rejects_missing_or_substituted_borrow_events() {
    let original = checked(SOURCE);
    let caller = machine_named(&original, "enter");
    let plan = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(caller)
        .expect("local plan");
    let borrow_state = original
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == caller && state.state_symbol == plan.state)
        .unwrap();
    let borrow_call = original
        .facts
        .borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .find(|call| call.statement_index == 1 && call.call_ordinal == 0)
        .unwrap();
    let access = borrow_call.accesses.start();
    let call_handle = original
        .facts
        .borrow
        .calls
        .iter()
        .find_map(|(handle, candidate)| std::ptr::eq(candidate, borrow_call).then_some(handle))
        .unwrap();
    for mutation in 0..5 {
        let mut changed = original.facts.clone();
        match mutation {
            0 => changed.borrow.calls.get_mut(call_handle).accesses = arena::HandleSpan::empty(),
            1 => {
                changed.borrow.argument_accesses.get_mut(access).kind =
                    checked_trees::BorrowAccessKind::Read
            }
            2 => changed.borrow.argument_accesses.get_mut(access).root_symbol = caller,
            3 => changed.borrow.calls.get_mut(call_handle).call_ordinal = 1,
            _ => changed.borrow.calls.get_mut(call_handle).statement_index = 0,
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &original.typed,
            &changed,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(caller).is_none(),
            "borrow mutation {mutation}"
        );
    }
}

#[test]
fn computed_primitive_assignment_keeps_exact_rhs_and_destination() {
    let original = checked(
        "machine write(output: &mut u64, choose: bool) {
        output = 7;
        output = match choose { true -> 42, false -> 9 };
    }",
    );
    let owner = machine_named(&original, "write");
    let plan = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(owner)
        .unwrap_or_else(|| {
            panic!(
                "pure and computed stores; roots={:?}; pure={:?}",
                original
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .collect::<Vec<_>>(),
                original.facts.values.scalar_expressions.expressions
            )
        });
    assert!(matches!(
        &plan.operations[0],
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            value: checked_trees::CheckedCallScalarArgument::Pure(_),
            ..
        }
    ));
    let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        path: store_path,
        statement_index: 1,
        destination: CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
        value: checked_trees::CheckedCallScalarArgument::Computation(value),
    } = &plan.operations[1]
    else {
        panic!("exact computed store");
    };
    assert!(store_path.is_empty());
    let (root_handle, _) = original
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .find(|(_, root)| root.root == *value && root.state == plan.state)
        .unwrap();
    for mutation in 0..4 {
        let mut changed = original.facts.clone();
        match mutation {
            0 => {
                changed
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(*value)
                    .authored_root = arena::Handle::invalid()
            }
            1 => {
                changed
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(*value)
                    .primitive_type = PrimitiveType::Bool
            }
            2 => {
                changed
                    .values
                    .scalar_computations
                    .roots
                    .get_mut(root_handle)
                    .machine = plan.state
            }
            _ => {
                let root = changed
                    .values
                    .scalar_computations
                    .roots
                    .get(root_handle)
                    .clone();
                changed.values.scalar_computations.roots.append(root);
            }
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &original.typed,
            &changed,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(owner).is_none(),
            "computed assignment mutation {mutation}"
        );
    }
}
