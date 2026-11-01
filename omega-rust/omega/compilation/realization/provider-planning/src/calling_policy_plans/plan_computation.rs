//! Computing calling plans: evaluating the authored policy at build time,
//! materializing the boundary signature from the ABI result, and validating
//! the plan the policy produced.

use crate::calling_policy_plans::boundary_signatures::{
    compatibility_call_signature, exact_compatibility_overload_index,
    materialize_prepared_signature,
};
use crate::calling_policy_plans::callback_bindings::{
    close_callback_materializations, validate_retained_callback_binders,
};
use crate::calling_policy_plans::{BoundaryCallingPlanRealization, MaterializedBoundarySignature};
use build_time_evaluation::BuildTimeProgram;
use calling_conventions::callback_nominal_report_fingerprint;
use calling_conventions::{
    CallingPolicy, ValidatedBoundaryEntryPlan, evaluate_ordinary_boundary_entry_plan,
};
use diagnostics::Diagnostic;
use representation_planning::OpaqueRepresentationSelection;

pub(crate) fn evaluate_default_calling_application(
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    layout_target: layout::LayoutTarget,
    trait_name: &str,
    trait_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    method_name: &str,
    requirement_identity: &str,
    policy: CallingPolicy,
    dispatch_only_parameter_count: usize,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
) -> Result<Option<calling_conventions::BoundaryCallingApplication>, String> {
    let candidates = services
        .signatures()
        .declarations()
        .filter(|declaration| {
            declaration.default_eligible
                && declaration.path == trait_name
                && declaration.package == trait_package_identity
                && declaration.method == method_name
        })
        .collect::<Vec<_>>();
    let Some(candidate_index) = exact_compatibility_overload_index(
        trait_name,
        method_name,
        requirement_identity,
        candidates
            .iter()
            .map(|declaration| declaration.requirement_identity.as_str()),
    )?
    else {
        return Ok(None);
    };
    let declaration = candidates.get(candidate_index).ok_or_else(|| {
        "compatibility boundary selection index is outside its exact candidate catalog".to_owned()
    })?;
    let materialized = materialize_prepared_signature(
        services,
        declaration,
        None,
        &[],
        layout_target,
        opaque_representation_selections,
    )?;
    let classified =
        compatibility_call_signature(&materialized, policy, dispatch_only_parameter_count)?;
    let validated = evaluate_ordinary_boundary_entry_plan(policy, &classified)
        .map_err(|diagnostic| diagnostic.to_string())?;
    for representation in materialized.opaque_representation_uses() {
        materialized.opaque_representation_movement(representation, &validated)?;
    }
    publish_materialized_application(&materialized, &validated).map(Some)
}

