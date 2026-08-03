use super::{ItemSnapshot, SyntaxTreesSnapshot, TypeReferenceSnapshot};
use crate::identifier::Identifier;
use crate::item::{DataDefinition, DataField, DataMember, Item};
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
        supply_mode: psi_language_core::DataSupplyMode::CheckedShape,
        lifetime_parameters: Vec::new(),
        type_parameters: psi_arena::HandleSpan::empty(),
        properties: crate::item::DataProperties::default(),
        quotient: None,
        where_facts: psi_arena::HandleSpan::empty(),
        members: psi_arena::HandleSpan::from_parts(
            syntax_trees
                .items
                .append_data_member(DataMember::Field(DataField {
                    identity: None,
                    name: Identifier::generated("field"),
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
                supply: "checked_shape",
                lifetime_parameters: Vec::new(),
                type_parameters: Vec::new(),
                properties: super::DataPropertiesSnapshot {
                    multiplicity: "affine",
                    carry: None,
                },
                quotient: None,
                members: vec![super::DataMemberSnapshot::Field {
                    identity: None,
                    name: super::IdentifierSnapshot {
                        text: "field".to_owned(),
                        source_id: 0,
                        start: 0,
                        end: 0,
                        source_backed: false,
                    },
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
