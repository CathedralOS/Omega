//! Computing calling plans: evaluating the authored policy at build time,
//! materializing the boundary signature from the ABI result, and validating
//! the plan the policy produced.

use crate::calling_policy_plans::boundary_signatures::BoundaryOpaqueMovementGeometry;
use crate::calling_policy_plans::callback_bindings::validate_retained_callback_binders;
use crate::calling_policy_plans::{BoundaryCallingPlanRealization, MaterializedBoundarySignature};
use calling_conventions::ValidatedBoundaryEntryPlan;
use calling_conventions::callback_nominal_report_fingerprint;
use calling_conventions::{CallingPolicy, evaluate_ordinary_boundary_entry_plan};
use diagnostics::Diagnostic;
use std::sync::{Arc, Weak};

pub(crate) fn evaluate_default_calling_applications(
    program: &super::CallingProgram,
    declarations: &[semantic_vocabulary::CallingAbiIdentity],
    target: target::TargetProfile,
    policy: CallingPolicy,
    dispatch_only_parameter_count: usize,
) -> Result<Vec<calling_conventions::BoundaryCallingApplication>, String> {
    let mut applications = Vec::with_capacity(declarations.len());
    for declaration in declarations {
        let canonical = program.materialize_default(declaration, target)?;
        let signature = &canonical.policy;
        let geometry = &signature.geometry;
        let dispatch_only_parameter_count = u32::try_from(dispatch_only_parameter_count)
            .map_err(|_| "default dispatch prefix exceeds u32")?;
        let projection = calling_conventions::NativeCallProjection::new(
            signature.native_parameters()?,
            geometry.parameters.len(),
            dispatch_only_parameter_count,
        )?;
        let graph = calling_conventions::BoundaryValueGraph {
            shapes: &geometry.shapes,
            fields: &geometry.fields,
        };
        // Dispatch formals retain their public identities but do not occupy a
        // callee argument. Native-only callback lanes are never removed here.
        let mut parameters = Vec::new();
        for parameter in projection.parameters() {
            let shape = match parameter.shape {
                calling_conventions::BoundaryNativeParameterShape::Semantic(root) => {
                    graph.classify(root, policy)?
                }
                calling_conventions::BoundaryNativeParameterShape::TargetFunctionPointer {
                    byte_size,
                    alignment,
                } => calling_conventions::ValueShape::integer(u32::from(byte_size), alignment),
            };
            parameters.push(shape);
        }
        let classified = calling_conventions::CallSignature {
            parameters,
            result: geometry
                .result
                .map(|root| graph.classify(root, policy))
                .transpose()?,
        };
        let ordinary = evaluate_ordinary_boundary_entry_plan(policy, &classified)
            .map_err(|diagnostic| diagnostic.to_string())?;
        let validated =
            calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                ordinary.plan().clone(),
                &classified,
                &signature.callbacks,
            )
            .map_err(|diagnostic| diagnostic.to_string())?;
        let movement = BoundaryOpaqueMovementGeometry {
            shapes: &geometry.shapes,
            fields: &geometry.fields,
            parameters: &geometry.parameters,
            result: geometry.result,
            native_parameters: signature.native_parameters()?,
        };
        for representation in &canonical.opaque_uses {
            movement.movement(
                representation.shape_root,
                &validated,
                usize::try_from(dispatch_only_parameter_count)
                    .map_err(|_| "default dispatch prefix exceeds usize")?,
            )?;
        }
        applications.push(canonical.native_application(&validated, dispatch_only_parameter_count)?);
    }
    Ok(applications)
}
/// Follow canonical requests over owned original correspondence. Target choice
/// and helper selection need no Source owner, catalog, or signature traversal.
/// Invocation admission still precedes activation and deferred review attachment.
pub fn prepare_boundary_calling_plans(
    target_profile: target::TargetProfile,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
    code: &super::CallingProgram,
) -> Result<PreparedBoundaryCallingPlans, Vec<Diagnostic>> {
    let source = &code.source_preparation;
    code.require_owner(&source.owner)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let module = code.artifact.semantic_module();
    if module.calling_policy_requests.len() != source.requests.len() {
        return Err(vec![Diagnostic::error(
            "calling review lost its complete canonical request roster",
        )]);
    }
    let mut pending = Vec::new();
    for (request_ordinal, request) in module.calling_policy_requests.iter().enumerate() {
        let original = &source.requests[request_ordinal];
        let diagnostic =
            |reason: String| vec![Diagnostic::error(reason).with_source_span(original.span)];
        let original_instances = original.instances.as_ref().map_err(Clone::clone)?;
        let instances = request
            .instances
            .as_ref()
            .map_err(|reasons| diagnostic(reasons.join("; ")))?;
        if instances.len() != original_instances.len() {
            return Err(diagnostic(
                "calling review lost its exact canonical instance roster".into(),
            ));
        }
        for (instance_ordinal, instance) in instances.iter().enumerate() {
            let supplied = &original_instances[instance_ordinal];
            let policy_name = supplied.policy_name.as_ref().map_err(Clone::clone)?;
            let declaration = *instance
                .policy_declaration
                .as_ref()
                .map_err(|reason| diagnostic(reason.clone()))?;
            if module
                .scalar_applications
                .proof_scope
                .plan
                .declared_callable_for_target(declaration, target_profile.target_name())
                .is_none()
            {
                return Err(diagnostic(format!(
                    "boundary trait `{}` cannot evaluate Calling<Policy>: calling policy realization `{policy_name}` has no body for target `{}`",
                    request.display_name,
                    target_profile.target_name(),
                )));
            }
            let original_signatures = supplied.signatures.as_ref().map_err(Clone::clone)?;
            let signatures = instance
                .signatures
                .as_ref()
                .map_err(|reason| diagnostic(reason.clone()))?;
            if signatures.len() != original_signatures.len() {
                return Err(diagnostic(
                    "calling review lost its complete canonical ABI roster".into(),
                ));
            }
            // These are flattened ABI rows, not Source application ordinals:
            // one application may produce several exact declaration keys.
            for (&signature, original_signature) in signatures.iter().zip(original_signatures) {
                if signature != original_signature.signature {
                    return Err(diagnostic(
                        "calling review ABI order differs from its canonical instance".into(),
                    ));
                }
                pending.push(PreparedCallingPolicy {
                    request_ordinal,
                    instance_ordinal,
                    signature,
                    abi: original_signature.abi.clone(),
                    boundary_trait: original.boundary,
                    request_owner: original.identity.clone(),
                    boundary_arguments: supplied.arguments.clone(),
                    argument_owner: supplied.argument_owner,
                    requirement_machine: original_signature.requirement,
                    policy_machine: policy_name.clone(),
                    relationship_span: original.span,
                    requester: original.requester,
                    review: original_signature.review.clone(),
                });
            }
        }
    }
    let mut provider_declarations = Vec::new();
    if !pending.is_empty() {
        let roots = pending
            .iter()
            .map(|policy| {
                policy
                    .canonical_instance(code)
                    .and_then(|instance| instance.policy_declaration.clone())
                    .map_err(|reason| {
                        vec![Diagnostic::error(reason).with_source_span(policy.relationship_span)]
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        provider_declarations = super::selected_providers::select(target_profile, &roots, code)?;
    }
    Ok(PreparedBoundaryCallingPlans {
        source_owner: source.owner.clone(),
        pending,
        target: target_profile,
        providers: provider_declarations,
        authority: package_inputs.map(|inputs| {
            Arc::new(inputs.clone())
                as Arc<dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority>
        }),
    })
}

/// Source request and review addresses are complete before activation. This
/// recipe keeps no checked trees alive and can run after their owner is dropped.
pub struct PreparedBoundaryCallingPlans {
    source_owner: Weak<checked_trees::CheckedTrees>,
    pending: Vec<PreparedCallingPolicy>,
    target: target::TargetProfile,
    providers: Vec<backend_plan::TerminalTargetProviderPlan>,
    authority:
        Option<Arc<dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority>>,
}

struct PreparedCallingPolicy {
    request_ordinal: usize,
    instance_ordinal: usize,
    signature: semantic_vocabulary::CallingSignatureId,
    abi: Result<semantic_vocabulary::CallingAbiIdentity, String>,
    boundary_trait: symbols::SymbolHandle,
    request_owner: Result<String, String>,
    boundary_arguments: Vec<checked_trees::typed_trees::types::TypeReferenceHandle>,
    argument_owner: symbols::SymbolHandle,
    requirement_machine: symbols::SymbolHandle,
    policy_machine: String,
    relationship_span: source::SourceSpan,
    requester: semantic_vocabulary::compiler_evaluation::DeclarationCustody,
    review: Result<super::signature_review::PreparedCallingReview, String>,
}

impl PreparedCallingPolicy {
    fn canonical_instance<'code>(
        &self,
        code: &'code super::CallingProgram,
    ) -> Result<&'code terminal_psi::CallingPolicyInstance, String> {
        let module = code.artifact.semantic_module();
        let request = module
            .calling_policy_requests
            .get(self.request_ordinal)
            .ok_or("calling policy lost its exact canonical request")?;
        let owner = *request.declaration.as_ref().map_err(Clone::clone)?;
        let request_owner = self.request_owner.as_ref().map_err(Clone::clone)?;
        if module.identities.text(owner) != Some(request_owner.as_str()) {
            return Err("calling policy request differs from its original source owner".into());
        }
        let instance = request
            .instances
            .as_ref()
            .map_err(|reasons| reasons.join("; "))?
            .get(self.instance_ordinal)
            .ok_or("calling policy lost its exact canonical instance")?;
        if !instance
            .signatures
            .as_ref()
            .map_err(Clone::clone)?
            .contains(&self.signature)
        {
            return Err(
                "calling policy instance does not contain its exact declaration ABI".into(),
            );
        }
        let expected = self.abi.as_ref().map_err(Clone::clone)?;
        let actual = module
            .calling_abi_signature(self.signature)
            .ok_or("calling policy lost its complete canonical ABI declaration")?
            .identity(&module.identities)?;
        if &actual != expected {
            return Err(
                "calling policy ABI differs from its exact original Source application".into(),
            );
        }
        Ok(instance)
    }
}

impl PreparedBoundaryCallingPlans {
    pub fn evaluate(
        self,
        code: &super::CallingProgram,
    ) -> Result<Vec<BoundaryCallingPlanRealization>, Vec<Diagnostic>> {
        // Each policy runs in its own sponsor and invocation account over the
        // same immutable code, so the activations are independent of each
        // other and of their order. They run together; their results are then
        // consumed in request order, so the first refusal in that order is
        // still the one reported.
        use rayon::prelude::*;
        let activations: Vec<_> = validation::checking_thread_pool().install(|| {
            self.pending
                .par_iter()
                .map(|policy| {
                    code.require_owner(&self.source_owner)?;
                    let instance = policy.canonical_instance(code)?;
                    let evaluated = code.evaluate(
                        instance,
                        self.target,
                        policy.signature,
                        &self.providers,
                        policy.requester,
                        self.authority.as_deref(),
                    )?;
                    Ok::<_, String>((instance, evaluated))
                })
                .collect()
        });
        let mut evaluated = Vec::with_capacity(self.pending.len());
        for (policy, activation) in self.pending.into_iter().zip(activations) {
            let diagnostic = |reason: String| {
                vec![Diagnostic::error(reason).with_source_span(policy.relationship_span)]
            };
            let module = code.artifact.semantic_module();
            let (instance, (canonical, validated)) = activation.map_err(&diagnostic)?;
            let callback_context_closed = !canonical.policy.callbacks.binders.is_empty()
                || !canonical.policy.callbacks.demands.is_empty();
            let review = policy
                .review
                .as_ref()
                .map_err(|reason| diagnostic(reason.clone()))?;
            let signature = review
                .attach(canonical, module, &code.source_records, self.target)
                .map_err(&diagnostic)?;
            let calling_owner_application = module
                .identities
                .text(instance.declaring_application)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    diagnostic("calling instance lost its complete owner application".into())
                })?;
            let (report_fingerprint, commitment) = boundary_plan_application_identity(
                &signature,
                &validated,
                calling_owner_application,
            )
            .map_err(&diagnostic)?;
            evaluated.push(BoundaryCallingPlanRealization {
                declaration: policy.signature,
                calling_owner_application: calling_owner_application.to_owned(),
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
    calling_owner_application: &str,
) -> Result<(u64, [u8; 32]), String> {
    let application =
        publish_materialized_application(signature, validated, calling_owner_application)?;
    Ok((application.report_fingerprint, application.commitment))
}

/// Both authored and default policies publish the complete source-free C shape
/// together with its validated plan. Returning only a bare plan discarded the
/// opaque representation graph before the runtime could admit a typed host.
fn publish_materialized_application(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
    calling_owner_application: &str,
) -> Result<calling_conventions::BoundaryCallingApplication, String> {
    let mut application = super::callback_bindings::source_free_application(
        signature,
        validated,
        calling_owner_application,
    )?;
    application.commitment = application.recomputed_commitment()?;
    application.report_fingerprint = callback_nominal_report_fingerprint(
        b"omega.boundary-plan-application-report.v3",
        &[&application.commitment],
    );
    Ok(application)
}
