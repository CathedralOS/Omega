use arena::{Arena, OrderedRootArena};
use language_core::is_self_receiver;
use symbol_resolved_trees::data::{DataDefinition, DataMember, TypeParameter};
use symbol_resolved_trees::lookup_indexes::ResolvedIndexes;
use symbol_resolved_trees::types::TypeReference;
use symbols::{SymbolHandle, SymbolLookup, SymbolTable};

/// Owner binders retain their declaration identities in attached signatures
/// and bodies. Method-local names shadow them without rebinding the owner's
/// telescope. This supplies a lexical type scope, not an implicit receiver,
/// value argument, or new machine-local generic parameter.
pub(super) fn declaration_type_scope(
    local_parameters: &[TypeParameter],
    owner_parameters: &[TypeParameter],
) -> Vec<TypeParameter> {
    let mut parameters = local_parameters.to_vec();
    parameters.extend(
        owner_parameters
            .iter()
            .filter(|owner| {
                !local_parameters
                    .iter()
                    .any(|local| local.name == owner.name)
            })
            .cloned(),
    );
    parameters
}

/// Source lexical captures of owned map rows. Invocation binders remain in
/// the machine's own telescope; an ordinary referenced row captures nothing.
pub(super) struct ConformanceTypeScopes {
    owned: Vec<(SymbolHandle, Vec<TypeParameter>)>,
}

impl ConformanceTypeScopes {
    pub(super) fn of(
        program: &symbol_resolved_trees::SymbolResolvedTrees,
    ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        let mut owned = Vec::new();
        let mut diagnostics = Vec::new();
        for conformance in program.conformances.iter() {
            for row in conformance
                .rows
                .iter()
                .filter(|row| row.source.owns_realization())
            {
                let provisional = row
                    .provisional_realization_ordinal
                    .and_then(|ordinal| program.machines.iter().nth(ordinal))
                    .map(|machine| machine.symbol);
                let retained = row.realization_machine;
                if row.provisional_realization_ordinal.is_some() && provisional.is_none() {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "owned conformance realization has an invalid construction ordinal",
                    ));
                    continue;
                }
                let selected = if retained.is_valid() {
                    // Completion consumes the finalized row after selection
                    // clears its construction ordinal. Retain that exact
                    // declaration instead of losing its enclosing binders.
                    provisional
                        .is_none_or(|symbol| symbol == retained)
                        .then_some(retained)
                } else {
                    provisional.filter(|symbol| symbol.is_valid())
                };
                if let Some(machine) = selected {
                    if program
                        .machines
                        .iter()
                        .filter(|declaration| declaration.symbol == machine)
                        .count()
                        != 1
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "owned conformance realization has no unique retained machine declaration",
                        ));
                        continue;
                    }
                    if owned.iter().any(|(owner, _)| *owner == machine) {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "owned conformance realization has ambiguous enclosing capture scope",
                        ));
                        continue;
                    }
                    owned.push((
                        machine,
                        program
                            .data_type_parameters(conformance.type_parameters)
                            .to_vec(),
                    ));
                } else {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "owned conformance realization lost its exact enclosing capture scope",
                    ));
                }
            }
        }
        if diagnostics.is_empty() {
            Ok(Self { owned })
        } else {
            Err(diagnostics)
        }
    }

    pub(super) fn parameters(&self, machine: SymbolHandle) -> &[TypeParameter] {
        let mut owners = self.owned.iter().filter(|(owner, _)| *owner == machine);
        let Some((_, parameters)) = owners.next() else {
            return &[];
        };
        if owners.next().is_some() {
            return &[];
        }
        parameters
    }
}

#[derive(Clone, Copy)]
pub(super) struct AttachedMachine {
    pub(super) owner: SymbolHandle,
    pub(super) machine: SymbolHandle,
    pub(super) entry: SymbolHandle,
}

