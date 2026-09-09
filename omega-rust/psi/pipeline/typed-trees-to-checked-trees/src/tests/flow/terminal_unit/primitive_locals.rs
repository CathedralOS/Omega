use super::*;
use checked_trees::{CheckedPrimitiveStoreDestination, CheckedUnitStructuralArgumentSourcePlan};

const SOURCE: &str = r#"
    machine reset(value: &mut u64) -> u64 { value = 0; 7 }
    machine enter(value: &mut u64) {
        let mut scratch: u64 = 91;
        let returned: u64 = reset(&mut scratch);
        value = scratch;
    }
"#;

#[test]
fn primitive_local_borrow_and_later_read_keep_the_authored_storage() {
    let mut checked = checked(SOURCE);
    let caller = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(caller)
        .expect("initialized local, borrowed call, and current storage read")
        .clone();
    let [
        CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            statement_index: 0,
            symbol,
            type_identity,
            primitive_type: PrimitiveType::U64,
            value: CheckedScalarExpression::IntegerLiteral { literal },
        },
        CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            result,
            structural_arguments,
            scalar_arguments,
            claim_transfers,
            ..
        },
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index: 2,
            destination: CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
            value:
                CheckedScalarExpression::StorageRead {
                    symbol: read_symbol,
                    primitive_type: PrimitiveType::U64,
                },
        },
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 3, ..
        },
    ] = plan.operations.as_slice()
    else {
        panic!(
            "ordered primitive storage operations: {:?}",
            plan.operations
        );
    };
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller)
        .unwrap();
    let state = checked
        .typed
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == plan.state)
        .unwrap();
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    let typed_trees::statement::StatementNode::LocalData(local) = &statements[0] else {
        panic!("local");
    };
    let typed_trees::statement::StatementNode::LocalData(returned) = &statements[1] else {
        panic!("returned local");
    };
    assert_eq!(*symbol, local.symbol);
    assert_eq!(*read_symbol, local.symbol);
    assert_eq!(literal.value_i64(), Some(91));
    assert!(checked.facts.flow.control.calls.iter().any(|(_, call)| {
        call.statement_index == 1
            && call.call_ordinal == 0
            && call.authored_expression == returned.initial_value
    }));
    assert_eq!(
        (coordinate.statement_index, coordinate.call_ordinal),
        (1, 0)
    );
    assert_eq!((result.statement_index, result.binding_ordinal), (1, 0));
    assert!(scalar_arguments.is_empty());
    assert!(claim_transfers.is_empty());
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(
        structural_arguments[0].source,
        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
            symbol: local.symbol
        }
    );
    assert_eq!(
        structural_arguments[0].access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert_eq!(&structural_arguments[0].type_identity, type_identity);
    assert!(structural_arguments[0].path.is_empty());
    assert_eq!(plan.structural_parameters.len(), 1);
    assert!(plan.trivial_affine_locals.is_empty());
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .any(|shape| {
                &shape.identity == type_identity
                    && shape.shape
                        == CheckedUnitStructuralTypeShape::PrimitiveScalar(PrimitiveType::U64)
            })
    );
    crate::rebuild_checked_terminal_plans_with_selected_execution(&mut checked, &[], &[])
        .expect("selected rebuild");
    assert_eq!(
        checked.facts.flow.terminal_unit_effects.for_machine(caller),
        Some(&plan)
    );
    crate::rebuild_checked_unit_effect_plans_with_selected_execution(&mut checked, &[], &[]);
    assert_eq!(
        checked.facts.flow.terminal_unit_effects.for_machine(caller),
        Some(&plan)
    );
}

