//! Ordinary callers preserve borrowed primitive effects and scalar results.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};
use terminal_psi::{OperationKind, StructuralAccess};

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
    let original = crate::front_end::checked_program(TWO_LOCAL_SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &original,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
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
            terminal_production::TerminalProductionRequest::new(
                &changed,
                TerminalMachineSelection::Name("enter")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "primitive declaration/borrow custody mutation {mutation}"
        );
    }
}

#[test]
fn primitive_storage_read_cannot_substitute_another_symbol_initializer_or_scalar_binding() {
    let original = crate::front_end::checked_program(TWO_LOCAL_SOURCE);
    let _ = terminal_production::TerminalProductionRequest::new(
        &original,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("original storage read")
    .into_artifact();
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
            value: checked_trees::CheckedCallScalarArgument::Pure(value),
            ..
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
            terminal_production::TerminalProductionRequest::new(
                &changed,
                TerminalMachineSelection::Name("enter")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "storage read custody mutation {mutation}, synchronized expression={synchronize_expression}"
        );
    }
}

#[test]
fn primitive_local_plan_cannot_grant_mutability_to_an_immutable_authored_binding() {
    let mut changed = crate::front_end::checked_program(TWO_LOCAL_SOURCE);
    let _ = terminal_production::TerminalProductionRequest::new(
        &changed,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("original mutable local")
    .into_artifact();
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
        terminal_production::TerminalProductionRequest::new(
            &changed,
            TerminalMachineSelection::Name("enter")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err()
    );
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
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        arguments,
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            primitive_values: &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: unsigned(91),
            }],
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut complete = false;
    for _ in 0..32 {
        let status = execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap();
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
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Earlier::enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    execute(&artifact, &[], 7);
}

#[test]
fn borrowed_scalar_call_rejects_missing_duplicated_or_substituted_callee_custody() {
    let original = crate::front_end::checked_program(SOURCE);
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
            terminal_production::TerminalProductionRequest::new(
                &changed,
                TerminalMachineSelection::Name("enter")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "callee custody mutation {mutation}"
        );
    }
}

#[test]
fn ordinary_borrowed_scalar_body_replays_store_result_and_completion_custody() {
    let mut original = crate::front_end::checked_program(SOURCE);
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
    let artifact = terminal_production::TerminalProductionRequest::new(
        &original,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("complete ordinary borrowed scalar body")
    .into_artifact();
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
            terminal_production::TerminalProductionRequest::new(
                &changed,
                TerminalMachineSelection::Name("enter")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "consumed ordinary callee custody mutation {mutation}"
        );
    }
}
