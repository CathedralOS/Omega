//! Fixed record layouts and schemas of the container sections.

use crate::artifacts::container_bytes::{
    ENTRY_RECORD_BYTES, OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES,
    OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES, PLACEMENT_RECORD_BYTES,
    RELOCATION_RECORD_BYTES,
};
use layout_plans::{
    LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport, ScalarFieldSchema,
};

fn scalar_layout(size: u64, fields: &[(&str, u64, u16)]) -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: fields
            .iter()
            .map(|(field, offset, _)| LayoutFieldEntryReport {
                field: (*field).into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: *offset },
            })
            .collect(),
        offsets: Some(fields.iter().map(|(_, offset, _)| *offset).collect()),
        size: Some(size),
        align: 1,
    }
}

fn scalar_schema(fields: &[(&str, u64, u16)]) -> Vec<ScalarFieldSchema> {
    fields
        .iter()
        .map(|(field, _, width)| {
            ScalarFieldSchema::new(*field, *width).expect("static scalar schema is valid")
        })
        .collect()
}

const HEADER_FIELDS: &[(&str, u64, u16)] = &[
    ("magic", 0, 64),
    ("format_marker", 8, 16),
    ("header_bytes", 10, 16),
    ("architecture", 12, 8),
    ("reserved0", 13, 8),
    ("section_count", 14, 16),
    ("directory_offset", 16, 64),
    ("total_length", 24, 64),
    ("artifact", 32, 64),
    ("content", 40, 64),
    ("reserved1", 48, 64),
    ("reserved2", 56, 64),
];

pub(crate) fn header_layout() -> LayoutPlanReport {
    scalar_layout(OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES, HEADER_FIELDS)
}

pub(crate) fn header_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(HEADER_FIELDS)
}

const SECTION_FIELDS: &[(&str, u64, u16)] = &[
    ("kind", 0, 16),
    ("flags", 2, 16),
    ("reserved", 4, 32),
    ("identity", 8, 64),
    ("offset", 16, 64),
    ("length", 24, 64),
];

pub(crate) fn section_layout() -> LayoutPlanReport {
    scalar_layout(
        OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES,
        SECTION_FIELDS,
    )
}

pub(crate) fn section_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(SECTION_FIELDS)
}

const IDENTITY_FIELDS: &[(&str, u64, u16)] = &[("identity", 0, 64)];

pub(crate) fn identity_layout() -> LayoutPlanReport {
    scalar_layout(8, IDENTITY_FIELDS)
}

pub(crate) fn identity_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(IDENTITY_FIELDS)
}

const PLACEMENT_FIELDS: &[(&str, u64, u16)] = &[
    ("plan", 0, 64),
    ("range_present", 8, 8),
    ("phase", 9, 8),
    ("regime_present", 10, 8),
    ("scope_present", 11, 8),
    ("reserved0", 12, 32),
    ("range_start", 16, 64),
    ("range_end", 24, 64),
    ("alignment", 32, 64),
    ("regime", 40, 64),
    ("scope", 48, 64),
    ("reserved1", 56, 64),
];

pub(crate) fn placement_layout() -> LayoutPlanReport {
    scalar_layout(PLACEMENT_RECORD_BYTES, PLACEMENT_FIELDS)
}

pub(crate) fn placement_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(PLACEMENT_FIELDS)
}

const ENTRY_FIELDS: &[(&str, u64, u16)] = &[("identity", 0, 64), ("offset", 8, 64)];

pub(crate) fn entry_layout() -> LayoutPlanReport {
    scalar_layout(ENTRY_RECORD_BYTES, ENTRY_FIELDS)
}

pub(crate) fn entry_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(ENTRY_FIELDS)
}

const RELOCATION_FIELDS: &[(&str, u64, u16)] = &[
    ("kind", 0, 16),
    ("target_kind", 2, 16),
    ("reserved", 4, 32),
    ("destination", 8, 64),
    ("target", 16, 64),
    ("addend", 24, 64),
];

pub(crate) fn relocation_layout() -> LayoutPlanReport {
    scalar_layout(RELOCATION_RECORD_BYTES, RELOCATION_FIELDS)
}

pub(crate) fn relocation_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(RELOCATION_FIELDS)
}
