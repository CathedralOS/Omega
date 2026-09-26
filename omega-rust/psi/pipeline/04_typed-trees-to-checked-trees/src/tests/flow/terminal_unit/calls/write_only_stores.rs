use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::flow::terminal_unit::{
    CheckedScalarExpression, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
    PrimitiveType, checked, machine_named,
};
use crate::tests::front_end::typed_program;

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
    );
    assert!(
        rebuilt
            .for_machine(machine_named(&checked, "Harness::exercise"))
            .is_none(),
        "a shared alias without its exact certificate remains unsupported"
    );
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
    let typed = typed_program(&overlapping);
    assert!(
        lower_typed_trees(typed, &CheckingRequest::settled()).is_err(),
        "overlapping exclusive arguments must reject"
    );
}
