use layout_plans::RelocationTarget;

/// Closed semantic relocation vocabulary accepted from the checked artifact
/// schema. These are normalized relocation meanings, not object-format
/// record numbers; the target writer translates them to its native format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactRelocationKind {
    Absolute64,
    X86Relative32,
    Aarch64Page21,
    Aarch64PageOffset12,
    Aarch64Branch26,
}

/// One checked-schema relocation record before semantic validation. The
/// target remains symbolic: decoding an artifact never exposes a numeric code
/// or data address to source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedArtifactRelocation {
    pub kind: ArtifactRelocationKind,
    pub destination_offset: u64,
    pub target: RelocationTarget,
    pub addend: i64,
}
