use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
    PackageReviewRuntimeIndex, PackageReviewStructuralParameterField,
    PackageReviewStructuralPredicatePathSegment,
};

mod boolean;
mod scalar;
pub(crate) use boolean::encode_boolean_expression;
pub(crate) use scalar::encode_scalar_expression;

mod vocabulary;

use vocabulary::{arithmetic_domain_name, primitive_type_name};
pub(crate) use vocabulary::{encode_primitive_type, integer_binary_row, integer_comparison_row};

pub(crate) fn encode_structural_field(
    encoder: &mut Encoder,
    field: &PackageReviewStructuralParameterField,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("parameter_position", |encoder| {
        encoder.u32(field.parameter_position);
        Ok(())
    })?;
    encoder.field("path", |encoder| {
        encode_structural_path(encoder, &field.path)
    })
}

pub(crate) fn encode_structural_path(
    encoder: &mut Encoder,
    path: &[PackageReviewStructuralPredicatePathSegment],
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(path, |encoder, segment| {
        match segment {
            PackageReviewStructuralPredicatePathSegment::Field(field) => {
                encoder.tag("field", 0);
                encoder.field("field", |encoder| encoder.string(field))?;
            }
            PackageReviewStructuralPredicatePathSegment::Case(case) => {
                encoder.tag("case", 1);
                encoder.field("case", |encoder| encoder.string(case))?;
            }
            PackageReviewStructuralPredicatePathSegment::FixedIndex(element_index) => {
                encoder.tag("fixed_index", 2);
                encoder.field("index", |encoder| {
                    encoder.u64(*element_index);
                    Ok(())
                })?;
            }
            PackageReviewStructuralPredicatePathSegment::RuntimeIndex(index) => {
                encoder.tag("runtime_index", 3);
                encoder.field("index", |encoder| encode_runtime_index(encoder, index))?;
            }
        }
        Ok(())
    })
}

fn encode_runtime_index(
    encoder: &mut Encoder,
    index: &PackageReviewRuntimeIndex,
) -> Result<(), PackageReviewEncodingError> {
    match index {
        PackageReviewRuntimeIndex::Parameter { position } => {
            encoder.tag("parameter", 0);
            encoder.u32(*position);
        }
        PackageReviewRuntimeIndex::AssignmentIndex { depth } => {
            encoder.tag("assignment_index", 1);
            encoder.u32(*depth);
        }
        PackageReviewRuntimeIndex::StructuralField(field) => {
            encoder.tag("structural_field", 2);
            encoder.u32(field.parameter_position);
            encode_structural_path(encoder, &field.path)?;
        }
        PackageReviewRuntimeIndex::Local { position } => {
            encoder.tag("local", 3);
            encoder.u32(*position);
        }
    }
    Ok(())
}
