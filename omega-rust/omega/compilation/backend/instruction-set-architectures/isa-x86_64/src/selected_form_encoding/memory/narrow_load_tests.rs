//! Exact narrow pointer reads reject opcode, operand and displacement substitutions.

use super::{encode_x86_64_selected_memory_form, validate_x86_64_selected_memory_form};
use crate::selected_form_encoding::ValidatedX86_64SelectedFormEncoding;
use isa_conformance::narrow_load::{self, Exact, Footprint, Roster};

/// Report one validated narrow load as the shared conformance footprint.
fn observed(encoded: &ValidatedX86_64SelectedFormEncoding) -> Footprint {
    Footprint {
        bytes: encoded.bytes().to_vec(),
        register_reads: encoded.footprint().register_reads.clone(),
        register_writes: encoded.footprint().register_writes.clone(),
        memory: encoded.footprint().encoded.memory,
        stack: encoded.footprint().encoded.stack,
        trap: encoded.footprint().encoded.trap,
        flags_untouched: !encoded.footprint().writes_rflags,
    }
}

#[test]
fn narrow_loads_bind_widths_footprints_opcodes_and_refuse_substituted_requests() {
    let physical =
        register_model::validate_physical_register_model(crate::x86_64_physical_register_model())
            .unwrap();
    narrow_load::conformance(
        &physical,
        &Roster {
            bases: &["rax", "rbp", "r12", "r13", "r15"],
            destination: "r11",
            offsets: |_width| vec![0, 2, 4096],
        },
        &Exact {
            operands: ["rax", "rcx"],
            expected: [
                (1, &[0x48, 0x0f, 0xb6, 0x88, 0, 0, 0, 0]),
                (2, &[0x48, 0x0f, 0xb7, 0x88, 0, 0, 0, 0]),
            ],
            signed_opcode: (2, [0xbe, 0xbf]),
            invalid_views: &["rsp", "eax", "xmm0"],
            invalid_offsets: &[(1, u32::MAX)],
        },
        |kind, alternative, operands, displacement| {
            encode_x86_64_selected_memory_form(&physical, kind, alternative, operands, displacement)
                .map(|encoded| observed(&encoded))
        },
        |kind, alternative, operands, displacement, bytes| {
            validate_x86_64_selected_memory_form(
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
