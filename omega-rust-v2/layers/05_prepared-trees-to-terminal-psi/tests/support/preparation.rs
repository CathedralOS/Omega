use omega_preparation::{PreparationFailure, PreparedTrees, prepare_source_package_diagnosed};
use omega_typed::TypeError;
use omega_typer::SourcePackage;

/// Stage token for an L3 refusal of a retype that consumed L4 answers. L3 owns
/// the refusal; the token records that L4 evaluated a demanded identity first.
pub const RESUMED_TYPING: &str = "L3-resumed";

/// The layer that owns a stage token's refusal, for recorded outcomes.
#[allow(dead_code, reason = "not every including test records owners")]
pub fn owner(stage: &'static str) -> &'static str {
    if stage == RESUMED_TYPING { "L3" } else { stage }
}

pub fn prepare<'text>(source: &SourcePackage<'text>) -> Result<PreparedTrees<'text>, (&'static str, TypeError)> {
    prepare_source_package_diagnosed(source).map_err(|failure| match failure {
        PreparationFailure::Typing(e) => ("L3", e),
        PreparationFailure::ResumedTyping(e) => (RESUMED_TYPING, e),
        PreparationFailure::Preparation(refusal) | PreparationFailure::MissingTarget(refusal) => ("L4", refusal.diagnostic),
    })
}
