//! Focused normalized foreign-scalar boundary-call lowering tests.
use super::{
    BTreeMap, BTreeSet, BoundaryMachineId, CallSignature, CallingPolicy, IntegerSign, IntegerType,
    IntegerValue, KnownUnitInteger, LoweringError, MachineId, NativeTarget, OperationId, PlaceId,
    ScalarType, StructuralTypeId, StructuralTypeLookup, TargetStructuralParameter,
    TargetUnitScalarArgumentSource, TargetUnitScalarHomeRequirement, ValueId, ValueLocation,
    ValuePlacement, ValueShape, lower_normalized_foreign_scalar_arguments,
    lower_normalized_foreign_scalar_arguments_with_result, lower_normalized_foreign_scalar_result,
    lower_normalized_foreign_structural_arguments,
};
use crate::lowering::control_flow::scalar_sources::ScalarSources;
use calling_conventions::MachineRegister;
use semantic_vocabulary::{BlockId, IeeeFloatFormat, IeeeFloatValue};

#[derive(Default)]
struct Sources {
    integers: BTreeMap<ValueId, KnownUnitInteger>,
    scalar_homes: BTreeMap<ValueId, TargetUnitScalarHomeRequirement>,
    booleans: BTreeMap<ValueId, (OperationId, bool)>,
    ieee_float_constants: BTreeMap<ValueId, (OperationId, IeeeFloatValue)>,
    scalar_block_parameters: BTreeMap<ValueId, target_operations::TargetScalarBlockValue>,
}

impl Sources {
    fn view(&self) -> ScalarSources<'_> {
        ScalarSources {
            integers: &self.integers,
            scalar_homes: &self.scalar_homes,
            booleans: &self.booleans,
            ieee_float_constants: &self.ieee_float_constants,
            scalar_block_parameters: &self.scalar_block_parameters,
        }
    }
}

fn function(
    parameters: Vec<abstract_operations::AbstractParameter>,
) -> abstract_operations::AbstractFunction {
    abstract_operations::AbstractFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: BlockId::new(1).unwrap(),
        parameters,
        structural_parameters: Vec::new(),
        result: abstract_operations::AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: Vec::new(),
        operations: Vec::new(),
    }
}

fn declaration(
    boundary: BoundaryMachineId,
    scalar_parameters: Vec<ScalarType>,
) -> terminal_psi::BoundaryMachineDeclaration {
    terminal_psi::BoundaryMachineDeclaration {
        parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar; scalar_parameters.len()],
        fixed_service_reach: Vec::new(),
        id: boundary,
        identity: "Foreign::leaf".into(),
        attachment: None,
        scalar_parameters,
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
        crash_routes: Vec::new(),
    }
}

fn entry_plan(
    target: NativeTarget,
    scalar_types: &[IntegerType],
) -> calling_conventions::BoundaryEntryPlan {
    calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_types
                .iter()
                .map(|scalar_type| {
                    let bytes = scalar_type.bits().div_ceil(8);
                    ValueShape::integer(bytes, bytes.next_power_of_two().min(8))
                })
                .collect(),
            result: None,
        },
    )
    .expect("evaluated entry plan")
    .plan()
    .clone()
}

fn interleaved_callback(
    boundary: BoundaryMachineId,
) -> (
    calling_conventions::BoundaryEntryPlan,
    target_operations::TargetNativeCallbackArgument,
) {
    let target = NativeTarget::linux_x64();
    let shape = ValueShape::integer(8, 8);
    let mut plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape, shape, shape],
            result: None,
        },
    )
    .expect("three-slot registrar plan")
    .plan()
    .clone();
    let binder = calling_conventions::StaticMachineBinderId::new(71).unwrap();
    let parameter = calling_conventions::NativeParameterId::new(72).unwrap();
    let requirement = calling_conventions::CallbackRequirementId::new(73).unwrap();
    let destination = calling_conventions::NativePlace::Parameter(parameter);
    plan.call.callback_materializations = vec![calling_conventions::CallbackMaterialization {
        binder,
        destination: destination.clone(),
    }];
    let context = calling_conventions::CallbackMaterializationContext {
        binders: vec![calling_conventions::CallbackBinderRequirement {
            binder,
            requirement,
        }],
        demands: vec![calling_conventions::NativeCallbackDemand {
            destination,
            requirement,
        }],
    };
    let application = calling_conventions::NativeParameterApplication {
        parameter,
        native_ordinal: 1,
        shape,
        placement: plan.call.parameters[1].clone(),
    };
    (
        plan.clone(),
        target_operations::TargetNativeCallbackArgument {
            terminal_operation: OperationId::new(boundary.get()).unwrap(),
            placement_index: 0,
            callback_function: function_identity::MachineFunctionIdentity::default(),
            application,
            registrar_boundary_entry_plan: plan,
            registrar_context: context,
            registrar_application_commitment: [0x55; 32],
        },
    )
}

#[test]
fn interleaved_native_callback_preserves_semantic_sources_at_physical_ordinals_zero_and_two() {
    let boundary = BoundaryMachineId::new(61).unwrap();
    let first = ValueId::new(62).unwrap();
    let second = ValueId::new(63).unwrap();
    let first_operation = OperationId::new(64).unwrap();
    let second_operation = OperationId::new(65).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let declaration = declaration(
        boundary,
        vec![
            ScalarType::Integer(integer_type),
            ScalarType::Integer(integer_type),
        ],
    );
    let scalar_values = BTreeMap::from([
        (
            first,
            KnownUnitInteger::Immediate {
                defining_operation: first_operation,
                scalar_type: integer_type,
                value: IntegerValue::Unsigned(11),
            },
        ),
        (
            second,
            KnownUnitInteger::Immediate {
                defining_operation: second_operation,
                scalar_type: integer_type,
                value: IntegerValue::Unsigned(22),
            },
        ),
    ]);
    let (plan, callback) = interleaved_callback(boundary);
    let function = function(Vec::new());
    let sources = Sources {
        integers: scalar_values,
        ..Default::default()
    };
    let arguments = lower_normalized_foreign_scalar_arguments_with_result(
        boundary,
        &declaration,
        &function,
        &[first, second],
        &plan,
        &sources.view(),
        None,
        Some(&callback),
        &[],
    )
    .expect("one interleaved native-only callback argument");
    assert_eq!(arguments.len(), 2);
    assert_eq!(arguments[0].parameter_index, 0);
    assert_eq!(arguments[0].source_value(), first);
    assert_eq!(arguments[0].placement, plan.call.parameters[0]);
    assert_eq!(arguments[1].parameter_index, 2);
    assert_eq!(arguments[1].source_value(), second);
    assert_eq!(arguments[1].placement, plan.call.parameters[2]);

    let mut wrong_ordinal = callback.clone();
    wrong_ordinal.application.native_ordinal = 2;
    assert!(
        lower_normalized_foreign_scalar_arguments_with_result(
            boundary,
            &declaration,
            &function,
            &[first, second],
            &plan,
            &sources.view(),
            None,
            Some(&wrong_ordinal),
            &[],
        )
        .is_err()
    );

    let mut wrong_plan = callback;
    wrong_plan
        .registrar_boundary_entry_plan
        .call
        .parameters
        .swap(1, 2);
    assert!(
        lower_normalized_foreign_scalar_arguments_with_result(
            boundary,
            &declaration,
            &function,
            &[first, second],
            &plan,
            &sources.view(),
            None,
            Some(&wrong_plan),
            &[],
        )
        .is_err()
    );
}

