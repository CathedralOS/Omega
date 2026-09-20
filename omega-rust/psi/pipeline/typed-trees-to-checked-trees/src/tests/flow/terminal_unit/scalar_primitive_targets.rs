use super::super::super::ContractProofFactOwner;
use super::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
    PrimitiveType,
};
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

#[test]
fn initial_finalization_restores_complete_rosters_without_changing_check_evidence() {
    let checked = checked(SOURCE);
    let expected = checked.facts.clone();
    let mut facts = expected.clone();
    facts.flow.terminal_boundary_scalar_returns = Default::default();
    facts.flow.terminal_structural_scalar_returns = Default::default();
    facts.flow.terminal_unit_effects = Default::default();
    let rebuilt = crate::execution::finalize_execution::finalize_execution(&checked.typed, facts)
        .expect("complete initial execution plans");
    assert_eq!(rebuilt, expected);
}

#[test]
fn failed_selected_rebuild_preserves_previously_published_facts() {
    let source = format!(
        r#"{SOURCE}
        data Token {{ ready: bool; }}
        machine Token::drop(&mut self) requires self.ready {{}}
        data Root {{}}
        machine Root::measure(token: Token) -> u64 requires token.ready {{ 7u64 }}
    "#
    );
    let mut checked = checked(&source);
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
    // A rebuild would repopulate both rosters before finding the missing cleanup
    // premise. Failure must not publish independent returns or the Unit closure.
    checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .clear();
    checked.facts.flow.terminal_unit_effects.machines.clear();
    let before = checked.clone();
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
    let diagnostics =
        crate::rebuild_checked_terminal_plans_with_selected_execution(&mut checked, &[], &[])
            .expect_err("missing cleanup premise rejects the complete rebuild");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at scalar return edge")
    }));
    assert_eq!(
        checked, before,
        "a failed rebuild publishes no intermediate facts"
    );
}

const SOURCE: &str = r#"
    machine reset(value: &mut u64) -> u64 { value = 0; 7 }
    machine enter(value: &mut u64) {
        let returned: u64 = reset(&mut value);
        value = returned;
    }
"#;

#[test]
fn unit_planning_uses_explicit_callees_without_publishing_them() {
    let mut checked = checked(SOURCE);
    let caller = machine_named(&checked, "enter");
    let expected = checked.facts.flow.terminal_unit_effects.clone();
    assert!(expected.for_machine(caller).is_some());
    let boundary_returns = std::mem::take(&mut checked.facts.flow.terminal_boundary_scalar_returns);
    let structural_returns =
        std::mem::take(&mut checked.facts.flow.terminal_structural_scalar_returns);
    let before = checked.clone();
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &boundary_returns,
            structural_returns: &structural_returns,
        },
        &[],
        &[],
    );
    assert_eq!(
        rebuilt, expected,
        "published rosters are not planning inputs"
    );
    assert_eq!(checked, before, "planning borrows and never publishes");
}

#[test]
fn primitive_scalar_callee_is_discovered_before_its_unit_caller() {
    let mut checked = checked(SOURCE);
    let caller = machine_named(&checked, "enter");
    let callee = machine_named(&checked, "reset");
    let primitive = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(callee)
        .expect("primitive body")
        .clone();
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(caller)
        .expect("ordinary caller of primitive scalar body")
        .clone();
    let [
        CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            result,
            target_machine,
            target_state,
            scalar_arguments,
            structural_arguments,
            claim_transfers,
            ..
        },
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index: 1,
            destination:
                checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("call establishes scalar result before the caller store");
    };
    assert_eq!((*target_machine, *target_state), (callee, primitive.state));
    assert_eq!(
        (coordinate.statement_index, coordinate.call_ordinal),
        (0, 0)
    );
    assert_eq!((result.statement_index, result.binding_ordinal), (0, 0));
    assert_eq!(result.primitive_type, PrimitiveType::U64);
    assert!(scalar_arguments.is_empty());
    assert!(claim_transfers.is_empty());
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
    assert_eq!(
        structural_arguments[0].access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert!(structural_arguments[0].path.is_empty());
    let ordered_callee = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(callee)
        .expect("callee retains its complete ordered scalar body")
        .clone();
    assert_eq!(
        ordered_callee.structural_parameters,
        primitive.structural_parameters
    );
    let [
        store,
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value },
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 2, ..
        },
    ] = ordered_callee.operations.as_slice()
    else {
        panic!("ordered store then scalar return");
    };
    assert_eq!(store, &primitive.effects[0]);
    assert_eq!(ordered_callee.scalar_result.as_ref(), Some(result));
    assert_eq!(
        (result.statement_index, result.primitive_type),
        (1, PrimitiveType::U64)
    );
    assert!(
        matches!(value, checked_trees::CheckedCallScalarArgument::Pure(
        CheckedScalarExpression::IntegerLiteral { literal }) if literal.value_i64() == Some(7))
    );

    crate::rebuild_checked_terminal_plans_with_selected_execution(&mut checked, &[], &[])
        .expect("full selected rebuild");
    assert_eq!(
        checked.facts.flow.terminal_unit_effects.for_machine(caller),
        Some(&plan)
    );
    assert_eq!(
        checked.facts.flow.terminal_unit_effects.for_machine(callee),
        Some(&ordered_callee)
    );
    let rebuilt = checked.clone();
    crate::rebuild_checked_terminal_plans_with_selected_execution(&mut checked, &[], &[])
        .expect("repeated full selected rebuild");
    assert_eq!(checked, rebuilt, "full rebuild is idempotent");
    assert_eq!(
        checked.facts.flow.terminal_unit_effects.for_machine(caller),
        Some(&plan)
    );
    assert_eq!(
        checked.facts.flow.terminal_unit_effects.for_machine(callee),
        Some(&ordered_callee)
    );
    assert_eq!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(callee),
        Some(&primitive)
    );
}

