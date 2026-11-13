//! Identify the compiler-owned content algebra carried by a source type.
use crate::{
    TypedTrees,
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use language_semantics::content::ContentAlgebraIdentity;

pub struct ContentAlgebraApplication {
    pub identity: ContentAlgebraIdentity,
    pub parameter: TypeReferenceHandle,
}

pub fn content_algebra_for_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<ContentAlgebraIdentity> {
    Some(content_algebra_application(program, reference)?.identity)
}

pub fn content_algebra_application(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<ContentAlgebraApplication> {
    if !program.contains_type_reference(reference) {
        return None;
    }
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if !program.symbols.symbols().nodes().is_valid(*base_symbol)
        || program.symbols.symbol_source_origin(*base_symbol)
            != Some(source::SourceOrigin::Toolchain)
    {
        return None;
    }
    let declaration = program.data_definition_by_symbol(*base_symbol)?;
    let [parameter] = program
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        return None;
    };
    if !program.contains_type_reference(*parameter) {
        return None;
    }
    let identity = program.normalized_type_identity_text(*parameter);
    // These closed algebra constructors belong to the retained toolchain declaration.
    // The type operand's display spelling does not select a constructor.
    let algebra = match declaration.name.as_str().rsplit("::").next() {
        Some("IntervalSet") => ContentAlgebraIdentity::IntervalSet {
            coordinate_space: identity,
        },
        Some("CountedQuantity") => ContentAlgebraIdentity::CountedQuantity { unit: identity },
        _ => return None,
    };
    Some(ContentAlgebraApplication {
        identity: algebra,
        parameter: *parameter,
    })
}

/// The normalized description of an authored content clause. This row carries
/// no body-completion or qualification authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentConservationSourcePlan {
    pub source_expression: crate::expression::ExpressionHandle,
    pub plan: language_semantics::content::ContentConservationPlan,
}
