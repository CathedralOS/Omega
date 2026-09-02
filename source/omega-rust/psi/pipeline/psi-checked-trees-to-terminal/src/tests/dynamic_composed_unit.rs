use super::*;

const DIRECT_DYNAMIC_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] {
        value: bool;
    }

    machine Item::measure(&self) -> bool {
        transition { _ -> false }
    }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool {
            transition { _ -> self.value }
        }
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&self) {
        let erased: &dyn Measure = &self.item as &dyn Item::Primary;
        let result: bool = erased.measure();
    }
"#;

const DIRECT_DYNAMIC_INTEGER_STORE_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] {
        value: i32;
    }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&mut self) {
        self.item.value = 17;
        let erased: &dyn Measure = &self.item as &dyn Item::Primary;
        let result: i32 = erased.measure();
    }
"#;

const MUTATING_REALIZATION_SOURCE: &str = r#"
    trait Measure {
        machine measure(&mut self) -> i32;
    }

    data Item [copy] {
        value: i32;
    }

    Primary: Item satisfies Measure {
        machine measure(&mut self) -> i32 {
            self.value = 23;
            transition { _ -> self.value }
        }
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&mut self) {
        let erased: &mut dyn Measure = &mut self.item as &mut dyn Item::Primary;
        let result: i32 = erased.measure();
    }
"#;

const PROJECTED_MUTATING_REALIZATION_SOURCE: &str = r#"
    trait Measure {
        machine measure(&mut self) -> i32;
    }

    data Payload [copy] {
        value: i32;
    }

    data Item [copy] {
        payload: Payload;
        code: i32;
    }

    Primary: Item satisfies Measure {
        machine measure(&mut self) -> i32 {
            self.payload.value = 23;
            transition { _ -> self.code }
        }
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&mut self) {
        let erased: &mut dyn Measure = &mut self.item as &mut dyn Item::Primary;
        let result: i32 = erased.measure();
    }
"#;

const DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE: &str = r#"
    boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
    }

    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        console: Console;
        item: Item;
    }

    machine Main::run(&mut self) reaches Console {
        let erased: &dyn Measure = &self.item as &dyn Item::Primary;
        let result: i32 = erased.measure();
        transition result == 0 {
            true -> good()
            _ -> bad()
        }

        state good(&mut self) { self.console.exit_process(70); }
        state bad(&mut self) { self.console.exit_process(71); }
    }
"#;

const REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE: &str = r#"
    boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
    }

    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        console: Console;
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) reaches Console {
        let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        let result: i32 = erased.measure();
        transition result == 0 {
            true -> good()
            _ -> bad()
        }

        state good(&mut self) { self.console.exit_process(70); }
        state bad(&mut self) { self.console.exit_process(71); }
    }
"#;

const FORWARDED_REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE: &str = r#"
    boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
    }

    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        console: Console;
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) reaches Console {
        let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        let result: i32 = forward(erased);
        transition result == 0 {
            true -> good()
            _ -> bad()
        }

        state good(&mut self) { self.console.exit_process(70); }
        state bad(&mut self) { self.console.exit_process(71); }
    }

    machine forward(erased: &dyn Measure) -> i32 {
        let result: i32 = erased.measure();
        transition { _ -> result }
    }
"#;

const CHANGED_CONFORMANCE_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Measure { machine measure(&self) -> i32; }
    data Item { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 { transition { _ -> self.value } }
    }

    Secondary: Item satisfies Measure {
        machine measure(&self) -> i32 { transition { _ -> self.value } }
    }

    data Main { decoy: Item; selected: Item; }

    machine Main::run(&mut self) {
        let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Secondary;
        let result: i32 = erased.measure();
    }
"#;

const FORWARDED_REBOUND_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) {
        let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        let result: i32 = forward(erased);
    }

    machine forward(erased: &dyn Measure) -> i32 {
        let result: i32 = erased.measure();
        transition { _ -> result }
    }
"#;

const FORWARDED_DIRECT_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main [copy] { selected: Item; }

    machine Main::run(&mut self) {
        self.selected.value = 23;
        let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
        let result: i32 = forward(erased);
    }

    machine forward(erased: &dyn Measure) -> i32 {
        let result: i32 = erased.measure();
        transition { _ -> result }
    }
