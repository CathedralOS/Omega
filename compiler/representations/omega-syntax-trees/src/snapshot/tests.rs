use super::{ExpressionSnapshot, ItemSnapshot, SyntaxTreesSnapshot, TypeReferenceSnapshot};
use crate::expression::{ExpressionNode, TableStructLiteral, TableStructLiteralField};
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
    let one = syntax_trees.expressions.insert(ExpressionNode::Integer(1));
    let field = TableStructLiteralField {
        name: Identifier::generated("value"),
        value: one,
    };
    let field = syntax_trees.expressions.append_struct_field(field);
    let struct_literal = syntax_trees
        .expressions
        .insert(ExpressionNode::StructLiteral(TableStructLiteral {
            type_name: Identifier::generated("Boxed"),
            case_name: None,
            fields: omega_core::arena::HandleSpan::from_parts(field, 1),
        }));

    let data = Item::Data(DataDefinition {
        name: Identifier::generated("Example"),
        type_parameters: omega_core::arena::HandleSpan::empty(),
        members: omega_core::arena::HandleSpan::from_parts(
            syntax_trees
                .items
                .append_data_member(DataMember::Field(DataField {
                    name: Identifier::generated("field"),
                    type_reference: i32_type,
                    initial_value: struct_literal,
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
                type_parameters: Vec::new(),
                members: vec![super::DataMemberSnapshot::Field {
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
                    initial_value: ExpressionSnapshot::StructLiteral {
                        type_name: super::IdentifierSnapshot {
                            text: "Boxed".to_owned(),
                            source_id: 0,
                            start: 0,
                            end: 0,
                            source_backed: false,
                        },
                        fields: vec![super::StructLiteralFieldSnapshot {
                            name: super::IdentifierSnapshot {
                                text: "value".to_owned(),
                                source_id: 0,
                                start: 0,
                                end: 0,
                                source_backed: false,
                            },
                            value: ExpressionSnapshot::Integer { value: 1 },
                        }],
                    },
                }],
            }],
        }
    );

    let json = syntax_trees
        .snapshot_json_pretty()
        .expect("snapshot json should serialize");
    assert!(json.contains("\"root_items\""));
    assert!(json.contains("\"struct_literal\""));
}
