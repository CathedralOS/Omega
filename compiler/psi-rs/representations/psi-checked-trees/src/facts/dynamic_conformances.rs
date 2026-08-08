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
    /// Local binding that stores this selected borrowed dynamic value.
    pub binding: SymbolHandle,
    /// Stable owner coordinates survive state segmentation and expression
    /// remapping, so backend dispatch never has to identify this selection by
    /// a copied expression handle.
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: usize,
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

/// Handle-free selection identity suitable for state-graph and control-flow
/// representations whose expression tables no longer share typed handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicConformanceBindingFact {
    pub binding: SymbolHandle,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub source_data: SymbolHandle,
    pub target_trait: SymbolHandle,
    pub conformance: Option<SymbolHandle>,
    pub rows: Vec<DynamicConformanceRowFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DynamicConformanceBindingFacts {
    pub selections: Vec<DynamicConformanceBindingFact>,
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

    pub fn for_binding(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        binding: SymbolHandle,
    ) -> Option<&DynamicConformanceSelectionFact> {
        self.selections.iter().find(|selection| {
            selection.machine == machine && selection.state == state && selection.binding == binding
        })
    }

    pub fn binding_facts(&self) -> DynamicConformanceBindingFacts {
        DynamicConformanceBindingFacts {
            selections: self
                .selections
                .iter()
                .map(|selection| DynamicConformanceBindingFact {
                    binding: selection.binding,
                    machine: selection.machine,
                    state: selection.state,
                    statement_index: selection.statement_index,
                    source_data: selection.source_data,
                    target_trait: selection.target_trait,
                    conformance: selection.conformance,
                    rows: selection.rows.clone(),
                })
                .collect(),
        }
    }
}

impl DynamicConformanceBindingFacts {
    pub fn for_binding(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        binding: SymbolHandle,
    ) -> Option<&DynamicConformanceBindingFact> {
        self.selections.iter().find(|selection| {
            selection.machine == machine && selection.state == state && selection.binding == binding
        })
    }
}
