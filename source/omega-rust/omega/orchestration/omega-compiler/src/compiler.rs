//! The production compiler coordinator.
//!
//! This file intentionally declares only [`Compiler`]. Request, harness, and
//! host-execution infrastructure live beneath it; language and target semantics
//! belong to the stage crates the coordinator invokes.

use psi_diagnostics::Diagnostic;

mod driver;
pub(crate) mod execution;
mod harness;
mod options;
pub(crate) use omega_compilation_report as report;
mod request;

pub use harness::CompileHarnessRequest;
pub use options::{ArtifactEmissionPolicy, CompileOptions};
pub use report::{
    CompileOutputKind, CompileReport, ExecutablePublicationDestination,
    ExecutablePublicationReceipt, RetainedNativeArtifact, TerminalComponentDeploymentReportError,
};
pub use request::{CompileRequest, RequestedCompileProduct};

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

    #[doc(hidden)]
    pub fn compile_harness(
        self,
        request: CompileHarnessRequest,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        execution::run_on_compile_thread(move || {
            let requested_product = if request.options.write_output {
                RequestedCompileProduct::NativeArtifact
            } else {
                RequestedCompileProduct::Check
            };
            driver::compile(
                CompileRequest::new(request.options)
                    .with_requested_product(requested_product)
                    .with_artifact_policy(request.artifact_policy),
            )
        })
    }
}

/// Execute one typed production compiler request.
pub fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    Compiler::new().compile(request)
}

/// Explicitly test-only compiler seam for fixture controls.
#[doc(hidden)]
pub fn compile_harness(request: CompileHarnessRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    Compiler::new().compile_harness(request)
}
