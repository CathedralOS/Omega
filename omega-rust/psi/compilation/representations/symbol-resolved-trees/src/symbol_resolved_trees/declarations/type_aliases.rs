//! Source alias declarations and the independent obligations on their uses.

use crate::data::TypeParameter;
use crate::machine::GenericConformanceBound;
use crate::name::DiagnosticName;
use crate::types::TypeReference;
use arena::HandleSpan;
use symbols::SymbolHandle;

/// The declaration is retained for namespace, telescope and source admission.
/// `expansion` has the ordinary underlying type; the alias owns no nominal
/// data identity, fields, geometry or runtime entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TypeAliasDefinition {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub is_public: bool,
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    pub expansion: TypeReference,
}
