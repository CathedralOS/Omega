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
use representation_selections::OpaqueRepresentationSelection;
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
/// Prepare exact Source request/review coordinates and canonical helper choices.
/// Shared inert Terminal publication precedes invocation-scoped helper choice.
/// Canonical invocation admission runs before any target activation, including
/// when the source owner has already been dropped.
pub fn prepare_boundary_calling_plans(
    checked: &Arc<checked_trees::CheckedTrees>,
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    target_profile: target::TargetProfile,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
    provider_inputs: super::CallingPolicyProviderInputs<'_>,
    code: &super::CallingProgram,
) -> Result<PreparedBoundaryCallingPlans, Vec<Diagnostic>> {
    code.require_owner(&Arc::downgrade(checked))
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
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
    let mut opaque_reviews = None;
    let mut pending = Vec::new();
    for (request_ordinal, request) in calling.requests().enumerate() {
        let request_owner = typed.normalized_hermetic_symbol_identity(request.boundary);
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
                        request_owner: request_owner.clone(),
                        boundary_arguments: calling.arguments(instance).to_vec(),
                        argument_owner: instance.argument_owner,
                        requirement_machine: declaration.symbol,
                        policy_machine: policy_machine.name.as_str().to_owned(),
                        relationship_span: request.span,
                        requester: match typed.symbols.source_file(request.span) {
                            Some(file) if file.origin == source::SourceOrigin::Toolchain => semantic_vocabulary::compiler_evaluation::DeclarationCustody::Toolchain,
                            Some(file) => file.package_identity.map(semantic_vocabulary::compiler_evaluation::DeclarationCustody::Package)
                                .unwrap_or(semantic_vocabulary::compiler_evaluation::DeclarationCustody::UnownedUser),
                            None => semantic_vocabulary::compiler_evaluation::DeclarationCustody::Missing,
                        },
                        review: super::signature_review::PreparedCallingReview::prepare(
                            declaration,
                            Arc::clone(opaque_reviews.get_or_insert_with(|| {
                                super::signature_review::prepare_opaque_reviews(
                                    typed,
                                    opaque_representation_selections,
                                )
                            })),
                        ),
                    });
                }
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
        let providers = super::selected_providers::prepare(
            services,
            target_profile,
            &roots,
            &provider_inputs,
            code,
        )?;
        provider_declarations = providers.declarations;
    }
    Ok(PreparedBoundaryCallingPlans {
        source_owner: Arc::downgrade(checked),
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
        Ok(instance)
    }
}

impl PreparedBoundaryCallingPlans {
    pub fn evaluate(
        self,
        code: &super::CallingProgram,
    ) -> Result<Vec<BoundaryCallingPlanRealization>, Vec<Diagnostic>> {
        let mut evaluated = Vec::with_capacity(self.pending.len());
        for policy in self.pending {
            let diagnostic = |reason: String| {
                vec![Diagnostic::error(reason).with_source_span(policy.relationship_span)]
            };
            code.require_owner(&self.source_owner)
                .map_err(&diagnostic)?;
            let module = code.artifact.semantic_module();
            let instance = policy.canonical_instance(code).map_err(&diagnostic)?;
            let (canonical, validated) = code
                .evaluate(
                    instance,
                    self.target,
                    policy.signature,
                    &self.providers,
                    policy.requester,
                    self.authority.as_deref(),
                )
                .map_err(&diagnostic)?;
            let callback_context_closed = !canonical.policy.callbacks.binders.is_empty()
                || !canonical.policy.callbacks.demands.is_empty();
            let review = policy
                .review
                .as_ref()
                .map_err(|reason| diagnostic(reason.clone()))?;
            let signature = review
                .attach(canonical, module, &code.source_records, self.target)
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
