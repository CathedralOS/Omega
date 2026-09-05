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
fn published_contract_rejects_inferred_reach_outside_authored_ceiling() {
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
