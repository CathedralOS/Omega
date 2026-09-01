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

fn direct_dynamic_checked() -> psi_checked_trees::CheckedTrees {
    checked_source(DIRECT_DYNAMIC_SOURCE)
}

fn direct_plan(
    checked: &psi_checked_trees::CheckedTrees,
) -> &psi_checked_trees::CheckedDirectDynamicScalarCallPlan {
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

fn direct_plan_mut(
    checked: &mut psi_checked_trees::CheckedTrees,
) -> &mut psi_checked_trees::CheckedDirectDynamicScalarCallPlan {
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
fn rejects_tampered_checked_dynamic_store_custody() {
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
