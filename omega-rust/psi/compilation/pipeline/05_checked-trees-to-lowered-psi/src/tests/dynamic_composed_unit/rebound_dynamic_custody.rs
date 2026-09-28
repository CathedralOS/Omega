use super::{
    REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE, STORED_DYNAMIC_INTEGER_CONTROL_SOURCE,
    STORED_DYNAMIC_SOURCE, assert_stored_dynamic_scalar_artifact_executes, unsupported_message,
};
use crate::TerminalMachineSelection;
use crate::tests::{checked_source_with_core_service, lower_machine};
use checked_trees::CheckedDynamicBinding::{Joined, Rebound, Stored};
use checked_trees::CheckedDynamicDispatchPlan::Scalar;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, Terminator};

#[test]
fn lowers_stored_dynamic_descriptor_as_verified_terminal_storage_and_reload() {
    let mut checked = crate::front_end::checked_program(STORED_DYNAMIC_SOURCE);
    let catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [Scalar(Stored { descriptor, call })] = catalog.calls.as_slice() else {
        panic!("one checked stored dynamic plan expected, got {catalog:#?}")
    };
    assert_eq!(descriptor.storage.statement_index, 1);
    assert_eq!(call.coordinate.statement_index, 2);

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

    let [Scalar(Stored { descriptor, .. })] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls
        .as_mut_slice()
    else {
        unreachable!("checked above")
    };
    descriptor.destination_field_identity = "other".into();
    assert_eq!(
        unsupported_message(&checked),
        "stored dynamic descriptor drifted from checked aggregate custody"
    );
}

