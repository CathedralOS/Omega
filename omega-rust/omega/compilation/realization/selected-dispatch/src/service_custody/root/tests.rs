//! ProgramEntry service nomination reads establishment's field traversal.

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let typed = crate::core_service_fixture::typed_with_core_service(
        "selected-dispatch/static-entry.omg",
        source,
    );
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check static entry service fixture")
}

#[test]
fn service_nomination_follows_context_not_namespace() {
    for declaration in [
        "machine run",
        "machine Context::run",
        "machine Namespace::run",
    ] {
        let checked = checked(&format!(
            "pub boundary trait Host {{ machine tick(); }}
             pub boundary trait Wrong {{ machine tick(); }}
             data Inner {{ host: Binding<Host>; }}
             data Context {{ inner: Inner; }}
             data Namespace {{ wrong: Binding<Wrong>; }}
             {declaration}(context: &mut Context) {{}}"
        ));
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str().ends_with("run"))
            .unwrap();
        let requirements = super::program_entry_service_requirements(&checked, machine.symbol);
        assert_eq!(requirements.len(), 1);
        assert_eq!(checked.symbols.display_path(requirements[0], "::"), "Host");
    }
}

#[test]
fn namespace_without_context_nominates_no_services() {
    let checked = checked(
        "pub boundary trait Host { machine tick(); }
         data Namespace { host: Binding<Host>; }
         machine Namespace::run() {}",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("run"))
        .unwrap();
    assert!(super::program_entry_service_requirements(&checked, machine.symbol).is_empty());
}

#[test]
fn explicit_context_establishes_services_and_rejects_occurrence_drift() {
    let mut typed = crate::core_service_fixture::typed_with_core_service(
        "selected-dispatch/static-establishment.omg",
        "pub boundary trait Host { machine tick() -> u32; }
         data Provider {}
         machine Provider::tick_adapter() -> u32 satisfies Host::tick { 7 }
         data Context { host: Binding<Host>; count: u32; }
         machine run(context: &mut Context) reaches Host {
             context.count = context.host.tick();
         }",
    );
    let selected = provider_planning::derive_satisfies_plans(
        provider_planning::ProviderPlanDerivation::unevaluated(
            provider_planning::ProviderPlanningContext::new(
                &typed,
                source::DependencyScope::Product,
            ),
            None,
        ),
    )
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
    assert_eq!(selected.len(), 1);
    let authorizations = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .map(
            |definition| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                requirement: definition.symbol,
            },
        )
        .collect();
    typed.bind_fused_service_erasures(authorizations).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let source = provider_planning::program_entry::validate_selected_program_entry_shape(
        &checked,
        provider_planning::program_entry::SelectedProgramEntry {
            machine_name: machine.name.as_str(),
            machine_symbol: machine.symbol,
            slot: target::TargetProfile::WindowsX64.program_entry_slot(),
        },
    )
    .unwrap();
    let rows =
        super::derive_fused_program_entry_establishments(&checked, &source, &selected, false)
            .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].field_path(), &["host".to_owned()]);

    for (position, symbol) in [
        (1, source.receiver().source_symbol().unwrap()),
        (0, machine.symbol),
    ] {
        let substituted =
            program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
                source.target_slot(),
                source.machine_symbol(),
                source.state_symbol(),
                source.machine_name().to_owned(),
                source.state_name().to_owned(),
                source.normalized_callable_identity().to_owned(),
                program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                    source_position: position,
                    source_symbol: symbol,
                    normalized_type_identity: source
                        .receiver()
                        .normalized_type_identity()
                        .unwrap()
                        .to_owned(),
                },
                Vec::new(),
            )
            .unwrap();
        let errors = super::derive_fused_program_entry_establishments(
            &checked,
            &substituted,
            &selected,
            false,
        )
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.to_string().contains("context occurrence drifted")),
            "{errors:?}"
        );
    }
}
