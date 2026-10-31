//! Transparent type declarations keep their own telescope and source custody.

use super::{GenericConformanceBound, TypeParameter};
use crate::identifier::Identifier;
use crate::types::TypeReferenceHandle;
use arena::HandleSpan;

/// `data Alias<...> = TypeExpression;` has no members, properties or storage.
/// Applications normalize to the expansion before type or ABI identity, while
/// this declaration retains its own parameter contracts and exposure boundary.
#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
pub struct TypeAliasDefinition {
    pub name: Identifier,
    pub is_public: bool,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    /// Immutable declaration syntax, never a type-work root. Expansion always
    /// binds these source occurrences, including after an activated re-entry.
    pub authored_expansion: TypeReferenceHandle,
    pub expansion: TypeReferenceHandle,
}
