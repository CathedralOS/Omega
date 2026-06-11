//! Per-architecture dispatch for the compact_binary v0 wire-encode appends.

use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::RuntimeStorageRegion;

pub fn encode_append_wire_literal_byte(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
    value: u8,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_append_wire_literal_byte(out_offset, written_offset, value)
        }
        Architecture::X86_64 => {
            x86_64::encode_append_wire_literal_byte(out_offset, written_offset, value)
        }
    }
}

pub fn encode_append_wire_scalar_varint(
    architecture: Architecture,
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_append_wire_scalar_varint(
            source_region,
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::encode_append_wire_scalar_varint(
            source_region,
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset,
        ),
    }
}

// THE WIDTHS INVARIANT, pinned: every wire-append encoder's emitted byte
// count must equal its width function for every parameter shape, on both
// architectures, or relocations drift and binaries segfault.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::widths;

    const OFFSETS: &[usize] = &[0, 4, 8, 64, 4000, 4096, 65536];

    #[test]
    fn wire_literal_byte_widths_match_encoded_bytes() {
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &out_offset in OFFSETS {
                for &written_offset in OFFSETS {
                    let bytes = encode_append_wire_literal_byte(
                        architecture,
                        out_offset,
                        written_offset,
                        0x80,
                    )
                    .expect("literal byte append should encode");
                    assert_eq!(
                        bytes.len(),
                        widths::append_wire_literal_byte_width(
                            architecture,
                            out_offset,
                            written_offset
                        ),
                        "{architecture:?} literal byte width drifted at out {out_offset} written {written_offset}"
                    );
                }
            }
        }
    }

    #[test]
    fn wire_scalar_varint_widths_match_encoded_bytes() {
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &source_offset in OFFSETS {
                for &out_offset in OFFSETS {
                    for &written_offset in OFFSETS {
                        for (byte_size, zigzag) in
                            [(1, false), (4, false), (4, true), (8, false), (8, true)]
                        {
                            let bytes = encode_append_wire_scalar_varint(
                                architecture,
                                RuntimeStorageRegion::RuntimeFrame,
                                source_offset,
                                byte_size,
                                zigzag,
                                out_offset,
                                written_offset,
                            )
                            .expect("scalar varint append should encode");
                            assert_eq!(
                                bytes.len(),
                                widths::append_wire_scalar_varint_width(
                                    architecture,
                                    source_offset,
                                    byte_size,
                                    zigzag,
                                    out_offset,
                                    written_offset
                                ),
                                "{architecture:?} varint width drifted at source {source_offset} size {byte_size} zigzag {zigzag} out {out_offset} written {written_offset}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// The relocation offsets must land exactly on the page/imm64
    /// materialization instructions inside the emitted bytes.
    #[test]
    fn wire_append_relocation_offsets_stay_inside_the_prologue() {
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &out_offset in OFFSETS {
                for &written_offset in OFFSETS {
                    let literal_width = widths::append_wire_literal_byte_width(
                        architecture,
                        out_offset,
                        written_offset,
                    );
                    let written_page =
                        widths::wire_append_written_page_offset(architecture, out_offset);
                    assert!(written_page < literal_width);
                    let source_page = widths::wire_append_varint_source_page_offset(
                        architecture,
                        out_offset,
                        written_offset,
                    );
                    assert!(written_page < source_page);
                    assert!(
                        source_page
                            < widths::append_wire_scalar_varint_width(
                                architecture,
                                0,
                                8,
                                false,
                                out_offset,
                                written_offset
                            )
                    );
                }
            }
        }
    }
}
