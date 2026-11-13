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
    productions: Arc<psi_compiler::PortableProductions>,
    route: Option<RouteTerminalProgram>,
}

/// The application's program as the checker on the lowered form published
/// it: the canonical artifact of exactly the module that checker judged, and
/// the evidence seals bound to that module's identity
/// (`lowered_discharge::AcceptedClosure::publish`). The seals exist only in
/// memory beside this artifact and travel with it to Omega 00.
#[derive(Debug, Clone)]
pub struct RouteTerminalProgram {
    pub artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub seals: terminal_codec::EvidenceSeals,
}

impl std::fmt::Debug for ProductTerminalPublication {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductTerminalPublication")
            .field("optimizations", &self.optimizations)
            .field("published", &self.program.get().map(Result::is_ok))
            .field("route", &self.route.is_some())
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
            route: None,
        }
    }

    /// The same publication with the application's program taken from the
    /// lowered-form route. An executable application is then attached to
    /// that artifact; the shared program above still serves every other
    /// reader of published code.
    pub fn with_route(mut self, route: Option<RouteTerminalProgram>) -> Self {
        self.route = route;
        self
    }

    pub fn route(&self) -> Option<&RouteTerminalProgram> {
        self.route.as_ref()
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