#[test]
fn primitive_local_mutations_preserve_input_snapshot_and_returned_binding_namespaces() {
    let checked = checked(
        r#"
        machine reset(value: &write u64, result_value: u64) -> u64 { value = 0; result_value }
        machine enter(value: &mut u64, seed: u64) {
            let mut scratch: u64 = seed;
            let snapshot: u64 = scratch;
            let returned: u64 = reset(&write scratch, seed);
            scratch = returned;
            value = scratch;
            value = snapshot;
            value = returned;
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("storage interleaved with scalar inputs and immutable bindings");
    let [
        CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            symbol,
            value:
                CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type: PrimitiveType::U64,
                },
            ..
        },
        CheckedUnitEffectOperationPlan::EstablishScalarLocal {
            result: snapshot,
            value:
                CheckedScalarExpression::StorageRead {
                    symbol: snapshot_symbol,
                    primitive_type: PrimitiveType::U64,
                },
        },
        CheckedUnitEffectOperationPlan::ScalarCall {
            result: returned,
            structural_arguments,
            scalar_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index: 3,
            destination:
                CheckedPrimitiveStoreDestination::Local {
                    symbol: written_symbol,
                },
            value:
                CheckedScalarExpression::Local {
                    position: 2,
                    primitive_type: PrimitiveType::U64,
                },
        },
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index: 4,
            value:
                CheckedScalarExpression::StorageRead {
                    symbol: read_symbol,
                    primitive_type: PrimitiveType::U64,
                },
            ..
        },
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index: 5,
            value:
                CheckedScalarExpression::Local {
                    position: 1,
                    primitive_type: PrimitiveType::U64,
                },
            ..
        },
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index: 6,
            value:
                CheckedScalarExpression::Local {
                    position: 2,
                    primitive_type: PrimitiveType::U64,
                },
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("separate namespaces: {:?}", plan.operations);
    };
    assert_eq!(
        (*symbol, *symbol, *symbol),
        (*snapshot_symbol, *written_symbol, *read_symbol)
    );
    assert_eq!((snapshot.statement_index, snapshot.binding_ordinal), (1, 0));
    assert_eq!((returned.statement_index, returned.binding_ordinal), (2, 1));
    assert_eq!(
        structural_arguments[0].access,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert!(matches!(
        scalar_arguments.as_slice(),
        [checked_trees::CheckedCallScalarArgument::Pure(
            CheckedScalarExpression::Parameter { position: 0, .. }
        )]
    ));
}

#[test]
fn primitive_local_boolean_storage_reads_and_writes_use_the_same_symbol() {
    let checked = checked(
        r#"
        machine enter(value: &mut bool) {
            let mut scratch: bool = true;
            scratch = false;
            value = scratch;
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("Boolean primitive storage");
    let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { symbol, .. } =
        &plan.operations[0]
    else {
        panic!("storage");
    };
    let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        destination: CheckedPrimitiveStoreDestination::Local { symbol: written },
        ..
    } = &plan.operations[1]
    else {
        panic!("local store");
    };
    let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        value: CheckedScalarExpression::Boolean(expression),
        ..
    } = &plan.operations[2]
    else {
        panic!("Boolean read");
    };
    assert_eq!(symbol, written);
    assert_eq!(
        expression.as_ref(),
        &CheckedBooleanExpression::StorageRead { symbol: *symbol }
    );
}

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
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&original.typed, &changed, &[], &[]);
        assert!(
            rebuilt.for_machine(caller).is_none(),
            "source custody mutation {mutation}"
        );
    }
}

#[test]
fn primitive_local_store_sequence_without_calls_retains_parameter_store() {
    let checked = checked(
        r#"
        machine enter(value: &mut u64) {
            let mut scratch: u64 = 91;
            scratch = 0;
            value = scratch;
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("ordinary primitive stores do not require a call");
    let [
        CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { symbol, .. },
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index: 1,
            destination: CheckedPrimitiveStoreDestination::Local { symbol: written },
            ..
        },
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index: 2,
            destination: CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
            value:
                CheckedScalarExpression::StorageRead {
                    symbol: read,
                    primitive_type: PrimitiveType::U64,
                },
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("ordinary store sequence: {:?}", plan.operations);
    };
    assert_eq!((*symbol, *symbol), (*written, *read));
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
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&original.typed, &changed, &[], &[]);
        assert!(
            rebuilt.for_machine(caller).is_none(),
            "borrow mutation {mutation}"
        );
    }
}

#[test]
fn primitive_local_returned_binding_rejects_input_or_storage_namespace_substitution() {
    let original = checked(
        r#"
        machine reset(value: &mut u64) -> u64 { value = 0; 7 }
        machine enter(value: &mut u64, seed: u64) {
            let mut scratch: u64 = seed;
            let returned: u64 = reset(&mut scratch);
            value = returned;
        }
    "#,
    );
    let caller = machine_named(&original, "enter");
    let plan = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(caller)
        .expect("result after storage and scalar input");
    let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { symbol, .. } = plan.operations[0]
    else {
        panic!("storage");
    };
    for substituted in [
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U64,
        },
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::U64,
        },
        CheckedScalarExpression::Local {
            position: 2,
            primitive_type: PrimitiveType::U64,
        },
        CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type: PrimitiveType::U64,
        },
    ] {
        let mut changed = original.facts.clone();
        let assignment = changed
            .values
            .scalar_expressions
            .expressions
            .iter_mut()
            .find(|expression| {
                expression.state == plan.state
                    && expression.statement_ordinal == 2
                    && expression.role == CheckedScalarExpressionRole::AssignmentValue
            })
            .unwrap();
        assignment.expression = substituted.clone();
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&original.typed, &changed, &[], &[]);
        assert!(
            rebuilt.for_machine(caller).is_none(),
            "returned binding replaced with {substituted:?}"
        );
    }
}
