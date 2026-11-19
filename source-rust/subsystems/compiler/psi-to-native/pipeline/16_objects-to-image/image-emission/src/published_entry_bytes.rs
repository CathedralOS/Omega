//! Verify entry instructions after the object or image has applied relocations.
use diagnostics::Diagnostic;
use physical_recipes::native_entry_bridge::{
    NativeEntryBridgeCode, NativeEntryBridgeError as Error,
};
use physical_recipes::physical_stack_switch::StackSwitch;
use target::Architecture;

/// Check the published instructions and their state effects. Relocation may
/// change the entry bridge only at its declared call to the body.
pub fn verify_entry_bridge_bytes(
    bridge: &NativeEntryBridgeCode,
    bytes: &[u8],
) -> Result<(), Error> {
    let field = bridge.call_field_offset();
    let end = field.checked_add(4).ok_or(Error::ReplayMismatch)?;
    if bytes.len() != bridge.bytes().len()
        || bytes.get(..field) != bridge.bytes().get(..field)
        || bytes.get(end..) != bridge.bytes().get(end..)
    {
        return Err(Error::ReplayMismatch);
    }
    if bridge.request().target.architecture == Architecture::Aarch64 {
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
    let decoded = match bridge.request().target.architecture {
        Architecture::X86_64 => target_x86_64::decode_x86_64_generated_entry(bytes, call),
        Architecture::Aarch64 => target_aarch64::decode_aarch64_generated_entry(bytes, call),
    }
    .map_err(Error::GeneratedEntryEffects)?;
    if decoded.effects != *bridge.adapter_effects()
        || bridge
            .request()
            .checked_complete_effects(&decoded.effects)?
            != *bridge.complete_effects()
    {
        return Err(Error::ReplayMismatch);
    }
    Ok(())
}

pub(crate) fn verify_stack_switch_bytes(
    switch: &StackSwitch,
    bytes: &[u8],
) -> Result<(), Diagnostic> {
    let invalid = |reason: &str| Diagnostic::error(format!("physical image entry: {reason}"));
    let changed = || invalid("physical entry changed outside its declared relocation fields");
    if bytes.len() != switch.bytes.len() {
        return Err(changed());
    }
    let mut cursor = 0;
    for (_, fixup) in switch.fixups.iter() {
        let end = fixup.field.checked_add(4).ok_or_else(changed)?;
        if cursor > fixup.field
            || end > bytes.len()
            || bytes[cursor..fixup.field] != switch.bytes[cursor..fixup.field]
        {
            return Err(changed());
        }
        cursor = end;
    }
    if switch.bytes[cursor..] != bytes[cursor..] {
        return Err(changed());
    }
    // Compare decoded control flow as well as the planned state effects.
    // Relocation may change an address while preserving both.
    let expected =
        target_x86_64::decode_x86_64_generated_entry(&switch.bytes, Some(switch.body_call))
            .map_err(|error| invalid(&error))?;
    let actual = target_x86_64::decode_x86_64_generated_entry(bytes, Some(switch.body_call))
        .map_err(|error| invalid(&error))?;
    if actual != expected || actual.effects != switch.effects {
        return Err(invalid(
            "physical entry instruction/state evidence fails exact target replay",
        ));
    }
    Ok(())
}
