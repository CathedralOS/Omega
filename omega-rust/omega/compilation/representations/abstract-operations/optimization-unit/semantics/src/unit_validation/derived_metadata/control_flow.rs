//! Dominance reconstruction and terminator classification.

pub(crate) fn is_terminator(operation: &abstract_operations::AbstractOperation) -> bool {
    use abstract_operations::AbstractOperation as O;
    matches!(
        operation,
        abstract_operations::jump_or_conditional_pattern!()
            | O::StructuralCase { .. }
            | abstract_operations::return_or_return_unit_pattern!()
            | abstract_operations::return_structural_or_crash_pattern!()
            | O::InvokeNoNormal { .. }
            | O::ExternalCompletion { .. }
    )
}
