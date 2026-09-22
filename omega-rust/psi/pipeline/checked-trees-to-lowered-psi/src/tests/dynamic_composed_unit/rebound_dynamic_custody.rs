use super::{
    CHANGED_CONFORMANCE_DYNAMIC_INTEGER_SOURCE, FORWARDED_DIRECT_DYNAMIC_INTEGER_SOURCE,
    FORWARDED_REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE, JOINED_DYNAMIC_BOOLEAN_SOURCE,
    REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE, STORED_DYNAMIC_INTEGER_CONTROL_SOURCE,
    STORED_DYNAMIC_SOURCE, assert_dynamic_unit_artifact_executes,
    assert_stored_dynamic_scalar_artifact_executes, unsupported_message,
};
use crate::TerminalMachineSelection;
use crate::tests::{checked_source, checked_source_with_core_service, lower_machine};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{Operation, OperationKind, Terminator};

#[test]
fn lowers_stored_dynamic_descriptor_as_verified_terminal_storage_and_reload() {
    let mut checked = checked_source(STORED_DYNAMIC_SOURCE);
    let catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(catalog.direct_scalar_calls.is_empty());
    assert!(catalog.rebound_scalar_calls.is_empty());
    let [plan] = catalog.stored_scalar_calls.as_slice() else {
        panic!("one checked stored dynamic plan expected, got {catalog:#?}")
    };
    assert_eq!(plan.storage.statement_index, 1);
    assert_eq!(plan.call.coordinate.statement_index, 2);

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("stored dynamic call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("stored dynamic module verifies");
    let terminal = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(terminal.selections.len(), 1);
    assert!(terminal.rebound_descriptors.is_empty());
    let [descriptor] = terminal.stored_descriptors.as_slice() else {
        panic!("one terminal stored descriptor expected")
    };
    assert!(descriptor.aggregate_type_identity.contains("Holder"));
    assert_eq!(descriptor.field_identity, "handler");
    let [dispatch] = terminal.stored_dispatches.as_slice() else {
        panic!("one terminal stored dispatch expected")
    };
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == descriptor.owner)
        .expect("stored descriptor owner");
    let operations = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert!(matches!(
        operations[0].kind,
        OperationKind::StoreDynamicDescriptor {
            descriptor_ordinal: 0
        }
    ));
    assert_eq!(operations[0].id, descriptor.establishment_operation);
    assert_eq!(operations[1].id, dispatch.operation);
    assert!(matches!(
        operations[1].kind,
        OperationKind::CallDynamicScalar {
            descriptor_ordinal: 0,
            ..
        }
    ));
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("stored dynamic module has canonical source-free encoding")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("decode stored dynamic module"),
        lowered.semantic_module
    );
    assert_stored_dynamic_scalar_artifact_executes(&artifact);

    let mut tampered = lowered.semantic_module.clone();
    tampered.dynamic_dispatch.stored_descriptors[0].selection_ordinal = 1;
    assert!(terminal_verifier::validate_module(&tampered).is_err());

    checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .stored_scalar_calls[0]
        .destination_field_identity = "other".into();
    assert_eq!(
        unsupported_message(&checked),
        "stored dynamic descriptor drifted from checked aggregate custody"
    );
}

