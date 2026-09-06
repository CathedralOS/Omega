//! Affine temporaries retain their source ownership across nested calls.

use super::*;

#[test]
fn nested_boundary_results_keep_dense_postorder_and_exact_temporary_transfers() {
    for (factory, create, nominal) in [
        (
            "pub data Factory {} boundary machine Factory::create(first: u16, last: u16) -> Token ensures true;",
            "Factory::create",
            false,
        ),
        (
            "boundary trait Factory { machine create(first: u16, last: u16) -> Token; }",
            "Factory::create",
            false,
        ),
        (
            "boundary trait Factory { machine create(first: u16, last: u16) -> Token; }",
            "Create",
            true,
        ),
    ] {
        let signature = if nominal {
            "machine Root::enter<machine Create>(input: u16) where machine Create satisfies Factory::create;"
        } else {
            "machine Root::enter(input: u16)"
        };
        let source = format!(
            r#"
            pub data Token {{ flag: bool; }}
            {factory}
            boundary trait Sink {{
                machine replace(token: Token, value: u16) -> Token;
                machine take(token: Token, value: u16);
            }}
            machine identity(input: u16) -> u16 {{ input }}
            machine forward(first: u16, token: Token, last: u16) -> Token {{ token }}
            data Root {{}}
            {signature} {{
                let prior: u16 = input;
                Sink::take(Sink::replace(
                    forward(identity(11u16), {create}(identity(prior), identity(22u16)), identity(33u16)),
                    identity(44u16)), prior);
            }}
            "#
        );
        let checked = checked(&source);
        let machine = machine_named(&checked, "enter");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .unwrap_or_else(|| panic!("anonymous boundary results use the shared statement sequencer: {create}, {factory}"));
        let [
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. },
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate: create_coordinate,
                result: created,
                discard_result_on_return: false,
                completion_receipts: create_receipts,
                ..
            },
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate: forward_coordinate,
                result: forwarded,
                structural_arguments: forward_arguments,
                discard_result_on_return: false,
                ..
            },
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate: replace_coordinate,
                result: replaced,
                structural_arguments: replace_arguments,
                discard_result_on_return: false,
                completion_receipts: replace_receipts,
                ..
            },
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate: take_coordinate,
                structural_arguments: take_arguments,
                completion_receipts,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] = plan.operations.as_slice()
        else {
            panic!(
                "boundary, ordinary and boundary results execute before the root consumer: {:?}",
                plan.operations
            );
        };
        assert_eq!(
            (
                created.binding_ordinal,
                forwarded.binding_ordinal,
                replaced.binding_ordinal
            ),
            (0, 1, 2)
        );
        assert!(create_coordinate.call_ordinal > forward_coordinate.call_ordinal);
        assert!(forward_coordinate.call_ordinal > replace_coordinate.call_ordinal);
        assert!(replace_coordinate.call_ordinal > take_coordinate.call_ordinal);
        assert_eq!(take_coordinate.call_ordinal, 0);
        for coordinate in [
            create_coordinate,
            forward_coordinate,
            replace_coordinate,
            take_coordinate,
        ] {
            assert_eq!(coordinate.statement_index, 1);
        }
        assert_eq!(
            forward_arguments[0].source_structural_result_binding_ordinal(),
            Some(0)
        );
        assert_eq!(
            replace_arguments[0].source_structural_result_binding_ordinal(),
            Some(1)
        );
        assert_eq!(
            take_arguments[0].source_structural_result_binding_ordinal(),
            Some(2)
        );
        assert!(
            create_receipts.is_empty()
                && replace_receipts.is_empty()
                && completion_receipts.is_empty()
        );
        let flow = checked
            .facts
            .flow
            .control
            .states
            .iter()
            .map(|(_, flow)| flow)
            .find(|flow| flow.machine_symbol == machine && flow.state_symbol == plan.state)
            .expect("caller flow");
        let calls = checked.facts.flow.control.calls.span_or_empty(flow.calls);
        for (producer, consumer) in [
            (create_coordinate, forward_coordinate),
            (forward_coordinate, replace_coordinate),
            (replace_coordinate, take_coordinate),
        ] {
            let producer = calls
                .iter()
                .find(|call| {
                    call.statement_index == 1 && call.call_ordinal == producer.call_ordinal as usize
                })
                .unwrap();
            let consumer = calls
                .iter()
                .find(|call| {
                    call.statement_index == 1 && call.call_ordinal == consumer.call_ordinal as usize
                })
                .unwrap();
            let moves = checked
                .facts
                .flow
                .ownership
                .permissions
                .iter()
                .map(|(_, event)| event)
                .filter(|event| {
                    event.machine_symbol == machine
                        && event.state_symbol == plan.state
                        && event.root
                            == ::facts::PlaceRoot::Expression(producer.authored_expression)
                })
                .collect::<Vec<_>>();
            let [event] = moves.as_slice() else {
                panic!("each temporary has exactly one transfer");
            };
            assert_eq!(
                event.source,
                language_semantics::PermissionEventSource::Call {
                    statement_index: 1,
                    call_ordinal: consumer.call_ordinal,
                    target_symbol: consumer.target_symbol,
                }
            );
            assert_eq!(
                event.kind,
                language_semantics::PermissionEventKind::Transfer
            );
            assert_eq!(event.access, language_semantics::PermissionAccess::Owned);
            assert_eq!(event.multiplicity, Multiplicity::Affine);
            assert_eq!(
                event.claim_identity,
                language_semantics::PermissionClaimIdentity::Unknown
            );
            assert!(!event.obligation_live);
        }
        for (coordinate, argument_count, boundary) in [
            (create_coordinate, 2, true),
            (forward_coordinate, 2, false),
            (replace_coordinate, 1, true),
        ] {
            for argument_ordinal in 0..argument_count {
                let role = if boundary {
                    CheckedScalarExpressionRole::BoundaryCallArgument {
                        call_ordinal: coordinate.call_ordinal,
                        argument_ordinal,
                    }
                } else {
                    CheckedScalarExpressionRole::UnitCallArgument {
                        call_ordinal: coordinate.call_ordinal,
                        argument_ordinal,
                    }
                };
                let root = checked
                    .facts
                    .values
                    .scalar_computations
                    .root_at(plan.state, 1, role)
                    .expect("nested producer operands keep their exact source roles");
                assert_eq!(root.machine, machine);
            }
        }
    }
}

