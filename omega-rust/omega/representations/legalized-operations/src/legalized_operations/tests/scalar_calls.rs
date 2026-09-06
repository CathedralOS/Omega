use super::*;

fn call(plan: &mut LegalizedOperationPlan) -> &mut LegalizedScalarCallUnitCall {
    plan.scalar_call_unit_functions[0]
        .operations
        .iter_mut()
        .find_map(|operation| {
            if let LegalizedScalarCallUnitOperation::Call(call) = operation {
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
            .enumerate()
            .map(|(index, placement)| LegalizedScalarCallUnitArgument {
                parameter_index: index as u32,
                placement: placement.clone(),
                ..argument.clone()
            })
            .collect();
        assert_eq!(call.validate_shape(), Ok(()));
        if arity > 0 {
            let mut corrupt = call.clone();
            corrupt.arguments[0].parameter_index = 1;
            assert!(corrupt.validate_shape().is_err());
            let mut corrupt = call.clone();
            corrupt.arguments.pop();
            assert!(corrupt.validate_shape().is_err());
            let mut corrupt = call.clone();
            corrupt.arguments[0].placement.locations.clear();
            assert!(corrupt.validate_shape().is_err());
        }
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
            2 => call.arguments[0].parameter_index += 1,
            _ => call.arguments[0].placement.locations.clear(),
        }
        assert_ne!(legalized_operation_plan_identity(&proposed), identity);
    }
}