"#;

const DIRECT_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch {
        machine touch(&self);
    }

    data Item { value: i32; }

    Primary: Item satisfies Touch {
        machine touch(&self) {
        }
    }

    data Main { item: Item; }

    machine Main::run(&self) {
        let erased: &dyn Touch = &self.item as &dyn Item::Primary;
        erased.touch();
    }
"#;

const FORWARDED_DIRECT_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch {
        machine touch(&self);
    }

    data Item { value: i32; }

    Primary: Item satisfies Touch {
        machine touch(&self) {}
    }

    data Main { selected: Item; }

    machine Main::run(&mut self) {
        let erased: &dyn Touch = &self.selected as &dyn Item::Primary;
        forward(erased);
    }

    machine forward(erased: &dyn Touch) {
        erased.touch();
    }
"#;

const REBOUND_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch {
        machine touch(&self);
    }

    data Item { value: i32; }

    Primary: Item satisfies Touch {
        machine touch(&self) {
        }
    }

    data Main {
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) {
        let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        erased.touch();
    }
"#;

const CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch { machine touch(&self); }
    data Item { value: i32; }

    Primary: Item satisfies Touch { machine touch(&self) {} }
    Secondary: Item satisfies Touch { machine touch(&self) {} }

    data Main { decoy: Item; selected: Item; }

    machine Main::run(&mut self) {
        let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Secondary;
        erased.touch();
    }
"#;

const FORWARDED_CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch { machine touch(&self); }
    data Item { value: i32; }

    Primary: Item satisfies Touch { machine touch(&self) {} }
    Secondary: Item satisfies Touch { machine touch(&self) {} }

    data Main { decoy: Item; selected: Item; }

    machine Main::run(&mut self) {
        let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Secondary;
        forward(erased);
    }

    machine forward(erased: &dyn Touch) { erased.touch(); }
"#;

const FORWARDED_REBOUND_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch {
        machine touch(&self);
    }

    data Item { value: i32; }

    Primary: Item satisfies Touch {
        machine touch(&self) {
        }
    }

    data Main {
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) {
        let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        forward(erased);
    }

    machine forward(erased: &dyn Touch) {
        erased.touch();
    }
"#;

fn direct_dynamic_checked() -> psi_checked_trees::CheckedTrees {
    checked_source(DIRECT_DYNAMIC_SOURCE)
}

fn direct_plan(
    checked: &psi_checked_trees::CheckedTrees,
) -> &psi_checked_trees::CheckedDynamicScalarCallPlan {
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .rebound_scalar_calls
            .is_empty(),
        "direct dynamic call must not enter the rebound catalog"
    );
    let plans = &checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_scalar_calls;
    let [plan] = plans.as_slice() else {
        panic!("one direct dynamic plan expected, got {plans:#?}")
    };
    plan
}

