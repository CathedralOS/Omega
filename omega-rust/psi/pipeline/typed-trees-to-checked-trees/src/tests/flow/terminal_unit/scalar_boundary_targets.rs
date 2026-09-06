//! A Unit closure consumes the scalar boundary wrapper's real checked body.

use super::*;

const SOURCE: &str = r#"
    boundary trait Host {
        machine measure(value: i32) -> i32;
        machine send(value: i32);
    }
    data Scalar {}
    machine Scalar::measure() -> i32
    reaches Host
    { let result: i32 = Host::measure(70); result }
    data Root {}
    machine Root::run()
    reaches Host
    { let value: i32 = Scalar::measure(); Host::send(value); }
"#;

const SCALAR_PARAMETERS_SOURCE: &str = r#"
    boundary trait Host {
        machine measure(flag: bool, value: i32) -> i32;
        machine send(value: i32);
    }
    data Scalar {}
    machine Scalar::measure(value: i32, flag: bool) -> i32
    reaches Host
    { let result: i32 = Host::measure(flag, value); result }
    data Root {}
    machine Root::run(flag: bool, value: i32)
    reaches Host
    { let result: i32 = Scalar::measure(value, flag); Host::send(result); }
"#;

#[test]
fn scalar_boundary_wrapper_retains_reordered_scalar_formals_and_actuals() {
    let original = checked(SCALAR_PARAMETERS_SOURCE);
    let root = machine_named(&original, "run");
    let target = machine_named(&original, "Scalar::measure");
    let wrapper = original
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(target)
        .expect("parameterized wrapper retains its real boundary-return body");
    assert_eq!(
        wrapper
            .scalar_parameters
            .iter()
            .map(|parameter| { (parameter.source_position, parameter.primitive_type) })
            .collect::<Vec<_>>(),
        vec![(0, PrimitiveType::I32), (1, PrimitiveType::Bool)]
    );
    assert!(wrapper.structural_parameters.is_empty());
    let unit = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .expect("ordinary Unit call retains scalar-only wrapper");
    let CheckedUnitEffectOperationPlan::ScalarCall {
        scalar_arguments, ..
    } = &unit.operations[0]
    else {
        panic!("expected real scalar invocation")
    };
    assert!(matches!(
        &scalar_arguments[0],
        checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::I32
        })
    ));
    assert!(
        matches!(&scalar_arguments[1], checked_trees::CheckedCallScalarArgument::Pure(
        CheckedScalarExpression::Boolean(expression)
    ) if matches!(expression.as_ref(), CheckedBooleanExpression::Parameter { position: 0 }))
    );
    for mutation in 0..4 {
        let mut changed = original.clone();
        let plans = &mut changed.facts.flow.terminal_boundary_scalar_returns.machines;
        let index = plans
            .iter()
            .position(|plan| plan.machine == target)
            .unwrap();
        match mutation {
            0 => plans[index].scalar_parameters[0].source_position = 1,
            1 => plans[index].scalar_parameters[0].primitive_type = PrimitiveType::Bool,
            2 => {
                plans[index].scalar_parameters.pop();
            }
            _ => plans.push(plans[index].clone()),
        }
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
        assert!(
            rebuilt.for_machine(root).is_none(),
            "scalar roster drift {mutation}"
        );
    }
}

#[test]
fn scalar_boundary_wrapper_partitions_interleaved_structural_signature() {
    let source = r#"
        data Metrics { current: i32; }
        boundary trait Host {
            machine measure(flag: bool, metrics: Metrics, value: i32) -> i32;
        }
        data Scalar {}
        machine Scalar::measure(flag: bool, metrics: Metrics, value: i32) -> i32
        reaches Host
        { let result: i32 = Host::measure(flag, metrics, value); result }
    "#;
    let checked = checked(source);
    let target = machine_named(&checked, "Scalar::measure");
    let wrapper = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(target)
        .expect("named root retains interleaved structural and scalar parameters");
    assert_eq!(wrapper.structural_parameters.len(), 1);
    assert_eq!(wrapper.structural_parameters[0].position, 1);
    assert_eq!(
        wrapper
            .scalar_parameters
            .iter()
            .map(|parameter| { (parameter.source_position, parameter.primitive_type) })
            .collect::<Vec<_>>(),
        vec![(0, PrimitiveType::Bool), (2, PrimitiveType::I32)]
    );
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        scalar_arguments,
        structural_arguments,
        ..
    } = &wrapper.boundary_call
    else {
        panic!("expected boundary call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
    assert!(matches!(
        &scalar_arguments[1],
        checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::I32
        })
    ));
    assert!(matches!(
        checked.facts.values.scalar_expressions.expression_at(
            wrapper.state,
            1,
            CheckedScalarExpressionRole::Return
        ),
        Some(CheckedScalarExpression::Local {
            position: 2,
            primitive_type: PrimitiveType::I32
        })
    ));
}