#[test]
fn stored_dynamic_cleanup_requires_exact_affine_establishment_and_disposal() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};

    let checked = checked_source_with_core_service(STORED_DYNAMIC_INTEGER_CONTROL_SOURCE);
    let [Scalar(Stored { descriptor, call })] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls
        .as_slice()
    else {
        unreachable!("one stored dynamic plan")
    };
    let local = descriptor.storage.destination_binding;
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == call.caller_machine
                && event.state_symbol == call.caller_state
                && event.root == facts::PlaceRoot::Symbol(local)
        })
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2, "local establishment and disposal");
    assert_eq!(events[0].1.kind, PermissionEventKind::Establish);
    assert_eq!(events[1].1.kind, PermissionEventKind::AffineDrop);
    let provenance = PermissionProvenance::Established {
        machine_symbol: call.caller_machine,
        state_symbol: call.caller_state,
        source: PermissionEventSource::Statement {
            statement_index: descriptor.storage.statement_index,
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
    let [plan @ Scalar(Rebound { initial, latest })] = catalog.calls.as_slice() else {
        panic!("one rebound dynamic plan expected, got {catalog:#?}")
    };
    assert_eq!(initial.fact.statement_index, 0);
    assert_eq!(latest.selection.statement_index, 1);
    assert_eq!(latest.coordinate.statement_index, 2);
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
        .calls
        .push(duplicate);
    assert_eq!(
        unsupported_message(&checked),
        "rebound dynamic dispatch plan is duplicated for one caller"
    );
}

fn lower_joined_guard_caller(source: &str) -> terminal_psi::TerminalMachine {
    let checked = crate::front_end::checked_program(source);
    let checked_catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [Scalar(Joined { .. })] = checked_catalog.calls.as_slice() else {
        panic!("one checked dynamic join expected: {checked_catalog:#?}")
    };
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("joined dynamic control lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("joined dynamic module verifies");
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("joined dynamic module encodes canonically")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("joined dynamic module decodes"),
        lowered.semantic_module,
    );
    let entry_id = lowered.semantic_module.entry;
    lowered
        .semantic_module
        .machines
        .into_iter()
        .find(|machine| machine.id == entry_id)
        .expect("joined dynamic caller machine")
}

fn joined_entry_condition(caller: &terminal_psi::TerminalMachine) -> semantic_vocabulary::ValueId {
    let Terminator::Conditional { condition, .. } = &caller.blocks[0].terminator else {
        panic!("joined caller entry must end in a conditional")
    };
    *condition
}

const JOINED_RETAINED_FIELD_INEQUALITY_GUARD_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] { marker: bool; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    Secondary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    data Main [copy] { flag: bool; first: Item; second: Item; }

    machine Main::run(&self) {
        transition self.flag != true {
            true -> take_first()
            _ -> take_second()
        }

        state take_first(&self) {
            let selected: &dyn Measure = &self.first as &dyn Item::Primary;
            let result: bool = finish(selected);
        }

        state take_second(&self) {
            let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
            let result: bool = finish(selected);
        }
    }

    machine finish(erased: &dyn Measure) -> bool {
        let result: bool = erased.measure();
        transition { _ -> result }
    }
"#;

#[test]
fn lowers_joined_retained_field_inequality_into_negated_equality() {
    let caller = lower_joined_guard_caller(JOINED_RETAINED_FIELD_INEQUALITY_GUARD_SOURCE);
    assert!(caller.parameters.is_empty());
    let [read, constant, equality, negation] = caller.blocks[0].operations.as_slice() else {
        panic!(
            "field inequality lowers to read, constant, equality, negation: {:?}",
            caller.blocks[0].operations
        )
    };
    assert!(matches!(
        read.kind,
        OperationKind::BooleanStructuralField { .. }
    ));
    assert!(matches!(
        constant.kind,
        OperationKind::BooleanConstant { value: true }
    ));
    let OperationKind::BooleanEqual { left, right } = equality.kind else {
        panic!("field equality lowers to BooleanEqual: {equality:?}")
    };
    assert_eq!(left, read.result.expect_scalar().id);
    assert_eq!(right, constant.result.expect_scalar().id);
    let OperationKind::BooleanNot { operand } = negation.kind else {
        panic!("negated field equality ends in BooleanNot: {negation:?}")
    };
    assert_eq!(operand, equality.result.expect_scalar().id);
    assert_eq!(
        joined_entry_condition(&caller),
        negation.result.expect_scalar().id
    );
}

const JOINED_TWO_PARAMETER_EQUALITY_GUARD_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] { marker: bool; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    Secondary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    data Main [copy] { first: Item; second: Item; }

    machine Main::run(&self, flag: bool, other_flag: bool) {
        transition flag == other_flag {
            true -> take_first()
            _ -> take_second()
        }

        state take_first(&self) {
            let selected: &dyn Measure = &self.first as &dyn Item::Primary;
            let result: bool = finish(selected);
        }

        state take_second(&self) {
            let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
            let result: bool = finish(selected);
        }
    }

    machine finish(erased: &dyn Measure) -> bool {
        let result: bool = erased.measure();
        transition { _ -> result }
    }
"#;

#[test]
fn lowers_joined_two_parameter_equality_guard_into_entry_comparison() {
    let caller = lower_joined_guard_caller(JOINED_TWO_PARAMETER_EQUALITY_GUARD_SOURCE);
    let [flag, other_flag] = caller.parameters.as_slice() else {
        panic!("joined caller keeps both Boolean parameters")
    };
    assert_eq!(flag.scalar_type, semantic_vocabulary::ScalarType::Boolean);
    assert_eq!(
        other_flag.scalar_type,
        semantic_vocabulary::ScalarType::Boolean
    );
    let [equality] = caller.blocks[0].operations.as_slice() else {
        panic!(
            "two-parameter equality lowers to a single comparison: {:?}",
            caller.blocks[0].operations
        )
    };
    let OperationKind::BooleanEqual { left, right } = equality.kind else {
        panic!("two-parameter equality guard lowers to BooleanEqual: {equality:?}")
    };
    assert_eq!(left, flag.id);
    assert_eq!(right, other_flag.id);
    assert_eq!(
        joined_entry_condition(&caller),
        equality.result.expect_scalar().id
    );
}

const JOINED_INDEXED_FIELD_GUARD_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] { marker: bool; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    Secondary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    data Slot [copy] { flag: bool; }

    data Main [copy] { first: Item; second: Item; items: [Slot; 2]; }

    machine Main::run(&self) {
        transition self.items[0].flag {
            true -> take_first()
            _ -> take_second()
        }

        state take_first(&self) {
            let selected: &dyn Measure = &self.first as &dyn Item::Primary;
            let result: bool = finish(selected);
        }

        state take_second(&self) {
            let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
            let result: bool = finish(selected);
        }
    }

    machine finish(erased: &dyn Measure) -> bool {
        let result: bool = erased.measure();
        transition { _ -> result }
    }
"#;

#[test]
fn lowers_joined_indexed_field_guard_into_entry_indexed_read() {
    let caller = lower_joined_guard_caller(JOINED_INDEXED_FIELD_GUARD_SOURCE);
    assert!(caller.parameters.is_empty());
    let [read] = caller.blocks[0].operations.as_slice() else {
        panic!(
            "indexed field guard lowers to a single field read: {:?}",
            caller.blocks[0].operations
        )
    };
    let OperationKind::BooleanStructuralField {
        source, ref path, ..
    } = read.kind
    else {
        panic!("indexed field guard lowers to BooleanStructuralField: {read:?}")
    };
    assert_eq!(source, caller.structural_places[0].id);
    assert_eq!(path.len(), 2);
    assert!(matches!(
        path[0],
        semantic_vocabulary::CanonicalStructuralPathSegment::Field(_)
    ));
    assert!(matches!(
        path[1],
        semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(0)
    ));
    assert_eq!(
        joined_entry_condition(&caller),
        read.result.expect_scalar().id
    );
}

const JOINED_INDEXED_ELEMENT_GUARD_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] { marker: bool; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    Secondary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    data Main [copy] { first: Item; second: Item; counts: [u64; 4]; }

    machine Main::run(&self) {
        transition self.counts[2] < 5 {
            true -> take_first()
            _ -> take_second()
        }

        state take_first(&self) {
            let selected: &dyn Measure = &self.first as &dyn Item::Primary;
            let result: bool = finish(selected);
        }

        state take_second(&self) {
            let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
            let result: bool = finish(selected);
        }
    }

    machine finish(erased: &dyn Measure) -> bool {
        let result: bool = erased.measure();
        transition { _ -> result }
    }
"#;

#[test]
fn lowers_joined_indexed_element_guard_into_entry_primitive_read() {
    let caller = lower_joined_guard_caller(JOINED_INDEXED_ELEMENT_GUARD_SOURCE);
    assert!(caller.parameters.is_empty());
    let operations = caller.blocks[0].operations.as_slice();
    assert_eq!(
        operations.len(),
        3,
        "indexed element guard lowers to a read, a constant, and a comparison: {operations:?}"
    );
    let (read, constant, comparison) = (&operations[0], &operations[1], &operations[2]);
    assert!(matches!(
        comparison.kind,
        OperationKind::IntegerLessThan { .. }
    ));
    assert!(matches!(
        constant.kind,
        OperationKind::IntegerConstant { .. }
    ));
    assert_eq!(
        read.result.expect_scalar().id,
        match comparison.kind {
            OperationKind::IntegerLessThan { left, .. } => left,
            _ => panic!("integer comparison"),
        }
    );
    assert_eq!(
        joined_entry_condition(&caller),
        comparison.result.expect_scalar().id
    );
}

const JOINED_NEGATED_ORDERING_GUARD_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] { marker: bool; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    Secondary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    data Main [copy] { first: Item; second: Item; }

    machine Main::run(&self, pick: u64) {
        transition !(pick < 5) {
            true -> take_first()
            _ -> take_second()
        }

        state take_first(&self) {
            let selected: &dyn Measure = &self.first as &dyn Item::Primary;
            let result: bool = finish(selected);
        }

        state take_second(&self) {
            let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
            let result: bool = finish(selected);
        }
    }

    machine finish(erased: &dyn Measure) -> bool {
        let result: bool = erased.measure();
        transition { _ -> result }
    }
"#;

#[test]
fn lowers_joined_negated_ordering_guard_into_entry_operations() {
    let caller = lower_joined_guard_caller(JOINED_NEGATED_ORDERING_GUARD_SOURCE);
    let [constant, comparison, negation] = caller.blocks[0].operations.as_slice() else {
        panic!(
            "negated ordering lowers to constant, comparison, negation: {:?}",
            caller.blocks[0].operations
        )
    };
    assert!(matches!(
        constant.kind,
        OperationKind::IntegerConstant { .. }
    ));
    assert!(matches!(
        comparison.kind,
        OperationKind::IntegerLessThan { .. }
    ));
    let OperationKind::BooleanNot { operand } = negation.kind else {
        panic!("negated ordering guard ends in BooleanNot: {negation:?}")
    };
    assert_eq!(operand, comparison.result.expect_scalar().id);
    assert_eq!(
        joined_entry_condition(&caller),
        negation.result.expect_scalar().id
    );
}

const JOINED_LOCAL_BOOLEAN_GUARD_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] { marker: bool; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    Secondary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    data Main [copy] { first: Item; second: Item; }

    machine Main::run(&self, pick: u64) {
        let small: bool = pick < 5;
        transition small {
            true -> take_first()
            _ -> take_second()
        }

        state take_first(&self) {
            let selected: &dyn Measure = &self.first as &dyn Item::Primary;
            let result: bool = finish(selected);
        }

        state take_second(&self) {
            let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
            let result: bool = finish(selected);
        }
    }

    machine finish(erased: &dyn Measure) -> bool {
        let result: bool = erased.measure();
        transition { _ -> result }
    }
"#;

#[test]
fn lowers_joined_local_boolean_guard_into_entry_comparison() {
    let caller = lower_joined_guard_caller(JOINED_LOCAL_BOOLEAN_GUARD_SOURCE);
    let [parameter] = caller.parameters.as_slice() else {
        panic!("joined caller keeps its one integer parameter")
    };
    let [constant, comparison] = caller.blocks[0].operations.as_slice() else {
        panic!(
            "local guard substitutes its initializer into a comparison: {:?}",
            caller.blocks[0].operations
        )
    };
    assert!(matches!(
        constant.kind,
        OperationKind::IntegerConstant { .. }
    ));
    let OperationKind::IntegerLessThan { left, right } = comparison.kind else {
        panic!("local guard lowers to IntegerLessThan: {comparison:?}")
    };
    assert_eq!(left, parameter.id);
    assert_eq!(right, constant.result.expect_scalar().id);
    assert_eq!(
        joined_entry_condition(&caller),
        comparison.result.expect_scalar().id
    );
}
