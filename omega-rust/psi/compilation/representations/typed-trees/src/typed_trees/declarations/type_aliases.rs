//! Transparent declaration headers and their use-site formation obligations.

use crate::data::TypeParameter;
use crate::machine::GenericConformanceBound;
use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use arena::HandleSpan;
use symbols::SymbolHandle;

/// Source metadata only. All semantic type references name `expansion`'s
/// carrier; this header creates no nominal, storage or ABI identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TypeAliasDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub is_public: bool,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    pub expansion: TypeReferenceHandle,
}

/// The exact alias application still owes every declared parameter contract,
/// including a parameter absent from the normalized expansion.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TypeAliasApplication {
    pub instance: TypeReferenceHandle,
    pub declaration: SymbolHandle,
    pub declaration_source: source::SourceSpan,
    pub reference: source::SourceSpan,
    pub arguments: HandleSpan<TypeReferenceHandle>,
    pub lifetime_arguments: Vec<Identifier>,
    pub supplied_operand_owner: SymbolHandle,
}
