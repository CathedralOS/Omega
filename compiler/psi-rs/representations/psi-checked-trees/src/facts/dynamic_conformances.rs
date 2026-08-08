use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;

/// Checked selection behind one local `&T as &dyn Trait` coercion.
///
/// `conformance` names one complete nominal conformance. Descriptor/table
/// lowering must consume this row and must not rediscover an implementation
/// from attached-machine names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicConformanceSelectionFact {
    pub occurrence: ExpressionHandle,
    pub source_data: SymbolHandle,
    pub target_trait: SymbolHandle,
    /// Stable child symbol for a named conformance. `None` denotes the unique
    /// unnamed conformance identified by `source_data + target_trait`.
    pub conformance: Option<SymbolHandle>,
    /// Exact normalized rows retained by a closed implementation block. Empty
    /// only for the legacy attached-machine compatibility form; descriptor
    /// lowering must consume these rows whenever present.
    pub rows: Vec<DynamicConformanceRowFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicConformanceRowFact {
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub source: DynamicConformanceRowSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicConformanceRowSource {
    Inline,
    Reference,
    TraitDefault,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DynamicConformanceFacts {
    pub selections: Vec<DynamicConformanceSelectionFact>,
}

impl DynamicConformanceFacts {
    pub fn for_occurrence(
        &self,
        occurrence: ExpressionHandle,
    ) -> Option<&DynamicConformanceSelectionFact> {
        self.selections
            .iter()
            .find(|selection| selection.occurrence == occurrence)
    }
}
