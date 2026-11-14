// One invocation's portable Terminal code, shared by policy and Product
// consumers. `ProductTerminalPublication` fixes Psi optimization choices before
// targets settle and memoizes the produced `PortableTerminalProgram` (or its
// refusal) in a `OnceLock` without retrying or permitting a different Psi
// optimization projection to replace it.

use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use psi_compiler::PortableTerminalProgram;
use std::sync::{Arc, OnceLock};

pub struct ProductTerminalPublication {
    optimizations: OptimizationSelections,
    program: OnceLock<Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>>>,
    accepted: Option<(Arc<psi_compiler::AcceptedTerminalSource>, ProductRoster)>,
}

/// What a Product compilation asks the lowered-form checker to judge: the
/// roots the evaluated Build states (`psi_compiler::TerminalSourceCompilation`
/// holds the same fields by reference). It is kept beside the judgment it
/// produced because provider settlement extends it: the supply chosen for a
/// requirement the accepted module reaches is a further root of the same
/// compilation, known only after the first judgment.
#[derive(Debug, Clone, Default)]
pub struct ProductRoster {
    /// The Build's bound program entries. The first is the module's entry.
    pub entries: Vec<symbols::SymbolHandle>,
    /// The package whose declarations the check claims; `None` for a source
    /// checked outside a package.
    pub subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Layout-scope entries, native binding declarations and chosen opaque
    /// representations the Build names.
    pub selected: Vec<symbols::SymbolHandle>,
    /// One visible-continuation obligation per executed Build binding.
    pub bindings: Vec<typed_trees::callable_signature_shapes::BoundCallableShape>,
    /// The providers chosen to supply boundary slots: the Build's own
    /// selections first, then what settlement chose for reached requirements.
    pub supplies: Vec<psi_compiler::TerminalSourceSupply>,
}

impl ProductRoster {
    /// The lowered-form checker's judgment of this roster over `typed`: the
    /// closure of the roots, the check form of each generic machine the
    /// subject declares, and the refinement witness of each implementation
    /// row and chosen supply. Its verdict is the only verdict of a product
    /// compilation: it accepts only when every part does, a roster it does
    /// not accept produces no module, and a missing publication never
    /// selects another stage's module.
    pub fn judge(
        &self,
        typed: Arc<typed_trees::FrozenTypedTrees>,
    ) -> Result<Arc<psi_compiler::AcceptedTerminalSource>, Vec<Diagnostic>> {
        psi_compiler::AcceptedTerminalSource::judge(
            typed,
            psi_compiler::TerminalSourceCompilation {
                entries: &self.entries,
                subjects: &[self.subject],
                purpose: source::DependencyScope::Product,
                selected: &self.selected,
                bindings: &self.bindings,
                supplies: &self.supplies,
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .map(Arc::new)
    }
}

impl std::fmt::Debug for ProductTerminalPublication {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductTerminalPublication")
            .field("optimizations", &self.optimizations)
            .field("published", &self.program.get().map(Result::is_ok))
            .field("accepted", &self.accepted.is_some())
            .finish()
    }
}

impl ProductTerminalPublication {
    pub fn new(optimizations: OptimizationSelections) -> Self {
        Self {
            optimizations,
            program: OnceLock::new(),
            accepted: None,
        }
    }

    /// Retains the judgment of `roster` and the roster it judged.
    pub fn with_accepted(
        mut self,
        source: Arc<psi_compiler::AcceptedTerminalSource>,
        roster: ProductRoster,
    ) -> Self {
        self.accepted = Some((source, roster));
        self
    }

    /// The roster the retained judgment accepted.
    pub fn roster(&self) -> Result<&ProductRoster, Vec<Diagnostic>> {
        self.accepted
            .as_ref()
            .map(|(_, roster)| roster)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "no product route yet: this root selection has no accepted source",
                )]
            })
    }

    pub fn require_source_owner(
        &self,
        source: &Arc<typed_trees::FrozenTypedTrees>,
    ) -> Result<(), Diagnostic> {
        let (accepted, _) = self.accepted.as_ref().ok_or_else(|| {
            Diagnostic::error("no product route yet: the source has no retained accepted judgment")
        })?;
        if !Arc::ptr_eq(source, accepted.source()) {
            return Err(Diagnostic::error(
                "no product route yet: product publication retains a different frozen source owner",
            ));
        }
        Ok(())
    }

    pub fn accepted(&self) -> Result<&psi_compiler::AcceptedTerminalSource, Vec<Diagnostic>> {
        // The factory seals the judged module with the empty Psi execution
        // record. A requested transform needs its own evidence transport.
        if !self.optimizations.project_psi().selections().is_empty() {
            return Err(vec![Diagnostic::error(
                "no product route yet: selected Psi optimizations have no accepted evidence transport",
            )]);
        }
        self.accepted
            .as_ref()
            .map(|(accepted, _)| accepted.as_ref())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "no product route yet: this root selection has no accepted source",
                )]
            })
    }

    // A separately selected test root needs its own structural judgment. It
    // cannot reuse the application's closure or a cached old production.
    pub fn for_other_roots(&self) -> Self {
        Self::new(self.optimizations.clone())
    }

    pub fn optimizations(&self) -> &OptimizationSelections {
        &self.optimizations
    }

    pub fn published(&self) -> Option<&PortableTerminalProgram> {
        self.program.get()?.as_ref().ok().map(Arc::as_ref)
    }

    pub fn require_published(
        &self,
        optimizations: &OptimizationSelections,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        if self.optimizations.project_psi().selections() != optimizations.project_psi().selections()
        {
            return Err(vec![Diagnostic::error(
                "Terminal publication differs from the Product's fixed Psi optimization selection",
            )]);
        }
        self.program.get().cloned().ok_or_else(|| {
            vec![Diagnostic::error(
                "target compilation requires the shared Terminal program to be published first",
            )]
        })?
    }

    pub fn retain(
        &self,
        optimizations: &OptimizationSelections,
        produce: impl FnOnce() -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>>,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        if self.optimizations.project_psi().selections() != optimizations.project_psi().selections()
        {
            return Err(vec![Diagnostic::error(
                "Terminal publication differs from the Product's fixed Psi optimization selection",
            )]);
        }
        self.program.get_or_init(produce).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use optimization_core::Optimization;

    #[test]
    fn publication_keeps_its_projection_and_one_refusal_without_retry() {
        let publication = ProductTerminalPublication::new(OptimizationSelections::default());
        let mut attempts = 0;
        for _ in 0..2 {
            let result = publication.retain(&OptimizationSelections::default(), || {
                attempts += 1;
                Err(vec![Diagnostic::error("fixed source refusal")])
            });
            assert_eq!(result.unwrap_err()[0].message, "fixed source refusal");
        }
        assert_eq!(attempts, 1);
        let different = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        let error = publication
            .retain(&different, || {
                attempts += 1;
                Err(Vec::new())
            })
            .unwrap_err();
        assert!(
            error[0]
                .message
                .contains("fixed Psi optimization selection")
        );
        assert_eq!(
            attempts, 1,
            "a different projection cannot replace published code or its refusal"
        );
    }
    #[test]
    fn accepted_publication_refuses_unexecuted_psi_optimizations() {
        let selected = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        let publication = ProductTerminalPublication::new(selected);
        assert!(
            publication.accepted().unwrap_err()[0]
                .message
                .contains("Psi optimizations")
        );
    }
}
