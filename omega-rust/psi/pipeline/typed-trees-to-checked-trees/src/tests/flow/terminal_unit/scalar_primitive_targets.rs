use super::*;

const SOURCE: &str = r#"
    machine reset(value: &mut u64) -> u64 { value = 0; 7 }
    machine enter(value: &mut u64) {
        let returned: u64 = reset(&mut value);
        value = returned;
    }
"#;

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
        CheckedUnitEffectOperationPlan::ReturnUnit { .. },
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
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(callee)
            .is_none()
    );

    crate::rebuild_checked_terminal_plans_with_selected_execution(&mut checked, &[], &[])
        .expect("full selected rebuild");
    assert_eq!(
        checked.facts.flow.terminal_unit_effects.for_machine(caller),
        Some(&plan)
    );
    crate::rebuild_checked_unit_effect_plans_with_selected_execution(&mut checked, &[], &[]);
    assert_eq!(
        checked.facts.flow.terminal_unit_effects.for_machine(caller),
        Some(&plan)
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
    let independent = crate::flow::build_checked_primitive_store_scalar_return_plans(
        &checked.typed,
        &checked.facts,
    );
    assert!(independent.for_machine(primitive_machine).is_some());
    assert!(independent.for_machine(nominal_machine).is_none());
    crate::rebuild_checked_unit_effect_plans_with_selected_execution(&mut checked, &[], &[]);
    assert_eq!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(nominal_machine),
        Some(&nominal)
    );
    crate::rebuild_checked_terminal_plans_with_selected_execution(&mut checked, &[], &[]).unwrap();
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
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
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
            statement_index: 1,
            destination:
                checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
            value:
                CheckedScalarExpression::Local {
                    position: 1,
                    primitive_type: PrimitiveType::U64,
                },
        },
        CheckedUnitEffectOperationPlan::ReturnUnit { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("the caller stores the completed result, not its scalar input");
    };
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
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
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
