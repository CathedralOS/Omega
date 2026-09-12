use super::*;

struct ReachFixture {
    rows: language_semantics::ServiceReachRowTable,
    console: ServiceReachId,
    console_row: ServiceReachRowId,
    console_and_network_row: ServiceReachRowId,
    terminal_services: Vec<(ServiceReachId, ServiceId)>,
}

fn reach_fixture() -> ReachFixture {
    let mut services = language_semantics::ServiceReachTable::default();
    let console = services.intern(SymbolHandle::from_arena_index(1), "Console");
    let network = services.intern(SymbolHandle::from_arena_index(2), "Network");
    let mut rows = language_semantics::ServiceReachRowTable::default();
    rows.intern(Vec::new());
    let console_row = rows.intern(vec![console]);
    let console_and_network_row = rows.intern(vec![console, network]);
    ReachFixture {
        rows,
        console,
        console_row,
        console_and_network_row,
        terminal_services: vec![(console, service_id(1)), (network, service_id(2))],
    }
}

fn summary(row: ServiceReachRowId) -> ServiceReachSummary {
    ServiceReachSummary {
        direct: row,
        transitive: row,
    }
}

#[test]
fn internal_inferred_reach_is_retained_for_executable_lowering() {
    let fixture = reach_fixture();
    let contract = ServiceReachPlan {
        interface: ServiceReachInterface::InternalInferred,
        checked_inferred: fixture.console_row,
    };
    let mut selected = Vec::new();

    collect_contract_services(
        &fixture.rows,
        contract,
        summary(fixture.console_row),
        &mut selected,
    )
    .expect("private inferred reach remains executable");
    assert_eq!(selected, vec![fixture.console]);
    assert_eq!(
        lower_contract_service_ceiling(
            &fixture.rows,
            contract,
            summary(fixture.console_row),
            &fixture.terminal_services,
        )
        .expect("private inferred reach lowers into the executable contract"),
        vec![service_id(1)],
    );
}

#[test]
fn published_contract_rejects_inferred_reach_outside_published_ceiling() {
    let fixture = reach_fixture();
    let contract = ServiceReachPlan {
        interface: ServiceReachInterface::PublishedCeiling(fixture.console_row),
        checked_inferred: fixture.console_and_network_row,
    };
    let expected =
        LoweringError::Unsupported("checked Unit service reach exceeds its published ceiling");
    let mut selected = Vec::new();

    assert_eq!(
        collect_contract_services(
            &fixture.rows,
            contract,
            summary(fixture.console_and_network_row),
            &mut selected,
        ),
        Err(expected.clone()),
    );
    assert_eq!(
        lower_contract_service_ceiling(
            &fixture.rows,
            contract,
            summary(fixture.console_and_network_row),
            &fixture.terminal_services,
        ),
        Err(expected),
    );
}

fn checked_public_reach_wrapper() -> checked_trees::CheckedTrees {
    checked_source(
        r#"
            pub boundary trait Audit { machine record() reaches Audit; }
            pub boundary trait Host { machine ping() reaches Host + Audit; }
            pub data Root {}
            machine Root::helper() reaches Host + Audit { Host::ping(); }
            pub machine Root::enter() invokes Host; { Root::helper(); }
        "#,
    )
}

#[test]
fn public_wrapper_publishes_propagated_reach_and_pinned_boundary_ceiling() {
    let checked = checked_public_reach_wrapper();
    let wrapper = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .expect("public wrapper");
    assert!(wrapper.is_public);
    assert!(
        checked
            .authored_service_reach_rows_for(wrapper.symbol)
            .next()
            .is_none(),
        "the wrapper does not author its callee's reaches clause",
    );
    let lowered = lower_machine(&checked, "Root::enter").expect("public wrapper lowers");
    let artifact = produce_terminal_artifact(&checked, "Root::enter")
        .expect("public propagated contract publishes");
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("published Terminal module decodes");
    assert_eq!(module, lowered.semantic_module);
    terminal_verifier::verify_module(
        &module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent replay accepts the propagated public contract");
    let mut identities = module
        .services
        .iter()
        .map(|service| service.identity.as_str())
        .collect::<Vec<_>>();
    identities.sort_unstable();
    assert_eq!(identities, ["Audit", "Host"]);
    let mut services = module
        .services
        .iter()
        .map(|service| service.id)
        .collect::<Vec<_>>();
    services.sort_unstable();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("published wrapper entry");
    assert_eq!(entry.published_service_ceiling, services);
    assert_eq!(module.boundary_machines.len(), 1);
    assert_eq!(
        module.boundary_machines[0].published_service_ceiling,
        services
    );
    assert_eq!(module.root_service_reach.concrete, services);
    assert!(
        module
            .root_service_reach
            .installation_dependencies
            .is_empty()
    );
}

#[test]
fn public_wrapper_replay_rejects_removed_propagated_service() {
    let checked = checked_public_reach_wrapper();
    let lowered = lower_machine(&checked, "Root::enter").expect("public wrapper lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("untampered wrapper verifies");
    let mut module = lowered.semantic_module;
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .expect("published wrapper entry");
    assert_eq!(entry.published_service_ceiling.len(), 2);
    entry.published_service_ceiling.pop();
    let result = terminal_verifier::verify_module(
        &module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    );
    assert!(
        matches!(
            result,
            Err(terminal_verifier::VerificationError::Module(
                terminal_verifier::ModuleError::OperationServiceOutsidePublishedCeiling { .. },
            )),
        ),
        "the unchanged callee contract cannot fit a narrowed public wrapper row: {result:?}",
    );
}

#[test]
fn public_contract_lowering_rejects_internal_inference() {
    let fixture = reach_fixture();
    let contract = ServiceReachPlan {
        interface: ServiceReachInterface::InternalInferred,
        checked_inferred: fixture.console_row,
    };
    let expected =
        LoweringError::Unsupported("public Unit contract has no published service ceiling");
    let mut selected = Vec::new();

    assert_eq!(
        collect_published_contract_services(
            &fixture.rows,
            contract,
            summary(fixture.console_row),
            &mut selected,
        ),
        Err(expected.clone()),
    );
    assert_eq!(
        lower_published_service_ceiling(
            &fixture.rows,
            contract,
            summary(fixture.console_row),
            &fixture.terminal_services,
        ),
        Err(expected),
    );
}
