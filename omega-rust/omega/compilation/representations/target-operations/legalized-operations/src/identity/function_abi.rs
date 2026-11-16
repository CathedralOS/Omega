//! Canonical bytes for the per-function target ABI rows and the borrowed
//! storage layouts the prepared plan carries.

use super::calling::{encode_call_plan, encode_placement};
use super::structural_types::encode_target_structural_parameter;
use super::{encode_len, encode_option, encode_scalar_type};
use crate::LegalizedFunctionAbi;
use runtime_abi::{NativeBorrowedStorageLayout, NativeStorageRepetition};
use semantic_vocabulary::CanonicalStructuralPathSegment;
use target_vocabulary::ScalarAbiValue;

pub(super) fn encode_function_abi(bytes: &mut Vec<u8>, abi: &LegalizedFunctionAbi) {
    crate::codec_bytes::push_encoded(bytes, abi.machine.get().to_le_bytes());
    encode_call_plan(bytes, &abi.call_plan);
    encode_scalar_values(bytes, &abi.scalar_parameters);
    encode_len(bytes, abi.structural_parameters.len());
    for parameter in &abi.structural_parameters {
        encode_target_structural_parameter(bytes, parameter);
    }
    encode_option(bytes, abi.scalar.as_ref(), |bytes, scalar| {
        encode_call_plan(bytes, &scalar.call_plan);
        encode_scalar_values(bytes, &scalar.parameters);
        encode_option(bytes, scalar.result.as_ref(), encode_scalar_value);
    });
    encode_option(
        bytes,
        abi.mixed_structural_scalar.as_ref(),
        |bytes, mixed| {
            encode_call_plan(bytes, &mixed.call_plan);
            encode_scalar_values(bytes, &mixed.scalar_parameters);
            encode_len(bytes, mixed.structural_parameters.len());
            for parameter in &mixed.structural_parameters {
                encode_target_structural_parameter(bytes, parameter);
            }
            encode_scalar_value(bytes, &mixed.result);
        },
    );
}

fn encode_scalar_values(bytes: &mut Vec<u8>, values: &[ScalarAbiValue]) {
    encode_len(bytes, values.len());
    for value in values {
        encode_scalar_value(bytes, value);
    }
}

fn encode_scalar_value(bytes: &mut Vec<u8>, value: &ScalarAbiValue) {
    crate::codec_bytes::push_encoded(bytes, value.value.get().to_le_bytes());
    encode_scalar_type(bytes, value.scalar_type);
    encode_placement(bytes, &value.placement);
}

/// A layout is named by what its accessors report, never by how its arenas
/// happen to be filled, so two layouts that describe the same storage encode
/// to the same bytes.
pub(super) fn encode_borrowed_storage_layout(
    bytes: &mut Vec<u8>,
    layout: &NativeBorrowedStorageLayout,
) {
    crate::codec_bytes::push_encoded(bytes, layout.structural_type.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, layout.byte_size.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, layout.alignment.to_le_bytes());
    let fields = layout.fields();
    encode_len(bytes, fields.len());
    for field in fields {
        encode_path(bytes, layout.field_path(field));
        encode_repetitions(bytes, layout.field_repetitions(field));
        crate::codec_bytes::push_encoded(bytes, field.byte_offset.to_le_bytes());
        encode_scalar_type(bytes, field.logical_scalar);
        encode_scalar_type(bytes, field.stored_scalar);
    }
    let discriminators = layout.discriminators().collect::<Vec<_>>();
    encode_len(bytes, discriminators.len());
    for discriminator in discriminators {
        encode_path(bytes, layout.discriminator_path(discriminator));
        encode_repetitions(bytes, layout.discriminator_repetitions(discriminator));
        crate::codec_bytes::push_encoded(bytes, discriminator.structural_type.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, discriminator.byte_offset.to_le_bytes());
        let cases = layout
            .discriminator_cases(discriminator)
            .collect::<Vec<_>>();
        encode_len(bytes, cases.len());
        for case in cases {
            crate::codec_bytes::push_encoded(bytes, case.case.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, case.native_tag.to_le_bytes());
        }
    }
    let regions = layout.opaque_regions();
    encode_len(bytes, regions.len());
    for region in regions {
        encode_path(bytes, layout.opaque_region_path(region));
        encode_repetitions(bytes, layout.opaque_region_repetitions(region));
        crate::codec_bytes::push_encoded(bytes, region.byte_offset.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, region.byte_size.to_le_bytes());
    }
}

fn encode_path(
    bytes: &mut Vec<u8>,
    path: impl ExactSizeIterator<Item = CanonicalStructuralPathSegment>,
) {
    encode_len(bytes, path.len());
    for segment in path {
        let (tag, identity) = match segment {
            CanonicalStructuralPathSegment::Field(field) => (1u8, field.get()),
            CanonicalStructuralPathSegment::FixedIndex(index) => (2, index),
            CanonicalStructuralPathSegment::Case(case) => (3, case.get()),
            CanonicalStructuralPathSegment::Referent => (4, 0),
        };
        bytes.push(tag);
        crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
    }
}

fn encode_repetitions(bytes: &mut Vec<u8>, repetitions: &[NativeStorageRepetition]) {
    encode_len(bytes, repetitions.len());
    for repetition in repetitions {
        encode_len(bytes, repetition.path_position);
        crate::codec_bytes::push_encoded(bytes, repetition.length.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, repetition.byte_stride.to_le_bytes());
    }
}
