//! Gives retained storage paths one byte vocabulary across Psi and native stages.
//!
//! Enclosing formats own list framing, scalar-type encoding and which projections
//! their context admits. They share segment tags and fixed-width payloads here;
//! identity encoders must not invent a second numbering for the same path.

use crate::StructuralPathSegment;
use semantic_vocabulary::ScalarType;

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

/// Writes one segment without allocating or adding a path-length prefix.
pub fn encode_structural_path_segment<Output>(
    output: &mut Output,
    segment: &StructuralPathSegment,
    write_bytes: impl FnMut(&mut Output, &[u8]),
    mut write_scalar: impl FnMut(&mut Output, ScalarType),
) {
    let result =
        try_encode_structural_path_segment(output, segment, write_bytes, |output, scalar| {
            write_scalar(output, scalar);
            Ok::<(), core::convert::Infallible>(())
        });
    match result {
        Ok(()) => {}
        Err(never) => match never {},
    }
}

/// Writes one segment, propagating scalar encoding failures without changing its bytes.
pub fn try_encode_structural_path_segment<Output, Error>(
    output: &mut Output,
    segment: &StructuralPathSegment,
    mut write_bytes: impl FnMut(&mut Output, &[u8]),
    mut write_scalar: impl FnMut(&mut Output, ScalarType) -> Result<(), Error>,
) -> Result<(), Error> {
    use SegmentTag as Tag;
    match segment {
        StructuralPathSegment::Field(field) => {
            write_bytes(output, &[Tag::Field as u8]);
            write_bytes(output, &field.0.to_le_bytes());
        }
        StructuralPathSegment::FixedIndex(index) => {
            write_bytes(output, &[Tag::FixedIndex as u8]);
            write_bytes(output, &index.to_le_bytes());
        }
        StructuralPathSegment::Referent => write_bytes(output, &[Tag::Referent as u8]),
        StructuralPathSegment::FixedRange { start, end } => {
            write_bytes(output, &[Tag::FixedRange as u8]);
            write_bytes(output, &start.to_le_bytes());
            write_bytes(output, &end.to_le_bytes());
        }
        StructuralPathSegment::RuntimeIndex { index, obligation } => {
            write_bytes(output, &[Tag::RuntimeIndex as u8]);
            write_bytes(output, &index.get().to_le_bytes());
            write_bytes(output, &obligation.get().to_le_bytes());
        }
        StructuralPathSegment::Case(case) => {
            write_bytes(output, &[Tag::Case as u8]);
            write_bytes(output, &case.get().to_le_bytes());
        }
        StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } => {
            write_bytes(output, &[Tag::RepresentationView as u8]);
            write_bytes(output, &structural_type.get().to_le_bytes());
            write_bytes(output, &[u8::from(stored_scalar.is_some())]);
            if let Some(scalar) = stored_scalar {
                write_scalar(output, *scalar)?;
            }
        }
        StructuralPathSegment::AllElements => write_bytes(output, &[Tag::AllElements as u8]),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TerminalIdentityIndex;
    use semantic_vocabulary::{ObligationId, StructuralCaseId, StructuralTypeId, ValueId};

    #[test]
    fn segment_bytes_match_the_published_tags_and_payloads() {
        let cases = [
            (
                StructuralPathSegment::Field(TerminalIdentityIndex(0x1234)),
                vec![1, 0x34, 0x12, 0, 0],
            ),
            (
                StructuralPathSegment::FixedIndex(2),
                vec![2, 2, 0, 0, 0, 0, 0, 0, 0],
            ),
            (StructuralPathSegment::Referent, vec![3]),
            (
                StructuralPathSegment::FixedRange { start: 2, end: 7 },
                vec![4, 2, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                StructuralPathSegment::RuntimeIndex {
                    index: ValueId::new(2).unwrap(),
                    obligation: ObligationId::new(7).unwrap(),
                },
                vec![6, 2, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                StructuralPathSegment::Case(StructuralCaseId::new(7).unwrap()),
                vec![7, 7, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                StructuralPathSegment::RepresentationView {
                    structural_type: StructuralTypeId::new(7).unwrap(),
                    stored_scalar: None,
                },
                vec![8, 7, 0, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                StructuralPathSegment::RepresentationView {
                    structural_type: StructuralTypeId::new(7).unwrap(),
                    stored_scalar: Some(ScalarType::Boolean),
                },
                vec![8, 7, 0, 0, 0, 0, 0, 0, 0, 1, 0xa5],
            ),
            (StructuralPathSegment::AllElements, vec![10]),
        ];
        for (segment, expected) in cases {
            let mut encoded = Vec::new();
            encode_structural_path_segment(
                &mut encoded,
                &segment,
                Vec::extend_from_slice,
                |output, scalar| {
                    assert_eq!(scalar, ScalarType::Boolean);
                    output.push(0xa5);
                },
            );
            assert_eq!(encoded, expected, "{segment:?}");
            assert_eq!(SegmentTag::from_byte(encoded[0]).unwrap() as u8, encoded[0]);
        }
    }

    #[test]
    fn scalar_failure_propagates_from_a_representation_view() {
        let segment = StructuralPathSegment::RepresentationView {
            structural_type: StructuralTypeId::new(7).unwrap(),
            stored_scalar: Some(ScalarType::Boolean),
        };
        let mut bytes = Vec::new();
        let result = try_encode_structural_path_segment(
            &mut bytes,
            &segment,
            Vec::extend_from_slice,
            |_, _| Err("unclosed scalar"),
        );
        assert_eq!(result, Err("unclosed scalar"));
        assert_eq!(bytes, [8, 7, 0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn retired_and_enclosing_tags_are_not_segment_tags() {
        for tag in [0, 5, 9, 11, u8::MAX] {
            assert_eq!(SegmentTag::from_byte(tag), None);
        }
    }
}
