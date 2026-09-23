//! A structural scalar field store may consume the SSA result of the scalar
//! call its own statement performs. The authored form binds no local, so the
//! checked plan names the call's dense scalar-namespace position instead of an
//! `AssignmentValue` expression, and the store follows its producing call in
//! ordinary evaluation order.
use checked_trees::{
    CheckedStructuralScalarFieldStoreValue, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan,
};

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
fn a_field_store_reads_the_scalar_call_result_its_own_statement_produced() {
    let plan = plan(&format!(
        r#"
        {HOST}
        data Main {{ fs: Binding<Host>; fd_in: i32; rc: i32; }}
        machine Main::main(&mut self) reaches Host {{
            self.fd_in = 5;
            self.rc = self.fs.close(self.fd_in);
        }}
    "#
    ))
    .expect("the call-result field store is admitted");
    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(literal),
        CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate, result, ..
        },
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("unexpected operation sequence: {:#?}", plan.operations)
    };
    assert_eq!(literal.field_identity, "fd_in");
    assert_eq!(coordinate.statement_index, 1);
    assert_eq!(coordinate.call_ordinal, 0);
    assert_eq!(result.binding_ordinal, 0);
    assert_eq!(store.statement_index, 1);
    assert_eq!(store.field_identity, "rc");
    assert_eq!(
        store.value,
        CheckedStructuralScalarFieldStoreValue::ScalarResult { position: 0 }
    );
}

#[test]
fn consecutive_call_result_stores_keep_dense_scalar_positions() {
    let plan = plan(&format!(
        r#"
        {HOST}
        data Main {{ fs: Binding<Host>; fd_in: i32; rc: i32; written: i64; }}
        machine Main::main(&mut self) reaches Host {{
            self.rc = self.fs.close(self.fd_in);
            self.written = self.fs.flush(self.fd_in);
        }}
    "#
    ))
    .expect("two call-result field stores are admitted");
    let positions = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => Some((
                store.field_identity.as_str(),
                store.statement_index,
                store.value.clone(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        positions,
        vec![
            (
                "rc",
                0,
                CheckedStructuralScalarFieldStoreValue::ScalarResult { position: 0 }
            ),
            (
                "written",
                1,
                CheckedStructuralScalarFieldStoreValue::ScalarResult { position: 1 }
            ),
        ]
    );
}

#[test]
fn a_call_result_field_store_rejects_a_domain_constrained_field() {
    // A call result carries no evidence for the destination's declared domain,
    // and the store vocabulary admits only an unconstrained scalar carrier.
    // The body therefore stays unadmitted rather than granting `Degrees` to a
    // value the boundary never proved.
    let plan = plan(&format!(
        r#"
        domain i32::Degrees;
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
