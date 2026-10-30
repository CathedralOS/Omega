//! One immutable program's exact requirement-to-machine associations.
//!
//! Termination fixed points and denotational closures repeatedly ask which
//! machines satisfy a declaration. The associations do not change with either
//! judgment: prepare them once in the owning checking invocation, retaining the
//! ordinary declaration resolver and every machine roster occurrence. Bare
//! mutable programs retain their one-shot scan, so no stale association can
//! escape a context or follow a mutation projection.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(crate) struct RequirementSatisfierIndex {
    // Requirement identity, then authored machine position. Positions retain
    // distinct occurrences even when malformed input repeats a machine symbol.
    rows: Vec<(SymbolHandle, usize)>,
}

fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

impl RequirementSatisfierIndex {
    fn prepare(program: &TypedTrees) -> Self {
        let mut rows = Vec::new();
        let mut requirements = Vec::new();
        for (position, machine) in program.machines().iter().enumerate() {
            requirements.clear();
            for conformance in program.machine_trait_conformances(machine) {
                if let Some(declaration) = typed_trees::machine::resolve_satisfied_declaration(
                    program,
                    machine,
                    conformance,
                ) {
                    requirements.push(declaration.symbol());
                }
            }
            // The original per-machine `any` publishes at most one result for
            // each requirement, however many conformances name that same edge.
            requirements.sort_unstable_by_key(|symbol| symbol_key(*symbol));
            requirements.dedup();
            rows.extend(
                requirements
                    .iter()
                    .map(|requirement| (*requirement, position)),
            );
        }
        rows.sort_unstable_by_key(|(requirement, position)| (symbol_key(*requirement), *position));
        Self { rows }
    }

    fn satisfiers(&self, program: &TypedTrees, requirement: SymbolHandle) -> Vec<SymbolHandle> {
        let key = symbol_key(requirement);
        let start = self
            .rows
            .partition_point(|(symbol, _)| symbol_key(*symbol) < key);
        let end = self
            .rows
            .partition_point(|(symbol, _)| symbol_key(*symbol) <= key);
        self.rows[start..end]
            .iter()
            .map(|(_, position)| program.machines()[*position].symbol)
            .collect()
    }
}

/// The machines that declare this boundary requirement satisfied. A call to a
/// requirement denotes whichever satisfier the build selects, so a source-level
/// judgment that has no selection row must account for all of them. The
/// denotational-call closure and the machine progress summary consult this same
/// query, so both layers agree on what a requirement denotes.
pub fn requirement_satisfiers(
    program: &dyn crate::CheckingInput,
    requirement: SymbolHandle,
) -> Vec<SymbolHandle> {
    if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
        return memos
            .requirement_satisfiers
            .get_or_init(|| {
                std::sync::Arc::new(RequirementSatisfierIndex::prepare(program.typed_program()))
            })
            .satisfiers(program.typed_program(), requirement);
    }
    direct_satisfiers(program.typed_program(), requirement)
}

fn direct_satisfiers(program: &TypedTrees, requirement: SymbolHandle) -> Vec<SymbolHandle> {
    program
        .machines()
        .iter()
        .filter(|machine| {
            program
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| {
                    typed_trees::machine::resolve_satisfied_declaration(
                        program,
                        machine,
                        conformance,
                    )
                    .is_some_and(|satisfied| satisfied.symbol() == requirement)
                })
        })
        .map(|machine| machine.symbol)
        .collect()
}