#[test]
fn lowers_rebound_dynamic_custody_as_verified_indirect_terminal_dispatch() {
    let mut checked = checked_source(REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE);
    let catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(catalog.direct_scalar_calls.is_empty());
    let [plan] = catalog.rebound_scalar_calls.as_slice() else {
        panic!("one rebound dynamic plan expected, got {catalog:#?}")
    };
    assert_eq!(plan.initial.fact.statement_index, 0);
    assert_eq!(plan.latest.selection.statement_index, 1);
    assert_eq!(plan.latest.coordinate.statement_index, 2);
    let duplicate = plan.clone();

    let lowered = lower_machine(&checked, "Main::run").expect("rebound dynamic call lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
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
    let _artifact = produce_terminal_artifact(&checked, "Main::run")
        .expect("rebound dynamic module has canonical source-free encoding");

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

    let lowered =
        lower_machine(&checked, "Main::run").expect("changed-conformance rebound should lower");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
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
    let mut checked = checked_source(FORWARDED_REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE);
    let catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [transfer] = catalog.transfers.as_slice() else {
        panic!("one checked dynamic descriptor transfer expected, got {catalog:#?}")
    };
    let [plan] = catalog.rebound_scalar_calls.as_slice() else {
        panic!("one forwarded rebound dynamic plan expected, got {catalog:#?}")
    };
    let psi_checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
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
    assert_eq!(transfer.selection, plan.latest.selection);

    let lowered =
        lower_machine(&checked, "Main::run").expect("transparent forwarded dynamic call lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
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
    let psi_checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { coordinate, .. } =
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
    let mut lowered = lower_machine(&checked, "Main::run")
        .expect("transparent direct scalar forwarding should lower");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
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
        psi_terminal::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    assert_eq!(
        parameter.requirements[0].result,
        psi_terminal::ClosedConformanceCallableResult::I32
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
        psi_terminal::TerminalDynamicDescriptorSource::Selection { ordinal: 1 };
    assert!(psi_terminal_verifier::validate_module(&lowered.semantic_module).is_err());

    let artifact = produce_terminal_artifact(&checked, "Main::run")
        .expect("forwarded direct scalar module should encode");
    let decoded = psi_terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("forwarded direct scalar module should decode");
    assert_eq!(
        decoded.dynamic_dispatch.arguments[0].source,
        psi_terminal::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    assert_dynamic_unit_artifact_executes(&artifact);
}

#[test]
fn lowers_direct_dynamic_unit_without_allocating_a_scalar_result() {
    let checked = checked_source(DIRECT_DYNAMIC_UNIT_SOURCE);
    let lowered = lower_machine(&checked, "Main::run").expect("direct dynamic Unit call lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("direct dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 1);
    assert_eq!(catalog.direct_dispatches.len(), 1);
    assert!(catalog.rebound_descriptors.is_empty());
    assert!(catalog.indirect_dispatches.is_empty());
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("dynamic Unit caller");
    let [operation] = caller.blocks[0].operations.as_slice() else {
        panic!("one direct Unit operation expected")
    };
    assert_eq!(operation.result, OperationResult::Unit);
    assert!(matches!(operation.kind, OperationKind::CallUnit { .. }));
    let [application] = lowered
        .semantic_module
        .closed_conformance_applications
        .as_slice()
    else {
        panic!("one exact dynamic Unit application expected")
    };
    let [callable] = application.realization_callables.as_slice() else {
        panic!("one Unit realization expected")
    };
    assert_eq!(
        callable.result,
        psi_terminal::ClosedConformanceCallableResult::Unit
    );
    let artifact = produce_terminal_artifact(&checked, "Main::run")
        .expect("direct dynamic Unit module encodes");
    let decoded = psi_terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("direct dynamic Unit module decodes");
    assert_eq!(decoded, lowered.semantic_module);
    assert_dynamic_unit_artifact_executes(&artifact);
}

#[test]
fn lowers_rebound_dynamic_unit_to_a_resultless_indirect_dispatch() {
    let checked = checked_source(REBOUND_DYNAMIC_UNIT_SOURCE);
    let mut lowered =
        lower_machine(&checked, "Main::run").expect("rebound dynamic Unit call lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("rebound dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    assert_eq!(catalog.rebound_descriptors.len(), 1);
    assert_eq!(catalog.indirect_dispatches.len(), 1);
    let caller = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("dynamic Unit caller");
    let [operation] = caller.blocks[0].operations.as_mut_slice() else {
        panic!("one indirect Unit operation expected")
    };
    assert_eq!(operation.result, OperationResult::Unit);
    assert!(matches!(
        operation.kind,
        OperationKind::CallDynamicUnit {
            descriptor_ordinal: 0,
            ..
        }
    ));
    operation.result = OperationResult::Scalar(ValueDeclaration {
        id: value_id(999),
        scalar_type: psi_core::ScalarType::Boolean,
    });
    assert!(psi_terminal_verifier::validate_module(&lowered.semantic_module).is_err());
    let artifact = produce_terminal_artifact(&checked, "Main::run")
        .expect("rebound dynamic Unit module encodes");
    assert_dynamic_unit_artifact_executes(&artifact);
}

#[test]
fn retains_changed_conformance_unit_applications_without_a_scalar_result() {
    let lowered = lower_machine(
        &checked_source(CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE),
        "Main::run",
    )
    .expect("changed-conformance dynamic Unit call lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("changed-conformance dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    assert_ne!(
        catalog.selections[0].conformance_application_commitment,
        catalog.selections[1].conformance_application_commitment
    );
    assert_eq!(
        lowered
            .semantic_module
            .closed_conformance_applications
            .len(),
        2
    );
    assert!(matches!(
        lowered.semantic_module.machines[0].blocks[0].operations[0],
        Operation {
            result: OperationResult::Unit,
            kind: OperationKind::CallDynamicUnit { .. },
            ..
        }
    ));
}

#[test]
fn forwards_changed_conformance_unit_custody_without_a_scalar_result() {
    let lowered = lower_machine(
        &checked_source(FORWARDED_CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE),
        "Main::run",
    )
    .expect("forwarded changed-conformance dynamic Unit call lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("forwarded changed-conformance dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    assert_ne!(
        catalog.selections[0].conformance_application_commitment,
        catalog.selections[1].conformance_application_commitment
    );
    assert_eq!(catalog.arguments.len(), 1);
    assert_eq!(catalog.parameter_dispatches.len(), 1);
    assert!(matches!(
        catalog.arguments[0].source,
        psi_terminal::TerminalDynamicDescriptorSource::ReboundDescriptor { .. }
    ));
    assert!(lowered.semantic_module.machines.iter().all(|machine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .all(|operation| operation.result == OperationResult::Unit)
    }));
}

#[test]
fn preserves_forwarded_dynamic_unit_parameter_abi_without_a_result_value() {
    let checked = checked_source(FORWARDED_REBOUND_DYNAMIC_UNIT_SOURCE);
    let lowered = lower_machine(&checked, "Main::run").expect("forwarded dynamic Unit call lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("forwarded dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    let [parameter] = catalog.parameters.as_slice() else {
        panic!("one dynamic Unit parameter expected, got {catalog:#?}")
    };
    let [argument] = catalog.arguments.as_slice() else {
        panic!("one dynamic Unit argument expected, got {catalog:#?}")
    };
    let [dispatch] = catalog.parameter_dispatches.as_slice() else {
        panic!("one dynamic Unit parameter dispatch expected, got {catalog:#?}")
    };
    assert_eq!(parameter.owner, dispatch.owner);
    assert_eq!(
        parameter.requirements[0].result,
        psi_terminal::ClosedConformanceCallableResult::Unit
    );
    assert_eq!(argument.parameter_ordinal, parameter.ordinal);
    assert_eq!(
        argument.source,
        psi_terminal::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal: 0 }
    );
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == argument.owner)
        .expect("forwarded Unit caller");
    let outer = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == argument.operation)
        .expect("outer Unit call");
    assert_eq!(outer.result, OperationResult::Unit);
    assert!(matches!(outer.kind, OperationKind::CallUnit { .. }));
    let helper = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == parameter.owner)
        .expect("forwarded Unit helper");
    let inner = helper
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == dispatch.operation)
        .expect("inner Unit parameter dispatch");
    assert_eq!(inner.result, OperationResult::Unit);
    assert!(matches!(
        inner.kind,
        OperationKind::CallDynamicParameterUnit {
            parameter_ordinal: 0,
            requirement_slot: 0,
            ..
        }
    ));
    assert_eq!(lowered.source_call_occurrences.len(), 2);
    let artifact = produce_terminal_artifact(&checked, "Main::run")
        .expect("forwarded dynamic Unit module encodes");
    psi_terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("forwarded dynamic Unit module decodes");
    assert_dynamic_unit_artifact_executes(&artifact);
}

#[test]
fn forwards_a_direct_dynamic_unit_selection_without_fabricating_a_rebound_descriptor() {
    let checked = checked_source(FORWARDED_DIRECT_DYNAMIC_UNIT_SOURCE);
    let mut lowered =
        lower_machine(&checked, "Main::run").expect("direct forwarded dynamic Unit call lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("direct forwarded dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 1);
    assert!(catalog.rebound_descriptors.is_empty());
    let [parameter] = catalog.parameters.as_slice() else {
        panic!("one direct dynamic Unit parameter expected, got {catalog:#?}")
    };
    let [argument] = catalog.arguments.as_slice() else {
        panic!("one direct dynamic Unit argument expected, got {catalog:#?}")
    };
    let [dispatch] = catalog.parameter_dispatches.as_slice() else {
        panic!("one direct dynamic Unit parameter dispatch expected, got {catalog:#?}")
    };
    assert_eq!(parameter.owner, dispatch.owner);
    assert_eq!(
        argument.source,
        psi_terminal::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    assert_eq!(
        parameter.requirements[0].result,
        psi_terminal::ClosedConformanceCallableResult::Unit
    );
    assert_eq!(lowered.source_call_occurrences.len(), 2);

    lowered.semantic_module.dynamic_dispatch.arguments[0].source =
        psi_terminal::TerminalDynamicDescriptorSource::Selection { ordinal: 1 };
    assert!(psi_terminal_verifier::validate_module(&lowered.semantic_module).is_err());

    let artifact = produce_terminal_artifact(&checked, "Main::run")
        .expect("direct forwarded dynamic Unit module encodes");
    let decoded = psi_terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("direct forwarded dynamic Unit module decodes");
    assert_eq!(
        decoded.dynamic_dispatch.arguments[0].source,
        psi_terminal::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    assert_dynamic_unit_artifact_executes(&artifact);
}

fn assert_dynamic_unit_artifact_executes(artifact: &psi_terminal_codec::CanonicalTerminalArtifact) {
    let module = psi_terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("dynamic Unit module decodes for execution");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("dynamic Unit entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("dynamic Unit entry requires one structural self parameter")
    };
    let argument = psi_terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 1,
        structural_type: parameter.structural_type,
        qualifications: parameter.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution =
        psi_terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &psi_proof_admission::AdmissionProfile::default(),
            &[],
            &[argument],
        )
        .expect("dynamic Unit artifact starts");
    let mut meter = psi_terminal_fuel::TerminalFuelMeter::unbounded();
    assert_eq!(
        execution.resume(&mut meter).expect("dynamic Unit executes"),
        psi_terminal_interpreter::TerminalExecutionStatus::Complete(
            psi_terminal_interpreter::TerminalExecutionResult::Unit,
        ),
    );
}

#[test]
fn preserves_forwarded_dynamic_parameter_abi_from_checked_source() {
    let checked = checked_source(FORWARDED_REBOUND_DYNAMIC_INTEGER_SOURCE);
    let lowered =
        lower_machine(&checked, "Main::run").expect("forwarded dynamic parameter source lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("forwarded dynamic parameter module verifies");

    let catalog = &lowered.semantic_module.dynamic_dispatch;
    let [parameter] = catalog.parameters.as_slice() else {
        panic!("one forwarded dynamic parameter expected, got {catalog:#?}")
    };
    let [argument] = catalog.arguments.as_slice() else {
        panic!("one forwarded dynamic argument expected, got {catalog:#?}")
    };
    let [dispatch] = catalog.parameter_dispatches.as_slice() else {
        panic!("one forwarded parameter dispatch expected, got {catalog:#?}")
    };
    assert_eq!(parameter.owner, dispatch.owner);
    assert_eq!(parameter.ordinal, 0);
    assert_eq!(parameter.source_position, 0);
    assert_eq!(parameter.requirements.len(), 1);
    assert_eq!(argument.parameter_ordinal, parameter.ordinal);
    assert_eq!(
        argument.source,
        psi_terminal::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal: 0 }
    );
    assert!(catalog.indirect_dispatches.is_empty());

    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == argument.owner)
        .expect("forwarded caller machine");
    let caller_operation = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == argument.operation)
        .expect("forwarded caller operation");
    assert!(matches!(
        caller_operation.kind,
        OperationKind::CallStructuralScalar { callee, .. } if callee == parameter.owner
    ));
    let helper = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == parameter.owner)
        .expect("forwarded helper machine");
    let helper_operation = helper
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == dispatch.operation)
        .expect("forwarded helper dispatch operation");
    assert!(matches!(
        helper_operation.kind,
        OperationKind::CallDynamicParameterScalar {
            parameter_ordinal: 0,
            requirement_slot: 0,
            ..
        }
    ));
    assert_eq!(lowered.source_call_occurrences.len(), 2);

    let produced = produce_terminal_artifact(&checked, "Main::run")
        .expect("source-produced dynamic parameter module encodes");
    let decoded = psi_terminal_codec::decode_module(produced.semantic_bytes())
        .expect("source-produced dynamic parameter module decodes");
    assert_eq!(decoded, lowered.semantic_module);
}

