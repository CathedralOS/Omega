//! Prepare and extend scalar work for a published program using the target inputs that produced each receipt.
use crate::execution::TargetImportRealizations;
use crate::receipts::CanonicalScalarEvaluationReceipt;
use psi_scalar_applications::{ScalarApplicationClosure, ScalarApplicationObservations};
use semantic_vocabulary::ScalarApplicationId;
use terminal_interpreter::TerminalExecutionResult;

pub struct ScalarScopeEvaluation {
    pub application_roots: Option<Vec<terminal_psi::ProofDeclarationId>>,
    /// No deployment means ordinary Psi evaluation, not host selection.
    /// Layout facts must be absent as well; target-only operations then refuse
    /// at their actual demand rather than suppressing pure conditions.
    pub targets: Option<psi_target_layouts::TerminalTargetContext>,
    pub providers: psi_provider_selections::AdmittedProviderSelections,
    pub address_model: Result<Option<semantic_vocabulary::ScalarAddressModel>, String>,
    pub carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    pub layout_target: Option<psi_target_layouts::LayoutTargetFacts>,
    /// `None` leaves every bodyless import unavailable to evaluation.
    pub import_realizations: Option<Box<dyn TargetImportRealizations>>,
}

/// A completed Product scope and the actual target execution inputs that
/// produced it. This owner is invocation-local; saved artifacts carry no cache.
/// Native completion may add roots and demands, but cannot swap its providers,
/// import adapters, physical layout, address model or sponsored work account.
pub struct ScalarApplicationCompletion {
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: Option<String>,
    completed: psi_scalar_applications::AdmittedScalarApplications,
    scopes: Vec<ScalarScopeEvaluation>,
    sponsor: Option<build_model::accounting::EvaluationInvocationAccount>,
}

impl ScalarApplicationCompletion {
    pub fn artifact(&self) -> &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact> {
        &self.artifact
    }

    pub fn primary_target(&self) -> Option<&str> {
        self.primary.as_deref()
    }

    pub fn admitted(&self) -> &psi_scalar_applications::AdmittedScalarApplications {
        &self.completed
    }

    pub fn into_admitted(self) -> psi_scalar_applications::AdmittedScalarApplications {
        self.completed
    }

    pub fn extend(
        self,
        deployment_scopes: Option<&[String]>,
        roots: &[(String, Vec<terminal_psi::ProofDeclarationId>)],
        requested: &[ScalarApplicationId],
        accounting: &mut impl Extend<CanonicalScalarEvaluationReceipt>,
    ) -> Result<psi_scalar_applications::AdmittedScalarApplications, String> {
        if roots.iter().any(|(target, _)| {
            !self.scopes.iter().any(|scope| {
                scope
                    .targets
                    .as_ref()
                    .is_some_and(|selected| selected.target() == target)
            })
        }) {
            return Err("scalar completion extension names another authored target".into());
        }
        let mut scopes = self.scopes.into_iter().map(Some).collect::<Vec<_>>();
        let mut records = Vec::new();
        let mut completed_scopes = Vec::new();
        let result = complete_scopes(
            &self.artifact,
            self.primary.as_deref(),
            deployment_scopes,
            requested,
            self.sponsor.as_ref(),
            &mut records,
            &mut completed_scopes,
            Some(&self.completed),
            &mut |target| {
                let position = scopes
                    .iter()
                    .position(|scope| {
                        scope.as_ref().is_some_and(|scope| {
                            scope.targets.as_ref().map(|selected| selected.target()) == target
                        })
                    })
                    .ok_or("scalar completion extension has no retained target inputs")?;
                let mut selected = scopes[position].take().expect("selected retained scope");
                if let Some((_, declarations)) = roots
                    .iter()
                    .find(|(scope, _)| Some(scope.as_str()) == target)
                {
                    selected.application_roots = Some(declarations.clone());
                }
                Ok(selected)
            },
        );
        accounting.extend(records);
        result
    }
}

