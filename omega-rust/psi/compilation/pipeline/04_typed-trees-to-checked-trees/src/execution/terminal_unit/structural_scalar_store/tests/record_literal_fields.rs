//! Record operands retain authored order and use ordinary establishment before
//! replacing a field. Literal-backed descriptors also work in local construction.

use checked_trees::{
    CheckedStructuralRecordFieldValue, CheckedStructuralValueKind, CheckedUnitEffectOperationPlan,
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
fn a_quoted_view_member_uses_record_construction_in_authored_order() {
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
    assert!(
        plan.operations.iter().any(|operation| matches!(
            operation, CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. }
                if *value == literal.root
        )),
        "replacement constructs the complete record first"
    );
    assert!(
        !plan.operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceStore(_)
        )),
        "whole construction does not decompose into member stores"
    );
}

#[test]
fn a_quoted_view_member_is_established_as_a_local() {
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
        checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .any(|plan| plan.machine == machine.symbol),
        "ordinary local construction carries the literal descriptor"
    );
}
