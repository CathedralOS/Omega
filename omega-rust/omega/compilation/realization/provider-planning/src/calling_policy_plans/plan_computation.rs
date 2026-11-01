//! Computing calling plans: evaluating the authored policy at build time,
//! materializing the boundary signature from the ABI result, and validating
//! the plan the policy produced.

use crate::calling_policy_plans::callback_bindings::validate_retained_callback_binders;
use crate::calling_policy_plans::{BoundaryCallingPlanRealization, MaterializedBoundarySignature};
use build_time_evaluation::BuildTimeProgram;
use calling_conventions::ValidatedBoundaryEntryPlan;
use calling_conventions::callback_nominal_report_fingerprint;
use diagnostics::Diagnostic;
use representation_planning::OpaqueRepresentationSelection;

/// Prepare Source invocation admission and exact review coordinates only.
/// Canonical publication owns declaration storage; target activation reads it.
pub fn prepare_boundary_calling_plans<'checked>(
    checked: &'checked checked_trees::CheckedTrees,
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    target_profile: target::TargetProfile,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<PreparedBoundaryCallingPlans<'checked>, Vec<Diagnostic>> {
    if !std::ptr::eq(&*checked.typed, services.context().typed)
        || services.context().scope != source::DependencyScope::Product
    {
        return Err(vec![Diagnostic::error(
            "calling plans require the exact Product source owner's prepared signature catalog",
        )]);
    }
    let typed = &checked.typed;
    let signatures = std::sync::Arc::clone(typed.boundary_signatures());
    if !std::ptr::eq(signatures.as_ref(), services.signatures()) {
        return Err(vec![Diagnostic::error(
            "calling admission has a different frozen declaration census",
        )]);
    }
    let calling = signatures.calling();
    let mut pending = Vec::new();
    for (request_ordinal, request) in calling.requests().enumerate() {
        for (instance_ordinal, instance) in calling.instances(request)?.iter().enumerate() {
            let policy = instance.policy.as_ref().map_err(|reason| {
                vec![
                    Diagnostic::error(format!(
                        "boundary trait `{}` cannot evaluate Calling<Policy>: {reason}",
                        request.name
                    ))
                    .with_source_span(request.span),
                ]
            })?;
            let policy = typed.machine(*policy).ok_or_else(|| {
                vec![
                    Diagnostic::error("prepared calling policy lost its exact machine")
                        .with_source_span(request.span),
                ]
            })?;
            let policy_machine = typed.machine_for_target(policy, target_profile.target_name()).ok_or_else(|| vec![
                Diagnostic::error(format!("boundary trait `{}` cannot evaluate Calling<Policy>: calling policy realization `{}` has no body for target `{}`", request.name, policy.name, target_profile.target_name())).with_source_span(request.span)
            ])?;
            for application in calling.signatures(instance) {
                if let Some(reason) = &application.abi_refusal {
                    return Err(vec![
                        Diagnostic::error(reason).with_source_span(request.span),
                    ]);
                }
                let declaration = signatures.applied_declaration(application).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "calling admission lost its exact prepared signature declaration",
                    )]
                })?;
                for &signature in &application.abi_signatures {
                    pending.push(PreparedCallingPolicy {
                        request_ordinal,
                        instance_ordinal,
                        signature,
                        boundary_trait: request.boundary,
                        boundary_arguments: calling.arguments(instance).to_vec(),
                        argument_owner: instance.argument_owner,
                        requirement_machine: declaration.symbol,
                        policy_machine: policy_machine.name.as_str().to_owned(),
                        policy_symbol: policy_machine.symbol,
                        relationship_span: request.span,
                        admission: Ok(()),
                    });
                }
            }
        }
    }
    if !pending.is_empty() {
        // Source admission remains independent of geometry and policy results.
        // Its exact selected-body map must agree with the canonical activation.
        let machine_bodies = typed
            .target_machine_families
            .iter()
            .filter_map(|family| {
                typed
                    .target_family_member(family.canonical, target_profile.target_name())
                    .map(|body| build_time_evaluation::SelectedBuildTimeMachineBody {
                        family: family.canonical,
                        body: body.symbol,
                    })
            })
            .collect::<Vec<_>>();
        let admission = build_time_evaluation::BuildTimeAdmissionPlan::infer(
            typed,
            package_inputs.map(|inputs| {
                std::sync::Arc::new(inputs.clone())
                    as std::sync::Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
            }),
        )
        .with_selected_machine_bodies(typed, &machine_bodies)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        for policy in &mut pending {
            policy.admission = admission
                .admitted_program_machine_closure_symbols(
                    BuildTimeProgram::Checked(checked),
                    policy.policy_symbol,
                    build_time_evaluation::BuildTimeInvocationCustody::Source(
                        policy.relationship_span,
                    ),
                )
                .map(|_closure| ());
        }
    }
    Ok(PreparedBoundaryCallingPlans {
        checked,
        signatures,
        pending,
        target: target_profile,
        representations: opaque_representation_selections.to_vec(),
    })
}

pub struct PreparedBoundaryCallingPlans<'checked> {
    checked: &'checked checked_trees::CheckedTrees,
    signatures: std::sync::Arc<
        checked_trees::typed_trees::service_declarations::signatures::PreparedBoundarySignatures,
    >,
    pending: Vec<PreparedCallingPolicy>,
    target: target::TargetProfile,
    representations: Vec<OpaqueRepresentationSelection>,
}