#[test]
fn normalized_foreign_parameters_and_block_parameters_retain_order_identity_and_destinations() {
    let boundary = BoundaryMachineId::new(301).unwrap();
    let narrow = IntegerType::new(IntegerSign::Signed, 16).unwrap();
    let wide = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let block = BlockId::new(302).unwrap();

    for (target, register_count, first_stack_offset, first_register) in [
        (NativeTarget::windows_x64(), 4, 32, MachineRegister::X86Rcx),
        (NativeTarget::linux_x64(), 6, 0, MachineRegister::X86Rdi),
        (
            NativeTarget::linux_arm64(),
            8,
            0,
            MachineRegister::Aarch64X(0),
        ),
        (
            NativeTarget::macos_arm64(),
            8,
            0,
            MachineRegister::Aarch64X(0),
        ),
    ] {
        // Reverse the incoming order and repeat a source across the register/stack
        // boundary. Neither map order nor the incoming parameter ordinal is the
        // outgoing native argument position.
        let source_ordinals = [9_u32, 8, 7, 6, 5, 4, 3, 2, 1, 0, 9];
        let source_values =
            source_ordinals.map(|ordinal| ValueId::new(400 + u64::from(ordinal)).unwrap());
        let scalar_types =
            source_ordinals.map(|ordinal| if ordinal % 2 == 0 { narrow } else { wide });
        let declaration = declaration(
            boundary,
            scalar_types
                .iter()
                .copied()
                .map(ScalarType::Integer)
                .collect(),
        );
        let plan = entry_plan(target, &scalar_types);
        let mut parameters = BTreeMap::new();
        let mut block_parameters = BTreeMap::new();
        for ((source_value, scalar_type), parameter_index) in source_values
            .into_iter()
            .zip(scalar_types)
            .zip(source_ordinals)
        {
            parameters.insert(
                source_value,
                KnownUnitInteger::Parameter {
                    parameter_index,
                    scalar_type,
                },
            );
            block_parameters.insert(
                source_value,
                KnownUnitInteger::BlockParameter {
                    block,
                    value: source_value,
                    scalar_type,
                },
            );
        }

        for (is_block_parameter, scalar_values) in [(false, &parameters), (true, &block_parameters)]
        {
            let arguments = lower_normalized_foreign_scalar_arguments(
                boundary,
                &declaration,
                &source_values,
                &plan,
                scalar_values,
            )
            .expect("incoming scalar sources retain the evaluated foreign destinations");
            assert_eq!(arguments.len(), source_values.len());
            for (argument_index, argument) in arguments.iter().enumerate() {
                let source_value = source_values[argument_index];
                let scalar_type = ScalarType::Integer(scalar_types[argument_index]);
                let expected_source = if !is_block_parameter {
                    TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: source_ordinals[argument_index],
                        source_value,
                        scalar_type,
                    }
                } else {
                    TargetUnitScalarArgumentSource::BlockParameter(
                        target_operations::TargetScalarBlockValue {
                            block,
                            value: source_value,
                            scalar_type,
                        },
                    )
                };
                assert_eq!(
                    argument.source, expected_source,
                    "source at {argument_index} on {target:?}"
                );
                assert_eq!(argument.source_value(), source_value);
                assert_eq!(argument.scalar_type(), scalar_type);
                assert_eq!(
                    argument.parameter_index,
                    u32::try_from(argument_index).unwrap()
                );
                assert_eq!(argument.placement, plan.call.parameters[argument_index]);
                if argument_index < register_count {
                    assert!(matches!(
                        argument.placement.locations.as_slice(),
                        [ValueLocation::Register { .. }]
                    ));
                } else {
                    let expected_offset = first_stack_offset
                        + 8 * u32::try_from(argument_index - register_count).unwrap();
                    assert!(
                        matches!(
                            argument.placement.locations.as_slice(),
                            [ValueLocation::Stack { stack_byte_offset, value_byte_offset: 0, byte_size, .. }]
                                if *stack_byte_offset == expected_offset
                                    && *byte_size == scalar_types[argument_index].bits().div_ceil(8)
                        ),
                        "stack argument {argument_index} on {target:?}"
                    );
                }
            }
            assert!(matches!(
                arguments[0].placement.locations.as_slice(),
                [ValueLocation::Register { register, .. }] if *register == first_register
            ));
        }
    }
}

#[test]
fn normalized_foreign_parameters_and_block_parameters_reject_wrong_types_and_absent_sources() {
    let boundary = BoundaryMachineId::new(501).unwrap();
    let source = ValueId::new(502).unwrap();
    let block = BlockId::new(503).unwrap();
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let declaration = declaration(boundary, vec![ScalarType::Integer(integer)]);
    for target in [
        NativeTarget::windows_x64(),
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let plan = entry_plan(target, &[integer]);
        for known in [
            KnownUnitInteger::Parameter {
                parameter_index: 3,
                scalar_type: integer,
            },
            KnownUnitInteger::BlockParameter {
                block,
                value: source,
                scalar_type: integer,
            },
        ] {
            let scalar_values = BTreeMap::from([(source, known)]);
            assert!(
                lower_normalized_foreign_scalar_arguments(
                    boundary,
                    &declaration,
                    &[source],
                    &plan,
                    &scalar_values,
                )
                .is_ok(),
                "valid source {known:?} on {target:?}"
            );

            // Equal-width signedness drift must reject even though its physical
            // placement is unchanged. A width mismatch must reject as well.
            for wrong_type in [
                IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                IntegerType::new(IntegerSign::Signed, 64).unwrap(),
            ] {
                let mut wrong_known = known;
                match &mut wrong_known {
                    KnownUnitInteger::Parameter { scalar_type, .. }
                    | KnownUnitInteger::BlockParameter { scalar_type, .. } => {
                        *scalar_type = wrong_type
                    }
                    _ => unreachable!("fixture contains only parameter sources"),
                }
                assert_eq!(
                    lower_normalized_foreign_scalar_arguments(
                        boundary,
                        &declaration,
                        &[source],
                        &plan,
                        &BTreeMap::from([(source, wrong_known)]),
                    ),
                    Err(LoweringError::BoundaryRealizationMismatch(boundary)),
                    "wrong source type on {target:?}"
                );
            }
            let absent = ValueId::new(504).unwrap();
            assert_eq!(
                lower_normalized_foreign_scalar_arguments(
                    boundary,
                    &declaration,
                    &[absent],
                    &plan,
                    &scalar_values,
                ),
                Err(LoweringError::BoundaryRealizationMismatch(boundary)),
                "absent source on {target:?}"
            );
        }
    }
}

fn assert_normalized_foreign_source_identity_is_checked(known: KnownUnitInteger) {
    let boundary = BoundaryMachineId::new(601).unwrap();
    let source = ValueId::new(602).unwrap();
    let substituted = ValueId::new(603).unwrap();
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let declaration = declaration(boundary, vec![ScalarType::Integer(integer)]);
    let mut wrong_known = known;
    match &mut wrong_known {
        KnownUnitInteger::BlockParameter { value, .. } => *value = substituted,
        KnownUnitInteger::Home(home) => home.source_value = substituted,
        _ => unreachable!("fixture contains an embedded source identity"),
    }
    for target in [
        NativeTarget::windows_x64(),
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let plan = entry_plan(target, &[integer]);
        let lowered = lower_normalized_foreign_scalar_arguments(
            boundary,
            &declaration,
            &[source],
            &plan,
            &BTreeMap::from([(source, known)]),
        )
        .expect("matching embedded source identity");
        assert_eq!(lowered[0].source_value(), source);
        assert_eq!(lowered[0].scalar_type(), ScalarType::Integer(integer));
        assert_eq!(lowered[0].placement, plan.call.parameters[0]);

        // Keep the lookup key, type, and destination intact; only the carried
        // source identity changes. Lowering cannot publish that substitution.
        assert_eq!(
            lower_normalized_foreign_scalar_arguments(
                boundary,
                &declaration,
                &[source],
                &plan,
                &BTreeMap::from([(source, wrong_known)]),
            ),
            Err(LoweringError::BoundaryRealizationMismatch(boundary)),
            "substituted source {wrong_known:?} on {target:?}"
        );
    }
}

#[test]
fn normalized_foreign_block_parameter_rejects_mismatched_source_identity() {
    assert_normalized_foreign_source_identity_is_checked(KnownUnitInteger::BlockParameter {
        block: BlockId::new(604).unwrap(),
        value: ValueId::new(602).unwrap(),
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
    });
}

#[test]
fn normalized_foreign_home_rejects_mismatched_source_identity() {
    assert_normalized_foreign_source_identity_is_checked(KnownUnitInteger::Home(
        TargetUnitScalarHomeRequirement {
            defining_operation: OperationId::new(605).unwrap(),
            source_value: ValueId::new(602).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
            shape: ValueShape::integer(4, 4),
        },
    ));
}

#[test]
fn fixed_integer_literal_preserves_source_type_value_order_and_register_placement() {
    let boundary = BoundaryMachineId::new(41).expect("boundary");
    let source = ValueId::new(42).expect("source");
    let constant = OperationId::new(43).expect("constant");
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let declaration = declaration(boundary, vec![ScalarType::Integer(integer_type)]);
    let constants = BTreeMap::from([(
        source,
        KnownUnitInteger::Immediate {
            defining_operation: constant,
            scalar_type: integer_type,
            value: IntegerValue::Signed(-17),
        },
    )]);

    for (target, expected_register) in [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
    ] {
        let plan = entry_plan(target, &[integer_type]);
        let arguments = lower_normalized_foreign_scalar_arguments(
            boundary,
            &declaration,
            &[source],
            &plan,
            &constants,
        )
        .expect("one evaluated literal argument");
        let [argument] = arguments.as_slice() else {
            panic!("one argument")
        };
        assert_eq!(argument.source_value(), source);
        assert_eq!(
            argument.scalar_type(),
            semantic_vocabulary::ScalarType::Integer(integer_type)
        );
        assert_eq!(
            argument.source,
            TargetUnitScalarArgumentSource::IntegerImmediate {
                defining_operation: constant,
                source_value: source,
                scalar_type: integer_type,
                value: IntegerValue::Signed(-17),
            }
        );
        assert_eq!(argument.parameter_index, 0);
        assert_eq!(argument.placement, plan.call.parameters[0]);
        assert!(matches!(
            argument.placement.locations.as_slice(),
            [ValueLocation::Register { register, .. }] if *register == expected_register
        ));
    }
}

