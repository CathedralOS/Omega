use psi_arena::{Arena, OrderedRootArena};
use psi_symbol_resolved_trees::data::{DataDefinition, DataMember};
use psi_symbol_resolved_trees::types::TypeReference;
use psi_symbols::{SymbolHandle, SymbolTable};

pub(super) struct MachineScope<'program> {
    pub(super) symbol: SymbolHandle,
    pub(super) type_parameters: &'program [psi_symbol_resolved_trees::data::TypeParameter],
    pub(super) attached_data: Option<&'program psi_symbol_resolved_trees::name::DiagnosticName>,
    pub(super) attached_data_symbol: SymbolHandle,
    pub(super) inherited_data_members:
        Option<&'program [psi_symbol_resolved_trees::data::DataMember]>,
    pub(super) owned_data: &'program [psi_symbol_resolved_trees::machine::OwnedData],
    /// Only the already-stamped prefix of the current state's statements.
    /// Local receiver types cannot come from a later declaration or from the
    /// binding whose initializer is currently being resolved.
    pub(super) prior_statements: &'program [psi_symbol_resolved_trees::statement::Statement],
    /// All top-level data definitions and the shared member arena -- lets the
    /// receiver walk resolve a NESTED member chain's declared field types
    /// (`self.p.a` -> `p: PairD` -> `a: BoxI`). Empty for scopes built outside
    /// body resolution (field-initializer resolution), where nested method
    /// receivers do not occur.
    pub(super) data_definitions: &'program OrderedRootArena<DataDefinition>,
    pub(super) data_members: &'program Arena<DataMember>,
    pub(super) type_constraints: &'program Arena<psi_symbol_resolved_trees::types::TypeConstraint>,
}

impl MachineScope<'_> {
    pub(super) fn field_type_reference(
        &self,
        symbols: &SymbolTable,
        field_symbol: SymbolHandle,
    ) -> Option<&psi_symbol_resolved_trees::types::TypeReference> {
        if let Some(data_members) = self.inherited_data_members {
            for member in data_members {
                let psi_symbol_resolved_trees::data::DataMember::Field(field) = member else {
                    continue;
                };
                if field.symbol == field_symbol
                    || (field_symbol.is_valid()
                        && field.name.as_str() == symbols.name(field_symbol))
                {
                    return Some(&field.type_reference);
                }
            }
        }

        self.owned_data
            .iter()
            .find(|owned_data| owned_data.symbol == field_symbol)
            .map(|owned_data| &owned_data.type_reference)
    }

    /// Walk a `self`-rooted member chain of SPELLED names (`["self", "p", "a"]`)
    /// through the declared field types and return the type name AFTER the last
    /// segment (`"BoxI"` for `self.p.a` where `a: BoxI`). Each hop's field type
    /// must be a plain `Named` type: shell-wrapped intermediates (`&mut`,
    /// constrained, arrays) return `None` -- conservative, so an unsupported
    /// nested receiver keeps the existing loud unresolved-call error rather
    /// than silently binding 0. `None` unless the root is `self` and every hop
    /// resolves. Used by the nested-receiver symbol stamping (rung 2b of the
    /// receiver-place staircase).
    pub(super) fn nested_self_chain_type(&self, chain: &[&str]) -> Option<&str> {
        let (root, hops) = chain.split_first()?;
        if *root != "self" {
            return None;
        }
        let mut current_symbol = self.attached_data_symbol;
        if !current_symbol.is_valid() {
            return None;
        }
        let mut current_type = self.attached_data?.as_str();
        for hop in hops {
            let definition = self
                .data_definitions
                .iter()
                .find(|definition| definition.symbol == current_symbol)?;
            let field = self
                .data_members
                .span_or_empty(definition.storage.members)
                .iter()
                .find_map(|member| match member {
                    DataMember::Field(field) if field.name.as_str() == *hop => Some(field),
                    _ => None,
                })?;
            (current_symbol, current_type) = match &field.type_reference {
                TypeReference::Named { symbol, name } if symbol.is_valid() => {
                    (*symbol, name.as_str())
                }
                _ => return None,
            };
        }
        Some(current_type)
    }
}
