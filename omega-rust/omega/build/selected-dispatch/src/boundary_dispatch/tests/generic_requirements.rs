use super::{
    Arc, CheckedTrees, ProviderBinding, ProviderPlan, bind_fixture_fused_service_erasures, plan,
    selected_every_plan, selected_plan, settle_selected_boundary_adapter_dispatch,
    typed_with_core_service,
};
use provider_planning::ProviderPlanDerivation;
/// A pub boundary trait carrying two requirement-local generic requirements beside
/// a nongeneric one. Without an authored finite `where` roster the generic
/// requirements are dynamically ineligible individually: `ping` still settles
/// while `scan`/`watch` calls reject rather than reaching the unbound generic
/// adapter template at every demanded tuple.
const FAMILY_SOURCE: &str = r#"
    pub boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64;
        machine watch<const Width: u32>(value: u32);
        machine ping(value: u32) -> u32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ScanProvider::watch<const Width: u32>(value: u32) satisfies Scanner::watch {}
    machine ScanProvider::ping(value: u32) -> u32 satisfies Scanner::ping {
        transition { _ -> (value) }
    }
    data Client { service: Service<Scanner>; }
    machine Client::run(&mut self) -> u32 reaches Scanner {
        _ = self.service.ping(1);
        transition { _ -> (self.service.ping(2)) }
    }
"#;

const GENERIC_VALUE_CALL: &str = r#"
    pub boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    data Client { service: Service<Scanner>; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<16>(7) + self.service.scan<32>(8)) }
    }
"#;

const GENERIC_STATEMENT_CALL: &str = r#"
    pub boundary trait Scanner {
        machine watch<const Width: u32>(value: u32);
        machine ping(value: u32) -> u32;
    }
    data ScanProvider {}
    machine ScanProvider::watch<const Width: u32>(value: u32) satisfies Scanner::watch {}
    machine ScanProvider::ping(value: u32) -> u32 satisfies Scanner::ping {
        transition { _ -> (value) }
    }
    data Client { service: Service<Scanner>; }
    machine Client::run(&mut self) -> u32 reaches Scanner {
        self.service.watch<8>(1);
        transition { _ -> (self.service.ping(2)) }
    }
"#;

fn family_fixture(source: &str) -> (CheckedTrees, Vec<ProviderPlan>) {
    let mut typed = typed_with_core_service("selected-dispatch/generic_requirements.omg", source);
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    bind_fixture_fused_service_erasures(&mut typed, &selected_every_plan(&plans));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check generic-requirement fixture");
    (checked, plans)
}

fn requirement_symbol(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .traits()
        .iter()
        .flat_map(|definition| checked.trait_machine_signatures(definition))
        .find(|signature| signature.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing requirement `{name}`"))
        .symbol
}

#[test]
fn generic_requirements_are_excluded_individually() {
    let (checked, plans) = family_fixture(FAMILY_SOURCE);
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let watch = requirement_symbol(&checked, "watch");
    let ping = requirement_symbol(&checked, "ping");
    let original = Arc::new(checked);
    let mut settled = Arc::clone(&original);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("nongeneric siblings still settle beside ineligible requirements");
    assert_eq!(settled.typed, original.typed);
    let dispatch = &settled.facts.boundary_adapter_dispatch;
    assert!(
        dispatch.iter().any(|row| row.requirement == ping),
        "the nongeneric sibling keeps its exact row"
    );
    assert!(
        dispatch
            .iter()
            .all(|row| row.requirement != scan && row.requirement != watch),
        "generic requirements supply no dispatch rows"
    );
}

#[test]
fn value_calls_to_a_generic_requirement_reject() {
    let (checked, plans) = family_fixture(GENERIC_VALUE_CALL);
    let selected = selected_plan(&plans, "Scanner");
    let original = Arc::new(checked);
    let mut rejected = Arc::clone(&original);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut rejected, &selected)
        .expect_err("a demanded tuple cannot silently reach the generic template");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("supplies no executable dispatch row")),
        "{diagnostics:?}"
    );
    assert!(Arc::ptr_eq(&original, &rejected));
}

#[test]
fn statement_calls_to_a_generic_requirement_reject() {
    let (checked, plans) = family_fixture(GENERIC_STATEMENT_CALL);
    let selected = selected_plan(&plans, "Scanner");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("a statement call cannot silently reach the generic template");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("supplies no executable dispatch row")),
        "{diagnostics:?}"
    );
}

#[test]
fn ineligible_rows_skip_adapter_resolution() {
    let mut typed = typed_with_core_service(
        "selected-dispatch/generic_ineligible.omg",
        GENERIC_VALUE_CALL,
    );
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    let mut selected = plan(&plans, "Scanner").clone();
    let row = selected
        .rows
        .iter_mut()
        .find(|row| row.method == "scan")
        .expect("generic scan row");
    row.binding = ProviderBinding::CheckedAdapter {
        machine_identity: "ScanProvider::missing".into(),
        machine_package_identity: None,
    };
    // The mutated binding is part of the selected plan's identity, so the
    // fused-service authorization must be bound from this drifted selection —
    // checking first would authorize the unmutated digest and the routed
    // `Service<Scanner>` field would fail its plan join before the row's own
    // diagnostic can be observed.
    let selected = effects::SelectedProviderPlanFacts::from_selected_plans(vec![selected])
        .expect("select mutated generic plan");
    bind_fixture_fused_service_erasures(&mut typed, &selected);
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check generic-ineligible fixture");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("the ineligible requirement is excluded before realization checks");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("supplies no executable dispatch row")),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("is absent from typed machines")),
        "{diagnostics:?}"
    );
}

/// A generic machine cannot be the exact realization of a nongeneric
/// requirement: its unbound binders never meet a closing tuple, so a selected
/// row naming it must reject rather than point dispatch at the template entry
/// state. Checked conformance already rejects that pair, so reach the guard by
/// drifting a nongeneric row's binding onto the generic adapter.
#[test]
fn generic_adapter_for_a_nongeneric_requirement_rejects() {
    let (checked, plans) = family_fixture(FAMILY_SOURCE);
    let mut selected = plan(&plans, "Scanner").clone();
    let generic_adapter = selected
        .rows
        .iter()
        .find_map(|row| match &row.binding {
            ProviderBinding::CheckedAdapter {
                machine_identity, ..
            } if row.method == "scan" => Some(machine_identity.clone()),
            _ => None,
        })
        .expect("generic scan adapter identity");
    let ping = selected
        .rows
        .iter_mut()
        .find(|row| row.method == "ping")
        .expect("nongeneric ping row");
    ping.binding = ProviderBinding::CheckedAdapter {
        machine_identity: generic_adapter,
        machine_package_identity: None,
    };
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&selected),
        std::slice::from_ref(&selected.name),
    )
    .expect("select drifted plan");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("a generic template is not an exact nongeneric realization");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("is generic over 1 machine binders")),
        "{diagnostics:?}"
    );
}