#[test]
fn two_fixed_integer_literals_preserve_ordered_occurrence_custody() {
    let boundary = BoundaryMachineId::new(45).expect("boundary");
    let first = ValueId::new(46).expect("first source");
    let second = ValueId::new(47).expect("second source");
    let i16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let declaration = declaration(
        boundary,
        vec![ScalarType::Integer(i16_type), ScalarType::Integer(i64_type)],
    );
    let constants = BTreeMap::from([
        (
            first,
            KnownUnitInteger::Immediate {
                defining_operation: OperationId::new(48).expect("first constant"),
                scalar_type: i16_type,
                value: IntegerValue::Unsigned(513),
            },
        ),
        (
            second,
            KnownUnitInteger::Immediate {
                defining_operation: OperationId::new(49).expect("second constant"),
                scalar_type: i64_type,
                value: IntegerValue::Signed(-29),
            },
        ),
    ]);

    for (target, expected_registers) in [
        (
            NativeTarget::linux_x64(),
            [MachineRegister::X86Rdi, MachineRegister::X86Rsi],
        ),
        (
            NativeTarget::linux_arm64(),
            [MachineRegister::Aarch64X(0), MachineRegister::Aarch64X(1)],
        ),
    ] {
        let plan = entry_plan(target, &[i16_type, i64_type]);
        let arguments = lower_normalized_foreign_scalar_arguments(
            boundary,
            &declaration,
            &[first, second],
            &plan,
            &constants,
        )
        .expect("two evaluated register literal arguments");
        assert_eq!(arguments.len(), 2);
        for (index, (argument, expected_register)) in
            arguments.iter().zip(expected_registers).enumerate()
        {
            assert_eq!(argument.source_value(), [first, second][index]);
            assert_eq!(
                argument.scalar_type(),
                semantic_vocabulary::ScalarType::Integer([i16_type, i64_type][index])
            );
            assert_eq!(argument.parameter_index, index as u32);
            assert_eq!(argument.placement, plan.call.parameters[index]);
            assert!(matches!(
                argument.placement.locations.as_slice(),
                [ValueLocation::Register { register, .. }] if *register == expected_register
            ));
        }
        assert!(matches!(
            arguments[0].source,
            TargetUnitScalarArgumentSource::IntegerImmediate {
                value: IntegerValue::Unsigned(513),
                ..
            }
        ));
        assert!(matches!(
            arguments[1].source,
            TargetUnitScalarArgumentSource::IntegerImmediate {
                value: IntegerValue::Signed(-29),
                ..
            }
        ));

        let mut malformed_stack_plan = plan;
        malformed_stack_plan.call.parameters[1].locations = vec![ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 1,
            byte_size: 8,
            alignment: 8,
        }];
        assert!(
            lower_normalized_foreign_scalar_arguments(
                boundary,
                &declaration,
                &[first, second],
                &malformed_stack_plan,
                &constants,
            )
            .is_err()
        );
    }
}

#[test]
fn zero_argument_leaf_stays_valid_and_scalar_mutations_fail_closed() {
    let boundary = BoundaryMachineId::new(51).expect("boundary");
    let source = ValueId::new(52).expect("source");
    let constant = OperationId::new(53).expect("constant");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let zero_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature::default(),
    )
    .expect("zero-argument plan")
    .plan()
    .clone();
    assert_eq!(
        lower_normalized_foreign_scalar_arguments(
            boundary,
            &declaration(boundary, Vec::new()),
            &[],
            &zero_plan,
            &BTreeMap::new(),
        ),
        Ok(Vec::new())
    );

    let one_parameter_declaration = declaration(boundary, vec![ScalarType::Integer(i32_type)]);
    let plan = entry_plan(NativeTarget::linux_x64(), &[i32_type]);
    let constants = BTreeMap::from([(
        source,
        KnownUnitInteger::Immediate {
            defining_operation: constant,
            scalar_type: i32_type,
            value: IntegerValue::Signed(9),
        },
    )]);
    for (arguments, constants) in [
        (Vec::new(), constants.clone()),
        (vec![source], BTreeMap::new()),
        (
            vec![source],
            BTreeMap::from([(
                source,
                KnownUnitInteger::Immediate {
                    defining_operation: constant,
                    scalar_type: i32_type,
                    value: IntegerValue::Unsigned(9),
                },
            )]),
        ),
    ] {
        assert!(matches!(
            lower_normalized_foreign_scalar_arguments(
                boundary,
                &one_parameter_declaration,
                &arguments,
                &plan,
                &constants,
            ),
            Err(LoweringError::BoundaryRealizationMismatch(actual)) if actual == boundary
        ));
    }

    let mut stack_plan = plan.clone();
    stack_plan.call.parameters[0].locations = vec![ValueLocation::Stack {
        stack_byte_offset: 0,
        value_byte_offset: 0,
        byte_size: 4,
        alignment: 4,
    }];
    assert!(
        lower_normalized_foreign_scalar_arguments(
            boundary,
            &one_parameter_declaration,
            &[source],
            &stack_plan,
            &constants,
        )
        .is_ok(),
        "lowering retains an exact stack placement from the selected plan",
    );
    let mut malformed_stack_plan = stack_plan;
    let [
        ValueLocation::Stack {
            value_byte_offset, ..
        },
    ] = malformed_stack_plan.call.parameters[0]
        .locations
        .as_mut_slice()
    else {
        unreachable!("fixture uses one stack placement")
    };
    *value_byte_offset = 1;
    let mut result_plan = plan.clone();
    result_plan.call.result = Some(plan.call.parameters[0].clone());
    for invalid in [malformed_stack_plan, result_plan] {
        assert!(
            lower_normalized_foreign_scalar_arguments(
                boundary,
                &one_parameter_declaration,
                &[source],
                &invalid,
                &constants,
            )
            .is_err()
        );
    }

    for (target, register_count, expected_last_register) in [
        (NativeTarget::linux_x64(), 6, MachineRegister::X86R9),
        (NativeTarget::linux_arm64(), 8, MachineRegister::Aarch64X(7)),
    ] {
        let parameter_types = vec![i32_type; register_count];
        let parameter_declaration = declaration(
            boundary,
            vec![ScalarType::Integer(i32_type); register_count],
        );
        let plan = entry_plan(target, &parameter_types);
        let arguments = lower_normalized_foreign_scalar_arguments(
            boundary,
            &parameter_declaration,
            &vec![source; register_count],
            &plan,
            &constants,
        )
        .expect("the complete register-resident literal argument bank");
        assert_eq!(
            arguments
                .iter()
                .map(|argument| argument.parameter_index)
                .collect::<Vec<_>>(),
            (0..u32::try_from(register_count).unwrap()).collect::<Vec<_>>()
        );
        assert!(matches!(
            arguments[register_count - 1].placement.locations.as_slice(),
            [ValueLocation::Register { register, .. }] if *register == expected_last_register
        ));

        let stack_argument_count = register_count + 2;
        let stack_argument_declaration = declaration(
            boundary,
            vec![ScalarType::Integer(i32_type); stack_argument_count],
        );
        let stack_argument_plan = entry_plan(target, &vec![i32_type; stack_argument_count]);
        assert!(matches!(
            stack_argument_plan
                .call
                .parameters
                .get(register_count)
                .unwrap()
                .locations
                .as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 0,
                ..
            }]
        ));
        assert!(matches!(
            stack_argument_plan
                .call
                .parameters
                .last()
                .unwrap()
                .locations
                .as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 8,
                ..
            }]
        ));
        let stack_arguments = lower_normalized_foreign_scalar_arguments(
            boundary,
            &stack_argument_declaration,
            &vec![source; stack_argument_count],
            &stack_argument_plan,
            &constants,
        )
        .expect("canonical stack-resident fixed-integer arguments");
        assert_eq!(
            stack_arguments
                .iter()
                .map(|argument| argument.placement.clone())
                .collect::<Vec<_>>(),
            stack_argument_plan.call.parameters,
            "lowering retains the complete canonical register-and-stack plan on {target:?}",
        );
    }
}