#[test]
fn primitive_discovery_keeps_nominal_return_cleanup_in_the_dependent_phase() {
    let source = format!(
        r#"{SOURCE}
        data Token {{ value: u64; }}
        machine Token::drop(&mut self) {{}}
        data Root {{}}
        machine Root::measure(token: Token) -> u64 {{ 7u64 }}
    "#
    );
    let mut checked = checked(&source);
    let nominal_machine = machine_named(&checked, "measure");
    let primitive_machine = machine_named(&checked, "reset");
    let nominal = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(nominal_machine)
        .expect("later nominal-dependent return")
        .clone();
    assert!(matches!(
        nominal.cleanup_actions.as_slice(),
        [checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_)]
    ));
    let independent =
        crate::execution::terminal_unit::returns::build_checked_primitive_store_scalar_return_plans(
            &checked.typed,
            &checked.facts,
        );
    assert!(independent.for_machine(primitive_machine).is_some());
    assert!(independent.for_machine(nominal_machine).is_none());
    crate::rebuild_checked_terminal_plans_with_selected_execution(&mut checked, &[], &[])
        .expect("full rebuild retains dependent nominal cleanup");
    assert_eq!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(nominal_machine),
        Some(&nominal)
    );
}

#[test]
fn primitive_scalar_call_keeps_dense_scalar_actual_positions() {
    let source = SOURCE
        .replace(
            "value: &mut u64) ->",
            "value: &mut u64, replacement: u64, returned: u64) ->",
        )
        .replace("value = 0; 7", "value = replacement; returned")
        .replace("reset(&mut value)", "reset(&mut value, 3, 7)");
    let checked = checked(&source);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("mixed actuals");
    let CheckedUnitEffectOperationPlan::ScalarCall {
        scalar_arguments,
        structural_arguments,
        ..
    } = &plan.operations[0]
    else {
        panic!("ordinary scalar call");
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(scalar_arguments.len(), 2);
    for (argument, expected) in scalar_arguments.iter().zip([3, 7]) {
        assert!(matches!(argument,
            checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::IntegerLiteral { literal })
            if literal.value_i64() == Some(expected)));
    }
    let callee = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "reset"))
        .unwrap();
    assert_eq!(
        callee
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.source_position)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn declared_range_runtime_index_produces_indexed_write_only_store() {
    for access in ["&mut", "&write"] {
        let checked = checked(&format!(
            r#"
            machine forward(values: {access} [u16; 4], index: u64 [0..=3]) {{
                values[index] = 17;
            }}
        "#
        ));
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "forward"))
            .unwrap_or_else(|| panic!("{access} declared-range runtime index store"));
        let [
            CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore {
                statement_index: 0,
                destination:
                    checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
                path,
                index,
                value,
            },
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] = plan.operations.as_slice()
        else {
            panic!("{access} runtime index keeps one indexed write-only store");
        };
        assert!(
            path.is_empty(),
            "the runtime selector stays an operand; the path terminates at the array"
        );
        assert!(
            matches!(
                index,
                CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type: PrimitiveType::U64,
                }
            ),
            "the retained index is the declared scalar parameter"
        );
        assert!(matches!(value,
            checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::IntegerLiteral { literal })
            if literal.value_u64() == Some(17)));
    }
}

