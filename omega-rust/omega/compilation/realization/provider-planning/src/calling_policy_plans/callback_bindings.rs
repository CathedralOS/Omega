/*
We own `BoundaryCallingPlanRealization`—the Omega-owned realization state for one
canonical source boundary contract—along with retained callback binders and the
closure of outbound and direct callback materializations.

Checked custody retains only the semantic key and fingerprint, while concrete
ABI, register, and stack choices stay beside the native/provider pipeline that
consumes them. The crate-sealed `validated_boundary_entry_plan` is minted with
the validated realization; the public `report_fingerprint` remains a compact
compatibility/report coordinate and cannot authorize replacement of the exact
plan or materialized signature.
*/

use crate::calling_policy_plans::callback_layout_catalog;
use crate::calling_policy_plans::plan_computation::boundary_plan_application_identity;
use crate::calling_policy_plans::{
    BoundaryCallbackBinder, BoundaryNativeParameter, MaterializedBoundarySignature,
};
use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallbackBinderRequirement, CallbackMaterializationContext,
    CallbackRequirementId, NativeCallbackDemand, ValidatedBoundaryEntryPlan,
    validate_boundary_entry_plan_with_callback_materializations,
};
use checked_trees::typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallingPlanRealization {
    pub declaration: semantic_vocabulary::CallingSignatureId,
    pub(crate) calling_owner_application: String, // complete canonical application of the Calling relationship's root
    pub boundary_trait: symbols::SymbolHandle,
    pub boundary_arguments: Vec<TypeReferenceHandle>,
    pub(crate) argument_owner: symbols::SymbolHandle, // supplying declaration for lifetime-bearing type operands
    pub requirement_machine: symbols::SymbolHandle,
    pub report_fingerprint: u64, // compact compatibility/report coordinate; exact plan and signature remain authority
    pub commitment: effects::provider_plan::BoundaryCallingPlanCommitment,
    pub boundary_entry_plan: BoundaryEntryPlan,
    pub(crate) validated_boundary_entry_plan: ValidatedBoundaryEntryPlan,
    pub callback_binders: Vec<BoundaryCallbackBinder>,
    pub callback_demands: Vec<NativeCallbackDemand>,
    pub callback_context_closed: bool,
    pub native_parameters: Vec<BoundaryNativeParameter>,
    pub(crate) materialized_signature: MaterializedBoundarySignature,
    pub policy_machine: String,
    pub relationship_span: source::SourceSpan,
}

impl BoundaryCallingPlanRealization {
    pub fn calling_owner_application(&self) -> &str {
        &self.calling_owner_application
    }

    pub const fn argument_owner(&self) -> symbols::SymbolHandle {
        self.argument_owner
    }

    pub const fn materialized_signature(&self) -> &MaterializedBoundarySignature {
        &self.materialized_signature
    }

    pub const fn exact_boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        self.validated_boundary_entry_plan.plan()
    }

    pub fn native_application(
        &self,
    ) -> Result<calling_conventions::BoundaryCallingApplication, String> {
        let mut application = source_free_application(
            &self.materialized_signature,
            &self.validated_boundary_entry_plan,
            &self.calling_owner_application,
        )?;
        application.report_fingerprint = self.report_fingerprint;
        application.commitment = self.commitment.as_bytes();
        application.callback_context_closed = self.callback_context_closed;
        Ok(application)
    }

    pub(crate) fn replayed_validated_plan(
        &self,
    ) -> Result<ValidatedBoundaryEntryPlan, calling_conventions::PlanDiagnostic> {
        let signature = CallSignature {
            parameters: self
                .boundary_entry_plan
                .call
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: self
                .boundary_entry_plan
                .call
                .result
                .as_ref()
                .map(|placement| placement.shape),
        };
        if self.callback_context_closed {
            let context = CallbackMaterializationContext {
                binders: self
                    .callback_binders
                    .iter()
                    .map(|binder| CallbackBinderRequirement {
                        binder: binder.binder,
                        requirement: binder.requirement,
                    })
                    .collect(),
                demands: self.callback_demands.clone(),
            };
            validate_boundary_entry_plan_with_callback_materializations(
                self.boundary_entry_plan.clone(),
                &signature,
                &context,
            )
        } else {
            calling_conventions::validate_boundary_entry_plan(
                self.boundary_entry_plan.clone(),
                &signature,
            )
        }
    }

    pub fn replayed_validated_application(
        &self,
    ) -> Result<
        (
            ValidatedBoundaryEntryPlan,
            u64,
            effects::provider_plan::BoundaryCallingPlanCommitment,
        ),
        calling_conventions::PlanDiagnostic,
    > {
        if self.callback_context_closed {
            callback_layout_catalog::validate(&self.materialized_signature, &self.callback_demands)
                .map_err(calling_conventions::PlanDiagnostic)?;
        }
        if self
            .materialized_signature
            .opaque_representations
            .iter()
            .any(|use_| {
                !use_.opaque.is_valid()
                    || !use_.conformance.is_valid()
                    || !use_.carrier.is_valid()
                    || usize::from(use_.shape_root) >= self.materialized_signature.shapes.len()
                    || use_.representation_schema_version
                        != language_semantics::opaque_representations::OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
                    || use_.selected_application_commitment
                        != use_.rederived_selected_application_commitment()
            })
        {
            return Err(calling_conventions::PlanDiagnostic(
                "boundary calling plan retained stale opaque-representation application custody"
                    .to_owned(),
            ));
        }
        let validated = self.replayed_validated_plan()?;
        for representation in &self.materialized_signature.opaque_representations {
            self.materialized_signature
                .opaque_representation_movement(representation, &validated)
                .map_err(|reason| {
                    calling_conventions::PlanDiagnostic(format!(
                        "boundary calling plan cannot rejoin opaque-representation movement: {reason}"
                    ))
                })?;
        }
        let (report_fingerprint, commitment) = boundary_plan_application_identity(
            &self.materialized_signature,
            &validated,
            &self.calling_owner_application,
        )
        .map_err(calling_conventions::PlanDiagnostic)?;
        Ok((
            validated,
            report_fingerprint,
            effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(commitment),
        ))
    }
}