#[test]
fn nested_ordinary_results_keep_postorder_and_exact_boundary_operand_roles() {
    for (declaration, invocation, nominal) in [
        (
            "pub data Sink {} boundary machine Sink::take(first: u16, token: Token, last: u16) ensures true;",
            "Sink::take",
            false,
        ),
        (
            "boundary trait Sink { machine take(first: u16, token: Token, last: u16); }",
            "Sink::take",
            false,
        ),
        (
            "boundary trait Sink { machine take(first: u16, token: Token, last: u16); }",
            "Take",
            true,
        ),
    ] {
        let signature = if nominal {
            "machine Root::enter<machine Take>(token: Token, input: u16) where machine Take satisfies Sink::take;"
        } else {
            "machine Root::enter(token: Token, input: u16)"
        };
        let source = format!(
            r#"
            pub data Token {{ flag: bool; }}
            {declaration}
            machine identity(input: u16) -> u16 {{ input }}
            machine forward(first: u16, token: Token, last: u16) -> Token {{ token }}
            data Root {{}}
            {signature} {{
                let prior: u16 = input;
                {invocation}(identity(prior),
                    forward(identity(11u16), forward(identity(22u16), token, identity(33u16)), identity(44u16)),
                    identity(55u16));
            }}
        "#
        );
        let checked = checked(&source);
        let machine = machine_named(&checked, "enter");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .expect("nested ordinary result boundary call");
        let [
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. },
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate: inner,
                result: inner_result,
                discard_result_on_return: false,
                ..
            },
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate: outer,
                result: outer_result,
                discard_result_on_return: false,
                structural_arguments: outer_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate: boundary,
                structural_arguments,
                completion_receipts,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] = plan.operations.as_slice()
        else {
            panic!("two producers execute before their boundary consumer")
        };
        assert_eq!(
            (
                inner.statement_index,
                outer.statement_index,
                boundary.statement_index
            ),
            (1, 1, 1)
        );
        assert!(
            inner.call_ordinal > outer.call_ordinal && outer.call_ordinal > boundary.call_ordinal
        );
        assert_eq!(boundary.call_ordinal, 0);
        assert_eq!(
            (inner_result.binding_ordinal, outer_result.binding_ordinal),
            (0, 1)
        );
        assert_eq!(
            outer_arguments[0].source_structural_result_binding_ordinal(),
            Some(0)
        );
        assert_eq!(
            structural_arguments[0].source_structural_result_binding_ordinal(),
            Some(1)
        );
        assert!(completion_receipts.is_empty());
        let computations = &checked.facts.values.scalar_computations;
        for coordinate in [inner, outer, boundary] {
            for argument_ordinal in 0..2 {
                let role = if coordinate.call_ordinal == 0 {
                    CheckedScalarExpressionRole::BoundaryCallArgument {
                        call_ordinal: 0,
                        argument_ordinal,
                    }
                } else {
                    CheckedScalarExpressionRole::UnitCallArgument {
                        call_ordinal: coordinate.call_ordinal,
                        argument_ordinal,
                    }
                };
                let root = computations
                    .root_at(plan.state, 1, role)
                    .expect("exact operand role");
                assert_eq!(root.machine, machine);
            }
        }
        let temporary_moves = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine
                    && event.state_symbol == plan.state
                    && matches!(event.root, ::facts::PlaceRoot::Expression(_))
            })
            .collect::<Vec<_>>();
        assert_eq!(temporary_moves.len(), 2);
        assert!(temporary_moves.iter().all(|event| event.kind
            == language_semantics::PermissionEventKind::Transfer
            && event.multiplicity == Multiplicity::Affine
            && event.claim_identity == language_semantics::PermissionClaimIdentity::Unknown
            && !event.obligation_live));
    }
}