struct PreparedCallingPolicy {
    request_ordinal: usize,
    instance_ordinal: usize,
    signature: semantic_vocabulary::CallingSignatureId,
    boundary_trait: symbols::SymbolHandle,
    boundary_arguments: Vec<checked_trees::typed_trees::types::TypeReferenceHandle>,
    argument_owner: symbols::SymbolHandle,
    requirement_machine: symbols::SymbolHandle,
    policy_machine: String,
    policy_symbol: symbols::SymbolHandle,
    relationship_span: source::SourceSpan,
    admission: Result<(), String>,
}

impl PreparedBoundaryCallingPlans<'_> {
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn evaluate(
        self,
        code: Option<&super::CallingCatalog>,
    ) -> Result<Vec<BoundaryCallingPlanRealization>, Vec<Diagnostic>> {
        let mut evaluated = Vec::with_capacity(self.pending.len());
        for policy in self.pending {
            let diagnostic = |reason: String| {
                vec![Diagnostic::error(reason).with_source_span(policy.relationship_span)]
            };
            policy.admission.map_err(&diagnostic)?;
            let code = code.ok_or_else(|| {
                diagnostic("calling policies have no published Terminal code".into())
            })?;
            code.require_owner(self.checked).map_err(&diagnostic)?;
            let module = code.artifact.semantic_module();
            let request = module
                .calling_policy_requests
                .get(policy.request_ordinal)
                .ok_or_else(|| {
                    diagnostic("calling policy lost its exact canonical request".into())
                })?;
            let owner = *request
                .declaration
                .as_ref()
                .map_err(|reason| diagnostic(reason.clone()))?;
            if module.identities.text(owner)
                != self
                    .checked
                    .typed
                    .normalized_hermetic_symbol_identity(policy.boundary_trait)
                    .ok()
                    .as_deref()
            {
                return Err(diagnostic(
                    "calling policy request differs from its original source owner".into(),
                ));
            }
            let instance = request
                .instances
                .as_ref()
                .map_err(|reasons| diagnostic(reasons.join("; ")))?
                .get(policy.instance_ordinal)
                .ok_or_else(|| {
                    diagnostic("calling policy lost its exact canonical instance".into())
                })?;
            let applications = instance
                .signatures
                .as_ref()
                .map_err(|reason| diagnostic(reason.clone()))?;
            if !applications.contains(&policy.signature) {
                return Err(diagnostic(
                    "calling policy instance does not contain its exact declaration ABI".into(),
                ));
            }
            let callable = *instance
                .policy
                .as_ref()
                .map_err(|reason| diagnostic(reason.clone()))?;
            let (canonical, validated) = code
                .evaluate(
                    policy.policy_symbol,
                    callable,
                    self.target,
                    policy.signature,
                )
                .map_err(&diagnostic)?;
            let callback_context_closed = !canonical.policy.callbacks.binders.is_empty()
                || !canonical.policy.callbacks.demands.is_empty();
            let declaration = self
                .signatures
                .declarations()
                .find(|row| row.symbol == policy.requirement_machine)
                .ok_or_else(|| {
                    diagnostic("calling review lost its original requirement declaration".into())
                })?;
            let signature = super::signature_source::attach(
                canonical,
                module,
                declaration,
                &self.checked.typed,
                &code.source_records,
                &self.representations,
                self.target,
            )
            .map_err(&diagnostic)?;
            let (report_fingerprint, commitment) =
                boundary_plan_application_identity(&signature, &validated).map_err(&diagnostic)?;
            evaluated.push(BoundaryCallingPlanRealization {
                declaration: policy.signature,
                boundary_trait: policy.boundary_trait,
                boundary_arguments: policy.boundary_arguments,
                argument_owner: policy.argument_owner,
                requirement_machine: policy.requirement_machine,
                report_fingerprint,
                commitment: effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                    commitment,
                ),
                boundary_entry_plan: validated.plan().clone(),
                validated_boundary_entry_plan: validated,
                callback_binders: signature.callback_binders.clone(),
                callback_demands: signature.callback_demands.clone(),
                callback_context_closed,
                native_parameters: signature.native_parameters.clone(),
                materialized_signature: signature,
                policy_machine: policy.policy_machine,
                relationship_span: policy.relationship_span,
            });
        }
        for realization in &evaluated {
            validate_retained_callback_binders(realization)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        }
        Ok(evaluated)
    }
}

pub(crate) fn boundary_plan_application_identity(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
) -> Result<(u64, [u8; 32]), String> {
    let application = publish_materialized_application(signature, validated)?;
    Ok((application.report_fingerprint, application.commitment))
}

/// Both authored and default policies publish the complete source-free C shape
/// together with its validated plan. Returning only a bare plan discarded the
/// opaque representation graph before the runtime could admit a typed host.
fn publish_materialized_application(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
) -> Result<calling_conventions::BoundaryCallingApplication, String> {
    let mut application = super::callback_bindings::source_free_application(signature, validated)?;
    application.commitment = application.recomputed_commitment()?;
    application.report_fingerprint = callback_nominal_report_fingerprint(
        b"omega.boundary-plan-application-report.v3",
        &[&application.commitment],
    );
    Ok(application)
}
