//! Checked callback selections and target-owned ABI placements.

use calling_conventions::{
    BoundaryEntryPlan, CallbackMaterializationContext, CallbackRequirementId,
    NativeParameterApplication, NativePlace, PlanDiagnostic, StaticMachineBinderId,
};

/// Target-owned ABI recipe for one portable Terminal callback application.
/// Source selection and resource checking finish before this record is produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCallbackPlacement {
    pub application_index: u32,
    pub static_machine_ordinal: u32,
    pub canonical_requirement_overload: String,
    pub contract_commitment: [u8; 32],
    pub boundary_entry_plan: BoundaryEntryPlan,
    /// Ordered nominal native rows of the callback's inbound requirement,
    /// including origins. This is not the registrar's outbound telescope.
    pub entry_parameters: Vec<NativeCallbackEntryParameter>,
    pub private_materialization: Option<BoundCallbackPrivateMaterialization>,
}

/// One admitted callback ingress row. Keep the nominal parameter, physical
/// recipe and authored origin together for the eventual internal ABI bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCallbackEntryParameter {
    pub application: NativeParameterApplication,
    pub origin: calling_conventions::BoundaryNativeParameterOrigin,
}

/// Complete address-free context and selected row for one private callback
/// materialization, including the retained field offset when applicable.
/// Target operations, bytes, object relocations, runtime storage, registration
/// authority, and leases are absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCallbackPrivateMaterialization {
    /// Whether the exact registrar requirement permits this callback service
    /// during the registration call. False denotes no same-stack edge, not
    /// permission to invoke the callback later.
    pub registrar_invokes_callback: bool,
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
    pub registrar_application: calling_conventions::BoundaryCallingApplication,
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
