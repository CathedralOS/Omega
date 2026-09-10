use super::*;

#[test]
fn graph_scalar_projection_retains_boolean_sources_beside_integer_homes() {
    let value = |raw| ValueId::new(raw).unwrap();
    let operation = OperationId::new(1).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let integer_home = TargetUnitScalarHomeRequirement {
        defining_operation: operation,
        source_value: value(1),
        scalar_type: ScalarType::Integer(integer),
        shape: ValueShape::integer(2, 2),
    };
    let comparison_home = TargetUnitScalarHomeRequirement {
        defining_operation: OperationId::new(2).unwrap(),
        source_value: value(2),
        scalar_type: ScalarType::Boolean,
        shape: ValueShape::integer(1, 1),
    };
    let block_parameter = target_operations::TargetScalarBlockValue {
        block: BlockId::new(1).unwrap(),
        value: value(4),
        scalar_type: ScalarType::Boolean,
    };
    let live = LiveDefinitions {
        structural_homes: BTreeMap::new(),
        nonreturning: false,
        integers: BTreeMap::from([(value(1), KnownUnitInteger::Home(integer_home))]),
        booleans: BTreeMap::from([(value(3), (operation, true))]),
        scalar_homes: BTreeMap::from([(value(2), comparison_home)]),
        ieee_float_constants: BTreeMap::new(),
        scalar_block_parameters: BTreeMap::from([(value(4), block_parameter)]),
        views: BTreeMap::new(),
        block_views: BTreeSet::new(),
        owned_arrivals: BTreeSet::new(),
        lengths: BTreeMap::new(),
    };
    let plan = evaluate_call_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![ValueShape::integer(1, 1)],
            result: None,
        },
    )
    .unwrap();
    let parameter = ScalarAbiValue {
        value: value(5),
        scalar_type: ScalarType::Boolean,
        placement: plan.parameters[0].clone(),
    };
    let mut projected = scalar_values(&live, &[parameter]).unwrap();
    assert_eq!(projected.len(), 5);
    assert_eq!(
        projected
            .remove(&value(2))
            .unwrap()
            .into_expression(value(2))
            .unwrap(),
        TargetScalarExpression::Boolean(TargetBooleanExpression::ScalarHome(comparison_home))
    );
    assert_eq!(
        projected.remove(&value(3)),
        Some(KnownScalar::Boolean(true))
    );
    assert_eq!(
        projected.remove(&value(4)),
        Some(KnownScalar::BooleanRuntime(
            TargetBooleanExpression::BlockParameter(block_parameter)
        ))
    );
    assert!(matches!(projected.remove(&value(5)),
        Some(KnownScalar::BooleanRuntime(TargetBooleanExpression::Parameter { source_value, parameter_index: 0, .. })) if source_value == value(5)));
    assert_eq!(
        projected
            .remove(&value(1))
            .unwrap()
            .into_expression(value(1))
            .unwrap(),
        TargetScalarExpression::Integer {
            scalar_type: integer,
            expression: TargetIntegerExpression::ScalarHome(integer_home)
        }
    );

    let mut wrong = live;
    wrong
        .scalar_block_parameters
        .get_mut(&value(4))
        .unwrap()
        .scalar_type = ScalarType::Integer(integer);
    assert_eq!(
        scalar_values(&wrong, &[]),
        Err(LoweringError::ValueTypeMismatch(value(4)))
    );
}
