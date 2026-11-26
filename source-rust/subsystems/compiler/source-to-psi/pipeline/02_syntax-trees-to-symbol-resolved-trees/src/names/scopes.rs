use super::lookup::child_symbol_by_kinds;
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::data::{DataDefinition, DataMember, TypeParameter};
use crate::source_resolved::lookup_indexes::ResolvedIndexes;
use crate::source_resolved::type_aliases::TypeAliasDefinition;
use crate::source_resolved::types::TypeReference;
use crate::source_symbols::{SymbolLookup, SymbolTable};
use arena::{Arena, OrderedRootArena};
use language_core::is_self_receiver;
use symbols::SymbolHandle;

/*
    Owner binders retain their declaration identities in attached signatures
    and bodies. Method-local names shadow them without rebinding the owner's
    telescope. This supplies a lexical type scope, not an implicit receiver,
    value argument, or new machine-local generic parameter.
*/
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

/*
    Machine evidence has its own durable binder roster. Reuse these exact
    handles wherever source signatures need the complete lexical type scope.
*/
pub(super) fn evidence_type_parameters(
    bounds: &[crate::source_resolved::machine::GenericConformanceBound],
) -> impl Iterator<Item = TypeParameter> + '_ {
    bounds.iter().map(|bound| TypeParameter {
        symbol: bound.binder,
        name: bound.binder_name.clone(),
        kind: crate::source_resolved::data::TypeParameterKind::Implementation,
        bounds: Default::default(),
    })
}

/*
    Resolve the construction ordinal and finalized declaration through the same
    exact row owner. Inline member machines are root declarations; their symbol
    parent is not the conformance's lexical ownership roster.
*/
pub(crate) fn owned_conformance_realization(
    program: &SymbolResolvedTrees,
    row: &crate::source_resolved::trait_definition::ConformanceRow,
) -> Result<SymbolHandle, &'static str> {
    if !row.source.owns_realization() {
        return Err("referenced conformance row has no owned realization");
    }
    let provisional = row
        .provisional_realization_ordinal
        .filter(|ordinal| *ordinal < program.machines.len())
        .map(|ordinal| &program.machines[ordinal])
        .map(|machine| machine.symbol);
    if row.provisional_realization_ordinal.is_some() && provisional.is_none() {
        return Err("owned conformance realization has an invalid construction ordinal");
    }
    let retained = row.realization_machine;
    let selected = if retained.is_valid() {
        provisional
            .is_none_or(|symbol| symbol == retained)
            .then_some(retained)
    } else {
        provisional.filter(|symbol| symbol.is_valid())
    }
    .ok_or("owned conformance realization lost its exact enclosing capture scope")?;
    if program.machines.unique_by_symbol(selected).is_none() {
        return Err("owned conformance realization has no unique retained machine declaration");
    }
    Ok(selected)
}

/*
    Publish authored containment when declaration symbols first exist. This is
    independent of requirement-overload normalization: a provisional row still
    has to agree with any retained realization. Every later scope consumer reads
    this same inverse relationship instead of reconstructing it from all maps.
*/
pub(super) fn record_declaration_owners(
    program: &SymbolResolvedTrees,
    symbols: &mut SymbolTable,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut ownership = Vec::new();
    let mut diagnostics = Vec::new();
    for conformance in program.conformances.iter() {
        for row in conformance
            .rows
            .iter()
            .filter(|row| row.source.owns_realization())
        {
            let machine = match owned_conformance_realization(program, row) {
                Ok(machine) => machine,
                Err(reason) => {
                    diagnostics.push(diagnostics::Diagnostic::error(reason));
                    continue;
                }
            };
            if ownership
                .iter()
                .any(|(declaration, _)| *declaration == machine)
            {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "owned conformance realization has ambiguous enclosing capture scope",
                ));
            } else {
                ownership.push((machine, conformance.symbol));
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for (declaration, owner) in ownership {
        symbols
            .record_declaration_owner(declaration, owner)
            .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    }
    Ok(())
}

/*
    Exact namespace owning this machine, if it is a retained map member.
    A second row cannot lend its capture or normalization domain to the first.
*/
pub(crate) fn owned_conformance_namespace(
    program: &SymbolResolvedTrees,
    machine: SymbolHandle,
) -> Result<SymbolHandle, &'static str> {
    let owner = program.symbols.get(machine).declaration_owner;
    if !owner.is_valid() {
        return Ok(owner);
    }
    for conformance in program
        .conformances
        .iter()
        .filter(|map| map.symbol == owner)
    {
        for row in conformance
            .rows
            .iter()
            .filter(|row| row.source.owns_realization())
        {
            if owned_conformance_realization(program, row)? == machine {
                return Ok(owner);
            }
        }
    }
    Ok(SymbolHandle::invalid())
}