/// The attached-machine roster together with the by-owner index every
/// ambient-method lookup re-derives: the walk used to bucket the roster
/// through a thread-local keyed on the slice address; carrying the index in
/// the roster makes the built-once index a value the caller owns.
pub(super) struct AttachedMachines {
    pub(super) list: Vec<AttachedMachine>,
    by_owner: symbols::SymbolKeyMap<SymbolHandle, Vec<usize>>,
}

impl AttachedMachines {
    /// Roster positions of machines attached to `owner`, in roster order.
    pub(super) fn positions_for_owner(&self, owner: SymbolHandle) -> &[usize] {
        self.by_owner.get(&owner).map(Vec::as_slice).unwrap_or(&[])
    }
}

pub(super) fn attached_machines(
    program: &symbol_resolved_trees::SymbolResolvedTrees,
) -> AttachedMachines {
    let list = attached_machine_list(program);
    let mut by_owner: symbols::SymbolKeyMap<SymbolHandle, Vec<usize>> = Default::default();
    for (index, entry) in list.iter().enumerate() {
        by_owner.entry(entry.owner).or_default().push(index);
    }
    AttachedMachines { list, by_owner }
}

fn attached_machine_list(
    program: &symbol_resolved_trees::SymbolResolvedTrees,
) -> Vec<AttachedMachine> {
    program
        .machines
        .iter()
        .enumerate()
        .filter_map(|(ordinal, machine)| {
            // Inline/default realizations belong to their closed conformance map.
            // They are routed there after ordinary resolution, not competing
            // ambient methods. Referenced authored machines remain ordinary.
            let closed_realization = program.conformances.iter().any(|conformance| {
                let rows = &conformance.rows;
                rows.iter().any(|row| {
                    row.source.owns_realization()
                        && row.provisional_realization_ordinal.map_or_else(
                            || {
                                row.realization_machine.is_valid()
                                    && row.realization_machine == machine.symbol
                            },
                            |selected| selected == ordinal,
                        )
                })
            });
            // A target sibling is checked but never an ambient method of the
            // realized program; its own target's bodies reach it through
            // sibling binding.
            (!closed_realization
                && machine.symbol.is_valid()
                && machine.attached_data_symbol.is_valid()
                && machine.target.is_none())
            .then_some(AttachedMachine {
                owner: machine.attached_data_symbol,
                machine: machine.symbol,
                entry: program
                    .machine_state_handles(machine.states)
                    .first()
                    .map(|state| program.machine_state(*state).symbol)
                    .unwrap_or_else(SymbolHandle::invalid),
            })
        })
        .collect()
}

/// For every trait, the requirements it inherits through `requires` and
/// header parents, built once per assignment batch from declaration
/// identities. A trait's own requirements are its symbol children and need no
/// roster.
pub(super) struct TraitLexicalScopes {
    inherited: Vec<(SymbolHandle, Vec<SymbolHandle>)>,
}

impl TraitLexicalScopes {
    pub(super) fn of(program: &symbol_resolved_trees::SymbolResolvedTrees) -> Self {
        let inherited = program
            .traits
            .iter()
            .map(|trait_definition| {
                let mut visited = vec![trait_definition.symbol];
                let mut requirements = Vec::new();
                for parent in program.trait_requirements(trait_definition.requires) {
                    collect_trait_requirements(
                        program,
                        parent.symbol,
                        &mut visited,
                        &mut requirements,
                    );
                }
                (trait_definition.symbol, requirements)
            })
            .collect();
        Self { inherited }
    }

    /// The bare requirement names visible in `owner`'s lexical scope; an
    /// invalid owner sees none.
    pub(super) fn requirements_of(&self, owner: SymbolHandle) -> LexicalRequirements<'_> {
        LexicalRequirements {
            owner,
            inherited: self
                .inherited
                .iter()
                .find(|(trait_symbol, _)| owner.is_valid() && *trait_symbol == owner)
                .map_or(&[], |(_, requirements)| requirements.as_slice()),
        }
    }
}

