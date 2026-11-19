#![forbid(unsafe_code)]

/*
Compilation requests, retained stage products, content-addressed identities, and
publication receipts across the Psi-to-Omega boundary.

`compile_request` admits target destinations, invocation policy, and product
eligibility without executing a compiler phase. `compile_outcomes` retains
every target result in coordinator order and classifies each as the compiler
request's `Complete`, `Reject`, or `Incomplete` outcome. `prepared_source`
retains parsed and typed frontend checkpoints and selects reuse without
invoking assembly, resolution, typing, or Build evaluation. `prepared_build`
and `evaluated_build` retain completed typed Build continuations, normalized
settings, and observation summaries without depending on the evaluator.
`checked_compilation` associates accepted Terminal publication and settled execution
with exact typed package source custody, supplies their target projections, and reconstructs explicit trust
obligations through `trust_model`.
*/

/*
CompileReportSettlement joins a finished product to the source accounting,
timings, and completed Build outputs of the admitted compilation. Phase 10 owns
the direct and dynamically linked product states, checks receipts against those
products, and installs their files. The report publishes Build-output
companions after phase 10 completes the primary product.
*/

pub use installation_evidence::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
pub use published_runtime_input::RuntimeExecutionProvisions;
pub use verified_optimization_input::{
    decode_installation_assumptions, encode_installation_assumptions,
    recover_native_derivation_inputs,
};

mod checked_compilation;
mod published_compilation;
pub use portable_publication::{ExecutablePackageInputs, PublishedProgram};
pub use published_compilation::{PublishedCompilation, PublishedExecutableProgram};
mod compilation_products;
mod compile_outcomes;
mod request_admission;
pub mod compile_request;
mod evaluated_build;
mod independent_component;
mod package_build;
mod prepared_build;
pub use source_compiler::source_state::ProductSourceIdentity;
mod prepared_source;
mod program_entry;
pub use program_entry::PublishedNativeEntry;
mod provider_selection;
pub use provider_selection::{SelectedExecution, SelectedProviderResult};
mod provider_instances;
pub use provider_instances::SelectedProviderSource;
pub mod publication;
mod test_products;
pub use test_products::{
    PendingTestProduct, TestFilesystem, TestRootIdentity, TestRosterEntry, TestRosterStatus,
};

pub use backend_plan::{TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact};
pub use boundary_applications::{
    TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
};
pub use checked_compilation::{
    CheckedCompilation, CheckedCompilationInputs, CheckedProductReport, CompileReportSettlement,
    ProductReportPlan, ProductReportRequest,
};
pub use compilation_products::{
    CompileOutputKind, CompileReport, PendingNativeProduct, ProductScalarCompletion,
    RetainedNativeProgram, RetainedNativeProgramIdentity,
};
pub use compile_outcomes::{
    CheckedCompileOutcome, CompileFailure, CompileOutcomes, CompileTargetOutcome,
    CompilerRequestOutcome,
};
pub use compile_request::{
    CheckedCompileRequest, CheckedExecutionRequest, CompileOptions, CompileRequest,
    NativeImageDemand, NativeProductRequest, RequestedCompileProduct,
};
pub use evaluated_build::{
    BuildAdmission, BuildConfig, BuildConstants, BuildSubjectValidation, ComputedBuildConfig,
    EvaluatedBuild, IndependentComponentDiscovery, IndependentComponentSelection,
    ProviderDeployment, ServiceAuditRegistration,
};
pub use independent_component::{IndependentComponentProduct, SelectedIndependentComponent};
pub use optimization_requests::OptimizationRollbackReceipt;
pub use package_build::PreparedPackageBuild;
pub use package_compilation::RetainedBuildOutputs;
pub use prepared_build::{
    BuiltTypedProgram, CheckedProductSource, PreparedBuild, PreparedProductSource,
    SettledBuildSource, SettledPackageBuild,
};
pub use prepared_source::{
    DeferredProductCompilation, PreparedCheckedSource, PreparedSourceFrontier,
    TypedCompilationFrontier,
};
pub use product_publication::RetainedNativeArtifact;
pub use product_publication::{
    ExecutableContainerDigest, ExecutableInstallationEvidenceDigest, ExecutablePublicationReceipt,
    NativePublicationEvidenceDigest, NativePublicationInputsDigest,
    executable_installation_evidence_digest,
};
pub use product_publication::{NativePackagePublicationReceipt, PackagePublicationComponent};
pub use product_publication::{TerminalRuntimeHostProvision, TerminalRuntimeProvisions};
pub use provider_admission::provider_admission::TerminalCompilerBuiltinProposal;
pub use terminal_installation::PrivateEntryRamInitialization;
pub use verified_optimization_input::native_proposal::{
    TerminalNativeBoundaryApplications, TerminalNativeOccurrences, TerminalNativeProviders,
};
pub use verified_optimization_input::{
    RetainedTerminalArtifact, TerminalNativeRealizationProposal,
};

// The actual invocation records retained by source and failure reports.
pub use source_compiler::evaluation::SourceCompletionAccounting;