/*
    Static declaration context of owned map rows and nested machines. These
    exact binders are visible during formation without becoming runtime captures
    or fresh invocation parameters. Referenced map rows capture nothing.
*/
pub(super) struct DeclarationTypeScopes {
    owned: Vec<(SymbolHandle, Vec<TypeParameter>)>,
}

impl DeclarationTypeScopes {
    pub(super) fn of(program: &SymbolResolvedTrees) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        let mut owned = Vec::new();
        let mut diagnostics = Vec::new();
        for conformance in program.conformances.iter() {
            for row in conformance
                .rows
                .iter()
                .filter(|row| row.source.owns_realization())
            {
                let machine = match owned_conformance_realization(program, row) {
                    Ok(machine) => machine,
                    Err(reason) => {
                        diagnostics.push(diagnostics::Diagnostic::error(reason));
                        continue;
                    }
                };
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
            }
        }
        for machine in program
            .machines
            .iter()
            .filter(|machine| machine.lexical_parent.is_valid())
        {
            let parent = program.machines.get(machine.lexical_parent);
            let mut parameters = program.machine_type_parameters(parent).to_vec();
            parameters.extend(evidence_type_parameters(&parent.conformance_bounds));
            if let Some((_, inherited)) = owned.iter().find(|(owner, _)| *owner == parent.symbol) {
                parameters = declaration_type_scope(&parameters, inherited);
            }
            if let Some(data) = program
                .data_definitions
                .iter()
                .find(|data| data.symbol == parent.attached_data_symbol)
            {
                parameters = declaration_type_scope(
                    &parameters,
                    program.data_type_parameters(data.type_parameters),
                );
            }
            if let Some((_, inherited)) =
                owned.iter_mut().find(|(owner, _)| *owner == machine.symbol)
            {
                *inherited = declaration_type_scope(inherited, &parameters);
            } else {
                owned.push((machine.symbol, parameters));
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

/*
    The attached-machine roster together with the by-owner index every
    ambient-method lookup re-derives: the walk used to bucket the roster
    through a thread-local keyed on the slice address; carrying the index in
    the roster makes the built-once index a value the caller owns.
*/
pub(super) struct AttachedMachines {
    pub(super) list: Vec<AttachedMachine>,
    by_owner: arena::HandleKeyMap<SymbolHandle, Vec<usize>>,
}

impl AttachedMachines {
    /* Roster positions of machines attached to `owner`, in roster order. */
    pub(super) fn positions_for_owner(&self, owner: SymbolHandle) -> &[usize] {
        self.by_owner.get(&owner).map(Vec::as_slice).unwrap_or(&[])
    }
}

pub(super) fn attached_machines(program: &SymbolResolvedTrees) -> AttachedMachines {
    let list = attached_machine_list(program);
    let mut by_owner: arena::HandleKeyMap<SymbolHandle, Vec<usize>> = Default::default();
    for (index, entry) in list.iter().enumerate() {
        by_owner.entry(entry.owner).or_default().push(index);
    }
    AttachedMachines { list, by_owner }
}

fn attached_machine_list(program: &SymbolResolvedTrees) -> Vec<AttachedMachine> {
    program
        .machines
        .iter()
        .enumerate()
        .filter_map(|(ordinal, machine)| {
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

/*
    The role a generic binder gives the static argument written for it
    (`wiki/spec/language/modules.md#builtin-type-names`): a type binder
    reads a type, `const N: u64` or a runtime value binder reads a value,
    and the remaining binders select a declaration.
*/
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BinderRole {
    Type,
    Value,
    Declaration,
}

impl BinderRole {
    pub(super) fn of(kind: &crate::source_resolved::data::TypeParameterKind) -> Self {
        use crate::source_resolved::data::TypeParameterKind;
        match kind {
            TypeParameterKind::Type => Self::Type,
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => Self::Value,
            TypeParameterKind::Implementation
            | TypeParameterKind::Machine { .. }
            | TypeParameterKind::Proof { .. }
            | TypeParameterKind::Proposition { .. } => Self::Declaration,
        }
    }
}

/*
    Binder roles of machines and indexed domain families in declaration order.
    A call or domain application reads its named arguments in the role each
    slot declares. A generic domain carrier is implicit at use sites.
*/
pub(super) struct BinderRoles {
    declarations: Vec<(SymbolHandle, u32, u32)>,
    roles: Vec<BinderRole>,
}

impl BinderRoles {
    pub(super) fn of(program: &SymbolResolvedTrees, symbols: &SymbolTable) -> Self {
        let mut declarations =
            Vec::with_capacity(program.machines.len() + program.domain_definitions.len());
        let mut roles = Vec::new();
        for machine in program.machines.iter() {
            let binders = program.machine_type_parameters(machine);
            if !machine.symbol.is_valid() || binders.is_empty() {
                continue;
            }
            let start = roles.len() as u32;
            // The installer names its registrar and private callback slot before
            // the first declared type parameter. Both select declarations.
            if machine.name.as_str() == "X86StaticInterruptScope::run"
                && symbols.sealed_declaration(machine.symbol, machine.name.as_str())
                    == Some(
                        language_core::sealed_intrinsics::SealedIntrinsic::StaticInterrupt(
                            language_core::static_interrupts::StaticInterruptOperation::Install,
                        ),
                    )
            {
                roles.extend([BinderRole::Declaration; 2]);
            }
            roles.extend(binders.iter().map(|binder| BinderRole::of(&binder.kind)));
            declarations.push((machine.symbol, start, roles.len() as u32));
        }
        for domain in &program.domain_definitions {
            let binders = program.data_type_parameters(domain.type_parameters);
            let indices = domain.index_binders(binders);
            if domain.symbol.is_valid() && !indices.is_empty() {
                let start = roles.len() as u32;
                roles.extend(indices.iter().map(|binder| BinderRole::of(&binder.kind)));
                declarations.push((domain.symbol, start, roles.len() as u32));
            }
        }
        declarations.sort_unstable_by_key(|(symbol, _, _)| symbol.arena_index());
        Self {
            declarations,
            roles,
        }
    }

    /*
        The role of the binder at `position` of the machine a call selected,
        whether the call names the machine or one of its states. A target
        with no known telescope, or a position past its binders, selects a
        declaration and never a role.
    */
    pub(super) fn role(
        &self,
        symbols: &SymbolTable,
        target: SymbolHandle,
        position: usize,
    ) -> BinderRole {
        if !target.is_valid() {
            return BinderRole::Declaration;
        }
        let selected = symbols.get(target);
        let machine = match selected.kind {
            symbols::SymbolKind::State => selected.parent,
            symbols::SymbolKind::Machine => target,
            _ => return BinderRole::Declaration,
        };
        self.for_declaration(machine)
            .get(position)
            .copied()
            .unwrap_or(BinderRole::Declaration)
    }

    pub(super) fn for_declaration(&self, declaration: SymbolHandle) -> &[BinderRole] {
        let Ok(row) = self
            .declarations
            .binary_search_by_key(&declaration.arena_index(), |(symbol, _, _)| {
                symbol.arena_index()
            })
        else {
            return &[];
        };
        let (symbol, start, end) = self.declarations[row];
        if symbol != declaration {
            return &[];
        }
        &self.roles[start as usize..end as usize]
    }
}

/*
    For every trait, the requirements it inherits through `requires` and
    header parents, built once per assignment batch from declaration
    identities. A trait's own requirements are its symbol children and need no
    roster.
*/
pub(super) struct TraitLexicalScopes {
    machine_traits: arena::HandleKeyMap<SymbolHandle, SymbolHandle>,
    inherited: Vec<(SymbolHandle, Vec<SymbolHandle>)>,
}

impl TraitLexicalScopes {
    pub(super) fn of(program: &SymbolResolvedTrees) -> Self {
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
        let trait_by_implementation: std::collections::HashMap<_, _> = program
            .conformances
            .iter()
            .map(|implementation| (implementation.symbol, implementation.trait_symbol))
            .collect();
        let mut machine_traits = arena::HandleKeyMap::default();
        for machine in program.machines.iter() {
            let root = machine.lexical_root(&program.machines);
            let declaring = root.trait_default_origin.declaring_trait;
            let owner = if declaring.is_valid() {
                Some(declaring)
            } else {
                trait_by_implementation
                    .get(&program.symbols.get(root.symbol).declaration_owner)
                    .copied()
            };
            if let Some(owner) = owner.filter(|owner| owner.is_valid()) {
                machine_traits.insert(machine.symbol, owner);
            }
        }
        Self {
            inherited,
            machine_traits,
        }
    }

    pub(super) fn machine_requirements(&self, machine: SymbolHandle) -> LexicalRequirements<'_> {
        self.requirements_of(
            self.machine_traits
                .get(&machine)
                .copied()
                .unwrap_or_default(),
        )
    }

    /*
        The bare requirement names visible in `owner`'s lexical scope; an
        invalid owner sees none.
    */
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
    program: &SymbolResolvedTrees,
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

/*
    Requirements a receiverless call may name inside one trait's lexical scope:
    a requirement contract written in the trait, or a default body the trait
    authored. Default synthesis instantiates that body onto each conforming
    carrier, but the body stays the trait's text, so its bare names keep the
    trait's scope rather than the carrier's
    ([complete rows](../../../../../../../wiki/spec/language/conformances.md#complete-row-identity):
    "Default bodies call other requirements through this same selected map").
    Selection names the requirement declaration itself; each instantiation's
    conformance row is chosen later, after the authored-selection ledger
    records the requirement as the trait author's own selection.
*/
#[derive(Clone, Copy)]
pub(super) struct LexicalRequirements<'program> {
    /* The trait whose own requirements are visible; invalid outside a trait. */
    owner: SymbolHandle,
    /* Requirements `owner` inherits, each once. */
    inherited: &'program [SymbolHandle],
}

impl LexicalRequirements<'_> {
    pub(super) fn none() -> Self {
        Self {
            owner: SymbolHandle::invalid(),
            inherited: &[],
        }
    }

    /*
        The trait's own requirement wins, as its own declaration shadows an
        inherited one of the same name. An inherited name must select one
        requirement: a short name is not a row identity, so a name several
        parents declare stays with ordinary resolution.
    */
    pub(super) fn select(&self, symbols: &SymbolTable, name: &str) -> SymbolHandle {
        if !self.owner.is_valid() {
            return SymbolHandle::invalid();
        }
        let own = child_symbol_by_kinds(symbols, self.owner, &[symbols::SymbolKind::State], name);
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
    /* Bare requirement names of the trait this scope is lexically inside. */
    pub(super) lexical_requirements: LexicalRequirements<'program>,
    pub(super) attached_machines: &'program AttachedMachines,
    /* Each callee's binder roles, which decide how a static argument reads. */
    pub(super) binder_roles: &'program BinderRoles,
    pub(super) resolution_indexes: &'program ResolvedIndexes,
    pub(super) type_parameters: &'program [crate::source_resolved::data::TypeParameter],
    pub(super) attached_data_symbol: SymbolHandle,
    pub(super) inherited_data_members: Option<&'program [crate::source_resolved::data::DataMember]>,
    pub(super) owned_data: &'program [crate::source_resolved::machine::OwnedData],
    /*
        Only the already-stamped prefix of the current state's statements.
        Local receiver types cannot come from a later declaration or from the
        binding whose initializer is currently being resolved.
    */
    pub(super) prior_statements: &'program [crate::source_resolved::statement::Statement],
    /// The declared result's nominal data owner while resolving its
    /// contract. This selects fields, never a runtime value or permission.
    pub(super) contract_result_data: SymbolHandle,
    /*
        All top-level data definitions and the shared member arena -- lets the
        receiver walk resolve a NESTED member chain's declared field types
        (`self.p.a` -> `p: PairD` -> `a: BoxI`). Empty for scopes built outside
        body resolution (field-initializer resolution), where nested method
        receivers do not occur.
    */
    pub(super) data_definitions: &'program OrderedRootArena<DataDefinition>,
    /* Alias headers supply argument roles, never runtime receiver members. */
    pub(super) type_aliases: &'program OrderedRootArena<TypeAliasDefinition>,
    pub(super) conformances:
        &'program OrderedRootArena<crate::source_resolved::trait_definition::Conformance>,
    pub(super) traits:
        &'program OrderedRootArena<crate::source_resolved::trait_definition::TraitDefinition>,
    pub(super) operand_owners: &'program Arena<crate::source_resolved::types::OperandLifetimeOwner>,
    pub(super) data_members: &'program Arena<DataMember>,
    pub(super) data_type_parameters: &'program Arena<TypeParameter>,
    pub(super) data_payload_fields: &'program Arena<crate::source_resolved::data::DataField>,
    pub(super) type_constraints: &'program Arena<crate::source_resolved::types::TypeConstraint>,
}

/*
    The reference bundle every contract-time `MachineScope` shares. The four
    scope literals in `symbols/contracts` differ only in the fields this
    constructor takes; the rest are constants.
*/
pub(super) struct ContractScopeContext<'program> {
    pub(super) attached_machines: &'program AttachedMachines,
    pub(super) binder_roles: &'program BinderRoles,
    pub(super) resolution_indexes: &'program ResolvedIndexes,
    pub(super) data_definitions: &'program OrderedRootArena<DataDefinition>,
    pub(super) type_aliases: &'program OrderedRootArena<TypeAliasDefinition>,
    pub(super) conformances:
        &'program OrderedRootArena<crate::source_resolved::trait_definition::Conformance>,
    pub(super) traits:
        &'program OrderedRootArena<crate::source_resolved::trait_definition::TraitDefinition>,
    pub(super) operand_owners: &'program Arena<crate::source_resolved::types::OperandLifetimeOwner>,
    pub(super) data_members: &'program Arena<DataMember>,
    pub(super) data_type_parameters: &'program Arena<TypeParameter>,
    pub(super) data_payload_fields: &'program Arena<crate::source_resolved::data::DataField>,
    pub(super) type_constraints: &'program Arena<crate::source_resolved::types::TypeConstraint>,
}

impl<'program> ContractScopeContext<'program> {
    pub(super) fn scope(
        &self,
        symbol: SymbolHandle,
        lexical_requirements: LexicalRequirements<'program>,
        type_parameters: &'program [crate::source_resolved::data::TypeParameter],
        attached_data_symbol: SymbolHandle,
        inherited_data_members: Option<&'program [DataMember]>,
        owned_data: &'program [crate::source_resolved::machine::OwnedData],
    ) -> MachineScope<'program> {
        MachineScope {
            symbol,
            lexical_requirements,
            attached_machines: self.attached_machines,
            binder_roles: self.binder_roles,
            resolution_indexes: self.resolution_indexes,
            type_parameters,
            attached_data_symbol,
            inherited_data_members,
            owned_data,
            prior_statements: &[],
            contract_result_data: symbols::SymbolHandle::invalid(),
            data_definitions: self.data_definitions,
            type_aliases: self.type_aliases,
            conformances: self.conformances,
            traits: self.traits,
            operand_owners: self.operand_owners,
            data_members: self.data_members,
            data_type_parameters: self.data_type_parameters,
            data_payload_fields: self.data_payload_fields,
            type_constraints: self.type_constraints,
        }
    }
}

impl<'program> MachineScope<'program> {
    pub(super) fn declaration_scope(
        &self,
        declaration: SymbolHandle,
        parameters: &'program [TypeParameter],
    ) -> Self {
        Self {
            symbol: declaration,
            lexical_requirements: LexicalRequirements::none(),
            type_parameters: parameters,
            attached_data_symbol: SymbolHandle::invalid(),
            inherited_data_members: None,
            owned_data: &[],
            prior_statements: &[],
            contract_result_data: symbols::SymbolHandle::invalid(),
            ..*self
        }
    }

    /*
        A copied type operand's existing input receipt owns its names. An
        omitted/ambiguous header or malformed span supplies no lexical context.
    */
    pub(super) fn supplied_scope(
        &self,
        owner: crate::source_resolved::types::OperandLifetimeOwnerHandle,
    ) -> Option<Self> {
        let declaration = self.operand_owners.get(owner).declaration;
        if !declaration.is_valid() {
            return None;
        }
        let mut declarations = self
            .conformances
            .iter()
            .filter(|header| header.symbol == declaration)
            .map(|header| header.type_parameters)
            .chain(
                self.traits
                    .iter()
                    .filter(|header| header.symbol == declaration)
                    .map(|header| header.type_parameters),
            );
        let type_parameters = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        let parameters = self.data_type_parameters.span_or_empty(type_parameters);
        if parameters.len() != type_parameters.count() as usize {
            return None;
        }
        Some(self.declaration_scope(declaration, parameters))
    }
}

impl MachineScope<'_> {
    pub(super) fn attached_call_target(
        &self,
        symbols: &SymbolTable,
        owner: SymbolHandle,
        target: &crate::source_resolved::name::DiagnosticName,
    ) -> SymbolHandle {
        if !owner.is_valid() || symbols.get(owner).kind != symbols::SymbolKind::Data {
            return SymbolHandle::invalid();
        }
        let owner_attachments = self.attached_machines.positions_for_owner(owner);
        let owner_machines: std::collections::HashSet<SymbolHandle> = owner_attachments
            .iter()
            .map(|index| self.attached_machines.list[*index].machine)
            .collect();
        let mut selected = SymbolHandle::invalid();
        for attachment in owner_attachments
            .iter()
            .map(|index| &self.attached_machines.list[*index])
        {
            let state = attachment.entry;
            if !state.is_valid() || symbols.name(state) != target.as_str() {
                continue;
            }
            let path = symbols.display_path(attachment.machine, "::");
            let selects = |path: &str| match symbols
                .lookup_top_level_by_name_and_kinds_from_source_matching(
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
            };
            let mut visible = selects(&path);
            if !visible {
                for import in symbols.prelude_module_paths(target.source_span().source_id) {
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
    ) -> Option<&crate::source_resolved::types::TypeReference> {
        if symbols.get(self.symbol).kind == symbols::SymbolKind::Variant
            && symbols.get(field_symbol).parent == self.symbol
        {
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
                let crate::source_resolved::data::DataMember::Field(field) = member else {
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

    /*
        Walk a `self`-rooted member chain of SPELLED names (`["self", "p", "a"]`)
        through the declared field types and return the exact type symbol AFTER the last
        segment (`"BoxI"` for `self.p.a` where `a: BoxI`). Each hop's field type
        must be a plain `Named` type: shell-wrapped intermediates (`@mutable`,
        constrained, arrays) return `None` -- conservative, so an unsupported
        nested receiver keeps the existing loud unresolved-call error rather
        than silently binding 0. `None` unless the root is `self` and every hop
        resolves. Used by the nested-receiver symbol stamping (rung 2b of the
        receiver-place staircase).
    */
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
