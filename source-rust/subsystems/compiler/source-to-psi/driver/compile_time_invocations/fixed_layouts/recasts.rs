//! A borrow checks geometry at the original place and at its destination.
//! Follow only the storage that composes those representations.
use crate::source_state::TypedTrees;
use crate::source_state::data::DataMember;
use crate::source_state::layout_storage::LayoutStorageArguments;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn storage(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Vec<LayoutStorageArguments> {
    let mut pending = vec![reference];
    let mut visited = std::collections::HashSet::new();
    let mut applications = Vec::new();
    while let Some(reference) = pending.pop() {
        let reference = typed
            .type_reference_table
            .unconstrained_type_reference(reference);
        if !visited.insert(reference) {
            continue;
        }
        if let Some(storage) = typed.layout_storage_arguments(reference) {
            if typed
                .closed_data_definition(storage.semantic_type)
                .is_some_and(|schema| schema.type_parameters.is_empty())
                && crate::source_state::domain::index_argument_is_closed(typed, storage.policy)
            {
                applications.push(storage);
                pending.push(storage.semantic_type);
            }
            continue;
        }
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray { element_type, .. } => pending.push(*element_type),
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                if let Some(data) = typed.closed_data_definition(reference) {
                    for member in typed.data_members(data) {
                        match member {
                            DataMember::Field(field) if !field.relevance.is_erased() => {
                                pending.push(field.type_reference)
                            }
                            DataMember::Variant(case) => pending.extend(
                                typed
                                    .data_payload_fields(case)
                                    .iter()
                                    .filter(|field| !field.relevance.is_erased())
                                    .map(|field| field.type_reference),
                            ),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
    applications
}