pub(crate) fn validate_retained_callback_binders(
    realization: &BoundaryCallingPlanRealization,
) -> Result<(), String> {
    for (index, binder) in realization.callback_binders.iter().enumerate() {
        if !binder.parameter_symbol.is_valid()
            || !binder.requirement_trait.is_valid()
            || !binder.requirement_machine.is_valid()
        {
            return Err(
                "evaluated registrar plan retained an invalid nominal callback binder symbol"
                    .to_owned(),
            );
        }
        if realization.callback_binders[..index].iter().any(|prior| {
            prior.binder == binder.binder
                || prior.static_machine_ordinal == binder.static_machine_ordinal
                || prior.parameter_symbol == binder.parameter_symbol
        }) {
            return Err(
                "evaluated registrar plan retained a duplicate nominal callback binder identity"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_callback_requirement_report_collision(
    exact_catalog: &[(CallbackRequirementId, String)],
    report: CallbackRequirementId,
    exact: &str,
) -> Result<(), String> {
    if exact_catalog
        .iter()
        .any(|(prior_report, prior_exact)| *prior_report == report && prior_exact != exact)
    {
        return Err(
            "distinct exact callback requirements collide on one compact report identity"
                .to_owned(),
        );
    }
    Ok(())
}

pub(crate) fn callback_materialization_context(
    signature: &MaterializedBoundarySignature,
) -> CallbackMaterializationContext {
    CallbackMaterializationContext {
        binders: signature
            .callback_binders
            .iter()
            .map(|binder| CallbackBinderRequirement {
                binder: binder.binder,
                requirement: binder.requirement,
            })
            .collect(),
        demands: signature.callback_demands.clone(),
    }
}

pub(super) fn source_free_application(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
    calling_owner_application: &str,
) -> Result<calling_conventions::BoundaryCallingApplication, String> {
    let policy = signature.policy_signature()?;
    Ok(calling_conventions::BoundaryCallingApplication {
        abi: signature.abi.clone(),
        calling_owner_application: Some(calling_owner_application.to_owned()),
        requirement_identity: signature.owner_requirement_identity().to_owned(),
        target: policy.target,
        report_fingerprint: 0,
        commitment: [0; 32],
        plan: validated.clone(),
        dispatch_only_parameter_count: 0,
        signature: policy.geometry,
        opaque_applications: signature
            .opaque_representation_uses()
            .iter()
            .map(|use_| calling_conventions::BoundaryOpaqueApplication {
                shape_root: use_.shape_root(),
                application_report_fingerprint: use_.application_report_fingerprint(),
                selected_application_commitment: use_.selected_application_commitment(),
                conformance_application_commitment: use_.conformance_application_commitment,
                representation_schema_version: use_.representation_schema_version,
                checked_semantic_copy: matches!(
                    use_.copy_disposition,
                    language_semantics::opaque_representations::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
                ),
            })
            .collect(),
        callback_context: policy.callbacks,
        callback_context_closed: false,
    })
}

impl MaterializedBoundarySignature {
    pub fn policy_signature(&self) -> Result<super::CallingPolicySignature, String> {
        let signature = self;
        let mut callback_bindings = arena::Arena::with_capacity(signature.callback_binders.len());
        let mut callback_binding_rows = arena::HandleSpan::empty();
        for binder in &signature.callback_binders {
            callback_bindings.append_to_span(
                &mut callback_binding_rows,
                calling_conventions::BoundaryCallbackBinding {
                    binder: binder.binder,
                    requirement: binder.requirement,
                    static_machine_ordinal: binder.static_machine_ordinal,
                    requirement_identity: binder.requirement_identity.clone(),
                    invokes_synchronously: binder.invokes_synchronously,
                },
            );
        }
        let mut native_parameters = arena::Arena::with_capacity(signature.native_parameters.len());
        let mut native_parameter_rows = arena::HandleSpan::empty();
        for parameter in &signature.native_parameters {
            native_parameters.append_to_span(
                &mut native_parameter_rows,
                calling_conventions::BoundaryNativeParameterGeometry {
                    identity: parameter.identity,
                    native_ordinal: parameter.native_ordinal,
                    origin: parameter.origin,
                    shape: parameter.shape,
                },
            );
        }
        let mut callback_layouts =
            arena::Arena::with_capacity(signature.callback_layout_catalog.len());
        let mut callback_layout_rows = arena::HandleSpan::empty();
        for entry in &signature.callback_layout_catalog {
            callback_layouts.append_to_span(&mut callback_layout_rows, entry.source_free()?);
        }
        Ok(super::CallingPolicySignature {
            target: signature.native_target(),
            geometry: calling_conventions::BoundarySignatureGeometry {
                shapes: signature.shapes().to_vec(),
                fields: signature.fields().to_vec(),
                parameters: signature.parameters().to_vec(),
                result: signature.result(),
                native_parameters,
                native_parameter_rows,
                callback_layouts,
                callback_layout_rows,
                callback_bindings,
                callback_binding_rows,
            },
            callbacks: callback_materialization_context(signature),
        })
    }
}
