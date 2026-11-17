//! Translate the selected entry adapter and record its exact call relocation.
use calling_conventions::StateEffectsEvidence;
use physical_planning::native_entry_bridge::{
    InstalledInterruptEntryPlan, NativeEntryBridgeError as Error, NativeEntryBridgePlan,
    NativeEntryBridgeRequest, call_stack_bias,
};
use target::Architecture;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntryBridge {
    request: NativeEntryBridgeRequest,
    bytes: Vec<u8>,
    frame_bytes: u32,
    call_field_offset: usize,
    installed_interrupt: Option<InstalledInterruptEntryPlan>,
    adapter_effects: StateEffectsEvidence,
    complete_effects: StateEffectsEvidence,
}

impl NativeEntryBridge {
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
            Architecture::X86_64 => isa_x86_64::decode_x86_64_generated_entry(bytes, call),
            Architecture::Aarch64 => isa_aarch64::decode_aarch64_generated_entry(bytes, call),
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

pub fn encode_native_entry_bridge(plan: NativeEntryBridgePlan) -> Result<NativeEntryBridge, Error> {
    let (bytes, call_field_offset) =
        crate::adapters::encode_instructions(plan.instructions()).map_err(|_| Error::Encoding)?;
    let frame_bytes = plan.frame_bytes();
    let installed_interrupt = plan.installed_interrupt();
    let (request, adapter_effects, complete_effects) = plan.into_contract();
    Ok(NativeEntryBridge {
        request,
        bytes,
        frame_bytes,
        call_field_offset,
        installed_interrupt,
        adapter_effects,
        complete_effects,
    })
}
