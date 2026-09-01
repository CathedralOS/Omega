//! The production compiler coordinator.
//!
//! This file intentionally declares only [`Compiler`]. Request and
//! host-execution infrastructure live beneath it; language and target semantics
//! belong to the stage crates the coordinator invokes.

use psi_diagnostics::Diagnostic;

mod driver;
pub(crate) mod execution;
mod intrinsic_settlements;
mod native_checked;
mod optimization;
mod options;
pub(crate) use omega_compilation_report as report;
mod request;
mod terminal_authority_permissions;
mod terminal_native_realization;
mod terminal_product;

pub use omega_trust_model::{TrustAdmission, TrustAdmissionSettlement};
pub use optimization::{OptimizationRollback, OptimizationRollbackInputError};
pub use options::{ArtifactEmissionPolicy, CompileOptions};
pub use report::{
    CompileOutputKind, CompileReport, ExecutablePublicationDestination,
    ExecutablePublicationReceipt, FinalRealizationEvidenceError, OptimizationRollbackReceipt,
    ProductionArtifactIdentity, ProductionCompilationManifest,
    ProductionCompilationManifestIdentity, ProductionCompilationSubject, RetainedNativeArtifact,
};
pub use request::{CompileRequest, RequestedCompileProduct};
pub use terminal_native_realization::{
    SourceEvaluatedImportSettlement,
    realize_retained_terminal_artifact_with_source_evaluated_imports,
    realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy,
};

/// The reusable production compiler coordinator.
#[derive(Debug, Default, Clone, Copy)]
pub struct Compiler;

impl Compiler {
    pub const fn new() -> Self {
        Self
    }

    pub fn compile(self, request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
        execution::run_on_compile_thread(move || driver::compile(request))
    }
}

/// Execute one typed production compiler request.
pub fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    Compiler::new().compile(request)
}

#[cfg(test)]
#[path = "compiler/tests.rs"]
mod tests;
