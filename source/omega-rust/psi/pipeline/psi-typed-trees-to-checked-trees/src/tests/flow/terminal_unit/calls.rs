//! Structural Unit call-closure and custody tests.

use super::*;
use psi_checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedScalarBinding, CheckedScalarBindingValue,
};

#[test]
fn retains_owned_affine_i64_record_literal_for_direct_unit_call() {
    let checked = checked(
        r#"
        data Packet { value: i64; }

        data Sink {}
        machine Sink::accept(packet: Packet) {}

        data Root {}
        machine Root::enter() {
            let packet: Packet = Packet { value: 7 };
            Sink::accept(move packet);
        }
        "#,
    );
    let root = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::enter"))
        .expect("owned affine scalar-record caller plan");
    assert!(matches!(
        root.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal {
                declaration_ordinal: 0,
                field_identity,
                value: CheckedScalarExpression::IntegerLiteral { literal },
                ..
            },
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                trivial_affine_local_discard_ordinals,
                ..
            }
        ] if field_identity.ends_with("value")
            && literal.value_i64() == Some(7)
            && matches!(structural_arguments.as_slice(), [argument]
                if argument.source_affine_scalar_record_local_declaration_ordinal() == Some(0)
                    && argument.path.is_empty()
                    && argument.access == psi_checked_trees::CheckedStructuralAccess::Owned)
            && trivial_affine_local_discard_ordinals.is_empty()
    ));
}

#[test]
fn retains_exact_byte_literal_for_static_bodyless_boundary() {
    let checked = checked(
        r#"
        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
        }

        data Root {}
        machine Root::enter()
        reaches Console
        {
            Console::write_line("\x80A");
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let boundary = plans
        .boundary_machines
        .iter()
        .find(|boundary| boundary.structural_parameters.len() == 1)
        .expect("write_line boundary structural parameter");
    assert!(boundary.scalar_parameters.is_empty());
    let byte_type = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity == boundary.structural_parameters[0].type_identity)
        .expect("borrowed byte-sequence shape");
    assert!(matches!(
        byte_type.shape,
        CheckedUnitStructuralTypeShape::ByteSequence(
            psi_checked_trees::CheckedByteSequenceCarrier::BorrowedView
        )
    ));
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("literal boundary caller plan");
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        structural_arguments,
        ..
    } = &root.operations[0]
    else {
        panic!("write_line should retain a bodyless boundary call")
    };
    assert!(matches!(
        structural_arguments.as_slice(),
        [argument]
            if argument.source_parameter_index().is_none()
                && argument.path.is_empty()
                && argument.byte_sequence_literal() == Some(&[0x80, b'A'][..])
    ));
}

#[test]
fn retains_owned_structural_result_for_static_bodyless_boundary() {
    let checked = checked(
        r#"
        data ByteRead {
            case Eof;
            case Byte(value: i32);
        }

        boundary trait Console {
            machine read_byte() -> ByteRead
            reaches Console;
        }

        data Root {}
        machine Root::enter()
        reaches Console
        {
            let result: ByteRead = Console::read_byte();
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let boundary = plans
        .boundary_machines
        .iter()
        .find(|boundary| {
            matches!(
                boundary.result,
                CheckedBoundaryMachineResultPlan::Structural { .. }
            )
        })
        .expect("read_byte boundary structural result");
    assert!(matches!(
        &boundary.result,
        CheckedBoundaryMachineResultPlan::Structural {
            multiplicity: Multiplicity::Affine,
            qualifications,
            ..
        } if qualifications.is_empty()
    ));
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("structural boundary-result caller plan");
    assert!(matches!(
        root.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                result,
                discard_result_on_return: true,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. }
        ] if result.type_identity.contains("ByteRead")
            && result.multiplicity == Multiplicity::Affine
    ));
}

#[test]
fn specializes_one_provider_backed_attachment_field_into_exact_boundary_requirements() {
    let checked = checked(
        r#"
        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
            machine exit_process(return_code: i32)
            reaches Console;
        }

        data Main { console: Console; }
        machine Main::main(&mut self)
        reaches Console
        {
            self.console.write_line("Hello");
            self.console.exit_process(0);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let main = plans
        .for_machine(machine_named(&checked, "main"))
        .expect("provider-backed Main should retain a checked Unit plan");
    assert!(main.structural_parameters.is_empty());
    assert_eq!(main.provider_attachment_requirements.len(), 2);
    assert!(
        main.provider_attachment_requirements
            .iter()
            .all(|requirement| requirement.field_identity == "console")
    );
    let attachment = plans
        .structural_types
        .iter()
        .find(|shape| Some(shape.identity.as_str()) == main.attachment_type_identity.as_deref())
        .expect("Main attachment shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &attachment.shape else {
        panic!("Main should remain a record")
    };
    assert!(matches!(
        fields.as_slice(),
        [field]
            if field.identity == "console"
                && matches!(field.field_type,
                    CheckedUnitStructuralFieldType::ProviderBacked { .. })
    ));
}

#[test]
fn composes_conditional_unit_control_with_exact_boundary_call_leaves() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root {}
        machine Root::enter(flag: bool) {
            transition flag { true -> yes() _ -> no() }
            state yes() { Host::exit(1); }
            state no() { Host::exit(2); }
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let machine = plans
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("three-state Unit control and boundary effects compose atomically");
    assert!(plans.for_machine(machine.machine).is_none());
    let [entry, when_true, when_false] = machine.states.as_slice() else {
        panic!("composed Unit plan retains exactly three states")
    };
    assert!(entry.operations.is_empty());
    assert!(matches!(
        entry.terminator,
        psi_checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
    ));
    for leaf in [when_true, when_false] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
        ));
        assert!(matches!(
            leaf.terminator,
            psi_checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnUnit
        ));
    }
}

