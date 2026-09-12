//! Ordinary callers preserve borrowed primitive effects and scalar results.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, StructuralAccess};
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

// Catalog order includes scalar callees. Resolve the authored fixture owner once,
// then join its exact symbol to the retained body being corrupted.
fn ordinary_body_index(checked: &checked_trees::CheckedTrees, name: &str) -> usize {
    let mut machines = checked
        .machines()
        .iter()
        .filter(|machine| machine.name.as_str() == name);
    let symbol = machines.next().expect("authored fixture machine").symbol;
    assert!(machines.next().is_none());
    let mut plans = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .enumerate()
        .filter(|(_, plan)| plan.machine == symbol);
    let index = plans.next().expect("exact ordinary body").0;
    assert!(plans.next().is_none());
    index
}

const SOURCE: &str = r#"
    machine reset(value: &mut u64) -> u64 { value = 0; 7 }
    machine enter(value: &mut u64) {
        let returned: u64 = reset(&mut value);
        value = returned;
    }
"#;

#[test]
fn borrowed_primitive_local_read_observes_the_callee_write() {
    let checked = checked(
        r#"
        machine reset(value: &mut u64) -> u64 { value = 0; 7 }
        machine enter(value: &mut u64) {
            let mut scratch: u64 = 91;
            let returned: u64 = reset(&mut scratch);
            value = scratch;
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .expect("borrowing local storage must preserve the callee's write for a later read");
    execute_with_expectations(
        &artifact,
        &[],
        ExecutionExpectations {
            calls: 1,
            store_sites: 2,
            callee_store_executions: 1,
            primitive_locals: 1,
            primitive_reads: 1,
            observations: &[91, 0],
        },
    );
}

#[test]
fn borrowed_scalar_callee_and_returned_value_reach_the_callers_closure() {
    let checked = checked(SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .expect("borrowed scalar callee belongs to the ordinary shared call closure");
    execute(&artifact, &[], 7);
}

#[test]
fn immutable_snapshot_precedes_the_call_and_fresh_local_read_observes_zero() {
    let checked = checked(
        r#"
        machine reset(value: &mut u64) -> u64 { value = 0; 7 }
        machine enter(value: &mut u64) {
            let mut scratch: u64 = 41;
            let before: u64 = scratch;
            let returned: u64 = reset(&mut scratch);
            value = before;
            value = scratch;
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .unwrap();
    execute_with_expectations(
        &artifact,
        &[],
        ExecutionExpectations {
            calls: 1,
            store_sites: 3,
            callee_store_executions: 1,
            primitive_locals: 1,
            primitive_reads: 2,
            observations: &[91, 41, 0],
        },
    );
}

#[test]
fn local_overwrite_commits_the_returned_scalar_before_a_fresh_read() {
    let checked = checked(
        r#"
        machine reset(value: &mut u64) -> u64 { value = 0; 7 }
        machine enter(value: &mut u64) {
            let mut scratch: u64 = 41;
            let returned: u64 = reset(&mut scratch);
            scratch = returned;
            value = scratch;
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .unwrap();
    execute_with_expectations(
        &artifact,
        &[],
        ExecutionExpectations {
            calls: 1,
            store_sites: 3,
            callee_store_executions: 1,
            primitive_locals: 1,
            primitive_reads: 1,
            observations: &[91, 7],
        },
    );
}

#[test]
fn repeated_calls_keep_distinct_local_referents_and_charge_each_invocation() {
    let checked = checked(
        r#"
        machine reset(value: &mut u64) -> u64 { value = 0; 7 }
        machine enter(value: &mut u64) {
            let mut first: u64 = 41;
            let mut second: u64 = 53;
            let first_returned: u64 = reset(&mut first);
            value = second;
            let second_returned: u64 = reset(&mut second);
            value = first;
            first = first_returned;
            value = first;
            value = second;
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(
        module.machines.len(),
        2,
        "repeated calls share one callee body"
    );
    execute_with_expectations(
        &artifact,
        &[],
        ExecutionExpectations {
            calls: 2,
            store_sites: 6,
            callee_store_executions: 2,
            primitive_locals: 2,
            primitive_reads: 4,
            observations: &[91, 53, 0, 7, 0],
        },
    );
}

#[test]
fn unused_primitive_local_still_establishes_once_and_cannot_be_removed_or_duplicated() {
    let original =
        checked("machine enter(value: &mut u64) { let mut unused: u64 = 13; value = 7; }");
    let artifact = terminal_production::TerminalProductionRequest::new(&original, "enter")
        .produce_artifact()
        .unwrap();
    execute_with_expectations(
        &artifact,
        &[],
        ExecutionExpectations {
            calls: 0,
            store_sites: 1,
            callee_store_executions: 0,
            primitive_locals: 1,
            primitive_reads: 0,
            observations: &[91, 7],
        },
    );
    for duplicate in [false, true] {
        let mut changed = original.clone();
        let caller_index = ordinary_body_index(&changed, "enter");
        let operations =
            &mut changed.facts.flow.terminal_unit_effects.machines[caller_index].operations;
        assert!(matches!(
            operations[0],
            checked_trees::CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
        ));
        if duplicate {
            operations.insert(0, operations[0].clone());
        } else {
            operations.remove(0);
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "enter")
                .produce_artifact()
                .is_err(),
            "unused establishment roster mutation, duplicate={duplicate}"
        );
    }
}

const TWO_LOCAL_SOURCE: &str = r#"
    machine reset(value: &mut u64) -> u64 { value = 0; 7 }
    machine enter(value: &mut u64) {
        let mut first: u64 = 41;
        let mut second: u64 = 53;
        let returned: u64 = reset(&mut first);
        value = first;
    }
"#;

#[test]
fn primitive_local_initializer_cannot_move_after_its_borrow_or_use_another_symbol() {
    let original = checked(TWO_LOCAL_SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(&original, "enter")
        .produce_artifact()
        .unwrap();
    execute_with_expectations(
        &artifact,
        &[],
        ExecutionExpectations {
            calls: 1,
            store_sites: 2,
            callee_store_executions: 1,
            primitive_locals: 2,
            primitive_reads: 1,
            observations: &[91, 0],
        },
    );
    for mutation in 0..4 {
        let mut changed = original.clone();
        let caller_index = ordinary_body_index(&changed, "enter");
        let operations =
            &mut changed.facts.flow.terminal_unit_effects.machines[caller_index].operations;
        let checked_trees::CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            symbol: second,
            ..
        } = operations[1]
        else {
            panic!("second primitive declaration");
        };
        match mutation {
            0 => {
                let initializer = operations.remove(0);
                operations.insert(2, initializer);
            }
            1 => {
                let checked_trees::CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                    symbol,
                    ..
                } = &mut operations[0]
                else {
                    panic!("first primitive declaration");
                };
                *symbol = second;
            }
            2 => {
                let checked_trees::CheckedUnitEffectOperationPlan::ScalarCall {
                    structural_arguments,
                    ..
                } = &mut operations[2]
                else {
                    panic!("borrowed scalar call");
                };
                structural_arguments[0].source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                        symbol: second,
                    };
            }
            3 => {
                let checked_trees::CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                    statement_index,
                    ..
                } = &mut operations[0]
                else {
                    panic!("first primitive declaration");
                };
                *statement_index = 2;
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "enter")
                .produce_artifact()
                .is_err(),
            "primitive declaration/borrow custody mutation {mutation}"
        );
    }
}

#[test]
fn primitive_storage_read_cannot_substitute_another_symbol_initializer_or_scalar_binding() {
    let original = checked(TWO_LOCAL_SOURCE);
    let _ = terminal_production::TerminalProductionRequest::new(&original, "enter")
        .produce_artifact()
        .expect("original storage read");
    let caller_index = ordinary_body_index(&original, "enter");
    let caller_state = original.facts.flow.terminal_unit_effects.machines[caller_index].state;
    for (mutation, synchronize_expression) in
        (0..5).flat_map(|mutation| [false, true].map(|synchronize| (mutation, synchronize)))
    {
        let mut changed = original.clone();
        let caller_index = ordinary_body_index(&changed, "enter");
        let operations =
            &mut changed.facts.flow.terminal_unit_effects.machines[caller_index].operations;
        let checked_trees::CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            symbol: second,
            ..
        } = operations[1]
        else {
            panic!("second primitive declaration");
        };
        let checked_trees::CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            value: initializer,
            ..
        } = &operations[0]
        else {
            panic!("first primitive initializer");
        };
        let initializer = initializer.clone();
        let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            value, ..
        } = &mut operations[3]
        else {
            panic!("caller read and store");
        };
        let checked_trees::CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type,
        } = value
        else {
            panic!("authored read retains storage identity");
        };
        *value = match mutation {
            0 => checked_trees::CheckedScalarExpression::StorageRead {
                symbol: symbols::SymbolHandle::invalid(),
                primitive_type: *primitive_type,
            },
            1 => checked_trees::CheckedScalarExpression::StorageRead {
                symbol: second,
                primitive_type: *primitive_type,
            },
            2 => initializer,
            3 => checked_trees::CheckedScalarExpression::Local {
                position: 0,
                primitive_type: *primitive_type,
            },
            4 => checked_trees::CheckedScalarExpression::StorageRead {
                symbol: symbols::SymbolHandle::from_parts(
                    symbol.arena_index(),
                    symbol.generation() + 1,
                ),
                primitive_type: *primitive_type,
            },
            _ => unreachable!(),
        };
        let replacement = value.clone();
        if synchronize_expression {
            let mut expressions = changed
                .facts
                .values
                .scalar_expressions
                .expressions
                .iter_mut()
                .filter(|expression| {
                    expression.state == caller_state
                        && expression.statement_ordinal == 3
                        && expression.role
                            == checked_trees::CheckedScalarExpressionRole::AssignmentValue
                });
            let expression = expressions
                .next()
                .expect("exact authored assignment expression");
            assert!(expressions.next().is_none());
            assert!(matches!(
                expression.expression,
                checked_trees::CheckedScalarExpression::StorageRead { .. }
            ));
            expression.expression = replacement;
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "enter")
                .produce_artifact()
                .is_err(),
            "storage read custody mutation {mutation}, synchronized expression={synchronize_expression}"
        );
    }
}