fn direct_plan_mut(
    checked: &mut psi_checked_trees::CheckedTrees,
) -> &mut psi_checked_trees::CheckedDynamicScalarCallPlan {
    let plans = &mut checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_scalar_calls;
    let [plan] = plans.as_mut_slice() else {
        panic!("one direct dynamic plan expected")
    };
    plan
}

fn unsupported_message(checked: &psi_checked_trees::CheckedTrees) -> &'static str {
    match lower_machine(checked, "Main::run") {
        Err(LoweringError::Unsupported(message)) => message,
        result => panic!("tampered direct dynamic custody must reject, got {result:?}"),
    }
}

#[test]
fn lowers_exact_named_dynamic_field_call_without_selecting_ambient_lookalike() {
    let checked = direct_dynamic_checked();
    let plan = direct_plan(&checked);
    let selected_callable_identity = plan.realization_identity.clone();
    let source_type_identity = plan.source_type_identity.clone();
    let ambient = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Item::measure")
        .expect("ambient lookalike");
    let ambient_callable_identity = checked
        .typed
        .normalized_machine_overload_identity(ambient)
        .expect("ambient callable identity")
        .identity();
    assert_ne!(ambient.symbol, plan.realization_machine);
    assert_ne!(ambient_callable_identity, selected_callable_identity);

    let lowered = lower_machine(&checked, "Main::run").expect("direct dynamic call lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("lowered direct dynamic module verifies");
    let module = &lowered.semantic_module;
    assert_eq!(module.machines.len(), 2);
    let [application] = module.closed_conformance_applications.as_slice() else {
        panic!("one exact conformance application expected")
    };
    assert_eq!(
        application.subject_identity.as_deref(),
        Some(source_type_identity.as_str())
    );
    let [selection] = module.dynamic_dispatch.selections.as_slice() else {
        panic!("one dynamic selection expected")
    };
    assert_eq!(selection.source.path.len(), 1);
    assert_eq!(
        selection.source.access,
        psi_terminal::StructuralAccess::SharedBorrow
    );
    let [dispatch] = module.dynamic_dispatch.direct_dispatches.as_slice() else {
        panic!("one direct dynamic dispatch expected")
    };
    assert_eq!(
        dispatch.realization_callable_identity,
        selected_callable_identity
    );
    assert!(application.realization_callables.iter().any(|callable| {
        callable.source_callable_identity == selected_callable_identity
            && callable.machine == dispatch.realization
    }));
    assert!(
        !application
            .realization_callables
            .iter()
            .any(|callable| callable.source_callable_identity == ambient_callable_identity)
    );
    let realization = module
        .machines
        .iter()
        .find(|machine| machine.id == dispatch.realization)
        .expect("selected realization machine");
    assert!(
        realization
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                OperationKind::BooleanStructuralField { .. }
            ))
    );
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .all(|operation| !matches!(
                operation.kind,
                OperationKind::BooleanConstant { value: false }
            ))
    );

    let _artifact = produce_terminal_artifact(&checked, "Main::run")
        .expect("direct dynamic module has canonical source-free encoding");
}