#[test]
fn scalar_boundary_wrapper_does_not_erase_scalar_requirements() {
    let source = r#"
        boundary trait Host { machine measure(value: i32) -> i32; }
        data Scalar {}
        machine Scalar::measure(value: i32) -> i32
        requires 1 <= value
        reaches Host
        { let result: i32 = Host::measure(value); result }
    "#;
    let original = checked(source);
    let target = machine_named(&original, "Scalar::measure");
    assert!(
        original
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .for_machine(target)
            .is_some()
    );
    assert!(matches!(
        original
            .facts
            .contract_plans
            .for_machine(target)
            .unwrap()
            .closed_scalar_values
            .requires(),
        [Some(checked_trees::ClosedScalarContractValue::Predicate(_))]
    ));
    let unsupported = checked(&source.replace("1 <= value", "value + 0 <= 100"));
    let target = machine_named(&unsupported, "Scalar::measure");
    assert!(
        unsupported
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .for_machine(target)
            .is_none(),
        "unsupported authored requirement must not be discarded"
    );
}

#[test]
fn mixed_scalar_boundary_wrapper_retains_implicit_range_in_dense_namespace() {
    let source = r#"
        data Metrics { current: i32; }
        boundary trait Host {
            machine measure(flag: bool, metrics: Metrics, value: i32) -> i32;
        }
        data Scalar {}
        machine Scalar::measure(flag: bool, metrics: Metrics, value: i32 [1..=100]) -> i32
        reaches Host
        { let result: i32 = Host::measure(flag, metrics, value); result }
    "#;
    let checked = checked(source);
    let target = machine_named(&checked, "Scalar::measure");
    let wrapper = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(target)
        .expect("mixed wrapper keeps supported implicit scalar range");
    assert_eq!(wrapper.scalar_parameters[1].source_position, 2);
    let requirements = checked
        .facts
        .contract_plans
        .for_machine(target)
        .unwrap()
        .closed_scalar_values
        .requires();
    let [
        Some(checked_trees::ClosedScalarContractValue::Predicate(CheckedBooleanExpression::And {
            left,
            right,
        })),
    ] = requirements
    else {
        panic!("one complete retained range: {requirements:?}")
    };
    for (predicate, lower) in [(left.as_ref(), true), (right.as_ref(), false)] {
        let CheckedBooleanExpression::IntegerComparison { left, right, .. } = predicate else {
            panic!("range retains comparisons")
        };
        let subject = if lower { right } else { left };
        assert!(matches!(
            subject.as_ref(),
            CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::I32
            }
        ));
    }

    let source = source
        .replace("flag: bool", "first: i32")
        .replace("Host::measure(flag,", "Host::measure(first,")
        .replace("reaches Host", "requires first <= value\n reaches Host");
    let mixed = super::checked(&source);
    let target = machine_named(&mixed, "Scalar::measure");
    assert!(
        mixed
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .for_machine(target)
            .is_some()
    );
    let requirements = mixed
        .facts
        .contract_plans
        .for_machine(target)
        .unwrap()
        .closed_scalar_values
        .requires();
    let [
        Some(checked_trees::ClosedScalarContractValue::Predicate(
            CheckedBooleanExpression::IntegerComparison { left, right, .. },
        )),
        Some(_),
    ] = requirements
    else {
        panic!("explicit predicate and range: {requirements:?}")
    };
    assert!(matches!(
        left.as_ref(),
        CheckedScalarExpression::Parameter { position: 0, .. }
    ));
    assert!(matches!(
        right.as_ref(),
        CheckedScalarExpression::Parameter { position: 1, .. }
    ));
}