#[test]
fn primitive_local_plan_cannot_grant_mutability_to_an_immutable_authored_binding() {
    let mut changed = checked(TWO_LOCAL_SOURCE);
    let _ = terminal_production::TerminalProductionRequest::new(&changed, "enter")
        .produce_artifact()
        .expect("original mutable local");
    let caller_index = ordinary_body_index(&changed, "enter");
    let state_symbol = changed.facts.flow.terminal_unit_effects.machines[caller_index].state;
    let statements = changed
        .machines()
        .iter()
        .flat_map(|machine| changed.machine_states(machine))
        .find(|state| state.symbol == state_symbol)
        .unwrap()
        .statement_nodes;
    let checked_trees::statement::StatementNode::LocalData(local) =
        &mut changed.typed.statement_table.statements_mut(statements)[0]
    else {
        panic!("first authored local");
    };
    assert!(local.is_mutable);
    local.is_mutable = false;
    assert!(
        terminal_production::TerminalProductionRequest::new(&changed, "enter")
            .produce_artifact()
            .is_err()
    );
}

#[test]
fn pure_call_argument_replays_its_authored_local_even_when_cached_and_plan_reads_agree() {
    let original = checked(
        r#"
        machine consume(value: u64) -> u64 { value }
        machine enter(value: &mut u64) {
            let mut first: u64 = 41;
            let mut second: u64 = 53;
            let returned: u64 = consume(first);
            value = returned;
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&original, "enter")
        .produce_artifact()
        .unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(
        caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
            .count(),
        1
    );
    execute_with_expectations(
        &artifact,
        &[],
        ExecutionExpectations {
            calls: 0,
            store_sites: 1,
            callee_store_executions: 0,
            primitive_locals: 2,
            primitive_reads: 1,
            observations: &[91, 41],
        },
    );

    for synchronize_expression in [false, true] {
        let mut changed = original.clone();
        let caller_index = ordinary_body_index(&changed, "enter");
        let plan = &mut changed.facts.flow.terminal_unit_effects.machines[caller_index];
        let caller_state = plan.state;
        let locals = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                    symbol,
                    ..
                } => Some(*symbol),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(locals.len(), 2);
        assert_ne!(locals[0], locals[1]);
        let call = plan
            .operations
            .iter_mut()
            .find(|operation| {
                matches!(
                    operation,
                    checked_trees::CheckedUnitEffectOperationPlan::ScalarCall { .. }
                )
            })
            .unwrap();
        let checked_trees::CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            scalar_arguments,
            ..
        } = call
        else {
            panic!("consume scalar call");
        };
        let coordinate = *coordinate;
        let [checked_trees::CheckedCallScalarArgument::Pure(expression)] =
            scalar_arguments.as_mut_slice()
        else {
            panic!("one pure local read argument");
        };
        let checked_trees::CheckedScalarExpression::StorageRead { symbol, .. } = expression else {
            panic!("consume reads current primitive storage");
        };
        assert_eq!(*symbol, locals[0]);
        *symbol = locals[1];
        let substituted = expression.clone();
        if synchronize_expression {
            let mut cached = changed
                .facts
                .values
                .scalar_expressions
                .expressions
                .iter_mut()
                .filter(|row| {
                    row.state == caller_state
                        && row.statement_ordinal == coordinate.statement_index
                        && row.role
                            == checked_trees::CheckedScalarExpressionRole::UnitCallArgument {
                                call_ordinal: coordinate.call_ordinal,
                                argument_ordinal: 0,
                            }
                });
            let cached_row = cached.next().expect("source-bound consume argument");
            assert!(cached.next().is_none());
            assert!(matches!(cached_row.expression,
                checked_trees::CheckedScalarExpression::StorageRead { symbol, .. } if symbol == locals[0]));
            cached_row.expression = substituted;
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "enter")
                .produce_artifact()
                .is_err(),
            "same-typed consume argument substitution, synchronized expression={synchronize_expression}"
        );
    }
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn execute(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    arguments: &[TerminalScalarValue],
    expected: u128,
) {
    execute_with_expectations(
        artifact,
        arguments,
        ExecutionExpectations {
            calls: 1,
            store_sites: 2,
            callee_store_executions: 1,
            primitive_locals: 0,
            primitive_reads: 0,
            observations: &[91, 0, expected],
        },
    );
}

