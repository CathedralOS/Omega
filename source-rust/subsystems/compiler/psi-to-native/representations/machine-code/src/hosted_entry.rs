//! Hosted entry fixups and completed bytes used by object placement.
use calling_conventions::StateEffectsEvidence;
use target::NativeTarget;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedEntryMode {
    LoaderStack,
    OwnedStack,
    Receiver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntrySymbol {
    Scratch,
    StackTop,
    PrivateRam(usize),
    ObservationHeader(usize),
    Body,
    Import(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryFixupKind {
    X86Relative32,
    Aarch64Branch26,
    Aarch64Page21,
    Aarch64PageOffset12,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryFixup {
    pub offset: usize,
    pub destination: EntrySymbol,
    pub addend: i64,
    pub kind: EntryFixupKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedHostedEntry {
    pub target: NativeTarget,
    pub mode: HostedEntryMode,
    pub bytes: Vec<u8>,
    pub fixups: Vec<EntryFixup>,
    pub imports: Vec<target::NormalizedForeignLocator>,
    pub effects: StateEffectsEvidence,
}