#[test]
fn composes_closed_guard_with_one_provider_backed_attachment() {
    let checked = checked(
        r#"
        boundary trait Console {
            machine exit_process(return_code: i32)
            reaches Console;
        }
        const PAGE_SIZE: u32 = 64;
        data Main { console: Console; }
        machine Main::main(&mut self)
        reaches Console
        {
            transition PAGE_SIZE == 64 { true -> yes() _ -> no() }
            state yes(&mut self) { self.console.exit_process(70); }
            state no(&mut self) { self.console.exit_process(71); }
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let machine = plans
        .composed_for_machine(machine_named(&checked, "main"))
        .expect("closed guard and provider attachment compose atomically");
    assert!(machine.states[0].scalar_parameters.is_empty());
    assert_eq!(machine.provider_attachment_requirements.len(), 1);
    assert_eq!(
        machine.provider_attachment_requirements[0].field_identity,
        "console"
    );
    assert!(matches!(
        machine.states[0].terminator,
        psi_checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
    ));
}

#[test]
fn composes_one_compile_known_u64_binding_with_exact_boundary_leaves() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root { values: [i32; 5]; }
        machine Root::enter(&mut self) {
            let length: u64 = (self.values[1..4]).len;
            transition length == 3 {
                true -> yes()
                false -> no()
            }
            state yes(&mut self) { Host::exit(1); }
            state no(&mut self) { Host::exit(2); }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("one compile-known scalar local should compose with the exact control graph");
    let [entry, when_true, when_false] = plan.states.as_slice() else {
        panic!("one-local composed Unit plan retains three states")
    };
    assert!(matches!(
        entry.bindings.as_slice(),
        [CheckedScalarBinding {
            statement_ordinal: 0,
            primitive_type: PrimitiveType::U64,
            value: CheckedScalarBindingValue::Expression,
        }]
    ));
    assert!(matches!(
        entry.binding_initializers.as_slice(),
        [CheckedScalarExpression::IntegerLiteral { literal }] if literal.value_u64() == Some(3)
    ));
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true: true_edge,
        when_false: false_edge,
        ..
    } = &entry.terminator
    else {
        panic!("one-local entry remains conditional")
    };
    assert_eq!(true_edge.statement_ordinal, 1);
    assert_eq!(false_edge.statement_ordinal, 2);
    for leaf in [when_true, when_false] {
        assert!(leaf.binding_initializers.is_empty());
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
        ));
    }
}

#[test]
fn rejects_the_whole_composed_control_plan_when_one_leaf_is_unsupported() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Helper {}
        machine Helper::touch() {}
        data Root {}
        machine Root::enter(flag: bool) {
            transition flag { true -> yes() _ -> no() }
            state yes() { Host::exit(1); }
            state no() { Helper::touch(); Helper::touch(); }
        }
        "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "one unsupported leaf must remove the composed plan atomically"
    );
}

#[test]
fn provider_attachment_specialization_rejects_ambiguous_or_unrouted_fields() {
    for source in [
        r#"
        boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        data Main { console: Console; backup: Console; }
        machine Main::main(&mut self) reaches Console {
            self.console.exit_process(0);
        }
        "#,
        r#"
        boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        data Main { console: Console; }
        machine Main::main(&mut self) reaches Console {
            Console::exit_process(0);
        }
        "#,
        r#"
        boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        data Main { console: Console; }
        machine Main::main(&mut self) {}
        "#,
    ] {
        let checked = checked(source);
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&checked, "main"))
                .is_none(),
            "unsupported provider-backed attachment shapes must fail closed"
        );
    }
}

#[test]
fn retains_shared_byte_sequence_forwarding_access() {
    let checked = checked(
        r#"
        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
        }

        data Root {}
        machine Root::enter(text: &[u8])
        reaches Console
        {
            Console::write_line(text);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let enter = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("shared byte-sequence forwarding plan");
    assert_eq!(
        enter.structural_parameters[0].access,
        psi_checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        structural_arguments,
        ..
    } = &enter.operations[0]
    else {
        panic!("shared forwarding should retain its boundary call")
    };
    assert_eq!(
        structural_arguments[0].access,
        psi_checked_trees::CheckedStructuralAccess::SharedBorrow
    );
}

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
        psi_checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert_eq!(
        fill.structural_parameters[0].access,
        psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
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
        psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
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
        psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
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
                statement_index: 0,
                destination_parameter_index: 0,
                value: CheckedScalarExpression::IntegerLiteral { literal },
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 1,
                ..
            },
        ] if literal.value_i64() == Some(2)
            && literal.landing().is_some_and(|landing|
                landing.landed_type == psi_numerics::literals::LandedIntegerType::I32)
    ));

    let enter = plans
        .for_machine(machine_named(&checked, "Root::enter"))
        .expect("mutable caller in the literal-store closure");
    assert_eq!(
        enter.structural_parameters[0].access,
        psi_checked_trees::CheckedStructuralAccess::MutableBorrow
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
                    == psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow)
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
        psi_checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert!(matches!(
        fill.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                destination_parameter_index: 0,
                value: CheckedScalarExpression::IntegerLiteral { literal },
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] if literal.value_i64() == Some(2)
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
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] if coordinate.statement_index == 2
            && coordinate.call_ordinal == 0
            && matches!(structural_arguments.as_slice(), [argument]
                if argument.source_parameter_index() == Some(0)
                    && argument.path.is_empty()
                    && argument.access
                        == psi_checked_trees::CheckedStructuralAccess::MutableBorrow)
    ));

    let mut without_certificate = checked.facts.clone();
    without_certificate
        .borrow
        .reborrow_restored_call_use_certificates = psi_arena::Arena::new();
    let rebuilt = crate::flow::build_checked_unit_effect_plans(
        &checked.typed,
        &without_certificate,
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
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] if coordinate.statement_index == 2
            && coordinate.call_ordinal == 0
            && matches!(structural_arguments.as_slice(), [argument]
                if argument.source_parameter_index() == Some(0)
                    && argument.path.is_empty()
                    && argument.access
                        == psi_checked_trees::CheckedStructuralAccess::MutableBorrow)
    ));

    let mut without_certificate = checked.facts.clone();
    without_certificate
        .borrow
        .reborrow_restored_call_use_certificates = psi_arena::Arena::new();
    let rebuilt = crate::flow::build_checked_unit_effect_plans(
        &checked.typed,
        &without_certificate,
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
                statement_index: 0,
                destination_parameter_index: 0,
                value: CheckedScalarExpression::Boolean(expression),
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 1,
                ..
            },
        ] if matches!(
            expression.as_ref(),
            psi_checked_trees::CheckedBooleanExpression::Constant(true)
        )
    ));
}

