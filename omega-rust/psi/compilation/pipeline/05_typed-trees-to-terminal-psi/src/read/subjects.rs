/*
Which declarations a compilation is responsible for, beyond the machines its
entries call: what the Build's selections name, and every declaration of the
subject. `wiki/spec/build/validation_subjects.md`: "A passed check
therefore means exactly: every declaration of the subject packages is valid,
and every declaration they depend on is valid", and the closure "starts from
every declaration of every subject, including unused ones".

So every subject declaration is a root here, used or not. A generic machine
is set aside for its own check form (`Elaboration::generic_templates`). Any
other machine becomes a `MachineId` and is read, lowered and judged like a
called one; its callees
follow as they do from an entry, whichever package declares them. A data
declaration is resolved as a record. A plain trait has each requirement's
contract read, and a plain implementation has each row set aside as the
statement that its machine refines the requirement
(`Elaboration::conformance_witnesses`). A transparent mathematical predicate
is formed under its own inputs, independently of any application. Every other
kind of declaration is refused as `Unsupported`, because leaving an unjudged
declaration out would accept it without a row or a refusal.

Whether a declaration belongs to the subject is decided from the source file
its symbol was declared in, never from a path or a name:

- A toolchain file, or a file of another admitted package, is a dependency.
  Its declarations are judged only when a subject declaration reaches them.
- A package's Build sources and Product sources are two programs
  (`wiki/spec/build/scoped_execution.md`, "Two checked contexts"). The
  compilation states which one it judges (`Compilation::purpose`); a file
  read under the other dependency scope is a dependency of this one. So a
  Product compilation leaves the Build program to the Build activation,
  which checks it before it runs, and that activation's compilation claims
  the subject's Build declarations and none of its Product declarations.
- A user file is a subject only when its package identity is requested.
  An explicit `None` subject selects standalone user source: every user file,
  whatever identity its completed Build derived for it. An empty request
  selects no full-package or standalone subject.
- A declaration with no source file cannot be placed, so it is refused.
*/

use semantic_vocabulary::{MachineId, PackageKeyIdentity};
use source::DependencyScope;
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};

use super::Program;
use crate::{Unserved, refuse};

/// Whose declaration a symbol is, for one compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Subject,
    /// Judged only when a subject declaration or an entry reaches it.
    Dependency,
}