#[test]
fn rejects_ambient_lookalike_substitution_in_checked_plan() {
    let mut checked = direct_dynamic_checked();
    let ambient = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Item::measure")
        .expect("ambient lookalike")
        .clone();
    let [ambient_state] = checked.typed.machine_states(&ambient) else {
        panic!("ambient lookalike has one state")
    };
    let ambient_state = ambient_state.clone();
    let ambient_identity = checked
        .typed
        .normalized_machine_overload_identity(&ambient)
        .expect("ambient identity")
        .identity();
    let ambient_contract = checked
        .facts
        .contract_plans
        .for_machine(ambient.symbol)
        .expect("ambient contract")
        .clone();
    let plan = direct_plan_mut(&mut checked);
    plan.realization_machine = ambient.symbol;
    plan.realization_state = ambient_state.symbol;
    plan.realization_identity = ambient_identity;
    plan.realization_contract_report_fingerprint = ambient_contract.report_fingerprint;
    plan.realization_contract_commitment = ambient_contract.commitment;

    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic dispatch lost its exact selected conformance row"
    );
}

#[test]
fn rejects_source_path_and_machine_contract_tampering() {
    let mut source_path = direct_dynamic_checked();
    direct_plan_mut(&mut source_path).source_path.clear();
    assert_eq!(
        unsupported_message(&source_path),
        "direct dynamic source must be one exact attachment field"
    );

    let mut contract = direct_dynamic_checked();
    direct_plan_mut(&mut contract).realization_contract_commitment =
        psi_checked_trees::MachineContractCommitment::from_digest([0xA5; 32]);
    assert_eq!(
        unsupported_message(&contract),
        "direct dynamic machine requires an unsupported contract lane"
    );

    let mut caller_contract = direct_dynamic_checked();
    direct_plan_mut(&mut caller_contract).caller_contract_commitment =
        psi_checked_trees::MachineContractCommitment::from_digest([0x5A; 32]);
    assert_eq!(
        unsupported_message(&caller_contract),
        "direct dynamic machine requires an unsupported contract lane"
    );
}

