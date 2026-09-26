//! Callback bindings: the calling-plan realization, the retained callback
//! binders, nominal callback placement validation and the closure of
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
    ValidatedBoundaryEntryPlan, ValueShape, callback_requirement_id,
    validate_boundary_entry_plan_with_callback_materializations,
};
use diagnostics::Diagnostic;
use representation_planning::OpaqueRepresentationSelection;
use target::NativeTarget;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceHandle;

/// Omega-owned realization state for one canonical source boundary contract.
///
/// Typed trees retain only the semantic key and fingerprint. Concrete ABI,
/// register, and stack choices stay beside the native/provider pipeline that
/// consumes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallingPlanRealization {
    pub boundary_trait: symbols::SymbolHandle,
    pub boundary_arguments: Vec<TypeReferenceHandle>,
    pub requirement_machine: symbols::SymbolHandle,
    /// Compact compatibility/report coordinate for the complete target-closed
    /// application. Exact plan and materialized-signature custody below remain
    /// the authority for realization joins.
    pub report_fingerprint: u64,
    pub commitment: typed_trees::typed_trees::BoundaryCallingPlanCommitment,
    pub boundary_entry_plan: BoundaryEntryPlan,
    /// Crate-sealed exact evidence minted with the validated realization.
    /// The public fingerprint remains a compact compatibility/report
    /// coordinate and cannot authorize replacement of this plan.
    pub(crate) exact_boundary_entry_plan: BoundaryEntryPlan,
    pub callback_binders: Vec<BoundaryCallbackBinder>,
    pub callback_demands: Vec<NativeCallbackDemand>,
    pub callback_context_closed: bool,
    pub native_parameters: Vec<BoundaryNativeParameter>,
    pub(crate) materialized_signature: MaterializedBoundarySignature,
    pub policy_machine: String,
    pub relationship_span: source::SourceSpan,
}

impl BoundaryCallingPlanRealization {
    /// The named layout catalog and ABI signature are published together.
    /// Callers may inspect them but cannot substitute an unhashed catalog.
    pub const fn materialized_signature(&self) -> &MaterializedBoundarySignature {
        &self.materialized_signature
    }

