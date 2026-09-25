//! A write to a field of data whose `where` facts each compare one of its own
//! fields with a literal. Those facts are the fields' bounds, so the store's
//! range obligation re-proves them as it does a bracketed field's. A fact
//! relating two fields is evidence no store retains.
use checked_trees::{
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralTypeShape,
};

fn plan(
    source: &str,
) -> (
    Option<CheckedUnitEffectMachinePlan>,
    checked_trees::CheckedTrees,
) {
    let typed = crate::tests::front_end::typed_program(source);
    let checked =
        crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()).expect("check");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main is declared")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine)
        .cloned();
    (plan, checked)
}

fn program(facts: &str) -> String {
    format!(
        "data Main where {facts} {{ slot: u64; spare: u64; }}
        machine Main::main(&mut self) {{ self.spare = 4; self.slot = 3; }}"
    )
}

#[test]
fn a_store_to_an_interval_where_field_is_admitted_with_its_bounds() {
    // Each admits the zero value, so the data is not zero-gated.
    for facts in ["slot <= 8", "slot < 9 && spare <= 4", "0 <= slot, slot <= 8"] {
        let (plan, checked) = plan(&program(facts));
        let plan = plan.unwrap_or_else(|| panic!("{facts}: store admitted"));
        assert!(
            plan.operations.iter().any(|operation| matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            )),
            "{facts}: {plan:#?}"
        );
        let bounded = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .any(|structural| match &structural.shape {
                CheckedUnitStructuralTypeShape::Record { fields } => fields.iter().any(|field| {
                    field.identity.ends_with("slot")
                        && matches!(
                            field.field_type,
                            CheckedUnitStructuralFieldType::BoundedInteger(_)
                        )
                }),
                _ => false,
            });
        assert!(bounded, "{facts}: `slot` carries its stated bounds");
    }
}

#[test]
fn a_store_under_a_relational_where_fact_stays_unadmitted() {
    let (plan, _) = plan(&program("slot <= spare"));
    assert!(
        plan.is_none(),
        "`slot <= spare` relates two fields; no store retains that evidence"
    );
}