struct ExecutionExpectations<'values> {
    calls: usize,
    store_sites: usize,
    callee_store_executions: u64,
    primitive_locals: usize,
    primitive_reads: usize,
    observations: &'values [u128],
}

fn execute_with_expectations(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    arguments: &[TerminalScalarValue],
    expected: ExecutionExpectations<'_>,
) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = &caller.structural_parameters[0];
    assert_eq!(parameter.access, StructuralAccess::MutableBorrow);
    let calls = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::CallStructuralScalar { .. }))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), expected.calls);
    let stores = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .map(move |operation| (machine.id, operation))
        })
        .filter(|(_, operation)| {
            matches!(
                operation.kind,
                OperationKind::WriteOnlyPrimitiveStore { .. }
            )
        })
        .map(|(machine, operation)| (machine, operation.id))
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), expected.store_sites);
    let locals = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishPrimitiveLocal { .. }
            )
        })
        .collect::<Vec<_>>();
    let reads = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::PrimitiveScalarRead { .. }))
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), expected.primitive_locals);
    assert_eq!(reads.len(), expected.primitive_reads);
    let local_places = locals
        .iter()
        .map(|operation| operation.result.structural().unwrap().place)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        local_places.len(),
        locals.len(),
        "each local has its own referent"
    );
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            arguments,
            &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: unsigned(91),
            }],
        )
        .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut complete = false;
    for _ in 0..32 {
        let status = execution.resume(&mut meter).unwrap();
        observations.push(execution.structural_primitive_values()[0].value);
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected status {other:?}"),
        }
    }
    assert!(complete);
    observations.dedup();
    let mut expected_observations = expected
        .observations
        .iter()
        .copied()
        .map(unsigned)
        .collect::<Vec<_>>();
    expected_observations.dedup();
    assert_eq!(
        observations, expected_observations,
        "ordered caller-visible writes"
    );
    for (operation, executions) in stores
        .into_iter()
        .map(|(machine, operation)| {
            (
                operation,
                if machine == caller.id {
                    1
                } else {
                    expected.callee_store_executions
                },
            )
        })
        .chain(
            calls
                .iter()
                .chain(&locals)
                .chain(&reads)
                .map(|operation| (operation.id, 1)),
        )
    {
        let usage = meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(operation))
            .unwrap();
        assert_eq!(
            usage.executions(),
            executions,
            "operation {operation:?} must not replay across suspension"
        );
        assert_eq!(
            usage.units(),
            executions,
            "one fuel unit per committed operation"
        );
    }
}

