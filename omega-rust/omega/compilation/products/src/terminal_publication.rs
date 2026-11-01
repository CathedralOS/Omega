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
    pub fn new(optimizations: OptimizationSelections) -> Self {
        Self {
            optimizations,
            program: OnceLock::new(),
        }
    }

    pub fn optimizations(&self) -> &OptimizationSelections {
        &self.optimizations
    }

    /// Retain one exact Psi projection. Post-Terminal choices do not change
    /// portable code; a different Psi projection must belong to another source
    /// product, never silently replace this publication or borrow stale code.
    pub fn retain(
        &self,
        optimizations: &OptimizationSelections,
        produce: impl FnOnce() -> Result<PortableTerminalProgram, Vec<Diagnostic>>,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        if self.optimizations.project_psi().selections() != optimizations.project_psi().selections()
        {
            return Err(vec![Diagnostic::error(
                "Terminal publication differs from the Product's fixed Psi optimization selection",
            )]);
        }
        self.program.get_or_init(|| produce().map(Arc::new)).clone()
    }
}
