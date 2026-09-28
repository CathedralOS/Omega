use super::selected_program_storage_source_extent_value_layout;
use calling_conventions::ValueShape;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

fn extent(length_type: &str) -> (TypedTrees, TypeReferenceHandle) {
    let mut typed = TypedTrees::default();
    let base_type = typed.type_reference_table.insert(TypeReferenceNode::Named {
        symbol: SymbolHandle::invalid(),
        name: Identifier::from("addr"),
    });
    let length_type = typed.type_reference_table.insert(TypeReferenceNode::Named {
        symbol: SymbolHandle::invalid(),
        name: Identifier::from(length_type),
    });
    let members = typed.tables.data_members.insert_many([
        DataMember::Field(DataField {
            symbol: SymbolHandle::from_arena_index(2),
            name: Identifier::from("base"),
            type_reference: base_type,
            ..DataField::default()
        }),
        DataMember::Field(DataField {
            symbol: SymbolHandle::from_arena_index(3),
            name: Identifier::from("length"),
            type_reference: length_type,
            ..DataField::default()
        }),
    ]);
    let symbol = SymbolHandle::from_arena_index(1);
    typed.roots.data_definitions = typed.tables.data_definitions.insert_many([DataDefinition {
        symbol,
        name: Identifier::from("Extent"),
        members,
        ..DataDefinition::default()
    }]);
    let root = typed.type_reference_table.insert(TypeReferenceNode::Named {
        symbol,
        name: Identifier::from("Extent"),
    });
    (typed, root)
}

#[test]
fn source_extent_requires_exact_fields_and_target_schema_not_equal_size() {
    let slot = target::TargetProfile::UefiX64.program_entry_slot();
    let (typed, root) = extent("u64");
    let layout = selected_program_storage_source_extent_value_layout(&typed, slot, root)
        .expect("exact source Extent");
    assert_eq!(layout.fields()[0].byte_offset(), 0);
    assert_eq!(layout.fields()[1].byte_offset(), 8);
    assert_eq!(layout.fields()[1].shape(), ValueShape::integer(8, 8));

    let error = selected_program_storage_source_extent_value_layout(
        &typed,
        target::TargetProfile::MacosArm64.program_entry_slot(),
        root,
    )
    .expect_err("foreign schema");
    assert!(error.contains("restricted to the exact UEFI/Microsoft"));

    let (typed, root) = extent("i64");
    let error = selected_program_storage_source_extent_value_layout(&typed, slot, root)
        .expect_err("same size is not source identity");
    assert!(error.contains("exact `base: addr; length: u64`"));
}