    pub const fn exact_boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        &self.exact_boundary_entry_plan
    }

    pub fn replayed_validated_plan(
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
            typed_trees::typed_trees::BoundaryCallingPlanCommitment,
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
            boundary_plan_application_identity(&self.materialized_signature, &validated);
        Ok((
            validated,
            report_fingerprint,
            typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(commitment),
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
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == binder.requirement_trait)
            .ok_or_else(|| {
                "evaluated callback binder lost its exact requirement trait".to_owned()
            })?;
        let requirement = typed
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|candidate| candidate.symbol == binder.requirement_machine)
            .ok_or_else(|| {
                "evaluated callback binder lost its exact requirement machine".to_owned()
            })?;
        let exact = typed
            .normalized_trait_requirement_overload_identity(trait_definition, requirement)
            .identity();
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

/// Bind checked nominal callback authority back to the one target-owned plan
/// realization that produced its retained fingerprint. This runs before any
/// backend lowering so a missing, duplicated, or changed placement recipe
/// cannot silently fall back to a convention oracle.
pub fn validate_nominal_callback_placement_bindings(
    checked: &checked_trees::CheckedTrees,
    realizations: &[BoundaryCallingPlanRealization],
) -> Result<Vec<backend_plan::BoundNominalCallbackPlacement>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut bound = Vec::new();
    for nominal_use in &checked.facts.nominal_machine_uses.uses {
        let matching = realizations
            .iter()
            .filter(|realization| {
                realization.boundary_trait == nominal_use.satisfaction_trait
                    && realization.requirement_machine == nominal_use.satisfaction_requirement
                    && realization.boundary_arguments.is_empty()
            })
            .collect::<Vec<_>>();
        let Some(placement) = nominal_use.callback_placement else {
            if !matching.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "nominal callback use for `{}` lost its evaluated boundary calling-plan identity",
                    nominal_use.canonical_requirement_overload
                )));
            }
            continue;
        };
        let [realization] = matching.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` resolves to {} target calling-plan realizations; exactly one is required",
                nominal_use.canonical_requirement_overload,
                matching.len()
            )));
            continue;
        };
        let (validated, realized_report_fingerprint, realized_commitment) = match realization
            .replayed_validated_application()
        {
            Ok(application) => application,
            Err(error) => {
                diagnostics.push(Diagnostic::error(format!(
                    "nominal callback use for `{}` retained an invalid target calling-plan realization: {error}",
                    nominal_use.canonical_requirement_overload
                )));
                continue;
            }
        };
        if realization.report_fingerprint != realized_report_fingerprint
            || realization.commitment != realized_commitment
            || realization.exact_boundary_entry_plan() != validated.plan()
            || placement.boundary_calling_plan_report_fingerprint != realized_report_fingerprint
            || placement.boundary_calling_plan_commitment != realized_commitment
        {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` does not bind its exact evaluated target calling plan",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        }
        let Some(resource_envelope) = checked
            .facts
            .contract_plans
            .resource_envelope(nominal_use.selected_machine, nominal_use.selected_entry)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` is missing its exact checked entry resource envelope",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        };
        let Some(actual_contract) = checked
            .facts
            .contract_plans
            .for_machine(nominal_use.selected_machine)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` is missing its canonical selected contract",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        };
        let Some(published_contract) = checked.facts.contract_plans.crash_capsule(
            nominal_use.satisfaction_trait,
            nominal_use.satisfaction_requirement,
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` is missing its canonical published requirement contract",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        };
        if nominal_use
            .published_requirement_envelope
            .contract_report_fingerprint
            != published_contract.target_contract_report_fingerprint()
            || nominal_use
                .published_requirement_envelope
                .contract_commitment
                != published_contract.target_contract_commitment()
        {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` does not bind its exact published requirement contract",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        }
        if placement.resource_receipt.contract_report_fingerprint()
            != nominal_use
                .selected_actual_envelope
                .contract_report_fingerprint
            || placement.resource_receipt.contract_commitment()
                != nominal_use.selected_actual_envelope.contract_commitment
            || nominal_use.selected_actual_envelope.contract_commitment
                != actual_contract.commitment
            || placement
                .resource_receipt
                .validate_against(resource_envelope)
                .is_err()
        {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` does not bind its exact checked entry resource receipt",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        }
        let private_materialization = match bound_private_callback_materialization(
            nominal_use,
            realizations,
        ) {
            Ok(materialization) => materialization,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(format!(
                    "nominal callback use for `{}` could not retain its exact private materialization: {reason}",
                    nominal_use.canonical_requirement_overload
                )));
                continue;
            }
        };
        bound.push(backend_plan::BoundNominalCallbackPlacement {
            site: nominal_use.site,
            registration_operation: nominal_use.registration_operation,
            static_machine_ordinal: nominal_use.static_machine_ordinal,
            selected_machine: nominal_use.selected_machine,
            selected_entry: nominal_use.selected_entry,
            satisfaction_trait: nominal_use.satisfaction_trait,
            satisfaction_requirement: nominal_use.satisfaction_requirement,
            canonical_requirement_overload: nominal_use.canonical_requirement_overload.clone(),
            boundary_calling_plan_report_fingerprint: validated.contract_report_fingerprint(),
            resource_receipt: placement.resource_receipt,
            boundary_entry_plan: validated.plan().clone(),
            private_materialization,
        });
    }

    if diagnostics.is_empty() {
        Ok(bound)
    } else {
        Err(diagnostics)
    }
}

fn bound_private_callback_materialization(
    nominal_use: &checked_trees::CheckedNominalMachineUse,
    realizations: &[BoundaryCallingPlanRealization],
) -> Result<Option<backend_plan::BoundCallbackPrivateMaterialization>, String> {
    let matching_registrars = realizations
        .iter()
        .filter(|realization| {
            realization.requirement_machine == nominal_use.registration_operation
                && realization.callback_binders.iter().any(|binder| {
                    binder.static_machine_ordinal == nominal_use.static_machine_ordinal
                        && binder.requirement_trait == nominal_use.satisfaction_trait
                        && binder.requirement_machine == nominal_use.satisfaction_requirement
                })
        })
        .collect::<Vec<_>>();
    let [registrar] = matching_registrars.as_slice() else {
        return Err(format!(
            "registration operation and exact satisfaction row resolve to {} outbound registrar realizations; exactly one is required",
            matching_registrars.len()
        ));
    };
    validate_retained_callback_binders(registrar)?;

    let matching_binders = registrar
        .callback_binders
        .iter()
        .filter(|binder| {
            binder.static_machine_ordinal == nominal_use.static_machine_ordinal
                && binder.requirement_trait == nominal_use.satisfaction_trait
                && binder.requirement_machine == nominal_use.satisfaction_requirement
        })
        .collect::<Vec<_>>();
    let [binder] = matching_binders.as_slice() else {
        return Err(format!(
            "static-machine ordinal {} and exact satisfaction row resolve to {} binder identities in the selected registrar realization; exactly one is required",
            nominal_use.static_machine_ordinal,
            matching_binders.len()
        ));
    };
    if !registrar.callback_context_closed {
        if !registrar
            .boundary_entry_plan
            .call
            .callback_materializations
            .is_empty()
        {
            return Err(
                "an unclosed target context retained private callback materialization rows"
                    .to_owned(),
            );
        }
        let (validated, report_fingerprint, commitment) = registrar
            .replayed_validated_application()
            .map_err(|error| error.to_string())?;
        if validated.plan() != &registrar.boundary_entry_plan
            || validated.plan() != registrar.exact_boundary_entry_plan()
            || report_fingerprint != registrar.report_fingerprint
            || commitment != registrar.commitment
        {
            return Err(
                "the exact outbound registrar realization drifted from its retained fingerprint"
                    .to_owned(),
            );
        }
        return Ok(None);
    }

    let context = CallbackMaterializationContext {
        binders: registrar
            .callback_binders
            .iter()
            .map(|binder| CallbackBinderRequirement {
                binder: binder.binder,
                requirement: binder.requirement,
            })
            .collect(),
        demands: registrar.callback_demands.clone(),
    };
    let (validated, application_report_fingerprint, commitment) = registrar
        .replayed_validated_application()
        .map_err(|error| error.to_string())?;
    if validated.plan() != &registrar.boundary_entry_plan
        || validated.plan() != registrar.exact_boundary_entry_plan()
        || application_report_fingerprint != registrar.report_fingerprint
        || commitment != registrar.commitment
    {
        return Err(
            "the exact outbound registrar realization drifted from its retained fingerprint"
                .to_owned(),
        );
    }
    let matching_rows = registrar
        .boundary_entry_plan
        .call
        .callback_materializations
        .iter()
        .filter(|row| row.binder == binder.binder)
        .collect::<Vec<_>>();
    let [row] = matching_rows.as_slice() else {
        return Err(format!(
            "exact binder identity resolves to {} private materialization rows; exactly one is required",
            matching_rows.len()
        ));
    };
    let matching_demands = registrar
        .callback_demands
        .iter()
        .filter(|demand| {
            demand.destination == row.destination && demand.requirement == binder.requirement
        })
        .collect::<Vec<_>>();
    let [demand] = matching_demands.as_slice() else {
        return Err(format!(
            "exact binder destination and callback requirement resolve to {} private demands; exactly one is required",
            matching_demands.len()
        ));
    };
    let direct_registrar_parameter_application = match row.destination {
        NativePlace::Parameter(parameter) => {
            let matching_parameters = registrar
                .native_parameters
                .iter()
                .filter(|candidate| candidate.identity == parameter)
                .collect::<Vec<_>>();
            let [native_parameter] = matching_parameters.as_slice() else {
                return Err(format!(
                    "direct callback destination resolves to {} target-closed native parameter rows; exactly one is required",
                    matching_parameters.len(),
                ));
            };
            if native_parameter.origin
                != (BoundaryNativeParameterOrigin::PrivateCallback {
                    binder: binder.binder,
                    requirement: binder.requirement,
                })
            {
                return Err(
                    "direct callback native parameter is not owned by its exact binder and requirement"
                        .to_owned(),
                );
            }
            let BoundaryNativeParameterShape::TargetFunctionPointer {
                byte_size,
                alignment,
            } = native_parameter.shape
            else {
                return Err(
                    "direct callback native parameter does not retain a target function-pointer shape"
                        .to_owned(),
                );
            };
            let shape = ValueShape::integer(byte_size, alignment);
            let placement = usize::try_from(native_parameter.native_ordinal)
                .ok()
                .and_then(|ordinal| validated.plan().call.parameters.get(ordinal))
                .ok_or_else(|| {
                    "direct callback native parameter ordinal is absent from the exact registrar plan"
                        .to_owned()
                })?;
            if placement.shape != shape {
                return Err(
                    "direct callback native parameter shape drifted from its exact registrar placement"
                        .to_owned(),
                );
            }
            Some(calling_conventions::NativeParameterApplication {
                parameter,
                native_ordinal: native_parameter.native_ordinal,
                shape,
                placement: placement.clone(),
            })
        }
        NativePlace::Field { .. } => None,
    };
    let application_commitment = commitment.as_bytes();
    Ok(Some(backend_plan::BoundCallbackPrivateMaterialization {
        binder: binder.binder,
        destination: row.destination.clone(),
        requirement: demand.requirement,
        registrar_boundary_entry_plan: validated.plan().clone(),
        registrar_calling_plan_report_fingerprint: validated.contract_report_fingerprint(),
        registrar_application_report_fingerprint: application_report_fingerprint,
        registrar_application_commitment: application_commitment,
        direct_registrar_parameter_application,
        context,
    }))
}

/// Re-evaluate outbound registrar policies only after the checked native
/// layout pipeline has published its authoritative private-demand catalog.
/// The target-neutral typed reports are deliberately not consulted here.
pub fn close_outbound_callback_materializations(
    checked: &mut checked_trees::CheckedTrees,
    realizations: &mut [BoundaryCallingPlanRealization],
    native_target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<(), Vec<Diagnostic>> {
    let layout_plan =
        layout::build_layout_plan(checked, native_target, opaque_representation_selections)
            .map_err(|diagnostic| vec![diagnostic])?;
    let admission = build_time_evaluation::BuildTimeAdmissionPlan::infer(
        &checked.typed,
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
            exact_callback_requirement_catalog(&checked.typed, &realization.callback_binders)
                .map_err(|reason| {
                    vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
                })?;
        let mut demand_requirement_catalog = Vec::new();
        for parameter in &signature.native_parameters {
            for demand in layout_plan
                .private_callback_demands
                .iter()
                .filter(|demand| demand.data_symbol == parameter.layout_data_symbol)
            {
                validate_callback_requirement_report_collision(
                    &binder_requirement_catalog,
                    demand.requirement,
                    &demand.callback_requirement_identity,
                )
                .and_then(|()| {
                    validate_callback_requirement_report_collision(
                        &demand_requirement_catalog,
                        demand.requirement,
                        &demand.callback_requirement_identity,
                    )
                })
                .map_err(|reason| {
                    vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
                })?;
                demand_requirement_catalog.push((
                    demand.requirement,
                    demand.callback_requirement_identity.to_string(),
                ));
                let entry = BoundaryCallbackLayoutEntry::direct(parameter, &layout_plan, demand)
                    .map_err(|reason| {
                        vec![
                            Diagnostic::error(reason)
                                .with_source_span(realization.relationship_span),
                        ]
                    })?;
                demands.push((demand.native_demand(parameter.identity), Some(entry)));
            }
            for path in layout_plan
                .two_hop_private_callback_paths
                .iter()
                .filter(|path| path.root_layout.data_symbol == parameter.layout_data_symbol)
            {
                validate_callback_requirement_report_collision(
                    &binder_requirement_catalog,
                    path.terminal_demand.requirement,
                    &path.terminal_demand.callback_requirement_identity,
                )
                .and_then(|()| {
                    validate_callback_requirement_report_collision(
                        &demand_requirement_catalog,
                        path.terminal_demand.requirement,
                        &path.terminal_demand.callback_requirement_identity,
                    )
                })
                .map_err(|reason| {
                    vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
                })?;
                demand_requirement_catalog.push((
                    path.terminal_demand.requirement,
                    path.terminal_demand
                        .callback_requirement_identity
                        .to_string(),
                ));
                let entry =
                    BoundaryCallbackLayoutEntry::two_hop(parameter, path).map_err(|reason| {
                        vec![
                            Diagnostic::error(reason)
                                .with_source_span(realization.relationship_span),
                        ]
                    })?;
                demands.push((path.native_demand(parameter.identity), Some(entry)));
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

        let old_report_fingerprint = realization.report_fingerprint;
        let old_commitment = realization.commitment;
        let (demands, entries): (Vec<_>, Vec<_>) = demands.into_iter().unzip();
        signature.callback_demands = demands.clone();
        signature.callback_layout_catalog = entries.into_iter().flatten().collect();
        let validated = evaluate_materialized_calling_policy_plan(
            &checked.typed,
            &admission,
            &realization.policy_machine,
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
            boundary_plan_application_identity(&signature, &validated);
        realization.report_fingerprint = new_fingerprint;
        realization.commitment =
            typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(new_commitment);
        realization.boundary_entry_plan = validated.plan().clone();
        realization.exact_boundary_entry_plan = validated.plan().clone();
        realization.callback_demands = demands;
        realization.native_parameters = signature.native_parameters.clone();
        realization.callback_context_closed = true;
        realization.materialized_signature = signature;

        checked.typed.record_boundary_calling_plan(
            typed_trees::typed_trees::BoundaryCallingPlanIdentity {
                boundary_trait: realization.boundary_trait,
                boundary_arguments: realization.boundary_arguments.clone(),
                requirement_machine: realization.requirement_machine,
                report_fingerprint: new_fingerprint,
                commitment: realization.commitment,
            },
        );
        reconcile_closed_callback_plan_identity(
            &mut checked.facts.nominal_machine_uses.uses,
            realization.boundary_trait,
            realization.requirement_machine,
            old_report_fingerprint,
            old_commitment,
            new_fingerprint,
            realization.commitment,
        );
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

pub(crate) fn reconcile_closed_callback_plan_identity(
    nominal_uses: &mut [checked_trees::CheckedNominalMachineUse],
    boundary_trait: symbols::SymbolHandle,
    requirement_machine: symbols::SymbolHandle,
    old_report_fingerprint: u64,
    old_commitment: typed_trees::typed_trees::BoundaryCallingPlanCommitment,
    new_report_fingerprint: u64,
    new_commitment: typed_trees::typed_trees::BoundaryCallingPlanCommitment,
) {
    for nominal_use in nominal_uses {
        if nominal_use.satisfaction_trait == boundary_trait
            && nominal_use.satisfaction_requirement == requirement_machine
            && nominal_use.callback_placement.is_some_and(|placement| {
                placement.boundary_calling_plan_report_fingerprint == old_report_fingerprint
                    && placement.boundary_calling_plan_commitment == old_commitment
            })
        {
            let resource_receipt = nominal_use
                .callback_placement
                .expect("matched callback placement")
                .resource_receipt;
            nominal_use.callback_placement =
                Some(checked_trees::CheckedCallbackPlacementIdentity {
                    boundary_calling_plan_report_fingerprint: new_report_fingerprint,
                    boundary_calling_plan_commitment: new_commitment,
                    resource_receipt,
                });
        }
    }
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

/// Compact callback-binder catalog discriminator. The exact binder ordinal,
/// parameter symbol, and requirement symbols remain beside it and collisions
/// reject before a calling policy is evaluated.
pub(crate) fn callback_plan_report_fingerprint(domain: &[u8], parts: &[&[u8]]) -> u64 {
    let mut report_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    for bytes in std::iter::once(domain).chain(parts.iter().copied()) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            report_fingerprint ^= u64::from(byte);
            report_fingerprint = report_fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    if report_fingerprint == 0 {
        1
    } else {
        report_fingerprint
    }
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