#[test]
fn normalized_foreign_results_admit_only_exact_fixed_integer_register_shapes() {
    let boundary = BoundaryMachineId::new(61).expect("boundary");
    let operation = OperationId::new(62).expect("operation");
    let value = ValueId::new(63).expect("value");

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for (sign, bits) in [
            (IntegerSign::Signed, 8),
            (IntegerSign::Unsigned, 8),
            (IntegerSign::Signed, 16),
            (IntegerSign::Unsigned, 16),
            (IntegerSign::Signed, 32),
            (IntegerSign::Unsigned, 32),
            (IntegerSign::Signed, 64),
            (IntegerSign::Unsigned, 64),
        ] {
            let integer = IntegerType::new(sign, bits).unwrap();
            let bytes = bits.div_ceil(8);
            let shape = ValueShape::integer(bytes, bytes.next_power_of_two().min(8));
            let mut declaration = declaration(boundary, Vec::new());
            declaration.result =
                terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Integer(integer));
            let plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: Vec::new(),
                    result: Some(shape),
                },
            )
            .unwrap()
            .plan()
            .clone();
            let result = abstract_operations::AbstractResult {
                value,
                scalar_type: ScalarType::Integer(integer),
            };
            assert_eq!(
                lower_normalized_foreign_scalar_result(
                    boundary,
                    &declaration,
                    operation,
                    Some(result),
                    &plan,
                ),
                Ok(Some(TargetUnitScalarHomeRequirement {
                    defining_operation: operation,
                    source_value: value,
                    scalar_type: semantic_vocabulary::ScalarType::Integer(integer),
                    shape,
                }))
            );

            let mut wrong_sign_declaration = declaration.clone();
            wrong_sign_declaration.result =
                terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Integer(
                    IntegerType::new(
                        match sign {
                            IntegerSign::Signed => IntegerSign::Unsigned,
                            IntegerSign::Unsigned => IntegerSign::Signed,
                        },
                        bits,
                    )
                    .unwrap(),
                ));
            assert!(
                lower_normalized_foreign_scalar_result(
                    boundary,
                    &wrong_sign_declaration,
                    operation,
                    Some(result),
                    &plan,
                )
                .is_err()
            );

            let mut wrong_fragment = plan.clone();
            let ValueLocation::Register { byte_size, .. } =
                &mut wrong_fragment.call.result.as_mut().unwrap().locations[0]
            else {
                unreachable!()
            };
            *byte_size = byte_size.saturating_add(1);
            assert!(
                lower_normalized_foreign_scalar_result(
                    boundary,
                    &declaration,
                    operation,
                    Some(result),
                    &wrong_fragment,
                )
                .is_err()
            );
        }
    }

    for invalid in [
        IntegerType::new(IntegerSign::Signed, 24).unwrap(),
        IntegerType::address(64).unwrap(),
    ] {
        let mut declaration = declaration(boundary, Vec::new());
        declaration.result =
            terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Integer(invalid));
        let plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(NativeTarget::linux_x64()),
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap()
        .plan()
        .clone();
        assert!(
            lower_normalized_foreign_scalar_result(
                boundary,
                &declaration,
                operation,
                Some(abstract_operations::AbstractResult {
                    value,
                    scalar_type: ScalarType::Integer(invalid),
                }),
                &plan,
            )
            .is_err()
        );
    }
}

/// `Main { m: i64; p: Point; q: Point; }` stands in for a caller whose `self`
/// receiver owns two flat-record fields behind one machine slot, matching the
/// authored `self.m.shift(&self.p)` probe shape. `p` lands at byte offset 8
/// and `q` at byte offset 16.
fn flat_record_catalog() -> (
    StructuralTypeId,
    StructuralTypeId,
    abstract_operations::StructuralTypeCatalog,
) {
    let point = StructuralTypeId::new(201).unwrap();
    let main = StructuralTypeId::new(202).unwrap();
    let i32_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let i64_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    let mut next_field = 203_u64;
    let mut field = |identity: &str, field_type: terminal_psi::StructuralFieldType| {
        let declaration = terminal_psi::StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(next_field).unwrap(),
            identity: identity.to_owned(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type,
        };
        next_field += 1;
        declaration
    };
    let catalog = abstract_operations::StructuralTypeCatalog::from(vec![
        terminal_psi::StructuralTypeDeclaration {
            id: point,
            identity: "Point".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![
                    field("x", terminal_psi::StructuralFieldType::Scalar(i32_scalar)),
                    field("y", terminal_psi::StructuralFieldType::Scalar(i32_scalar)),
                ],
            },
        },
        terminal_psi::StructuralTypeDeclaration {
            id: main,
            identity: "Main".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![
                    field("m", terminal_psi::StructuralFieldType::Scalar(i64_scalar)),
                    field("p", terminal_psi::StructuralFieldType::Structural(point)),
                    field("q", terminal_psi::StructuralFieldType::Structural(point)),
                ],
            },
        },
    ]);
    (point, main, catalog)
}

fn caller_receiver(place: PlaceId, root: StructuralTypeId) -> TargetStructuralParameter {
    TargetStructuralParameter {
        place,
        structural_type: root,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        access: terminal_psi::StructuralAccess::MutableBorrow,
        projected_qualifications: Vec::new(),
        shape: ValueShape::borrowed_reference(24, 8),
        placement: ValuePlacement {
            shape: ValueShape::integer(8, 8),
            locations: vec![ValueLocation::Register {
                register: MachineRegister::Aarch64X(19),
                value_byte_offset: 0,
                byte_size: 8,
            }],
        },
    }
}

fn structural_formal(
    position: u32,
    structural_type: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
) -> terminal_psi::StructuralParameterDeclaration {
    terminal_psi::StructuralParameterDeclaration {
        place: PlaceId::new(9_000 + u64::from(position)).unwrap(),
        position,
        is_self: false,
        structural_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn pointer_plan(target: NativeTarget, count: usize) -> calling_conventions::BoundaryEntryPlan {
    calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); count],
            result: None,
        },
    )
    .expect("pointer-word entry plan")
    .plan()
    .clone()
}

#[test]
fn borrowed_flat_record_arguments_preserve_source_custody_and_plan_positions() {
    let boundary = BoundaryMachineId::new(211).unwrap();
    let machine = MachineId::new(212).unwrap();
    let caller_place = PlaceId::new(213).unwrap();
    let (point, main, catalog) = flat_record_catalog();
    let structural_types = StructuralTypeLookup::new(&catalog);
    let receiver = caller_receiver(caller_place, main);
    let parameters_by_place = BTreeMap::from([(caller_place, &receiver)]);
    let mut declaration = declaration(boundary, Vec::new());
    declaration.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural; 2];
    declaration.structural_parameters = vec![
        structural_formal(0, point, terminal_psi::StructuralAccess::SharedBorrow),
        structural_formal(1, point, terminal_psi::StructuralAccess::SharedBorrow),
    ];
    let arguments = vec![
        terminal_psi::StructuralArgument {
            place: caller_place,
            path: vec![terminal_psi::StructuralPathSegment::Field("p".into())],
            access: terminal_psi::StructuralAccess::SharedBorrow,
        },
        terminal_psi::StructuralArgument {
            place: caller_place,
            path: vec![terminal_psi::StructuralPathSegment::Field("q".into())],
            access: terminal_psi::StructuralAccess::SharedBorrow,
        },
    ];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = pointer_plan(target, 2);
        let mut shape_cache = BTreeMap::new();
        let mut active = BTreeSet::new();
        let lowered = lower_normalized_foreign_structural_arguments(
            boundary,
            machine,
            target,
            &declaration,
            &arguments,
            &plan,
            &structural_types,
            &parameters_by_place,
            &mut shape_cache,
            &mut active,
            None,
            &[],
        )
        .expect("two borrowed flat-record arguments");
        assert_eq!(lowered.len(), 2);
        for (index, (field, offset)) in [("p", 8_u32), ("q", 16_u32)].into_iter().enumerate() {
            let argument = &lowered[index];
            assert_eq!(argument.place, caller_place);
            assert_eq!(
                argument.access,
                terminal_psi::StructuralAccess::SharedBorrow
            );
            assert_eq!(
                argument.path.as_slice(),
                [terminal_psi::StructuralPathSegment::Field(field.to_owned())]
            );
            assert_eq!(argument.root_structural_type, main);
            assert_eq!(argument.structural_type, point);
            assert_eq!(argument.shape, ValueShape::borrowed_reference(8, 4));
            assert_eq!(argument.source_byte_offset, offset);
            assert_eq!(argument.fixed_array_length, None);
            assert_eq!(argument.element_stride, None);
            assert_eq!(
                argument.source,
                target_operations::TargetStructuralArgumentSource::Placement(
                    receiver.placement.clone()
                )
            );
            assert_eq!(argument.destination, plan.call.parameters[index]);
            assert!(matches!(
                argument.destination.locations.as_slice(),
                [ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size: 8,
                    ..
                }]
            ));
        }
    }
}

