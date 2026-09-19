use crate::lower_typed_trees;
use crate::tests::flow::terminal_unit::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypeShape, Lexer, Multiplicity, PrimitiveType, ResolutionRequest, checked,
    lower_symbol_resolved_trees, machine_named, parse_syntax_trees, resolve,
};

#[test]
fn retains_explicit_mutable_to_write_only_attenuation() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write [u8]) {}

        data Root {}
        machine Root::enter(bytes: &mut [u8]) {
            Sink::fill(&write bytes);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let enter = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("mutable-to-write-only caller plan");
    let fill = plans
        .for_machine(machine_named(&checked, "fill"))
        .expect("write-only callee plan");
    assert_eq!(
        enter.structural_parameters[0].access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert_eq!(
        fill.structural_parameters[0].access,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        structural_arguments,
        ..
    } = &enter.operations[0]
    else {
        panic!("attenuation should retain its checked call")
    };
    assert_eq!(coordinate.statement_index, 0);
    assert_eq!(coordinate.call_ordinal, 0);
    assert_eq!(
        structural_arguments[0].access,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
}

#[test]
fn retains_one_direct_write_only_primitive_literal_store() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write i32) {
            destination = 2;
        }

        data Root {}
        machine Root::enter(destination: &mut i32) {
            Sink::fill(&write destination);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let fill = plans
        .for_machine(machine_named(&checked, "Sink::fill"))
        .expect("literal write-only callee plan");
    assert_eq!(fill.structural_parameters.len(), 1);
    assert_eq!(
        fill.structural_parameters[0].access,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        fill.structural_parameters[0].multiplicity,
        Multiplicity::Unrestricted
    );
    let scalar_shape = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity == fill.structural_parameters[0].type_identity)
        .expect("primitive structural shape");
    assert!(matches!(
        scalar_shape.shape,
        CheckedUnitStructuralTypeShape::PrimitiveScalar(PrimitiveType::I32)
    ));
    assert!(matches!(
        fill.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                path: store_path,
                statement_index: 0,
                destination: checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
                value: checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::IntegerLiteral { literal }),
            },
            CheckedUnitEffectOperationPlan::Complete {
                statement_index: 1,
                ..
            },
        ] if store_path.is_empty() && literal.value_i64() == Some(2)
            && literal.landing().is_some_and(|landing|
                landing.landed_type == numerics::literals::LandedIntegerType::I32)
    ));

    let enter = plans
        .for_machine(machine_named(&checked, "Root::enter"))
        .expect("mutable caller in the literal-store closure");
    assert_eq!(
        enter.structural_parameters[0].access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert!(matches!(
        &enter.operations[0],
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        } if matches!(structural_arguments.as_slice(), [argument]
            if argument.source_parameter_index() == Some(0)
                && argument.path.is_empty()
                && argument.access
                    == checked_trees::CheckedStructuralAccess::WriteOnlyBorrow)
    ));
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(fill.state, 0, CheckedScalarExpressionRole::AssignmentValue,)
            .is_some_and(|value| matches!(
                value,
                CheckedScalarExpression::IntegerLiteral { literal }
                    if literal.value_i64() == Some(2)
            ))
    );
}

#[test]
fn retains_one_direct_mutable_primitive_literal_store() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &mut i32) {
            destination = 2;
        }
        "#,
    );
    let fill = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Sink::fill"))
        .expect("literal store through readable mutable authority");
    assert_eq!(
        fill.structural_parameters[0].access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert!(matches!(
        fill.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                destination: checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
                value: checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::IntegerLiteral { literal }),
                ..
            },
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] if literal.value_i64() == Some(2)
    ));
}

