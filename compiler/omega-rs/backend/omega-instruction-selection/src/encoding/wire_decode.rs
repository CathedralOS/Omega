//! Per-architecture dispatch for the compact_binary v0 wire-decode reads.

use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::RuntimeStorageRegion;
use psi_diagnostics::Diagnostic;

pub fn encode_read_wire_expected_byte(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    expected: u8,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_read_wire_expected_byte(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            expected,
        ),
        Architecture::X86_64 => x86_64::encode_read_wire_expected_byte(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            expected,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_read_wire_scalar_varint(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_read_wire_scalar_varint(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            range,
        ),
        Architecture::X86_64 => x86_64::encode_read_wire_scalar_varint(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            range,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_read_wire_byte_slice(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    predicate_mask: u8,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_read_wire_byte_slice(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_region,
            target_offset,
            predicate_mask,
        ),
        Architecture::X86_64 => x86_64::encode_read_wire_byte_slice(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_region,
            target_offset,
            predicate_mask,
        ),
    }
}

pub fn encode_read_wire_nested_open(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_read_wire_nested_open(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
        ),
        Architecture::X86_64 => x86_64::encode_read_wire_nested_open(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
        ),
    }
}

pub fn encode_read_wire_nested_close(
    architecture: Architecture,
    buffer_offset: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_read_wire_nested_close(
            buffer_offset,
            read_offset,
            ok_offset,
            end_offset,
        ),
        Architecture::X86_64 => {
            x86_64::encode_read_wire_nested_close(buffer_offset, read_offset, ok_offset, end_offset)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_read_wire_repeated_scalar_varint(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
    count_region: RuntimeStorageRegion,
    count_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_read_wire_repeated_scalar_varint(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            count_region,
            count_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            range,
        ),
        Architecture::X86_64 => x86_64::encode_read_wire_repeated_scalar_varint(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            count_region,
            count_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            range,
        ),
    }
}

// THE WIDTHS INVARIANT, pinned: every wire-decode encoder's emitted byte
// count must equal its width function for every parameter shape, on both
// architectures, or relocations drift and binaries segfault.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::widths;

    const OFFSETS: &[usize] = &[0, 4, 8, 64, 4000, 4096, 65536];
    const LENGTHS: &[usize] = &[1, 16, 64, 4096, 65536];

    #[test]
    fn wire_expected_byte_widths_match_encoded_bytes() {
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &buffer_offset in OFFSETS {
                for &buffer_length in LENGTHS {
                    for &read_offset in OFFSETS {
                        for &ok_offset in OFFSETS {
                            let bytes = encode_read_wire_expected_byte(
                                architecture,
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                ok_offset,
                                0x80,
                            )
                            .expect("expected-byte read should encode");
                            assert_eq!(
                                bytes.len(),
                                widths::read_wire_expected_byte_width(
                                    architecture,
                                    buffer_offset,
                                    buffer_length,
                                    read_offset,
                                    ok_offset
                                ),
                                "{architecture:?} expected-byte width drifted at buffer {buffer_offset} length {buffer_length} read {read_offset} ok {ok_offset}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn wire_scalar_varint_read_widths_match_encoded_bytes() {
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &buffer_offset in OFFSETS {
                for &buffer_length in LENGTHS {
                    for &target_offset in OFFSETS {
                        for (byte_size, zigzag) in
                            [(1, false), (4, false), (4, true), (8, false), (8, true)]
                        {
                            let bytes = encode_read_wire_scalar_varint(
                                architecture,
                                buffer_offset,
                                buffer_length,
                                64,
                                72,
                                RuntimeStorageRegion::RuntimeFrame,
                                target_offset,
                                byte_size,
                                zigzag,
                                None,
                            )
                            .expect("scalar varint read should encode");
                            assert_eq!(
                                bytes.len(),
                                widths::read_wire_scalar_varint_width(
                                    architecture,
                                    buffer_offset,
                                    buffer_length,
                                    64,
                                    72,
                                    target_offset,
                                    byte_size,
                                    zigzag,
                                    None
                                ),
                                "{architecture:?} varint read width drifted at buffer {buffer_offset} length {buffer_length} target {target_offset} size {byte_size} zigzag {zigzag}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn wire_nested_open_and_close_widths_match_encoded_bytes() {
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &buffer_offset in OFFSETS {
                for &buffer_length in LENGTHS {
                    for &end_offset in OFFSETS {
                        let bytes = encode_read_wire_nested_open(
                            architecture,
                            buffer_offset,
                            buffer_length,
                            64,
                            72,
                            end_offset,
                        )
                        .expect("nested open should encode");
                        assert_eq!(
                            bytes.len(),
                            widths::read_wire_nested_open_width(
                                architecture,
                                buffer_offset,
                                buffer_length,
                                64,
                                72,
                                end_offset
                            ),
                            "{architecture:?} nested open width drifted at buffer {buffer_offset} length {buffer_length} end {end_offset}"
                        );

                        let bytes = encode_read_wire_nested_close(
                            architecture,
                            buffer_offset,
                            64,
                            72,
                            end_offset,
                        )
                        .expect("nested close should encode");
                        assert_eq!(
                            bytes.len(),
                            widths::read_wire_nested_close_width(
                                architecture,
                                buffer_offset,
                                64,
                                72,
                                end_offset
                            ),
                            "{architecture:?} nested close width drifted at buffer {buffer_offset} end {end_offset}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn wire_repeated_scalar_varint_read_widths_match_encoded_bytes() {
        let ranges = [
            None,
            Some(psi_language_semantics::wire::WireScalarRange {
                minimum: 0,
                maximum: 100,
                signed: false,
            }),
            Some(psi_language_semantics::wire::WireScalarRange {
                minimum: -10,
                maximum: 10,
                signed: true,
            }),
        ];
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &buffer_offset in OFFSETS {
                for &buffer_length in LENGTHS {
                    for &target_offset in OFFSETS {
                        for &count_offset in OFFSETS {
                            for &range in &ranges {
                                for (byte_size, zigzag) in
                                    [(1, false), (4, false), (4, true), (8, false), (8, true)]
                                {
                                    let bytes = encode_read_wire_repeated_scalar_varint(
                                        architecture,
                                        buffer_offset,
                                        buffer_length,
                                        64,
                                        72,
                                        80,
                                        RuntimeStorageRegion::RuntimeFrame,
                                        count_offset,
                                        RuntimeStorageRegion::RuntimeFrame,
                                        target_offset,
                                        byte_size,
                                        zigzag,
                                        range,
                                    )
                                    .expect("repeated varint read should encode");
                                    assert_eq!(
                                        bytes.len(),
                                        widths::read_wire_repeated_scalar_varint_width(
                                            architecture,
                                            buffer_offset,
                                            buffer_length,
                                            64,
                                            72,
                                            80,
                                            count_offset,
                                            target_offset,
                                            byte_size,
                                            zigzag,
                                            range,
                                        ),
                                        "{architecture:?} repeated varint read width drifted at buffer {buffer_offset} length {buffer_length} target {target_offset} count {count_offset} size {byte_size} zigzag {zigzag} range {range:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// The relocation offsets must land exactly on the page/imm64
    /// materialization instructions inside the emitted bytes.
    #[test]
    fn wire_decode_relocation_offsets_stay_inside_the_sequences() {
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for &buffer_offset in OFFSETS {
                for &read_offset in OFFSETS {
                    let expected_width = widths::read_wire_expected_byte_width(
                        architecture,
                        buffer_offset,
                        64,
                        read_offset,
                        0,
                    );
                    let read_page =
                        widths::wire_decode_read_page_offset(architecture, buffer_offset);
                    let ok_page = widths::wire_decode_ok_page_offset(
                        architecture,
                        buffer_offset,
                        read_offset,
                    );
                    assert!(read_page < ok_page);
                    assert!(ok_page < expected_width);
                    for zigzag in [false, true] {
                        let target_page = widths::wire_decode_varint_target_page_offset(
                            architecture,
                            buffer_offset,
                            64,
                            read_offset,
                            zigzag,
                        );
                        assert!(ok_page < target_page);
                        assert!(
                            target_page
                                < widths::read_wire_scalar_varint_width(
                                    architecture,
                                    buffer_offset,
                                    64,
                                    read_offset,
                                    0,
                                    0,
                                    8,
                                    zigzag,
                                    None
                                )
                        );
                    }
                    // The nested open/close END-slot page sits right after the
                    // shared prologue, inside both sequences.
                    let end_page = widths::wire_decode_nested_end_page_offset(
                        architecture,
                        buffer_offset,
                        read_offset,
                    );
                    assert!(ok_page < end_page);
                    assert!(
                        end_page
                            < widths::read_wire_nested_close_width(
                                architecture,
                                buffer_offset,
                                read_offset,
                                0,
                                0
                            )
                    );
                    assert!(
                        end_page
                            < widths::read_wire_nested_open_width(
                                architecture,
                                buffer_offset,
                                64,
                                read_offset,
                                0,
                                0
                            )
                    );
                    // The repeated read shares the END page position with the
                    // nested decodes; its TARGET and COUNT pages follow in
                    // order, all inside the sequence.
                    for zigzag in [false, true] {
                        let repeated_target = widths::wire_decode_repeated_target_page_offset(
                            architecture,
                            buffer_offset,
                            64,
                            read_offset,
                            0,
                            zigzag,
                        );
                        let repeated_count = widths::wire_decode_repeated_count_page_offset(
                            architecture,
                            buffer_offset,
                            64,
                            read_offset,
                            0,
                            8,
                            8,
                            zigzag,
                            None,
                        );
                        assert!(end_page < repeated_target);
                        assert!(repeated_target < repeated_count);
                        assert!(
                            repeated_count
                                < widths::read_wire_repeated_scalar_varint_width(
                                    architecture,
                                    buffer_offset,
                                    64,
                                    read_offset,
                                    0,
                                    0,
                                    8,
                                    8,
                                    8,
                                    zigzag,
                                    None,
                                )
                        );
                    }
                }
            }
        }
    }
}
