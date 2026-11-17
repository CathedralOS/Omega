use crate::identifier::Identifier;
use crate::item::{GenericConformanceBound, TypeParameter};
use crate::types::TypeReferenceHandle;
use arena::HandleSpan;

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TypeAliasDefinition {
    pub name: Identifier,
    pub is_public: bool,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    pub authored_expansion: TypeReferenceHandle,
    pub expansion: TypeReferenceHandle,
}
