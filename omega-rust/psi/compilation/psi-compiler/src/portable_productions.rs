//! Lower one checked Product to portable Terminal code once per invocation.
//!
//! A package that depends on `std` checks the same Product program twice in one
//! compilation: once to produce `std`'s Build provider and once for its own
//! Product. The second check already takes the first judgment, so both arrive
//! with the same shared checked program, yet each owned a fresh publication and
//! lowered all of it again. On a `std` program that second lowering was about
//! 7 s of the compile.
//!
//! Portable production reads only the inputs named by
//! [`PortableProductionInputs`]: the exact checked program, its entry selection,
//! its retained machine and boundary roots, the exact toolchain sources and the
//! Psi optimization selection. The provider inventory it also reads is formed
//! from that program and those sources alone. Equal inputs therefore produce
//! equal code, and a later request takes the earlier result, refusal included.
//! Sharing code grants nothing: each consumer still admits its own application,
//! providers and target.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use psi_terminal_production::{
    PortableTerminalProgram, TerminalMachineSelection, TerminalProgramRoots,
};
use std::sync::{Arc, Mutex, OnceLock};
use symbols::SymbolHandle;

type Production = Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>>;

/// Everything a portable Product production reads.
pub struct PortableProductionInputs<'request> {
    pub program: &'request Arc<CheckedTrees>,
    pub machine: TerminalMachineSelection<'request>,
    pub roots: TerminalProgramRoots<'request>,
    pub exact_toolchain_sources: &'request [(source::SourceId, [u8; 32])],
    pub optimizations: &'request optimization::PsiOptimizationSelections,
}

/// The same inputs, owned. Holding the program keeps its address from being
/// reused by a different program while this entry can still match.
struct ProductionKey {
    program: Arc<CheckedTrees>,
    entry: Option<SymbolHandle>,
    machines: Vec<SymbolHandle>,
    boundaries: Vec<SymbolHandle>,
    exact_toolchain_sources: Vec<(source::SourceId, [u8; 32])>,
    optimizations: optimization::PsiOptimizationSelections,
}

impl ProductionKey {
    /// `None` for a selection this table does not share: inspection by name
    /// asks for another root and keeps its own production.
    fn new(inputs: &PortableProductionInputs<'_>) -> Option<Self> {
        let entry = match inputs.machine {
            TerminalMachineSelection::Symbol(symbol) => Some(symbol),
            TerminalMachineSelection::Declarations => None,
            _ => return None,
        };
        Some(Self {
            program: Arc::clone(inputs.program),
            entry,
            machines: inputs.roots.machines.to_vec(),
            boundaries: inputs.roots.boundaries.to_vec(),
            exact_toolchain_sources: inputs.exact_toolchain_sources.to_vec(),
            optimizations: inputs.optimizations.clone(),
        })
    }

    fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.program, &other.program)
            && self.entry == other.entry
            && self.machines == other.machines
            && self.boundaries == other.boundaries
            && self.exact_toolchain_sources == other.exact_toolchain_sources
            && self.optimizations == other.optimizations
    }
}

/// Invocation-owned portable productions, shared beside [`crate::ProductSelections`].
#[derive(Default)]
pub struct PortableProductions {
    entries: Mutex<Vec<(ProductionKey, Arc<OnceLock<Production>>)>>,
}

impl std::fmt::Debug for PortableProductions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entries = self.entries.lock().map_or(0, |entries| entries.len());
        formatter
            .debug_struct("PortableProductions")
            .field("entries", &entries)
            .finish()
    }
}

impl PortableProductions {
    /// Take the production of equal inputs, or run `produce` for them once.
    /// A concurrent request for equal inputs waits for that one run.
    pub fn produce(
        &self,
        inputs: PortableProductionInputs<'_>,
        produce: impl FnOnce() -> Result<PortableTerminalProgram, Vec<Diagnostic>>,
    ) -> Production {
        let Some(key) = ProductionKey::new(&inputs) else {
            return produce().map(Arc::new);
        };
        let slot = {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match entries.iter().find(|(known, _)| known.matches(&key)) {
                Some((_, slot)) => Arc::clone(slot),
                None => {
                    let slot = Arc::new(OnceLock::new());
                    entries.push((key, Arc::clone(&slot)));
                    slot
                }
            }
        };
        slot.get_or_init(|| produce().map(Arc::new)).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs<'a>(
        program: &'a Arc<CheckedTrees>,
        machine: TerminalMachineSelection<'a>,
        machines: &'a [SymbolHandle],
        sources: &'a [(source::SourceId, [u8; 32])],
        optimizations: &'a optimization::PsiOptimizationSelections,
    ) -> PortableProductionInputs<'a> {
        PortableProductionInputs {
            program,
            machine,
            roots: TerminalProgramRoots {
                machines,
                boundaries: &[],
            },
            exact_toolchain_sources: sources,
            optimizations,
        }
    }

    /// A refusal stands in for produced code: it is shared on the same terms.
    fn run(
        productions: &PortableProductions,
        inputs: PortableProductionInputs<'_>,
        runs: &mut usize,
    ) {
        let result = productions.produce(inputs, || {
            *runs += 1;
            Err(vec![Diagnostic::error("refused")])
        });
        assert_eq!(result.unwrap_err()[0].message, "refused");
    }

    #[test]
    fn equal_inputs_produce_once_and_any_difference_produces_again() {
        let productions = PortableProductions::default();
        let program = Arc::new(CheckedTrees::default());
        let other_program = Arc::new(CheckedTrees::default());
        let optimizations = optimization::PsiOptimizationSelections::default();
        let root = [SymbolHandle::default()];
        let sources = [(source::SourceId(0), [0; 32])];
        let other_sources = [(source::SourceId(1), [0; 32])];
        let entry = TerminalMachineSelection::Declarations;
        let mut runs = 0;

        run(
            &productions,
            inputs(&program, entry, &[], &sources, &optimizations),
            &mut runs,
        );
        run(
            &productions,
            inputs(&program, entry, &[], &sources, &optimizations),
            &mut runs,
        );
        assert_eq!(runs, 1, "equal inputs share one production");

        // An equal but separately owned program is a different program.
        run(
            &productions,
            inputs(&other_program, entry, &[], &sources, &optimizations),
            &mut runs,
        );
        run(
            &productions,
            inputs(&program, entry, &root, &sources, &optimizations),
            &mut runs,
        );
        run(
            &productions,
            inputs(&program, entry, &[], &other_sources, &optimizations),
            &mut runs,
        );
        let symbol = TerminalMachineSelection::Symbol(SymbolHandle::default());
        run(
            &productions,
            inputs(&program, symbol, &[], &sources, &optimizations),
            &mut runs,
        );
        assert_eq!(runs, 5, "each differing input produces its own code");
    }

    #[test]
    fn named_inspection_is_never_shared() {
        let productions = PortableProductions::default();
        let program = Arc::new(CheckedTrees::default());
        let optimizations = optimization::PsiOptimizationSelections::default();
        let named = TerminalMachineSelection::Name("main");
        let mut runs = 0;
        run(
            &productions,
            inputs(&program, named, &[], &[], &optimizations),
            &mut runs,
        );
        run(
            &productions,
            inputs(&program, named, &[], &[], &optimizations),
            &mut runs,
        );
        assert_eq!(runs, 2);
    }
}