#[test]
fn lowers_stored_dynamic_result_into_console_effect_control() {
    let checked = checked_source_with_core_service(STORED_DYNAMIC_INTEGER_CONTROL_SOURCE);
    assert_eq!(
        checked.facts.dynamic_conformances.storages.len(),
        1,
        "stored control fixture should retain descriptor storage"
    );
    let catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [plan] = catalog.stored_scalar_calls.as_slice() else {
        panic!("one checked stored dynamic control plan expected, got {catalog:#?}")
    };
    assert!(plan.call.unit_continuation.is_some());

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("stored dynamic result control lowers as one module");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("stored dynamic result control verifies");
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("stored dynamic caller");
    assert_eq!(caller.blocks.len(), 3);
    assert!(matches!(
        caller.blocks[0].operations[0].kind,
        OperationKind::StoreDynamicDescriptor {
            descriptor_ordinal: 0
        }
    ));
    assert!(matches!(
        caller.blocks[0].operations[1].kind,
        OperationKind::CallDynamicScalar {
            descriptor_ordinal: 0,
            ..
        }
    ));
    assert!(matches!(
        caller.blocks[0].terminator,
        Terminator::Conditional { .. }
    ));
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("stored dynamic result control has canonical source-free encoding")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("decode stored dynamic control module"),
        lowered.semantic_module
    );

    let mut tampered = checked.clone();
    let stored = &mut tampered
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .stored_scalar_calls[0];
    stored
        .call
        .unit_continuation
        .as_mut()
        .expect("stored control continuation")
        .trivial_affine_local_discard = Some(stored.storage.source_binding);
    assert_eq!(
        unsupported_message(&tampered),
        "stored dynamic continuation local cleanup drifted after checking"
    );
}

#[test]
fn stored_dynamic_cleanup_requires_exact_affine_establishment_and_disposal() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};

    let checked = checked_source_with_core_service(STORED_DYNAMIC_INTEGER_CONTROL_SOURCE);
    let stored = &checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .stored_scalar_calls[0];
    let local = stored.storage.destination_binding;
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == stored.call.caller_machine
                && event.state_symbol == stored.call.caller_state
                && event.root == facts::PlaceRoot::Symbol(local)
        })
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2, "local establishment and disposal");
    assert_eq!(events[0].1.kind, PermissionEventKind::Establish);
    assert_eq!(events[1].1.kind, PermissionEventKind::AffineDrop);
    let provenance = PermissionProvenance::Established {
        machine_symbol: stored.call.caller_machine,
        state_symbol: stored.call.caller_state,
        source: PermissionEventSource::Statement {
            statement_index: stored.storage.statement_index,
        },
    };
    let mut reversed = checked.clone();
    *reversed
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(events[0].0) = events[1].1.clone();
    *reversed
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(events[1].0) = events[0].1.clone();
    assert_eq!(
        unsupported_message(&reversed),
        "stored dynamic continuation local cleanup drifted after checking"
    );
    for (handle, event) in events {
        assert_eq!(event.provenance, provenance);
        let mutations: &[fn(&mut checked_trees::FlowPermissionEventFact)] = &[
            |event| event.provenance = PermissionProvenance::Unknown,
            |event| {
                event.provenance = PermissionProvenance::Established {
                    machine_symbol: event.machine_symbol,
                    state_symbol: event.state_symbol,
                    source: PermissionEventSource::StateEntry,
                }
            },
            |event| event.obligation_live = true,
            |event| event.multiplicity = language_semantics::Multiplicity::Linear,
            |event| event.access = language_semantics::PermissionAccess::Shared,
            |event| event.kind = PermissionEventKind::Transfer,
            |event| event.source = PermissionEventSource::StateEntry,
            |event| event.state_symbol = symbols::SymbolHandle::default(),
            |event| event.root = facts::PlaceRoot::Symbol(symbols::SymbolHandle::default()),
            |event| {
                event.claim_identity = language_semantics::PermissionClaimIdentity::Established {
                    machine_symbol: event.machine_symbol,
                    state_symbol: event.state_symbol,
                    source: event.source,
                    ordinal: 0,
                }
            },
            |event| event.segments = arena::HandleSpan::from_parts(arena::Handle::invalid(), 1),
        ];
        for mutate in mutations {
            let mut tampered = checked.clone();
            mutate(tampered.facts.flow.ownership.permissions.get_mut(handle));
            assert_eq!(
                unsupported_message(&tampered),
                "stored dynamic continuation local cleanup drifted after checking"
            );
        }
        let mut duplicate = checked.clone();
        duplicate
            .facts
            .flow
            .ownership
            .permissions
            .insert(event.clone());
        assert_eq!(
            unsupported_message(&duplicate),
            "stored dynamic continuation local cleanup drifted after checking"
        );
        for kind in [PermissionEventKind::Transfer, PermissionEventKind::Consume] {
            let mut extra = checked.clone();
            let mut extra_event = event.clone();
            extra_event.kind = kind;
            extra.facts.flow.ownership.permissions.insert(extra_event);
            assert_eq!(
                unsupported_message(&extra),
                "stored dynamic continuation local cleanup drifted after checking"
            );
        }
    }
}