#[test]
fn lowers_checked_integer_field_store_through_the_selected_dynamic_realization() {
    let checked = checked_source(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE);
    let plan = direct_plan(&checked);
    let store = plan
        .caller_structural_scalar_field_store
        .as_ref()
        .expect("checked caller field store");
    assert_eq!(store.field_identity, "value");

    let lowered = lower_machine(&checked, "Main::run").expect("integer store route lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("integer store route verifies");
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("caller machine");
    assert_eq!(
        caller.structural_parameters[0].access,
        psi_terminal::StructuralAccess::MutableBorrow
    );
    assert!(matches!(
        caller.blocks[0].operations.as_slice(),
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::StructuralScalarFieldStore { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::CallStructuralScalar { .. },
                ..
            }
        ]
    ));
    let realization = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id != lowered.semantic_module.entry)
        .expect("selected realization");
    assert!(matches!(
        realization.blocks[0].operations.as_slice(),
        [psi_terminal::Operation {
            kind: OperationKind::IntegerStructuralField { .. },
            ..
        }]
    ));

    let _artifact = produce_terminal_artifact(&checked, "Main::run")
        .expect("integer store route has canonical source-free encoding");
}

#[test]
fn lowers_checked_mutating_dynamic_realization_before_its_scalar_return() {
    let checked = checked_source(MUTATING_REALIZATION_SOURCE);
    let plan = direct_plan(&checked);
    assert!(plan.realization_structural_scalar_field_store.is_some());

    let lowered = lower_machine(&checked, "Main::run").expect("mutating realization lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("mutating realization module verifies");
    let realization = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id != lowered.semantic_module.entry)
        .expect("selected realization");
    assert_eq!(
        realization.structural_parameters[0].access,
        psi_terminal::StructuralAccess::MutableBorrow
    );
    assert!(matches!(
        realization.blocks[0].operations.as_slice(),
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::StructuralScalarFieldStore { path, .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::IntegerStructuralField { .. },
                ..
            }
        ] if path.is_empty()
    ));
}

