pub(super) fn is_reference_type(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { .. } => true,
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_reference_type(program, *base_type)
        }
        typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | typed_trees::types::TypeReferenceNode::Generic { .. }
        | typed_trees::types::TypeReferenceNode::Named { .. }
        | typed_trees::types::TypeReferenceNode::Slice { .. }
        | typed_trees::types::TypeReferenceNode::Unit => false,
    }
}

pub(super) fn reference_borrow_access_kind(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<checked_trees::BorrowAccessKind> {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { access, .. } => Some(match access {
            language_semantics::ReferenceAccess::Shared => checked_trees::BorrowAccessKind::Read,
            language_semantics::ReferenceAccess::Mutable => {
                checked_trees::BorrowAccessKind::Mutable
            }
            language_semantics::ReferenceAccess::WriteOnly => {
                checked_trees::BorrowAccessKind::WriteOnly
            }
        }),
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            reference_borrow_access_kind(program, *base_type)
        }
        typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | typed_trees::types::TypeReferenceNode::Generic { .. }
        | typed_trees::types::TypeReferenceNode::Named { .. }
        | typed_trees::types::TypeReferenceNode::Slice { .. }
        | typed_trees::types::TypeReferenceNode::Unit => None,
    }
}
