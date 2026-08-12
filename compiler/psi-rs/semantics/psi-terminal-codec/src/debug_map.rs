//! Canonical, replaceable debug/source-map section for terminal Psi.
//!
//! This section is presentation evidence only. It binds itself to one exact
//! semantic identity and may name only identities that actually occur in that
//! module. Replacing it changes the artifact manifest, never terminal-Psi
//! semantic identity.

use std::num::NonZeroU32;

use psi_core::{
    BlockId, ClaimId, ContractId, EdgeId, MachineId, ObligationId, OperationId, PlaceId, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalModule, TerminalPsiIdentity, VocabularyMarker};
use sha2::{Digest, Sha256};

use crate::{CodecError, Reader, Writer, terminal_psi_identity};

const MAGIC: &[u8; 8] = b"PSIDBG\0\0";
const FORMAT_MARKER: u16 = 1;
const SOURCE_DIGEST_DOMAIN: &[u8] = b"psi-terminal-debug-source\0";

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
    /// Presentation path. It is deliberately not part of semantic identity.
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DebugSourceSpan {
    pub file: DebugFileId,
    pub start: u64,
    pub end: u64,
}

/// A stable semantic subject that may be associated with one source span.
/// Claim identities are machine-local and therefore retain their machine.
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
    /// Strictly ordered by nonzero file identity.
    pub files: Vec<DebugSourceFile>,
    /// Strictly ordered by semantic subject; at most one primary span per subject.
    pub sites: Vec<DebugSite>,
}

pub fn source_digest(source: &[u8]) -> DebugSourceDigest {
    let mut digest = Sha256::new();
    digest.update(SOURCE_DIGEST_DOMAIN);
    digest.update(
        u64::try_from(source.len())
            .expect("source bytes fit the debug digest domain")
            .to_le_bytes(),
    );
    digest.update(source);
    DebugSourceDigest(digest.finalize().into())
}

pub fn encode_debug_map(
    module: &TerminalModule,
    debug_map: &TerminalDebugMap,
) -> Result<Vec<u8>, DebugMapError> {
    validate_debug_map(module, debug_map)?;
    encode_raw(debug_map)
}

pub fn decode_debug_map(
    module: &TerminalModule,
    bytes: &[u8],
) -> Result<TerminalDebugMap, DebugMapError> {
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(DebugMapError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != FORMAT_MARKER {
        return Err(DebugMapError::UnsupportedFormatMarker(format_marker));
    }
    let vocabulary_marker_raw = reader.u16()?;
    let vocabulary_marker = VocabularyMarker::new(vocabulary_marker_raw).ok_or(
        DebugMapError::UnsupportedVocabularyMarker(vocabulary_marker_raw),
    )?;
    let program_fingerprint = SemanticFingerprint::from_bytes(reader.array()?);
    let file_count = reader.count()?;
    let mut files = Vec::with_capacity(file_count as usize);
    for _ in 0..file_count {
        let raw_file = reader.u32()?;
        let id = DebugFileId::new(raw_file).ok_or(DebugMapError::ZeroFileIdentity)?;
        let origin = match reader.u8()? {
            1 => DebugSourceOrigin::User,
            2 => DebugSourceOrigin::Toolchain,
            tag => return Err(DebugMapError::InvalidTag("DebugSourceOrigin", tag)),
        };
        let byte_len = reader.u64()?;
        let digest = DebugSourceDigest::from_bytes(reader.array()?);
        let path = reader.string("debug source path")?;
        files.push(DebugSourceFile {
            id,
            origin,
            byte_len,
            digest,
            path,
        });
    }
    let site_count = reader.count()?;
    let mut sites = Vec::with_capacity(site_count as usize);
    for _ in 0..site_count {
        let subject = decode_subject(&mut reader)?;
        let raw_file = reader.u32()?;
        let file = DebugFileId::new(raw_file).ok_or(DebugMapError::ZeroFileIdentity)?;
        sites.push(DebugSite {
            subject,
            span: DebugSourceSpan {
                file,
                start: reader.u64()?,
                end: reader.u64()?,
            },
        });
    }
    if reader.remaining() != 0 {
        return Err(DebugMapError::TrailingBytes(reader.remaining()));
    }
    let debug_map = TerminalDebugMap {
        semantic: TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint,
        },
        files,
        sites,
    };
    validate_debug_map(module, &debug_map)?;
    if encode_raw(&debug_map)? != bytes {
        return Err(DebugMapError::NonCanonicalEncoding);
    }
    Ok(debug_map)
}

