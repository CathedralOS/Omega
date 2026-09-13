use arena::{Arena, OrderedRootArena};
use symbol_resolved_trees::data::{DataDefinition, DataMember};
use symbol_resolved_trees::types::TypeReference;
use symbols::{SymbolHandle, SymbolTable};

#[derive(Clone, Copy)]
pub(super) struct AttachedMachine {
    pub(super) owner: SymbolHandle,
    pub(super) machine: SymbolHandle,
}

pub(super) fn attached_machines(
    program: &symbol_resolved_trees::SymbolResolvedTrees,
) -> Vec<AttachedMachine> {
    use symbol_resolved_trees::trait_definition::{
        ConformanceImplementation, ConformanceRowSource,
    };
    program
        .machines
        .iter()
        .enumerate()
        .filter_map(|(ordinal, machine)| {
            // Inline/default realizations belong to their closed conformance map.
            // They are routed there after ordinary resolution, not competing
            // ambient methods. Referenced authored machines remain ordinary.
            let closed_realization = program.conformances.iter().any(|conformance| {
                let ConformanceImplementation::Closed { rows } = &conformance.implementation else {
                    return false;
                };
                rows.iter().any(|row| {
                    matches!(
                        row.source,
                        ConformanceRowSource::Inline | ConformanceRowSource::TraitDefault
                    ) && row.provisional_realization_ordinal.map_or_else(
                        || {
                            row.realization_machine.is_valid()
                                && row.realization_machine == machine.symbol
                        },
                        |selected| selected == ordinal,
                    )
                })
            });
            (!closed_realization
                && machine.symbol.is_valid()
                && machine.attached_data_symbol.is_valid())
            .then_some(AttachedMachine {
                owner: machine.attached_data_symbol,
                machine: machine.symbol,
            })
        })
        .collect()
}

pub(super) struct MachineScope<'program> {
    pub(super) symbol: SymbolHandle,
    pub(super) attached_machines: &'program [AttachedMachine],
    pub(super) type_parameters: &'program [symbol_resolved_trees::data::TypeParameter],
    pub(super) attached_data: Option<&'program symbol_resolved_trees::name::DiagnosticName>,
    pub(super) attached_data_symbol: SymbolHandle,
    pub(super) inherited_data_members: Option<&'program [symbol_resolved_trees::data::DataMember]>,
    pub(super) owned_data: &'program [symbol_resolved_trees::machine::OwnedData],
    /// Only the already-stamped prefix of the current state's statements.
    /// Local receiver types cannot come from a later declaration or from the
    /// binding whose initializer is currently being resolved.
    pub(super) prior_statements: &'program [symbol_resolved_trees::statement::Statement],
    /// All top-level data definitions and the shared member arena -- lets the
    /// receiver walk resolve a NESTED member chain's declared field types
    /// (`self.p.a` -> `p: PairD` -> `a: BoxI`). Empty for scopes built outside
    /// body resolution (field-initializer resolution), where nested method
    /// receivers do not occur.
    pub(super) data_definitions: &'program OrderedRootArena<DataDefinition>,
    pub(super) data_members: &'program Arena<DataMember>,
    pub(super) data_payload_fields: &'program Arena<symbol_resolved_trees::data::DataField>,
    pub(super) type_constraints: &'program Arena<symbol_resolved_trees::types::TypeConstraint>,
}

impl MachineScope<'_> {
    pub(super) fn attached_call_target(
        &self,
        symbols: &SymbolTable,
        owner: SymbolHandle,
        target: &symbol_resolved_trees::name::DiagnosticName,
    ) -> SymbolHandle {
        if !owner.is_valid() || symbols.get(owner).kind != symbols::SymbolKind::Data {
            return SymbolHandle::invalid();
        }
        let mut selected = SymbolHandle::invalid();
        for attachment in self
            .attached_machines
            .iter()
            .filter(|entry| entry.owner == owner)
        {
            let state = super::lookup::child_symbol_by_kinds(
                symbols,
                attachment.machine,
                &[symbols::SymbolKind::State],
                target.as_str(),
            );
            if !state.is_valid() {
                continue;
            }
            // Lookup spellings only test exposure of an already-selected
            // declaration. The exact attachment establishes receiver identity.
            let path = symbols.display_path(attachment.machine, "::");
            let selects = |path: &str| {
                symbols
                    .lookup_top_level_by_name_and_kinds_from_source_matching(
                        path,
                        &[symbols::SymbolKind::Machine],
                        target.source_span(),
                        |candidate| {
                            self.attached_machines
                                .iter()
                                .any(|entry| entry.machine == candidate && entry.owner == owner)
                        },
                    )
                    .unique()
                    == Some(attachment.machine)
            };
            let mut visible = selects(&path);
            if !visible {
                for import in symbols.source_module_import_paths(target.source_span().source_id) {
                    let mut prefix_end = 0;
                    for member in import.split("::") {
                        prefix_end += member.len();
                        if selects(&format!("{}::{path}", &import[..prefix_end])) {
                            visible = true;
                            break;
                        }
                        prefix_end += 2;
                    }
                    if visible {
                        break;
                    }
                }
            }
            if !visible {
                continue;
            }
            if selected.is_valid() {
                return SymbolHandle::invalid();
            }
            selected = state;
        }
        selected
    }

    pub(super) fn field_type_reference(
        &self,
        symbols: &SymbolTable,
        field_symbol: SymbolHandle,
    ) -> Option<&symbol_resolved_trees::types::TypeReference> {
        if symbols.get(self.symbol).kind == symbols::SymbolKind::Variant
            && symbols.get(field_symbol).parent == self.symbol
        {
            // Payload fields may shadow a differently typed common field.
            // Resolve their exact declaration before the inherited field view.
            let payload = self
                .inherited_data_members?
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.symbol == self.symbol => {
                        Some(variant.payload)
                    }
                    _ => None,
                })?;
            return self
                .data_payload_fields
                .span_or_empty(payload)
                .iter()
                .find(|field| field.symbol == field_symbol)
                .map(|field| &field.type_reference);
        }
        if let Some(data_members) = self.inherited_data_members {
            for member in data_members {
                let symbol_resolved_trees::data::DataMember::Field(field) = member else {
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
    /// through the declared field types and return the exact type symbol AFTER the last
    /// segment (`"BoxI"` for `self.p.a` where `a: BoxI`). Each hop's field type
    /// must be a plain `Named` type: shell-wrapped intermediates (`&mut`,
    /// constrained, arrays) return `None` -- conservative, so an unsupported
    /// nested receiver keeps the existing loud unresolved-call error rather
    /// than silently binding 0. `None` unless the root is `self` and every hop
    /// resolves. Used by the nested-receiver symbol stamping (rung 2b of the
    /// receiver-place staircase).
    pub(super) fn nested_self_chain_type(&self, chain: &[&str]) -> Option<SymbolHandle> {
        let (root, hops) = chain.split_first()?;
        if *root != "self" {
            return None;
        }
        let mut current_symbol = self.attached_data_symbol;
        if !current_symbol.is_valid() {
            return None;
        }
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
            current_symbol = match &field.type_reference {
                TypeReference::Named { symbol, .. } if symbol.is_valid() => *symbol,
                _ => return None,
            };
        }
        Some(current_symbol)
    }
}