#[test]
fn primitive_store_planning_fails_closed_outside_the_literal_whole_write_root() {
    let cases = [
        (
            "runtime replacement",
            r#"
            data Sink {}
            machine Sink::fill(destination: &write i32, replacement: i32) {
                destination = replacement;
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
            "projected record field",
            r#"
            data Cell [copy] { value: i32; }
            data Sink {}
            machine Sink::fill(destination: &write Cell) {
                destination.value = 2;
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

    for (case, source) in cases {
        let checked = checked(source);
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&checked, "Sink::fill"))
                .is_none(),
            "unsupported primitive store shape crossed checked planning: {case}"
        );
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
        psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(argument.path.len(), 2);
    assert!(argument.path.iter().all(|segment| matches!(
        segment,
        psi_checked_trees::CheckedUnitStructuralPathSegment::Field(_)
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
        psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert!(matches!(
        argument.path.as_slice(),
        [
            psi_checked_trees::CheckedUnitStructuralPathSegment::Field(_),
            psi_checked_trees::CheckedUnitStructuralPathSegment::Field(_),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1),
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
        psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        argument.path,
        [psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1)]
    );
}

#[test]
fn retains_exact_two_index_direct_root_write_only_subloan() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(values: &write [[u16; 3]; 2]) {
            Sink::fill(&write values[1][2]);
        }
        "#,
    );
    let forward = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("two-index write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("two-index attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one two-index write-only argument")
    };
    assert_eq!(
        argument.access,
        psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        argument.path,
        [
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(2),
        ]
    );
}

#[test]
fn retains_exact_field_prefixed_two_index_write_only_subloan() {
    let checked = checked(
        r#"
        data Outer [copy] { values: [[u16; 3]; 2]; sibling: u16; }
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.values[1][2]);
        }
        "#,
    );
    let forward = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("field-prefixed two-index write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("field-prefixed two-index attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one field-prefixed two-index write-only argument")
    };
    assert!(matches!(
        argument.path.as_slice(),
        [
            psi_checked_trees::CheckedUnitStructuralPathSegment::Field(_),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(2),
        ]
    ));
}

#[test]
fn retains_exact_three_index_direct_root_write_only_subloan() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(values: &write [[[u16; 4]; 3]; 2]) {
            Sink::fill(&write values[1][2][3]);
        }
        "#,
    );
    let forward = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("three-index write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("three-index attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one three-index write-only argument")
    };
    assert_eq!(
        argument.access,
        psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        argument.path,
        [
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(2),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(3),
        ]
    );
}

#[test]
fn retains_exact_field_prefixed_three_index_write_only_subloan() {
    let checked = checked(
        r#"
        data Outer [copy] { values: [[[u16; 4]; 3]; 2]; sibling: u16; }
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.values[1][2][3]);
        }
        "#,
    );
    let forward = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("field-prefixed three-index write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("field-prefixed three-index attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one field-prefixed three-index write-only argument")
    };
    assert!(matches!(
        argument.path.as_slice(),
        [
            psi_checked_trees::CheckedUnitStructuralPathSegment::Field(_),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(2),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(3),
        ]
    ));
}

#[test]
fn retains_exact_four_index_direct_root_write_only_subloan() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(values: &write [[[[u16; 5]; 4]; 3]; 2]) {
            Sink::fill(&write values[1][2][3][4]);
        }
        "#,
    );
    let forward = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("four-index write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("four-index attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one four-index write-only argument")
    };
    assert_eq!(
        argument.access,
        psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        argument.path,
        [
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(2),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(3),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(4),
        ]
    );
}

#[test]
fn retains_exact_field_prefixed_four_index_write_only_subloan() {
    let checked = checked(
        r#"
        data Outer [copy] { values: [[[[u16; 5]; 4]; 3]; 2]; sibling: u16; }
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.values[1][2][3][4]);
        }
        "#,
    );
    let forward = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("field-prefixed four-index write-only caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &forward.operations[0]
    else {
        panic!("field-prefixed four-index attenuation should retain its checked call")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one field-prefixed four-index write-only argument")
    };
    assert!(matches!(
        argument.path.as_slice(),
        [
            psi_checked_trees::CheckedUnitStructuralPathSegment::Field(_),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(1),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(2),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(3),
            psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(4),
        ]
    ));
}

#[test]
fn write_only_common_field_subloan_does_not_bypass_ordinary_call_shape() {
    for (name, source) in [
        (
            "multiple structural parameters",
            r#"
                data Leaf [copy] { value: u16; }
                data Outer [copy] { leaf: Leaf; sibling: Leaf; }
                data Sink {}
                machine Sink::fill(destination: &write Leaf, other: &write Leaf) {}
                data Root {}
                machine Root::forward(left: &write Outer, right: &write Outer) {
                    Sink::fill(&write left.leaf, &write right.leaf);
                }
            "#,
        ),
        (
            "caller local",
            r#"
                data Leaf [copy] { value: u16; }
                data Outer [copy] { leaf: Leaf; }
                data Sink {}
                machine Sink::fill(destination: &write Leaf) {}
                data Root {}
                machine Root::forward(outer: &write Outer) {
                    let local: Leaf = Leaf { value: 1 };
                    Sink::fill(&write local);
                }
            "#,
        ),
    ] {
        let checked = checked(source);
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&checked, "Root::forward"))
                .is_none(),
            "{name} unexpectedly bypassed the exact one-parameter projected-call referee"
        );
    }
}

