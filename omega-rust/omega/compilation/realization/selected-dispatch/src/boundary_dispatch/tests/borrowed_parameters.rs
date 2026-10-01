use super::{Arc, CheckedTrees, selected_plan, settle_selected_boundary_adapter_dispatch};
use provider_planning::ProviderPlanDerivation;
const BORROWED_SOURCE: &str = r#"
    pub boundary trait Output {
        machine emit(value: i32);
        machine echo(value: i32) -> i32;
    }
    pub boundary trait Other {
        machine emit(value: i32);
        machine echo(value: i32) -> i32;
    }
    data OutputProvider {}
    machine OutputProvider::emit(value: i32) satisfies Output::emit {}
    machine OutputProvider::echo(value: i32) -> i32 satisfies Output::echo { value }
    data OtherProvider {}
    machine OtherProvider::emit(value: i32) satisfies Other::emit {}
    machine OtherProvider::echo(value: i32) -> i32 satisfies Other::echo { value }

    machine output(service: &mut Binding<Output>, value: i32) reaches Output {
        transition { _ -> send(service, value) }
        state send(service: &mut Binding<Output>, value: i32) { service.emit(value); }
    }
    machine query(service: &Binding<Output>) -> i32 reaches Output { service.echo(35) }
    machine other(service: &mut Binding<Other>) reaches Other { service.emit(2); }
"#;

fn borrowed_fixture() -> (CheckedTrees, effects::SelectedProviderPlanFacts) {
    let mut typed = crate::core_service_fixture::typed_with_core_service(
        "selected-dispatch/borrowed.omg",
        BORROWED_SOURCE,
    );
    let plans = provider_planning::derive_satisfies_plans(ProviderPlanDerivation::unevaluated(
        provider_planning::ProviderPlanningContext::new(&typed, source::DependencyScope::Product),
        None,
    ))
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    super::bind_fixture_fused_service_erasures(&mut typed, &super::selected_every_plan(&plans));
    let selected = selected_plan(&plans, "Output");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap();
    (checked, selected)
}

fn send_state(program: &CheckedTrees) -> typed_trees::state::State {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "output")
        .unwrap();
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "send")
        .unwrap()
        .clone()
}

#[test]
fn borrowed_boundary_dispatch_rejects_symbol_and_method_drift_atomically() {
    for mutation in 0..3 {
        let (mut checked, selected) = borrowed_fixture();
        let send = send_state(&checked);
        match mutation {
            0 => {
                checked
                    .typed
                    .state_parameters
                    .span_mut_or_empty(send.parameters)[0]
                    .symbol = symbols::SymbolHandle::invalid()
            }
            1 => {
                let other_target = checked
                    .traits()
                    .iter()
                    .find(|definition| definition.name.as_str() == "Other")
                    .and_then(|definition| {
                        checked
                            .trait_machine_signatures(definition)
                            .iter()
                            .find(|signature| signature.name.as_str() == "emit")
                    })
                    .unwrap()
                    .symbol;
                let typed_trees::statement::StatementNode::Call(call) = &mut checked
                    .typed
                    .statement_table
                    .statements_mut(send.statement_nodes)[0]
                else {
                    panic!("source statement call");
                };
                call.target_symbol = other_target;
            }
            2 => {
                let typed_trees::statement::StatementNode::Call(call) = &mut checked
                    .typed
                    .statement_table
                    .statements_mut(send.statement_nodes)[0]
                else {
                    panic!("source statement call");
                };
                call.target = typed_trees::name::Identifier::generated("echo");
            }
            _ => unreachable!(),
        }
        let original = Arc::new(checked);
        let mut rejected = Arc::clone(&original);
        assert!(
            settle_selected_boundary_adapter_dispatch(&mut rejected, &selected).is_err(),
            "mutation {mutation}"
        );
        assert!(Arc::ptr_eq(&original, &rejected), "mutation {mutation}");
    }
}

