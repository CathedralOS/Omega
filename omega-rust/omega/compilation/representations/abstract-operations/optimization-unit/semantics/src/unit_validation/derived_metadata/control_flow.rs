//! Dominance reconstruction and terminator classification.

pub(crate) fn is_terminator(operation: &abstract_operations::AbstractOperation) -> bool {
    use abstract_operations::AbstractOperation as O;
    matches!(
        operation,
        O::Jump { .. }
            | O::Conditional { .. }
            | O::StructuralCase { .. }
            | O::Return { .. }
            | O::ReturnUnit { .. }
            | O::ReturnStructural { .. }
            | O::Crash { .. }
            | O::ExternalCompletion { .. }
    )
}
