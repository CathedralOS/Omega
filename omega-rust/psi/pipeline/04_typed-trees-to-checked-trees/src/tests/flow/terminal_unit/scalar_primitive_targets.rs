use super::super::super::ContractProofFactOwner;
use super::{CheckedUnitEffectOperationPlan, PrimitiveType};
use crate::tests::flow::terminal_unit::machine_named;
use crate::tests::front_end::checked_program;

#[test]
fn failed_settlement_publishes_no_intermediate_facts() {
    let source = format!(
        r#"{SOURCE}
        data Token {{ ready: bool; }}
        machine Token::drop(&mut self) requires self.ready {{}}
        data Root {{}}
        machine Root::measure(token: Token) -> u64 requires token.ready {{ 7u64 }}
    "#
    );
    let mut checked = checked_program(&source);
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "enter"))
            .is_some()
    );
    let measure = machine_named(&checked, "measure");
    let premises = checked
        .facts
        .proof
        .contract_facts
        .iter()
        .filter(|(_, fact)| {
            matches!(fact.owner,
                ContractProofFactOwner::Machine { machine_symbol } if machine_symbol == measure
            )
        })
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    assert!(!premises.is_empty());
    for premise in premises {
        assert!(checked.facts.proof.contract_facts.free(premise));
    }
    // Planning would repopulate both rosters before finding the missing cleanup
    // premise; the settlement consumes its input and publishes nothing on
    // failure, so the caller never sees the repopulated rosters.
    checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .clear();
    checked.facts.flow.terminal_unit_effects.machines.clear();
    let initial_diagnostics = crate::execution::finalize_execution::finalize_execution(
        &checked.typed,
        checked.facts.clone(),
    )
    .expect_err("initial finalization must also reject the missing cleanup premise");
    assert!(initial_diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at scalar return edge")
    }));
    let diagnostics = crate::settle_checked_execution(checked)
        .expect_err("missing cleanup premise rejects the complete settlement");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at scalar return edge")
    }));
}

const SOURCE: &str = r#"
    machine reset(value: &mut u64) -> u64 { value = 0; 7 }
    machine enter(value: &mut u64) {
        let returned: u64 = reset(&mut value);
        value = returned;
    }
"#;

/// The `requires` contract spelling carries the same caller-discharged entry
/// bound the declared range roster did; the store retains the selector as a
/// runtime element whose bound Terminal re-proves from that published
/// requirement.
#[test]
fn requires_bound_runtime_index_produces_a_runtime_element_store() {
    for access in ["&mut", "&write"] {
        for requires in [
            "requires index <= 3",
            "requires index < 4",
            "requires index >= 0 && index <= 3",
            "requires index <= 3 && value <= 9",
            // The mirror spelling states the same bound.
            "requires 4 > index",
        ] {
            let checked = checked_program(&format!(
                r#"
                machine forward(values: {access} [u16; 4], index: u64, value: u16)
                {requires}
                {{
                    values[index] = value;
                }}
            "#
            ));
            let plan = checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&checked, "forward"))
                .unwrap_or_else(|| {
                    panic!("{access} {requires}: contract-bound runtime index store")
                });
            let [
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    statement_index: 0,
                    destination:
                        checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                            parameter_index: 0,
                        },
                    path,
                    ..
                },
                CheckedUnitEffectOperationPlan::Complete { .. },
            ] = plan.operations.as_slice()
            else {
                panic!("{access} {requires}: runtime index keeps one write-only store");
            };
            assert_eq!(
                path.as_slice(),
                [
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth: 0 }
                    )
                ],
                "{access} {requires}: the runtime selector is the path's element segment"
            );
        }
    }
}

#[test]
fn primitive_scalar_call_borrows_the_body_for_a_deleted_registration_and_rejects_drift() {
    let original = checked_program(SOURCE);
    let caller = machine_named(&original, "enter");
    let callee = machine_named(&original, "reset");
    for mutation in 0..7 {
        let mut changed = original.clone();
        let plans = &mut changed
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines;
        let position = plans
            .iter()
            .position(|plan| plan.machine == callee)
            .unwrap();
        match mutation {
            0 => {
                plans.remove(position);
            }
            1 => plans.push(plans[position].clone()),
            2 => plans[position].effects.clear(),
            3 => plans[position].state = arena::Handle::invalid(),
            4 => plans[position].result_type = PrimitiveType::Bool,
            5 => plans[position]
                .cleanup_actions
                .push(checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(0)),
            6 => {
                plans[position].structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            _ => unreachable!(),
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
        );
        // A deleted registration leaves the callee's complete ordinary
        // scalar-result body, which the candidate closure borrows
        // (`scalar_targets::available_target`); a duplicate or drifted row
        // must match its own machine and cannot select that fallback.
        assert_eq!(
            rebuilt.for_machine(caller).is_some(),
            mutation == 0,
            "callee registration mutation {mutation}"
        );
    }
}
