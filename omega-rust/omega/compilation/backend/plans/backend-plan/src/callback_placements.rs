//! Checked callback selections and target-owned ABI placements.

use calling_conventions::{
    BoundaryEntryPlan, CallbackMaterializationContext, CallbackRequirementId,
    NativeParameterApplication, NativePlace, StaticMachineBinderId,
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

/// The registrar telescope row that carries one private callback slot, in the
/// custody its destination kind owns. The variant is exactly the producer's
/// destination match, so a consumer reads which row is live instead of
/// re-deriving custody from independent optional fields.
///
/// This is neither the destination place nor the registrar's outbound
/// telescope; [`BoundCallbackPrivateMaterialization::destination`] stays
/// beside it, and the native lowerer replays the plan join independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivateCallbackDestinationApplication {
    /// A direct parameter destination: the private slot occupies the callback
    /// parameter's own location.
    Direct {
        application: NativeParameterApplication,
    },
    /// A field destination: the slot occupies `byte_offset` bytes into the
    /// record reached through the root record parameter's location.
    Field {
        root: NativeParameterApplication,
        byte_offset: usize,
    },
}

impl PrivateCallbackDestinationApplication {
    /// The application the destination binds: the callback parameter itself
    /// for a direct destination, the root record parameter for a field one.
    pub fn application(&self) -> &NativeParameterApplication {
        match self {
            Self::Direct { application } => application,
            Self::Field { root, .. } => root,
        }
    }

    /// The private slot's byte offset inside the bound record. A direct
    /// destination's slot is the parameter's own location and names no
    /// interior offset.
    pub fn byte_offset(&self) -> Option<usize> {
        match self {
            Self::Direct { .. } => None,
            Self::Field { byte_offset, .. } => Some(*byte_offset),
        }
    }
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
    /// The exact registrar telescope row carrying this private slot, resolved
    /// by the producer from the target-closed registrar telescope it retains
    /// in `registrar_boundary_entry_plan`.
    pub destination_application: PrivateCallbackDestinationApplication,
    pub context: CallbackMaterializationContext,
}
