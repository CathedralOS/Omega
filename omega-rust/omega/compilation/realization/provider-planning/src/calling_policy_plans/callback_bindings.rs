//! Callback bindings: the calling-plan realization, the retained callback
//! binders and the closure of
//! outbound and direct callback materializations.

use crate::calling_policy_plans::build_time_decoding::CALLBACK_MATERIALIZATION_CAPACITY;
use crate::calling_policy_plans::callback_layout_catalog;
use crate::calling_policy_plans::callback_layout_catalog::BoundaryCallbackLayoutEntry;
use crate::calling_policy_plans::plan_computation::{
    boundary_plan_application_identity, evaluate_materialized_calling_policy_plan,
};
use crate::calling_policy_plans::{
    BoundaryCallbackBinder, BoundaryNativeParameter, BoundaryNativeParameterOrigin,
    BoundaryNativeParameterShape, MaterializedBoundarySignature,
};
use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallbackBinderRequirement, CallbackMaterializationContext,
    CallbackRequirementId, NativeCallbackDemand, NativeParameterId, NativePlace,
    ValidatedBoundaryEntryPlan, callback_requirement_id,
    validate_boundary_entry_plan_with_callback_materializations,
};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::types::TypeReferenceHandle;
use diagnostics::Diagnostic;
use representation_planning::OpaqueRepresentationSelection;
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
    /// Exact target member retained when callback layout closes its signature.
    pub(crate) policy_symbol: symbols::SymbolHandle,
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
    typed: &TypedTrees,
    binders: &[BoundaryCallbackBinder],
) -> Result<Vec<(CallbackRequirementId, String)>, String> {
    let mut catalog = Vec::with_capacity(binders.len());
    for binder in binders {
        let trait_definition = typed
            .trait_definition_by_symbol(binder.requirement_trait)
            .ok_or_else(|| {
                "evaluated callback binder lost its exact requirement trait".to_owned()
            })?;
        let requirement = typed
            .trait_machine_signature_by_symbol(trait_definition, binder.requirement_machine)
            .ok_or_else(|| {
                "evaluated callback binder lost its exact requirement machine".to_owned()
            })?;
        let exact = typed.trait_requirement_callable_identity_text(trait_definition, requirement);
        let report = callback_requirement_id(&exact);
        if report != binder.requirement {
            return Err(
                "evaluated callback binder requirement report drifted from its exact requirement"
                    .to_owned(),
            );
        }
        validate_callback_requirement_report_collision(&catalog, report, &exact)?;
        catalog.push((report, exact));
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

/// Close outbound registrar policies over the target's native layout catalog
/// before provider schemas and provenance capture the calling commitment.
/// Layout consumes typed structure, not a language judgment. This produces ABI
/// geometry only; checked callback authority is bound separately after Psi.
pub fn close_outbound_callback_materializations(
    typed: &TypedTrees,
    realizations: &mut [BoundaryCallingPlanRealization],
    target_profile: target::TargetProfile,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<(), Vec<Diagnostic>> {
    if realizations.is_empty() {
        return Ok(());
    }
    let layout_target = layout::LayoutTarget::for_profile(target_profile)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let native_target = layout_target.native_target();
    let layout_plan = layout::build_layout_plan_for_roots(
        typed,
        layout_target,
        opaque_representation_selections,
        realizations.iter().flat_map(|realization| {
            realization
                .materialized_signature
                .native_parameters
                .iter()
                .map(|parameter| parameter.layout_data_symbol)
        }),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let admission = build_time_evaluation::BuildTimeAdmissionPlan::infer(
        typed,
        package_inputs.map(|inputs| {
            std::sync::Arc::new(inputs.clone())
                as std::sync::Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        }),
    );

    for realization in realizations {
        let mut signature = realization.materialized_signature.clone();
        let direct_demands = close_direct_callback_parameters(&mut signature, native_target)
            .map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
            })?;
        // Keep each named catalog entry paired with its exact native demand
        // through sorting. Equal compact IDs must never select a different row.
        let mut demands = direct_demands
            .into_iter()
            .map(|demand| (demand, None))
            .collect::<Vec<_>>();
        let binder_requirement_catalog =
            exact_callback_requirement_catalog(typed, &realization.callback_binders).map_err(
                |reason| {
                    vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
                },
            )?;
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
                })
                .map_err(|reason| {
                    vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
                })?;
                demand_requirement_catalog.push((
                    terminal.requirement,
                    terminal.callback_requirement_identity.to_string(),
                ));
                let entry = BoundaryCallbackLayoutEntry::from_layout(parameter, layout).map_err(
                    |reason| {
                        vec![
                            Diagnostic::error(reason)
                                .with_source_span(realization.relationship_span),
                        ]
                    },
                )?;
                demands.push((
                    layout.native_demand(parameter.identity).map_err(|reason| {
                        vec![
                            Diagnostic::error(reason)
                                .with_source_span(realization.relationship_span),
                        ]
                    })?,
                    Some(entry),
                ));
            }
        }
        if demands.is_empty() && realization.callback_binders.is_empty() {
            continue;
        }
        if demands.len() > CALLBACK_MATERIALIZATION_CAPACITY {
            return Err(vec![Diagnostic::error(format!(
                "boundary registrar has {} target-closed private callback demands; calling policies currently support at most {CALLBACK_MATERIALIZATION_CAPACITY}",
                demands.len()
            ))
            .with_source_span(realization.relationship_span)]);
        }
        demands.sort_unstable_by(|left, right| left.0.destination.cmp(&right.0.destination));
        if demands
            .windows(2)
            .any(|pair| pair[0].0.destination == pair[1].0.destination)
        {
            return Err(vec![
                Diagnostic::error(
                    "boundary registrar target closure repeats one exact private callback demand",
                )
                .with_source_span(realization.relationship_span),
            ]);
        }

        let (demands, entries): (Vec<_>, Vec<_>) = demands.into_iter().unzip();
        signature.callback_demands = demands.clone();
        signature.callback_layout_catalog = entries.into_iter().flatten().collect();
        let validated = evaluate_materialized_calling_policy_plan(
            typed,
            &admission,
            &realization.policy_machine,
            realization.policy_symbol,
            &signature,
            Some(build_time_evaluation::BuildTimeInvocationCustody::Source(
                realization.relationship_span,
            )),
        )
        .map_err(|rejection| {
            vec![
                Diagnostic::error(rejection.reason).with_source_span(
                    rejection
                        .source_span
                        .unwrap_or(realization.relationship_span),
                ),
            ]
        })?;
        let classified = CallSignature {
            parameters: validated
                .plan()
                .call
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: validated
                .plan()
                .call
                .result
                .as_ref()
                .map(|placement| placement.shape),
        };
        let context = callback_materialization_context(&signature);
        let validated = validate_boundary_entry_plan_with_callback_materializations(
            validated.plan().clone(),
            &classified,
            &context,
        )
        .map_err(|diagnostic| {
            vec![
                Diagnostic::error(format!(
                    "target-closed outbound callback materialization is invalid: {diagnostic}"
                ))
                .with_source_span(realization.relationship_span),
            ]
        })?;
        let (new_fingerprint, new_commitment) =
            boundary_plan_application_identity(&signature, &validated)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        realization.report_fingerprint = new_fingerprint;
        realization.commitment =
            effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(new_commitment);
        realization.boundary_entry_plan = validated.plan().clone();
        realization.validated_boundary_entry_plan = validated;
        realization.callback_demands = demands;
        realization.native_parameters = signature.native_parameters.clone();
        realization.callback_context_closed = true;
        realization.materialized_signature = signature;
    }
    Ok(())
}