#[test]
fn retains_static_boundary_scalar_parameter_and_literal_argument() {
    let checked = checked(
        r#"
        boundary trait Console {
            machine exit_process(return_code: i32)
            reaches Console;
        }

        data Root {}

        machine Root::enter()
        reaches Console
        {
            Console::exit_process(37);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let boundary = plans
        .boundary_machines
        .iter()
        .find(|boundary| !boundary.scalar_parameters.is_empty())
        .expect("scalar boundary declaration should retain a checked plan");
    assert_eq!(boundary.structural_parameters, []);
    assert_eq!(boundary.scalar_parameters.len(), 1);
    assert_eq!(boundary.scalar_parameters[0].source_position, 0);
    assert_eq!(
        boundary.scalar_parameters[0].primitive_type,
        PrimitiveType::I32
    );
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expressions
            .iter()
            .any(|expression| matches!(
                expression.role,
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: 0,
                }
            )),
        "scalar facts: {:#?}",
        checked.facts.values.scalar_expressions.expressions
    );

    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("scalar boundary caller should retain a checked Unit plan");
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        scalar_arguments, ..
    } = &root.operations[0]
    else {
        panic!("root should retain the boundary effect")
    };
    assert_eq!(scalar_arguments.len(), 1);
    assert!(matches!(
        &scalar_arguments[0],
        CheckedScalarExpression::IntegerLiteral { literal }
            if literal.landing().is_some_and(|landing|
                landing.landed_type == psi_numerics::literals::LandedIntegerType::I32)
    ));
}

#[test]
fn selected_console_exit_intrinsic_projects_the_exact_boundary_requirement() {
    let checked = checked(
        r#"
        pub boundary trait Console {
            machine exit_process(return_code: i32)
            reaches Console;
        }

        pub data ConsoleNativeProvider {}
        boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
            satisfies Console::exit_process;

        data Root {}
        machine Root::enter()
        reaches Console
        {
            ConsoleNativeProvider::exit_process(37);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let requirement_symbol = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Console")
        .and_then(|definition| checked.typed.trait_machine_signatures(definition).first())
        .map(|requirement| requirement.symbol)
        .expect("exact Console exit requirement symbol");
    let requirement = plans
        .boundary_machines
        .iter()
        .find(|boundary| boundary.machine == requirement_symbol)
        .expect("exact Console exit requirement");
    let root = plans
        .for_machine(machine_named(&checked, "Root::enter"))
        .expect("selected bodyless intrinsic must not remove its caller plan");
    assert!(matches!(
        root.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::BoundaryCall {
                target_machine,
                scalar_arguments,
                structural_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] if *target_machine == requirement.machine
            && scalar_arguments.len() == 1
            && structural_arguments.is_empty()
    ));
    assert!(
        plans
            .for_machine(machine_named(
                &checked,
                "ConsoleNativeProvider::exit_process"
            ))
            .is_none(),
        "the bodyless intrinsic is a boundary realization, not a checked transitive body"
    );
}

#[test]
fn other_external_mechanisms_signatures_and_names_do_not_rejoin_as_intrinsic_boundaries() {
    for (label, source) in [
        (
            "DllImport mechanism",
            r#"
        boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        data ConsoleNativeProvider {}
        machine ConsoleNativeProvider::exit_process(return_code: i32)
            satisfies Console::exit_process
            via Binding::DllImport("libSystem.B.dylib", "_exit");
        data Root {}
        machine Root::enter() reaches Console {
            ConsoleNativeProvider::exit_process(37);
        }
        "#,
        ),
        (
            "wrong intrinsic signature",
            r#"
        boundary trait Console { machine write_byte(byte: i64) reaches Console; }
        data ConsoleNativeProvider {}
        machine ConsoleNativeProvider::write_byte(byte: i64)
            satisfies Console::write_byte
            via Binding::CompilerIntrinsic;
        data Root {}
        machine Root::enter() reaches Console {
            ConsoleNativeProvider::write_byte(37);
        }
        "#,
        ),
    ] {
        let checked = checked(source);
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&checked, "Root::enter"))
                .is_none(),
            "unsupported {label} unexpectedly rejoined a boundary"
        );
    }
}

#[test]
fn wrong_named_boundary_realization_does_not_rejoin_as_console_intrinsic() {
    let checked = checked(
        r#"
        pub boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        pub data OtherProvider {}
        boundary machine OtherProvider::exit_process(return_code: i32)
            satisfies Console::exit_process;
        data Root {}
        machine Root::enter() reaches Console {
            OtherProvider::exit_process(37);
        }
        "#,
    );
    let console_requirement = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Console")
        .and_then(|definition| checked.typed.trait_machine_signatures(definition).first())
        .map(|requirement| requirement.symbol)
        .expect("exact Console exit requirement symbol");
    let root = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::enter"))
        .expect("ordinary boundary-application custody retains the wrong-named caller");
    assert!(
        matches!(
            root.operations.first(),
            Some(CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. })
                if *target_machine != console_requirement
        ),
        "a same-shaped wrong-named boundary realization must remain its own boundary application rather than rejoin Console intrinsic custody"
    );
    let realization = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_named(&checked, "OtherProvider::exit_process"))
        .expect("wrong-named boundary realization");
    let [state] = checked.typed.machine_states(realization) else {
        panic!("one wrong-named realization state")
    };
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .boundary_machines
            .iter()
            .any(|boundary| boundary.state == state.symbol),
        "the wrong-named declaration keeps ordinary boundary-application custody",
    );
}

#[test]
fn retains_boundary_scalar_result_local_consumed_by_later_unit_call() {
    let checked = checked(
        r#"
        boundary trait Host {
            machine measure(value: i32) -> i32
            reaches Host;
            machine finish(value: i32)
            reaches Host;
        }

        data Main {}

        machine Main::main(&mut self)
        reaches Host
        {
            let result: i32 = Host::measure(70);
            Host::finish(result);
        }
        "#,
    );

    let main = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Main::main"))
        .expect("scalar boundary result flow should retain a complete Unit plan");
    let [
        CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate: result_call,
            result,
            scalar_arguments: result_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate: consumer_call,
            scalar_arguments: consumer_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 2, ..
        },
    ] = main.operations.as_slice()
    else {
        panic!("scalar boundary result flow retained the wrong operation sequence")
    };
    assert_eq!(
        (result_call.statement_index, result_call.call_ordinal),
        (0, 0)
    );
    assert_eq!(result.statement_index, 0);
    assert_eq!(result.binding_ordinal, 0);
    assert_eq!(result.primitive_type, PrimitiveType::I32);
    assert!(matches!(
        result_arguments.as_slice(),
        [CheckedScalarExpression::IntegerLiteral { .. }]
    ));
    assert_eq!(
        (consumer_call.statement_index, consumer_call.call_ordinal),
        (1, 0)
    );
    assert!(matches!(
        consumer_arguments.as_slice(),
        [CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::I32,
        }]
    ));
}