#[test]
fn scalar_parameters_and_write_only_reborrows_keep_their_authored_positions() {
    let checked = checked(
        "machine reset(value: &write u64, returned: u64) -> u64 { value = 0; returned } machine enter(value: &mut u64, returned: u64) { let replacement: u64 = reset(&write value, returned); value = replacement; }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .unwrap();
    execute(&artifact, &[unsigned(37)], 37);
}

#[test]
fn unrelated_structural_return_bodies_do_not_join_the_selected_call_catalog() {
    let source = format!(
        "{SOURCE} data Unused {{}} machine Unused::reset(value: &mut bool) -> bool {{ value = false; true }}"
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked(&source), "enter")
        .produce_artifact()
        .unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(module.machines.len(), 2);
    assert_eq!(module.structural_types.len(), 1);
    execute(&artifact, &[], 7);
}

#[test]
fn attached_callee_uses_the_shared_catalogs_nested_type_and_field_identities() {
    let source = r#"
        data Payload { number: u64; }
        data Owner { payload: Payload; }
        data Earlier { first: bool; second: u64; }
        machine Owner::reset(value: &mut u64) -> u64 { value = 0; 7 }
        machine Earlier::enter(value: &mut u64) {
            let returned: u64 = Owner::reset(&mut value);
            value = returned;
        }
    "#;
    let checked = checked(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Earlier::enter")
        .produce_artifact()
        .unwrap();
    execute(&artifact, &[], 7);
}

#[test]
fn borrowed_scalar_call_rejects_missing_duplicated_or_substituted_callee_custody() {
    let original = checked(SOURCE);
    // Absence selects the independently complete ordinary body; a present
    // malformed legacy row must never redirect to that fallback.
    for mutation in 1..5 {
        let mut changed = original.clone();
        let plans = &mut changed
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines;
        assert_eq!(plans.len(), 1);
        match mutation {
            1 => plans.push(plans[0].clone()),
            2 => plans[0].effects.clear(),
            3 => plans[0].return_statement_ordinal = 0,
            4 => {
                plans[0].structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "enter")
                .produce_artifact()
                .is_err(),
            "callee custody mutation {mutation}"
        );
    }
}

#[test]
fn same_typed_borrowed_parameter_cannot_replace_the_authored_actual() {
    let mut checked = checked(
        "machine reset(value: &mut u64) -> u64 { value = 0; 7 } machine enter(first: &mut u64, second: &mut u64) { let returned: u64 = reset(&mut first); }",
    );
    let _ = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .expect("original borrowed actual");
    let caller = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(
                    operation,
                    checked_trees::CheckedUnitEffectOperationPlan::ScalarCall { .. }
                )
            })
        })
        .unwrap();
    let checked_trees::CheckedUnitEffectOperationPlan::ScalarCall {
        structural_arguments,
        ..
    } = &mut caller.operations[0]
    else {
        panic!("scalar call");
    };
    structural_arguments[0].source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    assert!(
        terminal_production::TerminalProductionRequest::new(&checked, "enter")
            .produce_artifact()
            .is_err()
    );
}