fn collect_trait_requirements(
    program: &symbol_resolved_trees::SymbolResolvedTrees,
    trait_symbol: SymbolHandle,
    visited: &mut Vec<SymbolHandle>,
    requirements: &mut Vec<SymbolHandle>,
) {
    if !trait_symbol.is_valid() || visited.contains(&trait_symbol) {
        return;
    }
    visited.push(trait_symbol);
    let Some(trait_definition) = program.trait_definition(trait_symbol) else {
        return;
    };
    for requirement in program.trait_machine_signatures(trait_definition.machines) {
        if requirement.symbol.is_valid() && !requirements.contains(&requirement.symbol) {
            requirements.push(requirement.symbol);
        }
    }
    for parent in program.trait_requirements(trait_definition.requires) {
        collect_trait_requirements(program, parent.symbol, visited, requirements);
    }
}

/// Requirements a receiverless call may name inside one trait's lexical scope:
/// a requirement contract written in the trait, or a default body the trait
/// authored. Default synthesis instantiates that body onto each conforming
/// carrier, but the body stays the trait's text, so its bare names keep the
/// trait's scope rather than the carrier's
/// ([complete rows](../../../../../../../wiki/spec/language/conformances.md#complete-row-identity):
/// "Default bodies call other requirements through this same selected map").
/// Selection names the requirement declaration itself; each instantiation's
/// conformance row is chosen later, after the authored-selection ledger
/// records the requirement as the trait author's own selection.
#[derive(Clone, Copy)]
pub(super) struct LexicalRequirements<'program> {
    /// The trait whose own requirements are visible; invalid outside a trait.
    owner: SymbolHandle,
    /// Requirements `owner` inherits, each once.
    inherited: &'program [SymbolHandle],
}

impl LexicalRequirements<'_> {
    pub(super) fn none() -> Self {
        Self {
            owner: SymbolHandle::invalid(),
            inherited: &[],
        }
    }

    /// The trait's own requirement wins, as its own declaration shadows an
    /// inherited one of the same name. An inherited name must select one
    /// requirement: a short name is not a row identity, so a name several
    /// parents declare stays with ordinary resolution.
    pub(super) fn select(&self, symbols: &SymbolTable, name: &str) -> SymbolHandle {
        if !self.owner.is_valid() {
            return SymbolHandle::invalid();
        }
        let own = super::lookup::child_symbol_by_kinds(
            symbols,
            self.owner,
            &[symbols::SymbolKind::State],
            name,
        );
        if own.is_valid() {
            return own;
        }
        let mut matching = self
            .inherited
            .iter()
            .copied()
            .filter(|requirement| symbols.name(*requirement) == name);
        match (matching.next(), matching.next()) {
            (Some(selected), None) => selected,
            _ => SymbolHandle::invalid(),
        }
    }
}

pub(super) struct MachineScope<'program> {
    pub(super) symbol: SymbolHandle,
    /// Bare requirement names of the trait this scope is lexically inside.
    pub(super) lexical_requirements: LexicalRequirements<'program>,
    pub(super) attached_machines: &'program AttachedMachines,
    /// The program's owned position indexes — local-statement, parameter,
    /// and data-definition positions resolve through these instead of
    /// rescanning (or a thread-local side table).
    pub(super) resolution_indexes: &'program ResolvedIndexes,
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
    pub(super) data_type_parameters: &'program Arena<TypeParameter>,
    pub(super) data_payload_fields: &'program Arena<symbol_resolved_trees::data::DataField>,
    pub(super) type_constraints: &'program Arena<symbol_resolved_trees::types::TypeConstraint>,
}

