//! Portable references name a family contract, never an authored member.
//!
//! Resolution gives each product family a separate bodyless Requirement whose
//! fresh telescope conjoins member premises. Typing checks every retained
//! member's complete callable shape; checking publishes their joined operational
//! and crash ceilings before reaching this gate. That Requirement has no private
//! body or return-range guarantee to import. Exact member references remain
//! confined to target-scoped bodies, including references in proofs and types.
//! Execution selection is a later consumer obligation, not source authority.

use diagnostics::Diagnostic;
use std::collections::HashSet;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::references::{StaticArgumentCall, StaticArgumentRole, TypedReferences};

pub(super) fn validate_portable_references(
    checked: &checked_trees::CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let program = &checked.typed;
    if program.target_machine_families.is_empty() {
        return Ok(());
    }
    let mut references = PortableReferences {
        program,
        owner: SymbolHandle::invalid(),
        expressions: HashSet::new(),
        types: HashSet::new(),
        refused: Vec::new(),
    };
    typed_trees::references::visit_declarations(program, &mut references);
    for machine in program.machines() {
        if !references.declaration(machine.symbol) {
            continue;
        }
        typed_trees::references::visit_machine(program, &mut references, machine);
    }
    // Implicit execution edges (notably cleanup) have no authored call node.
    // Use the checking owner's resolved dependencies as well as typed syntax.
    for dependency in checked.facts.flow.semantic_dependencies.iter() {
        if references.declaration(dependency.consumer_machine) {
            references.extend([dependency.dependency]);
        }
    }
    for (_, state) in checked.facts.flow.control.states.iter() {
        if references.declaration(state.machine_symbol) {
            references.extend(
                crate::flow::call_flows(&checked.facts.flow.control, state.calls)
                    .iter()
                    .map(|call| call.target_symbol),
            );
        }
    }
    if references.refused.is_empty() {
        return Ok(());
    }
    Err(references
        .refused
        .into_iter()
        .map(|(owner, member)| {
            Diagnostic::error(format!(
                "unimplemented: portable target-family contract for `{}` referenced by `{}`; \
                 source checking cannot use a selected target body's guarantees",
                program.symbols.display_path(member, "::"),
                program.symbols.display_path(owner, "::"),
            ))
        })
        .collect())
}

struct PortableReferences<'program> {
    program: &'program TypedTrees,
    owner: SymbolHandle,
    expressions: HashSet<typed_trees::expression::ExpressionHandle>,
    types: HashSet<typed_trees::types::TypeReferenceHandle>,
    refused: Vec<(SymbolHandle, SymbolHandle)>,
}

impl Extend<SymbolHandle> for PortableReferences<'_> {
    fn extend<References: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: References) {
        for symbol in symbols {
            let machine = self
                .program
                .machine_by_symbol(symbol)
                .or_else(|| self.program.machine_holding_state(symbol));
            let Some(machine) = machine else { continue };
            if self.program.machine_authored_target(machine).is_some()
                && !self.refused.contains(&(self.owner, machine.symbol))
            {
                self.refused.push((self.owner, machine.symbol));
            }
        }
    }
}

impl TypedReferences for PortableReferences<'_> {
    fn layout_realization_dependency(&mut self, _: SymbolHandle) {
        // The layout retains every member's fold dependency. This does not
        // grant portable source access to any selected body's guarantees.
    }

    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        // A named implementation's row is an authored selection candidate,
        // not a use of that body's semantics. Its machine is checked separately.
        if self
            .program
            .machine_by_symbol(symbol)
            .is_some_and(|machine| {
                machine.purpose == typed_trees::machine::MachinePurpose::ProviderDefaults
                    || self.program.source_target_context(machine).is_some()
            })
            || self.program.symbols.get(symbol).kind == symbols::SymbolKind::Conformance
            || self
                .program
                .symbols
                .symbol_provenance_source_span(symbol)
                .and_then(|span| self.program.symbols.source_file(span))
                .is_some_and(|source| source.dependency_scope == source::DependencyScope::Build)
        {
            return false;
        }
        self.owner = symbol;
        self.expressions.clear();
        self.types.clear();
        true
    }

    fn expression(&mut self, handle: typed_trees::expression::ExpressionHandle) -> bool {
        self.expressions.insert(handle)
    }

    fn type_reference(&mut self, handle: typed_trees::types::TypeReferenceHandle) -> bool {
        self.types.insert(handle)
    }

    fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
        validation::compiler_operations::static_argument_role(program, call)
            == StaticArgumentRole::ValueDependency
    }
}