pub fn prepare_scalar_applications<'scope>(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: impl Into<Option<&'scope str>>,
    deployment_scopes: Option<&[String]>,
    requested: &[ScalarApplicationId],
    sponsor: Option<&build_model::accounting::EvaluationInvocationAccount>,
    accounting: &mut impl Extend<CanonicalScalarEvaluationReceipt>,
    mut select: impl FnMut(Option<&str>) -> Result<ScalarScopeEvaluation, String>,
) -> Result<ScalarApplicationCompletion, String> {
    let primary = primary.into();
    let mut records = Vec::new();
    let mut scopes = Vec::new();
    let completed = complete_scopes(
        artifact,
        primary,
        deployment_scopes,
        requested,
        sponsor,
        &mut records,
        &mut scopes,
        None,
        &mut select,
    );
    accounting.extend(records);
    Ok(ScalarApplicationCompletion {
        artifact: std::sync::Arc::clone(artifact),
        primary: primary.map(str::to_owned),
        completed: completed?,
        scopes,
        sponsor: sponsor.cloned(),
    })
}

fn complete_scopes(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: Option<&str>,
    deployment_scopes: Option<&[String]>,
    requested: &[ScalarApplicationId],
    sponsor: Option<&build_model::accounting::EvaluationInvocationAccount>,
    records: &mut Vec<CanonicalScalarEvaluationReceipt>,
    completed_scopes: &mut Vec<ScalarScopeEvaluation>,
    previous: Option<&psi_scalar_applications::AdmittedScalarApplications>,
    select: &mut impl FnMut(Option<&str>) -> Result<ScalarScopeEvaluation, String>,
) -> Result<psi_scalar_applications::AdmittedScalarApplications, String> {
    let mut closure = ScalarApplicationClosure::new(artifact.view(), primary);
    if let Some(scopes) = deployment_scopes {
        closure = closure.with_deployment_scopes(scopes);
    }
    let scopes = closure
        .scopes()
        .map(|scope| scope.map(str::to_owned))
        .collect::<Vec<_>>();
    for scope in scopes {
        let scope = scope.as_deref();
        let selected = select(scope)?;
        if selected.targets.as_ref().map(|targets| targets.target()) != scope {
            return Err("scalar evaluation selected a different authored scope".into());
        }
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            selected.targets.as_ref(),
            &selected.providers,
            selected.layout_target.as_ref(),
        )?
        .with_target_carriers(selected.carrier_layout)?
        .with_application_roots(selected.application_roots.as_deref())?;
        if let Some(previous) = previous {
            observations = observations.with_completed_scope(previous, &selected.address_model)?;
        }
        let demand = if scope == primary { requested } else { &[] };
        let input = crate::receipts::CanonicalScalarScope::capture(
            artifact,
            scope,
            &selected.providers,
            selected.carrier_layout,
        )?;
        let (conditions, executions) = crate::execution::evaluate_accounted_scope(
            &mut observations,
            primary,
            demand,
            selected
                .address_model
                .as_ref()
                .ok()
                .and_then(Option::as_ref),
            selected.import_realizations.as_deref(),
            sponsor,
            crate::receipts::PURE_PSI_WORK_BUDGET,
        );
        for execution in executions {
            let outcome = match execution.outcome {
                Ok(value) => crate::receipts::PureTerminalEvaluationOutcome::Returned {
                    value: TerminalExecutionResult::Scalar(value),
                },
                Err(reason) => crate::receipts::PureTerminalEvaluationOutcome::Failed { reason },
            };
            records.push(CanonicalScalarEvaluationReceipt::new(
                std::sync::Arc::clone(&input),
                execution.application,
                execution.values,
                execution.captures,
                outcome,
                execution.usage,
            ));
        }
        let conditions = conditions?;
        closure.close_scope(observations, conditions, selected.address_model.clone())?;
        completed_scopes.push(selected);
    }
    closure.finish()
}