#[test]
fn scalar_parameter_range_collector_keeps_unsupported_endpoint_rows() {
    let source = r#"
        boundary trait Host { machine measure(value: i32) -> i32; }
        data Scalar {}
        machine Scalar::measure(value: i32 [1..=100]) -> i32
        reaches Host
        { let result: i32 = Host::measure(value); result }
    "#;
    let mut checked = checked(source);
    let target = machine_named(&checked, "Scalar::measure");
    let mut missing = checked.clone();
    let contract = missing
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|contract| contract.machine == target)
        .unwrap();
    contract.closed_scalar_values = checked_trees::ClosedScalarValueContractPlan::new(
        Vec::new(),
        contract.closed_scalar_values.ensures().to_vec(),
        contract.closed_scalar_values.has_crash_clauses(),
        contract.closed_scalar_values.has_outcome_specific_clauses(),
    );
    let rebuilt =
        crate::flow::build_checked_boundary_scalar_return_plans(&missing.typed, &missing.facts);
    assert!(
        rebuilt.for_machine(target).is_none(),
        "a missing implicit range is not an empty contract"
    );
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == target)
        .unwrap()
        .clone();
    let state = &checked.typed.machine_states(&machine)[0];
    let reference = checked.typed.state_parameters(state)[0].type_reference;
    let TypeReferenceNode::Constrained { constraints, .. } =
        checked.typed.type_reference_table.type_reference(reference)
    else {
        panic!("authored parameter range")
    };
    let constraints = *constraints;
    let [typed_trees::types::TypeConstraintNode::Range { maximum, .. }] = checked
        .typed
        .type_reference_table
        .constraints_mut(constraints)
    else {
        panic!("one authored range")
    };
    *maximum = arena::Handle::invalid();
    let requirements = crate::values::lower_integer_parameter_range_requirements(
        &checked.typed,
        &checked.facts.operators,
        &machine,
    );
    assert_eq!(
        requirements,
        vec![None],
        "unsupported range remains a rejecting slot"
    );
}

#[test]
fn unit_scalar_call_retains_registered_boundary_return_target() {
    let mut checked = checked(SOURCE);
    let root = machine_named(&checked, "run");
    let target = machine_named(&checked, "Scalar::measure");
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(target)
            .is_none()
    );
    let wrapper = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .iter()
        .find(|plan| plan.machine == target)
        .unwrap()
        .clone();
    assert!(wrapper.structural_parameters.is_empty());
    assert!(wrapper.entry_claims.is_empty());
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .expect("Unit caller retains scalar wrapper");
    let CheckedUnitEffectOperationPlan::ScalarCall {
        target_machine,
        target_state,
        result,
        target_contract_commitment,
        ..
    } = &plan.operations[0]
    else {
        panic!("wrapper stays an ordinary scalar call");
    };
    assert_eq!(
        (*target_machine, *target_state, result.primitive_type),
        (target, wrapper.state, wrapper.result_type)
    );
    assert_eq!(
        *target_contract_commitment,
        checked
            .facts
            .contract_plans
            .for_machine(target)
            .unwrap()
            .commitment
    );
    crate::rebuild_checked_terminal_plans_with_selected_execution(&mut checked, &[], &[]).unwrap();
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(root)
            .is_some()
    );
    crate::rebuild_checked_unit_effect_plans_with_selected_execution(&mut checked, &[], &[]);
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(root)
            .is_some()
    );
}

#[test]
fn unit_scalar_call_rejects_missing_or_drifted_boundary_return_registration() {
    let original = checked(SOURCE);
    let root = machine_named(&original, "run");
    let target = machine_named(&original, "Scalar::measure");
    for mutation in 0..3 {
        let mut changed = original.clone();
        let plans = &mut changed.facts.flow.terminal_boundary_scalar_returns.machines;
        if mutation == 0 {
            plans.retain(|plan| plan.machine != target);
        } else {
            let plan = plans
                .iter_mut()
                .find(|plan| plan.machine == target)
                .unwrap();
            if mutation == 1 {
                plan.result_type = PrimitiveType::Bool;
            } else {
                plan.state = arena::Handle::invalid();
            }
        }
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
        assert!(
            rebuilt.for_machine(root).is_none(),
            "wrapper registration mutation {mutation}"
        );
    }
}