#[test]
fn retains_branch_free_scalar_local_after_boundary_scalar_result() {
    let checked = checked(
        r#"
        boundary trait Host {
            machine measure(value: i32) -> i32
            reaches Host;
            machine finish(value: i32)
            reaches Host;
        }

        data Main {}

        machine Main::main(&mut self)
        reaches Host
        {
            let measured: i32 = Host::measure(70);
            let result: i32 = measured + 0i32;
            Host::finish(result);
        }
        "#,
    );

    let main = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Main::main"))
        .expect("dependent scalar-local flow should retain a complete Unit plan");
    assert!(matches!(
        main.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::BoundaryScalarCall { result: measured, .. },
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value },
            CheckedUnitEffectOperationPlan::BoundaryCall { scalar_arguments, .. },
            CheckedUnitEffectOperationPlan::ReturnUnit { statement_index: 3, .. },
        ] if measured.binding_ordinal == 0
            && result.statement_index == 1
            && result.binding_ordinal == 1
            && result.primitive_type == PrimitiveType::I32
            && matches!(
                value,
                CheckedScalarExpression::IntegerBinary {
                    kind: psi_checked_trees::CheckedIntegerBinaryKind::ExactAdd,
                    primitive_type: PrimitiveType::I32,
                    left,
                    right,
                } if matches!(
                    left.as_ref(),
                    CheckedScalarExpression::Local {
                        position: 0,
                        primitive_type: PrimitiveType::I32,
                    }
                ) && matches!(
                    right.as_ref(),
                    CheckedScalarExpression::IntegerLiteral { .. }
                )
            )
            && matches!(
                scalar_arguments.as_slice(),
                [CheckedScalarExpression::Local {
                    position: 1,
                    primitive_type: PrimitiveType::I32,
                }]
            )
    ));
}

#[test]
fn fences_short_circuit_scalar_local_after_boundary_result() {
    let checked = checked(
        r#"
        boundary trait Host {
            machine measure(value: i32) -> i32
            reaches Host;
            machine finish(value: i32)
            reaches Host;
        }

        data Main {}

        machine Main::main(&mut self)
        reaches Host
        {
            let measured: i32 = Host::measure(70);
            let accepted: bool = measured == 70 && true;
            Host::finish(measured);
        }
        "#,
    );

    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "Main::main"))
            .is_none(),
        "short-circuit control must not enter the branch-free scalar-local carrier",
    );
}

#[test]
fn retains_provider_attached_boundary_scalar_result_and_exact_requirements() {
    let checked = checked(
        r#"
        boundary trait Console {
            machine read_code() -> i32
            reaches Console;
            machine exit_process(return_code: i32)
            reaches Console;
        }

        data Main { console: Console; }

        machine Main::main(&mut self)
        reaches Console
        {
            let result: i32 = self.console.read_code();
            self.console.exit_process(result);
        }
        "#,
    );

    let main = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Main::main"))
        .expect("provider-attached scalar result flow should retain a complete Unit plan");
    let [
        CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            target_machine: producer,
            result,
            ..
        },
        CheckedUnitEffectOperationPlan::BoundaryCall {
            target_machine: consumer,
            scalar_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::ReturnUnit { .. },
    ] = main.operations.as_slice()
    else {
        panic!("provider-attached scalar flow retained the wrong operation sequence")
    };
    assert_eq!(main.provider_attachment_requirements.len(), 2);
    assert!(
        main.provider_attachment_requirements
            .iter()
            .any(|requirement| requirement.boundary == *producer)
    );
    assert!(
        main.provider_attachment_requirements
            .iter()
            .any(|requirement| requirement.boundary == *consumer)
    );
    assert_eq!(result.binding_ordinal, 0);
    assert!(matches!(
        scalar_arguments.as_slice(),
        [CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::I32,
        }]
    ));
}

