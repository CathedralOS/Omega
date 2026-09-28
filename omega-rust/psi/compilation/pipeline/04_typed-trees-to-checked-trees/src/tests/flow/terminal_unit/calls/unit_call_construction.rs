use crate::tests::flow::terminal_unit::checked_with_service;
use crate::tests::flow::terminal_unit::{
    CheckedScalarExpression, CheckedUnitEffectOperationPlan, PrimitiveType, checked, machine_named,
};
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedScalarBinding, CheckedScalarBindingValue,
};

#[test]
fn retains_boundary_case_payload_and_mutable_view_on_the_same_state_edge() {
    let checked = checked(
        r#"
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
        }
        machine read_one(out: &mut [u8]) reaches Console {
            transition out.len > 0 { true -> read(out, 7) false -> done() }
            state read(out: &mut [u8], count: u64) {
                let observed: ByteRead = Console::read_byte();
                transition observed {
                    ByteRead::Byte { value } -> store(out, count, value)
                    ByteRead::Eof -> done()
                }
            }
            state store(out: &mut [u8], count: u64, value: i32 [0..=255]) {
                out[0] = value as u8;
            }
            state done() {}
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "read_one"))
        .expect("boundary payload inspection composes with the ordinary state graph");
    let read = &plan.states[1];
    assert!(matches!(
        read.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            discard_result_on_return: false,
            ..
        }]
    ));
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } = &read.terminator else {
        panic!("case edge");
    };
    let byte = cases
        .iter()
        .find(|case| case.case_identity == "Byte")
        .unwrap();
    let checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(byte_successor) = &byte.outcome
    else {
        panic!("the byte arm keeps a named successor")
    };
    assert_eq!(byte_successor.target_state, plan.states[2].state);
    assert_eq!(byte_successor.transfers.len(), 1);
    assert_eq!(byte_successor.transfers[0].target_parameter_index, 0);
    assert_eq!(byte_successor.scalar_arguments.len(), 1);
    assert_eq!(
        byte_successor.scalar_arguments[0].target_scalar_parameter_index,
        0
    );
    assert_eq!(byte.payloads.len(), 1);
    assert_eq!(byte.payloads[0].target_scalar_parameter_index, 1);
    assert_eq!(byte.payloads[0].field_identity, "value");
    let eof = cases
        .iter()
        .find(|case| case.case_identity == "Eof")
        .unwrap();
    let checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(eof_successor) = &eof.outcome
    else {
        panic!("the eof arm keeps a named successor")
    };
    assert!(eof_successor.transfers.is_empty());
    assert!(eof_successor.scalar_arguments.is_empty());
    assert!(eof.payloads.is_empty());
    let mut missing_cleanup = checked.facts.clone();
    missing_cleanup.flow.terminal_structural_control_cleanups = Default::default();
    let rejected = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &missing_cleanup,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &missing_cleanup.flow.terminal_boundary_scalar_returns,
            structural_returns: &missing_cleanup.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        rejected
            .composed_for_machine(machine_named(&checked, "read_one"))
            .is_none(),
        "case inspection must not bypass independently checked edge cleanup"
    );
    for mutation in ["root", "missing", "provenance"] {
        let mut changed = checked.facts.clone();
        let handles = changed
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.state_symbol == read.state
                    && event.source == language_semantics::PermissionEventSource::StateExit
                    && event.kind == language_semantics::PermissionEventKind::AffineDrop
            })
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        assert!(!handles.is_empty());
        for handle in handles {
            let event = changed.flow.ownership.permissions.get_mut(handle);
            match mutation {
                "missing" => event.source = language_semantics::PermissionEventSource::StateEntry,
                "root" => event.root = facts::PlaceRoot::Unknown,
                "provenance" => {
                    event.provenance = language_semantics::PermissionProvenance::Unknown
                }
                _ => unreachable!(),
            }
        }
        let rejected = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            &changed,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rejected
                .composed_for_machine(machine_named(&checked, "read_one"))
                .is_none(),
            "case result cleanup must be present and retain its exact source root"
        );
    }
}

#[test]
fn composes_one_compile_known_u64_binding_with_exact_boundary_leaves() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root { values: [i32; 5]; }
        machine Root::enter(&mut self) reaches Host {
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
            destination: checked_trees::CheckedScalarBindingDestination::Immutable,
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
fn provider_attachment_specialization_rejects_unrouted_fields() {
    // A direct boundary call names no `self.<field>` provider receiver.
    let source = r#"
        pub boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        data Main { console: Binding<Console>; }
        machine Main::main(&mut self) reaches Console {
            Console::exit_process(0);
        }
        "#;
    let checked = checked_with_service(source);
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