#[test]
fn retains_direct_and_nested_write_only_record_field_literal_stores() {
    let checked = checked(
        r#"
        data Pair { left: u8; right: u16; }
        data Inner { value: u8; }
        data Outer { inner: Inner; }
        data Cell [copy] { prefix: u8; value: u16; }
        data Matrix { prefix: u8; cells: [Cell; 3]; }
        data Sink {}

        machine Sink::direct(pair: &write Pair) {
            pair.left = 7;
        }

        machine Sink::nested(outer: &write Outer) {
            outer.inner.value = 9;
        }

        machine Sink::indexed(matrix: &write Matrix) {
            matrix.cells[2].value = 13;
        }

        machine Sink::mutable(pair: &mut Pair) {
            pair.right = 11;
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let direct = plans
        .for_machine(machine_named(&checked, "Sink::direct"))
        .expect("direct record-field store plan");
    let nested = plans
        .for_machine(machine_named(&checked, "Sink::nested"))
        .expect("nested record-field store plan");
    let indexed = plans
        .for_machine(machine_named(&checked, "Sink::indexed"))
        .expect("literal-indexed record-field store plan");

    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(direct_store),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = direct.operations.as_slice()
    else {
        panic!("direct field store must retain one checked store and return")
    };
    assert_eq!(direct_store.statement_index, 0);
    assert_eq!(direct_store.destination.parameter_position(), Some(0));
    assert!(direct_store.carrier_path.is_empty());
    assert_eq!(direct_store.primitive_type, PrimitiveType::U8);
    assert!(matches!(
        direct_store.value.as_pure().unwrap(),
        CheckedScalarExpression::IntegerLiteral { literal }
            if literal.value_u64() == Some(7)
    ));

    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(nested_store),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = nested.operations.as_slice()
    else {
        panic!("nested field store must retain one checked store and return")
    };
    assert_eq!(nested_store.statement_index, 0);
    assert_eq!(nested_store.destination.parameter_position(), Some(0));
    assert!(matches!(
        nested_store.carrier_path.as_slice(),
        [CheckedUnitStructuralPathSegment::Field(identity)] if !identity.is_empty()
    ));
    assert_eq!(nested_store.primitive_type, PrimitiveType::U8);
    assert!(matches!(
        nested_store.value.as_pure().unwrap(),
        CheckedScalarExpression::IntegerLiteral { literal }
            if literal.value_u64() == Some(9)
    ));

    assert!(matches!(
        indexed.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] if matches!(
            store.carrier_path.as_slice(),
            [
                CheckedUnitStructuralPathSegment::Field(identity),
                CheckedUnitStructuralPathSegment::FixedIndex(2),
            ] if !identity.is_empty()
        ) && store.primitive_type == PrimitiveType::U16
            && matches!(store.value.as_pure().unwrap(),
                CheckedScalarExpression::IntegerLiteral { literal }
                    if literal.value_u64() == Some(13))
    ));

    let mutable = plans
        .for_machine(machine_named(&checked, "Sink::mutable"))
        .expect("mutable record-field store plan");
    assert_eq!(
        mutable.structural_parameters[0].access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert!(matches!(
        mutable.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] if store.carrier_path.is_empty()
            && store.primitive_type == PrimitiveType::U16
            && matches!(store.value.as_pure().unwrap(),
                CheckedScalarExpression::IntegerLiteral { literal }
                    if literal.value_u64() == Some(11))
    ));
}

#[test]
fn retains_one_scalar_result_before_a_projected_write_only_store() {
    let checked = checked(
        r#"
        data Scalar {}
        machine Scalar::identity(value: i32) -> i32
        requires value == value
        ensures result == value
        {
            transition { _ -> value }
        }

        data Pair { prefix: u8; target: i32; }
        data Root {}
        machine Root::enter(destination: &write Pair) {
            let replacement: i32 = Scalar::identity(23);
            destination.target = replacement;
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::enter"))
        .expect("projected scalar-result store plan");
    assert!(matches!(
        plan.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::ScalarCall { result, .. },
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
            CheckedUnitEffectOperationPlan::Complete {
                statement_index: 2,
                ..
            },
        ] if result.statement_index == 0
            && result.binding_ordinal == 0
            && result.primitive_type == PrimitiveType::I32
            && store.statement_index == 1
            && store.destination.parameter_position() == Some(0)
            && store.carrier_path.is_empty()
            && store.primitive_type == PrimitiveType::I32
            && matches!(
                store.value.as_pure().unwrap(),
                CheckedScalarExpression::Local {
                    position: 0,
                    primitive_type: PrimitiveType::I32,
                }
            )
    ));
}

#[test]
fn retains_only_certificate_backed_restored_reference_alias_call() {
    let checked = checked(
        r#"
        data Harness {}
        data Sink {}
        machine Sink::mutate(value: &mut i32) { value = 2; }
        machine Harness::exercise(root: &mut i32) {
            let parent: &mut i32 = &mut root;
            let child: &write i32 = &write parent;
            Sink::mutate(parent);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let exercise = plans
        .for_machine(machine_named(&checked, "Harness::exercise"))
        .expect("checked certificate admits the erased reference aliases");
    assert!(exercise.trivial_affine_locals.is_empty());
    assert!(matches!(
        exercise.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                structural_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] if coordinate.statement_index == 2
            && coordinate.call_ordinal == 0
            && matches!(structural_arguments.as_slice(), [argument]
                if argument.source_parameter_index() == Some(0)
                    && argument.path.is_empty()
                    && argument.access
                        == checked_trees::CheckedStructuralAccess::MutableBorrow)
    ));

    let mut without_certificate = checked.facts.clone();
    without_certificate
        .borrow
        .reborrow_restored_call_use_certificates = arena::Arena::new();
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &without_certificate,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &without_certificate.flow.terminal_boundary_scalar_returns,
            structural_returns: &without_certificate.flow.terminal_structural_scalar_returns,
        },
        &[],
        &[],
    );
    assert!(
        rebuilt
            .for_machine(machine_named(&checked, "Harness::exercise"))
            .is_none(),
        "ordinary local aliases must not acquire heuristic Terminal meaning"
    );
}

#[test]
fn retains_only_certificate_backed_sole_shared_freeze_alias_call() {
    let checked = checked(
        r#"
        data Harness {}
        data Sink {}
        machine Sink::mutate(value: &mut i32) { value = 2; }
        machine Harness::exercise(root: &mut i32) {
            let parent: &mut i32 = &mut root;
            let child: &i32 = &parent;
            Sink::mutate(parent);
        }
        "#,
    );
    let exercise = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Harness::exercise"))
        .expect("sole shared-freeze certificate admits the erased aliases");
    assert!(exercise.trivial_affine_locals.is_empty());
    assert!(matches!(
        exercise.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                structural_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] if coordinate.statement_index == 2
            && coordinate.call_ordinal == 0
            && matches!(structural_arguments.as_slice(), [argument]
                if argument.source_parameter_index() == Some(0)
                    && argument.path.is_empty()
                    && argument.access
                        == checked_trees::CheckedStructuralAccess::MutableBorrow)
    ));

    let mut without_certificate = checked.facts.clone();
    without_certificate
        .borrow
        .reborrow_restored_call_use_certificates = arena::Arena::new();
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &without_certificate,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &without_certificate.flow.terminal_boundary_scalar_returns,
            structural_returns: &without_certificate.flow.terminal_structural_scalar_returns,
        },
        &[],
        &[],
    );
    assert!(
        rebuilt
            .for_machine(machine_named(&checked, "Harness::exercise"))
            .is_none(),
        "a shared alias without its exact certificate remains unsupported"
    );
}

