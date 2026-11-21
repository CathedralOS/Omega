//! Check or publish Omega source as Psi. Each request below names its stopping
//! point or entry. The source driver owns the numbered transformations.

#![forbid(unsafe_code)]

mod driver;

pub use compile_product::ProductDeclarations;
pub use driver::compile_build::{
    BuildInvocationJudgment, BuildPreparationSite, BuildStage, BuildStageSource,
    CheckedBuildInvocation, PendingBuildImport, PreparedBuildInvocation, ProductDeclarationSource,
    TypedBuildInterval, checked_build_stack, locate_build_suspending_call,
};
pub use driver::package_handoff::{PackageHandoffDeclarations, package_handoff_declarations};
pub use driver::parse_source_batch::{
    ParsedSourceBatch, ParsedSourceSummary, RetainedSourceParse, SourceDeclarations, SourceImport,
    SourceInput, SourceItems, SourceSyntax,
};
pub use driver::stage_sources::ParsedStageSources;
pub use driver::{compile_product, compile_time_invocations, registered_tests};
pub use representations::{PublishedTargetProgram, RuntimePermissionDeclarations, publication};

pub mod representations;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use artifacts::compile_timings::CompileTimings;
pub use driver::prepare_typed_source::SourcePreparationFailure;
pub use driver::resolve_and_type::{SourceExtensionRequest, SourcePreparation};
pub use source_declarations::source_selections::RootBinding;
pub use typed_trees_to_prepared_typed_trees::WireCompatibilityDemand;

use representations::{declaration_selection, source_inputs, source_state, source_symbols};

use compile_product::{ProductEntry, ProductSourceInputs};
use diagnostics::Diagnostic;
use driver::resolve_and_type::SourceTypingCheckpoint;
use std::sync::Arc;

pub type PublishedPsi = Arc<terminal_codec::CanonicalTerminalArtifact>;

/// Captured files include generated source from earlier stages.
/// Bindings belong to this compilation's selected packages and source identities.
pub struct StageSource<'a> {
    /// Source requests own the parser tables. Boxing them keeps saved Psi requests small.
    pub parsed: Box<ParsedStageSources>,
    pub product: ProductSourceInputs<'a>,
}

/// Return the source judgment's advisories as soon as the Product is checked.
pub fn check(source: StageSource<'_>) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
    let (product, resolved) = source.resolve()?;
    let checked = product.judge(resolved, ProductEntry::BuildBindings)?;
    Ok(checked
        .terminal_publication
        .accepted()?
        .advisories()
        .to_vec())
}

pub fn compile(
    source: StageSource<'_>,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<PublishedPsi, Vec<Diagnostic>> {
    driver::publish_stage(source, ProductEntry::BuildBindings, advisories)
        .map(|(artifact, _)| artifact)
}

pub fn compile_for_target(
    source: StageSource<'_>,
    target: &str,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<PublishedTargetProgram, Vec<Diagnostic>> {
    if !source.product.calling_targets.contains(&target) {
        return Err(vec![Diagnostic::error(format!(
            "source program did not select deployment target `{target}`"
        ))]);
    }
    let subject = source.product.subject;
    let (artifact, checked) =
        driver::publish_stage(source, ProductEntry::BuildBindings, advisories)?;
    driver::published_target(artifact, checked, target, subject)
}

pub fn inspect(source: StageSource<'_>, machine: &str) -> Result<PublishedPsi, Vec<Diagnostic>> {
    driver::publish_stage(source, ProductEntry::Inspection(machine), None)
        .map(|(artifact, _)| artifact)
}
