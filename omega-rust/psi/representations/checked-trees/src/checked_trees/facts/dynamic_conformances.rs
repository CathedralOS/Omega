use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::name::Identifier;

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
    /// Exact local/parameter/owned-data/field declaration at the leaf of
    /// `source_path`; never a synthesized member-accessor identity.
    pub source_symbol: SymbolHandle,
    pub source_name: Identifier,
    pub source_path: Vec<Identifier>,
    pub source_data: SymbolHandle,
    pub target_trait: SymbolHandle,
    /// Stable child symbol for the explicitly named conformance.
    pub conformance: Option<SymbolHandle>,
    /// Exact normalized rows retained by a closed implementation block.
    /// Bodyless attached-requirement conformances have no descriptor surface;
    /// descriptor lowering must consume these rows whenever present.
    pub rows: Vec<DynamicConformanceRowFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicConformanceRowFact {
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    /// Complete normalized overload identity of the declaring-trait slot.
    /// Physical descriptor lowering consumes this identity directly rather
    /// than reconstructing a slot from an unqualified requirement spelling.
    pub requirement_identity: String,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    /// Complete normalized callable identity of the selected realization.
    /// This remains logical descriptor identity; it is never a table address.
    pub realization_identity: String,
    pub source: DynamicConformanceRowSource,
}

/// One complete nominal conformance eligible for a bare dynamic parameter.
///
/// The candidate retains exact checked rows rather than a carrier name from
/// which a backend could rediscover attached machines. A concrete call site
/// may select one candidate by its source carrier; physical descriptor
/// lowering uses the same rows to materialize the selected table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicConformanceCandidateFact {
    pub source_data: SymbolHandle,
    pub source_name: Identifier,
    pub conformance: Option<SymbolHandle>,
    pub rows: Vec<DynamicConformanceRowFact>,
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
    pub storages: Vec<DynamicDescriptorStorageFact>,
}

/// Checked lineage for one borrowed dynamic descriptor stored in an exact
/// field of a local record. The embedded selection remains the authority for
/// conformance rows; storage contributes only the destination custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicDescriptorStorageFact {
    pub occurrence: ExpressionHandle,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub destination_binding: SymbolHandle,
    pub destination_name: Identifier,
    pub destination_field: SymbolHandle,
    pub destination_path: Vec<Identifier>,
    pub source_binding: SymbolHandle,
    pub source_name: Identifier,
    pub source_path: Vec<Identifier>,
    pub selection: DynamicConformanceBindingFact,
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

    pub fn stored_receiver(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        binding: SymbolHandle,
        path: &[Identifier],
        use_statement_index: usize,
    ) -> Option<&DynamicDescriptorStorageFact> {
        self.storages
            .iter()
            .filter(|storage| {
                storage.machine == machine
                    && storage.state == state
                    && storage.statement_index < use_statement_index
                    && storage.destination_binding == binding
                    && storage.destination_path == path
            })
            .max_by_key(|storage| storage.statement_index)
    }
}

impl DynamicConformanceBindingFacts {
    pub fn at_statement(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        binding: SymbolHandle,
        statement_index: usize,
    ) -> Option<&DynamicConformanceBindingFact> {
        self.selections.iter().find(|selection| {
            selection.machine == machine
                && selection.state == state
                && selection.binding == binding
                && selection.statement_index == statement_index
        })
    }

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
