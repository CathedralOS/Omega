//! Checked callback selections and target-owned ABI placements.

use calling_conventions::{
    BoundaryEntryPlan, CallbackMaterializationContext, CallbackRequirementId,
    NativeParameterApplication, NativePlace, PlanDiagnostic, StaticMachineBinderId,
};
use checked_trees::NominalMachineUseSite;
use symbols::SymbolHandle;

/// Target-owned callback recipe joined to one admitted nominal machine use.
///
/// The checked program owns the semantic admission and retains a compact
/// report coordinate. This row carries the exact validated plan past
/// orchestration so thunk lowering never has to rediscover ABI placement from
/// names, types, or a convention oracle. Authority never rests on the compact
/// coordinate alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundNominalCallbackPlacement {
    /// Joins the portable callback application in this compilation's Terminal artifact.
    pub application_index: u32,
    pub site: NominalMachineUseSite,
    pub registration_operation: SymbolHandle,
    pub static_machine_ordinal: u32,
    pub selected_machine: SymbolHandle,
    pub selected_entry: SymbolHandle,
    pub satisfaction_trait: SymbolHandle,
    pub satisfaction_requirement: SymbolHandle,
    pub canonical_requirement_overload: String,
    /// Non-authoritative compatibility/report coordinate beside the exact
    /// `boundary_entry_plan` below.
    pub boundary_calling_plan_report_fingerprint: u64,
    /// Exact checked per-entry resource anchor selected by semantic callback
    /// admission. It remains a derivation receipt only; later resource
    /// realizations must independently join it before admission.
    pub resource_receipt: checked_trees::CheckedCallbackResourceReceipt,
    pub boundary_entry_plan: BoundaryEntryPlan,
    /// Exact target-closed outbound binder/destination row selected by this
    /// nominal callback use. `None` retains the established non-materializing
    /// callback path; a plan containing private rows may not use that form.
    pub private_materialization: Option<BoundCallbackPrivateMaterialization>,
}

/// Complete address-free context and selected row for one private callback
/// materialization, including the retained field offset when applicable.
/// Target operations, bytes, object relocations, runtime storage, registration
/// authority, and leases are absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCallbackPrivateMaterialization {
    pub binder: StaticMachineBinderId,
    pub destination: NativePlace,
    pub requirement: CallbackRequirementId,
    /// Exact target-owned registrar plan whose outbound callback row owns the
    /// binder-to-destination mapping. This is distinct from the callback
    /// handler's inbound `boundary_entry_plan` retained by the placement.
    pub registrar_boundary_entry_plan: BoundaryEntryPlan,
    /// Non-authoritative compatibility/report coordinate beside the exact
    /// registrar plan above.
    pub registrar_calling_plan_report_fingerprint: u64,
    /// Versioned application identity over the exact requirement, ordered
    /// nominal native telescope, callback rows, placements, and physical plan.
    pub registrar_application_report_fingerprint: u64,
    pub registrar_application_commitment: [u8; 32],
    /// Exact target-closed registrar telescope row selected by a direct
    /// parameter destination. Field destinations retain `None`: their root
    /// semantic parameter is not itself a compiler-private callback argument.
    pub direct_registrar_parameter_application: Option<NativeParameterApplication>,
    /// For a field destination, the exact telescope row and evaluated
    /// placement of the record parameter whose pointee receives the private
    /// slot. Direct parameter destinations retain `None`.
    pub root_parameter_application: Option<NativeParameterApplication>,
    /// For a field destination, the private slot's byte offset inside the
    /// root parameter's record. Direct parameter destinations retain `None`.
    pub field_byte_offset: Option<usize>,
    pub context: CallbackMaterializationContext,
}

impl BoundCallbackPrivateMaterialization {
    /// Borrow the destination's exact registrar parameter application.
    ///
    /// Direct destinations apply the callback parameter; field destinations
    /// apply the root record parameter and require a retained byte offset.
    /// Reject mixed or missing custody and identity, ordinal, shape or placement
    /// drift. This does not authenticate the field layout or application
    /// commitment, or replace independent ABI-plan and callback-context replay.
    pub fn destination_parameter_application(
        &self,
    ) -> Result<&NativeParameterApplication, PlanDiagnostic> {
        let (parameter, application) = match &self.destination {
            NativePlace::Parameter(parameter) => {
                if self.root_parameter_application.is_some() || self.field_byte_offset.is_some() {
                    return Err(PlanDiagnostic(
                        "direct callback destination retains field-destination custody".to_owned(),
                    ));
                }
                let application = self.direct_registrar_parameter_application.as_ref().ok_or_else(|| {
                    PlanDiagnostic("direct callback destination lost its exact native parameter application".to_owned())
                })?;
                (parameter, application)
            }
            NativePlace::Field { parameter, .. } => {
                if self.direct_registrar_parameter_application.is_some()
                    || self.field_byte_offset.is_none()
                {
                    return Err(PlanDiagnostic(
                        "field callback destination lacks or mixes parameter-destination custody"
                            .to_owned(),
                    ));
                }
                let application = self.root_parameter_application.as_ref().ok_or_else(|| {
                    PlanDiagnostic(
                        "field callback destination lost its exact root parameter application"
                            .to_owned(),
                    )
                })?;
                (parameter, application)
            }
        };
        let placement = usize::try_from(application.native_ordinal)
            .ok()
            .and_then(|ordinal| {
                self.registrar_boundary_entry_plan
                    .call
                    .parameters
                    .get(ordinal)
            })
            .ok_or_else(|| {
                PlanDiagnostic(
                    "callback registrar native telescope ordinal is absent from its exact plan"
                        .to_owned(),
                )
            })?;
        if application.parameter != *parameter
            || application.shape != application.placement.shape
            || application.placement != *placement
        {
            return Err(PlanDiagnostic(
                "callback registrar native telescope identity, shape, or placement drifted"
                    .to_owned(),
            ));
        }
        Ok(application)
    }
}

