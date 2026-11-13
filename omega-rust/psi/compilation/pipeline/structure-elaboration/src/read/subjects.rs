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
  An explicit `None` subject selects standalone user source. An empty request
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
        let symbols = &self.typed.symbols;
        let file = symbol
            .is_valid()
            .then(|| symbols.symbol_provenance_source_span(symbol))
            .flatten()
            .and_then(|span| symbols.source_file(span))
            .ok_or_else(|| {
                refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Declaration("declaration without a source file"),
                )
            })?;
        let dependency = file.origin.is_toolchain()
            || file.dependency_scope != purpose
            || !subjects.contains(&file.package_identity);
        Ok(if dependency {
            Owner::Dependency
        } else {
            Owner::Subject
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
    /// selected data type is resolved as a record. A selection of anything
    /// else (a trait, a requirement, an operator family) is refused: the
    /// route does not judge a provider against what it provides.
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
            } else {
                return Err(refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Declaration("Build selection that is not a machine or a data type"),
                ));
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
            if self.owner(entry, domain.symbol, subjects, purpose)? == Owner::Subject
                && !judged_where_used(typed, domain)
            {
                return Err(refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Declaration(
                        "domain with predicates, a cleanup machine, operators or parameters",
                    ),
                ));
            }
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
        // One row per remaining declaration table of the typed trees' roots.
        // A subject declaration in any of them is outside what the route
        // judges.
        let unjudged = [
            (
                symbols_of(typed.const_declarations(), |row| row.symbol),
                "const",
            ),
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
            self.requirement_signature(entry, requirement, &[])?;
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
            self.conformance_witnesses.push(crate::ConformanceWitness {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                implementation: row.realization_machine,
            });
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
/// `domain i32::Km;`, optionally with an access policy or establishment
/// routes. It has no predicate, cleanup selection, operator, alias or
/// parameter. A type that names it resolves it where the type is read
/// (`declared_domain`).
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
        && domain.facts.is_empty()
        && domain.predicate_presence == language_semantics::DomainPredicatePresence::Absent
        && domain.operators.is_empty()
        && domain.index_arguments.is_empty()
        && typed.domain_type_parameters(domain).is_empty()
}

fn symbols_of<T>(rows: &[T], symbol: impl Fn(&T) -> SymbolHandle) -> Vec<SymbolHandle> {
    rows.iter().map(symbol).collect()
}