#[test]
fn retains_one_direct_write_only_boolean_literal_store() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write bool) {
            destination = true;
        }

        data Root {}
        machine Root::enter(destination: &mut bool) {
            Sink::fill(&write destination);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let fill = plans
        .for_machine(machine_named(&checked, "Sink::fill"))
        .expect("Boolean-literal write-only callee plan");
    assert!(matches!(
        plans
            .structural_types
            .iter()
            .find(|shape| shape.identity == fill.structural_parameters[0].type_identity)
            .map(|shape| &shape.shape),
        Some(CheckedUnitStructuralTypeShape::PrimitiveScalar(
            PrimitiveType::Bool
        ))
    ));
    assert!(matches!(
        fill.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                path: store_path,
                statement_index: 0,
                destination: checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
                value: checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::Boolean(expression)),
            },
            CheckedUnitEffectOperationPlan::Complete {
                statement_index: 1,
                ..
            },
        ] if store_path.is_empty() && matches!(
            expression.as_ref(),
            checked_trees::CheckedBooleanExpression::Constant(true)
        )
    ));
}

#[test]
fn retains_one_direct_write_only_ieee_float_literal_store() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write f32) {
            destination = 1.25f32;
        }

        data Root {}
        machine Root::enter(destination: &mut f32) {
            Sink::fill(&write destination);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let fill = plans
        .for_machine(machine_named(&checked, "Sink::fill"))
        .expect("IEEE-literal write-only callee plan");
    assert!(matches!(
        plans
            .structural_types
            .iter()
            .find(|shape| shape.identity == fill.structural_parameters[0].type_identity)
            .map(|shape| &shape.shape),
        Some(CheckedUnitStructuralTypeShape::PrimitiveScalar(
            PrimitiveType::F32
        ))
    ));
    assert!(matches!(
        fill.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                path: store_path,
                statement_index: 0,
                destination: checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                    parameter_index: 0
                },
                value: checked_trees::CheckedCallScalarArgument::Pure(
                    CheckedScalarExpression::IeeeFloatLiteral {
                        value: semantic_vocabulary::IeeeFloatValue::Binary32(0x3fa0_0000),
                    }
                ),
            },
            CheckedUnitEffectOperationPlan::Complete {
                statement_index: 1,
                ..
            },
        ] if store_path.is_empty()
    ));
}