#[test]
fn runtime_index_store_fails_closed_without_a_proven_bound() {
    for source in [
        // No declared range: nothing proves `index < extent`.
        "machine forward(values: &mut [u16; 4], index: u64) { values[index] = 17; }",
        // Declared range whose maximum is not below the array extent.
        "machine forward(values: &mut [u16; 4], index: u64 [0..=4]) { values[index] = 17; }",
        // A computed selector is not a proven scalar carrier.
        "machine forward(values: &mut [u16; 4], index: u64 [0..=3]) { values[index + 1] = 17; }",
    ] {
        let source = format!("boundary trait PortIo {{}}\n{source}");
        let tokens = super::super::super::Lexer::new(&source)
            .tokenize()
            .expect("tokenize");
        let syntax = super::super::super::parse_syntax_trees(&tokens).expect("parse");
        let resolved =
            super::super::super::resolve(super::super::super::ResolutionRequest::new(&syntax))
                .expect("resolve");
        let typed = super::super::super::lower_symbol_resolved_trees(&resolved).expect("type");
        crate::lower_typed_trees(typed)
            .expect_err("unproven runtime index must not produce a store plan");
    }
}

#[test]
fn literal_index_store_stays_on_the_static_path() {
    let checked = checked(
        r#"
        machine forward(values: &mut [u16; 4]) {
            values[2] = 17;
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "forward"))
        .expect("literal index store");
    let [
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index: 0,
            path,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("literal index keeps the static write-only store");
    };
    assert!(matches!(
        path.as_slice(),
        [checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
            2
        )]
    ));
}

#[test]
fn primitive_scalar_call_rejects_deleted_duplicate_or_drifted_body_registration() {
    let original = checked(SOURCE);
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
            &[],
            &[],
        );
        assert!(
            rebuilt.for_machine(caller).is_none(),
            "callee registration mutation {mutation}"
        );
    }
}

#[test]
fn write_only_scalar_call_stores_its_result_after_scalar_parameters() {
    let mut checked = checked(
        r#"
        machine reset(value: &write u64, returned: u64) -> u64 {
            value = 0;
            returned
        }
        machine enter(value: &mut u64, returned: u64) {
            let replacement: u64 = reset(&write value, returned);
            value = replacement;
        }
    "#,
    );
    let caller = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(caller)
        .expect("write-only reborrow and scalar result after scalar inputs")
        .clone();
    let [
        CheckedUnitEffectOperationPlan::ScalarCall {
            scalar_arguments,
            structural_arguments,
            claim_transfers,
            result,
            ..
        },
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            path: store_path,
            statement_index: 1,
            destination:
                checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
            value:
                checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::Local {
                    position: 1,
                    primitive_type: PrimitiveType::U64,
                }),
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("the caller stores the completed result, not its scalar input");
    };
    assert!(store_path.is_empty());
    assert_eq!(plan.scalar_parameters[0].source_position, 1);
    assert_eq!(result.binding_ordinal, 0);
    assert!(claim_transfers.is_empty());
    assert!(matches!(
        scalar_arguments.as_slice(),
        [checked_trees::CheckedCallScalarArgument::Pure(
            CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U64,
            }
        )]
    ));
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
    assert_eq!(
        structural_arguments[0].access,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert!(structural_arguments[0].path.is_empty());
    for substituted_position in [0, 2] {
        let mut changed = checked.clone();
        let assignment = changed
            .facts
            .values
            .scalar_expressions
            .expressions
            .iter_mut()
            .find(|expression| {
                expression.state == plan.state
                    && expression.statement_ordinal == 1
                    && expression.role == CheckedScalarExpressionRole::AssignmentValue
            })
            .expect("exact assignment expression");
        assignment.expression = CheckedScalarExpression::Local {
            position: substituted_position,
            primitive_type: PrimitiveType::U64,
        };
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
            &[],
            &[],
        );
        assert!(
            rebuilt.for_machine(caller).is_none(),
            "result position {substituted_position} must not replace the exact result home"
        );
    }
    crate::rebuild_checked_terminal_plans_with_selected_execution(&mut checked, &[], &[])
        .expect("full selected rebuild");
    assert_eq!(
        checked.facts.flow.terminal_unit_effects.for_machine(caller),
        Some(&plan)
    );
}
