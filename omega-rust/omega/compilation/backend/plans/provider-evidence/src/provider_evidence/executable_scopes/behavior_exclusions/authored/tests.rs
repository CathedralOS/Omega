use super::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind, authored_behavior_exclusion_set,
    authored_behavior_exclusion_set_in,
};
use semantic_vocabulary::ServiceId;
use symbols::SymbolHandle;
use terminal_psi::{CrashCause, TerminalModule};

fn service_id(raw: u64) -> ServiceId {
    ServiceId::new(raw).unwrap()
}

#[test]
fn service_rows_resolve_against_the_module_catalog_and_crash_rows_stand_alone() {
    let symbol = SymbolHandle::invalid();
    let rows = [
        AuthoredBehaviorExclusion {
            kind: AuthoredBehaviorExclusionKind::Service {
                trait_symbol: symbol,
            },
            selecting_machine: symbol,
            source_span: source::SourceSpan::default(),
        },
        AuthoredBehaviorExclusion {
            kind: AuthoredBehaviorExclusionKind::CrashCause {
                cause: CrashCause::Trap,
                case_symbol: symbol,
            },
            selecting_machine: symbol,
            source_span: source::SourceSpan::default(),
        },
    ];
    // This projection reads the service catalog, not executable bodies.
    let mut module = TerminalModule::for_entry(semantic_vocabulary::MachineId::new(1).unwrap());
    module.services.push(terminal_psi::ServiceDeclaration {
        id: service_id(7),
        identity: "Sink".to_owned(),
        parents: Vec::new(),
    });
    let sink = |_: SymbolHandle| Some("Sink".to_owned());
    let other = |_: SymbolHandle| Some("Console".to_owned());
    let unresolved = |_: SymbolHandle| None;

    // In a module that declares the service, the row names its exact id.
    let resolved = authored_behavior_exclusion_set_in(&rows, &module, &sink);
    assert!(resolved.excludes_service(service_id(7)));
    assert!(resolved.excludes_crash_cause(CrashCause::Trap));
    // A service the module never declares imposes nothing there; the crash
    // row still stands.
    for identity in [
        &other as &dyn Fn(SymbolHandle) -> Option<String>,
        &unresolved,
    ] {
        let resolved = authored_behavior_exclusion_set_in(&rows, &module, identity);
        assert!(resolved.services().is_empty());
        assert!(resolved.excludes_crash_cause(CrashCause::Trap));
    }
    // The module-free set carries crash causes only.
    let crash_only = authored_behavior_exclusion_set(&rows);
    assert!(crash_only.services().is_empty());
    assert!(crash_only.excludes_crash_cause(CrashCause::Trap));
}
