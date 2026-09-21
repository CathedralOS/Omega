//! A record literal assigned through a field path (`self.h = Handle { .. }`)
//! decomposes into the ordered scalar member stores an authored nested-field
//! sequence would carry: each member keeps its `RecordField` computation
//! coordinate and each store names the target's full carrier path. The same
//! admission covers `addr` members — an address rides the pointer-width
//! integer lane like any other scalar leaf.
use checked_trees::{
    CheckedStructuralScalarFieldStoreValue, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::types::PrimitiveType;

fn plan(source: &str) -> Option<CheckedUnitEffectMachinePlan> {
    let mut typed = crate::tests::parse_typed_trees_with_core_service(source);
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
    }
"#;

#[test]
fn record_literal_stores_each_scalar_member_through_the_field_path() {
    let plan = plan(&format!(
        r#"
        {HOST}
        data Handle {{ raw: u8; tag: u8; }}
        data Main {{ fs: Service<Host>; rc: i32; h: Handle; }}
        machine Main::main(&mut self) reaches Host {{
            self.h = Handle {{ raw: 7, tag: 9 }};
            self.rc = self.fs.close(0);
        }}
    "#
    ))
    .expect("record literal field store is admitted");
    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(raw),
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(tag),
        CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. },
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(result),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("unexpected operation sequence: {:#?}", plan.operations)
    };
    assert_eq!(raw.statement_index, 0);
    assert_eq!(
        raw.carrier_path.as_slice(),
        &[CheckedUnitStructuralPathSegment::Field("h".to_owned())]
    );
    assert_eq!(raw.field_identity, "raw");
    assert_eq!(raw.primitive_type, PrimitiveType::U8);
    assert_eq!(tag.statement_index, 0);
    assert_eq!(tag.carrier_path, raw.carrier_path);
    assert_eq!(tag.field_identity, "tag");
    assert_eq!(result.statement_index, 1);
    assert!(matches!(
        result.value,
        CheckedStructuralScalarFieldStoreValue::ScalarResult { position: 0 }
    ));
}

#[test]
fn record_literal_carries_addr_members_through_the_same_decomposition() {
    let plan = plan(&format!(
        r#"
        {HOST}
        data Handle {{ raw: addr; }}
        data Main {{ fs: Service<Host>; rc: i32; h: Handle; }}
        machine Main::main(&mut self) reaches Host {{
            self.h = Handle {{ raw: 4096 }};
            self.rc = self.fs.close(0);
        }}
    "#
    ))
    .expect("an addr record member store is admitted");
    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
        CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. },
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("unexpected operation sequence: {:#?}", plan.operations)
    };
    assert_eq!(store.field_identity, "raw");
    assert_eq!(store.primitive_type, PrimitiveType::Addr);
    assert_eq!(
        store.carrier_path.as_slice(),
        &[CheckedUnitStructuralPathSegment::Field("h".to_owned())]
    );
}

#[test]
fn a_direct_addr_field_store_is_admitted_like_any_scalar() {
    let plan = plan(&format!(
        r#"
        {HOST}
        data Main {{ fs: Service<Host>; rc: i32; a: addr; }}
        machine Main::main(&mut self) reaches Host {{
            self.a = 4096;
            self.rc = self.fs.close(0);
        }}
    "#
    ))
    .expect("a direct addr field store is admitted");
    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
        CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. },
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("unexpected operation sequence: {:#?}", plan.operations)
    };
    assert_eq!(store.field_identity, "a");
    assert_eq!(store.primitive_type, PrimitiveType::Addr);
    assert!(store.carrier_path.is_empty());
}

#[test]
fn a_nested_addr_field_store_is_admitted_like_any_scalar() {
    let plan = plan(&format!(
        r#"
        {HOST}
        data Handle {{ raw: addr; }}
        data Main {{ fs: Service<Host>; rc: i32; h: Handle; }}
        machine Main::main(&mut self) reaches Host {{
            self.h.raw = 4096;
            self.rc = self.fs.close(0);
        }}
    "#
    ))
    .expect("a nested addr field store is admitted");
    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
        CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. },
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("unexpected operation sequence: {:#?}", plan.operations)
    };
    assert_eq!(store.field_identity, "raw");
    assert_eq!(store.primitive_type, PrimitiveType::Addr);
}

#[test]
fn record_literal_field_store_rejects_a_non_literal_record_source() {
    let plan = plan(&format!(
        r#"
        {HOST}
        data Handle {{ raw: u8; }}
        data Main {{ fs: Service<Host>; rc: i32; h: Handle; }}
        machine Main::main(&mut self) reaches Host {{
            let source: Handle = Handle {{ raw: 7 }};
            self.h = source;
            self.rc = self.fs.close(0);
        }}
    "#
    ));
    assert!(
        plan.is_none(),
        "a bound record copy has no authored member computations to decompose"
    );
}

#[test]
fn record_literal_field_store_rejects_structural_members() {
    let plan = plan(&format!(
        r#"
        {HOST}
        data Inner {{ raw: u8; }}
        data Outer {{ inner: Inner; }}
        data Main {{ fs: Service<Host>; rc: i32; o: Outer; }}
        machine Main::main(&mut self) reaches Host {{
            self.o = Outer {{ inner: Inner {{ raw: 7 }} }};
            self.rc = self.fs.close(0);
        }}
    "#
    ));
    assert!(
        plan.is_none(),
        "a nested record member needs structural transport, not scalar stores"
    );
}
