//! The production compiler coordinator.
//!
//! This file intentionally declares only [`Compiler`]. Request and
//! host-execution infrastructure live beneath it; language and target semantics
//! belong to the stage crates the coordinator invokes.

use psi_diagnostics::Diagnostic;

mod driver;
pub(crate) mod execution;
mod native_checked;
mod optimization_rollback;
mod options;
pub(crate) use omega_compilation_report as report;
mod request;

pub use native_checked::NativeCompilationWithCheckedReceipt;
pub use omega_trust_model::{TrustAdmission, TrustAdmissionSettlement};
pub use optimization_rollback::{OptimizationRollback, OptimizationRollbackInputError};
pub use options::{ArtifactEmissionPolicy, CompileOptions};
pub use report::{
    CompileOutputKind, CompileReport, ExecutablePublicationDestination,
    ExecutablePublicationReceipt, OptimizationRollbackReceipt, ProductionArtifactIdentity,
    ProductionCompilationManifest, ProductionCompilationManifestIdentity,
    ProductionCompilationSubject, RetainedNativeArtifact,
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

    /// Compile one native artifact while retaining the exact checked program
    /// consumed by the same invocation.
    pub fn compile_native_with_checked_receipt(
        self,
        request: CompileRequest,
    ) -> Result<NativeCompilationWithCheckedReceipt, Vec<Diagnostic>> {
        execution::run_on_compile_thread(move || {
            driver::compile_native_with_checked_receipt(request)
        })
    }
}

/// Execute one typed production compiler request.
pub fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    Compiler::new().compile(request)
}

/// Compile one native artifact while retaining the exact checked program
/// consumed by the same invocation.
pub fn compile_native_with_checked_receipt(
    request: CompileRequest,
) -> Result<NativeCompilationWithCheckedReceipt, Vec<Diagnostic>> {
    Compiler::new().compile_native_with_checked_receipt(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_native_api_rejects_other_products_before_source_access() {
        for product in [
            RequestedCompileProduct::Check,
            RequestedCompileProduct::TerminalArtifact,
        ] {
            let request = CompileRequest::new(CompileOptions {
                root_path: "missing-checked-native-api-source.omg".into(),
                build_dir: None,
                target_name: None,
            })
            .with_requested_product(product);
            let diagnostics = compile_native_with_checked_receipt(request)
                .expect_err("checked native API must reject non-native products");
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(
                diagnostics[0].message,
                format!(
                    "checked native compilation requires NativeArtifact production; received {product:?}"
                )
            );
        }
    }
}
