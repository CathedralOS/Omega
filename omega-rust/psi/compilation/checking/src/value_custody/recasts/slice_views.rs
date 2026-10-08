//! Authored slice recast coordinates shared by planning and source replay.
//!
//! These coordinates identify the borrowed place and stated element; they
//! confer no layout, validity or loan authority. The recast judgment owns
//! source admission, and Terminal reconstructs the representation relation.
use typed_trees::{
    expression::ExpressionHandle,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

#[derive(Debug, Clone, Copy)]
pub struct SliceRecastSource {
    pub source: ExpressionHandle,
    pub element: TypeReferenceHandle,
    pub access: language_core::ReferenceAccess,
}

pub fn slice_recast_source(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SliceRecastSource> {
    let formation = super::reference_formation_source(program, expression)?;
    if !formation.view_type.is_valid() {
        return None;
    }
    let TypeReferenceNode::Slice { element_type } = program
        .type_reference_table
        .type_reference(formation.view_type)
    else {
        return None;
    };
    Some(SliceRecastSource {
        source: formation.source,
        element: *element_type,
        access: formation.access,
    })
}
