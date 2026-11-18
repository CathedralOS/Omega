//! Calling contract and physical instructions for an inbound entry bridge.
use arena::Arena;
use calling_conventions::{BoundaryEntryPlan, CallPlan, StateEffectsEvidence};
use post_allocation_machine::adapters::AdapterInstructions;
pub use runtime_abi::static_interrupts::{InstalledInterruptEntryPlan, InstalledInterruptFrame};
use target::{Architecture, NativeTarget};
pub type NativeEntryBridgePlan<Origin> = crate::PhysicalRecipe<Origin, NativeEntryInstructions>;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeEntryParameter {
    pub inbound_ordinal: u32,
    pub scalar_codec: u64, // Zero means no descriptor conversion; nonzero is a closed scalar codec.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntryBridgeRequest {
    pub target: NativeTarget,
    pub internal: CallPlan,
    pub inbound: BoundaryEntryPlan, // Owns both argument placement and interrupted-state obligations.
    pub body_footprint: Option<StateEffectsEvidence>, // Checked body closure required for asynchronous entry ceilings.
    pub parameters: Arena<NativeEntryParameter>, // Internal scalar-then-structural order joined through nominal origins.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntryInstructions {
    pub request: NativeEntryBridgeRequest,
    pub instructions: AdapterInstructions,
    pub effects: StateEffectsEvidence,
    pub complete_effects: StateEffectsEvidence,
    pub frame_bytes: u32,
    pub installed_interrupt: Option<InstalledInterruptEntryPlan>,
}

impl<Origin> NativeEntryBridgePlan<Origin> {
    pub fn instructions(&self) -> &AdapterInstructions {
        &self.instructions.instructions
    }
    pub fn into_contract(
        self,
    ) -> (
        NativeEntryBridgeRequest,
        StateEffectsEvidence,
        StateEffectsEvidence,
    ) {
        (
            self.instructions.request,
            self.instructions.effects,
            self.instructions.complete_effects,
        )
    }

    pub fn effects(&self) -> &StateEffectsEvidence {
        &self.instructions.effects
    }
    pub fn complete_effects(&self) -> &StateEffectsEvidence {
        &self.instructions.complete_effects
    }
    pub fn request(&self) -> &NativeEntryBridgeRequest {
        &self.instructions.request
    }
    pub fn frame_bytes(&self) -> u32 {
        self.instructions.frame_bytes
    }
    pub fn call_stack_bias(&self) -> u32 {
        call_stack_bias(self.instructions.request.target.architecture)
    }
    pub fn installed_interrupt(&self) -> Option<InstalledInterruptEntryPlan> {
        self.instructions.installed_interrupt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeEntryBridgeError {
    IncompatiblePlan,
    UnsupportedEntryControl,
    UnsupportedInstalledInterruptSignature,
    UnsupportedFatalInterruptSignature,
    UnsupportedFatalInterruptPreservation,
    InvalidInstalledInterruptGeometry,
    UnsupportedInterruptState,
    MissingInterruptBodyFootprint,
    InterruptBodyStateExceedsPlan,
    FatalInterruptBodyStateExceedsPlan,
    InvalidParameterJoin,
    UnsupportedValueTransport,
    MissingScalarCodec,
    NoUnoccupiedScratch,
    FrameOverflow,
    Encoding,
    ReplayMismatch,
    GeneratedEntryEffects(String),
}

impl std::fmt::Display for NativeEntryBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self {
            Self::GeneratedEntryEffects(reason) => {
                return write!(formatter, "native entry bridge failed: {reason}");
            }
            Self::UnsupportedInstalledInterruptSignature => {
                "installed timer arrival requires exactly one owned 40-byte acknowledgement and no result"
            }
            Self::UnsupportedFatalInterruptSignature => {
                "installed fatal arrival requires exactly one shared context reference and no result"
            }
            Self::UnsupportedFatalInterruptPreservation => {
                "nonreturning fatal entry cannot promise to preserve or restore an interrupted continuation"
            }
            Self::InvalidInstalledInterruptGeometry => {
                "installed interrupt entry changed its root, gate or hardware-frame geometry"
            }
            Self::UnsupportedEntryControl => "this target cannot emit the requested entry return",
            Self::UnsupportedInterruptState => {
                "interrupt entry requires the supported x86-64 integer-register save/restore plan and 16-byte stack alignment"
            }
            Self::MissingInterruptBodyFootprint => {
                "interrupt entry lacks the checked handler's machine-state effects"
            }
            Self::InterruptBodyStateExceedsPlan => {
                "interrupt handler uses machine state its entry does not permit, save and restore"
            }
            Self::FatalInterruptBodyStateExceedsPlan => {
                "fatal interrupt handler exceeds its entry's machine-state ceiling"
            }
            _ => return write!(formatter, "native entry bridge failed: {self:?}"),
        };
        write!(formatter, "native entry bridge failed: {reason}")
    }
}

