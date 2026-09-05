use super::{ItemSnapshot, SyntaxTreesSnapshot, TypeReferenceSnapshot};
use crate::identifier::Identifier;
use crate::item::{
    DataDefinition, DataField, DataMember, DataVariant, Item, WireDataDefinition, WireDataField,
    WireDataMember,
};
use crate::syntax_trees::SyntaxTrees;
use crate::types::TypeReferenceNode;

#[test]
fn snapshots_materialize_handle_backed_syntax_shape() {
    let mut syntax_trees = SyntaxTrees::new(Default::default());
    let i32_type = syntax_trees
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated("i32")));
    let data = Item::Data(DataDefinition {
        name: Identifier::generated("Example"),
        is_public: true,
        supply_mode: language_core::DataSupplyMode::CheckedShape,
        lifetime_parameters: Vec::new(),
        type_parameters: arena::HandleSpan::empty(),
        generic_instance: None,
        properties: crate::item::DataProperties::default(),
        quotient: None,
        where_facts: arena::HandleSpan::empty(),
        members: arena::HandleSpan::from_parts(
            syntax_trees
                .items
                .append_data_member(DataMember::Field(DataField {
                    identity: None,
                    name: Identifier::generated("field"),
                    relevance: language_core::BindingRelevance::Relevant,
                    type_reference: i32_type,
                })),
            1,
        ),
    });
    syntax_trees.push_root_item(data);

    let snapshot = syntax_trees.snapshot();

    assert_eq!(
        snapshot,
        SyntaxTreesSnapshot {
            source_id: 0,
            root_items: vec![ItemSnapshot::Data {
                name: super::IdentifierSnapshot {
                    text: "Example".to_owned(),
                    source_id: 0,
                    start: 0,
                    end: 0,
                    source_backed: false,
                },
                is_public: true,
                supply: "checked_shape",
                lifetime_parameters: Vec::new(),
                type_parameters: Vec::new(),
                generic_instance: None,
                properties: super::DataPropertiesSnapshot {
                    multiplicity: "affine",
                    carry: None,
                },
                quotient: None,
                where_facts: Vec::new(),
                members: vec![super::DataMemberSnapshot::Field {
                    identity: None,
                    name: super::IdentifierSnapshot {
                        text: "field".to_owned(),
                        source_id: 0,
                        start: 0,
                        end: 0,
                        source_backed: false,
                    },
                    relevance: "relevant",
                    type_reference: TypeReferenceSnapshot::Named {
                        name: super::IdentifierSnapshot {
                            text: "i32".to_owned(),
                            source_id: 0,
                            start: 0,
                            end: 0,
                            source_backed: false,
                        },
                    },
                }],
            }],
        }
    );

    let json = syntax_trees
        .snapshot_json_pretty()
        .expect("snapshot json should serialize");
    assert!(json.contains("\"root_items\""));
}

#[test]
fn variant_snapshot_retains_payload_only_erased_field() {
    let mut syntax_trees = SyntaxTrees::new(Default::default());
    let evidence_type = syntax_trees
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated("Evidence")));
    let payload = syntax_trees.items.append_data_payload_field(DataField {
        identity: Some(7),
        name: Identifier::generated("proof"),
        relevance: language_core::BindingRelevance::Erased,
        type_reference: evidence_type,
    });
    let member = syntax_trees
        .items
        .append_data_member(DataMember::Variant(DataVariant {
            identity: Some(3),
            name: Identifier::generated("Certified"),
            payload: arena::HandleSpan::from_parts(payload, 1),
            retired_payload_identities: Vec::new(),
        }));
    syntax_trees.push_root_item(Item::Data(DataDefinition {
        name: Identifier::generated("Envelope"),
        is_public: false,
        supply_mode: language_core::DataSupplyMode::CheckedShape,
        lifetime_parameters: Vec::new(),
        type_parameters: arena::HandleSpan::empty(),
        generic_instance: None,
        properties: crate::item::DataProperties::default(),
        quotient: None,
        where_facts: arena::HandleSpan::empty(),
        members: arena::HandleSpan::from_parts(member, 1),
    }));

    let snapshot = syntax_trees.snapshot();
    let ItemSnapshot::Data { members, .. } = &snapshot.root_items[0] else {
        panic!("data snapshot");
    };
    let super::DataMemberSnapshot::Variant { payload, .. } = &members[0] else {
        panic!("variant snapshot");
    };
    assert_eq!(payload.len(), 1);
    assert_eq!(payload[0].name.text, "proof");
    assert_eq!(payload[0].relevance, "erased");
    assert_eq!(payload[0].identity, Some(7));
    assert!(matches!(
        &payload[0].type_reference,
        TypeReferenceSnapshot::Named { name } if name.text == "Evidence"
    ));
}

#[test]
fn wire_snapshot_retains_erased_field_relevance() {
    let mut syntax_trees = SyntaxTrees::new(Default::default());
    let evidence_type = syntax_trees
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated("Evidence")));
    let field = syntax_trees
        .items
        .append_wire_data_member(WireDataMember::Field(WireDataField {
            number: 7,
            name: Identifier::generated("proof"),
            relevance: language_core::BindingRelevance::Erased,
            type_reference: evidence_type,
        }));
    syntax_trees.push_root_item(Item::WireData(WireDataDefinition {
        is_public: false,
        name: Identifier::generated("Certified"),
        encoding: None,
        members: arena::HandleSpan::from_parts(field, 1),
    }));

    let snapshot = syntax_trees.snapshot();
    let ItemSnapshot::WireData { members, .. } = &snapshot.root_items[0] else {
        panic!("wire data snapshot");
    };
    let super::WireDataMemberSnapshot::Field {
        number,
        name,
        relevance,
        type_reference,
    } = &members[0]
    else {
        panic!("wire field snapshot");
    };
    assert_eq!(*number, 7);
    assert_eq!(name.text, "proof");
    assert_eq!(*relevance, "erased");
    assert!(matches!(
        type_reference,
        TypeReferenceSnapshot::Named { name } if name.text == "Evidence"
    ));
}