impl Program<'_> {
    fn owner(
        &self,
        entry: MachineId,
        symbol: SymbolHandle,
        subjects: &[Option<PackageKeyIdentity>],
        purpose: DependencyScope,
    ) -> Checked<Owner> {
        let subject = typed_trees::source_subjects::contains(self.typed, symbol, subjects, purpose)
            .map_err(|reason| refuse(entry, Construct::Machine, Unserved::Declaration(reason)))?;
        Ok(if subject {
            Owner::Subject
        } else {
            Owner::Dependency
        })
    }

    /// Holds a Build activation's entries to the Build source context: a
    /// toolchain file or a file read under the Build dependency scope. Name
    /// resolution decides what Build code may name; this is the one fact it
    /// cannot see, that the request roots the Build program at a declaration
    /// of that program. A Product declaration given as a Build entry would
    /// otherwise be judged and run with the Build's authority.
    pub fn build_entry_context(
        &self,
        first: MachineId,
        purpose: DependencyScope,
        entries: &[SymbolHandle],
    ) -> Checked<()> {
        if purpose != DependencyScope::Build {
            return Ok(());
        }
        let symbols = &self.typed.symbols;
        for entry in entries {
            let in_context = entry
                .is_valid()
                .then(|| symbols.symbol_provenance_source_span(*entry))
                .flatten()
                .and_then(|span| symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin.is_toolchain() || file.dependency_scope == DependencyScope::Build
                });
            if !in_context {
                return Err(refuse(
                    first,
                    Construct::Machine,
                    Unserved::Declaration("Build entry outside the Build source context"),
                ));
            }
        }
        Ok(())
    }

    /// Makes every declaration the Build's selections name a root, whichever
    /// package declares it. A selected machine is read like an entry and a
    /// selected data type is resolved as a record.
    ///
    /// A native binding (`Build::bind<Host::tick>(builder, target, locator)`)
    /// claims the requirement it names and the boundary trait that declares
    /// it. The binding says where the requirement is found; neither has a
    /// body to judge. The requirement is read as the boundary it is, by the
    /// one reader every call of it takes, so its signature and declared
    /// ceilings are held to the same rules whether or not this compilation
    /// calls it. The trait is read as the service it declares.
    ///
    /// A claim is not a call, so no caller's visibility is checked here.
    /// Whether the binding's author may name the requirement was decided
    /// when the Build's binding was admitted, and a compilation claims only
    /// the bindings its own package's Build made: a dependency's bindings
    /// are reached through the providers a target chooses, whose calls check
    /// visibility like any other. A declaration check has no entry at all to
    /// call from.
    ///
    /// A selection of anything else (a trait that is no boundary, an
    /// operator family) is refused here. A provider is judged against what
    /// it provides as a `Supply` (`supplied_slots`), never as a bare
    /// selected declaration.
    pub fn selected_declarations(
        &mut self,
        entry: MachineId,
        selected: &[SymbolHandle],
    ) -> Checked<()> {
        let typed = self.typed;
        for symbol in selected {
            if let Some(owner) = typed
                .trait_definition_by_symbol(*symbol)
                .filter(|owner| owner.is_boundary)
            {
                self.boundary_trait_declaration(entry, owner)?;
                self.declared_service(entry, owner)?;
                continue;
            }
            if self.trait_declaration(entry, *symbol)?
                || self.conformance_declaration(entry, *symbol)?
            {
                self.admitted_declarations.include(*symbol);
                continue;
            }
            if self.select_native_provider(entry, *symbol)? {
                continue;
            }
            if typed.machine_by_symbol(*symbol).is_some() {
                let machine = self.machine(*symbol)?;
                self.fixed_inputs.push(machine);
            } else if let Some(data) = self.data_declaration(*symbol) {
                let names_only = names_only(data);
                self.admitted_declarations.include(*symbol);
                if !names_only {
                    self.record(entry, *symbol)?;
                }
            } else if typed
                .trait_machine_signature(*symbol)
                .is_some_and(|(owner, requirement)| {
                    owner.is_boundary && requirement.symbol == *symbol
                })
            {
                self.boundary_declaration(entry, *symbol)?;
            } else {
                return Err(refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Declaration(
                        "Build selection that is not a machine, a data type or a boundary",
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Claims each chosen provider for the slot it supplies. For every
    /// requirement the slot names, the provider must give exactly one
    /// machine: that machine becomes a root with its declared input
    /// contract, and the statement that it refines the requirement is set
    /// aside as a conformance witness, exactly as a subject implementation's
    /// rows are. A supply the route cannot read this way is refused by name,
    /// so a provider is never accepted without having been judged against
    /// what it provides.
    pub fn supplied_slots(&mut self, entry: MachineId, supplies: &[crate::Supply]) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| {
            Err(refuse(
                entry,
                Construct::Machine,
                Unserved::Declaration(what),
            ))
        };
        for supply in supplies {
            // The slot is a nominal trait, supplied whole, or one
            // exact requirement. Ordinary traits grant no boundary capability.
            let (owner, requirements) = match typed.trait_definition_by_symbol(supply.slot) {
                Some(owner) => {
                    // No applied parent environment is carried by this bare
                    // whole-trait nomination. Refuse rather than omit inherited
                    // requirements or pretend the application was empty.
                    if !owner.lifetime_parameters.is_empty()
                        || !owner.type_parameters.is_empty()
                        || !owner.conformance_bounds.is_empty()
                        || !owner.requires.is_empty()
                        || owner.refines.is_some()
                        || !owner.refinement_clauses.is_empty()
                    {
                        return unserved(
                            "whole-trait supply with parameters, parents or refinements",
                        );
                    }
                    (
                        owner,
                        typed
                            .trait_machine_signatures(owner)
                            .iter()
                            .map(|requirement| requirement.symbol)
                            .collect::<Vec<_>>(),
                    )
                }
                None => match typed.trait_machine_signature(supply.slot) {
                    Some((owner, requirement)) if requirement.symbol == supply.slot => {
                        (owner, vec![supply.slot])
                    }
                    _ => {
                        return unserved(
                            "provider supply of a slot that is not a trait or one of its requirements",
                        );
                    }
                },
            };
            for requirement in requirements {
                let machine = match supply.realization(typed, requirement) {
                    Ok(machine) => machine,
                    Err(reason) => return unserved(reason),
                };
                let witness = crate::ConformanceWitness {
                    declaring_trait: owner.symbol,
                    requirement,
                    implementation: machine.symbol,
                    value_arguments: Vec::new(),
                };
                let root = self.provider(machine.symbol, owner.symbol, requirement)?;
                if !self.fixed_inputs.contains(&root) {
                    self.fixed_inputs.push(root);
                }
                // Ordinary policies have checked bodies and refinement witnesses,
                // but supply no external service. Only a boundary requirement
                // needs a declaration and a candidate for provider planning.
                if owner.is_boundary {
                    self.boundary_declaration(root, requirement)?;
                    let supplied = crate::SuppliedRequirement {
                        provider: supply.provider,
                        requirement,
                        machine: root,
                    };
                    if !self.supplied.contains(&supplied) {
                        self.supplied.push(supplied);
                    }
                }
                // A machine that states `satisfies` already owes this row
                // (`satisfied_requirements`), and two supplies may name one
                // row; it is one obligation.
                if !self.conformance_witnesses.contains(&witness) {
                    self.conformance_witnesses.push(witness);
                }
            }
        }
        Ok(())
    }

    /// Makes every subject declaration a root of the elaboration. `entry`
    /// only attributes a refusal: a declaration that is not a machine has no
    /// `MachineId` of its own.
    pub fn subject_declarations(
        &mut self,
        entry: MachineId,
        subjects: &[Option<PackageKeyIdentity>],
        purpose: DependencyScope,
    ) -> Checked<()> {
        // A concrete evaluation asks for its existing entry and call closure,
        // not a full check of the source files retained for name resolution.
        if subjects.is_empty() {
            return Ok(());
        }
        let typed = self.typed;
        for machine in typed.machines() {
            if self.owner(entry, machine.symbol, subjects, purpose)? != Owner::Subject {
                continue;
            }
            if self.select_native_provider(entry, machine.symbol)? {
                continue;
            }
            // A generic machine is checked once as written, as its own check
            // form, not as a closed body (`Elaboration::generic_templates`).
            // One whose only parameters are implementations is generic too.
            if !self.is_generic_template(machine) {
                let id = self.subject_machine(machine.symbol)?;
                self.subject_machines.push(id);
            } else {
                self.admitted_declarations.include(machine.symbol);
                self.generic_templates.push(machine.symbol);
            }
        }
        for data in typed.data_definitions() {
            if self.owner(entry, data.symbol, subjects, purpose)? != Owner::Subject
                || names_only(data)
            {
                continue;
            }
            // A declaration with type or const parameters has no record to
            // resolve until a type applies it; it is read as written.
            if data.type_parameters.is_empty() {
                self.record(entry, data.symbol)?;
            } else {
                self.subject_data_template(entry, data)?;
            }
            self.admitted_declarations.include(data.symbol);
        }
        for domain in typed.domain_definitions() {
            if self.owner(entry, domain.symbol, subjects, purpose)? != Owner::Subject {
                continue;
            }
            if !judged_where_used(typed, domain) {
                return Err(refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Declaration("domain with a cleanup machine, operators or parameters"),
                ));
            }
            // Its predicates are stated wherever a type carries the domain;
            // a declaration nothing uses must still be one the route reads.
            self.admitted_declarations.include(domain.symbol);
            self.declared_predicates(entry, domain)?;
        }
        for declaration in typed.traits() {
            if self.owner(entry, declaration.symbol, subjects, purpose)? == Owner::Subject {
                if declaration.is_boundary {
                    self.boundary_trait_declaration(entry, declaration)?;
                    self.declared_service(entry, declaration)?;
                } else {
                    self.trait_declaration(entry, declaration.symbol)?;
                }
                self.admitted_declarations.include(declaration.symbol);
            }
        }
        for declaration in typed.conformances() {
            if self.owner(entry, declaration.symbol, subjects, purpose)? == Owner::Subject {
                self.conformance_declaration(entry, declaration.symbol)?;
                self.admitted_declarations.include(declaration.symbol);
            }
        }
        for constant in typed.const_declarations() {
            if self.owner(entry, constant.symbol, subjects, purpose)? == Owner::Subject {
                self.subject_constant(entry, constant)?;
            }
        }
        for definition in typed.mathematical_definitions() {
            if self.owner(entry, definition.symbol, subjects, purpose)? == Owner::Subject {
                self.mathematical_predicate_definition(entry, definition, None, &[])?;
            }
        }
        // One row per remaining declaration table of the typed trees' roots.
        // A subject declaration in any of them is outside what the route
        // judges.
        let unjudged = [
            (
                symbols_of(typed.type_aliases(), |row| row.symbol),
                "type alias",
            ),
            (symbols_of(typed.measures(), |row| row.symbol), "measure"),
            (symbols_of(typed.operators(), |row| row.symbol), "operator"),
            (
                symbols_of(typed.machine_token_bindings(), |row| row.symbol),
                "machine token binding",
            ),
            (
                symbols_of(typed.propositions(), |row| row.symbol),
                "proposition",
            ),
            (
                symbols_of(typed.wire_schemas(), |row| row.symbol),
                "wire schema",
            ),
        ];
        for (declared, kind) in unjudged {
            for symbol in declared {
                if self.owner(entry, symbol, subjects, purpose)? == Owner::Subject {
                    return Err(refuse(
                        entry,
                        Construct::Machine,
                        Unserved::Declaration(kind),
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Program<'_> {
    /// Reads a subject data declaration that takes type or const parameters,
    /// `data Buffer<T, const Capacity: u64> { .. }`, as it is written.
    ///
    /// Its fields have no types to resolve until the parameters are given:
    /// generics.md, "Applications and specialization": "Every application validates the
    /// complete argument tuple, including kinds, ranges, and constraints of
    /// unused parameters". So each application is resolved and judged where
    /// a type applies it (`record_at`), unused fields included, and that is
    /// where a field of the applied type is read, moved or constructed.
    ///
    /// What the declaration says by itself is its header and its member
    /// roster, and a declaration nothing applies must still be one this
    /// route reads. So the reading holds both to what the reading of an
    /// application serves (`source_records::source_application`):
    ///
    /// - the header binds types, and integer constants of a builtin integer
    ///   type, with no bound, lifetime, `requires`, cleanup, carry policy or
    ///   quotient, each of which a declaration would owe a judgment for here;
    /// - each member is an ordinary runtime field or payload field, and its
    ///   type is built from builtin scalars, the header's own type
    ///   parameters, fixed arrays whose length is a number or one of the
    ///   header's constants, data with no parameters, and applications of
    ///   data. A closed application or a data type with no parameters is
    ///   resolved here like any other subject type.
    ///
    /// Anything else refuses the declaration.
    fn subject_data_template(
        &mut self,
        entry: MachineId,
        data: &typed_trees::data::DataDefinition,
    ) -> Checked<()> {
        use typed_trees::data::{DataMember, TypeParameterKind};
        let typed = self.typed;
        let unserved = |what| refuse(entry, Construct::Machine, Unserved::Type(what));
        if data.generic_instance.is_some()
            || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || !data.lifetime_parameters.is_empty()
            || !data.conformance_bounds.is_empty()
            || !data.requires_facts.is_empty()
            || data.properties.carry.is_some()
            || data.cleanup_selection.is_some()
            || data.quotient.is_some()
            || !data.retired_identities.is_empty()
        {
            return Err(unserved(
                "generic data with lifetimes, bounds, requirements, cleanup, a quotient or \
                 retired members",
            ));
        }
        let parameters = typed.data_type_parameters(data);
        for parameter in parameters {
            let served = parameter.bounds == typed_trees::data::DataProperties::default()
                && match parameter.kind {
                    TypeParameterKind::Type => true,
                    TypeParameterKind::Const { type_reference } => matches!(
                        typed.type_reference_table.type_reference(type_reference),
                        typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                            if matches!(
                                typed.symbols.builtin_type_atom(*symbol),
                                Some(symbols::BuiltinTypeAtom::Integer(_))
                            )
                    ),
                    _ => false,
                };
            if !served {
                return Err(unserved(
                    "generic data with a bounded parameter or one that is neither a type nor an \
                     integer constant",
                ));
            }
        }
        for member in typed.data_members(data) {
            let fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => typed.data_payload_fields(variant),
            };
            for field in fields {
                if field.relevance != language_core::BindingRelevance::Relevant {
                    return Err(unserved("generic data with an erased member"));
                }
                self.template_member_type(entry, parameters, field.type_reference, 0)?;
            }
        }
        Ok(())
    }

    /// One member type of a generic data declaration, or an argument inside
    /// one, as `subject_data_template` reads it.
    fn template_member_type(
        &mut self,
        entry: MachineId,
        parameters: &[typed_trees::data::TypeParameter],
        reference: typed_trees::types::TypeReferenceHandle,
        depth: usize,
    ) -> Checked<()> {
        use typed_trees::data::TypeParameterKind;
        use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
        let typed = self.typed;
        let unserved = || {
            refuse(
                entry,
                Construct::Machine,
                Unserved::Type("generic data with a member type that is not read as written"),
            )
        };
        // A source type nests far less deeply than this; a graph that does
        // not end is not a member type.
        if depth > 64
            || !typed
                .type_reference_table
                .contains_type_reference(reference)
        {
            return Err(unserved());
        }
        let own = |symbol: SymbolHandle, constant: bool| {
            parameters.iter().any(|parameter| {
                parameter.symbol == symbol
                    && constant == matches!(parameter.kind, TypeParameterKind::Const { .. })
            })
        };
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                use symbols::BuiltinTypeAtom as Atom;
                match typed.symbols.builtin_type_atom(*symbol) {
                    Some(Atom::Bool | Atom::Integer(_) | Atom::F32 | Atom::F64) => Ok(()),
                    Some(_) => Err(unserved()),
                    None if own(*symbol, false) => Ok(()),
                    None if self.data_declaration(*symbol).is_some() => {
                        self.record_at(entry, reference).map(|_| ())
                    }
                    None => Err(unserved()),
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                match length {
                    FixedArrayLength::Literal(_) => {}
                    FixedArrayLength::ConstParameter { symbol, .. } if own(*symbol, true) => {}
                    _ => return Err(unserved()),
                }
                self.template_member_type(entry, parameters, *element_type, depth + 1)
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } => {
                if !lifetime_arguments.is_empty() {
                    return Err(unserved());
                }
                // Applied at closed arguments it is a type like any other.
                if typed.closed_data_definition(reference).is_some() {
                    return self.record_at(entry, reference).map(|_| ());
                }
                // Applied at this header's parameters it is judged, like
                // this declaration, where the whole type is applied.
                let applied = typed
                    .unique_data_definition_by_symbol(*base_symbol)
                    .ok_or_else(unserved)?;
                let formals = typed.data_type_parameters(applied);
                let arguments = typed
                    .type_reference_table
                    .type_reference_handles(*arguments);
                if formals.len() != arguments.len() {
                    return Err(unserved());
                }
                for (formal, argument) in formals.iter().zip(arguments) {
                    match formal.kind {
                        TypeParameterKind::Type => {
                            self.template_member_type(entry, parameters, *argument, depth + 1)?;
                        }
                        TypeParameterKind::Const { .. } => {
                            let named_constant = matches!(
                                typed.type_reference_table.type_reference(*argument),
                                TypeReferenceNode::Named { symbol, .. }
                                    if symbol.is_valid() && own(*symbol, true)
                            );
                            if !named_constant
                                && typed
                                    .static_const_literal_from_type_reference(*argument)
                                    .is_none()
                            {
                                return Err(unserved());
                            }
                        }
                        _ => return Err(unserved()),
                    }
                }
                Ok(())
            }
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Constrained { .. }
            | TypeReferenceNode::Callable { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => Err(unserved()),
        }
    }

    /// A generic schema is checked in its own source binder namespace. The
    /// same declaration owner forms its complete requirement contracts.
    pub(crate) fn generic_trait(
        &mut self,
        declared: &typed_trees::trait_definition::TraitDefinition,
    ) -> Checked<()> {
        let diagnostic = MachineId::new(1).expect("1 is nonzero");
        self.generic_binders.extend(
            self.typed
                .trait_type_parameters(declared)
                .iter()
                .map(|parameter| parameter.symbol),
        );
        if !self.trait_declaration(diagnostic, declared.symbol)? {
            return Err(refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Declaration("generic trait lost its actual declaration"),
            ));
        }
        Ok(())
    }
}

/// Whether a data declaration declares a name and nothing else: no field or
/// case, no parameter, no `requires`, no cleanup and no carry policy. Its
/// multiplicity is not read: with no value of the type there is no custody.
/// Such a declaration has nothing to prove, so as a subject it is passed over. A
/// binding, formal or field of the type resolves it where its type is read
/// (`record`), as a record whose roster is empty.
fn names_only(data: &typed_trees::data::DataDefinition) -> bool {
    data.members.is_empty()
        && data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && data.lifetime_parameters.is_empty()
        && data.type_parameters.is_empty()
        && data.generic_instance.is_none()
        && data.conformance_bounds.is_empty()
        && data.properties.carry.is_none()
        && data.cleanup_selection.is_none()
        && data.quotient.is_none()
        && data.requires_facts.is_empty()
        && data.retired_identities.is_empty()
}

/// Whether everything a domain declaration says is judged where a value of
/// the domain is used, so that as a subject it is passed over: a tag,
/// `domain i32::Km;`, optionally with predicates, an access policy or
/// establishment routes. It has no cleanup selection, operator, alias or
/// parameter. A type that names it resolves it where the type is read
/// (`declared_domain`), and its predicates join the contract of each place
/// declared in it (`predicates`).
///
/// A policy or a route list restricts who may add the domain, read inside
/// a value of it and change one. Each of those uses asks the declaration
/// where it occurs (`casts::issue`, `authority::authorized`), and an
/// answer the checker cannot give refuses the use, so the declaration
/// itself has nothing to prove. Every other domain declaration of a
/// subject is refused until the checker judges what it declares.
fn judged_where_used(
    typed: &typed_trees::TypedTrees,
    domain: &typed_trees::domain::DomainDefinition,
) -> bool {
    domain.alias.is_none()
        && domain.cleanup_selection.is_none()
        && domain.operators.is_empty()
        && domain.index_arguments.is_empty()
        && typed.domain_type_parameters(domain).is_empty()
}

fn symbols_of<T>(rows: &[T], symbol: impl Fn(&T) -> SymbolHandle) -> Vec<SymbolHandle> {
    rows.iter().map(symbol).collect()
}