#[test]
fn lowers_rebound_dynamic_custody_as_verified_indirect_terminal_dispatch() {
    let mut checked = checked_source_with_core_service(REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE);
    let catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(catalog.direct_scalar_calls.is_empty());
    let [plan] = catalog.rebound_scalar_calls.as_slice() else {
        panic!("one rebound dynamic plan expected, got {catalog:#?}")
    };
    assert_eq!(plan.initial.fact.statement_index, 0);
    assert_eq!(plan.latest.selection.statement_index, 1);
    assert_eq!(plan.latest.coordinate.statement_index, 2);
    let duplicate = plan.clone();

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("rebound dynamic call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("rebound dynamic module verifies");
    let terminal_catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(terminal_catalog.selections.len(), 2);
    assert_eq!(terminal_catalog.rebound_descriptors.len(), 1);
    assert!(terminal_catalog.direct_dispatches.is_empty());
    let [dispatch] = terminal_catalog.indirect_dispatches.as_slice() else {
        panic!("one indirect dynamic dispatch expected")
    };
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == dispatch.owner)
        .expect("indirect caller");
    let operation = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == dispatch.operation)
        .expect("indirect call operation");
    assert!(matches!(
        operation.kind,
        OperationKind::CallDynamicScalar {
            descriptor_ordinal: 0,
            ..
        }
    ));
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("rebound dynamic module has canonical source-free encoding")
    .into_artifact();

    checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .rebound_scalar_calls
        .push(duplicate);
    assert_eq!(
        unsupported_message(&checked),
        "rebound dynamic dispatch plan is duplicated for one caller"
    );
}

#[test]
fn retains_distinct_applications_when_rebinding_to_another_conformance() {
    let checked = checked_source(CHANGED_CONFORMANCE_DYNAMIC_INTEGER_SOURCE);
    let catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(catalog.direct_scalar_calls.is_empty());
    let [plan] = catalog.rebound_scalar_calls.as_slice() else {
        panic!("one changed-conformance rebound plan expected: {catalog:#?}")
    };
    assert_ne!(
        plan.initial.fact.conformance,
        plan.latest.selection.conformance
    );
    assert_ne!(plan.initial.fact.rows, plan.latest.selection.rows);

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("changed-conformance rebound should lower");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("changed-conformance rebound should verify");
    let [initial, rebound] = lowered
        .semantic_module
        .dynamic_dispatch
        .selections
        .as_slice()
    else {
        panic!("two exact conformance selections expected")
    };
    assert_ne!(
        initial.conformance_application_commitment,
        rebound.conformance_application_commitment
    );
    assert_eq!(
        lowered
            .semantic_module
            .closed_conformance_applications
            .len(),
        2
    );
}

