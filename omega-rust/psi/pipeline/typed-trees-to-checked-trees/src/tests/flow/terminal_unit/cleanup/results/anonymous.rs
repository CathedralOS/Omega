//! Temporary projections retain exact producer, transfer, and residual facts.
use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};

fn source(boundary: bool) -> checked_trees::CheckedTrees {
    let (producer, parameters, expression, reach) = if boundary {
        (
            "boundary trait Factory { machine create() -> Pair reaches Factory; }",
            "",
            "Factory::create()",
            "reaches Factory",
        )
    } else {
        (
            "machine forward(value: Pair) -> Pair { value }",
            "value: Pair",
            "forward(value)",
            "",
        )
    };
    checked(&format!(
        r#"
        pub data Token {{ value: u64; }}
        pub data Pair {{ left: Token; right: Token; }}
        data Sink {{}}
        machine Sink::take(value: Token) {{}}
        {producer}
        data Main {{}}
        machine Main::enter({parameters}) {reach} {{
            Sink::take({expression}.right);
        }}
    "#
    ))
}

#[test]
fn anonymous_projection_permissions_name_exact_producer_and_residual() {
    for boundary in [false, true] {
        let checked = source(boundary);
        let machine = machine_named(&checked, "enter");
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine)
            .expect("anonymous partial cleanup plan");
        let state = plan.machine.state;
        let flow = checked
            .facts
            .flow
            .control
            .states
            .iter()
            .map(|(_, flow)| flow)
            .find(|flow| flow.machine_symbol == machine && flow.state_symbol == state)
            .unwrap();
        let calls = checked.facts.flow.control.calls.span_or_empty(flow.calls);
        let producer = calls.iter().find(|call| call.call_ordinal == 1).unwrap();
        let consumer = calls.iter().find(|call| call.call_ordinal == 0).unwrap();
        assert_eq!((producer.statement_index, consumer.statement_index), (0, 0));
        let root = facts::PlaceRoot::Expression(producer.authored_expression);
        let rows = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| event.machine_symbol == machine && event.root == root)
            .collect::<Vec<_>>();
        let [establish, transfer, discard] = rows.as_slice() else {
            panic!("three exact temporary events");
        };
        let producer_source = PermissionEventSource::Call {
            statement_index: 0,
            call_ordinal: 1,
            target_symbol: producer.target_symbol,
        };
        let consumer_source = PermissionEventSource::Call {
            statement_index: 0,
            call_ordinal: 0,
            target_symbol: consumer.target_symbol,
        };
        assert_eq!(
            (establish.kind, establish.source),
            (PermissionEventKind::Establish, producer_source)
        );
        assert_eq!(
            (transfer.kind, transfer.source),
            (PermissionEventKind::Transfer, consumer_source)
        );
        assert_eq!(
            (discard.kind, discard.source),
            (PermissionEventKind::AffineDrop, consumer_source)
        );
        let provenance = PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: producer_source,
        };
        for event in &rows {
            assert_eq!(event.provenance, provenance);
            assert_eq!(event.state_symbol, state);
            assert_eq!(event.access, PermissionAccess::Owned);
            assert_eq!(event.multiplicity, Multiplicity::Affine);
            assert_eq!(event.claim_identity, PermissionClaimIdentity::Unknown);
            assert!(!event.obligation_live);
        }
        assert!(establish.segments.is_empty());
        for (event, name) in [(transfer, "right"), (discard, "left")] {
            let [facts::PlaceSegment::Field { symbol }] = checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
            else {
                panic!("one exact field");
            };
            assert!(checked.typed.data_definitions().iter().flat_map(|data| checked.typed.data_members(data))
                .any(|member| matches!(member, typed_trees::data::DataMember::Field(field) if field.symbol == *symbol && field.name.as_str() == name)));
        }
        assert_eq!(plan.machine.operations.len(), 3);
        assert!(matches!(
            plan.machine.operations.last(),
            Some(CheckedUnitEffectOperationPlan::Complete {
                statement_index: 1,
                ..
            })
        ));
        assert_eq!(plan.residual_affine_discards.len(), 1);
        assert_eq!(
            plan.residual_affine_discards[0].source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: 0
            }
        );
    }
}

