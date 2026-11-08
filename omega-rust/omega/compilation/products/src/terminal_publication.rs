//! One invocation's portable code, shared by policy and Product consumers.
//!
//! Source completion fixes optimization choices before targets settle. The
//! producing operation supplies code once; retaining it grants no application,
//! provider or native authority and does not retain checked source.

use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use psi_compiler::PortableTerminalProgram;
use std::sync::{Arc, OnceLock};

pub struct ProductTerminalPublication {
    optimizations: OptimizationSelections,
    program: OnceLock<Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>>>,
    /// Another publication of the same checked program and roots in this
    /// invocation reads the code this one produced instead of lowering again.
    productions: Arc<psi_compiler::PortableProductions>,
}

impl std::fmt::Debug for ProductTerminalPublication {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductTerminalPublication")
            .field("optimizations", &self.optimizations)
            .field("published", &self.program.get().map(Result::is_ok))
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
        }
    }

    /// The invocation's shared productions, keyed by their exact inputs.
    pub fn productions(&self) -> &psi_compiler::PortableProductions {
        &self.productions
    }

    pub fn optimizations(&self) -> &OptimizationSelections {
        &self.optimizations
    }

    /// Borrow already published code for descriptive consumers. This cannot
    /// start production, retry a refusal, or grant target execution authority.
    pub fn published(&self) -> Option<&PortableTerminalProgram> {
        self.program.get()?.as_ref().ok().map(Arc::as_ref)
    }

    /// Read the shared program after publication. Target compilation cannot
    /// invoke a source producer or retry a saved failure through this method.
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

    /// Retain one exact Psi projection. Post-Terminal choices do not change
    /// portable code; a different Psi projection must belong to another source
    /// product, never silently replace this publication or borrow stale code.
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