/// The reference bundle every contract-time `MachineScope` shares. The four
/// scope literals in `symbols/contracts` differ only in the fields this
/// constructor takes; the rest are constants.
pub(super) struct ContractScopeContext<'program> {
    pub(super) attached_machines: &'program AttachedMachines,
    pub(super) resolution_indexes: &'program ResolvedIndexes,
    pub(super) data_definitions: &'program OrderedRootArena<DataDefinition>,
    pub(super) data_members: &'program Arena<DataMember>,
    pub(super) data_type_parameters: &'program Arena<TypeParameter>,
    pub(super) data_payload_fields: &'program Arena<symbol_resolved_trees::data::DataField>,
    pub(super) type_constraints: &'program Arena<symbol_resolved_trees::types::TypeConstraint>,
}

impl<'program> ContractScopeContext<'program> {
    pub(super) fn scope(
        &self,
        symbol: SymbolHandle,
        lexical_requirements: LexicalRequirements<'program>,
        type_parameters: &'program [symbol_resolved_trees::data::TypeParameter],
        attached_data: Option<&'program symbol_resolved_trees::name::DiagnosticName>,
        attached_data_symbol: SymbolHandle,
        inherited_data_members: Option<&'program [DataMember]>,
        owned_data: &'program [symbol_resolved_trees::machine::OwnedData],
    ) -> MachineScope<'program> {
        MachineScope {
            symbol,
            lexical_requirements,
            attached_machines: self.attached_machines,
            resolution_indexes: self.resolution_indexes,
            type_parameters,
            attached_data,
            attached_data_symbol,
            inherited_data_members,
            owned_data,
            prior_statements: &[],
            data_definitions: self.data_definitions,
            data_members: self.data_members,
            data_type_parameters: self.data_type_parameters,
            data_payload_fields: self.data_payload_fields,
            type_constraints: self.type_constraints,
        }
    }
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
        let owner_attachments = self.attached_machines.positions_for_owner(owner);
        // The visibility lookup's candidate filter asks "is this machine
        // attached to `owner`" per roster candidate; answer it from a set
        // built once instead of rescanning the attached list each probe.
        let owner_machines: std::collections::HashSet<SymbolHandle> = owner_attachments
            .iter()
            .map(|index| self.attached_machines.list[*index].machine)
            .collect();
        let mut selected = SymbolHandle::invalid();
        for attachment in owner_attachments
            .iter()
            .map(|index| &self.attached_machines.list[*index])
        {
            // The declaration supplies its callable entry. Searching all state
            // children would expose private jump destinations as methods.
            let state = attachment.entry;
            if !state.is_valid() || symbols.name(state) != target.as_str() {
                continue;
            }
            // Lookup spellings only test exposure of an already-selected
            // declaration. The exact attachment establishes receiver identity.
            let path = symbols.display_path(attachment.machine, "::");
            let selects = |path: &str| {
                // Same-path same-owner candidates are a result-overload
                // family: the lookup cannot return one Unique member, so
                // membership in the ambiguity set marks this member visible.
                // The earliest match stays the provisional binding; the typed
                // result-overload pass rebinds the exact member from the
                // destination's dispatch set.
                match symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
                    path,
                    &[symbols::SymbolKind::Machine],
                    target.source_span(),
                    |candidate| owner_machines.contains(&candidate),
                ) {
                    SymbolLookup::Unique(candidate) => candidate == attachment.machine,
                    SymbolLookup::Ambiguous { first, second } => {
                        attachment.machine == first || attachment.machine == second
                    }
                    SymbolLookup::NotFound => false,
                }
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
                // A same-named attached family keeps its first visible member
                // as the provisional binding; declaration validation owns the
                // duplicate rejection when the pair is not a legal overload.
                continue;
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
        if !is_self_receiver(root) {
            return None;
        }
        let mut current_symbol = self.attached_data_symbol;
        if !current_symbol.is_valid() {
            return None;
        }
        for hop in hops {
            let definition = &self.data_definitions[self
                .resolution_indexes
                .data_definition_position(self.data_definitions, current_symbol)?
                as usize];
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