#[test]
fn normalized_foreign_owned_aggregate_arguments_retain_whole_place_and_plan_transport() {
    let boundary = BoundaryMachineId::new(231).unwrap();
    let machine = MachineId::new(232).unwrap();
    let caller_place = PlaceId::new(233).unwrap();
    let quad = StructuralTypeId::new(241).unwrap();
    let f32_scalar = ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32);
    let mut next_field = 242_u64;
    let mut field = |identity: &str| {
        let declaration = terminal_psi::StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(next_field).unwrap(),
            identity: identity.to_owned(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: terminal_psi::StructuralFieldType::Scalar(f32_scalar),
        };
        next_field += 1;
        declaration
    };
    let catalog = abstract_operations::StructuralTypeCatalog::from(vec![
        terminal_psi::StructuralTypeDeclaration {
            id: quad,
            identity: "Quad".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![field("a"), field("b"), field("c"), field("d")],
            },
        },
    ]);
    let structural_types = StructuralTypeLookup::new(&catalog);
    let source = TargetStructuralParameter {
        place: caller_place,
        structural_type: quad,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        access: terminal_psi::StructuralAccess::Owned,
        projected_qualifications: Vec::new(),
        shape: ValueShape::integer(16, 4),
        placement: ValuePlacement {
            shape: ValueShape::integer(16, 4),
            locations: vec![ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 16,
                alignment: 4,
            }],
        },
    };
    let parameters_by_place = BTreeMap::from([(caller_place, &source)]);
    let mut declaration = declaration(boundary, Vec::new());
    declaration.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
    declaration.structural_parameters = vec![structural_formal(
        0,
        quad,
        terminal_psi::StructuralAccess::Owned,
    )];
    let arguments = vec![terminal_psi::StructuralArgument {
        place: caller_place,
        path: Vec::new(),
        access: terminal_psi::StructuralAccess::Owned,
    }];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let abi_shape = ValueShape::homogeneous_float_aggregate(4, 4);
        let plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![abi_shape],
                result: None,
            },
        )
        .expect("owned aggregate entry plan")
        .plan()
        .clone();
        let lowered = lower_normalized_foreign_structural_arguments(
            boundary,
            machine,
            target,
            &declaration,
            &arguments,
            &plan,
            &structural_types,
            &parameters_by_place,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
            None,
            &[],
        )
        .expect("owned aggregate argument lowers");
        let [argument] = lowered.as_slice() else {
            panic!("expected exactly one structural argument")
        };
        assert_eq!(argument.place, caller_place);
        assert_eq!(argument.access, terminal_psi::StructuralAccess::Owned);
        assert!(argument.path.is_empty());
        assert_eq!(argument.root_structural_type, quad);
        assert_eq!(argument.structural_type, quad);
        assert_eq!(argument.shape, ValueShape::integer(16, 4));
        assert_eq!(argument.source_byte_offset, 0);
        assert_eq!(argument.fixed_array_length, None);
        assert_eq!(argument.element_stride, None);
        assert_eq!(
            argument.source,
            target_operations::TargetStructuralArgumentSource::Placement(source.placement.clone())
        );
        assert_eq!(argument.destination, plan.call.parameters[0]);
    }

    // A field projection, a borrowed-class destination, and a size-mismatched
    // plan each fail closed for an owned formal.
    let mut projected = arguments[0].clone();
    projected.path = vec![terminal_psi::StructuralPathSegment::Field("a".into())];
    let point_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 4)],
            result: None,
        },
    )
    .expect("size-mismatched entry plan")
    .plan()
    .clone();
    let borrowed_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![ValueShape::borrowed_reference(16, 4)],
            result: None,
        },
    )
    .expect("borrowed-class entry plan")
    .plan()
    .clone();
    let x64 = NativeTarget::linux_x64();
    let aggregate_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(x64),
        &CallSignature {
            parameters: vec![ValueShape::homogeneous_float_aggregate(4, 4)],
            result: None,
        },
    )
    .expect("aggregate entry plan")
    .plan()
    .clone();
    for (argument, plan) in [
        (projected, &aggregate_plan),
        (arguments[0].clone(), &point_plan),
        (arguments[0].clone(), &borrowed_plan),
    ] {
        assert!(
            lower_normalized_foreign_structural_arguments(
                boundary,
                machine,
                x64,
                &declaration,
                std::slice::from_ref(&argument),
                plan,
                &structural_types,
                &parameters_by_place,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                None,
                &[],
            )
            .is_err()
        );
    }
}

#[test]
fn owned_aggregate_argument_from_call_result_admits_affine_home() {
    let boundary = BoundaryMachineId::new(281).unwrap();
    let machine = MachineId::new(282).unwrap();
    let result_place = PlaceId::new(283).unwrap();
    let producer = OperationId::new(284).unwrap();
    let (point, _main, catalog) = flat_record_catalog();
    let structural_types = StructuralTypeLookup::new(&catalog);
    let parameters_by_place = BTreeMap::new();
    let mut declaration = declaration(boundary, Vec::new());
    declaration.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
    let mut formal = structural_formal(0, point, terminal_psi::StructuralAccess::Owned);
    formal.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
    declaration.structural_parameters = vec![formal];
    let arguments = vec![terminal_psi::StructuralArgument {
        place: result_place,
        path: Vec::new(),
        access: terminal_psi::StructuralAccess::Owned,
    }];
    let result_shape = ValueShape::integer(8, 4);
    let record_result = terminal_psi::StructuralOperationResult {
        place: result_place,
        structural_type: point,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let result_placement = ValuePlacement {
        shape: result_shape,
        locations: vec![ValueLocation::Register {
            register: MachineRegister::X86Rax,
            value_byte_offset: 0,
            byte_size: 8,
        }],
    };
    let operations = vec![target_operations::TargetUnitOperation::Call {
        origin: target_operations::NativeCallOrigin::Authored,
        psi_operation: producer,
        callee: MachineId::new(285).unwrap(),
        call_plan: calling_conventions::CallPlan {
            policy: CallingPolicy::native_for_target(NativeTarget::linux_x64()),
            parameters: Vec::new(),
            result: Some(result_placement),
            callback_materializations: Vec::new(),
            ordinary_clobbers: calling_conventions::RegisterSet::default(),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: calling_conventions::EntryControl::CallReturn,
        },
        result: target_operations::TargetCallResult::Structural {
            result: record_result.clone(),
            callee_result: terminal_psi::StructuralResultDeclaration {
                place: result_place,
                structural_type: point,
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                reference_sources: Vec::new(),
            },
            result_home: Some(target_operations::TargetStructuralHomeRequirement {
                origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation: producer,
                    result: record_result,
                },
                layout: target_operations::TargetStructuralHomeLayout::Aggregate(result_shape),
            }),
            reference_results: Vec::new(),
            returned_claim_transfers: Vec::new(),
        },
        scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    }];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![result_shape],
                result: None,
            },
        )
        .expect("owned aggregate entry plan")
        .plan()
        .clone();
        let lowered = lower_normalized_foreign_structural_arguments(
            boundary,
            machine,
            target,
            &declaration,
            &arguments,
            &plan,
            &structural_types,
            &parameters_by_place,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
            None,
            &operations,
        )
        .expect("call-result aggregate argument lowers");
        let [argument] = lowered.as_slice() else {
            panic!("expected exactly one structural argument")
        };
        assert_eq!(argument.place, result_place);
        assert_eq!(argument.access, terminal_psi::StructuralAccess::Owned);
        assert!(argument.path.is_empty());
        assert_eq!(argument.root_structural_type, point);
        assert_eq!(argument.structural_type, point);
        assert_eq!(argument.shape, result_shape);
        assert_eq!(argument.source_byte_offset, 0);
        assert_eq!(
            argument.source,
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer,
            }
        );
        assert_eq!(argument.destination, plan.call.parameters[0]);
    }

    // The same place must still fail closed when the formal keeps the
    // unrestricted contract a re-readable caller parameter carries: the
    // consumed-once result home cannot satisfy it.
    let mut unrestricted = declaration.clone();
    unrestricted.structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Unrestricted;
    let unrestricted_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![result_shape],
            result: None,
        },
    )
    .expect("unrestricted owned aggregate entry plan")
    .plan()
    .clone();
    assert!(
        lower_normalized_foreign_structural_arguments(
            boundary,
            machine,
            NativeTarget::linux_x64(),
            &unrestricted,
            &arguments,
            &unrestricted_plan,
            &structural_types,
            &parameters_by_place,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
            None,
            &operations,
        )
        .is_err()
    );
}