#[test]
fn scalar_boundary_wrapper_retains_structural_call_custody() {
    let source = SOURCE
        .replace(
            "boundary trait Host",
            "data Metrics { current: i32; }\n boundary trait Host",
        )
        .replace(
            "machine measure(value: i32)",
            "machine measure(metrics: Metrics, value: i32)",
        )
        .replace(
            "Scalar::measure() ->",
            "Scalar::measure(metrics: Metrics) ->",
        )
        .replace("Host::measure(70)", "Host::measure(metrics, 70)")
        .replace("Root::run()", "Root::run(metrics: Metrics)")
        .replace("Scalar::measure();", "Scalar::measure(metrics);");
    let checked = checked(&source);
    let root = machine_named(&checked, "run");
    let target = machine_named(&checked, "Scalar::measure");
    let wrapper = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .iter()
        .find(|plan| plan.machine == target)
        .expect("structural wrapper keeps its existing body plan");
    assert_eq!(wrapper.structural_parameters.len(), 1);
    let caller = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .expect("ordinary scalar invocation retains the owned structural argument");
    let CheckedUnitEffectOperationPlan::ScalarCall {
        structural_arguments,
        claim_transfers,
        ..
    } = &caller.operations[0]
    else {
        panic!("ordinary scalar call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
    assert!(claim_transfers.is_empty());
    assert!(
        matches!(caller.operations.last(), Some(CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards, ..
    }) if trivial_affine_discards.is_empty()),
        "transferred affine parameter is not discarded twice"
    );
}

#[test]
fn scalar_boundary_wrapper_transfers_exact_linear_claim_with_mixed_signature() {
    let source = r#"
        pub data Receipt [linear] { value: u64; }
        boundary machine Receipt::settle(self, value: u16) -> u16
        reaches PortIo ensures true;
        data Wrapper {}
        machine Wrapper::measure(value: u16, receipt: Receipt) -> u16
        reaches PortIo
        { let result: u16 = receipt.settle(value); result }
        data Root {}
        machine Root::run(receipt: Receipt) reaches PortIo
        { let result: u16 = Wrapper::measure(70u16, receipt); }
    "#;
    let original = checked(source);
    let root = machine_named(&original, "run");
    let target = machine_named(&original, "Wrapper::measure");
    let wrapper = original
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(target)
        .expect("real scalar boundary-return body");
    assert_eq!(wrapper.scalar_parameters[0].source_position, 0);
    assert_eq!(wrapper.structural_parameters[0].position, 1);
    assert_eq!(wrapper.entry_claims.len(), 1);
    let caller = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .expect("Unit call transfers linear custody to scalar wrapper");
    let CheckedUnitEffectOperationPlan::ScalarCall {
        structural_arguments,
        claim_transfers,
        scalar_arguments,
        ..
    } = &caller.operations[0]
    else {
        panic!("ordinary scalar call with custody")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(scalar_arguments.len(), 1);
    assert_eq!(claim_transfers.len(), 1);
    assert_eq!(claim_transfers[0].argument_index, 0);
    assert_eq!(
        claim_transfers[0].claim_identity,
        caller.entry_claims[0].claim_identity
    );
    assert_ne!(
        claim_transfers[0].claim_identity,
        wrapper.entry_claims[0].claim_identity
    );
    assert!(
        matches!(caller.operations.last(), Some(CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards, trivial_affine_local_discard_ordinals, ..
    }) if trivial_affine_discards.is_empty() && trivial_affine_local_discard_ordinals.is_empty())
    );
    for mutation in 0..4 {
        let mut changed = original.clone();
        let wrapper = changed
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .machines
            .iter_mut()
            .find(|plan| plan.machine == target)
            .unwrap();
        match mutation {
            0 => wrapper.entry_claims.clear(),
            1 => wrapper.entry_claims[0].parameter_index = 1,
            2 => wrapper.structural_parameters[0].position = 0,
            _ => wrapper.structural_parameters[0]
                .type_identity
                .push_str("-foreign"),
        }
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
        assert!(
            rebuilt.for_machine(root).is_none(),
            "mixed target custody mutation {mutation}"
        );
    }
}

#[test]
fn scalar_boundary_wrapper_consumes_established_affine_result_once() {
    let source = r#"
        data Metrics { current: i32; }
        boundary trait Host { machine measure(metrics: Metrics, value: i32) -> i32; }
        machine forward(metrics: Metrics) -> Metrics { metrics }
        data Wrapper {}
        machine Wrapper::measure(metrics: Metrics, value: i32) -> i32
        reaches Host
        { let result: i32 = Host::measure(metrics, value); result }
        data Root {}
        machine Root::run(metrics: Metrics) reaches Host {
            let moved: Metrics = forward(metrics);
            let result: i32 = Wrapper::measure(moved, 70);
        }
    "#;
    let checked = checked(source);
    let root = machine_named(&checked, "run");
    let caller = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .expect("established affine result feeds the scalar wrapper");
    assert!(
        matches!(&caller.operations[0], CheckedUnitEffectOperationPlan::StructuralCall {
        discard_result_on_return: false, result, ..
    } if result.binding_ordinal == 0)
    );
    let CheckedUnitEffectOperationPlan::ScalarCall {
        structural_arguments,
        claim_transfers,
        ..
    } = &caller.operations[1]
    else {
        panic!("scalar result consumer")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(
        structural_arguments[0].source_structural_result_binding_ordinal(),
        Some(0)
    );
    assert!(claim_transfers.is_empty());
    assert!(
        matches!(caller.operations.last(), Some(CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards, trivial_affine_local_discard_ordinals, ..
    }) if trivial_affine_discards.is_empty() && trivial_affine_local_discard_ordinals.is_empty())
    );
}

#[test]
fn scalar_boundary_wrapper_consumes_existing_constructed_local_kinds() {
    for (fields, values, scalar_record) in [("", "", false), ("value: i64;", "value: 7i64", true)] {
        let source = format!(
            r#"
            data Payload {{ {fields} }}
            boundary trait Host {{ machine measure(payload: Payload, value: u16) -> u16; }}
            data Wrapper {{}}
            machine Wrapper::measure(payload: Payload, value: u16) -> u16 reaches Host
            {{ let result: u16 = Host::measure(payload, value); result }}
            data Root {{}}
            machine Root::run() reaches Host {{
                let payload: Payload = Payload {{ {values} }};
                let result: u16 = Wrapper::measure(payload, 70u16);
            }}
        "#
        );
        let original = checked(&source);
        let root = machine_named(&original, "run");
        let caller = original
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(root)
            .expect("constructed local reaches scalar wrapper through existing argument source");
        assert_eq!(caller.operations.len(), 3);
        if scalar_record {
            assert!(matches!(
                &caller.operations[0],
                CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal {
                    statement_index: 0,
                    declaration_ordinal: 0,
                    ..
                }
            ));
        } else {
            assert!(matches!(
                &caller.operations[0],
                CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                    statement_index: 0,
                    declaration_ordinal: 0,
                    ..
                }
            ));
        }
        let CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            result,
            structural_arguments,
            claim_transfers,
            ..
        } = &caller.operations[1]
        else {
            panic!("scalar wrapper consumer")
        };
        assert_eq!(coordinate.statement_index, 1);
        assert_eq!(
            result.binding_ordinal, 0,
            "structural construction occupies no scalar binding"
        );
        assert_eq!(structural_arguments.len(), 1);
        if scalar_record {
            assert_eq!(
                structural_arguments[0].source_affine_scalar_record_local_declaration_ordinal(),
                Some(0)
            );
        } else {
            assert_eq!(
                structural_arguments[0].source_local_declaration_ordinal(),
                Some(0)
            );
        }
        assert!(claim_transfers.is_empty());
        assert!(
            matches!(caller.operations.last(), Some(CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_discards, trivial_affine_local_discard_ordinals, ..
        }) if trivial_affine_discards.is_empty() && trivial_affine_local_discard_ordinals.is_empty())
        );

        let mutable = checked(&source.replace("let payload:", "let mut payload:"));
        let root = machine_named(&mutable, "run");
        assert!(
            mutable
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(root)
                .is_none(),
            "mutable construction remains outside the immutable local path"
        );
    }
}

#[test]
fn scalar_wrapper_transfer_keeps_unconsumed_empty_prefix_cleanup() {
    let source = r#"
        data Payload {}
        boundary trait Host { machine measure(payload: Payload) -> u16; }
        data Wrapper {}
        machine Wrapper::measure(payload: Payload) -> u16 reaches Host
        { let result: u16 = Host::measure(payload); result }
        data Root {}
        machine Root::run() reaches Host {
            let unused: Payload = Payload {};
            let consumed: Payload = Payload {};
            let result: u16 = Wrapper::measure(consumed);
        }
    "#;
    let checked = checked(source);
    let root = machine_named(&checked, "run");
    let caller = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .unwrap();
    assert!(
        matches!(caller.operations.last(), Some(CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_local_discard_ordinals, ..
    }) if trivial_affine_local_discard_ordinals == &[0])
    );
}
