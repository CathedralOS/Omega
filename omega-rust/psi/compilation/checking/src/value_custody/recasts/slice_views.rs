//! Authored slice recast coordinates shared by planning and source replay.
//!
//! These coordinates identify the borrowed place and stated element; they
//! confer no layout, validity or loan authority. The recast judgment owns
//! source admission, and Terminal reconstructs the representation relation.
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    types::{TypeReferenceHandle, TypeReferenceNode},
};

#[derive(Debug, Clone, Copy)]
pub struct SliceRecastSource {
    pub source: ExpressionHandle,
    pub element: TypeReferenceHandle,
    pub access: language_core::ReferenceAccess,
}

pub fn slice_recast_source(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SliceRecastSource> {
    let initializer = super::strip_mutable(program, expression);
    let ExpressionNode::Cast(cast) = program.expression_table.expression(initializer) else {
        return None;
    };
    if !cast.form.is_recast() {
        return None;
    }
    let TypeReferenceNode::Slice { element_type } = program
        .type_reference_table
        .type_reference(cast.target_type)
    else {
        return None;
    };
    let access = if cast.form == language_core::cast_form::CastForm::RecastMutable {
        language_core::ReferenceAccess::Mutable
    } else {
        language_core::ReferenceAccess::Shared
    };
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression)
        && borrow.access != access
    {
        return None;
    }
    Some(SliceRecastSource {
        source: super::strip_mutable(program, cast.value),
        element: *element_type,
        access,
    })
}
