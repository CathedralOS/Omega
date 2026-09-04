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

pub(super) fn reference_borrow_access_kind(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<psi_checked_trees::BorrowAccessKind> {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { access, .. } => Some(match access {
            psi_language_semantics::ReferenceAccess::Shared => {
                psi_checked_trees::BorrowAccessKind::Read
            }
            psi_language_semantics::ReferenceAccess::Mutable => {
                psi_checked_trees::BorrowAccessKind::Mutable
            }
            psi_language_semantics::ReferenceAccess::WriteOnly => {
                psi_checked_trees::BorrowAccessKind::WriteOnly
            }
        }),
        psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            reference_borrow_access_kind(program, *base_type)
        }
        psi_typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | psi_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | psi_typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | psi_typed_trees::types::TypeReferenceNode::Generic { .. }
        | psi_typed_trees::types::TypeReferenceNode::Named { .. }
        | psi_typed_trees::types::TypeReferenceNode::Slice { .. }
        | psi_typed_trees::types::TypeReferenceNode::Unit => None,
    }
}