#[test]
fn retains_a_later_direct_write_only_fixed_integer_parameter_store() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write i32, ignored: i32, replacement: i32) {
            destination = replacement;
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let fill = plans
        .for_machine(machine_named(&checked, "Sink::fill"))
        .expect("runtime-parameter write-only callee plan");
    assert!(matches!(
        fill.scalar_parameters.as_slice(),
        [ignored, replacement] if ignored.source_position == 1
            && ignored.primitive_type == PrimitiveType::I32
            && replacement.source_position == 2
            && replacement.primitive_type == PrimitiveType::I32
    ));
    assert!(matches!(
        fill.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                path: store_path,
                statement_index: 0,
                destination: checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                    parameter_index: 0
                },
                value: checked_trees::CheckedCallScalarArgument::Pure(
                    CheckedScalarExpression::Parameter {
                        position: 1,
                        primitive_type: PrimitiveType::I32,
                    }
                ),
            },
            CheckedUnitEffectOperationPlan::Complete {
                statement_index: 1,
                ..
            },
        ] if store_path.is_empty()
    ));
}

#[test]
fn scalar_store_planning_retains_computed_sources_and_multiple_stores_in_order() {
    let cases = [
        (
            "computed runtime replacement",
            r#"
            data Sink {}
            machine Sink::fill(destination: &write i32, replacement: i32) {
                destination = replacement ^ 1i32;
            }
            "#,
        ),
        (
            "computed Boolean replacement",
            r#"
            data Sink {}
            machine Sink::fill(destination: &write bool) {
                destination = !true;
            }
            "#,
        ),
        (
            "more than one store",
            r#"
            data Sink {}
            machine Sink::fill(destination: &write i32) {
                destination = 2;
                destination = 3;
            }
            "#,
        ),
    ];

    for (case_index, (case, source)) in cases.into_iter().enumerate() {
        let checked = checked(source);
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "Sink::fill"))
            .unwrap_or_else(|| panic!("missing ordinary store sequence: {case}"));
        let (last, stores) = plan.operations.split_last().unwrap();
        assert!(
            matches!(last, CheckedUnitEffectOperationPlan::Complete { statement_index, .. }
            if *statement_index as usize == stores.len())
        );
        assert_eq!(stores.len(), if case_index == 2 { 2 } else { 1 });
        for (ordinal, operation) in stores.iter().enumerate() {
            let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                path: store_path,
                statement_index,
                destination:
                    checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
                value,
            } = operation
            else {
                panic!("exact parameter store: {case}");
            };
            assert!(store_path.is_empty());
            assert_eq!(*statement_index as usize, ordinal);
            let checked_trees::CheckedCallScalarArgument::Pure(value) = value else {
                panic!("retained pure store operand");
            };
            match case_index {
                0 => {
                    let CheckedScalarExpression::IntegerBinary {
                        kind: checked_trees::CheckedIntegerBinaryKind::BitwiseXor,
                        primitive_type: PrimitiveType::I32,
                        left,
                        right,
                    } = value
                    else {
                        panic!("retained XOR");
                    };
                    assert!(matches!(
                        left.as_ref(),
                        CheckedScalarExpression::Parameter {
                            position: 0,
                            primitive_type: PrimitiveType::I32
                        }
                    ));
                    assert!(
                        matches!(right.as_ref(), CheckedScalarExpression::IntegerLiteral { literal } if literal.value_i64() == Some(1))
                    );
                }
                1 => {
                    assert_eq!(
                        value,
                        &CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Not(
                            Box::new(CheckedBooleanExpression::Constant(true))
                        ),))
                    );
                }
                _ => assert!(
                    matches!(value, CheckedScalarExpression::IntegerLiteral { literal }
                    if literal.value_i64() == Some(2 + ordinal as i64))
                ),
            }
        }
    }
}

