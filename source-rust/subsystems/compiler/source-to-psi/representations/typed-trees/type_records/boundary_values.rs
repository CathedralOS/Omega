use crate::source_state::types::TypeReferenceHandle;

#[derive(Clone, Default)]
pub struct BoundaryTypeBinding {
    pub parameter_symbol: symbols::SymbolHandle,
    pub actual: TypeReferenceHandle,
}
