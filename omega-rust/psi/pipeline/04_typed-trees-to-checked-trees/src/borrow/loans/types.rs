pub(super) fn is_reference_type(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    type_reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Reference { .. } => true,
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_reference_type(program, *base_type)
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Generic { .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Named { .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Slice { .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Unit => false,
    }
}

pub(crate) fn reference_borrow_access_kind(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    type_reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
) -> Option<crate::checked_trees::BorrowAccessKind> {
    match program.type_reference_table.type_reference(type_reference) {
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Reference { access, .. } => Some(match access {
            language_semantics::ReferenceAccess::Shared => crate::checked_trees::BorrowAccessKind::Read,
            language_semantics::ReferenceAccess::Mutable => {
                crate::checked_trees::BorrowAccessKind::Mutable
            }
            language_semantics::ReferenceAccess::WriteOnly => {
                crate::checked_trees::BorrowAccessKind::WriteOnly
            }
        }),
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            reference_borrow_access_kind(program, *base_type)
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Generic { .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Named { .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Slice { .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Unit => None,
    }
}
