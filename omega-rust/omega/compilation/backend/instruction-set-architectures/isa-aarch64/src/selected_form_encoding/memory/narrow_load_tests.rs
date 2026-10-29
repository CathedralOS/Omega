//! Exact narrow pointer reads reject opcode, operand and displacement substitutions.

use super::{encode_aarch64_selected_memory_form, validate_aarch64_selected_memory_form};
use crate::selected_form_encoding::ValidatedAarch64SelectedFormEncoding;
use isa_conformance::narrow_load::{self, Exact, Footprint, Roster};

/// Report one validated narrow load as the shared conformance footprint.
fn observed(encoded: &ValidatedAarch64SelectedFormEncoding) -> Footprint {
    Footprint {
        bytes: encoded.bytes().to_vec(),
        register_reads: encoded.footprint().register_reads.clone(),
        register_writes: encoded.footprint().register_writes.clone(),
        memory: encoded.footprint().encoded.memory,
        stack: encoded.footprint().encoded.stack,
        trap: encoded.footprint().encoded.trap,
        flags_untouched: !encoded.footprint().writes_nzcv,
    }
}

#[test]
fn narrow_loads_bind_widths_footprints_opcodes_and_refuse_substituted_requests() {
    let physical =
        register_model::validate_physical_register_model(crate::aarch64_physical_register_model())
            .unwrap();
    narrow_load::conformance(
        &physical,
        &Roster {
            bases: &["x0", "x1", "x15", "x28"],
            destination: "x2",
            offsets: |width| vec![0, u32::from(width), 4095 * u32::from(width)],
        },
        &Exact {
            operands: ["x1", "x2"],
            expected: [(1, &[0x22, 0, 0x40, 0x39]), (2, &[0x22, 0, 0x40, 0x79])],
            signed_opcode: (2, [0x80, 0x80]),
            invalid_views: &["sp", "w1", "d0"],
            invalid_offsets: &[(2, 1), (2, 8192), (1, 4096)],
        },
        |kind, alternative, operands, displacement| {
            encode_aarch64_selected_memory_form(
                &physical,
                kind,
                alternative,
                operands,
                displacement,
            )
            .map(|encoded| observed(&encoded))
        },
        |kind, alternative, operands, displacement, bytes| {
            validate_aarch64_selected_memory_form(
                &physical,
                kind,
                alternative,
                operands,
                displacement,
                bytes,
            )
            .map(|encoded| observed(&encoded))
        },
    );
}
