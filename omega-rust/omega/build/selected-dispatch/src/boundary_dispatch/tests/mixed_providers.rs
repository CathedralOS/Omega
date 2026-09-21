use super::{
    Arc, CheckedTrees, ProviderPlan, ProviderPlanDerivation, adapter_entry_symbol,
    bind_fixture_fused_service_erasures, plan, selected_every_plan, selected_plan,
    settle_selected_boundary_adapter_dispatch, typed_with_core_service,
};

fn mixed_provider_fixture(parameter: bool) -> (CheckedTrees, Vec<ProviderPlan>) {
    let receiver_source = if parameter {
        "machine emit(service: Service<Output>) reaches Output { service.emit(7); }"
    } else {
        "data Client { service: Service<Output>; }
         machine Client::emit(&mut self) reaches Output { self.service.emit(7); }"
    };
    let source = format!(
        r#"
        pub boundary trait Output {{ machine emit(value: i32); }}
        data OutputProvider {{}}
        machine OutputProvider::emit(value: i32) satisfies Output::emit
            via Binding::CompilerIntrinsic;
        pub boundary trait Echo {{ machine echo(value: i32) -> i32; }}
        data EchoProvider {{}}
        machine EchoProvider::echo(value: i32) -> i32 satisfies Echo::echo {{ value }}
        data EchoClient {{ service: Service<Echo>; }}
        machine EchoClient::run(&mut self) -> i32 reaches Echo {{ self.service.echo(35) }}
        {receiver_source}
        "#
    );
    let mut typed = typed_with_core_service("selected-dispatch/mixed.omg", &source);
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    assert_eq!(plans.len(), 2);
    bind_fixture_fused_service_erasures(&mut typed, &selected_every_plan(&plans));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check mixed intrinsic and checked provider source");
    if parameter {
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter()
                .any(|machine| {
                    machine
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.fused_service_erasure.is_some())
                }),
            "the routed parameter must actually retain its Fused receipt"
        );
    }
    (checked, plans)
}

fn assert_mixed_provider_dispatch(parameter: bool) {
    let (checked, plans) = mixed_provider_fixture(parameter);
    let selected = selected_every_plan(&plans);
    let expected = adapter_entry_symbol(&checked, "EchoProvider::echo");
    let original = Arc::new(checked);
    let mut settled = Arc::clone(&original);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("an intrinsic service must coexist with an unrelated checked adapter");
    assert_eq!(settled.typed, original.typed);
    assert!(!settled.facts.boundary_adapter_dispatch.is_empty());
    assert!(
        settled
            .facts
            .boundary_adapter_dispatch
            .iter()
            .all(|dispatch| { dispatch.realization_state == expected }),
        "only the checked Echo provider may acquire adapter dispatch"
    );
}

#[test]
fn mixed_provider_service_field_joins_intrinsic_plan_without_adapter() {
    assert_mixed_provider_dispatch(false);
}

#[test]
fn mixed_provider_service_parameter_joins_intrinsic_plan_without_adapter() {
    assert_mixed_provider_dispatch(true);
}

#[test]
fn mixed_provider_service_receipts_reject_missing_and_substituted_plans_atomically() {
    for parameter in [false, true] {
        for drift in [
            "missing",
            "changed digest",
            "other requirement",
            "foreign package",
        ] {
            let (mut checked, plans) = mixed_provider_fixture(parameter);
            let requirement = checked
                .traits()
                .iter()
                .find(|definition| definition.name.as_str() == "Output")
                .expect("intrinsic requirement")
                .symbol;
            let mut digest = *plan(&plans, "Output").identity_digest().as_bytes();
            let selected = match drift {
                "missing" => selected_plan(&plans, "Echo"),
                "changed digest" => {
                    digest[0] ^= 1;
                    selected_every_plan(&plans)
                }
                "other requirement" => {
                    digest = *plan(&plans, "Echo").identity_digest().as_bytes();
                    selected_every_plan(&plans)
                }
                "foreign package" => {
                    let mut foreign_plans = plans.clone();
                    let foreign = foreign_plans
                        .iter_mut()
                        .find(|plan| plan.schema.trait_name == "Output")
                        .expect("same-spelled foreign plan");
                    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x41; 32])
                        .expect("nonzero foreign package identity");
                    foreign.schema.trait_package_identity = Some(package);
                    for method in &mut foreign.schema.methods {
                        method.requirement_owner_package_identity = Some(package);
                    }
                    digest = *foreign.identity_digest().as_bytes();
                    selected_every_plan(&foreign_plans)
                }
                _ => unreachable!(),
            };
            if parameter {
                let receipt = checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .machines
                    .iter_mut()
                    .flat_map(|machine| &mut machine.structural_parameters)
                    .filter_map(|parameter| parameter.fused_service_erasure.as_mut())
                    .find(|receipt| receipt.requirement == requirement)
                    .expect("intrinsic parameter receipt");
                receipt.provider_plan_digest = digest;
            } else {
                let authorizations = checked
                    .traits()
                    .iter()
                    .filter_map(|definition| checked.fused_service_erasure(definition.symbol))
                    .map(|mut authorization| {
                        if authorization.requirement == requirement {
                            authorization.provider_plan_digest = digest;
                        }
                        authorization
                    })
                    .collect();
                checked
                    .typed
                    .bind_fused_service_erasures(authorizations)
                    .expect("nonzero receipt with exact requirement symbol");
            }
            let original = Arc::new(checked);
            let mut settled = Arc::clone(&original);
            let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
                .expect_err("a missing or substituted plan must not authorize erasure");
            let receiver = if parameter { "parameter" } else { "field" };
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains(&format!("routed service {receiver}"))
                        && diagnostic
                            .message
                            .contains("no exact Fused selected-provider-plan join")
                }),
                "{receiver}, {drift}: {diagnostics:?}"
            );
            assert!(
                Arc::ptr_eq(&settled, &original),
                "failure must not publish partial dispatch"
            );
        }
    }
}
