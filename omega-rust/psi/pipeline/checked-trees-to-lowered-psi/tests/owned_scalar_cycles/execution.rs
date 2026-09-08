use semantic_vocabulary::IntegerValue;
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralScalarFieldValue, TerminalStructuralValue,
};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralTypeShape, TerminalModule, Terminator,
};

use super::{CUSTOMER, support};

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: support::unsigned_type(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn unchanged_ranked_walk_executes_selected_resets_with_one_unit_pauses() {
    let (module, _, semantic_bytes, proof_bytes) = support::publish(CUSTOMER);
    execute_cases(&module, &semantic_bytes, &proof_bytes);
}

pub fn execute_cases(module: &TerminalModule, semantic_bytes: &[u8], proof_bytes: &[u8]) {
    execute(module, semantic_bytes, proof_bytes, |_| 0);
}

fn execute(
    module: &TerminalModule,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    expected: impl Fn(u128) -> u128,
) {
    let walk = support::walk(module);
    let scratch = support::operation(walk, |kind| {
        matches!(kind, OperationKind::EstablishPrimitiveLocal { .. })
    });
    let OperationKind::EstablishPrimitiveLocal { value } = scratch.kind else {
        unreachable!();
    };
    support::assert_constant(walk, value, 0);
    let call = support::operation(walk, |kind| {
        matches!(kind, OperationKind::CallStructuralScalar { .. })
    });
    let OperationKind::CallStructuralScalar {
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        ..
    } = &call.kind
    else {
        unreachable!();
    };
    assert!(arguments.is_empty());
    assert!(claim_transfers.is_empty());
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(
        structural_arguments[0].place,
        scratch.result.structural().unwrap().place,
        "reset borrows the established scratch, not Limits or marker"
    );
    assert_eq!(
        structural_arguments[0].access,
        StructuralAccess::MutableBorrow
    );
    assert!(structural_arguments[0].path.is_empty());
    let marker_origins =
        std::collections::BTreeSet::from([walk.parameters[1].id, call.result.expect_scalar().id]);
    assert!(
        walk.blocks
            .iter()
            .flat_map(|block| &block.parameters)
            .any(|parameter| { support::scalar_origins(walk, parameter.id) == marker_origins }),
        "the exact reset snapshot is the next iteration's marker, through legitimate SSA staging"
    );
    let reset = module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .unwrap();
    assert_ne!(
        reset.id, walk.id,
        "walk recurs by a CFG edge, not a recursive call"
    );
    let store = support::operation(reset, |kind| {
        matches!(kind, OperationKind::WriteOnlyPrimitiveStore { .. })
    });
    let OperationKind::WriteOnlyPrimitiveStore { destination, value } = store.kind else {
        unreachable!();
    };
    assert_eq!(destination, reset.structural_parameters[0].place);
    support::assert_constant(reset, value, 0);
    let returns = reset
        .blocks
        .iter()
        .filter_map(|block| match block.terminator {
            Terminator::Return { edge, value, .. } => Some((edge, value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(returns.len(), 1);
    support::assert_constant(reset, returns[0].1, 0);
    let decrement = support::operation(walk, |kind| {
        matches!(kind, OperationKind::ExactIntegerSubtract { .. })
    });
    let guard = support::operation(walk, |kind| {
        matches!(kind, OperationKind::IntegerLessThan { .. })
    });
    let OperationKind::IntegerLessThan { left, right } = guard.kind else {
        unreachable!();
    };
    support::assert_constant(walk, left, 0);
    assert_eq!(
        support::scalar_origins(walk, right),
        support::scalar_origins(
            walk,
            match decrement.kind {
                OperationKind::ExactIntegerSubtract { left, .. } => left,
                _ => unreachable!(),
            }
        )
    );
    let branches = walk
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } if support::scalar_origins(walk, *condition)
                .contains(&guard.result.expect_scalar().id) =>
            {
                Some((when_true.edge, when_false.edge))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(branches.len(), 1);
    let observed_sites = [
        FuelChargeSite::Operation(scratch.id),
        FuelChargeSite::Edge(branches[0].0),
        FuelChargeSite::Operation(decrement.id),
        FuelChargeSite::Operation(call.id),
        FuelChargeSite::Operation(store.id),
        FuelChargeSite::Edge(returns[0].0),
        FuelChargeSite::Edge(branches[0].1),
    ];
    let limits = &walk.structural_parameters[0];
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == limits.structural_type)
        .unwrap();
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        panic!("unchanged Limits");
    };
    for remaining in 0..=5u64 {
        for limit in [0, 1, 42, 99, u128::from(u64::MAX)] {
            for divisor in 3..=5u128 {
                let field_values =
                    [("limit", limit), ("divisor", divisor)].map(|(identity, value)| {
                        TerminalStructuralScalarFieldValue {
                            argument_index: 0,
                            path: Vec::new(),
                            field: fields
                                .iter()
                                .find(|field| field.identity == identity)
                                .unwrap()
                                .id,
                            value: unsigned(value),
                        }
                    });
                let root = TerminalStructuralValue {
                    opaque_identity: 71,
                    structural_type: limits.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                };
                let mut execution =
                    TerminalExecution::start_artifact_with_structural_arguments_and_scalar_fields(
                        semantic_bytes,
                        proof_bytes,
                        &proof_admission::AdmissionProfile::default(),
                        &[unsigned(u128::from(remaining)), unsigned(201)],
                        &[root],
                        &field_values,
                    )
                    .expect("reload the actual owned scalar cycle with explicit integer fields");
                let mut meter = TerminalFuelMeter::with_allowance(0);
                let mut observed = Vec::new();
                let mut completed = false;
                for _ in 0..4096 {
                    let previous = meter.usage().clone();
                    let status = execution
                        .resume(&mut meter)
                        .expect("resume owned cyclic state");
                    assert!(meter.usage().total_units() - previous.total_units() <= 1);
                    for site in observed_sites {
                        let before = previous.at(site).map_or(0, |usage| usage.executions());
                        let after = meter.usage().at(site).map_or(0, |usage| usage.executions());
                        if after != before {
                            assert_eq!(after, before + 1);
                            observed.push(site);
                        }
                    }
                    match status {
                        TerminalExecutionStatus::Complete(result) => {
                            assert_eq!(
                                result,
                                TerminalExecutionResult::Scalar(unsigned(expected(limit))),
                                "remaining={remaining}, limit={limit}, divisor={divisor}"
                            );
                            completed = true;
                            break;
                        }
                        TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
                        other => panic!("unexpected cyclic outcome {other:?}"),
                    }
                }
                assert!(
                    completed,
                    "bounded test sponsor budget, not a compiler fixed-fuel requirement"
                );
                let mut expected_sites = Vec::new();
                for _ in 0..remaining {
                    expected_sites.extend_from_slice(&observed_sites[..6]);
                }
                expected_sites.extend([observed_sites[0], observed_sites[6]]);
                assert_eq!(
                    observed, expected_sites,
                    "one selected reset per descending edge; zero branch never calls reset"
                );
                assert!(
                    execution.live_affine_frontier().next().is_none(),
                    "Limits transferred on each backedge and discarded on final return"
                );
                assert!(execution.live_claim_frontier().next().is_none());
                for operation in walk.blocks.iter().flat_map(|block| &block.operations) {
                    if matches!(operation.kind, OperationKind::IntegerStructuralField { source, .. }
                        if walk.blocks.iter().any(|block| block.structural_parameters.iter().any(|parameter| parameter.place == source)))
                    {
                        assert_eq!(
                            meter
                                .usage()
                                .at(FuelChargeSite::Operation(operation.id))
                                .unwrap()
                                .executions(),
                            remaining + 1,
                            "field observes the rebound Limits on every visit"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn loop_body_observes_the_rebound_owned_integer_field_not_the_entry_alias() {
    let source = super::field_read_customer();
    let (module, _, semantic_bytes, proof_bytes) = support::publish(&source);
    let walk = support::walk(&module);
    let field_read = support::operation(walk, |kind| {
        matches!(kind, OperationKind::IntegerStructuralField { .. })
    });
    let OperationKind::IntegerStructuralField { source, .. } = field_read.kind else {
        unreachable!();
    };
    assert_ne!(source, walk.structural_parameters[0].place);
    assert!(walk.blocks.iter().any(|block| {
        block
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == source)
    }));
    let Some(terminal_psi::TerminalRankedScc::Natural(components)) = &walk.ranked_scc else {
        panic!("Natural loop");
    };
    let read_block = walk
        .blocks
        .iter()
        .find(|block| {
            block
                .operations
                .iter()
                .any(|operation| operation.id == field_read.id)
        })
        .unwrap();
    assert!(
        components.iter().any(|component| component
            .ranks
            .iter()
            .any(|rank| rank.block == read_block.id)),
        "field read executes in the actual SCC"
    );
    execute(&module, &semantic_bytes, &proof_bytes, |limit| limit);
}
