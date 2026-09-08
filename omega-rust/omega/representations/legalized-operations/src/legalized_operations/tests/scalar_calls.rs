use super::*;

fn call(plan: &mut LegalizedOperationPlan) -> &mut LegalizedScalarCall {
    plan.scalar_functions[0].blocks[0]
        .instructions
        .iter_mut()
        .find_map(|operation| {
            if let LegalizedScalarInstructionKind::Call(call) = &mut operation.kind {
                Some(call)
            } else {
                None
            }
        })
        .expect("call fixture")
}

#[test]
fn register_call_shape_admits_actual_arity_and_rejects_roster_corruption() {
    for arity in 0..=6 {
        let mut plan = scalar_call_unit_plan();
        let call = call(&mut plan);
        let argument = call.arguments[0].clone();
        call.call_plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); arity],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        call.arguments = call
            .call_plan
            .parameters
            .iter()
            .map(|placement| LegalizedScalarArgument::Scalar {
                placement: placement.clone(),
                source: argument.scalar_source().unwrap(),
            })
            .collect();
        assert_eq!(call.validate_shape(), Ok(()));
        if arity > 0 {
            let mut corrupt = call.clone();
            corrupt.result_placement.as_mut().unwrap().locations.clear();
            assert!(corrupt.validate_shape().is_err());
            let mut corrupt = call.clone();
            corrupt.arguments.pop();
            assert!(corrupt.validate_shape().is_err());
            let mut corrupt = call.clone();
            scalar_argument_mut(&mut corrupt.arguments[0])
                .1
                .locations
                .clear();
            assert!(corrupt.validate_shape().is_err());
        }
    }
}

#[test]
fn fixed_scalar_argument_widths_require_exact_register_geometry() {
    for policy in [CallingPolicy::SystemVAMD64, CallingPolicy::MicrosoftX64] {
        for width in [1, 2, 4, 8] {
            let mut plan = scalar_call_unit_plan();
            let call = call(&mut plan);
            let source = call.arguments[0].scalar_source().unwrap();
            call.call_plan = evaluate_call_plan(
                policy,
                &CallSignature {
                    parameters: vec![ValueShape::integer(width, width)],
                    result: None,
                },
            )
            .unwrap();
            call.result_placement = None;
            call.arguments = vec![LegalizedScalarArgument::Scalar {
                source,
                placement: call.call_plan.parameters[0].clone(),
            }];
            assert_eq!(call.validate_shape(), Ok(()));
            for mutation in 0..5 {
                let mut changed = call.clone();
                let placement = &mut changed.call_plan.parameters[0];
                match mutation {
                    0 => placement.shape.alignment = if width == 1 { 2 } else { 1 },
                    1 => placement.shape.byte_size = 3,
                    2 => {
                        let calling_conventions::ValueLocation::Register { byte_size, .. } =
                            &mut placement.locations[0]
                        else {
                            panic!("register");
                        };
                        *byte_size = if width == 1 { 2 } else { 1 };
                    }
                    3 => {
                        let calling_conventions::ValueLocation::Register {
                            value_byte_offset, ..
                        } = &mut placement.locations[0]
                        else {
                            panic!("register");
                        };
                        *value_byte_offset = 1;
                    }
                    _ => placement.locations.push(placement.locations[0]),
                }
                *scalar_argument_mut(&mut changed.arguments[0]).1 = placement.clone();
                assert!(
                    changed.validate_shape().is_err(),
                    "width {width}, mutation {mutation}"
                );
            }
            call.call_plan.result = Some(call.call_plan.parameters[0].clone());
            call.result_placement = call.call_plan.result.clone();
            assert_eq!(
                call.validate_shape().is_ok(),
                width == 8,
                "argument width must not widen scalar-result admission"
            );
        }
    }
}

#[test]
fn boolean_argument_shape_preserves_width_and_unit_result_absence() {
    for policy in [CallingPolicy::SystemVAMD64, CallingPolicy::MicrosoftX64] {
        let mut plan = scalar_call_unit_plan();
        let call = call(&mut plan);
        let source = call.arguments[0].scalar_source().unwrap();
        call.call_plan = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: vec![ValueShape::integer(1, 1)],
                result: None,
            },
        )
        .unwrap();
        call.result_placement = None;
        call.arguments = vec![LegalizedScalarArgument::Scalar {
            source,
            placement: call.call_plan.parameters[0].clone(),
        }];
        assert_eq!(call.validate_shape(), Ok(()));
        let mut changed = call.clone();
        changed.result_placement = Some(changed.call_plan.parameters[0].clone());
        assert!(changed.validate_shape().is_err());
        let mut changed = call.clone();
        let calling_conventions::ValueLocation::Register { byte_size, .. } =
            &mut changed.call_plan.parameters[0].locations[0]
        else {
            unreachable!()
        };
        *byte_size = 8;
        *scalar_argument_mut(&mut changed.arguments[0]).1 = changed.call_plan.parameters[0].clone();
        assert!(changed.validate_shape().is_err());
        let mut changed = call.clone();
        changed.call_plan.result = Some(changed.call_plan.parameters[0].clone());
        changed.result_placement = changed.call_plan.result.clone();
        assert!(
            changed.validate_shape().is_err(),
            "Boolean results are not admitted by an argument-width extension"
        );
    }
}

#[test]
fn register_call_identity_retains_argument_length_order_and_placement() {
    let plan = scalar_call_unit_plan();
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..4 {
        let mut proposed = plan.clone();
        let call = call(&mut proposed);
        match mutation {
            0 => {
                call.arguments.pop();
            }
            1 => call.arguments.swap(0, 1),
            2 => *scalar_argument_mut(&mut call.arguments[0]).0 = id(999),
            _ => scalar_argument_mut(&mut call.arguments[0])
                .1
                .locations
                .clear(),
        }
        assert_ne!(legalized_operation_plan_identity(&proposed), identity);
    }
}