#[test]
fn retains_static_attached_root_helper_port_and_boundary_settlement() {
    let checked = checked(
        r#"
        pub data Acknowledgement [linear] {
            root: u64;
            provider_execution: u64;
            invocation: u64;
            policy: u64;
            acknowledgement: u64;
        }

        pub domain Acknowledgement::Pending;

        boundary machine Acknowledgement::settle(self)
        reaches PortIo
        requires
            self in Acknowledgement::Pending
        ensures true;

        data Helper {}

        machine Helper::run(acknowledgement: Acknowledgement in Pending)
        reaches PortIo
        {
            asm { out 32, 7 }
            acknowledgement.settle();
        }

        data Root {}

        machine Root::enter(acknowledgement: Acknowledgement in Pending)
        reaches PortIo
        {
            Helper::run(acknowledgement);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root_symbol = machine_named(&checked, "enter");
    let helper_symbol = machine_named(&checked, "run");
    let settle_symbol = machine_named(&checked, "settle");
    let root = plans
        .for_machine(root_symbol)
        .expect("static attached root plan");
    let helper = plans
        .for_machine(helper_symbol)
        .expect("static attached helper plan");
    let settle = plans
        .boundary_for_machine(settle_symbol)
        .expect("boundary settlement plan");

    assert!(
        root.attachment_type_identity
            .as_deref()
            .is_some_and(|identity| identity.contains("Root"))
    );
    assert!(
        helper
            .attachment_type_identity
            .as_deref()
            .is_some_and(|identity| identity.contains("Helper"))
    );
    assert_eq!(root.structural_parameters.len(), 1);
    assert_eq!(helper.structural_parameters.len(), 1);
    assert_eq!(
        root.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(root.structural_parameters[0].qualifications.len(), 1);
    assert_eq!(root.entry_claims.len(), 1);
    assert!(root.entry_claims[0].path.is_empty());
    assert_eq!(helper.entry_claims.len(), 1);
    assert_eq!(settle.structural_parameters.len(), 1);
    assert_eq!(
        settle.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(settle.domain_requirements.len(), 1);
    assert_eq!(settle.domain_requirements[0].argument_index, 0);
    assert!(root.service_reach.transitive.is_valid());
    assert!(helper.service_reach.direct.is_valid());
    assert!(helper.service_reach.transitive.is_valid());
    assert!(settle.service_reach.direct.is_valid());
    assert_ne!(root.contract_report_fingerprint, 0);
    assert_ne!(helper.contract_report_fingerprint, 0);
    assert_ne!(settle.contract_report_fingerprint, 0);

    let port_values = checked
        .facts
        .values
        .values
        .iter()
        .filter_map(|(_, value)| {
            matches!(
                value.origin,
                psi_checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index: 0,
                    role: psi_checked_trees::CheckedValueStatementRole::CallArgument,
                } if machine_symbol == helper_symbol && state_symbol == helper.state
            )
            .then_some(value)
        })
        .collect::<Vec<_>>();
    assert_eq!(port_values.len(), 2);
    assert_eq!(port_values[0].primitive_type, Some(PrimitiveType::U16));
    assert_eq!(port_values[1].primitive_type, Some(PrimitiveType::U8));
    assert_eq!(
        port_values[0]
            .integer_range
            .as_ref()
            .and_then(|range| range.minimum.to_u64()),
        Some(32)
    );
    assert_eq!(
        port_values[1]
            .integer_range
            .as_ref()
            .and_then(|range| range.minimum.to_u64()),
        Some(7)
    );

    let acknowledgement = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("Acknowledgement"))
        .expect("acknowledgement shape");
    let acknowledgement_fields = record_fields(acknowledgement);
    assert_eq!(acknowledgement_fields.len(), 5);
    assert_eq!(
        acknowledgement_fields
            .iter()
            .map(|field| field.identity.as_str())
            .collect::<Vec<_>>(),
        [
            "root",
            "provider_execution",
            "invocation",
            "policy",
            "acknowledgement",
        ]
    );
    assert!(acknowledgement_fields.iter().all(|field| {
        field.field_type == CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64)
    }));

    assert_eq!(root.operations.len(), 2);
    match &root.operations[0] {
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            assert_eq!(coordinate.statement_index, 0);
            assert_eq!(coordinate.call_ordinal, 0);
            assert_eq!(*target_machine, helper_symbol);
            assert_eq!(structural_arguments.len(), 1);
            assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
            assert_eq!(claim_transfers.len(), 1);
            assert_eq!(claim_transfers[0].argument_index, 0);
            assert_eq!(
                claim_transfers[0].claim_identity,
                root.entry_claims[0].claim_identity
            );
        }
        operation => panic!("unexpected root operation: {operation:?}"),
    }
    assert!(matches!(
        root.operations[1],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 1,
            ..
        }
    ));

    assert_eq!(helper.operations.len(), 3);
    assert!(matches!(
        helper.operations[0],
        CheckedUnitEffectOperationPlan::PortWrite {
            coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                statement_index: 0,
                call_ordinal: 0,
            },
            port: 32,
            value: 7,
            ..
        }
    ));
    match &helper.operations[1] {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_machine,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            assert_eq!(coordinate.statement_index, 1);
            assert_eq!(coordinate.call_ordinal, 0);
            assert_eq!(*target_machine, settle_symbol);
            assert_eq!(structural_arguments.len(), 1);
            assert_eq!(completion_receipts.len(), 1);
            assert_eq!(completion_receipts[0].argument_index, 0);
            assert_eq!(
                completion_receipts[0].claim_identity,
                helper.entry_claims[0].claim_identity
            );
        }
        operation => panic!("unexpected helper operation: {operation:?}"),
    }
    assert!(matches!(
        helper.operations[2],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 2,
            ..
        }
    ));
}