pub fn validate_debug_map(
    module: &TerminalModule,
    debug_map: &TerminalDebugMap,
) -> Result<(), DebugMapError> {
    let expected = terminal_psi_identity(module)?;
    if debug_map.semantic != expected {
        return Err(DebugMapError::SemanticIdentityMismatch {
            expected,
            actual: debug_map.semantic,
        });
    }
    if !strictly_increasing(debug_map.files.iter().map(|file| file.id)) {
        return Err(DebugMapError::NonCanonicalOrder(
            "debug files by DebugFileId",
        ));
    }
    if !strictly_increasing(debug_map.sites.iter().map(|site| site.subject)) {
        return Err(DebugMapError::NonCanonicalOrder("debug sites by subject"));
    }
    for site in &debug_map.sites {
        let Some(file) = debug_map
            .files
            .iter()
            .find(|file| file.id == site.span.file)
        else {
            return Err(DebugMapError::UnknownFile(site.span.file));
        };
        if site.span.start > site.span.end || site.span.end > file.byte_len {
            return Err(DebugMapError::InvalidSpan(site.span));
        }
        if !subject_exists(module, site.subject) {
            return Err(DebugMapError::UnknownSubject(site.subject));
        }
    }
    Ok(())
}

fn encode_raw(debug_map: &TerminalDebugMap) -> Result<Vec<u8>, DebugMapError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(FORMAT_MARKER);
    writer.u16(debug_map.semantic.vocabulary_marker.get());
    writer.bytes(debug_map.semantic.program_fingerprint.as_bytes());
    writer.len("debug source files", debug_map.files.len())?;
    for file in &debug_map.files {
        writer.u32(file.id.get());
        writer.u8(match file.origin {
            DebugSourceOrigin::User => 1,
            DebugSourceOrigin::Toolchain => 2,
        });
        writer.u64(file.byte_len);
        writer.bytes(file.digest.as_bytes());
        writer.string("debug source path", &file.path)?;
    }
    writer.len("debug sites", debug_map.sites.len())?;
    for site in &debug_map.sites {
        encode_subject(&mut writer, site.subject);
        writer.u32(site.span.file.get());
        writer.u64(site.span.start);
        writer.u64(site.span.end);
    }
    Ok(writer.finish())
}

fn encode_subject(writer: &mut Writer, subject: DebugSubject) {
    match subject {
        DebugSubject::Machine(id) => {
            writer.u8(1);
            writer.id(id);
        }
        DebugSubject::Block(id) => {
            writer.u8(2);
            writer.id(id);
        }
        DebugSubject::Operation(id) => {
            writer.u8(3);
            writer.id(id);
        }
        DebugSubject::Edge(id) => {
            writer.u8(4);
            writer.id(id);
        }
        DebugSubject::Value(id) => {
            writer.u8(5);
            writer.id(id);
        }
        DebugSubject::Contract(id) => {
            writer.u8(6);
            writer.id(id);
        }
        DebugSubject::Obligation(id) => {
            writer.u8(7);
            writer.id(id);
        }
        DebugSubject::Place(id) => {
            writer.u8(8);
            writer.id(id);
        }
        DebugSubject::Claim { machine, claim } => {
            writer.u8(9);
            writer.id(machine);
            writer.id(claim);
        }
    }
}

