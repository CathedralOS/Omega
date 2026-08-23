pub(super) fn is_reference_type(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { .. } => true,
        psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_reference_type(program, *base_type)
        }
        psi_typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | psi_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | psi_typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | psi_typed_trees::types::TypeReferenceNode::Generic { .. }
        | psi_typed_trees::types::TypeReferenceNode::Named { .. }
        | psi_typed_trees::types::TypeReferenceNode::Slice { .. }
        | psi_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}

pub(super) fn is_mutable_reference_type(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
            access.is_exclusive()
        }
        psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_mutable_reference_type(program, *base_type)
        }
        psi_typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | psi_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | psi_typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | psi_typed_trees::types::TypeReferenceNode::Generic { .. }
        | psi_typed_trees::types::TypeReferenceNode::Named { .. }
        | psi_typed_trees::types::TypeReferenceNode::Slice { .. }
        | psi_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}