impl std::error::Error for NativeEntryBridgeError {}
type Error = NativeEntryBridgeError;

impl NativeEntryBridgeRequest {
    pub fn checked_complete_effects(
        &self,
        adapter: &StateEffectsEvidence,
    ) -> Result<StateEffectsEvidence, Error> {
        let body = self
            .body_footprint
            .as_ref()
            .ok_or(Error::MissingInterruptBodyFootprint)?;
        let complete = adapter.compose(body);
        complete.validate(&self.inbound.state).map_err(|reason| {
            Error::GeneratedEntryEffects(format!(
                "{reason}: used {:?}, permitted {:?}, clobbered {:?}, saved {:?}",
                complete.machine_state(),
                self.inbound.state.permitted_transitive_use,
                complete.clobbered_state(),
                self.inbound.state.saved_state
            ))
        })?;
        Ok(complete)
    }
}

pub fn call_stack_bias(architecture: Architecture) -> u32 {
    u32::from(architecture == Architecture::X86_64) * 8
}

/// Bytes and relocation metadata for inspection. Admission is carried by the owning encoded or placed bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntryBridgeCode {
    request: NativeEntryBridgeRequest,
    bytes: Vec<u8>,
    frame_bytes: u32,
    call_field_offset: usize,
    installed_interrupt: Option<InstalledInterruptEntryPlan>,
    adapter_effects: StateEffectsEvidence,
    complete_effects: StateEffectsEvidence,
}

impl NativeEntryBridgeCode {
    pub fn from_recipe<Origin>(
        plan: NativeEntryBridgePlan<Origin>,
        bytes: Vec<u8>,
        call_field_offset: usize,
    ) -> Self {
        let frame_bytes = plan.frame_bytes();
        let installed_interrupt = plan.installed_interrupt();
        let (request, adapter_effects, complete_effects) = plan.into_contract();
        Self {
            request,
            bytes,
            frame_bytes,
            call_field_offset,
            installed_interrupt,
            adapter_effects,
            complete_effects,
        }
    }

    pub fn adapter_effects(&self) -> &StateEffectsEvidence {
        &self.adapter_effects
    }

    // Container relocation may change only the declared body-call field. Decode
    // final bytes independently so a carried effect claim cannot authorize an
    // unaccounted instruction or control transfer.
    pub fn validate_published_code(&self, bytes: &[u8]) -> Result<(), Error> {
        let field = self.call_field_offset;
        let end = field.checked_add(4).ok_or(Error::ReplayMismatch)?;
        if bytes.len() != self.bytes.len()
            || bytes.get(..field) != self.bytes.get(..field)
            || bytes.get(end..) != self.bytes.get(end..)
        {
            return Err(Error::ReplayMismatch);
        }
        if self.request.target.architecture == Architecture::Aarch64 {
            let word = u32::from_le_bytes(
                bytes
                    .get(field..end)
                    .ok_or(Error::ReplayMismatch)?
                    .try_into()
                    .map_err(|_| Error::ReplayMismatch)?,
            );
            if word & 0xfc00_0000 != 0x9400_0000 {
                return Err(Error::ReplayMismatch);
            }
        }
        let call = Some(u32::try_from(field).map_err(|_| Error::FrameOverflow)?);
        let decoded = match self.request.target.architecture {
            Architecture::X86_64 => target_x86_64::decode_x86_64_generated_entry(bytes, call),
            Architecture::Aarch64 => target_aarch64::decode_aarch64_generated_entry(bytes, call),
        }
        .map_err(Error::GeneratedEntryEffects)?;
        if decoded.effects != self.adapter_effects
            || self.request.checked_complete_effects(&decoded.effects)? != self.complete_effects
        {
            return Err(Error::ReplayMismatch);
        }
        Ok(())
    }
    pub fn complete_effects(&self) -> &StateEffectsEvidence {
        &self.complete_effects
    }
    pub fn installed_interrupt(&self) -> Option<InstalledInterruptEntryPlan> {
        self.installed_interrupt
    }
    pub fn request(&self) -> &NativeEntryBridgeRequest {
        &self.request
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn frame_bytes(&self) -> u32 {
        self.frame_bytes
    }
    pub fn call_field_offset(&self) -> usize {
        self.call_field_offset
    }
    pub fn call_stack_bias(&self) -> u32 {
        call_stack_bias(self.request.target.architecture)
    }
}
