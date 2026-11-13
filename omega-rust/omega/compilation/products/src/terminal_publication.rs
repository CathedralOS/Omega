//! Retain the accepted program and its publication result for product consumers.
//! Reports share one portable wrapper with fixed Psi optimization choices. A
//! failed publication remains a failure; target selection never tries another
//! source producer to replace it.

use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use psi_compiler::PortableTerminalProgram;
use std::sync::{Arc, OnceLock};

pub struct ProductTerminalPublication {
    optimizations: OptimizationSelections,
    program: OnceLock<Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>>>,
    productions: Arc<psi_compiler::PortableProductions>,
    checked_application: Result<Arc<CheckedApplicationProgram>, Vec<Diagnostic>>,
}

// Product and provider preparation share one authenticated source owner;
// neither creates a second declaration packet or rechecks a source body.
pub use provider_planning::{
    PublishedProviderDeclarations as ProductSourceDeclarations,
    PublishedProviderSource as CheckedApplicationProgram,
};

impl std::fmt::Debug for ProductTerminalPublication {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductTerminalPublication")
            .field("optimizations", &self.optimizations)
            .field("published", &self.program.get().map(Result::is_ok))
            .field("checked_application", &self.checked_application.is_ok())
            .finish()
    }
}

impl ProductTerminalPublication {
    pub fn new(
        optimizations: OptimizationSelections,
        productions: Arc<psi_compiler::PortableProductions>,
    ) -> Self {
        Self {
            optimizations,
            program: OnceLock::new(),
            productions,
            checked_application: Err(vec![Diagnostic::error(
                "no checked application program was retained for product production",
            )]),
        }
    }

    /// Keep publication failures beside accepted source checking. A consumer
    /// needing executable code must report this result, never produce another
    /// program from the old checker's output after a failure.
    pub fn with_checked_application(
        mut self,
        program: Result<Arc<CheckedApplicationProgram>, Vec<Diagnostic>>,
    ) -> Self {
        self.checked_application = program;
        self
    }

    pub fn checked_application(&self) -> Result<&Arc<CheckedApplicationProgram>, Vec<Diagnostic>> {
        self.checked_application.as_ref().map_err(Clone::clone)
    }

    // The same portable productions published for a different root
    // selection. Roots decide which machines the Terminal program retains, so
    // a separately selected test root never reuses the application's program.
    pub fn for_other_roots(&self) -> Self {
        Self::new(self.optimizations.clone(), Arc::clone(&self.productions))
    }

    pub fn productions(&self) -> &psi_compiler::PortableProductions {
        &self.productions
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
        let publication =
            ProductTerminalPublication::new(OptimizationSelections::default(), Default::default());
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
}
