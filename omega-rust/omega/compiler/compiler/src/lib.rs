//! Source checking, compilation, and retained-artifact realization.
mod compiler;
mod pipeline;

pub use assembled_syntax_to_checked_compilation::{
    CheckedCompilation, CheckedCompileRequest, OptimizationRollback,
    OptimizationRollbackInputError, PreparedCheckedSource, compile_to_checked,
};
pub use compilation_report::{
    CompileOutputKind, CompileReport, ExecutablePublicationReceipt, FinalRealizationEvidenceError,
    OptimizationRollbackReceipt, ProductionArtifactIdentity, ProductionCompilationManifest,
    ProductionCompilationManifestIdentity, ProductionCompilationSubject, RetainedNativeArtifact,
};
pub use compiler::admission::{CheckedAdmission, admit_checked_compilation};
pub use compiler::compile;
pub use compiler::options::{ArtifactEmissionPolicy, CompileOptions};
pub use compiler::package::retained_terminal_report_from_checked_package;
pub use compiler::request::{
    CompileOutcomes, CompileRequest, CompileTargetOutcome, ExplicitTargetSet,
    RequestedCompileProduct, TargetCompileConfiguration,
};
pub use compiler::terminal_native_realization::{
    RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement,
    realize_retained_native_artifact,
};
pub use compiler::terminal_product::validate_lowered_ieee_float_comparison_custody;
pub use trust_model::{TrustAdmission, TrustAdmissionSettlement};
