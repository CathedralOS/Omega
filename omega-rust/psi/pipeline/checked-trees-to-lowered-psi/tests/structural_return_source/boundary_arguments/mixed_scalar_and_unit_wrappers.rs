use super::{
    NESTED_WRAPPER_SOURCE, ObserveNestedWrapper, ObserveSettlement, artifact,
    assert_constructed_wrapper_execution, constructed_wrapper_source, source, start,
    start_with_scalars, unit_wrapper_artifact, unit_wrapper_source, unsigned,
};
use crate::structural_return_source::{
    TerminalEffect, TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter,
    decode_module,
};
use checked_trees::{CheckedScalarComputationKind, CheckedUnitEffectOperationPlan};
use checked_trees_to_lowered_psi::TerminalMachineSelection;

#[test]
fn mixed_scalar_formals_retain_ranges_and_linear_boundary_settlement() {
    let source = source("u16", "first, second", "", false)
        .replace(
            "Root::enter(receipt: Receipt)",
            "Root::enter(first: u16 [1..=100], receipt: Receipt, second: u16)",
        )
        .replace(
            "reaches PortIo\n        \n",
            "reaches PortIo\n        requires first >= second\n",
        );
    let checked = crate::front_end::checked_program(&source);
    let artifact = artifact(&checked);
    let module = decode_module(&artifact.0).unwrap();
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(root.parameters.len(), 2);
    assert_eq!(root.structural_parameters[0].position, 0);
    assert_eq!(
        checked.facts.flow.terminal_boundary_scalar_returns.machines[0].structural_parameters[0]
            .position,
        1
    );
    assert_eq!(root.contract.requires.len(), 1);
    let mut execution = start_with_scalars(&artifact, &[unsigned(70), unsigned(7)]);
    assert_eq!(execution.live_claim_frontier().count(), 1);
    let mut observer = ObserveSettlement::default();
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(unsigned(7)))
    );
    assert_eq!(observer.calls, [vec![unsigned(70), unsigned(7)]]);
    assert_eq!(execution.live_claim_frontier().count(), 0);
}