#[test]
fn scalar_store_planning_retains_short_circuit_replacement() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write bool, replacement: bool) {
            destination = replacement && true;
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Sink::fill"))
        .expect("short-circuit replacement uses ordinary scalar evaluation");
    let [
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            path: store_path,
            statement_index: 0,
            destination:
                checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
            value,
        },
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 1, ..
        },
    ] = plan.operations.as_slice()
    else {
        panic!("{:#?}", plan.operations);
    };
    assert!(store_path.is_empty());
    match value {
        checked_trees::CheckedCallScalarArgument::Pure(value) => assert_eq!(
            Some(value),
            checked.facts.values.scalar_expressions.expression_at(
                plan.state,
                0,
                CheckedScalarExpressionRole::AssignmentValue
            )
        ),
        checked_trees::CheckedCallScalarArgument::Computation(value) => {
            let roots = &checked.facts.values.scalar_computations;
            assert!(
                roots
                    .roots
                    .iter()
                    .any(|(_, root)| root.machine == plan.machine
                        && root.state == plan.state
                        && root.statement_ordinal == 0
                        && root.role == CheckedScalarExpressionRole::AssignmentValue
                        && root.root == *value)
            );
            assert_eq!(roots.nodes.get(*value).primitive_type, PrimitiveType::Bool);
        }
    }
}

#[test]
fn retains_exact_write_only_common_field_subloan() {
    let checked = checked(
        r#"
        data Leaf [copy] { value: u16; }
        data Inner [copy] { leaf: Leaf; sibling: u16; }
        data Outer [copy] { inner: Inner; other: Inner; }

        data Sink {}
        machine Sink::fill(destination: &write Leaf) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.inner.leaf);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let forward = plans
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("write-only projected caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("projected attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one projected write-only argument")
    };
    assert_eq!(argument.source_parameter_index(), Some(0));
    assert_eq!(
        argument.access,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(argument.path.len(), 2);
    assert!(argument.path.iter().all(|segment| matches!(
        segment,
        checked_trees::CheckedUnitStructuralPathSegment::Field(_)
    )));
}

#[test]
fn retains_exact_literal_indexed_write_only_subloan() {
    let checked = checked(
        r#"
        data Inner [copy] { values: [u16; 2]; sibling: u16; }
        data Outer [copy] { inner: Inner; other: Inner; }

        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.inner.values[1]);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let forward = plans
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("literal-indexed write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("literal-indexed attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one literal-indexed write-only argument")
    };
    assert_eq!(argument.source_parameter_index(), Some(0));
    assert_eq!(
        argument.access,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert!(matches!(
        argument.path.as_slice(),
        [
            checked_trees::CheckedUnitStructuralPathSegment::Field(_),
            checked_trees::CheckedUnitStructuralPathSegment::Field(_),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1),
        ]
    ));
}

#[test]
fn retains_exact_direct_root_literal_indexed_write_only_subloan() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(values: &write [u16; 2]) {
            Sink::fill(&write values[1]);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let forward = plans
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("direct-root literal-indexed write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("direct-root literal-indexed attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one direct-root literal-indexed write-only argument")
    };
    assert_eq!(argument.source_parameter_index(), Some(0));
    assert_eq!(
        argument.access,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        argument.path,
        [checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
            1
        )]
    );
}

