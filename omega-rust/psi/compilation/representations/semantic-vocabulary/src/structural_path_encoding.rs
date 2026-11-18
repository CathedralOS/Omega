//! Shared segment tags; enclosing formats choose IDs and list framing.

use crate::CanonicalStructuralPathSegment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SegmentTag {
    Field = 1,
    FixedIndex = 2,
    Referent = 3,
    FixedRange = 4,
    RuntimeIndex = 6,
    Case = 7,
    RepresentationView = 8,
    AllElements = 10,
}

impl SegmentTag {
    /// Rejects retired tag 5 and tags belonging to enclosing path vocabularies.
    pub const fn from_byte(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Field),
            2 => Some(Self::FixedIndex),
            3 => Some(Self::Referent),
            4 => Some(Self::FixedRange),
            6 => Some(Self::RuntimeIndex),
            7 => Some(Self::Case),
            8 => Some(Self::RepresentationView),
            10 => Some(Self::AllElements),
            _ => None,
        }
    }
}

/// Writes a semantic path segment with its full-width semantic identity.
pub fn encode_canonical_structural_path_segment(
    output: &mut Vec<u8>,
    segment: CanonicalStructuralPathSegment,
) {
    match segment {
        CanonicalStructuralPathSegment::Field(field) => {
            output.push(SegmentTag::Field as u8);
            output.extend_from_slice(&field.get().to_le_bytes());
        }
        CanonicalStructuralPathSegment::FixedIndex(index) => {
            output.push(SegmentTag::FixedIndex as u8);
            output.extend_from_slice(&index.to_le_bytes());
        }
        CanonicalStructuralPathSegment::Case(case) => {
            output.push(SegmentTag::Case as u8);
            output.extend_from_slice(&case.get().to_le_bytes());
        }
        CanonicalStructuralPathSegment::Referent => output.push(SegmentTag::Referent as u8),
    }
}
