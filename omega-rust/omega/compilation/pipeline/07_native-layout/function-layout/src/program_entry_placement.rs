//! Resolve the layout of each declared entry field before the object assigns its relocation.
use diagnostics::Diagnostic;
use machine_code::{
    hosted_entry::{EncodedHostedEntry, EntryFixupKind, EntrySymbol},
    import_code::EncodedImportProgram,
};
use native_verification::bridges::{EncodedHostedProgram, EncodedPhysicalProgram};
use physical_recipes::physical_stack_switch::StackSwitch;
use target::Architecture;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedHostedProgram<Context> {
    context: Context,
    entry: EncodedHostedEntry,
    imports: EncodedImportProgram,
}

impl<Context> PlacedHostedProgram<Context> {
    pub fn into_parts(self) -> (Context, EncodedHostedEntry, EncodedImportProgram) {
        (self.context, self.entry, self.imports)
    }
}

pub fn place_hosted_program<Context>(
    encoded: EncodedHostedProgram<Context>,
) -> Result<PlacedHostedProgram<Context>, Diagnostic> {
    let (context, entry, imports) = encoded.into_parts();
    let invalid = || {
        Diagnostic::error("hosted entry has a misaligned, overlapping or unresolved fixup field")
    };
    let mut previous_end = 0;
    for fixup in &entry.fixups {
        let end = fixup.offset.checked_add(4).ok_or_else(invalid)?;
        let field: [u8; 4] = entry
            .bytes
            .get(fixup.offset..end)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(invalid)?;
        let word = u32::from_le_bytes(field);
        let admissible = match (entry.target.architecture, fixup.kind) {
            (Architecture::X86_64, EntryFixupKind::X86Relative32) => word == 0,
            (Architecture::Aarch64, kind) if fixup.offset.is_multiple_of(4) => match kind {
                EntryFixupKind::Aarch64Branch26 => word == 0x9400_0000,
                EntryFixupKind::Aarch64Page21 => word & !31 == 0x9000_0000,
                EntryFixupKind::Aarch64PageOffset12 => word & !1023 == 0x9100_0000,
                EntryFixupKind::X86Relative32 => false,
            },
            _ => false,
        };
        if !admissible
            || previous_end > fixup.offset
            || matches!(fixup.destination, EntrySymbol::Import(index) if index >= entry.imports.len())
        {
            return Err(invalid());
        }
        previous_end = end;
    }
    Ok(PlacedHostedProgram {
        context,
        entry,
        imports,
    })
}

pub struct PlacedPhysicalProgram<Context> {
    context: Context,
    bridge: crate::entry_placement::PlacedNativeEntryBridge,
    switch: StackSwitch,
}

impl<Context> PlacedPhysicalProgram<Context> {
    pub fn into_parts(
        self,
    ) -> (
        Context,
        crate::entry_placement::PlacedNativeEntryBridge,
        StackSwitch,
    ) {
        (self.context, self.bridge, self.switch)
    }
}

pub fn place_physical_program<Context>(
    encoded: EncodedPhysicalProgram<Context>,
) -> Result<PlacedPhysicalProgram<Context>, Diagnostic> {
    let (context, bridge, switch) = encoded.into_parts();
    switch.validate_bridge(&bridge)?;
    let mut previous_end = 0;
    for (_, fixup) in switch.fixups.iter() {
        let end = fixup
            .field
            .checked_add(4)
            .ok_or_else(|| Diagnostic::error("physical stack switch fixup overflows"))?;
        if fixup.field < previous_end || switch.bytes.get(fixup.field..end) != Some(&[0; 4]) {
            return Err(Diagnostic::error(
                "physical stack switch fixup is outside the unresolved instruction field",
            ));
        }
        previous_end = end;
    }
    let bridge = crate::entry_placement::place_native_entry_bridge(bridge)
        .map_err(|error| Diagnostic::error(error.to_string()))?;
    Ok(PlacedPhysicalProgram {
        context,
        bridge,
        switch,
    })
}
