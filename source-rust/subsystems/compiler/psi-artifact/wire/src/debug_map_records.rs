use crate::TerminalPsiIdentity;
use semantic_vocabulary::{
    BlockId, ClaimId, ContractId, EdgeId, MachineId, ObligationId, OperationId, PlaceId,
    ValueId,
};
use std::num::NonZeroU32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DebugFileId(NonZeroU32);

impl DebugFileId {
    pub const fn new(raw: u32) -> Option<Self> {
        match NonZeroU32::new(raw) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DebugSourceDigest([u8; 32]);

impl DebugSourceDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for DebugSourceDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DebugSourceOrigin {
    User,
    Toolchain,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DebugSourceFile {
    pub id: DebugFileId,
    pub origin: DebugSourceOrigin,
    pub byte_len: u64,
    pub digest: DebugSourceDigest,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DebugSourceSpan {
    pub file: DebugFileId,
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DebugSubject {
    Machine(MachineId),
    Block(BlockId),
    Operation(OperationId),
    Edge(EdgeId),
    Value(ValueId),
    Contract(ContractId),
    Obligation(ObligationId),
    Place(PlaceId),
    Claim { machine: MachineId, claim: ClaimId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DebugSite {
    pub subject: DebugSubject,
    pub span: DebugSourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalDebugMap {
    pub semantic: TerminalPsiIdentity,
    pub files: Vec<DebugSourceFile>,
    pub sites: Vec<DebugSite>,
}
