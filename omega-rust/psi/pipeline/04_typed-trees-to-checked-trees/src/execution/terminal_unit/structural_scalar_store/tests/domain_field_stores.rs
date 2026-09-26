//! A write to a field declared `in D` needs D's membership evidence. When D
//! means exactly an interval, the field's bounded carrier states that
//! membership and the store's own range obligation is the whole evidence, so
//! the store is admitted like a bracketed-range field's. Any other predicate
//! is evidence no store retains, and the body stays unadmitted.
use checked_trees::{CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan};

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
        machine Main::main(&mut self) {{ self.slot = 3; }}"
    )
}

#[test]
fn a_store_to_an_exact_interval_domain_field_is_admitted() {
    for domain in [
        "domain u64::Slot requires self <= 8;",
        "domain u64::Slot requires self >= 1 && self < 9;",
    ] {
        let plan = plan(&program(domain)).unwrap_or_else(|| panic!("{domain}: store admitted"));
        assert!(
            plan.operations.iter().any(|operation| matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            )),
            "{domain}: {plan:#?}"
        );
    }
}

#[test]
fn a_store_to_a_domain_beyond_an_interval_stays_unadmitted() {
    let domain = "domain u64::Slot requires self <= 8 && self != 5;";
    assert!(
        plan(&program(domain)).is_none(),
        "`self != 5` is evidence no store retains"
    );
}
