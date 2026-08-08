use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::name::Identifier;

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
    pub binding_name: Identifier,
    /// Stable owner coordinates survive state segmentation and expression
    /// remapping, so backend dispatch never has to identify this selection by
    /// a copied expression handle.
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub source_symbol: SymbolHandle,
    pub source_name: Identifier,
    pub source_path: Vec<Identifier>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicConformanceRowFact {
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    /// Stable slot spelling used only when an older call occurrence has no
    /// resolved requirement symbol. The conformance row remains authoritative;
    /// this never searches implementation names.
    pub requirement_name: Identifier,
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
    pub binding_name: Identifier,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub source_symbol: SymbolHandle,
    pub source_name: Identifier,
    pub source_path: Vec<Identifier>,
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
                    binding_name: selection.binding_name.clone(),
                    machine: selection.machine,
                    state: selection.state,
                    statement_index: selection.statement_index,
                    source_symbol: selection.source_symbol,
                    source_name: selection.source_name.clone(),
                    source_path: selection.source_path.clone(),
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

    pub fn for_receiver(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        binding: SymbolHandle,
        binding_name: &Identifier,
        use_statement_index: usize,
    ) -> Option<&DynamicConformanceBindingFact> {
        self.selections
            .iter()
            .filter(|selection| {
                selection.machine == machine
                    && selection.state == state
                    && selection.statement_index < use_statement_index
                    && if binding.is_valid() {
                        selection.binding == binding
                    } else {
                        selection.binding_name == *binding_name
                    }
            })
            .max_by_key(|selection| selection.statement_index)
    }
}