fn binding_fixture() -> (
    CheckedTrees,
    Vec<provider_planning::SelectedProviderReviewProvenance>,
    effects::SelectedProviderPlanFacts,
) {
    let mut typed = crate::core_service_fixture::typed_with_core_service(
        "selected-dispatch/borrowed-binding.omg",
        r#"
        pub boundary trait Output { machine emit(value: i32); }
        data Provider {}
        machine Provider::emit(value: i32) satisfies Output::emit {}
        machine output(service: &mut Binding<Output>, value: i32) reaches Output {
            transition { _ -> send(service, value) }
            state send(service: &mut Binding<Output>, value: i32) { service.emit(value); }
        }
        machine forward(service: &mut Binding<Output>, value: i32) reaches Output {
            output(service, value);
            output(service, value);
        }
        machine shared(first: &Binding<Output>, second: &Binding<Output>) reaches Output {
            first.emit(1);
            second.emit(2);
        }
        machine forward_pair(first: &Binding<Output>, second: &Binding<Output>) reaches Output {
            shared(first, second);
        }
        machine consume(service: Binding<Output>) reaches Output { service.emit(3); }
        machine move_on(service: Binding<Output>) reaches Output { consume(service); }
        "#,
    );
    let selected = provider_planning::derive_satisfies_plans(ProviderPlanDerivation::unevaluated(
        provider_planning::ProviderPlanningContext::new(&typed, source::DependencyScope::Product),
        None,
    ))
    .into_iter()
    .map(
        |derived| provider_planning::SelectedProviderReviewProvenance {
            plan: derived.plan,
            provider: derived.provenance,
            selected_by: provider_planning::ProviderSelectionProvenance::UniqueCoveringCandidate,
            row_compiler_intrinsic_executions: Vec::new(),
        },
    )
    .collect::<Vec<_>>();
    let plans = selected
        .iter()
        .map(|selected| selected.plan.clone())
        .collect::<Vec<_>>();
    let facts = super::selected_every_plan(&plans);
    super::bind_fixture_fused_service_erasures(&mut typed, &facts);
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("borrowed Binding calls and state transfers check");
    (checked, selected, facts)
}

#[test]
fn borrowed_binding_dispatch_rejoins_direct_composed_and_forwarded_parameters() {
    let (checked, selected, facts) = binding_fixture();
    crate::validate_fused_service_terminal_custody(&checked, &selected)
        .expect("Binding receipts preserve their exact source and Fused selection");
    let mut checked = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut checked, &facts)
        .expect("each state-local Binding receiver dispatches independently");
    let composed = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|machine| checked.symbols.display_path(machine.machine, "::") == "output")
        .expect("output retains its named-state graph");
    assert_eq!(composed.states.len(), 2);
    for state in &composed.states {
        let parameter = &state.structural_parameters[0];
        assert_eq!(
            parameter.access,
            checked_trees::CheckedStructuralAccess::MutableBorrow
        );
        assert_eq!(
            parameter.multiplicity,
            language_semantics::Multiplicity::Unrestricted
        );
        assert!(parameter.fused_service_erasure.is_some());
    }
    assert!(!checked.facts.boundary_adapter_dispatch.is_empty());
}

#[test]
fn borrowed_binding_custody_rejects_receipt_access_and_symbol_drift() {
    for mutation in 0..7 {
        let (mut checked, selected, _) = binding_fixture();
        let machine_symbol = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "output")
            .unwrap()
            .symbol;
        let composed = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|machine| machine.machine == machine_symbol)
            .unwrap();
        let parameter = &mut composed.states.last_mut().unwrap().structural_parameters[0];
        match mutation {
            0 => parameter.access = checked_trees::CheckedStructuralAccess::SharedBorrow,
            1 => parameter.multiplicity = language_semantics::Multiplicity::Affine,
            2 => {
                parameter
                    .fused_service_erasure
                    .as_mut()
                    .unwrap()
                    .source_parameter = machine_symbol
            }
            3 => {
                parameter
                    .fused_service_erasure
                    .as_mut()
                    .unwrap()
                    .carrier_type_identity = parameter.type_identity.clone()
            }
            4 => {
                parameter
                    .fused_service_erasure
                    .as_mut()
                    .unwrap()
                    .requirement = machine_symbol
            }
            5 => parameter.position = 1,
            _ => parameter.fused_service_erasure = None,
        }
        assert!(
            crate::validate_fused_service_terminal_custody(&checked, &selected).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn borrowed_binding_forwarding_rejects_same_requirement_argument_swap() {
    let (mut checked, selected, _) = binding_fixture();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward_pair")
        .unwrap()
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .unwrap();
    let call = plan
        .operations
        .iter_mut()
        .find_map(|operation| operation.call_mut())
        .unwrap();
    call.structural_arguments.swap(0, 1);
    assert!(crate::validate_fused_service_terminal_custody(&checked, &selected).is_err());
}