/// Discover concrete `Calling<Policy>` relationships, evaluate the selected implementation row once for
/// every method in the boundary service surface, and retain only canonical
/// evaluated applications in the realization output. Source trees are borrowed.
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
    // Provider selection consumes the source judgment. Retain it through
    // policy admission rather than rechecking its internal call premises.
    let typed = &checked.typed;
    let signatures = services.signatures();
    let calling = signatures.calling();
    let mut pending = Vec::new();
    for request in calling.requests() {
        for instance in calling.instances(request)? {
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
            // Psi retains the exact closed named map. Only deployment body
            // selection remains here; a family head is not executable code.
            let policy_machine = typed.machine_for_target(policy, target_profile.target_name())
                .ok_or_else(|| vec![
                    Diagnostic::error(format!(
                        "boundary trait `{}` cannot evaluate Calling<Policy>: calling policy realization `{}` has no body for target `{}`",
                        request.name, policy.name, target_profile.target_name(),
                    )).with_source_span(request.span),
                ])?;
            for application in calling.signatures(instance) {
                let declaration = signatures.applied_declaration(application).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "calling plan lost its exact prepared signature declaration",
                    )]
                })?;
                if let Some(reason) = &application.abi_refusal {
                    return Err(vec![
                        Diagnostic::error(reason.clone()).with_source_span(request.span),
                    ]);
                }
                for &abi in &application.abi_signatures {
                    let boundary_signature = materialize_prepared_signature(
                        services,
                        declaration,
                        Some(abi),
                        calling.bindings(application),
                        layout::LayoutTarget::for_profile(target_profile)
                            .map_err(|reason| vec![Diagnostic::error(reason)])?,
                        opaque_representation_selections,
                    )
                    .map_err(|reason| {
                        vec![
                            Diagnostic::error(format!(
                                "cannot materialize boundary signature `{}::{}` for `{}`: {reason}",
                                request.name, declaration.method, policy_machine.name,
                            ))
                            .with_source_span(request.span),
                        ]
                    })?;
                    pending.push(PreparedCallingPolicy {
                        boundary_trait: request.boundary,
                        boundary_arguments: calling.arguments(instance).to_vec(),
                        argument_owner: instance.argument_owner,
                        requirement_machine: declaration.symbol,
                        policy_machine: policy_machine.name.as_str().to_owned(),
                        policy_symbol: policy_machine.symbol,
                        signature: boundary_signature,
                        callback_context_closed: false,
                        relationship_span: request.span,
                        admission: Ok(()),
                    });
                }
            }
        }
    }

    if pending.is_empty() {
        return Ok(PreparedBoundaryCallingPlans {
            checked,
            pending,
            target: target_profile,
        });
    }
    // Complete the native telescope and private destinations before invoking
    // any policy. A partial signature is not an earlier admissible ABI: a
    // policy may correctly refuse it even though it accepts the actual call.
    let layout_target = layout::LayoutTarget::for_profile(target_profile)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let callback_layout = layout::build_layout_plan_for_roots(
        typed,
        layout_target,
        opaque_representation_selections,
        pending.iter().flat_map(|policy| {
            policy
                .signature
                .native_parameters
                .iter()
                .map(|parameter| parameter.layout_data_symbol)
        }),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    for policy in &mut pending {
        policy.callback_context_closed = close_callback_materializations(
            &mut policy.signature,
            &callback_layout,
            layout_target.native_target(),
        )
        .map_err(|reason| {
            vec![Diagnostic::error(reason).with_source_span(policy.relationship_span)]
        })?;
    }
    // Policy helpers use ordinary target families too. Select their exact
    // retained bodies in Omega, then give admission and evaluation one shared
    // identity mapping; Psi must neither guess the host nor pick a sibling.
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
                build_time_evaluation::BuildTimeInvocationCustody::Source(policy.relationship_span),
            )
            .map(|_closure| ());
    }
    Ok(PreparedBoundaryCallingPlans {
        checked,
        pending,
        target: target_profile,
    })
}

/// Source admission and signature preparation precede code publication.
/// Activation consumes only this owned preparation and retained policy code.
pub struct PreparedBoundaryCallingPlans<'checked> {
    checked: &'checked checked_trees::CheckedTrees,
    pending: Vec<PreparedCallingPolicy>,
    target: target::TargetProfile,
}

struct PreparedCallingPolicy {
    boundary_trait: symbols::SymbolHandle,
    boundary_arguments: Vec<checked_trees::typed_trees::types::TypeReferenceHandle>,
    argument_owner: symbols::SymbolHandle,
    requirement_machine: symbols::SymbolHandle,
    policy_machine: String,
    policy_symbol: symbols::SymbolHandle,
    signature: MaterializedBoundarySignature,
    callback_context_closed: bool,
    relationship_span: source::SourceSpan,
    admission: Result<(), String>,
}

impl PreparedBoundaryCallingPlans<'_> {
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn evaluate(
        self,
        code: Option<&super::CallingPolicyCode>,
    ) -> Result<Vec<BoundaryCallingPlanRealization>, Vec<Diagnostic>> {
        let mut evaluated = Vec::with_capacity(self.pending.len());
        for PreparedCallingPolicy {
            boundary_trait,
            boundary_arguments,
            argument_owner,
            requirement_machine,
            policy_machine,
            policy_symbol,
            signature,
            callback_context_closed,
            relationship_span,
            admission,
        } in self.pending
        {
            admission.map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(relationship_span)]
            })?;
            let code = code.ok_or_else(|| {
                vec![Diagnostic::error(
                    "calling policies have no published Terminal code",
                )]
            })?;
            code.require_owner(self.checked)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let validated = code
                .evaluate(policy_symbol, self.target, &signature)
                .map_err(|reason| {
                    vec![Diagnostic::error(reason).with_source_span(relationship_span)]
                })?;
            let (application_report_fingerprint, application_commitment) =
                boundary_plan_application_identity(&signature, &validated)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
            evaluated.push(BoundaryCallingPlanRealization {
                boundary_trait,
                boundary_arguments,
                argument_owner,
                requirement_machine,
                report_fingerprint: application_report_fingerprint,
                commitment: effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                    application_commitment,
                ),
                boundary_entry_plan: validated.plan().clone(),
                validated_boundary_entry_plan: validated,
                callback_binders: signature.callback_binders.clone(),
                callback_demands: signature.callback_demands.clone(),
                callback_context_closed,
                native_parameters: signature.native_parameters.clone(),
                materialized_signature: signature,
                policy_machine,
                relationship_span,
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
