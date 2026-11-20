//! The result of a checked Build activation, including the declaration requests it issued.
//! Package review borrows the same accepted source and the same execution observations.
#![forbid(unsafe_code)]

mod configuration;
mod failure;
pub use configuration::{BuildConfig, ComputedBuildConfig};
pub use failure::CompileFailure;

#[derive(Clone)]
pub struct EvaluatedBuild {
    pub configuration: std::sync::Arc<ComputedBuildConfig>,
    pub restricted_build_requests: Vec<build_consent::RestrictedBuildRequest>,
    pub base_source_consumption_commitment:
        Option<source_provenance::PackageSourceConsumptionCommitment>,
    pub constants: BuildConstants,
    pub subject_validation: BuildSubjectValidation,
    /// Costs of this Build context, not materialization evidence for Product.
    pub source_completion_accounting: source_completion_accounting::SourceCompletionAccounting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildSubjectValidation {
    EntryOnly,
    Checked,
    AwaitingOwner,
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