#[test]
fn composes_one_transparent_dynamic_forwarder_without_losing_descriptor_custody() {
    let mut checked =
        checked_source_with_core_service(FORWARDED_REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE);
    let catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [transfer] = catalog.transfers.as_slice() else {
        panic!("one checked dynamic descriptor transfer expected, got {catalog:#?}")
    };
    let [plan] = catalog.rebound_scalar_calls.as_slice() else {
        panic!("one forwarded rebound dynamic plan expected, got {catalog:#?}")
    };
    let checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
        machine,
        state,
        coordinate,
        parameter,
    } = plan.latest.origin
    else {
        panic!("forwarded origin expected")
    };
    assert_eq!(coordinate.statement_index, 0);
    assert_eq!(coordinate.call_ordinal, 0);
    assert_eq!(transfer.caller_machine, plan.latest.caller_machine);
    assert_eq!(transfer.caller_state, plan.latest.caller_state);
    assert_eq!(transfer.coordinate, plan.latest.coordinate);
    assert_eq!(transfer.target_machine, machine);
    assert_eq!(transfer.target_state, state);
    assert_eq!(transfer.parameter, parameter);
    assert_eq!(transfer.parameter_position, 0);
    assert_eq!(transfer.source_binding, plan.latest.receiver_binding);
    assert_eq!(transfer.sole_selection(), Some(&plan.latest.selection));

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("transparent forwarded dynamic call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("forwarded dynamic module verifies");
    assert_eq!(
        lowered
            .semantic_module
            .dynamic_dispatch
            .rebound_descriptors
            .len(),
        1
    );
    assert_eq!(
        lowered
            .semantic_module
            .dynamic_dispatch
            .indirect_dispatches
            .len(),
        0
    );
    assert_eq!(lowered.semantic_module.dynamic_dispatch.parameters.len(), 1);
    assert_eq!(lowered.semantic_module.dynamic_dispatch.arguments.len(), 1);
    assert_eq!(
        lowered
            .semantic_module
            .dynamic_dispatch
            .parameter_dispatches
            .len(),
        1
    );
    assert_eq!(lowered.source_call_occurrences.len(), 4);

    let [plan] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .rebound_scalar_calls
        .as_mut_slice()
    else {
        unreachable!("checked above")
    };
    let checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { coordinate, .. } =
        &mut plan.latest.origin
    else {
        unreachable!("checked above")
    };
    coordinate.call_ordinal = 1;
    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic call drifted from checked flow custody"
    );
}

#[test]
fn composes_one_direct_dynamic_scalar_forwarder_without_fabricating_a_rebound() {
    let checked = checked_source(FORWARDED_DIRECT_DYNAMIC_INTEGER_SOURCE);
    let catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert_eq!(catalog.transfers.len(), 1);
    assert_eq!(catalog.direct_scalar_calls.len(), 1);
    assert!(catalog.rebound_scalar_calls.is_empty());
    let mut lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("transparent direct scalar forwarding should lower");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("forwarded direct scalar module should verify");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 1);
    assert!(catalog.rebound_descriptors.is_empty());
    assert!(catalog.direct_dispatches.is_empty());
    assert!(catalog.indirect_dispatches.is_empty());
    let [parameter] = catalog.parameters.as_slice() else {
        panic!("one direct scalar descriptor parameter expected: {catalog:#?}")
    };
    let [argument] = catalog.arguments.as_slice() else {
        panic!("one direct scalar descriptor argument expected: {catalog:#?}")
    };
    let [dispatch] = catalog.parameter_dispatches.as_slice() else {
        panic!("one direct scalar parameter dispatch expected: {catalog:#?}")
    };
    assert_eq!(parameter.owner, dispatch.owner);
    assert_eq!(
        argument.source,
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    assert_eq!(
        parameter.requirements[0].result,
        terminal_psi::ClosedConformanceCallableResult::I32
    );
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == argument.owner)
        .expect("forwarded direct scalar caller");
    let outer = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == argument.operation)
        .expect("outer direct scalar call");
    assert!(matches!(
        outer.kind,
        OperationKind::CallStructuralScalar { .. }
    ));
    let helper = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == parameter.owner)
        .expect("forwarded direct scalar helper");
    assert!(
        helper
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                OperationKind::CallDynamicParameterScalar {
                    parameter_ordinal: 0,
                    requirement_slot: 0,
                    ..
                }
            ))
    );
    assert_eq!(lowered.source_call_occurrences.len(), 2);

    lowered.semantic_module.dynamic_dispatch.arguments[0].source =
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 1 };
    assert!(terminal_verifier::validate_module(&lowered.semantic_module).is_err());

    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("forwarded direct scalar module should encode")
    .into_artifact();
    let decoded = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("forwarded direct scalar module should decode");
    assert_eq!(
        decoded.dynamic_dispatch.arguments[0].source,
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    assert_dynamic_unit_artifact_executes(&artifact);
}

