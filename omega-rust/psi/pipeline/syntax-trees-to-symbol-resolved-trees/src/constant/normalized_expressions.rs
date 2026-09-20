//! Validation of normalized expressions and const arguments.

use diagnostics::Diagnostic;

/// Named origins already retain their selected declaration's value. Direct
/// structured atoms instead require the original constructor expression, even
/// when a malformed input clears its handle.
pub(crate) fn normalization_requires_expression(
    normalization: &syntax_trees::types::ConstArgumentNormalization,
) -> bool {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    normalization.authored_expression.is_valid()
        || (normalization.selections.is_empty()
            && matches!(
                CanonicalConstValue::new("", &normalization.canonical_result_encoding, "")
                    .decode_encoding(),
                Some(
                    DecodedCanonicalConstValue::Array { .. }
                        | DecodedCanonicalConstValue::Record { .. }
                        | DecodedCanonicalConstValue::Variant { .. }
                )
            ))
}

pub(crate) fn validate_normalized_expression(
    syntax: &syntax_trees::SyntaxTrees,
    normalization: &syntax_trees::types::ConstArgumentNormalization,
) -> Result<(), Diagnostic> {
    if normalization_requires_expression(normalization)
        && (!syntax
            .expressions
            .contains_expression(normalization.authored_expression)
            || syntax
                .expressions
                .source_span(normalization.authored_expression)
                != normalization.reference)
    {
        return Err(Diagnostic::error(
            "direct constant argument lost its exact authored expression",
        )
        .with_source_span(normalization.reference));
    }
    Ok(())
}

/// Check the rewritten payload against its captured canonical value before
/// binding the selected declaration to a final symbol.
pub(crate) fn validate_normalized_const_argument(
    argument: &syntax_trees::types::TypeReferenceNode,
    normalization: &syntax_trees::types::ConstArgumentNormalization,
) -> Result<(), Diagnostic> {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    let encoded = CanonicalConstValue::new("", &normalization.canonical_result_encoding, "");
    let matches = match (argument, encoded.decode_encoding()) {
        (
            syntax_trees::types::TypeReferenceNode::Named(name),
            Some(DecodedCanonicalConstValue::Integer { value, .. }),
        ) => name.as_str() == value.to_string(),
        (syntax_trees::types::TypeReferenceNode::Named(name), Some(decoded)) => {
            CanonicalConstValue::from_atom(name.as_str()).is_some_and(|value| {
                let carrier = match &decoded {
                    DecodedCanonicalConstValue::Float { .. } => return false,
                    DecodedCanonicalConstValue::Boolean(_) => "bool",
                    DecodedCanonicalConstValue::Array { type_name, .. }
                    | DecodedCanonicalConstValue::Record { type_name, .. }
                    | DecodedCanonicalConstValue::Variant { type_name, .. }
                    | DecodedCanonicalConstValue::Integer { type_name, .. } => type_name.as_str(),
                };
                value.encoding == normalization.canonical_result_encoding
                    && value.type_name == carrier
                    && value.decode_encoding() == Some(decoded)
            })
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(Diagnostic::error(
            "normalized constant argument value drifted from its retained canonical result",
        )
        .with_source_span(normalization.reference))
    }
}
