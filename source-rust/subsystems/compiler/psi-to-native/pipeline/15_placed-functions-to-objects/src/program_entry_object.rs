//! Object-relative relocation fields for entries already placed within their own function.
use crate::{NativeEntryBridge, RelocationKind};
use function_layout::program_entry_placement::{PlacedHostedProgram, PlacedPhysicalProgram};
use machine_code::{
    hosted_entry::{EncodedHostedEntry, EntryFixup, EntryFixupKind},
    import_code::EncodedImportProgram,
};
use physical_recipes::physical_stack_switch::StackSwitch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedProgramObject<Context> {
    context: Context,
    entry: EncodedHostedEntry,
    imports: EncodedImportProgram,
    relocations: Vec<RelocationKind>,
}

impl<Context> HostedProgramObject<Context> {
    pub fn from_placement(placement: PlacedHostedProgram<Context>) -> Self {
        let (context, entry, imports) = placement.into_layout().into_parts();
        let relocations = entry
            .fixups
            .iter()
            .map(|field| match field.kind {
                EntryFixupKind::X86Relative32 => RelocationKind::X86_64Relative32,
                EntryFixupKind::Aarch64Branch26 => RelocationKind::Aarch64Branch26,
                EntryFixupKind::Aarch64Page21 => RelocationKind::Aarch64Page21,
                EntryFixupKind::Aarch64PageOffset12 => RelocationKind::Aarch64PageOffset12,
            })
            .collect();
        Self {
            context,
            entry,
            imports,
            relocations,
        }
    }
    pub const fn context(&self) -> &Context {
        &self.context
    }
    pub const fn program(&self) -> &EncodedHostedEntry {
        &self.entry
    }
    pub const fn imports(&self) -> &EncodedImportProgram {
        &self.imports
    }
    pub fn relocations(&self) -> impl ExactSizeIterator<Item = (&EntryFixup, RelocationKind)> {
        self.entry
            .fixups
            .iter()
            .zip(self.relocations.iter().copied())
    }
}

pub struct PhysicalProgramObject<Context> {
    context: Context,
    bridge: NativeEntryBridge,
    switch: StackSwitch,
}

impl<Context> PhysicalProgramObject<Context> {
    pub fn from_placement(placement: PlacedPhysicalProgram<Context>) -> Self {
        let (context, bridge, switch) = placement.into_layout().into_parts();
        Self {
            context,
            bridge: NativeEntryBridge::from_call_field(bridge),
            switch,
        }
    }
    pub fn into_parts(self) -> (Context, NativeEntryBridge, StackSwitch) {
        (self.context, self.bridge, self.switch)
    }
}
