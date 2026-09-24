use crate::tests::flow::terminal_unit::checked_with_service;
use crate::tests::flow::terminal_unit::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment, Multiplicity, PrimitiveType,
    checked, machine_named, record_fields,
};

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
        checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::IntegerLiteral { literal })
            if literal.landing().is_some_and(|landing|
                landing.landed_type == numerics::literals::LandedIntegerType::I32)
    ));
}

#[test]
fn static_boundary_reaches_keep_every_direct_intrinsic_and_requirement_call() {
    // The requirement calls are direct: since c5843e4c43 a nominal binder
    // call (`Selected(0)` under `where machine Selected satisfies
    // Console::exit_process`) is an obligation for specialization, not an
    // executable boundary body, so an open generic entry retains no plan for
    // it and cannot carry the reach-conflict controls below.
    let original = checked(
        r#"
        pub boundary trait Console {
            machine write_byte(byte: i32) reaches Console;
            machine exit_process(code: i32) reaches Console;
            machine unused(code: i32) reaches Console;
        }
        pub data ConsoleNativeProvider {}
        boundary machine ConsoleNativeProvider::write_byte(byte: i32)
            satisfies Console::write_byte;
        data Root {}
        machine Root::enter()
        reaches Console {
            ConsoleNativeProvider::write_byte(1);
            ConsoleNativeProvider::write_byte(2);
            Console::write_byte(3);
            Console::exit_process(0);
            Console::exit_process(0);
        }
        "#,
    );
    let definition = original
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Console")
        .unwrap();
    let signatures = original.typed.trait_machine_signatures(definition);
    let requirement = |name: &str| {
        signatures
            .iter()
            .find(|signature| signature.name.as_str() == name)
            .unwrap()
            .symbol
    };
    let write = requirement("write_byte");
    let exit = requirement("exit_process");
    let unused = requirement("unused");
    let calls = original
        .facts
        .flow
        .control
        .calls
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 5);
    let retained = |plans: &checked_trees::CheckedUnitEffectPlans, symbol| {
        plans
            .boundary_machines
            .iter()
            .any(|plan| plan.machine == symbol)
    };
    let plans = &original.facts.flow.terminal_unit_effects;
    assert!(retained(plans, write));
    assert!(retained(plans, exit));
    assert!(
        !retained(plans, unused),
        "uncalled requirements stay absent"
    );
    for (call_ordinal, handle) in calls.iter().enumerate() {
        let mut changed = original.clone();
        let call = changed.facts.flow.control.calls.get_mut(*handle);
        let empty = language_semantics::ServiceReachRowTable::EMPTY_ROW;
        assert_ne!(call.service_reach.transitive, empty);
        call.service_reach.transitive = empty;
        let plans = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
            &[],
            &[],
        );
        let (conflicted, independent) = if call_ordinal < 3 {
            (write, exit)
        } else {
            (exit, write)
        };
        assert!(
            !retained(&plans, conflicted),
            "call {call_ordinal} must not lose its conflicting reach"
        );
        assert!(
            retained(&plans, independent),
            "another requirement retains its own evidence"
        );
        assert!(!retained(&plans, unused));
    }
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
            CheckedUnitEffectOperationPlan::Complete { .. },
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
fn selected_console_write_byte_intrinsic_projects_the_exact_boundary_requirement() {
    let checked = checked(
        r#"
        pub boundary trait Console {
            machine write_byte(byte: i32)
            reaches Console;
        }

        pub data ConsoleNativeProvider {}
        boundary machine ConsoleNativeProvider::write_byte(byte: i32)
            satisfies Console::write_byte;

        data Root {}
        machine Root::enter()
        reaches Console
        {
            ConsoleNativeProvider::write_byte(37);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let requirement_symbol = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Console")
        .and_then(|definition| {
            checked
                .typed
                .trait_machine_signatures(definition)
                .iter()
                .find(|requirement| requirement.name.as_str() == "write_byte")
        })
        .map(|requirement| requirement.symbol)
        .expect("exact Console write_byte requirement symbol");
    let requirement = plans
        .boundary_machines
        .iter()
        .find(|boundary| boundary.machine == requirement_symbol)
        .expect("exact Console write_byte requirement");
    assert!(requirement.structural_parameters.is_empty());
    assert_eq!(requirement.scalar_parameters.len(), 1);
    assert_eq!(
        requirement.scalar_parameters[0].primitive_type,
        PrimitiveType::I32,
    );
    let root = plans
        .for_machine(machine_named(&checked, "Root::enter"))
        .expect("selected bodyless byte intrinsic must retain its caller plan");
    assert!(matches!(
        root.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::BoundaryCall {
                target_machine,
                scalar_arguments,
                structural_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] if *target_machine == requirement_symbol
            && scalar_arguments.len() == 1
            && structural_arguments.is_empty()
    ));
    assert!(
        plans
            .for_machine(machine_named(&checked, "ConsoleNativeProvider::write_byte"))
            .is_none(),
        "the bodyless byte intrinsic must not acquire a manufactured checked body"
    );
}

#[test]
fn other_external_mechanisms_signatures_and_names_do_not_rejoin_as_intrinsic_boundaries() {
    for (label, source) in [
        (
            "evaluated import mechanism",
            r#"
        boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        data ConsoleNativeProvider {}
        machine exit_binding() -> i32 {
            0
        }
        machine ConsoleNativeProvider::exit_process(return_code: i32)
            satisfies Console::exit_process
            via exit_binding();
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
            via ForeignBinding::CompilerIntrinsic;
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
        CheckedUnitEffectOperationPlan::Complete {
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
        [checked_trees::CheckedCallScalarArgument::Pure(
            CheckedScalarExpression::IntegerLiteral { .. }
        )]
    ));
    assert_eq!(
        (consumer_call.statement_index, consumer_call.call_ordinal),
        (1, 0)
    );
    assert!(matches!(
        consumer_arguments.as_slice(),
        [checked_trees::CheckedCallScalarArgument::Pure(
            CheckedScalarExpression::Local {
                position: 0,
                primitive_type: PrimitiveType::I32,
            }
        )]
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
            CheckedUnitEffectOperationPlan::Complete { statement_index: 3, .. },
        ] if measured.binding_ordinal == 0
            && result.statement_index == 1
            && result.binding_ordinal == 1
            && result.primitive_type == PrimitiveType::I32
            && matches!(
                value.as_pure(),
                Some(CheckedScalarExpression::IntegerBinary {
                    kind: checked_trees::CheckedIntegerBinaryKind::ExactAdd,
                    primitive_type: PrimitiveType::I32,
                    left,
                    right,
                }) if matches!(
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
                [checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::Local {
                    position: 1,
                    primitive_type: PrimitiveType::I32,
                })]
            )
    ));
}

#[test]
fn retains_short_circuit_scalar_local_after_boundary_result() {
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

    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Main::main"))
        .expect("selective scalar local retains the boundary call sequence");
    assert!(matches!(plan.operations.as_slice(), [
        CheckedUnitEffectOperationPlan::BoundaryScalarCall { result: measured, .. },
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result: accepted, value },
        CheckedUnitEffectOperationPlan::BoundaryCall { .. },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] if measured.binding_ordinal == 0
        && accepted.binding_ordinal == 1
        && accepted.statement_index == 1
        && accepted.primitive_type == PrimitiveType::Bool
        && matches!(value.as_pure(), Some(CheckedScalarExpression::Boolean(_)))));
}

#[test]
fn retains_provider_attached_boundary_scalar_result_and_exact_requirements() {
    let checked = checked_with_service(
        r#"
        pub boundary trait Console {
            machine read_code() -> i32
            reaches Console;
            machine exit_process(return_code: i32)
            reaches Console;
        }

        data Main { console: Binding<Console>; }

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
        CheckedUnitEffectOperationPlan::Complete { .. },
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
        [checked_trees::CheckedCallScalarArgument::Pure(
            CheckedScalarExpression::Local {
                position: 0,
                primitive_type: PrimitiveType::I32,
            }
        )]
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
                checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index: 0,
                    role: checked_trees::CheckedValueStatementRole::CallArgument,
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
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 1,
            ..
        }
    ));

    assert_eq!(helper.operations.len(), 3);
    assert!(matches!(
        helper.operations[0],
        CheckedUnitEffectOperationPlan::PortWrite {
            coordinate: checked_trees::CheckedUnitCallCoordinate {
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
        CheckedUnitEffectOperationPlan::Complete {
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
