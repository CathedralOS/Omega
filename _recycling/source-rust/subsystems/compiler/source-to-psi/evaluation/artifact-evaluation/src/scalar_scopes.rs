use build_scalar_evaluation::CanonicalScalarEvaluationReceipt;
pub use build_scalar_evaluation::{
    ScalarApplicationCompletion, ScalarScopeEvaluation, prepare_scalar_applications,
};
use semantic_vocabulary::ScalarApplicationId;

pub fn complete_scalar_applications<'scope>(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: impl Into<Option<&'scope str>>,
    deployment_scopes: Option<&[String]>,
    requested: &[ScalarApplicationId],
    sponsor: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    accounting: &mut impl Extend<CanonicalScalarEvaluationReceipt>,
    select: impl FnMut(Option<&str>) -> Result<ScalarScopeEvaluation, String>,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    prepare_scalar_applications(
        artifact,
        primary,
        deployment_scopes,
        requested,
        sponsor,
        accounting,
        select,
    )
    .map(ScalarApplicationCompletion::into_admitted)
}
