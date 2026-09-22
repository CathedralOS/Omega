//! Fixtures shared by the service reach contract tests: the nominal schema
//! forwarding module and the reach fixture.

mod bounded_boundaries_and_public_wrappers;
mod generic_callback_schemas;
mod nominal_callbacks_and_reach;

use crate::terminal_identities::service_id;
use crate::tests::{SymbolHandle, TerminalModule};
use language_semantics::{ServiceReachId, ServiceReachRowId, ServiceReachSummary};
use semantic_vocabulary::ServiceId;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

fn nominal_schema_forwarding_module() -> terminal_psi::TerminalModule {
    let checked = crate::front_end::checked_program(
        r#"
        boundary trait Console { machine ping(); }
        boundary trait Family { machine call<const Number: u64>(value: u64) -> u64 reaches Console; }
        machine selected<const Count: u64>(value: u64) -> u64 satisfies Family::call reaches Console { Count }
        machine inner<machine Schema>(value: u64) -> u64
        where machine Schema satisfies Family::call;
        { Schema<3>(value) }
        machine outer<machine Schema>(value: u64) -> u64
        where machine Schema satisfies Family::call;
        { inner<Schema>(value) }
        pub machine enter(value: u64) -> u64 { outer<selected>(value) }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("nominal schema forwarded through a private helper")
    .into_artifact();
    drop(checked);
    terminal_codec::decode_module(artifact.semantic_bytes()).expect("forwarded schema reload")
}

struct ReachFixture {
    rows: language_semantics::ServiceReachRowTable,
    console: ServiceReachId,
    console_row: ServiceReachRowId,
    console_and_network_row: ServiceReachRowId,
    terminal_services: Vec<(ServiceReachId, ServiceId)>,
}

fn service_names<'module>(module: &'module TerminalModule, row: &[ServiceId]) -> Vec<&'module str> {
    row.iter()
        .map(|service| {
            module
                .services
                .iter()
                .find(|declaration| declaration.id == *service)
                .expect("declared service")
                .identity
                .as_str()
        })
        .collect()
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

fn checked_public_reach_wrapper() -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(
        r#"
            pub boundary trait Audit { machine record() reaches Audit; }
            pub boundary trait Host { machine ping() reaches Host + Audit; }
            pub data Root {}
            machine Root::helper() reaches Host + Audit { Host::ping(); }
            pub machine Root::enter() invokes Host; { Root::helper(); }
        "#,
    )
}