#[test]
fn normalized_foreign_borrowed_view_descriptors_admit_whole_place_and_stored_field() {
    let boundary = BoundaryMachineId::new(261).unwrap();
    let machine = MachineId::new(262).unwrap();
    let descriptor_place = PlaceId::new(263).unwrap();
    let holder_place = PlaceId::new(264).unwrap();
    let bytes = StructuralTypeId::new(265).unwrap();
    let holder = StructuralTypeId::new(266).unwrap();
    let i64_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    let mut next_field = 267_u64;
    let mut field = |identity: &str, field_type: terminal_psi::StructuralFieldType| {
        let declaration = terminal_psi::StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(next_field).unwrap(),
            identity: identity.to_owned(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type,
        };
        next_field += 1;
        declaration
    };
    let catalog = abstract_operations::StructuralTypeCatalog::from(vec![
        terminal_psi::StructuralTypeDeclaration {
            id: bytes,
            identity: "bytes".into(),
            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        },
        terminal_psi::StructuralTypeDeclaration {
            id: holder,
            identity: "Holder".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![
                    field(
                        "slice",
                        terminal_psi::StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView,
                        ),
                    ),
                    field(
                        "tail",
                        terminal_psi::StructuralFieldType::Scalar(i64_scalar),
                    ),
                ],
            },
        },
    ]);
    let structural_types = StructuralTypeLookup::new(&catalog);
    let descriptor_source = TargetStructuralParameter {
        place: descriptor_place,
        structural_type: bytes,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        access: terminal_psi::StructuralAccess::SharedBorrow,
        projected_qualifications: Vec::new(),
        shape: ValueShape::borrowed_reference(16, 8),
        placement: ValuePlacement {
            shape: ValueShape::integer(16, 8),
            locations: vec![ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 16,
                alignment: 8,
            }],
        },
    };
    let holder_source = TargetStructuralParameter {
        place: holder_place,
        structural_type: holder,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        access: terminal_psi::StructuralAccess::MutableBorrow,
        projected_qualifications: Vec::new(),
        shape: ValueShape::borrowed_reference(24, 8),
        placement: ValuePlacement {
            shape: ValueShape::integer(24, 8),
            locations: vec![ValueLocation::Stack {
                stack_byte_offset: 32,
                value_byte_offset: 0,
                byte_size: 24,
                alignment: 8,
            }],
        },
    };
    let parameters_by_place = BTreeMap::from([
        (descriptor_place, &descriptor_source),
        (holder_place, &holder_source),
    ]);
    let mut declaration = declaration(boundary, Vec::new());
    declaration.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
    declaration.structural_parameters = vec![structural_formal(
        0,
        bytes,
        terminal_psi::StructuralAccess::SharedBorrow,
    )];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(16, 8)],
                result: None,
            },
        )
        .expect("descriptor entry plan")
        .plan()
        .clone();
        for (place, path) in [
            (descriptor_place, Vec::new()),
            (
                holder_place,
                vec![terminal_psi::StructuralPathSegment::Field("slice".into())],
            ),
        ] {
            let arguments = vec![terminal_psi::StructuralArgument {
                place,
                path: path.clone(),
                access: terminal_psi::StructuralAccess::SharedBorrow,
            }];
            let lowered = lower_normalized_foreign_structural_arguments(
                boundary,
                machine,
                target,
                &declaration,
                &arguments,
                &plan,
                &structural_types,
                &parameters_by_place,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                None,
                &[],
            )
            .expect("borrowed-view descriptor argument lowers");
            let [argument] = lowered.as_slice() else {
                panic!("expected exactly one structural argument")
            };
            assert_eq!(argument.place, place);
            assert_eq!(
                argument.access,
                terminal_psi::StructuralAccess::SharedBorrow
            );
            assert_eq!(argument.path, path);
            assert_eq!(argument.structural_type, bytes);
            assert_eq!(argument.shape, ValueShape::borrowed_reference(16, 8));
            assert_eq!(argument.source_byte_offset, 0);
            assert_eq!(argument.fixed_array_length, None);
            assert_eq!(argument.element_stride, None);
            assert_eq!(argument.destination, plan.call.parameters[0]);
            assert_eq!(
                argument.destination.shape,
                ValueShape::integer(16, 8),
                "the descriptor's two words transport by value under the aggregate classification"
            );
            assert!(matches!(
                argument.destination.locations.as_slice(),
                [
                    ValueLocation::Register {
                        value_byte_offset: 0,
                        byte_size: 8,
                        ..
                    },
                    ValueLocation::Register {
                        value_byte_offset: 8,
                        byte_size: 8,
                        ..
                    },
                ]
            ));
        }
    }

    // A whole-place borrow still requires the descriptor root, a descriptor
    // formal must join an indirect destination, and a path landing on an
    // ordinary leaf cannot pass a descriptor formal.
    let x64 = NativeTarget::linux_x64();
    let thin_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(x64),
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("thin-pointer entry plan")
    .plan()
    .clone();
    let descriptor_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(x64),
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8)],
            result: None,
        },
    )
    .expect("descriptor entry plan")
    .plan()
    .clone();
    for (argument, plan) in [
        (
            terminal_psi::StructuralArgument {
                place: holder_place,
                path: Vec::new(),
                access: terminal_psi::StructuralAccess::SharedBorrow,
            },
            &descriptor_plan,
        ),
        (
            terminal_psi::StructuralArgument {
                place: descriptor_place,
                path: Vec::new(),
                access: terminal_psi::StructuralAccess::SharedBorrow,
            },
            &thin_plan,
        ),
        (
            terminal_psi::StructuralArgument {
                place: holder_place,
                path: vec![terminal_psi::StructuralPathSegment::Field("tail".into())],
                access: terminal_psi::StructuralAccess::SharedBorrow,
            },
            &descriptor_plan,
        ),
    ] {
        assert!(
            lower_normalized_foreign_structural_arguments(
                boundary,
                machine,
                x64,
                &declaration,
                std::slice::from_ref(&argument),
                plan,
                &structural_types,
                &parameters_by_place,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                None,
                &[],
            )
            .is_err()
        );
    }
}