#[test]
fn mixed_scalar_wrapper_cannot_erase_or_substitute_structural_membership() {
    let source = source("u16", "value, value", "", false)
        .replace(
            "pub data Receipt [linear] { value: u64; }",
            "pub data Receipt [linear] { value: u64; }\ndomain Receipt::Ready;\ndomain Receipt::Other;",
        )
        .replace(
            "Root::enter(receipt: Receipt)",
            "Root::enter(receipt: Receipt in Ready, value: u16 [1..=100])",
        );
    let original = crate::front_end::checked_program(&source);
    artifact(&original);
    for mutation in 0..2 {
        let mut checked = original.clone();
        if mutation == 0 {
            checked.facts.flow.terminal_boundary_scalar_returns.machines[0].structural_parameters
                [0]
            .qualifications
            .clear();
        } else {
            let other = checked
                .typed
                .domain_definitions()
                .iter()
                .find(|domain| domain.name.as_str() == "Receipt::Other")
                .unwrap()
                .symbol;
            let mut changed = 0;
            let handles = checked
                .typed
                .proof_facts
                .iter()
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            for handle in handles {
                let fact = checked.typed.proof_facts.get_mut(handle);
                if let typed_trees::domain::ProofFact::Membership(membership) = fact {
                    membership.domain_symbol = other;
                    changed += 1;
                }
            }
            assert!(changed > 0);
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn unit_caller_transfers_linear_claim_into_scalar_boundary_wrapper() {
    let artifact =
        unit_wrapper_artifact(&crate::front_end::checked_program(&unit_wrapper_source()));
    let mut execution = start(&artifact);
    assert_eq!(execution.live_claim_frontier().count(), 1);
    let mut observer = ObserveSettlement::default();
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observer.calls, [vec![unsigned(70), unsigned(70)]]);
    assert_eq!(execution.live_claim_frontier().count(), 0);
}

#[test]
fn boundary_crash_in_scalar_wrapper_abandons_claim_without_a_result_or_receipt() {
    let source = unit_wrapper_source().replace("reaches PortIo", "reaches PortIo crashes Abort");
    let artifact = unit_wrapper_artifact(&crate::front_end::checked_program(&source));
    let mut execution = start(&artifact);
    let mut observer = ObserveSettlement {
        crash: Some(terminal_psi::CrashCause::Abort),
        ..Default::default()
    };
    let mut fuel = TerminalFuelMeter::unbounded();
    let status = execution.resume(&mut fuel, &mut observer).unwrap();
    let TerminalExecutionStatus::Crashed(crash) = &status else {
        panic!("{status:?}")
    };
    assert_eq!(crash.cause, terminal_psi::CrashCause::Abort);
    let terminal_interpreter::TerminalCrashSite::BoundaryCall {
        machine,
        operation,
        boundary,
        ..
    } = crash.site
    else {
        panic!("exact boundary call site")
    };
    let module = decode_module(&artifact.0).unwrap();
    assert_ne!(machine, module.entry, "crash remains in the scalar wrapper");
    assert_eq!(crash.frontier_lower_bound.len(), 1);
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        crash.frontier_lower_bound
    );
    assert_eq!(observer.calls, [vec![unsigned(70), unsigned(70)]]);
    assert!(
        matches!(execution.effects(), [TerminalEffect::BoundaryCall {
        operation: observed_operation, boundary: observed_boundary, completion_receipts, ..
    }] if *observed_operation == operation && *observed_boundary == boundary
        && completion_receipts.len() == 1)
    );
    let units = fuel.usage().total_units();
    assert_eq!(execution.resume(&mut fuel, &mut observer).unwrap(), status);
    assert_eq!(fuel.usage().total_units(), units);
    assert_eq!(observer.calls.len(), 1);
}

#[test]
fn unit_wrapper_accepts_nested_affine_result_argument() {
    let source = unit_wrapper_source()
        .replace("Receipt [linear]", "Receipt")
        .replace(
            "Wrapper::measure(receipt, 70u16)",
            "Wrapper::measure(forward(receipt), 70u16)",
        );
    let source = format!("{source}\nmachine forward(receipt: Receipt) -> Receipt {{ receipt }}");
    unit_wrapper_artifact(&crate::front_end::checked_program(&source));
}

#[test]
fn nested_wrapper_arguments_keep_effect_order_and_the_published_scalar_result() {
    let artifact = unit_wrapper_artifact(&crate::front_end::checked_program(NESTED_WRAPPER_SOURCE));
    let expected = [
        vec![unsigned(5)],
        vec![unsigned(11)],
        vec![unsigned(22)],
        vec![unsigned(33)],
        vec![unsigned(44)],
        vec![unsigned(55)],
        vec![unsigned(5), unsigned(55)],
        vec![unsigned(700)],
        vec![unsigned(5)],
    ];
    let mut reference_effects = None;
    for incremental in [false, true] {
        let mut execution = start(&artifact);
        let mut observer = ObserveNestedWrapper::default();
        let mut meter = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut complete = false;
        for _ in 0..1024 {
            match execution.resume(&mut meter, &mut observer).unwrap() {
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    assert!(incremental);
                    meter.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                status => panic!("unexpected status: {status:?}"),
            }
        }
        assert!(complete);
        assert_eq!(observer.calls, expected);
        assert!(execution.live_affine_frontier().next().is_none());
        if let Some(reference) = &reference_effects {
            assert_eq!(execution.effects(), reference);
        } else {
            reference_effects = Some(execution.effects().to_vec());
        }
    }
}

#[test]
fn nested_wrapper_rejects_reordered_producers_and_scalar_binding_drift() {
    let original = crate::front_end::checked_program(NESTED_WRAPPER_SOURCE);
    let root = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap()
        .symbol;
    for mutation in 0..4 {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == root)
            .unwrap();
        let producers = plan
            .operations
            .iter()
            .enumerate()
            .filter_map(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { .. }
                )
                .then_some(index)
            })
            .collect::<Vec<_>>();
        let [inner, outer] = producers.as_slice() else {
            panic!("two nested affine producers");
        };
        match mutation {
            0 => plan.operations.swap(*inner, *outer),
            1 => {
                let CheckedUnitEffectOperationPlan::StructuralCall { source_site, .. } =
                    plan.operations[*inner]
                else {
                    unreachable!();
                };
                let CheckedUnitEffectOperationPlan::StructuralCall {
                    source_site: changed_site,
                    ..
                } = &mut plan.operations[*outer]
                else {
                    unreachable!();
                };
                *changed_site = source_site;
            }
            2 | 3 => {
                let CheckedUnitEffectOperationPlan::ScalarCall {
                    result,
                    scalar_arguments,
                    ..
                } = &mut plan.operations[*outer + 1]
                else {
                    panic!("enclosing wrapper publishes a scalar local");
                };
                if mutation == 2 {
                    result.binding_ordinal += 1;
                } else {
                    // Only `before` is a source binding at this statement.
                    // Slot one exists during staging, but must not be readable.
                    scalar_arguments[1] = checked_trees::CheckedCallScalarArgument::Pure(
                        checked_trees::CheckedScalarExpression::Local {
                            position: 1,
                            primitive_type: typed_trees::types::PrimitiveType::U16,
                        },
                    );
                }
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "nested wrapper custody mutation {mutation}"
        );
    }
}

