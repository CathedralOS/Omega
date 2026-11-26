//! The result of a checked Build activation, including the declaration requests it issued.
//! Its published programs expose their contracts beside the observations of the execution.
#![forbid(unsafe_code)]

mod configuration;
mod failure;
pub use configuration::{BuildConfig, ComputedBuildConfig, ExecutedBuildEntry};
pub use failure::CompileFailure;

/// The frozen package occurrence an executed registration asks the driver to check.
/// A preparation can select a library which its successor never forwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyValidationSubject {
    pub purpose: build_declarations::DependencyPurpose,
    pub inputs: package_compilation::PackageCompilationInputs,
    pub configuration: std::sync::Arc<ComputedBuildConfig>,
}

#[derive(Clone)]
pub struct EvaluatedBuild {
    /// Completed, independently admitted preparations in execution order.
    pub preparations: Vec<(
        build_declarations::BuildEntryPath,
        std::sync::Arc<ComputedBuildConfig>,
    )>,
    pub configuration: std::sync::Arc<ComputedBuildConfig>,
    pub restricted_build_requests: Vec<build_consent::RestrictedBuildRequest>,
    pub base_source_consumption_commitment:
        Option<source_provenance::PackageSourceConsumptionCommitment>,
    pub constants: BuildConstants,
    pub interpreter_providers: package_compilation::InterpretedProviderSelections,
    pub subject_validation: BuildSubjectValidation,
    pub dependency_validation: Vec<DependencyValidationSubject>,
    /// Costs of this Build context, not materialization evidence for Product.
    pub source_completion_accounting: source_completion_accounting::SourceCompletionAccounting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildSubjectValidation {
    EntryOnly,
    /// The whole subject was checked before Build ran, under a check-only owner.
    /// It is checked again under the owner the returned Build establishes.
    CheckedUnderProvisionalOwner,
    Checked,
}

/// What the Build admission's constants owe a Product frontier typed after
/// Build ran. That frontier holds the Build source again with its constants
/// unsettled, so whatever the admission evaluated has to be settled there
/// too. Both fields are read from the admission itself: the fold from what
/// it did, the names from the module it accepted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildConstants {
    /// The admission folded a constant under Build's own provider plans.
    pub folded_under_providers: bool,
    /// The constants the accepted Build module names
    /// (`AcceptedTerminalSource::constants_read`), each with its declaration
    /// and the value the judged source recorded for it.
    pub read: Vec<checked_program_to_psi::ConstantRead>,
}