#[test]
fn retains_finite_literal_index_suffix_for_direct_root_write_only_subloan() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(values: &write [[[[[[u16; 7]; 6]; 5]; 4]; 3]; 2]) {
            Sink::fill(&write values[1][2][3][4][5][6]);
        }
        "#,
    );
    let forward = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("finite literal-index write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("finite literal-index attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one finite literal-index write-only argument")
    };
    assert_eq!(
        argument.access,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        argument.path,
        [
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(2),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(3),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(4),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(5),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(6),
        ]
    );
}

#[test]
fn retains_finite_literal_index_suffix_after_write_only_field_prefix() {
    let checked = checked(
        r#"
        data Outer [copy] { values: [[[[[[u16; 7]; 6]; 5]; 4]; 3]; 2]; sibling: u16; }
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.values[1][2][3][4][5][6]);
        }
        "#,
    );
    let forward = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("field-prefixed finite literal-index write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("field-prefixed finite literal-index attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one field-prefixed finite literal-index write-only argument")
    };
    assert!(matches!(
        argument.path.as_slice(),
        [
            checked_trees::CheckedUnitStructuralPathSegment::Field(_),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(2),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(3),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(4),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(5),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(6),
        ]
    ));
}

#[test]
fn retains_scalar_parameter_beside_projected_write_only_argument() {
    let checked = checked(
        r#"
        data Outer [copy] { values: [[[[[[u16; 7]; 6]; 5]; 4]; 3]; 2]; sibling: u16; }
        data Sink {}
        machine Sink::fill(destination: &write u16, replacement: u16) {
            destination = replacement;
        }

        data Root {}
        machine Root::forward(outer: &write Outer, replacement: u16) {
            Sink::fill(&write outer.values[1][2][3][4][5][6], replacement);
        }
        "#,
    );
    let forward = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("parameter-bearing projected write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        scalar_arguments,
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("projected write-only call with scalar parameter is retained")
    };
    assert!(matches!(
        scalar_arguments.as_slice(),
        [checked_trees::CheckedCallScalarArgument::Pure(
            checked_trees::CheckedScalarExpression::Parameter { .. }
        )]
    ));
    assert!(matches!(
        structural_arguments.as_slice(),
        [argument]
            if argument.access
                == checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
                && argument.path.len() == 7
    ));
}

#[test]
fn write_only_common_field_subloans_retain_independent_roots() {
    let source = r#"
        data Leaf [copy] { value: u16; }
        data Outer [copy] { leaf: Leaf; sibling: Leaf; }
        data Sink {}
        machine Sink::fill(destination: &write Leaf, other: &write Leaf) {}
        data Root {}
        machine Root::forward(left: &write Outer, right: &write Outer) {
            Sink::fill(&write left.leaf, &write right.leaf);
        }
    "#;
    let checked = checked(source);
    let forward = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("disjoint projected write-only arguments compose");
    let [
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 1, ..
        },
    ] = forward.operations.as_slice()
    else {
        panic!("one call retaining both projected loans followed by ordinary completion");
    };
    assert_eq!(structural_arguments.len(), 2);
    for (position, argument) in structural_arguments.iter().enumerate() {
        assert_eq!(argument.source_parameter_index(), Some(position as u32));
        assert_eq!(
            argument.access,
            checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
        );
        assert_eq!(
            argument.path,
            vec![CheckedUnitStructuralPathSegment::Field("leaf".into())]
        );
    }

    let overlapping = source.replace("&write right.leaf", "&write left.leaf");
    let tokens = Lexer::new(&overlapping).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    assert!(
        lower_typed_trees(typed).is_err(),
        "overlapping exclusive arguments must reject"
    );
}

#[test]
fn write_only_common_field_subloan_does_not_authorize_an_unretained_local() {
    // The `&write local` formation is admitted on exact atoms because `local`
    // is a `mut` binding — but retained-custody authorization still covers
    // declared write-only roots only, so an unretained local records no
    // terminal-unit effect.
    let checked = checked(
        r#"
        data Leaf [copy] { value: u16; }
        data Outer [copy] { leaf: Leaf; }
        data Sink {}
        machine Sink::fill(destination: &write Leaf) {}
        data Root {}
        machine Root::forward(outer: &write Outer) {
            let mut local: Leaf = Leaf { value: 1 };
            Sink::fill(&write local);
        }
    "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "Root::forward"))
            .is_none(),
        "parameter projection support does not manufacture local storage custody"
    );
}
