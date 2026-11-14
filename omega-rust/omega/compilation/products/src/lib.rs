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
`CompileReport` records which payload a compilation produced and validates its
custody joins over payload-specific variants rather than optional fields. Report
construction belongs to `CompileReportSettlement`, while output path selection
and atomic filesystem installation belong exclusively to
`publication::publish_compilation`. Native evidence and image receipt
construction remain with `native_artifact`, and filesystem staging mechanics
remain with `platform_custody`.
*/

pub use terminal_psi_to_abstract_operations::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
pub use terminal_psi_to_abstract_operations::{
    decode_installation_assumptions, encode_installation_assumptions,
};

mod checked_compilation;
mod compilation_products;
mod compile_outcomes;
pub mod compile_request;
mod evaluated_build;
mod independent_component;
mod package_build;
mod prepared_build;
pub use typed_trees::ProductSourceIdentity;
mod prepared_source;
mod provider_instances;
pub use provider_instances::SelectedProviderSource;
pub mod publication;
mod terminal_publication;
mod test_products;
pub use terminal_publication::{ProductRoster, ProductTerminalPublication};
pub use test_products::{
    PendingTestProduct, TestFilesystem, TestRootIdentity, TestRosterEntry, TestRosterStatus,
};

pub use checked_compilation::{
    CheckedCompilation, CheckedCompilationInputs, CheckedProductReport, CompileReportSettlement,
    ProductReportPlan, ProductReportRequest,
};
pub use compilation_products::{
    CompileOutputKind, CompileReport, PendingNativeProduct, RetainedNativeProgram,
    RetainedNativeProgramIdentity,
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
    BuildAdmission, BuildConfig, BuildConstants, ComputedBuildConfig, EvaluatedBuild,
    IndependentComponentDiscovery, IndependentComponentSelection, ProviderDeployment,
    ServiceAuditRegistration,
};
pub use independent_component::{IndependentComponentProduct, SelectedIndependentComponent};
pub use native_artifact::NativeArtifact as RetainedNativeArtifact;
pub use native_artifact::{
    ExecutableContainerDigest, ExecutableInstallationEvidenceDigest, ExecutablePublicationReceipt,
    NativePublicationCertificateDigest, NativePublicationEvidenceDigest,
    executable_installation_evidence_digest,
};
pub use native_artifact::{NativePackagePublicationReceipt, PackagePublicationComponent};
pub use native_artifact::{TerminalRuntimeHostProvision, TerminalRuntimeProvisions};
pub use optimization_core::OptimizationRollbackReceipt;
pub use package_build::PreparedPackageBuild;
pub use package_compilation::RetainedBuildOutputs;
pub use prepared_build::{
    BuiltTypedProgram, CheckedProductSource, PreparedBuild, PreparedProductSource,
    SettledBuildSource, SettledPackageBuild,
};
pub use prepared_source::{
    DeferredProductCompilation, PreparedCheckedSource, PreparedSourceFrontier, SourcePreparation,
    TypedCompilationFrontier,
};
pub use terminal_installation::PrivateEntryRamInitialization;
pub use terminal_psi_to_abstract_operations::{
    RetainedTerminalArtifact, TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact,
    TerminalCompilerBuiltinProposal, TerminalIeeeFloatComparisonOccurrenceProposal,
    TerminalIntegerComparisonOccurrenceProposal, TerminalNativeBoundaryApplications,
    TerminalNativeOccurrences, TerminalNativeProviders, TerminalNativeRealizationProposal,
};