#[test]
fn normalized_foreign_structural_mutations_fail_closed() {
    let boundary = BoundaryMachineId::new(221).unwrap();
    let machine = MachineId::new(222).unwrap();
    let caller_place = PlaceId::new(223).unwrap();
    let (point, main, catalog) = flat_record_catalog();
    let structural_types = StructuralTypeLookup::new(&catalog);
    let target = NativeTarget::linux_x64();
    let base_argument = || terminal_psi::StructuralArgument {
        place: caller_place,
        path: vec![terminal_psi::StructuralPathSegment::Field("p".into())],
        access: terminal_psi::StructuralAccess::SharedBorrow,
    };
    let base_declaration = || {
        let mut declaration = declaration(boundary, Vec::new());
        declaration.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
        declaration.structural_parameters = vec![structural_formal(
            0,
            point,
            terminal_psi::StructuralAccess::SharedBorrow,
        )];
        declaration
    };
    let base_plan = pointer_plan(target, 1);
    let receiver = caller_receiver(caller_place, main);
    let parameters_by_place = BTreeMap::from([(caller_place, &receiver)]);

    let lower =
        |declaration: &terminal_psi::BoundaryMachineDeclaration,
         arguments: &[terminal_psi::StructuralArgument],
         plan: &calling_conventions::BoundaryEntryPlan,
         parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
         callback: Option<&target_operations::TargetNativeCallbackArgument>| {
            lower_normalized_foreign_structural_arguments(
                boundary,
                machine,
                target,
                declaration,
                arguments,
                plan,
                &structural_types,
                parameters_by_place,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                callback,
                &[],
            )
        };

    // The admitted control case must succeed before any mutation is trusted.
    assert!(
        lower(
            &base_declaration(),
            &[base_argument()],
            &base_plan,
            &parameters_by_place,
            None,
        )
        .is_ok()
    );

    // Path shape: empty, indexed, referent-crossing, and unknown paths fail.
    for path in [
        Vec::new(),
        vec![terminal_psi::StructuralPathSegment::FixedIndex(0)],
        vec![terminal_psi::StructuralPathSegment::Referent],
        vec![terminal_psi::StructuralPathSegment::Field("missing".into())],
    ] {
        let mut argument = base_argument();
        argument.path = path.clone();
        assert!(
            lower(
                &base_declaration(),
                &[argument],
                &base_plan,
                &parameters_by_place,
                None,
            )
            .is_err(),
            "path {path:?} must fail closed"
        );
    }

    // Semantic custody mismatches: access, multiplicity, qualification,
    // projected type, formal position, and an unknown caller place.
    let mut wrong_access_argument = base_argument();
    wrong_access_argument.access = terminal_psi::StructuralAccess::MutableBorrow;
    let mut owned = base_declaration();
    owned.structural_parameters[0].access = terminal_psi::StructuralAccess::Owned;
    let mut owned_argument = base_argument();
    owned_argument.access = terminal_psi::StructuralAccess::Owned;
    let mut affine = base_declaration();
    affine.structural_parameters[0].multiplicity = terminal_psi::StructuralMultiplicity::Affine;
    let mut wrong_type = base_declaration();
    wrong_type.structural_parameters[0].structural_type = main;
    let mut wrong_position = base_declaration();
    wrong_position.structural_parameters[0].position = 1;
    for (declaration, argument) in [
        (base_declaration(), wrong_access_argument),
        (owned, owned_argument),
        (affine, base_argument()),
        (wrong_type, base_argument()),
        (wrong_position, base_argument()),
    ] {
        assert!(
            lower(
                &declaration,
                &[argument],
                &base_plan,
                &parameters_by_place,
                None,
            )
            .is_err()
        );
    }
    assert_eq!(
        lower(
            &base_declaration(),
            &[base_argument()],
            &base_plan,
            &BTreeMap::new(),
            None,
        ),
        Err(LoweringError::UnknownStructuralArgumentPlace {
            machine,
            place: caller_place,
        })
    );

    // Lane-shape mismatches: counts, a mixed scalar/structural signature, and
    // a native callback present in the structural lane.
    assert!(
        lower(
            &base_declaration(),
            &[],
            &base_plan,
            &parameters_by_place,
            None,
        )
        .is_err()
    );
    assert!(
        lower(
            &declaration(boundary, Vec::new()),
            &[base_argument()],
            &base_plan,
            &parameters_by_place,
            None,
        )
        .is_err()
    );
    let mut mixed = base_declaration();
    mixed.scalar_parameters = vec![ScalarType::Integer(
        IntegerType::new(IntegerSign::Signed, 32).unwrap(),
    )];
    assert!(
        lower(
            &mixed,
            &[base_argument()],
            &base_plan,
            &parameters_by_place,
            None,
        )
        .is_err()
    );
    let (_, callback) = interleaved_callback(boundary);
    assert!(
        lower(
            &base_declaration(),
            &[base_argument()],
            &base_plan,
            &parameters_by_place,
            Some(&callback),
        )
        .is_err()
    );

    // Source-extent mismatch: the projected record must fit its root storage.
    let mut shallow_receiver = caller_receiver(caller_place, main);
    shallow_receiver.shape.byte_size = 8;
    let shallow_map = BTreeMap::from([(caller_place, &shallow_receiver)]);
    assert!(
        lower(
            &base_declaration(),
            &[base_argument()],
            &base_plan,
            &shallow_map,
            None,
        )
        .is_err()
    );

    // Plan mismatches: the evaluated destination must be exactly one
    // pointer-width word at byte offset zero.
    let mut wrong_shape = base_plan.clone();
    wrong_shape.call.parameters[0].shape = ValueShape::integer(4, 4);
    let mut split = base_plan.clone();
    split.call.parameters[0].locations = vec![
        ValueLocation::Register {
            register: MachineRegister::X86Rdi,
            value_byte_offset: 0,
            byte_size: 4,
        },
        ValueLocation::Register {
            register: MachineRegister::X86Rsi,
            value_byte_offset: 4,
            byte_size: 4,
        },
    ];
    let mut fragment_offset = base_plan.clone();
    let [
        ValueLocation::Register {
            value_byte_offset, ..
        },
    ] = fragment_offset.call.parameters[0].locations.as_mut_slice()
    else {
        unreachable!("one register location")
    };
    *value_byte_offset = 1;
    let mut fragment_size = base_plan.clone();
    let [ValueLocation::Register { byte_size, .. }] =
        fragment_size.call.parameters[0].locations.as_mut_slice()
    else {
        unreachable!("one register location")
    };
    *byte_size = 4;
    let mut missing = base_plan.clone();
    missing.call.parameters.clear();
    for plan in [wrong_shape, split, fragment_offset, fragment_size, missing] {
        assert!(
            lower(
                &base_declaration(),
                &[base_argument()],
                &plan,
                &parameters_by_place,
                None,
            )
            .is_err()
        );
    }

    // A canonical stack-resident pointer word is an admitted placement.
    let mut stack_plan = base_plan.clone();
    stack_plan.call.parameters[0].locations = vec![ValueLocation::Stack {
        stack_byte_offset: 0,
        value_byte_offset: 0,
        byte_size: 8,
        alignment: 8,
    }];
    assert!(
        lower(
            &base_declaration(),
            &[base_argument()],
            &stack_plan,
            &parameters_by_place,
            None,
        )
        .is_ok()
    );
}

#[test]
fn normalized_foreign_scalars_admit_boolean_and_ieee_float_shapes() {
    let boundary = BoundaryMachineId::new(41).unwrap();
    let f32_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
    let f64_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
    let bool_type = ScalarType::Boolean;
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let mut declaration = declaration(boundary, vec![f64_type, bool_type, f32_type]);
        declaration.result = terminal_psi::BoundaryMachineResult::Scalar(f64_type);
        let plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![
                    ValueShape::float(8),
                    ValueShape::integer(1, 1),
                    ValueShape::float(4),
                ],
                result: Some(ValueShape::float(8)),
            },
        )
        .expect("evaluated entry plan")
        .plan()
        .clone();
        let parameter = ValueId::new(50).unwrap();
        let flag = ValueId::new(51).unwrap();
        let homed = ValueId::new(52).unwrap();
        let result = ValueId::new(53).unwrap();
        let producing = OperationId::new(60).unwrap();
        let home = TargetUnitScalarHomeRequirement {
            defining_operation: producing,
            source_value: homed,
            scalar_type: f32_type,
            shape: ValueShape::float(4),
        };
        let function = function(vec![abstract_operations::AbstractParameter {
            value: parameter,
            scalar_type: f64_type,
        }]);
        let sources = Sources {
            booleans: BTreeMap::from([(flag, (producing, true))]),
            scalar_homes: BTreeMap::from([(homed, home)]),
            ..Default::default()
        };
        let arguments = lower_normalized_foreign_scalar_arguments_with_result(
            boundary,
            &declaration,
            &function,
            &[parameter, flag, homed],
            &plan,
            &sources.view(),
            Some(ValueShape::float(8)),
            None,
            &[],
        )
        .expect("boolean and floating arguments retain evaluated destinations");
        assert_eq!(arguments.len(), 3);
        assert_eq!(
            arguments[0].source,
            TargetUnitScalarArgumentSource::Parameter {
                parameter_index: 0,
                source_value: parameter,
                scalar_type: f64_type,
            }
        );
        assert_eq!(
            arguments[1].source,
            TargetUnitScalarArgumentSource::BooleanImmediate {
                defining_operation: producing,
                source_value: flag,
                value: true,
            }
        );
        assert_eq!(
            arguments[2].source,
            TargetUnitScalarArgumentSource::Home(home)
        );
        for (argument, declared) in arguments.iter().zip(plan.call.parameters.iter()) {
            assert_eq!(argument.placement, *declared);
        }

        let home = lower_normalized_foreign_scalar_result(
            boundary,
            &declaration,
            producing,
            Some(abstract_operations::AbstractResult {
                value: result,
                scalar_type: f64_type,
            }),
            &plan,
        )
        .expect("floating result home")
        .expect("scalar result");
        assert_eq!(home.scalar_type, f64_type);
        assert_eq!(home.shape, ValueShape::float(8));

        // A substituted source kind fails closed against the declared type.
        let substituted = Sources {
            booleans: BTreeMap::from([(parameter, (producing, true)), (flag, (producing, true))]),
            scalar_homes: sources.scalar_homes.clone(),
            ..Default::default()
        };
        assert!(
            lower_normalized_foreign_scalar_arguments_with_result(
                boundary,
                &declaration,
                &function,
                &[parameter, flag, homed],
                &plan,
                &substituted.view(),
                Some(ValueShape::float(8)),
                None,
                &[],
            )
            .is_err()
        );
        // An f32 declaration against an f64 source is likewise refused.
        let mut mismatched = declaration.clone();
        mismatched.scalar_parameters[0] = f32_type;
        assert!(
            lower_normalized_foreign_scalar_arguments_with_result(
                boundary,
                &mismatched,
                &function,
                &[parameter, flag, homed],
                &plan,
                &sources.view(),
                Some(ValueShape::float(8)),
                None,
                &[],
            )
            .is_err()
        );
    }
}

fn lower_direct_port_read_call(
    target: NativeTarget,
    boundary: BoundaryMachineId,
    operation: &super::AbstractOperation,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    realization: target_operations::BoundaryRealization,
    places: &BTreeMap<PlaceId, TargetStructuralParameter>,
) -> Result<
    (
        Vec<target_operations::TargetUnitOperation>,
        BTreeMap<ValueId, KnownUnitInteger>,
    ),
    LoweringError,
