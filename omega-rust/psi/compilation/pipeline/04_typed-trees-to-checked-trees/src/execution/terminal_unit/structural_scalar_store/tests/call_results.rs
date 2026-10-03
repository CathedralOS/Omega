//! A structural scalar field store may consume the SSA result of the scalar
//! call its own statement performs. The authored form binds no local, so the
//! checked plan names the call's dense scalar-namespace position instead of an
//! `AssignmentValue` expression, and the store follows its producing call in
//! ordinary evaluation order.
use checked_trees::CheckedUnitEffectMachinePlan;

fn plan(source: &str) -> Option<CheckedUnitEffectMachinePlan> {
    let mut typed = crate::tests::front_end::typed_program_with_core_service(source);
    crate::tests::bind_fixture_fused_service_erasures(&mut typed);
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

const HOST: &str = r#"
    pub boundary trait Host {
        machine close(fd: i32) -> i32 reaches Host;
        machine flush(fd: i32) -> i64 reaches Host;
    }
"#;

#[test]
fn a_call_result_field_store_rejects_missing_routed_membership() {
    // A scalar predicate cannot grant the independent provenance required by
    // the destination's routed domain.
    let plan = plan(&format!(
        r#"
        domain i32::Degrees established by AngleSource::degrees;
        trait AngleSource {{ machine degrees() -> i32 ensures result in i32::Degrees; }}
        {HOST}
        data Main {{ fs: Binding<Host>; fd_in: i32; rc: i32 in Degrees; }}
        machine Main::main(&mut self) reaches Host {{
            self.rc = self.fs.close(self.fd_in);
        }}
    "#
    ));
    assert!(
        plan.is_none(),
        "a domain-constrained field must not take an unproved call result: {plan:#?}"
    );
}
