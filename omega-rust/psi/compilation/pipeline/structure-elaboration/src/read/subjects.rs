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
declaration is resolved as a record. Every other kind of declaration is
refused as `Unsupported`, because this route judges none of them yet and a
declaration left out would be accepted without a row or a refusal.

Whether a declaration belongs to the subject is decided from the source file
its symbol was declared in, never from a path or a name:

- A toolchain file, or a file of another admitted package, is a dependency.
  Its declarations are judged only when a subject declaration reaches them.
- A file read under the Build's dependency scope belongs to the Build
  program, which its own activation checks before it runs.
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
            || file.dependency_scope == DependencyScope::Build
            || !subjects.contains(&file.package_identity);
        Ok(if dependency {
            Owner::Dependency
        } else {
            Owner::Subject
        })
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
            if self.trait_declaration(entry, *symbol)?
                || self.conformance_declaration(entry, *symbol)?
            {
                continue;
            }
            if self.select_native_provider(entry, *symbol)? {
                continue;
            }
            if typed.machine_by_symbol(*symbol).is_some() {
                let machine = self.machine(*symbol)?;
                self.fixed_inputs.push(machine);
            } else if let Some(data) = self.data_declaration(*symbol) {
                if !names_only(data) {
                    self.record(entry, *symbol)?;
                }
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
    ) -> Checked<()> {
        // A concrete evaluation asks for its existing entry and call closure,
        // not a full check of the source files retained for name resolution.
        if subjects.is_empty() {
            return Ok(());
        }
        let typed = self.typed;
        for machine in typed.machines() {
            if self.owner(entry, machine.symbol, subjects)? != Owner::Subject {
                continue;
            }
            if self.select_native_provider(entry, machine.symbol)? {
                continue;
            }
            // A generic machine is checked once as written, as its own check
            // form, not as a closed body (`Elaboration::generic_templates`).
            if machine.type_parameters.is_empty() {
                let id = self.machine(machine.symbol)?;
                self.subject_machines.push(id);
            } else {
                self.generic_templates.push(machine.symbol);
            }
        }
        for data in typed.data_definitions() {
            if self.owner(entry, data.symbol, subjects)? == Owner::Subject && !names_only(data) {
                self.record(entry, data.symbol)?;
            }
        }
        for declaration in typed.traits() {
            if self.owner(entry, declaration.symbol, subject)? == Owner::Subject {
                self.trait_declaration(entry, declaration.symbol)?;
            }
        }
        for declaration in typed.conformances() {
            if self.owner(entry, declaration.symbol, subject)? == Owner::Subject {
                self.conformance_declaration(entry, declaration.symbol)?;
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
            (
                symbols_of(typed.domain_definitions(), |row| row.symbol),
                "domain",
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
                if self.owner(entry, symbol, subjects)? == Owner::Subject {
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

fn symbols_of<T>(rows: &[T], symbol: impl Fn(&T) -> SymbolHandle) -> Vec<SymbolHandle> {
    rows.iter().map(symbol).collect()
}