/// Two projected helper temporaries die at one shared consumer; each keeps
/// its own residual row and its own producer custody handoff.
#[test]
fn anonymous_projected_operands_share_one_consumer_continuation() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Sink {}
        machine Sink::take2(first: Token, second: Token) {}
        data Root {}
        machine Root::forward(value: Pair) -> Pair { value }
        machine Root::enter(first: Pair, second: Pair) {
            Sink::take2(Root::forward(first).right, Root::forward(second).left);
        }
    "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "two anonymous temporaries share one dying continuation: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let [
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate: first_producer,
            result: first_result,
            discard_result_on_return: first_discard,
            ..
        },
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate: second_producer,
            result: second_result,
            discard_result_on_return: second_discard,
            ..
        },
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: consumer,
            structural_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate: cleanup,
            affine_discards,
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("producer, producer, consumer, cleanup, completion")
    };
    assert_eq!(
        (first_producer.statement_index, first_producer.call_ordinal),
        (0, 1)
    );
    assert_eq!(
        (
            second_producer.statement_index,
            second_producer.call_ordinal
        ),
        (0, 2)
    );
    assert_eq!((consumer.statement_index, consumer.call_ordinal), (0, 0));
    assert_eq!(*cleanup, *consumer);
    assert!(
        !*first_discard && !*second_discard,
        "each producer hands its residual custody to the continuation"
    );
    assert_eq!(first_result.binding_ordinal, 0);
    assert_eq!(second_result.binding_ordinal, 1);
    let [first_argument, second_argument] = structural_arguments.as_slice() else {
        panic!("both projected temporaries are consumer operands")
    };
    assert_eq!(
        first_argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 0
        }
    );
    assert_eq!(
        second_argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 1
        }
    );
    let field = |argument: &checked_trees::CheckedUnitStructuralArgumentPlan| {
        let [checked_trees::CheckedUnitStructuralPathSegment::Field(name)] =
            argument.path.as_slice()
        else {
            panic!("one exact field projection")
        };
        name.clone()
    };
    assert_eq!(field(first_argument), "right");
    assert_eq!(field(second_argument), "left");
    // Residual rows keep operand order: each complement names its own owner.
    let residual = |discard: &checked_trees::CheckedUnitPartialAffineDiscardPlan| {
        let [checked_trees::CheckedUnitStructuralPathSegment::Field(name)] =
            discard.path.as_slice()
        else {
            panic!("one exact residual field")
        };
        (discard.source.clone(), name.clone())
    };
    let [first_residual, second_residual] = affine_discards.as_slice() else {
        panic!("two temporaries keep two residual rows")
    };
    assert_eq!(
        residual(first_residual),
        (
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: 0
            },
            "left".into()
        )
    );
    assert_eq!(
        residual(second_residual),
        (
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: 1
            },
            "right".into()
        )
    );
}

/// A projected temporary shares its consumer's argument list with an
/// ordinary owned parameter and a scalar operand without losing custody.
#[test]
fn anonymous_projected_operand_shares_its_consumer_with_other_effects() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Sink {}
        machine Sink::take2(first: Token, count: u16, second: Token) {}
        machine Sink::done() {}
        data Root {}
        machine Root::forward(value: Pair) -> Pair { value }
        machine Root::enter(token: Token, input: Pair, count: u16) {
            Sink::take2(token, count, Root::forward(input).left);
            Sink::done();
        }
    "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "a projected temporary beside other operands dies at the call: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let [
        CheckedUnitEffectOperationPlan::StructuralCall {
            discard_result_on_return,
            ..
        },
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: consumer,
            structural_arguments,
            scalar_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate: cleanup,
            affine_discards,
        },
        CheckedUnitEffectOperationPlan::CallUnit { .. },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("producer, consumer, cleanup, trailing call, completion")
    };
    assert!(!*discard_result_on_return);
    assert_eq!(*cleanup, *consumer);
    assert_eq!(scalar_arguments.len(), 1);
    let [owned, projected] = structural_arguments.as_slice() else {
        panic!("the parameter and the temporary are separate operands")
    };
    assert_eq!(
        owned.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
    );
    assert!(owned.path.is_empty());
    assert_eq!(
        projected.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 0
        }
    );
    let [discard] = affine_discards.as_slice() else {
        panic!("one temporary keeps one residual row")
    };
    assert_eq!(discard.source, projected.source);
}

#[test]
fn anonymous_projection_permissions_cannot_be_removed_duplicated_or_rebound() {
    for boundary in [false, true] {
        let original = source(boundary);
        let machine = machine_named(&original, "enter");
        assert!(
            original
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .for_machine(machine)
                .is_some()
        );
        let rows = original
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == machine
                    && matches!(event.root, facts::PlaceRoot::Expression(_))
            })
            .map(|(handle, event)| (handle, event.clone()))
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 3);
        for (handle, event) in rows {
            for mutation in [
                "missing",
                "duplicate",
                "root",
                "path",
                "provenance",
                "coordinate",
            ] {
                let mut changed = original.clone();
                let permissions = &mut changed.facts.flow.ownership.permissions;
                match mutation {
                    "missing" => {
                        assert!(permissions.free(handle));
                    }
                    "duplicate" => {
                        permissions.insert(event.clone());
                    }
                    "root" => permissions.get_mut(handle).root = facts::PlaceRoot::Unknown,
                    "path" => {
                        permissions.get_mut(handle).segments = if event.segments.is_empty() {
                            original
                                .facts
                                .flow
                                .ownership
                                .permissions
                                .iter()
                                .map(|(_, event)| event)
                                .find(|event| {
                                    event.machine_symbol == machine
                                        && event.kind == PermissionEventKind::AffineDrop
                                })
                                .unwrap()
                                .segments
                        } else {
                            arena::HandleSpan::empty()
                        };
                    }
                    "provenance" => {
                        permissions.get_mut(handle).provenance = PermissionProvenance::Unknown
                    }
                    "coordinate" => {
                        permissions.get_mut(handle).source = PermissionEventSource::StateExit
                    }
                    _ => unreachable!(),
                }
                if let Ok(changed) =
                    crate::settle_checked_execution(changed, &crate::ExecutionSettlement::default())
                {
                    // The public rebuild refreshes call plans; partial cleanup has
                    // its own existing producer and must also be rederived.
                    let rebuilt = crate::execution::terminal_unit::build_checked_partial_affine_unit_cleanup_plans(
                        &changed.typed,
                        &changed.facts,
                        &changed.facts.flow.terminal_unit_effects,
                    );
                    assert!(
                        rebuilt.for_machine(machine).is_none(),
                        "{mutation} {:?}, boundary={boundary}",
                        event.kind
                    );
                    assert!(
                        changed
                            .facts
                            .flow
                            .terminal_unit_effects
                            .for_machine(machine)
                            .is_none()
                    );
                }
            }
        }
    }
}