fn close_direct_callback_parameters(
    signature: &mut MaterializedBoundarySignature,
    target: NativeTarget,
) -> Result<Vec<NativeCallbackDemand>, String> {
    signature.native_parameters.retain(|parameter| {
        matches!(
            parameter.origin,
            BoundaryNativeParameterOrigin::SemanticFormal { .. }
        )
    });
    let byte_size = u16::try_from(target.pointer_size)
        .map_err(|_| "target function-pointer size exceeds boundary shape range".to_owned())?;
    let alignment = u16::try_from(target.pointer_alignment)
        .map_err(|_| "target function-pointer alignment exceeds boundary shape range".to_owned())?;
    if byte_size == 0 || alignment == 0 || !alignment.is_power_of_two() {
        return Err(format!(
            "target function-pointer geometry {}/{} is invalid for a direct callback parameter",
            target.pointer_size, target.pointer_alignment,
        ));
    }

    let mut demands = Vec::with_capacity(signature.direct_callback_parameters.len());
    for direct in &signature.direct_callback_parameters {
        validate_fresh_native_parameter_report_identity(
            &signature.native_parameters,
            direct.identity,
            direct.native_ordinal,
        )?;
        signature.native_parameters.push(BoundaryNativeParameter {
            identity: direct.identity,
            native_ordinal: direct.native_ordinal,
            shape: BoundaryNativeParameterShape::TargetFunctionPointer {
                byte_size,
                alignment,
            },
            origin: BoundaryNativeParameterOrigin::PrivateCallback {
                binder: direct.binder,
                requirement: direct.requirement,
            },
            layout_data_symbol: symbols::SymbolHandle::invalid(),
        });
        demands.push(NativeCallbackDemand {
            destination: NativePlace::Parameter(direct.identity),
            requirement: direct.requirement,
        });
    }
    signature
        .native_parameters
        .sort_unstable_by_key(|parameter| parameter.native_ordinal);
    for (expected, parameter) in signature.native_parameters.iter().enumerate() {
        if usize::try_from(parameter.native_ordinal).ok() != Some(expected) {
            return Err(format!(
                "native parameter telescope has position {} where contiguous position {expected} is required",
                parameter.native_ordinal,
            ));
        }
        if matches!(
            parameter.shape,
            BoundaryNativeParameterShape::TargetFunctionPointer { .. }
        ) && (byte_size != 8 || alignment != 8)
        {
            // The current source-level closed vocabulary has only admitted
            // 64-bit native targets. Keep unsupported future pointer geometry
            // fail-closed until its scalar ABI carrier is present.
            return Err(format!(
                "direct callback parameters currently require an admitted 8/8 target function-pointer shape, got {byte_size}/{alignment}"
            ));
        }
    }
    Ok(demands)
}

