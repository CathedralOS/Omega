use crate::{
    CodecError,
    wire::{Reader, Writer, decode_counted},
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IeeeFloatComparisonKind, IeeeFloatFormat,
    IeeeFloatStructuralField, PlaceId,
};
pub fn encode_ieee_float_format(writer: &mut Writer, format: IeeeFloatFormat) {
    writer.u8(match format {
        IeeeFloatFormat::Binary32 => 1,
        IeeeFloatFormat::Binary64 => 2,
    });
}

pub fn encode_ieee_float_comparison_kind(writer: &mut Writer, kind: IeeeFloatComparisonKind) {
    writer.u8(match kind {
        IeeeFloatComparisonKind::Equal => 1,
        IeeeFloatComparisonKind::NotEqual => 2,
        IeeeFloatComparisonKind::LessThan => 3,
        IeeeFloatComparisonKind::LessOrEqual => 4,
        IeeeFloatComparisonKind::NotLessThan => 5,
        IeeeFloatComparisonKind::NotLessOrEqual => 6,
    });
}

pub fn decode_ieee_float_comparison_kind(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatComparisonKind, CodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatComparisonKind::Equal),
        2 => Ok(IeeeFloatComparisonKind::NotEqual),
        3 => Ok(IeeeFloatComparisonKind::LessThan),
        4 => Ok(IeeeFloatComparisonKind::LessOrEqual),
        5 => Ok(IeeeFloatComparisonKind::NotLessThan),
        6 => Ok(IeeeFloatComparisonKind::NotLessOrEqual),
        tag => Err(CodecError::InvalidTag("IeeeFloatComparisonKind", tag)),
    }
}

pub fn decode_ieee_float_format(reader: &mut Reader<'_>) -> Result<IeeeFloatFormat, CodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatFormat::Binary32),
        2 => Ok(IeeeFloatFormat::Binary64),
        tag => Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
    }
}

pub fn encode_ieee_float_field(
    writer: &mut Writer,
    field: &IeeeFloatStructuralField,
) -> Result<(), CodecError> {
    encode_canonical_structural_field(writer, field.root(), field.path(), "IEEE float field path")
}

pub fn decode_ieee_float_field(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatStructuralField, CodecError> {
    let (root, path) = decode_canonical_structural_field(reader)?;
    IeeeFloatStructuralField::new(root, path).map_err(CodecError::MalformedProposition)
}

pub fn encode_canonical_structural_field(
    writer: &mut Writer,
    root: PlaceId,
    path: &[CanonicalStructuralPathSegment],
    length_label: &'static str,
) -> Result<(), CodecError> {
    writer.id(root);
    encode_canonical_structural_path(writer, path, length_label)
}

pub fn encode_canonical_structural_path(
    writer: &mut Writer,
    path: &[CanonicalStructuralPathSegment],
    length_label: &'static str,
) -> Result<(), CodecError> {
    writer.len(length_label, path.len())?;
    for segment in path {
        match segment {
            CanonicalStructuralPathSegment::Referent => writer.u8(4),
            CanonicalStructuralPathSegment::Field(field) => {
                writer.u8(1);
                writer.id(*field);
            }
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                writer.u8(2);
                writer.u64(*index);
            }
            CanonicalStructuralPathSegment::Case(case) => {
                writer.u8(3);
                writer.id(*case);
            }
        }
    }
    Ok(())
}

pub fn decode_canonical_structural_field(
    reader: &mut Reader<'_>,
) -> Result<(PlaceId, Vec<CanonicalStructuralPathSegment>), CodecError> {
    let root = reader.id("PlaceId")?;
    let path = decode_canonical_structural_path(reader, "CanonicalStructuralPathSegment")?;
    Ok((root, path))
}

fn decode_canonical_structural_path(
    reader: &mut Reader<'_>,
    tag_name: &'static str,
) -> Result<Vec<CanonicalStructuralPathSegment>, CodecError> {
    decode_counted(reader, |reader| {
        match reader.u8()? {
        1 => Ok(CanonicalStructuralPathSegment::Field(
            reader.id("StructuralFieldId")?,
        )),
        2 => Ok(CanonicalStructuralPathSegment::FixedIndex(reader.u64()?)),
        3 => Ok(CanonicalStructuralPathSegment::Case(
            reader.id("StructuralCaseId")?,
        )),
        4 => Ok(CanonicalStructuralPathSegment::Referent),
        tag => Err(CodecError::InvalidTag(tag_name, tag)),
    }
    })
}

pub struct FieldPath;
impl crate::fields::Field<Vec<CanonicalStructuralPathSegment>> for FieldPath {
    fn write(
        writer: &mut Writer,
        path: &Vec<CanonicalStructuralPathSegment>,
    ) -> Result<(), CodecError> {
        encode_canonical_structural_path(writer, path, "scalar field carrier path")
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<CanonicalStructuralPathSegment>, CodecError> {
        decode_canonical_structural_path(reader, "scalar field carrier path")
    }
}