fn decode_subject(reader: &mut Reader<'_>) -> Result<DebugSubject, DebugMapError> {
    Ok(match reader.u8()? {
        1 => DebugSubject::Machine(reader.id("debug MachineId")?),
        2 => DebugSubject::Block(reader.id("debug BlockId")?),
        3 => DebugSubject::Operation(reader.id("debug OperationId")?),
        4 => DebugSubject::Edge(reader.id("debug EdgeId")?),
        5 => DebugSubject::Value(reader.id("debug ValueId")?),
        6 => DebugSubject::Contract(reader.id("debug ContractId")?),
        7 => DebugSubject::Obligation(reader.id("debug ObligationId")?),
        8 => DebugSubject::Place(reader.id("debug PlaceId")?),
        9 => DebugSubject::Claim {
            machine: reader.id("debug claim MachineId")?,
            claim: reader.id("debug ClaimId")?,
        },
        tag => return Err(DebugMapError::InvalidTag("DebugSubject", tag)),
    })
}

fn subject_exists(module: &TerminalModule, subject: DebugSubject) -> bool {
    match subject {
        DebugSubject::Machine(id) => module.machines.iter().any(|machine| machine.id == id),
        DebugSubject::Block(id) => module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .any(|block| block.id == id),
        DebugSubject::Operation(id) => module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| operation.id == id),
        DebugSubject::Edge(id) => module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .any(|block| block.terminator.edges().any(|edge| edge == id)),
        DebugSubject::Value(id) => module.machines.iter().any(|machine| {
            machine
                .result
                .scalar()
                .is_some_and(|result| result.id == id)
                || machine.parameters.iter().any(|value| value.id == id)
                || machine.blocks.iter().any(|block| {
                    block.parameters.iter().any(|value| value.id == id)
                        || block
                            .operations
                            .iter()
                            .any(|operation| operation.result.expect_scalar().id == id)
                })
        }),
        DebugSubject::Contract(id) => module
            .machines
            .iter()
            .any(|machine| machine.contract.id == id),
        DebugSubject::Obligation(id) => module
            .machines
            .iter()
            .flat_map(|machine| &machine.contract.ensures)
            .any(|clause| clause.obligation == id),
        DebugSubject::Place(id) => module
            .machines
            .iter()
            .flat_map(|machine| &machine.structural_places)
            .any(|place| place.id == id),
        DebugSubject::Claim { machine, claim } => module
            .machines
            .iter()
            .find(|candidate| candidate.id == machine)
            .is_some_and(|machine| {
                machine
                    .content_entry_claims
                    .iter()
                    .any(|binding| binding.claim == claim)
                    || machine
                        .content_identity_reshuffles
                        .iter()
                        .any(|reshuffle| reshuffle.claim == claim)
                    || machine
                        .content_partition_compositions
                        .iter()
                        .any(|composition| composition.input_claims.contains(&claim))
            }),
    }
}

fn strictly_increasing<T: Ord>(values: impl IntoIterator<Item = T>) -> bool {
    let mut values = values.into_iter();
    let Some(mut prior) = values.next() else {
        return true;
    };
    for value in values {
        if value <= prior {
            return false;
        }
        prior = value;
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugMapError {
    Codec(CodecError),
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    UnsupportedVocabularyMarker(u16),
    ZeroFileIdentity,
    InvalidTag(&'static str, u8),
    TrailingBytes(usize),
    NonCanonicalOrder(&'static str),
    NonCanonicalEncoding,
    SemanticIdentityMismatch {
        expected: TerminalPsiIdentity,
        actual: TerminalPsiIdentity,
    },
    UnknownFile(DebugFileId),
    InvalidSpan(DebugSourceSpan),
    UnknownSubject(DebugSubject),
}

impl From<CodecError> for DebugMapError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl std::fmt::Display for DebugMapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DebugMapError {}
