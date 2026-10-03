//! A record literal assigned through a field path (`self.h = Handle { .. }`)
//! decomposes into the ordered scalar member stores an authored nested-field
//! sequence would carry: each member keeps its `RecordField` computation
//! coordinate and each store names the target's full carrier path. The same
//! admission covers `addr` members — an address rides the pointer-width
//! integer lane like any other scalar leaf.
//!
//! A quoted member of a shared `&[u8] in D` view field is its literal's
//! octets (`ByteLiteralView`): the literal decomposes into that member's
//! descriptor store beside the scalar stores, in authored member order.

use checked_trees::{
    CheckedByteSequenceStoreDestination, CheckedStructuralRecordFieldValue,
    CheckedStructuralValueKind, CheckedUnitEffectOperationPlan,
};

const TAGGED: &str = r#"
    domain [u8]::Utf8(bytes) requires valid_utf8(bytes);
    data Tag { label: &[u8] in Utf8; weight: i32; }
    data Main { tag: Tag; }
    machine Main::main(root: &mut Main) {
        root.tag = Tag { weight: 4, label: "ring" };
    }
"#;

#[test]
fn a_quoted_view_member_decomposes_into_its_descriptor_store_in_authored_order() {
    let typed = crate::tests::front_end::typed_program(TAGGED);
    let checked =
        crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()).expect("check");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main is declared");
    let state = &checked.typed.machine_states(machine)[0];
    let values = &checked.facts.values.structural_values;
    let literal = values
        .root_at(state.symbol, 0)
        .expect("rooted record literal");
    let CheckedStructuralValueKind::Record { fields, .. } = &values.nodes.get(literal.root).kind
    else {
        panic!("the literal is a plain record value");
    };
    let fields = values
        .record_fields
        .span(*fields)
        .expect("live field roster");
    assert!(matches!(
        fields[0].value,
        CheckedStructuralRecordFieldValue::Scalar(_)
    ));
    let CheckedStructuralRecordFieldValue::Structural(label) = fields[1].value else {
        panic!("the quoted member is a structural node");
    };
    assert_eq!(
        values.nodes.get(label).kind,
        CheckedStructuralValueKind::ByteLiteralView {
            bytes: b"ring".to_vec()
        }
    );

    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .unwrap_or_else(|| {
            panic!(
                "the literal store must plan: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        });
    let stores = plan
        .operations
        .iter()
        .filter(|operation| operation.store_statement_index() == Some(0))
        .collect::<Vec<_>>();
    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(weight),
        CheckedUnitEffectOperationPlan::ByteSequenceStore(label),
    ] = stores.as_slice()
    else {
        panic!("one scalar store then one descriptor store: {stores:#?}");
    };
    assert_eq!(weight.field_identity, "weight");
    assert!(matches!(
        &label.destination,
        CheckedByteSequenceStoreDestination::ViewField { field_identity, carrier_path }
            if field_identity == "label"
                && carrier_path.len() == 1
                && carrier_path == &weight.carrier_path
    ));
}

/// The current construction planner does not emit a literal view member:
/// a record local keeps the rooted value but establishes nothing. Preserve
/// that readiness boundary until ordinary construction owns this producer.
#[test]
fn a_quoted_view_member_is_never_established_as_a_local() {
    let typed = crate::tests::front_end::typed_program(
        r#"
        domain [u8]::Utf8(bytes) requires valid_utf8(bytes);
        data Tag { label: &[u8] in Utf8; weight: i32; }
        data Main { weight: i32; }
        machine Main::main(root: &mut Main) {
            let tag: Tag = Tag { label: "ring", weight: 4 };
            root.weight = tag.weight;
        }
    "#,
    );
    let checked =
        crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()).expect("check");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main is declared");
    let state = &checked.typed.machine_states(machine)[0];
    let values = &checked.facts.values.structural_values;
    let literal = values
        .root_at(state.symbol, 0)
        .expect("rooted record literal");
    let CheckedStructuralValueKind::Record { fields, .. } = &values.nodes.get(literal.root).kind
    else {
        panic!("the literal is a plain record value");
    };
    assert!(
        values
            .record_fields
            .span(*fields)
            .expect("live field roster")
            .iter()
            .any(|field| matches!(field.value,
                CheckedStructuralRecordFieldValue::Structural(node)
                    if matches!(values.nodes.get(node).kind,
                        CheckedStructuralValueKind::ByteLiteralView { .. })))
    );
    assert!(
        !checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .any(|plan| plan.machine == machine.symbol),
        "no record establishment may carry a literal view member"
    );
}