#[test]
fn lowers_one_projected_mutating_realization_path_before_its_scalar_return() {
    let checked = checked_source(PROJECTED_MUTATING_REALIZATION_SOURCE);
    let store = direct_plan(&checked)
        .realization_structural_scalar_field_store
        .as_ref()
        .expect("checked projected realization field store");
    assert_eq!(
        store.carrier_path,
        [psi_checked_trees::CheckedUnitStructuralPathSegment::Field(
            "payload".into()
        )]
    );
    assert_eq!(store.field_identity, "value");

    let lowered = lower_machine(&checked, "Main::run").expect("projected realization lowers");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("projected realization module verifies");
    let realization = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id != lowered.semantic_module.entry)
        .expect("selected realization");
    assert!(matches!(
        realization.blocks[0].operations.as_slice(),
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::StructuralScalarFieldStore { path, .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::IntegerStructuralField { .. },
                ..
            }
        ] if path == &[psi_terminal::StructuralPathSegment::Field("payload".into())]
    ));
}

#[test]
fn rejects_mutating_realization_body_that_drifted_from_checked_custody() {
    let mut checked = checked_source(MUTATING_REALIZATION_SOURCE);
    direct_plan_mut(&mut checked)
        .realization_structural_scalar_field_store
        .as_mut()
        .expect("checked realization field store")
        .field_identity = "missing".into();

    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic selected body drifted from checked custody"
    );
}

