pub use build_scalar_evaluation::{AdmittedImportRealizations, TargetImportRealizations};
use semantic_vocabulary::ScalarApplicationId;
use terminal_installation::{
    ScalarApplicationClosure, ScalarApplicationObservations, ScalarConditionSelection,
};
use terminal_work::WorkMeter;

pub fn evaluate_scalar_applications<'artifact, 'scope>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: impl Into<Option<&'scope str>>,
    requested: &[ScalarApplicationId],
    meter: &mut WorkMeter,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    build_scalar_evaluation::evaluate_scalar_applications(
        observations,
        value_scope,
        requested,
        None,
        meter,
    )
}

pub fn evaluate_scalar_applications_with_address_model<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: &str,
    requested: &[ScalarApplicationId],
    address_model: Option<&terminal_installation::ScalarAddressModel>,
    meter: &mut WorkMeter,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    build_scalar_evaluation::evaluate_scalar_applications(
        observations,
        value_scope,
        requested,
        address_model,
        meter,
    )
}

/// The deployment target one build-time invocation runs under.
///
/// An invocation artifact can retain scalar conditions: ranges, predicates and
/// completions that checking deferred because their values depend on the
/// target. Terminal execution refuses to start until those are admitted, and
/// admitting them needs the target's layout facts, carriers and address model.
/// Those are target-owner knowledge, so the caller that names the target
/// supplies them here; a target without layout facts cannot discharge any
/// condition and the invocation refuses.
#[derive(Clone, Copy)]
pub struct InvocationTarget<'target> {
    pub name: Option<&'target str>,
    pub layout_target: Option<&'target terminal_installation::LayoutTargetFacts>,
    pub carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    pub address_model: Option<&'target terminal_installation::ScalarAddressModel>,
}

/// Discharge the scalar conditions retained for one invocation entry.
///
/// We close the single scope of `target` rooted at the entry's checked proof
/// declaration, which is the invocation-scoped closure calling policies
/// already use, and evaluate every application the entry demands on Terminal
/// Psi under `meter`. Each condition selected for that root is checked as soon
/// as its observations are ready, and a false one fails the invocation; none
/// is skipped.
pub fn complete_invocation_scalar_applications<'artifact>(
    artifact: terminal_codec::TerminalExecutionView<'artifact>,
    target: InvocationTarget<'_>,
    selected: &'artifact terminal_installation::TerminalTargetContext,
    providers: &'artifact terminal_installation::AdmittedProviderSelections,
    declaration: terminal_psi::ProofDeclarationId,
    requested: &[ScalarApplicationId],
    meter: &mut WorkMeter,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    let facts = target
        .layout_target
        .ok_or("invocation target has no immutable layout catalogue")?;
    let name = target
        .name
        .ok_or("scalar observations require an actual selected target")?;
    let mut closure =
        ScalarApplicationClosure::for_invocation(artifact, Some(name), &[declaration]);
    let mut observations =
        ScalarApplicationObservations::new(artifact, Some(selected), providers, Some(facts))?
            .with_target_carriers(target.carrier_layout)?
            .with_invocation_roots(&[declaration])?;
    let conditions = build_scalar_evaluation::evaluate_scalar_applications(
        &mut observations,
        Some(name),
        requested,
        target.address_model,
        meter,
    )?;
    closure.close_scope(observations, conditions, Ok(target.address_model.cloned()))?;
    closure.finish()
}
