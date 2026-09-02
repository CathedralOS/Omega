//! Omega compilation coordination.
//! The rooted API is [`Compiler`]. Domain models are imported from their
//! owning subsystem crates rather than republished here.
mod compiler;
mod pipeline;

pub use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileOutputKind, CompileReport, CompileRequest,
    Compiler, ExactTargetCompileOutcome, ExecutablePublicationDestination,
    ExecutablePublicationReceipt, ExplicitTargetSet, FinalRealizationEvidenceError,
    MultiTargetCompileOutcomes, MultiTargetCompileRequest, OptimizationRollback,
    OptimizationRollbackInputError, OptimizationRollbackReceipt, ProductionArtifactIdentity,
    ProductionCompilationManifest, ProductionCompilationManifestIdentity,
    ProductionCompilationSubject, RequestedCompileProduct, RetainedNativeArtifact,
    SourceEvaluatedImportSettlement, TrustAdmission, TrustAdmissionSettlement, compile,
    compile_targets, realize_retained_terminal_artifact_with_source_evaluated_imports,
    realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy,
    retained_terminal_report_from_checked_package,
};
pub use pipeline::checked_entry::{
    CheckedCompilation, compile_to_checked, compile_to_checked_with_packages,
    compile_to_checked_with_packages_and_replay_record,
    compile_to_checked_with_packages_in_build_dir,
    compile_to_checked_with_packages_in_sponsored_build_dir,
    compile_to_checked_with_packages_in_sponsored_build_session,
    compile_to_checked_with_replay_record,
};
pub use pipeline::x86_fma_plan_association::CheckedX86ScalarFmaPlanAssociation;
pub(crate) use psi_source as source;
pub(crate) use psi_source_files_to_tokens as lexer;
pub(crate) use psi_tokens_to_syntax_trees as parser;