#[test]
fn nested_wrapper_computation_cannot_read_a_private_argument_slot() {
    let mut changed = crate::front_end::checked_program(NESTED_WRAPPER_SOURCE);
    let root = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap()
        .symbol;
    let argument = changed
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .unwrap()
        .operations
        .iter()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ScalarCall {
                result,
                scalar_arguments,
                ..
            } if result.binding_ordinal == 1 => Some(scalar_arguments[1].clone()),
            _ => None,
        })
        .unwrap();
    let checked_trees::CheckedCallScalarArgument::Computation(computation) = argument else {
        panic!("last wrapper operand calls observe");
    };
    let computations = &mut changed.facts.values.scalar_computations;
    let CheckedScalarComputationKind::Call { arguments, .. } =
        computations.nodes.get(computation).kind
    else {
        panic!("captured observe call");
    };
    let Some([operand]) = computations.operands.span(arguments) else {
        panic!("one observed scalar");
    };
    let operand = *operand;
    assert!(matches!(
        computations.nodes.get(operand).kind,
        CheckedScalarComputationKind::Value(_)
    ));
    computations.nodes.get_mut(operand).kind =
        CheckedScalarComputationKind::Value(checked_trees::CheckedScalarExpression::Local {
            position: 1,
            primitive_type: typed_trees::types::PrimitiveType::U16,
        });
    let error = checked_trees_to_lowered_psi::lower_machine(
        &changed,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect_err("private argument slots are not authored local bindings");
    assert!(
        format!("{error:?}")
            .contains("scalar read differs from its authored binding or mutable place"),
        "{error:?}"
    );
}

#[test]
fn nested_wrapper_operand_crash_preserves_only_the_completed_effect_prefix() {
    let source = format!(
        "machine abort() -> u16 crashes Abort {{ crash Abort; }}\n{}",
        NESTED_WRAPPER_SOURCE
            .replace("Scalar::observe(44u16)", "abort()")
            .replace(
                "machine Root::enter(receipt: Receipt) reaches PortIo",
                "machine Root::enter(receipt: Receipt) reaches PortIo crashes Abort"
            )
    );
    let artifact = unit_wrapper_artifact(&crate::front_end::checked_program(&source));
    let mut execution = start(&artifact);
    let mut observer = ObserveNestedWrapper::default();
    let status = execution
        .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
        .unwrap();
    assert!(
        matches!(&status, TerminalExecutionStatus::Crashed(crash) if crash.cause == terminal_psi::CrashCause::Abort)
    );
    assert_eq!(
        observer.calls,
        [
            vec![unsigned(5)],
            vec![unsigned(11)],
            vec![unsigned(22)],
            vec![unsigned(33)]
        ]
    );
    let effects = execution.effects().to_vec();
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap(),
        status
    );
    assert_eq!(execution.effects(), effects);
}

#[test]
fn unit_wrapper_consumes_empty_record_local() {
    let source = constructed_wrapper_source("", "");
    assert_constructed_wrapper_execution(&source);
}

#[test]
fn unit_wrapper_consumes_scalar_record_local() {
    let source = constructed_wrapper_source("value: i64;", "value: 7i64");
    assert_constructed_wrapper_execution(&source);
}