#[test]
fn caller_store_cannot_substitute_a_literal_for_the_returned_value() {
    let mut checked = checked(SOURCE);
    let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        value: zero, ..
    } = &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0]
        .effects[0]
    else {
        panic!("callee store");
    };
    let zero = zero.clone();
    let caller_index = ordinary_body_index(&checked, "enter");
    let caller = &mut checked.facts.flow.terminal_unit_effects.machines[caller_index];
    let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { value, .. } =
        &mut caller.operations[1]
    else {
        panic!("caller store");
    };
    *value = zero;
    assert!(
        terminal_production::TerminalProductionRequest::new(&checked, "enter")
            .produce_artifact()
            .is_err()
    );
}

#[test]
fn caller_store_roster_rejects_deleted_duplicate_or_stale_assignment_sites() {
    let original = checked(SOURCE);
    for mutation in 0..4 {
        let mut changed = original.clone();
        let caller_index = ordinary_body_index(&changed, "enter");
        let operations =
            &mut changed.facts.flow.terminal_unit_effects.machines[caller_index].operations;
        match mutation {
            0 => {
                operations.remove(1);
            }
            1 => operations.insert(1, operations[1].clone()),
            2 | 3 => {
                let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    statement_index,
                    ..
                } = &mut operations[1]
                else {
                    panic!("caller store");
                };
                *statement_index = if mutation == 2 { 0 } else { 99 };
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "enter")
                .produce_artifact()
                .is_err(),
            "store roster mutation {mutation}"
        );
    }
}

