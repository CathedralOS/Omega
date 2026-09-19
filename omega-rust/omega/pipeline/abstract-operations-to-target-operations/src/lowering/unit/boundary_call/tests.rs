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
use calling_conventions::MachineRegister;

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
    let arguments = lower_normalized_foreign_scalar_arguments_with_result(
        boundary,
        &declaration,
        &[first, second],
        &plan,
        &scalar_values,
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
            &[first, second],
            &plan,
            &scalar_values,
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
            &[first, second],
            &plan,
            &scalar_values,
            None,
            Some(&wrong_plan),
            &[],
        )
        .is_err()
    );
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
