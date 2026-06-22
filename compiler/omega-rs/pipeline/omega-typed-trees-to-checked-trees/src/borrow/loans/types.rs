pub(super) fn is_reference_type(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { .. } => true,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_reference_type(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. }
        | omega_typed_trees::types::TypeReferenceNode::Named { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}

pub(super) fn is_mutable_reference_type(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { is_mutable, .. } => *is_mutable,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_mutable_reference_type(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. }
        | omega_typed_trees::types::TypeReferenceNode::Named { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}