#[test]
fn lowers_two_dynamic_predecessors_into_one_terminal_parameter() {
    let mut checked = checked_source(JOINED_DYNAMIC_BOOLEAN_SOURCE);
    let checked_catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(checked_catalog.direct_scalar_calls.is_empty());
    let [joined] = checked_catalog.joined_scalar_calls.as_slice() else {
        panic!("one checked dynamic join expected: {checked_catalog:#?}")
    };
    assert_ne!(
        joined.when_true.call.selection.conformance,
        joined.when_false.call.selection.conformance,
    );

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("two exact dynamic predecessors should lower");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("joined dynamic Terminal module should verify");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    assert_eq!(catalog.arguments.len(), 2);
    assert_eq!(catalog.parameters.len(), 1);
    assert_eq!(catalog.parameter_dispatches.len(), 1);
    assert_eq!(
        catalog.arguments[0].source,
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
    );
    assert_eq!(
        catalog.arguments[1].source,
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 1 },
    );
    assert_eq!(
        lowered
            .semantic_module
            .closed_conformance_applications
            .len(),
        2
    );
    assert_eq!(lowered.source_call_occurrences.len(), 3);
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("joined dynamic caller machine");
    assert_eq!(caller.blocks.len(), 3);
    assert!(matches!(
        caller.blocks[0].terminator,
        Terminator::Conditional { .. }
    ));
    let parameter_owner = catalog.parameters[0].owner;
    assert!(caller.blocks[1..].iter().all(|block| {
        matches!(
            block.operations.as_slice(),
            [Operation {
                kind: OperationKind::CallStructuralScalar { callee, .. },
                ..
            }] if *callee == parameter_owner
        )
    }));

    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("joined dynamic module should encode canonically")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("joined dynamic module should decode"),
        lowered.semantic_module,
    );
    let [self_parameter] = caller.structural_parameters.as_slice() else {
        panic!("joined caller requires one structural self parameter")
    };
    let terminal_psi::StructuralTypeShape::Record {
        fields: caller_fields,
    } = &lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == self_parameter.structural_type)
        .expect("joined dynamic caller type")
        .shape
    else {
        panic!("joined dynamic caller must be a record")
    };
    let [terminal_psi::StructuralPathSegment::Field(first_field)] =
        catalog.selections[0].source.path.as_slice()
    else {
        panic!("joined dynamic source must be one field")
    };
    let terminal_psi::StructuralFieldType::Structural(source_type) = caller_fields
        .iter()
        .find(|field| field.identity == *first_field)
        .expect("joined dynamic source field")
        .field_type
    else {
        panic!("joined dynamic source field must be structural")
    };
    let terminal_psi::StructuralTypeShape::Record {
        fields: source_fields,
    } = &lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == source_type)
        .expect("joined dynamic source type")
        .shape
    else {
        panic!("joined dynamic source must be a record")
    };
    let [marker] = source_fields.as_slice() else {
        panic!("joined dynamic source has one Boolean field")
    };
    let field_values = catalog
        .selections
        .iter()
        .enumerate()
        .map(
            |(index, selection)| terminal_interpreter::TerminalStructuralBooleanFieldValue {
                argument_index: 0,
                path: selection.source.path.clone(),
                field: marker.id,
                value: index == 0,
            },
        )
        .collect::<Vec<_>>();
    for choose_first in [true, false] {
        let structural = terminal_interpreter::TerminalStructuralValue {
            opaque_identity: 1,
            structural_type: self_parameter.structural_type,
            qualifications: self_parameter.qualifications.clone(),
            path: Vec::new(),
        };
        let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[terminal_interpreter::TerminalScalarValue::Boolean(
                choose_first,
            )],
            TerminalStructuralInputs {
                arguments: &[structural],
                boolean_fields: &field_values,
                ..Default::default()
            },
        )
        .expect("joined dynamic artifact should start");
        let mut meter = terminal_fuel::TerminalFuelMeter::unbounded();
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .expect("joined dynamic artifact should execute"),
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit,
            ),
        );
    }

    let [joined] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .joined_scalar_calls
        .as_mut_slice()
    else {
        unreachable!("checked above")
    };
    joined.when_false.successor.target_state = joined.when_true.successor.target_state;
    assert_eq!(
        unsupported_message(&checked),
        "joined dynamic control plan drifted from checked custody",
    );
}