pub(crate) fn validate_fresh_native_parameter_report_identity(
    prior: &[BoundaryNativeParameter],
    identity: NativeParameterId,
    ordinal: u32,
) -> Result<(), String> {
    if prior.iter().any(|parameter| parameter.identity == identity) {
        return Err(format!(
            "boundary runtime parameter {ordinal} collides with an earlier exact parameter on one compact native-parameter report identity"
        ));
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

/// Canonical source-free projection shared by initial policy evaluation and
/// callback closure. Admission recomputes this payload against an independently
/// selected method commitment; this constructor grants no receiving authority.
pub(super) fn source_free_application(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
) -> Result<calling_conventions::BoundaryCallingApplication, String> {
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
    let mut callback_layouts = arena::Arena::with_capacity(signature.callback_layout_catalog.len());
    let mut callback_layout_rows = arena::HandleSpan::empty();
    for entry in &signature.callback_layout_catalog {
        callback_layouts.append_to_span(&mut callback_layout_rows, entry.source_free()?);
    }
    Ok(calling_conventions::BoundaryCallingApplication {
        requirement_identity: signature.owner_requirement_identity().to_owned(),
        target: signature.native_target(), report_fingerprint: 0, commitment: [0;32],
        plan: validated.clone(),
        signature: calling_conventions::BoundarySignatureGeometry {
            shapes: signature.shapes().to_vec(), fields: signature.fields().to_vec(),
            parameters: signature.parameters().to_vec(), result: signature.result(),
            native_parameters, native_parameter_rows, callback_layouts, callback_layout_rows,
            callback_bindings, callback_binding_rows,
        },
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
        callback_context: callback_materialization_context(signature),
        callback_context_closed: false,
    })
}