/// Independently replay the exact evaluated calling plan retained by one
/// callback placement.
///
/// The source-to-checked join already validates this pair, but backend planning
/// and final emission are separate consumers. Reconstructing the signature
/// from the retained placements and revalidating the complete entry plan keeps
/// either consumer from trusting a copied fingerprint or a noncanonical plan.
pub fn validate_bound_nominal_callback_placement(
    placement: &BoundNominalCallbackPlacement,
) -> Result<calling_conventions::ValidatedBoundaryEntryPlan, calling_conventions::PlanDiagnostic> {
    placement.resource_receipt.validate().map_err(|error| {
        calling_conventions::PlanDiagnostic(format!(
            "callback placement retained an invalid checked resource receipt: {error}"
        ))
    })?;
    if placement.resource_receipt.machine() != placement.selected_machine
        || placement.resource_receipt.entry() != placement.selected_entry
    {
        return Err(calling_conventions::PlanDiagnostic(
            "callback placement resource receipt does not bind its selected machine entry"
                .to_owned(),
        ));
    }
    let signature = calling_conventions::CallSignature {
        parameters: placement
            .boundary_entry_plan
            .call
            .parameters
            .iter()
            .map(|parameter| parameter.shape)
            .collect(),
        result: placement
            .boundary_entry_plan
            .call
            .result
            .as_ref()
            .map(|result| result.shape),
    };
    let validated = calling_conventions::validate_boundary_entry_plan(
        placement.boundary_entry_plan.clone(),
        &signature,
    )?;
    if let Some(materialization) = &placement.private_materialization {
        let registrar_signature = calling_conventions::CallSignature {
            parameters: materialization
                .registrar_boundary_entry_plan
                .call
                .parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: materialization
                .registrar_boundary_entry_plan
                .call
                .result
                .as_ref()
                .map(|result| result.shape),
        };
        let validated_registrar =
            calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                materialization.registrar_boundary_entry_plan.clone(),
                &registrar_signature,
                &materialization.context,
            )?;
        if validated_registrar.plan() != &materialization.registrar_boundary_entry_plan
            || materialization.registrar_calling_plan_report_fingerprint == 0
            || validated_registrar.contract_report_fingerprint()
                != materialization.registrar_calling_plan_report_fingerprint
        {
            return Err(calling_conventions::PlanDiagnostic(
                "callback placement private materialization drifted from its exact evaluated registrar plan"
                    .to_owned(),
            ));
        }
        let matching_binders = materialization
            .context
            .binders
            .iter()
            .filter(|candidate| candidate.binder == materialization.binder)
            .collect::<Vec<_>>();
        let matching_demands = materialization
            .context
            .demands
            .iter()
            .filter(|candidate| candidate.destination == materialization.destination)
            .collect::<Vec<_>>();
        let matching_rows = materialization
            .registrar_boundary_entry_plan
            .call
            .callback_materializations
            .iter()
            .filter(|candidate| candidate.binder == materialization.binder)
            .collect::<Vec<_>>();
        let ([binder], [demand], [row]) = (
            matching_binders.as_slice(),
            matching_demands.as_slice(),
            matching_rows.as_slice(),
        ) else {
            return Err(calling_conventions::PlanDiagnostic(
                    "callback placement private materialization lost its exact binder, demand, or plan row"
                        .to_owned(),
                ));
        };
        if binder.requirement != materialization.requirement
            || demand.requirement != materialization.requirement
            || row.destination != materialization.destination
        {
            return Err(calling_conventions::PlanDiagnostic(
                    "callback placement private materialization binder, requirement, or destination drifted"
                        .to_owned(),
                ));
        }
        materialization.destination_parameter_application()?;
    }
    if validated.plan() != &placement.boundary_entry_plan {
        return Err(calling_conventions::PlanDiagnostic(
            "callback placement retained a noncanonical boundary entry plan".to_owned(),
        ));
    }
    if placement.boundary_calling_plan_report_fingerprint == 0
        || validated.contract_report_fingerprint()
            != placement.boundary_calling_plan_report_fingerprint
    {
        return Err(calling_conventions::PlanDiagnostic(
            "callback placement boundary entry plan drifted from its retained fingerprint"
                .to_owned(),
        ));
    }
    Ok(validated)
}

#[cfg(test)]
pub(crate) mod tests;
