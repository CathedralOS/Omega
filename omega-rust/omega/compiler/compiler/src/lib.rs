//! Omega compilation coordination rooted at [`Compiler`]; domain models stay in their owners.
mod compiler;
mod pipeline;

pub use compiler::{
    ArtifactEmissionPolicy, CheckedAdmission, CompileOptions, CompileOutcomes, CompileOutputKind,
    CompileReport, CompileRequest, CompileTargetOutcome, Compiler, ExecutablePublicationReceipt,
    ExplicitTargetSet, FinalRealizationEvidenceError, OptimizationRollback,
    OptimizationRollbackInputError, OptimizationRollbackReceipt, ProductionArtifactIdentity,
    ProductionCompilationManifest, ProductionCompilationManifestIdentity,
    ProductionCompilationSubject, RequestedCompileProduct, RetainedNativeArtifact,
    RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement, TargetCompileConfiguration,
    TrustAdmission, TrustAdmissionSettlement, admit_checked_compilation, compile,
    realize_retained_native_artifact, retained_terminal_report_from_checked_package,
    validate_lowered_ieee_float_comparison_custody,
};
pub use pipeline::checked_entry::{CheckedCompilation, CheckedCompileRequest, compile_to_checked};
pub use pipeline::x86_fma_plan_association::CheckedX86ScalarFmaPlanAssociation;
pub(crate) use source_files_to_tokens as lexer;
pub(crate) use tokens_to_syntax_trees as parser;
