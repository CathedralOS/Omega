//! Receiving slots preserve a case view's component and reference carrier.

fn reference_alias_type(
    program: &dyn crate::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    reference.is_valid()
        && matches!(
            program.type_reference_table.type_reference(
                program
                    .type_reference_table
                    .unconstrained_type_reference(reference)
            ),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        )
}

pub(crate) fn report_delivery(
    program: &dyn crate::CheckingInput,
    value: typed_trees::expression::ExpressionHandle,
    destination: typed_trees::types::TypeReferenceHandle,
    expected: Option<language_core::PatternComponent>,
    owner: &str,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> bool {
    if !value.is_valid() {
        return false;
    }
    let actual = program.pattern_component(value);
    if actual.is_none()
        || expected.is_none()
        || actual != expected
        || (actual == Some(language_core::PatternComponent::Case)
            && !reference_alias_type(program, destination))
    {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "{owner} cannot recover a whole data value or change the component of a case-only view"
        )));
        return true;
    }
    false
}
