//! Per-architecture dispatch for the compact_binary v0 wire-encode appends.

use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::RuntimeStorageRegion;
use psi_diagnostics::Diagnostic;

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

pub fn encode_append_wire_text_bytes(
    architecture: Architecture,
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_append_wire_text_bytes(
            source_region,
            source_offset,
            out_offset,
            out_length,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::encode_append_wire_text_bytes(
            source_region,
            source_offset,
            out_offset,
            out_length,
            written_offset,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_append_wire_scalar_slice(
    architecture: Architecture,
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    element_byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_append_wire_scalar_slice(
            source_region,
            source_offset,
            element_byte_size,
            zigzag,
            out_offset,
            out_length,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::encode_append_wire_scalar_slice(
            source_region,
            source_offset,
            element_byte_size,
            zigzag,
            out_offset,
            out_length,
            written_offset,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_append_wire_repeated_scalar_varint(
    architecture: Architecture,
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    index: u64,
    count_region: RuntimeStorageRegion,
    count_offset: usize,
    out_offset: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_append_wire_repeated_scalar_varint(
            source_region,
            source_offset,
            byte_size,
            zigzag,
            index,
            count_region,
            count_offset,
            out_offset,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::encode_append_wire_repeated_scalar_varint(
            source_region,
            source_offset,
            byte_size,
            zigzag,
            index,
            count_region,
            count_offset,
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

    #[test]
    fn wire_text_bytes_widths_match_encoded_bytes() {
        const LENGTHS: &[usize] = &[1, 16, 64, 4096, 65536];
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &source_offset in OFFSETS {
                for &out_length in LENGTHS {
                    for &out_offset in OFFSETS {
                        for &written_offset in OFFSETS {
                            let bytes = encode_append_wire_text_bytes(
                                architecture,
                                RuntimeStorageRegion::RuntimeFrame,
                                source_offset,
                                out_offset,
                                out_length,
                                written_offset,
                            )
                            .expect("text bytes append should encode");
                            assert_eq!(
                                bytes.len(),
                                widths::append_wire_text_bytes_width(
                                    architecture,
                                    source_offset,
                                    out_offset,
                                    out_length,
                                    written_offset
                                ),
                                "{architecture:?} text bytes width drifted at source {source_offset} length {out_length} out {out_offset} written {written_offset}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn wire_scalar_slice_widths_match_encoded_bytes() {
        const LENGTHS: &[usize] = &[1, 16, 64, 4096, 65536];
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &source_offset in OFFSETS {
                for &out_length in LENGTHS {
                    for (element_byte_size, zigzag) in
                        [(1, false), (4, false), (4, true), (8, false), (8, true)]
                    {
                        let bytes = encode_append_wire_scalar_slice(
                            architecture,
                            RuntimeStorageRegion::RuntimeFrame,
                            source_offset,
                            element_byte_size,
                            zigzag,
                            64,
                            out_length,
                            72,
                        )
                        .expect("scalar-slice append should encode");
                        assert_eq!(
                            bytes.len(),
                            widths::append_wire_scalar_slice_width(
                                architecture,
                                source_offset,
                                element_byte_size,
                                zigzag,
                                64,
                                out_length,
                                72
                            ),
                            "{architecture:?} scalar-slice width drifted at source {source_offset} length {out_length} size {element_byte_size} zigzag {zigzag}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn wire_repeated_scalar_varint_widths_match_encoded_bytes() {
        const INDICES: &[u64] = &[0, 1, 7, 100, 5000];
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &source_offset in OFFSETS {
                for &count_offset in OFFSETS {
                    for &index in INDICES {
                        for (byte_size, zigzag) in
                            [(1, false), (4, false), (4, true), (8, false), (8, true)]
                        {
                            let bytes = encode_append_wire_repeated_scalar_varint(
                                architecture,
                                RuntimeStorageRegion::RuntimeFrame,
                                source_offset,
                                byte_size,
                                zigzag,
                                index,
                                RuntimeStorageRegion::RuntimeFrame,
                                count_offset,
                                64,
                                72,
                            )
                            .expect("repeated varint append should encode");
                            assert_eq!(
                                bytes.len(),
                                widths::append_wire_repeated_scalar_varint_width(
                                    architecture,
                                    source_offset,
                                    byte_size,
                                    zigzag,
                                    index,
                                    count_offset,
                                    64,
                                    72
                                ),
                                "{architecture:?} repeated varint width drifted at source {source_offset} count {count_offset} index {index} size {byte_size} zigzag {zigzag}"
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
                    // The text append shares the varint append's source page
                    // position (right after the shared prologue).
                    assert!(
                        source_page
                            < widths::append_wire_text_bytes_width(
                                architecture,
                                0,
                                out_offset,
                                64,
                                written_offset
                            )
                    );
                    assert!(
                        source_page
                            < widths::append_wire_scalar_slice_width(
                                architecture,
                                0,
                                4,
                                true,
                                out_offset,
                                64,
                                written_offset
                            )
                    );
                    // The repeated append's COUNT page sits right after the
                    // shared prologue and its SOURCE page after the guard,
                    // both inside the sequence.
                    let repeated_count = widths::wire_append_repeated_count_page_offset(
                        architecture,
                        out_offset,
                        written_offset,
                    );
                    let repeated_source = widths::wire_append_repeated_source_page_offset(
                        architecture,
                        out_offset,
                        written_offset,
                        8,
                        3,
                    );
                    assert!(written_page < repeated_count);
                    assert!(repeated_count < repeated_source);
                    assert!(
                        repeated_source
                            < widths::append_wire_repeated_scalar_varint_width(
                                architecture,
                                0,
                                8,
                                false,
                                3,
                                8,
                                out_offset,
                                written_offset
                            )
                    );
                }
            }
        }
    }
}