#[test]
fn an_unused_scalar_result_cannot_erase_its_callees_borrowed_write() {
    let mut checked = checked(
        "machine reset(value: &mut u64) -> u64 { value = 0; 7 } machine enter(value: &mut u64) { let returned: u64 = reset(&mut value); }",
    );
    let _ = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .expect("unused result still calls");
    let caller_index = ordinary_body_index(&checked, "enter");
    checked.facts.flow.terminal_unit_effects.machines[caller_index]
        .operations
        .remove(0);
    assert!(
        terminal_production::TerminalProductionRequest::new(&checked, "enter")
            .produce_artifact()
            .is_err()
    );
}

#[test]
fn ordinary_borrowed_scalar_body_replays_store_result_and_completion_custody() {
    let mut original = checked(SOURCE);
    let callee_index = ordinary_body_index(&original, "reset");
    let callee = original.facts.flow.terminal_unit_effects.machines[callee_index].machine;
    let legacy = &mut original
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines;
    assert_eq!(legacy.len(), 1);
    assert_eq!(legacy[0].machine, callee);
    legacy.remove(0);
    // This actually executes the fallback, including the borrowed zero store
    // followed by the scalar seven returned into the caller's second store.
    let artifact = terminal_production::TerminalProductionRequest::new(&original, "enter")
        .produce_artifact()
        .expect("complete ordinary borrowed scalar body");
    execute(&artifact, &[], 7);
    let plan = &original.facts.flow.terminal_unit_effects.machines[callee_index];
    assert!(matches!(
        plan.operations.as_slice(),
        [
            checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. },
            checked_trees::CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. },
            checked_trees::CheckedUnitEffectOperationPlan::Complete { .. }
        ]
    ));
    for mutation in 0..9 {
        let mut changed = original.clone();
        let plans = &mut changed.facts.flow.terminal_unit_effects.machines;
        match mutation {
            0 => {
                plans.remove(callee_index);
            }
            1 => plans.push(plans[callee_index].clone()),
            2 => {
                plans[callee_index].operations.remove(0);
            }
            3 => {
                let store = plans[callee_index].operations[0].clone();
                plans[callee_index].operations.insert(0, store);
            }
            4 => plans[callee_index].operations.swap(0, 1),
            5 => {
                plans[callee_index].structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            6 => plans[callee_index].scalar_result = None,
            7 => {
                plans[callee_index].operations.remove(1);
            }
            8 => {
                plans[callee_index].operations.pop();
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "enter")
                .produce_artifact()
                .is_err(),
            "consumed ordinary callee custody mutation {mutation}"
        );
    }
}
