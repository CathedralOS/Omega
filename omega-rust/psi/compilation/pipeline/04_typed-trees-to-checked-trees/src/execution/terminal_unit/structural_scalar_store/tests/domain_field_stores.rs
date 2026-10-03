//! A scalar replacement retains its field's complete predicate and requests
//! an establishment proof for the completed operand.
use checked_trees::CheckedUnitEffectMachinePlan;

fn plan(source: &str) -> Option<CheckedUnitEffectMachinePlan> {
    let typed = crate::tests::front_end::typed_program(source);
    let checked =
        crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()).expect("check");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main is declared");
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .cloned()
}

fn program(domain: &str) -> String {
    format!(
        "{domain}
        data Main {{ slot: u64 in Slot; }}
        machine Main::main(main: &mut Main) {{ main.slot = 3; }}"
    )
}

#[test]
fn a_store_to_a_domain_beyond_an_interval_retains_its_obligation() {
    let domain = "domain u64::Slot(value) requires value <= 8 && value != 5;";
    assert!(
        plan(&program(domain)).is_some(),
        "the same field declaration governs a non-interval replacement"
    );
}
