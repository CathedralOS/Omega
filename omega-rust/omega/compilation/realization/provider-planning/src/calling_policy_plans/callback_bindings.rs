//! Callback bindings: the calling-plan realization, the retained callback
//! binders and the closure of
//! outbound and direct callback materializations.

use crate::calling_policy_plans::build_time_decoding::CALLBACK_MATERIALIZATION_CAPACITY;
use crate::calling_policy_plans::callback_layout_catalog;
use crate::calling_policy_plans::callback_layout_catalog::BoundaryCallbackLayoutEntry;
use crate::calling_policy_plans::plan_computation::boundary_plan_application_identity;
use crate::calling_policy_plans::{
    BoundaryCallbackBinder, BoundaryNativeParameter, BoundaryNativeParameterOrigin,
    MaterializedBoundarySignature,
};
use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallbackBinderRequirement, CallbackMaterializationContext,
    CallbackRequirementId, NativeCallbackDemand, ValidatedBoundaryEntryPlan,
    callback_requirement_id, validate_boundary_entry_plan_with_callback_materializations,
};
use checked_trees::typed_trees::types::TypeReferenceHandle;
use target::NativeTarget;

/// Omega-owned realization state for one canonical source boundary contract.
///
/// Typed trees retain only the semantic key and fingerprint. Concrete ABI,
/// register, and stack choices stay beside the native/provider pipeline that
/// consumes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallingPlanRealization {
    pub boundary_trait: symbols::SymbolHandle,
    pub boundary_arguments: Vec<TypeReferenceHandle>,
    /// Supplying declaration for lifetime-bearing type operands. An argument
    /// handle alone does not place its regions in the boundary trait's scope.
    pub(crate) argument_owner: symbols::SymbolHandle,
    pub requirement_machine: symbols::SymbolHandle,
    /// Compact compatibility/report coordinate for the complete target-closed
    /// application. Exact plan and materialized-signature custody below remain
    /// the authority for realization joins.
    pub report_fingerprint: u64,
    pub commitment: effects::provider_plan::BoundaryCallingPlanCommitment,
    pub boundary_entry_plan: BoundaryEntryPlan,
    /// Crate-sealed exact evidence minted with the validated realization.
    /// The public fingerprint remains a compact compatibility/report
    /// coordinate and cannot authorize replacement of this plan.
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
    pub const fn argument_owner(&self) -> symbols::SymbolHandle {
        self.argument_owner
    }

    /// The named layout catalog and ABI signature are published together.
    /// Callers may inspect them but cannot substitute an unhashed catalog.
    pub const fn materialized_signature(&self) -> &MaterializedBoundarySignature {
        &self.materialized_signature
    }

    pub const fn exact_boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        self.validated_boundary_entry_plan.plan()
    }

    /// Publish the completed ABI application without retaining evaluator state.
    /// The validated plan is retained at production and callback closure, not
    /// reconstructed by a downstream native stage.
    pub fn native_application(
        &self,
    ) -> Result<calling_conventions::BoundaryCallingApplication, String> {
        let mut application = source_free_application(
            &self.materialized_signature,
            &self.validated_boundary_entry_plan,
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
                        != representation_planning::OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
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
        let (report_fingerprint, commitment) =
            boundary_plan_application_identity(&self.materialized_signature, &validated)
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

fn exact_callback_requirement_catalog(
    binders: &[BoundaryCallbackBinder],
) -> Result<Vec<(CallbackRequirementId, String)>, String> {
    let mut catalog = Vec::with_capacity(binders.len());
    for binder in binders {
        let exact = &binder.requirement_identity;
        let report = callback_requirement_id(exact);
        if report != binder.requirement {
            return Err(
                "evaluated callback binder requirement report drifted from its exact requirement"
                    .to_owned(),
            );
        }
        validate_callback_requirement_report_collision(&catalog, report, exact)?;
        catalog.push((report, exact.clone()));
    }
    Ok(catalog)
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

/// Close a signature's native telescope and private destinations before its
/// policy is invoked. Geometry cannot depend on an earlier accepted plan:
/// the policy must see the complete declared demand on its only invocation.
/// This creates placement input, not checked callback authority.
pub(super) fn close_callback_materializations(
    signature: &mut MaterializedBoundarySignature,
    layout_plan: &layout::LayoutPlan,
    target: NativeTarget,
) -> Result<bool, String> {
    let direct_demands = close_direct_callback_parameters(signature, target)?;
    // Keep each named catalog entry paired with its exact native demand
    // through sorting. Equal compact IDs must never select a different row.
    let mut demands = direct_demands
        .into_iter()
        .map(|demand| (demand, None))
        .collect::<Vec<_>>();
    let binder_requirement_catalog =
        exact_callback_requirement_catalog(&signature.callback_binders)?;
    let mut demand_requirement_catalog = Vec::new();
    for parameter in &signature.native_parameters {
        for layout in layout_plan
            .private_callback_layouts
            .iter()
            .filter(|layout| layout.root_layout().data_symbol == parameter.layout_data_symbol)
        {
            let terminal = layout.terminal_slot();
            validate_callback_requirement_report_collision(
                &binder_requirement_catalog,
                terminal.requirement,
                &terminal.callback_requirement_identity,
            )
            .and_then(|()| {
                validate_callback_requirement_report_collision(
                    &demand_requirement_catalog,
                    terminal.requirement,
                    &terminal.callback_requirement_identity,
                )
            })?;
            demand_requirement_catalog.push((
                terminal.requirement,
                terminal.callback_requirement_identity.to_string(),
            ));
            let entry = BoundaryCallbackLayoutEntry::from_layout(parameter, layout)?;
            demands.push((layout.native_demand(parameter.identity)?, Some(entry)));
        }
    }
    if demands.is_empty() && signature.callback_binders.is_empty() {
        return Ok(false);
    }
    if demands.len() > CALLBACK_MATERIALIZATION_CAPACITY {
        return Err(format!(
            "boundary registrar has {} target-closed private callback demands; calling policies currently support at most {CALLBACK_MATERIALIZATION_CAPACITY}",
            demands.len()
        ));
    }
    demands.sort_unstable_by(|left, right| left.0.destination.cmp(&right.0.destination));
    if demands
        .windows(2)
        .any(|pair| pair[0].0.destination == pair[1].0.destination)
    {
        return Err(
            "boundary registrar target closure repeats one exact private callback demand"
                .to_owned(),
        );
    }

    let (demands, entries): (Vec<_>, Vec<_>) = demands.into_iter().unzip();
    signature.callback_demands = demands;
    signature.callback_layout_catalog = entries.into_iter().flatten().collect();
    Ok(true)
}

fn close_direct_callback_parameters(
    signature: &mut MaterializedBoundarySignature,
    target: NativeTarget,
) -> Result<Vec<NativeCallbackDemand>, String> {
    let semantic = signature
        .native_parameters
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.origin,
                BoundaryNativeParameterOrigin::SemanticFormal { .. }
            )
        })
        .map(
            |parameter| calling_conventions::BoundaryNativeParameterGeometry {
                identity: parameter.identity,
                native_ordinal: parameter.native_ordinal,
                shape: parameter.shape,
                origin: parameter.origin,
            },
        );
    let callbacks = signature.direct_callback_parameters.iter().map(|direct| {
        terminal_psi::CallingDirectCallback {
            identity: direct.identity,
            native_ordinal: direct.native_ordinal,
            binder: direct.binder,
            requirement: direct.requirement,
        }
    });
    let closed = calling_conventions::NativeParameterTelescope::close(semantic, callbacks, target)?;
    let demands = closed.direct_demands().collect();
    let mut parameters = Vec::with_capacity(closed.parameters.len());
    for parameter in closed.parameters {
        let layout_data_symbol = match parameter.origin {
            BoundaryNativeParameterOrigin::SemanticFormal { .. } => {
                signature
                    .native_parameters
                    .iter()
                    .find(|source| {
                        source.identity == parameter.identity && source.origin == parameter.origin
                    })
                    .ok_or("native telescope lost its exact semantic layout correspondence")?
                    .layout_data_symbol
            }
            BoundaryNativeParameterOrigin::PrivateCallback { .. } => {
                symbols::SymbolHandle::invalid()
            }
        };
        parameters.push(BoundaryNativeParameter {
            identity: parameter.identity,
            native_ordinal: parameter.native_ordinal,
            shape: parameter.shape,
            origin: parameter.origin,
            layout_data_symbol,
        });
    }
    signature.native_parameters = parameters;
    Ok(demands)
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

/// Canonical source-free projection shared by policy evaluation and
/// application publication. Admission recomputes this payload against an independently
/// selected method commitment; this constructor grants no receiving authority.
pub(super) fn source_free_application(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
) -> Result<calling_conventions::BoundaryCallingApplication, String> {
    let policy = signature.policy_signature()?;
    Ok(calling_conventions::BoundaryCallingApplication {
        requirement_identity: signature.owner_requirement_identity().to_owned(),
        target: policy.target, report_fingerprint: 0, commitment: [0;32],
        plan: validated.clone(),
        signature: policy.geometry,
        opaque_applications: signature.opaque_representation_uses().iter().map(|use_| {
            calling_conventions::BoundaryOpaqueApplication {
                shape_root: use_.shape_root(), application_report_fingerprint: use_.application_report_fingerprint(),
                selected_application_commitment: use_.selected_application_commitment(),
                conformance_application_commitment: use_.conformance_application_commitment,
                representation_schema_version: use_.representation_schema_version,
                checked_semantic_copy: matches!(use_.copy_disposition,
                    representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy),
            }
        }).collect(),
        callback_context: policy.callbacks,
        callback_context_closed: false,
    })
}

impl MaterializedBoundarySignature {
    /// Detach the ABI input from source correspondence. Native publication and
    /// policy activation consume this same projection, including unused callback
    /// binders and every target-closed destination demand.
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