#[test]
fn lowers_dynamic_scalar_result_into_console_effect_control() {
    let checked = checked_source(DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE);
    assert!(
        direct_plan(&checked)
            .caller_structural_scalar_field_store
            .is_none()
    );
    let lowered = lower_machine(&checked, "Main::run")
        .expect("direct dynamic result control lowers as one module");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("direct dynamic result control verifies");
    assert_eq!(lowered.semantic_module.machines.len(), 2);
    assert_eq!(lowered.semantic_module.boundary_machines.len(), 1);
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("caller machine");
    assert_eq!(caller.blocks.len(), 3);
    assert!(matches!(
        caller.blocks[0].terminator,
        Terminator::Conditional { .. }
    ));
    assert_eq!(
        caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
            .count(),
        2
    );
    assert_eq!(lowered.source_call_occurrences.len(), 3);
    let _artifact = produce_terminal_artifact(&checked, "Main::run")
        .expect("direct dynamic result control has canonical source-free encoding");

    let mut wrong_self = lowered.semantic_module.clone();
    let realization_attachment = wrong_self
        .machines
        .iter()
        .find(|machine| machine.id != wrong_self.entry)
        .and_then(|machine| machine.attachment)
        .expect("realization attachment");
    let caller = wrong_self
        .machines
        .iter_mut()
        .find(|machine| machine.id == wrong_self.entry)
        .expect("caller machine");
    caller.structural_parameters[0].structural_type = realization_attachment;
    let verification_error = psi_terminal_verifier::validate_module(&wrong_self)
        .expect_err("provider specialization rejects a mismatched self type");
    assert!(
        matches!(
            verification_error,
            psi_terminal_verifier::ModuleError::InvalidStructuralSelfParameter { .. }
        ),
        "unexpected error: {verification_error:?}"
    );
    let codec_error = psi_terminal_codec::encode_module(&wrong_self)
        .expect_err("canonical encoding rejects a mismatched provider self type");
    assert!(
        matches!(
            codec_error,
            psi_terminal_codec::CodecError::MalformedStructuralFoundation(
                "provider-backed attachment specialization is incomplete"
            )
        ),
        "unexpected error: {codec_error:?}"
    );
}

#[test]
fn rejects_tampered_checked_dynamic_store_custody() {
    let mut guard = checked_source(DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE);
    direct_plan_mut(&mut guard)
        .unit_continuation
        .as_mut()
        .expect("checked dynamic result continuation")
        .guard =
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Constant(true)));
    assert_eq!(
        unsupported_message(&guard),
        "direct dynamic continuation guard drifted from checked scalar facts"
    );

    let store = direct_plan(&checked_source(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE))
        .caller_structural_scalar_field_store
        .clone()
        .expect("checked caller field store");
    let mut combined = checked_source(DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE);
    direct_plan_mut(&mut combined).caller_structural_scalar_field_store = Some(store);
    assert_eq!(
        unsupported_message(&combined),
        "direct dynamic result control cannot also retain a caller field store"
    );

    let mut checked = checked_source(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE);
    direct_plan_mut(&mut checked)
        .caller_structural_scalar_field_store
        .as_mut()
        .expect("checked caller field store")
        .field_identity = "missing".into();
    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic store field is absent or ambiguous"
    );

    let mut checked = checked_source(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE);
    direct_plan_mut(&mut checked)
        .caller_structural_scalar_field_store
        .as_mut()
        .expect("checked caller field store")
        .carrier_path
        .clear();
    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic caller store drifted from checked custody"
    );
}
