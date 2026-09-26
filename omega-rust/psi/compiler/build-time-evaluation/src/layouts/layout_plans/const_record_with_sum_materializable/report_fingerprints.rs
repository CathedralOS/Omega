//! Occurrence hashes and report-only materialization fingerprints.

use crate::layouts::layout_plans::BuildTimeValue;
use crate::layouts::layout_plans::const_materializable::{
    hash_byte, hash_bytes, hash_text, hash_u64, hash_value,
};
use crate::layouts::layout_plans::const_record_with_sum_materializable::{
    ValidatedConstRecordSumArrayElementMaterialization,
    ValidatedConstRecordSumArrayElementSelection, ValidatedConstRecordSumArrayFieldMaterialization,
    ValidatedConstRecordSumFieldMaterialization,
};
use layout_plans::{
    ByteOrder, ConventionalSumArrayFieldLayoutReport,
    normalized_conventional_sum_layout_report_fingerprint,
};

fn hash_sum_array_occurrence(
    hash: &mut u64,
    array_layout: &ConventionalSumArrayFieldLayoutReport,
    elements: &[ValidatedConstRecordSumArrayElementMaterialization],
) {
    match array_layout.member_identity {
        Some(identity) => {
            hash_byte(hash, 1);
            hash_u64(hash, identity);
        }
        None => {
            hash_byte(hash, 0);
            hash_text(hash, &array_layout.field);
        }
    }
    hash_u64(hash, array_layout.element_count);
    hash_u64(hash, array_layout.element_stride);
    hash_u64(
        hash,
        normalized_conventional_sum_layout_report_fingerprint(&array_layout.element_layout),
    );
    hash_u64(hash, elements.len() as u64);
    for element in elements {
        hash_u64(hash, element.literal_index);
        hash_u64(
            hash,
            element
                .nested_sum
                .non_authoritative_layout_report_fingerprint(),
        );
        match element.nested_sum.selected_case_identity() {
            Some(identity) => {
                hash_byte(hash, 1);
                hash_u64(hash, identity);
            }
            None => hash_byte(hash, 0),
        }
        hash_u64(hash, u64::from(element.nested_sum.selected_case_ordinal()));
    }
}

pub(crate) fn hash_compact_sum_array_occurrence(
    hash: &mut u64,
    array_layout: &ConventionalSumArrayFieldLayoutReport,
    elements: &[ValidatedConstRecordSumArrayElementSelection],
) {
    match array_layout.member_identity {
        Some(identity) => {
            hash_byte(hash, 1);
            hash_u64(hash, identity);
        }
        None => {
            hash_byte(hash, 0);
            hash_text(hash, &array_layout.field);
        }
    }
    hash_u64(hash, array_layout.element_count);
    hash_u64(hash, array_layout.element_stride);
    hash_u64(
        hash,
        normalized_conventional_sum_layout_report_fingerprint(&array_layout.element_layout),
    );
    hash_u64(hash, elements.len() as u64);
    for element in elements {
        hash_u64(hash, element.literal_index);
        hash_u64(hash, element.non_authoritative_schema_report_fingerprint);
        hash_value(hash, &element.value);
        hash_u64(hash, element.non_authoritative_layout_report_fingerprint);
        match element.selected_case_identity {
            Some(identity) => {
                hash_byte(hash, 1);
                hash_u64(hash, identity);
            }
            None => hash_byte(hash, 0),
        }
        hash_u64(hash, u64::from(element.selected_case_ordinal));
        hash_u64(hash, element.bytes.len() as u64);
        hash_bytes(hash, &element.bytes);
        hash_u64(
            hash,
            element.non_authoritative_materialization_report_fingerprint,
        );
    }
}

pub(crate) fn non_authoritative_record_with_sum_array_materialization_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    layout_report_fingerprint: u64,
    array_layout: &ConventionalSumArrayFieldLayoutReport,
    elements: &[ValidatedConstRecordSumArrayElementMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-record-with-sum-array.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, layout_report_fingerprint);
    hash_sum_array_occurrence(&mut hash, array_layout, elements);
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

pub(crate) fn non_authoritative_record_with_sum_arrays_materialization_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    layout_report_fingerprint: u64,
    array_layouts: &[ConventionalSumArrayFieldLayoutReport],
    arrays: &[ValidatedConstRecordSumArrayFieldMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-record-with-sum-arrays.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, layout_report_fingerprint);
    hash_u64(&mut hash, array_layouts.len() as u64);
    for (layout, array) in array_layouts.iter().zip(arrays) {
        hash_compact_sum_array_occurrence(&mut hash, layout, &array.elements);
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

pub(crate) fn non_authoritative_record_with_sum_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    layout_report_fingerprint: u64,
    nested_sums: &[ValidatedConstRecordSumFieldMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.const-materializable-record-with-sum.v2");
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, layout_report_fingerprint);
    hash_u64(&mut hash, nested_sums.len() as u64);
    for nested in nested_sums {
        match nested.field_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &nested.field);
            }
        }
        hash_u64(
            &mut hash,
            nested
                .nested_sum
                .non_authoritative_layout_report_fingerprint(),
        );
        match nested.nested_sum.selected_case_identity() {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => hash_byte(&mut hash, 0),
        }
        hash_u64(
            &mut hash,
            u64::from(nested.nested_sum.selected_case_ordinal()),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}
