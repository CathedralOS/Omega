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
(`Elaboration::conformance_witnesses`). Every other kind of declaration is
refused as `Unsupported`, because this route judges none of them yet and a
declaration left out would be accepted without a row or a refusal.

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
            if typed.machine_by_symbol(*symbol).is_some() {
                let machine = self.machine(*symbol)?;
                self.fixed_inputs.push(machine);
            } else if self.data_declaration(*symbol).is_some() {
                self.record(entry, *symbol)?;
            } else if typed
                .trait_machine_signature(*symbol)
                .is_some_and(|(owner, requirement)| {
                    owner.is_boundary && requirement.symbol == *symbol
                })
            {
                self.boundary(entry, *symbol)?;
            } else if let Some(owner) = typed
                .trait_definition_by_symbol(*symbol)
                .filter(|owner| owner.is_boundary)
            {
                self.declared_service(entry, owner)?;
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
            // The slot is a boundary trait, supplied whole, or one
            // requirement of a boundary trait.
            let (owner, requirements) = match typed.trait_definition_by_symbol(supply.slot) {
                Some(owner) => (
                    owner,
                    typed
                        .trait_machine_signatures(owner)
                        .iter()
                        .map(|requirement| requirement.symbol)
                        .collect::<Vec<_>>(),
                ),
                None => match typed.trait_machine_signature(supply.slot) {
                    Some((owner, requirement)) if requirement.symbol == supply.slot => {
                        (owner, vec![supply.slot])
                    }
                    _ => {
                        return unserved(
                            "provider supply of a slot that is not a boundary trait or one of its requirements",
                        );
                    }
                },
            };
            if !owner.is_boundary {
                return unserved("provider supply of a trait that is not a boundary");
            }
            for requirement in requirements {
                let mut realizing = typed.machines().iter().filter(|machine| {
                    typed_trees::provider_declarations::satisfaction_provider_declaration(
                        typed, machine,
                    ) == Some(supply.provider)
                        && (typed
                            .machine_trait_conformances(machine)
                            .iter()
                            .any(|clause| clause.requirement_symbol == requirement)
                            || typed
                                .owned_conformance_realization(machine.symbol)
                                .is_some_and(|(_, row)| row.requirement == requirement))
                });
                let (Some(machine), None) = (realizing.next(), realizing.next()) else {
                    return unserved(
                        "provider that does not give a supplied requirement exactly one machine",
                    );
                };
                let witness = crate::ConformanceWitness {
                    declaring_trait: owner.symbol,
                    requirement,
                    implementation: machine.symbol,
                };
                let root = self.provider(machine.symbol, owner.symbol, requirement)?;
                if !self.fixed_inputs.contains(&root) {
                    self.fixed_inputs.push(root);
                }
                let supplied = crate::SuppliedRequirement {
                    provider: supply.provider,
                    requirement,
                    machine: root,
                };
                if !self.supplied.contains(&supplied) {
                    self.supplied.push(supplied);
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
            // A generic machine is checked once as written, as its own check
            // form, not as a closed body (`Elaboration::generic_templates`).
            // One whose only parameters are implementations is generic too.
            if !machine.has_specialization_parameters() {
                let id = self.machine(machine.symbol)?;
                self.subject_machines.push(id);
            } else {
                self.generic_templates.push(machine.symbol);
            }
        }
        for data in typed.data_definitions() {
            if self.owner(entry, data.symbol, subjects, purpose)? == Owner::Subject
                && !names_only(data)
            {
                self.record(entry, data.symbol)?;
            }
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
            self.declared_predicates(entry, domain)?;
        }
        for declared in typed.traits() {
            if self.owner(entry, declared.symbol, subjects, purpose)? == Owner::Subject {
                self.subject_trait(entry, declared)?;
            }
        }
        for declared in typed.conformances() {
            if self.owner(entry, declared.symbol, subjects, purpose)? == Owner::Subject {
                self.subject_implementation(entry, declared)?;
            }
        }
        for constant in typed.const_declarations() {
            if self.owner(entry, constant.symbol, subjects, purpose)? == Owner::Subject {
                self.subject_constant(entry, constant)?;
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
                symbols_of(typed.mathematical_definitions(), |row| row.symbol),
                "mathematical definition",
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

/// Whether a trait is a list of requirements and nothing else: no
/// parameter, bound, schema clause or refinement of another trait, and not a
/// boundary, whose requirements a provider supplies.
fn lists_requirements_only(declared: &typed_trees::trait_definition::TraitDefinition) -> bool {
    !declared.is_boundary
        && declared.lifetime_parameters.is_empty()
        && declared.type_parameters.is_empty()
        && declared.conformance_bounds.is_empty()
        && declared.requires.is_empty()
        && declared.refines.is_none()
        && declared.refinement_clauses.is_empty()
}

impl<'t> Program<'t> {
    /// A subject trait that only lists requirements declares contracts with
    /// no body: there is nothing to prove of it, and each contract must be
    /// one the route reads, or a requirement nothing reads would be accepted
    /// without a row or a refusal.
    fn subject_trait(
        &mut self,
        entry: MachineId,
        declared: &'t typed_trees::trait_definition::TraitDefinition,
    ) -> Checked<()> {
        if !lists_requirements_only(declared) {
            return Err(refuse(
                entry,
                Construct::Machine,
                Unserved::Declaration(
                    "trait with parameters, schema clauses, a refinement or a boundary",
                ),
            ));
        }
        for requirement in self.typed.trait_machine_signatures(declared) {
            // A requirement with value parameters has one contract per tuple
            // of values (`requirement`), read where a call or a selected
            // implementation supplies them. Read as a subject it has none,
            // and the witness its implementations owe for every value at
            // once is not built: refused by that name, not as a mismatch of
            // arguments nobody gave.
            if !self.requirement_binders(entry, requirement)?.is_empty() {
                return Err(refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Declaration("trait requirement with value parameters"),
                ));
            }
            let (signature, _) = self.requirement_signature(entry, requirement, &[])?;
            let id = self.declaration_signatures.push(signature);
            if self.declaration_selections.push(None) != id {
                return Err(refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Declaration("public requirement lost its selection coordinate"),
                ));
            }
            if self.declaration_sources.push(requirement) != id {
                return Err(refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Declaration("public requirement lost its declaration coordinate"),
                ));
            }
        }
        Ok(())
    }

    /// A subject implementation of a trait that only lists requirements,
    /// with no parameter of its own, owes one witness per requirement: the
    /// machine its row selects refines that requirement. An implementation
    /// that leaves a requirement without exactly one row is refused.
    fn subject_implementation(
        &mut self,
        entry: MachineId,
        declared: &'t typed_trees::trait_definition::Conformance,
    ) -> Checked<()> {
        let unserved = |what| {
            Err(refuse(
                entry,
                Construct::Machine,
                Unserved::Declaration(what),
            ))
        };
        let typed = self.typed;
        let plain = declared.lifetime_parameters.is_empty()
            && declared.type_parameters.is_empty()
            && declared.trait_lifetime_arguments.is_empty()
            && declared.arguments.is_empty();
        let Some(implemented) = typed
            .trait_definition_by_symbol(declared.trait_symbol)
            .filter(|implemented| plain && lists_requirements_only(implemented))
        else {
            return unserved("implementation with parameters or of a trait that has them");
        };
        let requirements = typed.trait_machine_signatures(implemented);
        if declared.rows.len() != requirements.len() {
            return unserved("implementation that does not give each requirement one machine");
        }
        for requirement in requirements {
            let mut rows = declared
                .rows
                .iter()
                .filter(|row| row.requirement == requirement.symbol);
            let (Some(row), None) = (rows.next(), rows.next()) else {
                return unserved("implementation that does not give each requirement one machine");
            };
            if row.declaring_trait != implemented.symbol {
                return unserved("implementation row of an inherited requirement");
            }
            if row.source == typed_trees::trait_definition::ConformanceRowSource::Reference {
                let Some(realization) = typed.machine_by_symbol(row.realization_machine) else {
                    return unserved("selection row without an existing machine declaration");
                };
                // A reference attaches a route only to an independently
                // valid plain machine. An owning body already has its
                // requirement, so another row cannot rebind that entry.
                if !realization.satisfies.is_empty()
                    || typed
                        .owned_conformance_realization(realization.symbol)
                        .is_some()
                {
                    return unserved("selection row cannot rebind a declared conformer");
                }
            }
            let witness = crate::ConformanceWitness {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                implementation: row.realization_machine,
            };
            if !self.conformance_witnesses.contains(&witness) {
                self.conformance_witnesses.push(witness);
            }
        }
        Ok(())
    }
}

/// Whether a data declaration declares a name and nothing else: no field or
/// case, no parameter, no `requires`, no cleanup and no carry policy. Its
/// multiplicity is not read: with no value of the type there is no custody.
/// `data Counter {}` beside `machine Counter::clear()` is one: it completes
/// machine names (`machine_shape`) and no value of it holds anything. Such a
/// declaration has nothing to prove, so as a subject it is passed over. A
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