#[test]
fn retains_numbered_record_field_custody_for_unit_call_closure() {
    let checked = checked(
        r#"
        pub data Token [linear] { value: u64; }
        pub data Envelope { #7 token: Token; }

        pub domain Envelope::Pending;

        boundary machine Envelope::settle(self)
        reaches PortIo
        requires
            self in Envelope::Pending
        ensures true;

        data Helper {}

        machine Helper::run(envelope: Envelope in Pending)
        reaches PortIo
        {
            envelope.settle();
        }

        data Root {}

        machine Root::enter(envelope: Envelope in Pending)
        reaches PortIo
        {
            Helper::run(envelope);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("aggregate-custody root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("aggregate-custody helper plan");
    assert_eq!(root.entry_claims.len(), 1);
    assert_eq!(
        root.entry_claims[0].path,
        [CheckedUnitStructuralPathSegment::Field("#7".to_owned())]
    );
    assert_eq!(helper.entry_claims.len(), 1);
    assert_eq!(
        helper.entry_claims[0].path,
        [CheckedUnitStructuralPathSegment::Field("#7".to_owned())]
    );
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &root.operations[0]
    else {
        panic!("root should transfer aggregate custody to helper")
    };
    assert_eq!(claim_transfers.len(), 1);
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &helper.operations[0]
    else {
        panic!("helper should settle aggregate custody at the boundary")
    };
    assert_eq!(completion_receipts.len(), 1);
}

#[test]
fn retains_completion_receipt_for_result_bearing_boundary_call() {
    let checked = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self) -> i32
        reaches PortIo
        ensures true;

        data Root {}

        machine Root::enter(receipt: Receipt) -> i32
        reaches PortIo
        {
            let status: i32 = receipt.settle();
            status
        }
        "#,
    );

    let plan = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(machine_named(&checked, "enter"))
        .expect("result-bearing boundary call should retain a checked custody plan");
    assert_eq!(plan.entry_claims.len(), 1);
    assert_eq!(plan.result_type, PrimitiveType::I32);
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &plan.boundary_call
    else {
        panic!("scalar boundary plan should retain one bodyless boundary call")
    };
    assert_eq!(completion_receipts.len(), 1);
    assert_eq!(
        completion_receipts[0].claim_identity,
        plan.entry_claims[0].claim_identity
    );
}

#[test]
fn retains_disjoint_sibling_custody_inside_one_affine_aggregate() {
    let checked = checked(
        r#"
        pub data Token [linear] { value: u64; }
        pub data Envelope { #7 left: Token; #9 right: Token; }

        pub domain Envelope::Pending;

        boundary machine Envelope::settle(self)
        reaches PortIo
        requires
            self in Envelope::Pending
        ensures true;

        data Helper {}

        machine Helper::run(envelope: Envelope in Pending)
        reaches PortIo
        {
            envelope.settle();
        }

        data Root {}

        machine Root::enter(envelope: Envelope in Pending)
        reaches PortIo
        {
            Helper::run(envelope);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("multi-field aggregate root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("multi-field aggregate helper plan");
    for machine in [root, helper] {
        assert_eq!(
            machine.structural_parameters[0].multiplicity,
            Multiplicity::Affine
        );
        assert_eq!(machine.entry_claims.len(), 2);
        assert_eq!(
            machine.entry_claims[0].path,
            [CheckedUnitStructuralPathSegment::Field("#7".to_owned())]
        );
        assert_eq!(
            machine.entry_claims[1].path,
            [CheckedUnitStructuralPathSegment::Field("#9".to_owned())]
        );
    }
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &root.operations[0]
    else {
        panic!("root should transfer both sibling claims to helper")
    };
    assert_eq!(claim_transfers.len(), 2);
    assert!(
        claim_transfers
            .iter()
            .all(|transfer| transfer.argument_index == 0)
    );
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &helper.operations[0]
    else {
        panic!("helper should settle both sibling claims at the boundary")
    };
    assert_eq!(completion_receipts.len(), 2);
    assert!(
        completion_receipts
            .iter()
            .all(|settlement| settlement.argument_index == 0)
    );
}

#[test]
fn retains_nested_record_field_custody_for_unit_call_closure() {
    let checked = checked(
        r#"
        pub data Token [linear] { value: u64; }
        pub data Pocket { #9 token: Token; }
        pub data Envelope { #7 pocket: Pocket; }

        pub domain Envelope::Pending;

        boundary machine Envelope::settle(self)
        reaches PortIo
        requires
            self in Envelope::Pending
        ensures true;

        data Helper {}

        machine Helper::run(envelope: Envelope in Pending)
        reaches PortIo
        {
            envelope.settle();
        }

        data Root {}

        machine Root::enter(envelope: Envelope in Pending)
        reaches PortIo
        {
            Helper::run(envelope);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("nested aggregate root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("nested aggregate helper plan");
    for machine in [root, helper] {
        assert_eq!(
            machine.structural_parameters[0].multiplicity,
            Multiplicity::Affine
        );
        assert_eq!(machine.entry_claims.len(), 1);
        assert_eq!(
            machine.entry_claims[0].path,
            [
                CheckedUnitStructuralPathSegment::Field("#7".to_owned()),
                CheckedUnitStructuralPathSegment::Field("#9".to_owned()),
            ]
        );
    }
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &root.operations[0]
    else {
        panic!("root should transfer nested custody to helper")
    };
    assert_eq!(claim_transfers.len(), 1);
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &helper.operations[0]
    else {
        panic!("helper should settle nested custody at the boundary")
    };
    assert_eq!(completion_receipts.len(), 1);
}

#[test]
fn retains_literal_fixed_array_boundary_settlements_with_sibling_claims() {
    let checked = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Root {}

        machine Root::enter(receipts: [Receipt; 2])
        reaches PortIo
        {
            Receipt::settle(receipts[0]);
            Receipt::settle(receipts[1]);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("literal fixed-array settlement plan");
    assert_eq!(root.structural_parameters.len(), 1);
    assert_eq!(
        root.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(root.entry_claims.len(), 2);
    for (index, claim) in root.entry_claims.iter().enumerate() {
        assert_eq!(claim.parameter_index, 0);
        assert_eq!(
            claim.path,
            [CheckedUnitStructuralPathSegment::FixedIndex(
                u64::try_from(index).unwrap()
            )]
        );
    }

    let array = plans
        .structural_types
        .iter()
        .find(|shape| {
            matches!(
                shape.shape,
                CheckedUnitStructuralTypeShape::FixedArray { .. }
            )
        })
        .expect("fixed-array structural shape");
    let CheckedUnitStructuralTypeShape::FixedArray {
        element_type_identity,
        length,
    } = &array.shape
    else {
        panic!("expected fixed-array shape")
    };
    assert!(element_type_identity.contains("Receipt"));
    assert_eq!(*length, 2);

    assert_eq!(root.operations.len(), 3);
    for (index, operation) in root.operations[..2].iter().enumerate() {
        let CheckedUnitEffectOperationPlan::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = operation
        else {
            panic!("each literal element should settle at the boundary")
        };
        assert_eq!(structural_arguments.len(), 1);
        assert_eq!(
            structural_arguments[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(
                u64::try_from(index).unwrap()
            )]
        );
        assert!(structural_arguments[0].type_identity.contains("Receipt"));
        assert_eq!(completion_receipts.len(), 1);
        assert_eq!(completion_receipts[0].argument_index, 0);
        assert_eq!(
            completion_receipts[0].claim_identity,
            root.entry_claims[index].claim_identity
        );
    }
}

#[test]
fn retains_literal_fixed_array_projection_for_direct_unit_calls_with_sibling_custody() {
    let checked = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Root {}
        machine Root::enter(receipts: [Receipt; 2])
        reaches PortIo
        {
            Helper::run(receipts[0]);
            Helper::run(receipts[1]);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("literal fixed-index ordinary calls should retain a complete checked plan");
    assert!(plans.for_machine(machine_named(&checked, "run")).is_some());
    assert_eq!(root.entry_claims.len(), 2);
    assert_eq!(root.operations.len(), 3);
    for (index, operation) in root.operations[..2].iter().enumerate() {
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } = operation
        else {
            panic!("each literal element should transfer through an ordinary Unit call")
        };
        assert_eq!(
            structural_arguments[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(index as u64)]
        );
        assert_eq!(claim_transfers.len(), 1);
        assert_eq!(claim_transfers[0].argument_index, 0);
        assert_eq!(
            claim_transfers[0].claim_identity,
            root.entry_claims[index].claim_identity
        );
    }
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &root.operations[2]
    else {
        unreachable!()
    };
    assert!(trivial_affine_discards.is_empty());
}

#[test]
fn fences_projected_unit_calls_outside_the_one_parameter_unit_slice() {
    let caller_with_extra_parameter = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Spare {}
        data Root {}
        machine Root::enter(receipts: [Receipt; 1], spare: Spare)
        reaches PortIo
        {
            Helper::run(receipts[0]);
        }
        "#,
    );
    assert!(
        caller_with_extra_parameter
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&caller_with_extra_parameter, "enter"))
            .is_none(),
        "a projected caller with another structural parameter must stay outside the slice"
    );

    let caller_with_scalar_parameter = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Root {}
        machine Root::enter(receipts: [Receipt; 1], flag: bool)
        reaches PortIo
        {
            Helper::run(receipts[0]);
        }
        "#,
    );
    assert!(
        caller_with_scalar_parameter
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&caller_with_scalar_parameter, "enter"))
            .is_none(),
        "a projected caller with a scalar parameter must stay outside the slice"
    );

    let callee_with_two_parameters = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(first: Receipt, second: Receipt)
        reaches PortIo
        {
            Receipt::settle(first);
            Receipt::settle(second);
        }

        data Root {}
        machine Root::enter(receipts: [Receipt; 2])
        reaches PortIo
        {
            Helper::run(receipts[0], receipts[1]);
        }
        "#,
    );
    assert!(
        callee_with_two_parameters
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&callee_with_two_parameters, "enter"))
            .is_none(),
        "a projected call with two callee parameters and arguments must stay outside the slice"
    );
}

#[test]
fn fences_nested_fixed_array_projection_without_partial_plan() {
    let checked = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Root {}
        machine Root::enter(receipts: [[Receipt; 2]; 2])
        reaches PortIo
        {
            Helper::run(receipts[0][0]);
            Helper::run(receipts[0][1]);
            Helper::run(receipts[1][0]);
            Helper::run(receipts[1][1]);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "nested indexed custody must not publish a partial checked plan"
    );
    assert!(
        plans.structural_types.iter().all(|shape| !matches!(
            shape.shape,
            CheckedUnitStructuralTypeShape::FixedArray { .. }
        )),
        "a rejected nested array must not leave a retained placeholder shape"
    );
}

#[test]
fn fences_four_index_direct_shared_projection_without_widening_legacy_calls() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::inspect(value: &u16) {}

        data Root {}
        machine Root::forward(values: &[[[[u16; 2]; 2]; 2]; 2]) {
            Sink::inspect(&values[0][0][0][0]);
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
        "four direct indexes must not widen the legacy non-write projected-call cohort"
    );
}

#[test]
fn fences_dynamic_fixed_array_projection_for_direct_unit_calls() {
    let checked = checked(
        r#"
        data Ticket { value: u64; }

        data Helper {}
        machine Helper::run(ticket: Ticket) {}

        data Root { index: u64 [0..=1]; }
        machine Root::enter(&self, tickets: [Ticket; 2])
        {
            Helper::run(tickets[self.index]);
        }
        "#,
    );

    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "runtime-indexed custody must remain outside the checked terminal slice"
    );
}

#[test]
fn retains_reverse_declaration_affine_discards_on_unit_return() {
    let checked = checked(
        r#"
        data Ticket { value: u64; }
        data Root {}

        machine Root::enter(first: Ticket, second: Ticket) {}
        "#,
    );

    let root = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("affine Unit root plan");
    assert_eq!(root.structural_parameters.len(), 2);
    assert!(root.entry_claims.is_empty());
    let [
        CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_discards,
            ..
        },
    ] = root.operations.as_slice()
    else {
        panic!("affine Unit root should contain only its return edge")
    };
    assert_eq!(trivial_affine_discards, &[1, 0]);
}

#[test]
fn transferred_affine_parameter_is_not_also_discarded_on_return() {
    let checked = checked(
        r#"
        data Ticket { value: u64; }
        data Helper {}
        machine Helper::run(ticket: Ticket) {}
        data Root {}
        machine Root::enter(ticket: Ticket) {
            Helper::run(ticket);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("affine transfer root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("affine transfer helper plan");
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards: root_discards,
        ..
    } = root.operations.last().unwrap()
    else {
        unreachable!()
    };
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards: helper_discards,
        ..
    } = helper.operations.last().unwrap()
    else {
        unreachable!()
    };
    assert!(root_discards.is_empty());
    assert_eq!(helper_discards, &[0]);
}

#[test]
fn omits_nonconstant_port_and_retains_supported_payloadless_sum() {
    let checked = checked(
        r#"
        data DynamicPort { port: u16; }
        machine DynamicPort::write(&mut self)
        reaches PortIo
        {
            asm { out self.port, 7 }
        }

        data SupportedSum {
            case Empty;
        }
        data NestedRoot { value: SupportedSum; }
        machine NestedRoot::run() {}
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .for_machine(machine_named(&checked, "write"))
            .is_none()
    );
    assert!(plans.for_machine(machine_named(&checked, "run")).is_some());
    let sum = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("SupportedSum"))
        .expect("the closed payload-less sum has an exact structural declaration");
    let CheckedUnitStructuralTypeShape::Sum { cases } = &sum.shape else {
        panic!("payload-less sum must not be represented as an empty record")
    };
    assert!(
        matches!(cases.as_slice(), [case] if case.identity == "Empty" && case.fields.is_empty())
    );
    let root = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("NestedRoot"))
        .expect("the enclosing record has an exact structural declaration");
    assert!(matches!(
        record_fields(root),
        [field]
            if matches!(&field.field_type,
                CheckedUnitStructuralFieldType::Structural { type_identity }
                    if type_identity == &sum.identity)
    ));
}

#[test]
fn retains_opaque_erased_field_identity_in_unit_structural_shape() {
    let checked = checked(
        r#"
        data Evidence { case Only; }
        data Certified {
            value: u64;
            proof [erased]: Evidence;
        }
        machine Certified::run(&self) {}
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(plans.for_machine(machine_named(&checked, "run")).is_some());
    let certified = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("Certified"))
        .expect("certified structural shape");
    let certified_fields = record_fields(certified);
    assert_eq!(certified_fields.len(), 2);
    assert_eq!(certified_fields[0].relevance, BindingRelevance::Relevant);
    assert_eq!(certified_fields[1].relevance, BindingRelevance::Erased);
    assert!(matches!(
        &certified_fields[1].field_type,
        CheckedUnitStructuralFieldType::Erased { type_identity }
            if type_identity.contains("Evidence")
    ));
    assert!(
        plans
            .structural_types
            .iter()
            .all(|shape| !shape.identity.contains("Evidence")),
        "opaque erased field carriers do not enter the executable structural graph"
    );
}