> {
    let function = function(Vec::new());
    let catalog: abstract_operations::StructuralTypeCatalog = Vec::new().into();
    let lookup = StructuralTypeLookup::new(&catalog);
    let boundary_machines = BTreeMap::from([(boundary, declaration)]);
    let settlements = BTreeMap::from([(
        boundary,
        target_operations::BoundarySettlementBinding {
            boundary,
            execution: target_operations::BoundaryExecutionBinding::AdmittedProvider(
                target_operations::ProviderExecutionBinding::from_execution_record(
                    target_operations::ProviderPlanReportIdentity::new(7).unwrap(),
                    11,
                    13,
                    17,
                    23,
                )
                .expect("nonzero provider identities"),
            ),
            realization: target_operations::BoundarySettlementRealization::Builtin(realization),
        },
    )]);
    let parameters_by_place = places
        .iter()
        .map(|(place, parameter)| (*place, parameter))
        .collect();
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let established_byte_sequences = BTreeMap::new();
    let mut scalar_values = BTreeMap::new();
    let mut scalar_homes = BTreeMap::new();
    let sources = Sources::default();
    let mut operations = Vec::new();
    let mut provenance = target_operations::TerminalPsiProvenance::default();
    let mut nonreturning = false;
    super::lower_boundary_call(
        operation,
        &function,
        target,
        &lookup,
        &boundary_machines,
        &settlements,
        &BTreeMap::new(),
        &parameters_by_place,
        &mut shape_cache,
        &mut active,
        &established_byte_sequences,
        &mut scalar_values,
        &mut scalar_homes,
        &sources.booleans,
        &sources.ieee_float_constants,
        &sources.scalar_block_parameters,
        &mut operations,
        &mut provenance,
        &mut nonreturning,
    )?;
    Ok((operations, scalar_values))
}

#[test]
fn direct_port_read_u8_settlement_carries_exact_scalar_home() {
    let boundary = BoundaryMachineId::new(31).unwrap();
    let psi_operation = OperationId::new(32).unwrap();
    let result = ValueId::new(33).unwrap();
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let mut declaration = declaration(boundary, Vec::new());
    declaration.result = terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Integer(u8_type));
    let operation = super::AbstractOperation::BoundaryCall {
        psi_operation,
        result: abstract_operations::AbstractBoundaryResult::Scalar(
            abstract_operations::AbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(u8_type),
            },
        ),
        boundary,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    let (operations, scalar_values) = lower_direct_port_read_call(
        NativeTarget::linux_x64(),
        boundary,
        &operation,
        &declaration,
        target_operations::BoundaryRealization::DirectPortReadU8(
            target_operations::DirectPortReadU8Realization {
                service: semantic_vocabulary::ServiceId::new(41).unwrap(),
                port: 0x3f8,
            },
        ),
        &BTreeMap::new(),
    )
    .expect("exact direct port-read settlement lowers");
    let expected_home = TargetUnitScalarHomeRequirement {
        defining_operation: psi_operation,
        source_value: result,
        scalar_type: ScalarType::Integer(u8_type),
        shape: ValueShape::integer(1, 1),
    };
    assert_eq!(operations.len(), 1);
    let target_operations::TargetUnitOperation::BoundarySettlement {
        result: lowered_result,
        realization: lowered_realization,
        arguments,
        scalar_arguments,
        byte_sequence_arguments,
        completion_claim_sources,
        completion_receipts,
        ..
    } = &operations[0]
    else {
        panic!("expected one boundary settlement operation");
    };
    assert_eq!(
        *lowered_result,
        target_operations::TargetBoundaryResult::Scalar(expected_home)
    );
    assert_eq!(
        *lowered_realization,
        target_operations::BoundaryRealization::DirectPortReadU8(
            target_operations::DirectPortReadU8Realization {
                service: semantic_vocabulary::ServiceId::new(41).unwrap(),
                port: 0x3f8,
            }
        )
    );
    assert!(arguments.is_empty());
    assert!(scalar_arguments.is_empty());
    assert!(byte_sequence_arguments.is_empty());
    assert!(completion_claim_sources.is_empty());
    assert!(completion_receipts.is_empty());
    assert_eq!(
        scalar_values.get(&result),
        Some(&KnownUnitInteger::Home(expected_home))
    );
}

#[test]
fn direct_port_read_u8_settlement_rejects_mutations() {
    let boundary = BoundaryMachineId::new(51).unwrap();
    let psi_operation = OperationId::new(52).unwrap();
    let result = ValueId::new(53).unwrap();
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let mut declaration = declaration(boundary, Vec::new());
    declaration.result = terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Integer(u8_type));
    let realization = || {
        target_operations::BoundaryRealization::DirectPortReadU8(
            target_operations::DirectPortReadU8Realization {
                service: semantic_vocabulary::ServiceId::new(55).unwrap(),
                port: 0x60,
            },
        )
    };
    let call = |result_type: ScalarType| super::AbstractOperation::BoundaryCall {
        psi_operation,
        result: abstract_operations::AbstractBoundaryResult::Scalar(
            abstract_operations::AbstractResult {
                value: result,
                scalar_type: result_type,
            },
        ),
        boundary,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    // A non-u8 scalar result has no honest one-byte port-read row.
    assert!(
        lower_direct_port_read_call(
            NativeTarget::linux_x64(),
            boundary,
            &call(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
            )),
            &declaration,
            realization(),
            &BTreeMap::new(),
        )
        .is_err()
    );
    // The declaration must itself promise the one-byte scalar result.
    let mut unit_result_declaration = declaration.clone();
    unit_result_declaration.result = terminal_psi::BoundaryMachineResult::Unit;
    assert!(
        lower_direct_port_read_call(
            NativeTarget::linux_x64(),
            boundary,
            &call(ScalarType::Integer(u8_type)),
            &unit_result_declaration,
            realization(),
            &BTreeMap::new(),
        )
        .is_err()
    );
    // The `in al, dx` encoding exists on x86-64 only.
    assert!(
        lower_direct_port_read_call(
            NativeTarget::linux_arm64(),
            boundary,
            &call(ScalarType::Integer(u8_type)),
            &declaration,
            realization(),
            &BTreeMap::new(),
        )
        .is_err()
    );
    // Runtime scalar arguments are not part of the closed port-read shape.
    let mut with_argument = call(ScalarType::Integer(u8_type));
    let super::AbstractOperation::BoundaryCall { arguments, .. } = &mut with_argument else {
        panic!("expected a boundary call");
    };
    arguments.push(ValueId::new(56).unwrap());
    assert!(
        lower_direct_port_read_call(
            NativeTarget::linux_x64(),
            boundary,
            &with_argument,
            &declaration,
            realization(),
            &BTreeMap::new(),
        )
        .is_err()
    );
    // A structural argument must rejoin a declared caller place.
    let mut with_structural = call(ScalarType::Integer(u8_type));
    let super::AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut with_structural
    else {
        panic!("expected a boundary call");
    };
    structural_arguments.push(terminal_psi::StructuralArgument {
        place: PlaceId::new(57).unwrap(),
        path: Vec::new(),
        access: terminal_psi::StructuralAccess::SharedBorrow,
    });
    assert!(
        lower_direct_port_read_call(
            NativeTarget::linux_x64(),
            boundary,
            &with_structural,
            &declaration,
            realization(),
            &BTreeMap::new(),
        )
        .is_err()
    );
    // Other closed realizations still cannot carry a scalar result.
    assert!(
        lower_direct_port_read_call(
            NativeTarget::linux_x64(),
            boundary,
            &call(ScalarType::Integer(u8_type)),
            &declaration,
            target_operations::BoundaryRealization::HostedWriteByteI32(
                target_operations::HostedWriteByteI32Realization
            ),
            &BTreeMap::new(),
        )
        .is_err()
    );
}

#[test]
fn direct_port_read_u8_rejects_unit_result() {
    let boundary = BoundaryMachineId::new(61).unwrap();
    let psi_operation = OperationId::new(62).unwrap();
    let mut declaration = declaration(boundary, Vec::new());
    declaration.result = terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
    ));
    let operation = super::AbstractOperation::BoundaryCall {
        psi_operation,
        result: abstract_operations::AbstractBoundaryResult::Unit,
        boundary,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    assert!(
        lower_direct_port_read_call(
            NativeTarget::linux_x64(),
            boundary,
            &operation,
            &declaration,
            target_operations::BoundaryRealization::DirectPortReadU8(
                target_operations::DirectPortReadU8Realization {
                    service: semantic_vocabulary::ServiceId::new(63).unwrap(),
                    port: 0x3f8,
                },
            ),
            &BTreeMap::new(),
        )
        .is_err()
    );
}
