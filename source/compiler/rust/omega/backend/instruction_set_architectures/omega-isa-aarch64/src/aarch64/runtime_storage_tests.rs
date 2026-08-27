use super::super::primitives::{
    encode_csinv_x, encode_movk, encode_msub_x_register, encode_sdiv_x_register,
    encode_sub_x_register, encode_udiv_x_register,
};
use super::super::widths;
use super::*;

#[test]
fn direct_copy_clobbers_track_base_data_and_large_offset_scratch() {
    assert_eq!(
        runtime_storage_copy_clobbers(0, 0, 0).as_slice(),
        &[MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17),]
    );
    assert_eq!(
        runtime_storage_copy_clobbers(4096, 0, 8).as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(26),
        ]
    );
    assert_eq!(
        runtime_storage_copy_clobbers(0, 0, 32_776).as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(26),
        ]
    );
}

#[test]
fn from_pointee_clobbers_track_base_data_and_large_offset_scratch() {
    assert_eq!(
        runtime_storage_copy_from_runtime_pointee_clobbers(0, 0, 0, 0).as_slice(),
        &[MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(20),]
    );
    assert_eq!(
        runtime_storage_copy_from_runtime_pointee_clobbers(0, 4096, 0, 8).as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
        ]
    );
    assert_eq!(
        runtime_storage_copy_from_runtime_pointee_clobbers(0, 0, 0, 32_776).as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(26),
        ]
    );
}

#[test]
fn pointee_pair_clobbers_track_fields_data_and_large_offset_scratch() {
    assert_eq!(
        runtime_storage_copy_pointee_pair_clobbers(0, 0, 0).as_slice(),
        &[MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(20),]
    );
    assert_eq!(
        runtime_storage_copy_pointee_pair_clobbers(4096, 0, 8).as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
        ]
    );
    assert_eq!(
        runtime_storage_copy_pointee_pair_clobbers(0, 0, 32_776).as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(26),
        ]
    );
}

#[test]
fn from_indexed_clobbers_cover_address_formation_and_copy_chunks() {
    assert_eq!(
        runtime_storage_copy_from_runtime_frame_indexed_clobbers().as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(21),
            MachineRegister::Aarch64X(26),
        ]
    );
    assert_eq!(
        runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(8, 16),
        widths::runtime_frame_index_setup_width(8, 16)
    );
}

#[test]
fn to_indexed_clobbers_match_the_frame_index_address_contract() {
    assert_eq!(
        runtime_storage_copy_to_runtime_frame_indexed_clobbers(),
        runtime_storage_copy_from_runtime_frame_indexed_clobbers()
    );
}

#[test]
fn indexed_to_pointee_clobbers_match_the_frame_index_address_contract() {
    assert_eq!(
        runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_clobbers(),
        runtime_storage_copy_from_runtime_frame_indexed_clobbers()
    );
}

#[test]
fn frame_base_indexed_clobbers_match_the_inline_array_encoder() {
    assert_eq!(
        runtime_storage_copy_from_runtime_frame_base_indexed_clobbers().as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(24),
            MachineRegister::Aarch64X(26),
        ]
    );
}

#[test]
fn frame_indexed_integer_write_clobbers_include_cross_region_base() {
    assert_eq!(
        runtime_frame_indexed_integer_write_clobbers(
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        )
        .as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(21),
            MachineRegister::Aarch64X(26),
        ]
    );
    assert_eq!(
        runtime_frame_indexed_integer_write_clobbers(
            omega_target_operations::RuntimeStorageRegion::Machine,
        )
        .as_slice(),
        &[
            MachineRegister::Aarch64X(15),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(21),
            MachineRegister::Aarch64X(26),
        ]
    );
}

#[test]
fn frame_base_indexed_integer_write_clobbers_cover_inline_address_recipe() {
    assert_eq!(
        runtime_frame_base_indexed_integer_write_clobbers().as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(26),
        ]
    );
}

#[test]
fn machine_indexed_integer_write_clobbers_cover_inline_address_recipe() {
    assert_eq!(
        runtime_machine_indexed_integer_write_clobbers().as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(26),
        ]
    );
}

#[test]
fn double_indexed_integer_write_clobbers_track_shared_frame_base() {
    assert_eq!(
        runtime_machine_double_indexed_integer_write_clobbers(
            omega_target_operations::RuntimeStorageRegion::Machine,
            omega_target_operations::RuntimeStorageRegion::Machine,
        )
        .as_slice(),
        &[
            MachineRegister::Aarch64X(14),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(26),
        ]
    );
    assert_eq!(
        runtime_machine_double_indexed_integer_write_clobbers(
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            omega_target_operations::RuntimeStorageRegion::Machine,
        )
        .as_slice(),
        &[
            MachineRegister::Aarch64X(14),
            MachineRegister::Aarch64X(15),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(26),
        ]
    );
}

#[test]
fn pointee_double_indexed_read_write_keep_shared_base_sites_and_clobbers() {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    let write = encode_runtime_pointee_double_indexed_integer_write(
        0, machine, 24, 8, 8, machine, 32, 8, 2, 4, 2, 17,
    )
    .expect("encode pointee double-indexed write");
    let read = encode_runtime_storage_copy_from_runtime_pointee_double_indexed_to_runtime_storage(
        0, machine, 24, 8, 8, machine, 32, 8, 2, 4, machine, 40, 2,
    )
    .expect("encode pointee double-indexed read");

    for bytes in [&write, &read] {
        assert!(bytes.len() > 40);
        assert_eq!(&bytes[0..4], &encode_adrp_placeholder(20));
        assert_eq!(&bytes[4..8], &encode_add_page_offset_placeholder(20));
        assert_eq!(&bytes[32..36], &encode_adrp_placeholder(15));
        assert_eq!(&bytes[36..40], &encode_add_page_offset_placeholder(15));
    }
    assert_eq!(
        runtime_pointee_double_indexed_integer_write_clobbers(machine, machine).as_slice(),
        &[
            MachineRegister::Aarch64X(14),
            MachineRegister::Aarch64X(15),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(26),
        ]
    );
    assert_eq!(
        runtime_storage_copy_from_runtime_pointee_double_indexed_clobbers(
            machine, machine, machine,
        ),
        runtime_pointee_double_indexed_integer_write_clobbers(machine, machine)
    );
    assert!(
        !runtime_pointee_double_indexed_integer_write_clobbers(frame, frame)
            .as_slice()
            .contains(&MachineRegister::Aarch64X(15))
    );
}

#[test]
fn machine_indexed_copy_clobbers_match_the_address_recipe() {
    assert_eq!(
        runtime_storage_copy_from_runtime_machine_indexed_clobbers().as_slice(),
        &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(26),
        ]
    );
}

#[test]
fn to_machine_indexed_clobbers_match_the_address_recipe() {
    assert_eq!(
        runtime_storage_copy_to_runtime_machine_indexed_clobbers(),
        runtime_storage_copy_from_runtime_machine_indexed_clobbers()
    );
}

#[test]
fn bounded_buffer_literal_write_rebases_large_machine_offsets() {
    let bytes = encode_runtime_machine_bounded_buffer_write(5072, b"torch")
        .expect("large carrier offset encodes");
    assert_eq!(
        bytes.len(),
        widths::runtime_machine_bounded_buffer_write_width(5072, b"torch")
    );
}

#[test]
fn indexed_bounded_buffer_writes_match_their_widths() {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    for index_region in [frame, machine] {
        let frame_indexed = encode_runtime_frame_indexed_bounded_buffer_write(
            24,
            index_region,
            8,
            8,
            16,
            0,
            b"Gate",
        )
        .expect("frame-indexed bounded-buffer write");
        assert_eq!(
            frame_indexed.len(),
            super::super::widths::runtime_frame_indexed_bounded_buffer_write_width(
                index_region,
                16,
                0,
                b"Gate",
            )
        );

        let machine_indexed = encode_runtime_machine_indexed_bounded_buffer_write(
            24,
            index_region,
            8,
            8,
            16,
            0,
            b"Gate",
        )
        .expect("machine-indexed bounded-buffer write");
        assert_eq!(
            machine_indexed.len(),
            super::super::widths::runtime_machine_indexed_bounded_buffer_write_width(
                24,
                index_region,
                8,
                8,
                16,
                0,
                b"Gate",
            )
        );
    }

    let frame_base =
        encode_runtime_frame_base_indexed_bounded_buffer_write(24, 8, 8, 16, 0, b"Gate")
            .expect("frame-base-indexed bounded-buffer write");
    assert_eq!(
        frame_base.len(),
        super::super::widths::runtime_frame_base_indexed_bounded_buffer_write_width(
            24, 8, 8, 16, 0, b"Gate",
        )
    );

    let cross_region_frame_base =
        encode_runtime_frame_base_indexed_bounded_buffer_write_with_index_region(
            24, machine, 8, 8, 16, 0, b"Gate",
        )
        .expect("cross-region frame-base-indexed bounded-buffer write");
    assert_eq!(
            cross_region_frame_base.len(),
            super::super::widths::runtime_frame_base_indexed_bounded_buffer_write_with_index_region_width(
                24, machine, 8, 8, 16, 0, b"Gate",
            )
        );
    let index_site = widths::runtime_frame_base_indexed_machine_index_base_offset(24);
    assert_eq!(
        &cross_region_frame_base[index_site..index_site + 8],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "the machine-held carrier index must own an x15 base pair"
    );

    for (outer_region, inner_region) in [
        (machine, machine),
        (frame, machine),
        (machine, frame),
        (frame, frame),
    ] {
        let double = encode_runtime_machine_double_indexed_bounded_buffer_write(
            24,
            8,
            outer_region,
            8,
            32,
            16,
            inner_region,
            8,
            16,
            0,
            b"Gate",
        )
        .expect("double-indexed bounded-buffer write");
        assert_eq!(
            double.len(),
            super::super::widths::runtime_machine_double_indexed_bounded_buffer_write_width(
                outer_region,
                inner_region,
                b"Gate",
            )
        );
    }
}

#[test]
fn indexed_bounded_buffer_literal_appends_match_their_widths() {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    for index_region in [frame, machine] {
        let frame_indexed = encode_runtime_frame_indexed_bounded_buffer_literal_append(
            24,
            index_region,
            8,
            8,
            16,
            0,
            b"te",
        )
        .expect("frame-indexed bounded-buffer literal append");
        assert_eq!(
            frame_indexed.len(),
            super::super::widths::runtime_frame_indexed_bounded_buffer_literal_append_width(
                index_region,
                16,
                0,
                b"te",
            )
        );

        let machine_indexed = encode_runtime_machine_indexed_bounded_buffer_literal_append(
            24,
            index_region,
            8,
            8,
            16,
            0,
            b"te",
        )
        .expect("machine-indexed bounded-buffer literal append");
        assert_eq!(
            machine_indexed.len(),
            super::super::widths::runtime_machine_indexed_bounded_buffer_literal_append_width(
                24,
                index_region,
                8,
                8,
                16,
                0,
                b"te",
            )
        );
    }

    let frame_base =
        encode_runtime_frame_base_indexed_bounded_buffer_literal_append(24, 8, 8, 16, 0, b"te")
            .expect("frame-base-indexed bounded-buffer literal append");
    assert_eq!(
        frame_base.len(),
        super::super::widths::runtime_frame_base_indexed_bounded_buffer_literal_append_width(
            24, 8, 8, 16, 0, b"te",
        )
    );

    let cross_region_frame_base =
        encode_runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region(
            24, machine, 8, 8, 16, 0, b"te",
        )
        .expect("cross-region frame-base-indexed bounded-buffer literal append");
    assert_eq!(
            cross_region_frame_base.len(),
            super::super::widths::runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region_width(
                24, machine, 8, 8, 16, 0, b"te",
            )
        );
    let index_site = widths::runtime_frame_base_indexed_machine_index_base_offset(24);
    assert_eq!(
        &cross_region_frame_base[index_site..index_site + 8],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "the machine-held literal-append index must own an x15 base pair"
    );

    let frame_double = encode_runtime_frame_base_double_indexed_bounded_buffer_literal_append(
        24, 64, 8, 48, 72, 8, 16, 8, b"te",
    )
    .expect("frame-double-indexed bounded-buffer literal append");
    assert_eq!(
        frame_double.len(),
        widths::runtime_frame_base_double_indexed_bounded_buffer_literal_append_width(b"te")
    );
    assert_eq!(
        &frame_double[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat(),
        "the frame-double literal append must reuse one frame base"
    );

    for (outer_region, inner_region) in [
        (machine, machine),
        (frame, machine),
        (machine, frame),
        (frame, frame),
    ] {
        let double = encode_runtime_machine_double_indexed_bounded_buffer_literal_append(
            24,
            8,
            outer_region,
            8,
            32,
            16,
            inner_region,
            8,
            16,
            0,
            b"te",
        )
        .expect("double-indexed bounded-buffer literal append");
        assert_eq!(
                double.len(),
                super::super::widths::runtime_machine_double_indexed_bounded_buffer_literal_append_width(
                    outer_region,
                    inner_region,
                    b"te",
                )
            );
    }
}

#[test]
fn indexed_bounded_buffer_source_appends_retain_the_direct_source_site() {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    let source = omega_target_operations::Place::at(frame, 40);

    for index_region in [frame, machine] {
        for (_, sites) in [
            encode_runtime_frame_indexed_bounded_buffer_source_append(
                24,
                index_region,
                8,
                8,
                16,
                0,
                &source,
            )
            .expect("frame-indexed bounded-buffer source append"),
            encode_runtime_machine_indexed_bounded_buffer_source_append(
                24,
                index_region,
                8,
                8,
                16,
                0,
                &source,
            )
            .expect("machine-indexed bounded-buffer source append"),
        ] {
            assert_eq!(
                sites.iter().collect::<Vec<_>>(),
                vec![(
                    sites.iter().next().expect("source relocation").0,
                    super::super::place_bounded_buffer::BoundedBufferPlaceSide::Source,
                )]
            );
        }
    }

    let (_, sites) =
        encode_runtime_frame_base_indexed_bounded_buffer_source_append(24, 8, 8, 16, 0, &source)
            .expect("frame-base-indexed bounded-buffer source append");
    assert_eq!(sites.iter().count(), 1);

    let (cross_region, sites) =
        encode_runtime_frame_base_indexed_bounded_buffer_source_append_with_index_region(
            24, machine, 8, 8, 16, 0, &source,
        )
        .expect("cross-region frame-base-indexed bounded-buffer source append");
    assert_eq!(sites.iter().count(), 1);
    let index_site = widths::runtime_frame_base_indexed_machine_index_base_offset(24);
    assert_eq!(
        &cross_region[index_site..index_site + 8],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "the machine-held source-append index must own an x15 base pair"
    );

    let (frame_double, sites) =
        encode_runtime_frame_base_double_indexed_bounded_buffer_source_append(
            24, 64, 8, 48, 72, 8, 16, 8, &source,
        )
        .expect("frame-double-indexed bounded-buffer source append");
    assert_eq!(sites.iter().count(), 1);
    assert_eq!(
        &frame_double[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat(),
        "the frame-double source append must reuse one frame base"
    );

    for (outer_region, inner_region) in [
        (machine, machine),
        (frame, machine),
        (machine, frame),
        (frame, frame),
    ] {
        let (_, sites) = encode_runtime_machine_double_indexed_bounded_buffer_source_append(
            24,
            8,
            outer_region,
            8,
            32,
            16,
            inner_region,
            8,
            16,
            0,
            &source,
        )
        .expect("double-indexed bounded-buffer source append");
        assert_eq!(sites.iter().count(), 1);
    }
}

#[test]
fn string_descriptor_write_materializes_large_machine_offsets() {
    encode_runtime_machine_string_write(37_024, 12)
        .expect("large String descriptor offset encodes");
}

#[test]
fn frame_base_indexed_address_write_materializes_a_machine_index_base() {
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    let bytes = encode_runtime_frame_base_indexed_address_to_runtime_frame_write_with_index_region(
        24, machine, 8, 8, 16, 0, 40,
    )
    .expect("cross-region frame-base-indexed address write");
    assert_eq!(
            bytes.len(),
            super::super::widths::runtime_frame_base_indexed_address_to_runtime_frame_write_with_index_region_width(
                24, machine, 8, 8, 16, 0, 40,
            )
        );
    let index_site = widths::runtime_frame_base_indexed_machine_index_base_offset(24);
    assert_eq!(
        &bytes[index_site..index_site + 8],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat()
    );
    assert!(
        runtime_frame_base_indexed_address_to_runtime_frame_write_clobbers_with_index_region(
            machine,
        )
        .contains(MachineRegister::Aarch64X(15))
    );
}

#[test]
fn frame_base_indexed_string_write_width_matches_emission() {
    for index_byte_size in [1usize, 2, 4, 8] {
        let bytes =
            encode_runtime_frame_base_indexed_string_write(24, 8, index_byte_size, 16, 0, 7)
                .expect("frame-base-indexed string descriptor write");
        assert_eq!(
            bytes.len(),
            widths::runtime_frame_base_indexed_string_write_width(24, 8, index_byte_size, 16, 0, 7,)
        );
        let data_site = super::super::widths::runtime_frame_base_indexed_string_data_address_offset(
            24,
            8,
            index_byte_size,
            16,
            0,
        );
        assert_eq!(bytes.len() - data_site, 20);
    }
}

#[test]
fn frame_base_indexed_string_write_materializes_a_machine_index_base() {
    let bytes = encode_runtime_frame_base_indexed_string_write_with_index_region(
        24,
        omega_target_operations::RuntimeStorageRegion::Machine,
        64,
        8,
        16,
        0,
        7,
    )
    .expect("cross-region frame-base-indexed string descriptor write");
    assert_eq!(
        bytes.len(),
        widths::runtime_frame_base_indexed_string_write_with_index_region_width(
            24,
            omega_target_operations::RuntimeStorageRegion::Machine,
            64,
            8,
            16,
            0,
            7,
        )
    );
    let index_site = widths::runtime_frame_base_indexed_machine_index_base_offset(24);
    assert_eq!(
        &bytes[index_site..index_site + 8],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "the machine-held string index must own an x15 base pair"
    );
    let data_site = widths::runtime_frame_base_indexed_string_data_address_offset_with_index_region(
        24,
        omega_target_operations::RuntimeStorageRegion::Machine,
        64,
        8,
        16,
        0,
    );
    assert_eq!(bytes.len() - data_site, 20);
}

#[test]
fn cross_region_frame_indexed_string_write_width_matches_emission() {
    for index_byte_size in [1usize, 2, 4, 8] {
        let bytes = encode_runtime_frame_indexed_string_write_with_index_region(
            24,
            omega_target_operations::RuntimeStorageRegion::Machine,
            8,
            index_byte_size,
            16,
            0,
            7,
        )
        .expect("cross-region frame-indexed string descriptor write");
        assert_eq!(
            bytes.len(),
            runtime_frame_indexed_string_write_width_with_index_region(
                omega_target_operations::RuntimeStorageRegion::Machine,
                16,
                0,
                7,
            )
        );
        let data_site = super::super::widths::runtime_frame_indexed_string_data_address_offset_with_index_region(
                omega_target_operations::RuntimeStorageRegion::Machine,
                16,
                0,
            );
        assert_eq!(bytes.len() - data_site, 20);
        assert!(super::super::widths::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET < data_site);
    }
}

#[test]
fn machine_indexed_string_write_with_machine_index_matches_width() {
    for index_byte_size in [1usize, 2, 4, 8] {
        let bytes = encode_runtime_machine_indexed_string_write_with_index_region(
            24,
            omega_target_operations::RuntimeStorageRegion::Machine,
            8,
            index_byte_size,
            16,
            0,
            7,
        )
        .expect("machine-indexed string descriptor write with machine index");
        assert_eq!(
            bytes.len(),
            runtime_machine_indexed_string_write_width_with_index_region(
                24,
                omega_target_operations::RuntimeStorageRegion::Machine,
                8,
                index_byte_size,
                16,
                0,
                7,
            )
        );
        let data_site = super::super::widths::runtime_machine_indexed_string_data_address_offset_with_index_region(
                24,
                omega_target_operations::RuntimeStorageRegion::Machine,
                8,
                index_byte_size,
                16,
                0,
            );
        assert_eq!(bytes.len() - data_site, 20);
    }
}

#[test]
fn machine_double_indexed_string_write_matches_width_and_data_site() {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    for (outer_region, inner_region) in [
        (machine, machine),
        (frame, machine),
        (machine, frame),
        (frame, frame),
    ] {
        let bytes = encode_runtime_machine_double_indexed_string_write(
            24,
            8,
            outer_region,
            8,
            32,
            16,
            inner_region,
            8,
            16,
            0,
            7,
        )
        .expect("double-indexed string descriptor write");
        assert_eq!(
            bytes.len(),
            super::super::widths::runtime_machine_double_indexed_string_write_width(
                outer_region,
                inner_region,
                7,
            )
        );
        let data_site =
            super::super::widths::runtime_machine_double_indexed_string_data_address_offset(
                outer_region,
                inner_region,
            );
        assert_eq!(bytes.len() - data_site, 20);
    }
}

#[test]
fn float_classification_sequences_stay_in_width_lockstep() {
    for byte_size in [4usize, 8] {
        for operator in [
            StateGuardOperator::IsFinite,
            StateGuardOperator::IsInfinite,
            StateGuardOperator::IsNormal,
            StateGuardOperator::IsSubnormal,
        ] {
            let mut bytes = Vec::new();
            append_runtime_float_binary_operation(
                &mut bytes,
                byte_size,
                17,
                operator,
                26,
                psi_numerics::arithmetic::ArithmeticDomain::Exact,
                [15, 14],
            )
            .expect("encode float classification");
            assert_eq!(
                bytes.len(),
                float_classification_predicate_width(operator, byte_size),
                "f{} {operator:?} width",
                byte_size * 8,
            );
        }
        let mut bytes = Vec::new();
        append_runtime_float_binary_operation(
            &mut bytes,
            byte_size,
            17,
            StateGuardOperator::FloatClassify,
            26,
            psi_numerics::arithmetic::ArithmeticDomain::Exact,
            [15, 14],
        )
        .expect("encode enum float classification");
        assert_eq!(
            bytes.len(),
            float_classify_width(byte_size),
            "f{} FloatClassify width",
            byte_size * 8,
        );
    }
}

/// `LDADDAL <Ws/Xs>, <Wt/Xt>, [<Xn>]` per width: the size field selects the
/// access size, the acquire+release bits are set, and Rt receives the prior.
#[test]
fn ldadd_encodes_per_width_and_ordering() {
    // (byte_size, expected size field in bits 31:30)
    for &(byte_size, size) in &[(1usize, 0u32), (2, 1), (4, 2), (8, 3)] {
        let bytes = encode_ldadd(
            byte_size,
            17,
            26,
            16,
            psi_language_core::MemoryOrdering::ReceivePublish,
        )
        .expect("encode");
        assert_eq!(bytes.len(), 4, "atomic add is a single instruction");
        let word = u32::from_le_bytes(bytes[..].try_into().unwrap());
        let expected = 0x38E0_0000 | (size << 30) | (17u32 << 16) | (16u32 << 5) | 26;
        assert_eq!(word, expected, "byte_size={byte_size}");
        assert_eq!(word >> 30, size, "size field");
        assert_eq!((word >> 22) & 0b11, 0b11, "acquire+release ordering bits");
        assert_eq!((word >> 16) & 0x1F, 17, "Rs = add register");
        assert_eq!((word >> 5) & 0x1F, 16, "Rn = address register");
        assert_eq!(word & 0x1F, 26, "Rt = prior-value result register");
    }
    assert!(
        encode_ldadd(3, 17, 26, 16, psi_language_core::MemoryOrdering::NoOrdering,).is_err(),
        "non-power-of-two width must error, not miscompile"
    );
    let words = [
        psi_language_core::MemoryOrdering::NoOrdering,
        psi_language_core::MemoryOrdering::Receive,
        psi_language_core::MemoryOrdering::Publish,
        psi_language_core::MemoryOrdering::ReceivePublish,
        psi_language_core::MemoryOrdering::GlobalOrder,
    ]
    .map(|ordering| u32::from_le_bytes(encode_ldadd(4, 17, 26, 16, ordering).unwrap()));
    assert_eq!(
        words,
        [
            0xB831_021A,
            0xB8B1_021A,
            0xB871_021A,
            0xB8F1_021A,
            0xB8F1_021A,
        ]
    );
}

#[test]
fn swp_encodes_per_width_and_ordering() {
    let words = [
        psi_language_core::MemoryOrdering::NoOrdering,
        psi_language_core::MemoryOrdering::Receive,
        psi_language_core::MemoryOrdering::Publish,
        psi_language_core::MemoryOrdering::ReceivePublish,
        psi_language_core::MemoryOrdering::GlobalOrder,
    ]
    .map(|ordering| u32::from_le_bytes(encode_swp(4, 17, 26, 16, ordering).unwrap()));
    assert_eq!(
        words,
        [
            0xB831_821A,
            0xB8B1_821A,
            0xB871_821A,
            0xB8F1_821A,
            0xB8F1_821A,
        ]
    );
    for byte_size in [1usize, 2, 4, 8] {
        assert!(
            encode_swp(
                byte_size,
                17,
                26,
                16,
                psi_language_core::MemoryOrdering::ReceivePublish,
            )
            .is_ok()
        );
    }
    assert!(encode_swp(3, 17, 26, 16, psi_language_core::MemoryOrdering::NoOrdering).is_err());
}

#[test]
fn atomic_load_store_select_no_ordering_and_ordered_encodings() {
    use psi_language_core::MemoryOrdering as O;

    assert_eq!(
        u32::from_le_bytes(encode_atomic_load(17, 16, 4, O::NoOrdering).unwrap()),
        0xB940_0211
    );
    assert_eq!(
        u32::from_le_bytes(encode_atomic_load(17, 16, 4, O::Receive).unwrap()),
        0x88DF_FE11
    );
    assert_eq!(
        u32::from_le_bytes(encode_atomic_store(17, 16, 4, O::NoOrdering).unwrap()),
        0xB900_0211
    );
    assert_eq!(
        u32::from_le_bytes(encode_atomic_store(17, 16, 4, O::Publish).unwrap()),
        0x889F_FE11
    );
    assert!(encode_atomic_load(17, 16, 4, O::Publish).is_err());
    assert!(encode_atomic_store(17, 16, 4, O::Receive).is_err());
}

/// The full `encode_atomic_fetch_add` path: the emitted length must equal
/// its width function at every offset, and its RMW must be
/// `LDADDAL w17, w26, [x16]`. The delta is an immediate so the operand load
/// is offset-independent.
#[test]
fn atomic_fetch_add_encoder_matches_width_and_ends_in_ldaddal() {
    use omega_target_operations::RuntimeValueOperand;
    use psi_arena::Arena;

    for &target_offset in &[0usize, 8, 4095] {
        let mut operands: Arena<RuntimeValueOperand> = Arena::default();
        let delta = operands.insert(RuntimeValueOperand::Immediate(5));
        let result_offset = 24;
        let bytes = encode_atomic_fetch_add(
            &operands,
            target_offset,
            4,
            result_offset,
            delta,
            psi_language_core::MemoryOrdering::ReceivePublish,
        )
        .expect("encode");
        assert_eq!(
            bytes.len(),
            runtime_atomic_fetch_add_width(&operands, target_offset, 4, result_offset, delta,),
            "width mismatch at offset {target_offset}"
        );
        let atomic_end =
            runtime_atomic_fetch_add_result_address_offset(&operands, target_offset, delta);
        let last = u32::from_le_bytes(bytes[atomic_end - 4..atomic_end].try_into().unwrap());
        assert_eq!(
            last, 0xB8F1_021A,
            "atomic instruction must be LDADDAL w17, w26, [x16] at offset {target_offset}"
        );
    }

    // An offset past the single ADD-immediate reach errors, not miscompiles.
    let mut operands: Arena<RuntimeValueOperand> = Arena::default();
    let delta = operands.insert(RuntimeValueOperand::Immediate(1));
    assert!(
        encode_atomic_fetch_add(
            &operands,
            4096,
            4,
            0,
            delta,
            psi_language_core::MemoryOrdering::NoOrdering,
        )
        .is_err()
    );
}

#[test]
fn atomic_fetch_sub_negates_at_width_then_uses_ldaddal() {
    use omega_target_operations::RuntimeValueOperand;
    use psi_arena::Arena;

    let mut operands: Arena<RuntimeValueOperand> = Arena::default();
    let delta = operands.insert(RuntimeValueOperand::Immediate(12));
    let bytes = encode_atomic_fetch_sub(
        &operands,
        0,
        4,
        24,
        delta,
        psi_language_core::MemoryOrdering::ReceivePublish,
    )
    .expect("encode");
    assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_sub_width(&operands, 0, 4, 24, delta)
    );
    let atomic_end = runtime_atomic_fetch_sub_result_address_offset(&operands, 0, delta);
    assert_eq!(
        u32::from_le_bytes(bytes[atomic_end - 8..atomic_end - 4].try_into().unwrap()),
        0x4B11_03F1,
        "fetch_sub must emit SUB w17,wzr,w17"
    );
    assert_eq!(
        u32::from_le_bytes(bytes[atomic_end - 4..atomic_end].try_into().unwrap()),
        0xB8F1_021A,
        "fetch_sub must emit LDADDAL w17,w26,[x16]"
    );
}

#[test]
fn atomic_fetch_xor_uses_ordered_ldeor_and_returns_prior() {
    use omega_target_operations::RuntimeValueOperand;
    use psi_arena::Arena;

    let mut operands: Arena<RuntimeValueOperand> = Arena::default();
    let value = operands.insert(RuntimeValueOperand::Immediate(12));
    let bytes = encode_atomic_fetch_xor(
        &operands,
        0,
        4,
        24,
        value,
        psi_language_core::MemoryOrdering::ReceivePublish,
    )
    .expect("encode");
    assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_xor_width(&operands, 0, 4, 24, value)
    );
    let atomic_end = runtime_atomic_fetch_xor_result_address_offset(&operands, 0, value);
    assert_eq!(
        u32::from_le_bytes(bytes[atomic_end - 4..atomic_end].try_into().unwrap()),
        0xB8F1_221A,
        "fetch_xor must emit LDEORAL w17,w26,[x16]"
    );
}

#[test]
fn atomic_fetch_or_uses_ordered_ldset_and_returns_prior() {
    use omega_target_operations::RuntimeValueOperand;
    use psi_arena::Arena;

    let mut operands: Arena<RuntimeValueOperand> = Arena::default();
    let value = operands.insert(RuntimeValueOperand::Immediate(5));
    let bytes = encode_atomic_fetch_or(
        &operands,
        0,
        4,
        24,
        value,
        psi_language_core::MemoryOrdering::ReceivePublish,
    )
    .expect("encode");
    assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_or_width(&operands, 0, 4, 24, value)
    );
    let atomic_end = runtime_atomic_fetch_or_result_address_offset(&operands, 0, value);
    assert_eq!(
        u32::from_le_bytes(bytes[atomic_end - 4..atomic_end].try_into().unwrap()),
        0xB8F1_321A,
        "fetch_or must emit LDSETAL w17,w26,[x16]"
    );
}

/// `CASAL <Ws/Xs>, <Wt/Xt>, [<Xn>]` per width: size field selects the access
/// size, Rs (bits 20:16) = compare/expected, Rn (bits 9:5) = address, Rt
/// (bits 4:0) = new value, with the acquire(L)/release(o0)/Rt2 fixed bits set.
#[test]
fn casal_encodes_per_width() {
    use super::super::primitives::encode_cas;
    for &(byte_size, size) in &[(1usize, 0u32), (2, 1), (4, 2), (8, 3)] {
        let word = u32::from_le_bytes(
            encode_cas(
                byte_size,
                26,
                17,
                16,
                psi_language_core::MemoryOrdering::ReceivePublish,
            )
            .expect("encode")[..]
                .try_into()
                .unwrap(),
        );
        let expected = 0x08E0_FC00 | (size << 30) | (26u32 << 16) | (16u32 << 5) | 17;
        assert_eq!(word, expected, "byte_size={byte_size}");
        assert_eq!(word >> 30, size, "size field");
        assert_eq!((word >> 16) & 0x1F, 26, "Rs = expected (compare/old)");
        assert_eq!((word >> 5) & 0x1F, 16, "Rn = address register");
        assert_eq!(word & 0x1F, 17, "Rt = new value");
        assert_eq!((word >> 10) & 0x1F, 0x1F, "Rt2 fixed 11111");
    }
    assert!(
        encode_cas(3, 26, 17, 16, psi_language_core::MemoryOrdering::NoOrdering,).is_err(),
        "non-power-of-two errors"
    );
    let words = [
        psi_language_core::MemoryOrdering::NoOrdering,
        psi_language_core::MemoryOrdering::Receive,
        psi_language_core::MemoryOrdering::Publish,
        psi_language_core::MemoryOrdering::ReceivePublish,
        psi_language_core::MemoryOrdering::GlobalOrder,
    ]
    .map(|ordering| u32::from_le_bytes(encode_cas(4, 26, 17, 16, ordering).unwrap()));
    assert_eq!(
        words,
        [
            0x88BA_7E11,
            0x88FA_7E11,
            0x88BA_FE11,
            0x88FA_FE11,
            0x88FA_FE11,
        ]
    );
}

/// Full `encode_atomic_compare_exchange`: emitted length equals the width fn
/// at every offset, and the final instruction is `CASAL w26, w17, [x16]`.
#[test]
fn atomic_compare_exchange_encoder_matches_width_and_ends_in_casal() {
    use omega_target_operations::RuntimeValueOperand;
    use psi_arena::Arena;

    for &target_offset in &[0usize, 4, 4095] {
        let mut operands: Arena<RuntimeValueOperand> = Arena::default();
        let expected = operands.insert(RuntimeValueOperand::Immediate(10));
        let new_value = operands.insert(RuntimeValueOperand::Immediate(99));
        let result_offset = 32;
        let bytes = encode_atomic_compare_exchange(
            &operands,
            target_offset,
            4,
            result_offset,
            expected,
            new_value,
            psi_language_core::MemoryOrdering::ReceivePublish,
        )
        .expect("encode");
        assert_eq!(
            bytes.len(),
            runtime_atomic_compare_exchange_width(
                &operands,
                target_offset,
                4,
                result_offset,
                expected,
                new_value
            ),
            "width mismatch at offset {target_offset}"
        );
        let atomic_end = runtime_atomic_compare_exchange_result_address_offset(
            &operands,
            target_offset,
            expected,
            new_value,
        );
        let last = u32::from_le_bytes(bytes[atomic_end - 4..atomic_end].try_into().unwrap());
        assert_eq!(
            last, 0x88FA_FE11,
            "final instruction must be CASAL w26, w17, [x16] at offset {target_offset}"
        );
    }

    let mut operands: Arena<RuntimeValueOperand> = Arena::default();
    let expected = operands.insert(RuntimeValueOperand::Immediate(1));
    let new_value = operands.insert(RuntimeValueOperand::Immediate(2));
    assert!(
        encode_atomic_compare_exchange(
            &operands,
            4096,
            4,
            0,
            expected,
            new_value,
            psi_language_core::MemoryOrdering::NoOrdering,
        )
        .is_err()
    );
}

/// Every supported index width keeps the fixed-width address recipe, so
/// width functions remain independent of the final load opcode.
#[test]
fn unsigned_index_loads_match_fixed_x_load_width() {
    let mut x_bytes = Vec::new();
    append_fixed_width_load_x_from_x_offset(&mut x_bytes, 17, 20, 0x40, 21);
    for byte_size in [1, 2, 4, 8] {
        let mut index_bytes = Vec::new();
        append_fixed_width_load_unsigned_index_from_x_offset(
            &mut index_bytes,
            17,
            20,
            0x40,
            byte_size,
            21,
        );
        assert_eq!(index_bytes.len(), x_bytes.len());
        assert_eq!(index_bytes.len(), 24);
    }
}

/// The final instruction must be `LDR Wt` (opcode family 0xB9400000), which
/// zero-extends the upper 32 bits, NOT `LDR Xt` (0xF9400000).
#[test]
fn index_w_load_emits_w_register_load() {
    let mut bytes = Vec::new();
    append_fixed_width_load_unsigned_index_from_x_offset(&mut bytes, 17, 20, 0x40, 4, 21);
    let last = u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap());
    // size field (bits 30-31) of LDR Wt is 0b10; LDR Xt is 0b11.
    assert_eq!(last & 0xFFC0_0000, 0xB940_0000, "expected LDR Wt (32-bit)");
}

#[test]
fn index_load_uses_the_exact_declared_width() {
    for index_byte_size in [1usize, 2, 4, 8] {
        let mut bytes = Vec::new();
        append_fixed_width_load_unsigned_index_from_x_offset(
            &mut bytes,
            17,
            20,
            0x40,
            index_byte_size,
            21,
        );
        let emitted = &bytes[bytes.len() - 4..];
        let expected = match index_byte_size {
            1 | 2 | 4 => encode_load_w_from_x(17, 21, 0, index_byte_size).unwrap(),
            8 => encode_load_x_from_x(17, 21, 0).unwrap(),
            _ => unreachable!(),
        };
        assert_eq!(emitted, expected, "index width {index_byte_size}");
    }
}

/// The frame-index target-address setup width must match what the encoder
/// emits for every exact-width index load.
#[test]
fn frame_index_setup_width_matches_emission() {
    for &(element_size, field_offset) in &[(1usize, 0usize), (4, 0), (8, 8), (24, 16), (40, 0)] {
        let mut bytes = Vec::new();
        append_runtime_frame_index_target_address_with_index_region(
            &mut bytes,
            16,
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            0x10,
            0x40,
            4,
            element_size,
            field_offset,
            17,
            26,
        )
        .unwrap();
        assert_eq!(
            bytes.len(),
            widths::runtime_frame_index_setup_width(element_size, field_offset),
            "element_size={element_size}, field_offset={field_offset}"
        );
    }
}

#[test]
fn frame_indexed_operand_keeps_pointee_and_machine_index_bases_distinct() {
    let mut bytes = Vec::new();
    append_runtime_frame_index_target_address_with_index_region(
        &mut bytes,
        15,
        omega_target_operations::RuntimeStorageRegion::Machine,
        0x10,
        0x40,
        4,
        4,
        2,
        17,
        26,
    )
    .unwrap();

    let machine_adrp = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
    assert_eq!(
        machine_adrp & 0x1f,
        19,
        "machine base must not overwrite x15"
    );
    assert_eq!(
        bytes.len(),
        widths::runtime_frame_index_setup_width(4, 2) + 8
    );
}

/// New frame-indexed -> pointee copy encoder length must equal its width.
#[test]
fn frame_indexed_to_pointee_copy_width_matches_emission() {
    let cases = [
        // (element_size, source_field, pointer_offset, target_field, byte_count)
        (24usize, 0usize, 0usize, 0usize, 8usize),
        (40, 8, 16, 8, 16),
        (16, 0, 24, 0, 4),
        (32, 16, 8, 16, 24),
    ];
    for &(element_size, source_field, pointer_offset, target_field, byte_count) in &cases {
        let bytes = encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
            0x10,
            0x40,
            4,
            element_size,
            source_field,
            pointer_offset,
            target_field,
            byte_count,
        )
        .unwrap();
        let expected =
            widths::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
                element_size,
                source_field,
                target_field,
                byte_count,
            );
        assert_eq!(
            bytes.len(),
            expected,
            "element_size={element_size}, source_field={source_field}, pointer_offset={pointer_offset}, target_field={target_field}, byte_count={byte_count}"
        );
    }
}

#[test]
fn frame_indexed_to_pointee_copy_materializes_a_machine_index_base() {
    let ordinary = encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
        24, 40, 8, 4, 0, 56, 0, 4,
    )
    .expect("encode all-frame indexed-to-pointee copy");
    let cross_region =
            encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region(
                24,
                omega_target_operations::RuntimeStorageRegion::Machine,
                40,
                8,
                4,
                0,
                56,
                0,
                4,
            )
            .expect("encode machine-indexed frame-descriptor-to-pointee copy");

    assert_eq!(cross_region.len(), ordinary.len() + 8);
    assert_eq!(
        &cross_region[32..40],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "the published second-base site must materialize MACHINE storage"
    );
    assert!(
            runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region_clobbers(
                omega_target_operations::RuntimeStorageRegion::Machine,
            )
            .contains(MachineRegister::Aarch64X(15)),
            "the exact footprint must retain the cross-region base register"
        );
}

#[test]
fn frame_base_indexed_storage_write_reuses_or_separates_machine_base() {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    for source_region in [frame, machine] {
        for index_region in [frame, machine] {
            let bytes =
                encode_runtime_storage_copy_to_runtime_frame_base_indexed_from_runtime_storage(
                    source_region,
                    88,
                    24,
                    index_region,
                    64,
                    8,
                    8,
                    0,
                    8,
                )
                .expect("encode frame-base-indexed storage write");
            assert_eq!(
                bytes.len(),
                widths::runtime_storage_copy_to_runtime_frame_base_indexed_from_runtime_storage_width(
                    source_region,
                    88,
                    24,
                    index_region,
                    64,
                    8,
                    8,
                    0,
                    8,
                )
            );
            if index_region == machine {
                let index_site = widths::runtime_frame_base_indexed_machine_index_base_offset(24);
                assert_eq!(
                    &bytes[index_site..index_site + 8],
                    [
                        encode_adrp_placeholder(15),
                        encode_add_page_offset_placeholder(15)
                    ]
                    .concat(),
                    "a machine index owns the reusable machine base pair"
                );
            } else if source_region == machine {
                let source_site =
                    widths::runtime_frame_base_indexed_operand_start_width_with_index_region(
                        24,
                        index_region,
                        64,
                        8,
                        8,
                        0,
                    );
                assert_eq!(
                    &bytes[source_site..source_site + 8],
                    [
                        encode_adrp_placeholder(15),
                        encode_add_page_offset_placeholder(15)
                    ]
                    .concat(),
                    "a machine-only source owns an independent base pair"
                );
            }
        }
    }
}

#[test]
fn frame_base_double_indexed_integer_write_uses_one_shared_frame_base() {
    let bytes =
        encode_runtime_frame_base_double_indexed_integer_write(24, 64, 8, 12, 72, 8, 4, 0, 4, 6)
            .expect("encode all-frame double-indexed integer write");

    assert_eq!(
        bytes.len(),
        widths::runtime_frame_base_double_indexed_integer_write_width(6)
    );
    assert_eq!(
        &bytes[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat(),
        "the collection and both index slots must share the opening frame base"
    );
    assert_eq!(
        runtime_frame_base_double_indexed_integer_write_clobbers(),
        RegisterSet::new([
            MachineRegister::Aarch64X(14),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(26),
        ])
    );
}

#[test]
fn frame_base_double_indexed_address_write_uses_one_shared_frame_base() {
    let bytes = encode_runtime_frame_base_double_indexed_address_to_runtime_frame_write(
        24, 64, 8, 12, 72, 8, 4, 0, 96,
    )
    .expect("encode all-frame double-indexed address write");

    assert_eq!(
        bytes.len(),
        widths::runtime_frame_base_double_indexed_address_to_runtime_frame_write_width(96)
    );
    assert_eq!(
        &bytes[..8],
        [
            encode_adrp_placeholder(20),
            encode_add_page_offset_placeholder(20)
        ]
        .concat(),
        "the collection, both index slots, and reference target share one frame base"
    );
    assert_eq!(
        runtime_frame_base_double_indexed_address_to_runtime_frame_write_clobbers(96),
        RegisterSet::new([
            MachineRegister::Aarch64X(14),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(26),
        ])
    );
}

#[test]
fn machine_double_indexed_address_write_places_the_frame_base_by_index_region() {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    for (outer_region, inner_region) in [(machine, machine), (frame, machine)] {
        let bytes = encode_runtime_machine_double_indexed_address_to_runtime_frame_write(
            24,
            outer_region,
            64,
            8,
            12,
            inner_region,
            72,
            8,
            4,
            0,
            96,
        )
        .expect("encode machine double-indexed address write");

        assert_eq!(
            bytes.len(),
            widths::runtime_machine_double_indexed_address_to_runtime_frame_write_width(96)
        );
        assert_eq!(
            &bytes[..8],
            [
                encode_adrp_placeholder(16),
                encode_add_page_offset_placeholder(16)
            ]
            .concat(),
            "the machine collection owns the opening relocation"
        );
        let frame_site = widths::runtime_machine_double_indexed_address_frame_base_offset(
            outer_region,
            inner_region,
        );
        assert_eq!(
            &bytes[frame_site..frame_site + 8],
            [
                encode_adrp_placeholder(15),
                encode_add_page_offset_placeholder(15)
            ]
            .concat(),
            "the destination reuses an index frame base or materializes it after address math"
        );
    }
    assert_eq!(
        runtime_machine_double_indexed_address_to_runtime_frame_write_clobbers(96),
        RegisterSet::new([
            MachineRegister::Aarch64X(14),
            MachineRegister::Aarch64X(15),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(26),
        ])
    );
}

#[test]
fn frame_base_double_indexed_string_write_reuses_one_frame_base() {
    let bytes =
        encode_runtime_frame_base_double_indexed_string_write(24, 64, 8, 48, 72, 8, 16, 8, 13)
            .expect("encode frame double-indexed string write");

    assert_eq!(
        bytes.len(),
        widths::runtime_frame_base_double_indexed_string_write_width(13)
    );
    assert_eq!(
        &bytes[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat()
    );
    let data_site = widths::runtime_frame_base_double_indexed_string_data_address_offset();
    assert_eq!(
        &bytes[data_site..data_site + 8],
        [
            encode_adrp_placeholder(17),
            encode_add_page_offset_placeholder(17)
        ]
        .concat()
    );
}

#[test]
fn frame_base_double_indexed_bounded_buffer_write_reuses_one_frame_base() {
    let bytes = encode_runtime_frame_base_double_indexed_bounded_buffer_write(
        24, 64, 8, 48, 72, 8, 16, 8, b"omega",
    )
    .expect("encode frame double-indexed bounded-buffer write");

    assert_eq!(
        bytes.len(),
        widths::runtime_frame_base_double_indexed_bounded_buffer_write_width(b"omega")
    );
    assert_eq!(
        &bytes[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat()
    );
}

#[test]
fn frame_base_double_indexed_convert_write_uses_one_shared_frame_base() {
    let (operands, source, _) = immediate_pair(70, 0);
    let bytes = encode_runtime_frame_base_double_indexed_convert_write(
        &operands, 24, 64, 8, 12, 72, 8, 4, 0, 4, source, 8, false, false, true, true, false, false,
    )
    .expect("encode all-frame double-indexed conversion write");

    assert_eq!(
        &bytes[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat(),
        "the collection and both index slots must share the opening frame base"
    );
    assert!(
        bytes.len() > widths::runtime_frame_base_double_indexed_convert_operand_offset(),
        "the conversion operand and store must follow the fixed address prefix"
    );
}

#[test]
fn frame_base_double_indexed_binary_write_uses_one_shared_frame_base() {
    let (operands, left, right) = immediate_pair(6, 1);
    let bytes = encode_runtime_frame_base_double_indexed_binary_write(
        &operands,
        24,
        64,
        8,
        12,
        72,
        8,
        4,
        0,
        4,
        left,
        StateGuardOperator::Add,
        right,
    )
    .expect("encode all-frame double-indexed binary write");

    assert_eq!(
        bytes.len(),
        widths::runtime_frame_base_double_indexed_binary_write_width(
            &operands,
            4,
            left,
            StateGuardOperator::Add,
            right,
        )
    );
    assert_eq!(
        &bytes[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat(),
        "the collection and both index slots must share the opening frame base"
    );
    assert_eq!(
        widths::runtime_frame_base_double_indexed_binary_left_operand_offset(),
        44
    );
}

#[test]
fn frame_base_double_indexed_storage_write_shares_frame_and_separates_machine_source() {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    for source_region in [frame, machine] {
        let bytes =
            encode_runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage(
                source_region,
                88,
                24,
                frame,
                64,
                8,
                12,
                frame,
                72,
                8,
                4,
                0,
                12,
            )
            .expect("encode all-frame double-indexed storage write");
        assert_eq!(
                bytes.len(),
                widths::runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage_width(
                    source_region,
                    frame,
                    frame,
                    88,
                    12,
                )
            );
        assert_eq!(
            &bytes[..8],
            [
                encode_adrp_placeholder(16),
                encode_add_page_offset_placeholder(16)
            ]
            .concat(),
            "the target and both indices share the opening frame base"
        );
        if source_region == machine {
            assert_eq!(
                &bytes[12..20],
                [
                    encode_adrp_placeholder(15),
                    encode_add_page_offset_placeholder(15)
                ]
                .concat(),
                "a machine source owns an independent base pair after the preserved frame base"
            );
        }
    }

    let read =
        encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
            24, frame, 64, 8, 12, frame, 72, 8, 4, 0, 104, 12,
        )
        .expect("encode all-frame double-indexed aggregate read");
    assert_eq!(
            read.len(),
            widths::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width(
                frame, frame, 104, 12,
            )
        );

    let mixed_write =
        encode_runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage(
            frame, 88, 24, machine, 64, 8, 12, frame, 72, 8, 4, 0, 12,
        )
        .expect("encode mixed-index frame double storage write");
    assert_eq!(
            mixed_write.len(),
            widths::runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage_width(
                frame, machine, frame, 88, 12,
            )
        );
    assert_eq!(
        &mixed_write[12..20],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat()
    );

    let mixed_read =
        encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
            24, frame, 64, 8, 12, machine, 72, 8, 4, 0, 104, 12,
        )
        .expect("encode mixed-index frame double aggregate read");
    assert_eq!(
            mixed_read.len(),
            widths::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width(
                frame, machine, 104, 12,
            )
        );
    assert_eq!(
        &mixed_read[8..16],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat()
    );
    assert_eq!(
        &mixed_read[52..60],
        [
            encode_adrp_placeholder(20),
            encode_add_page_offset_placeholder(20)
        ]
        .concat()
    );

    let pointee =
        encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee(
            24, frame, 64, 8, 12, frame, 72, 8, 4, 0, 104, 4, 12,
        )
        .expect("encode all-frame double-indexed aggregate copy to pointee");
    assert_eq!(
            pointee.len(),
            widths::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_width(
                frame, frame, 104, 4, 12,
            )
        );
    assert_eq!(
        &pointee[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat(),
        "the source, both indices, and pointer slot share the opening frame base"
    );

    let from_pointee =
        encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed(
            104, 4, 24, frame, 64, 8, 12, frame, 72, 8, 4, 0, 12,
        )
        .expect("encode aggregate pointee copy to all-frame double-indexed storage");
    assert_eq!(
            from_pointee.len(),
            widths::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed_width(
                frame, frame, 104, 4, 12,
            )
        );
    assert_eq!(
        &from_pointee[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat(),
        "the pointer slot, target, and both indices share the opening frame base"
    );

    let cross_double_to_pointee =
        encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee(
            24, machine, 64, 8, 12, frame, 72, 8, 4, 0, 104, 4, 12,
        )
        .expect("encode mixed-index frame double aggregate copy to pointee");
    assert_eq!(
            cross_double_to_pointee.len(),
            widths::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_width(
                machine, frame, 104, 4, 12,
            )
        );
    assert_eq!(
        &cross_double_to_pointee[12..20],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat()
    );
    let pointee_to_cross_double =
        encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed(
            104, 4, 24, frame, 64, 8, 12, machine, 72, 8, 4, 0, 12,
        )
        .expect("encode pointee copy to mixed-index frame double aggregate");
    assert_eq!(
            pointee_to_cross_double.len(),
            widths::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed_width(
                frame, machine, 104, 4, 12,
            )
        );

    let frame_indexed_to_pointee =
        encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee(
            24, frame, 72, 8, 12, 0, 104, 4, 12,
        )
        .expect("encode all-frame indexed aggregate copy to pointee");
    assert_eq!(
        frame_indexed_to_pointee.len(),
        widths::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_width(
            frame, 104, 4, 12,
        )
    );
    assert_eq!(
        &frame_indexed_to_pointee[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat()
    );

    let pointee_to_frame_indexed =
        encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed(
            104, 4, 24, frame, 72, 8, 12, 0, 12,
        )
        .expect("encode pointee aggregate copy to all-frame indexed storage");
    assert_eq!(
        pointee_to_frame_indexed.len(),
        widths::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed_width(
            frame, 104, 4, 12,
        )
    );
    assert_eq!(
        runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_clobbers(),
        runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed_clobbers(),
    );

    let cross_indexed_to_pointee =
        encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee(
            24, machine, 72, 8, 12, 0, 104, 4, 12,
        )
        .expect("encode machine-indexed frame aggregate copy to pointee");
    assert_eq!(
        cross_indexed_to_pointee.len(),
        widths::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_width(
            machine, 104, 4, 12,
        )
    );
    assert_eq!(
        &cross_indexed_to_pointee[12..20],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "a machine-held index owns the second base after the preserved frame root"
    );
    let pointee_to_cross_indexed =
        encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed(
            104, 4, 24, machine, 72, 8, 12, 0, 12,
        )
        .expect("encode pointee copy to machine-indexed frame aggregate");
    assert_eq!(
        pointee_to_cross_indexed.len(),
        widths::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed_width(
            machine, 104, 4, 12,
        )
    );

    let machine_to_pointee = encode_runtime_storage_copy_machine_double_indexed_to_runtime_pointee(
        24, machine, 64, 8, 12, frame, 72, 8, 4, 0, 104, 4, 12,
    )
    .expect("encode machine double-indexed aggregate copy to pointee");
    assert_eq!(
        machine_to_pointee.len(),
        widths::runtime_storage_copy_machine_double_indexed_to_runtime_pointee_width(104, 4, 12,)
    );
    assert_eq!(
        &machine_to_pointee[..16],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16),
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15),
        ]
        .concat(),
        "the machine collection and frame pointer/index slots keep separate roots"
    );

    let pointee_to_machine = encode_runtime_storage_copy_runtime_pointee_to_machine_double_indexed(
        104, 4, 24, frame, 64, 8, 12, machine, 72, 8, 4, 0, 12,
    )
    .expect("encode pointee aggregate copy to machine double-indexed storage");
    assert_eq!(
        pointee_to_machine.len(),
        widths::runtime_storage_copy_runtime_pointee_to_machine_double_indexed_width(104, 4, 12,)
    );
    assert_eq!(
        &pointee_to_machine[..16],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16),
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15),
        ]
        .concat(),
        "the machine target and frame pointer/index slots keep separate roots"
    );
    assert_eq!(
        runtime_storage_copy_machine_double_indexed_to_runtime_pointee_clobbers(),
        runtime_storage_copy_runtime_pointee_to_machine_double_indexed_clobbers(),
    );

    let machine_indexed_to_pointee =
        encode_runtime_storage_copy_machine_indexed_to_runtime_pointee(
            24, frame, 72, 8, 12, 0, 104, 4, 12,
        )
        .expect("encode machine indexed aggregate copy to pointee");
    assert_eq!(
        machine_indexed_to_pointee.len(),
        widths::runtime_storage_copy_machine_indexed_to_runtime_pointee_width(104, 4, 12)
    );
    assert_eq!(
        &machine_indexed_to_pointee[..16],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16),
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15),
        ]
        .concat()
    );

    let pointee_to_machine_indexed =
        encode_runtime_storage_copy_runtime_pointee_to_machine_indexed(
            104, 4, 24, machine, 72, 8, 12, 0, 12,
        )
        .expect("encode pointee aggregate copy to machine indexed storage");
    assert_eq!(
        pointee_to_machine_indexed.len(),
        widths::runtime_storage_copy_runtime_pointee_to_machine_indexed_width(104, 4, 12)
    );
    assert_eq!(
        runtime_storage_copy_machine_indexed_to_runtime_pointee_clobbers(),
        runtime_storage_copy_runtime_pointee_to_machine_indexed_clobbers(),
    );

    let indexed_pair = encode_runtime_storage_copy_frame_base_indexed_to_frame_base_indexed(
        24, frame, 64, 8, 12, 0, 128, frame, 72, 8, 12, 0, 12,
    )
    .expect("encode all-frame indexed aggregate pair copy");
    assert_eq!(
        indexed_pair.len(),
        widths::runtime_storage_copy_frame_base_indexed_to_frame_base_indexed_width(
            frame, frame, 12,
        )
    );
    assert_eq!(
        &indexed_pair[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat(),
        "both arrays and both index slots share the opening frame base"
    );

    let mixed_frame_indexed_pair =
        encode_runtime_storage_copy_frame_base_indexed_to_frame_base_indexed(
            24, machine, 64, 8, 12, 0, 128, frame, 72, 8, 12, 0, 12,
        )
        .expect("encode mixed-index frame aggregate pair copy");
    assert_eq!(
        mixed_frame_indexed_pair.len(),
        widths::runtime_storage_copy_frame_base_indexed_to_frame_base_indexed_width(
            machine, frame, 12,
        )
    );
    assert_eq!(
        &mixed_frame_indexed_pair[12..20],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat()
    );

    let cross_region_indexed_pair = encode_runtime_storage_copy_cross_region_indexed_pair(
        machine, 24, frame, 64, 8, 12, 0, frame, 128, machine, 72, 8, 12, 0, 12,
    )
    .expect("encode cross-region indexed aggregate pair copy");
    assert_eq!(
        cross_region_indexed_pair.len(),
        widths::runtime_storage_copy_cross_region_indexed_pair_width(12)
    );
    assert_eq!(
        &cross_region_indexed_pair[..16],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16),
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15),
        ]
        .concat()
    );

    let cross_region_double_indexed_pair =
        encode_runtime_storage_copy_cross_region_double_indexed_pair(
            machine, 24, frame, 64, 8, 24, machine, 72, 8, 12, 0, frame, 128, machine, 80, 8, 24,
            frame, 88, 8, 12, 0, 12,
        )
        .expect("encode cross-region double-indexed aggregate pair copy");
    assert_eq!(
        cross_region_double_indexed_pair.len(),
        widths::runtime_storage_copy_cross_region_double_indexed_pair_width(12)
    );
    assert_eq!(
        &cross_region_double_indexed_pair[..16],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16),
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15),
        ]
        .concat()
    );
    assert_eq!(
        runtime_storage_copy_cross_region_double_indexed_pair_clobbers(),
        RegisterSet::new([
            MachineRegister::Aarch64X(14),
            MachineRegister::Aarch64X(15),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(20),
            MachineRegister::Aarch64X(24),
            MachineRegister::Aarch64X(26),
        ])
    );

    let double_pair =
        encode_runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed(
            24, frame, 64, 8, 12, frame, 72, 8, 4, 0, 128, frame, 80, 8, 12, frame, 88, 8, 4, 0, 12,
        )
        .expect("encode all-frame double-indexed aggregate pair copy");
    assert_eq!(
        double_pair.len(),
        widths::runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed_width(
            frame, frame, frame, frame, 12,
        )
    );
    assert_eq!(
        &double_pair[..8],
        [
            encode_adrp_placeholder(16),
            encode_add_page_offset_placeholder(16)
        ]
        .concat(),
        "both 2D arrays and all four index slots share the opening frame base"
    );

    let mixed_frame_double_pair =
        encode_runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed(
            24, machine, 64, 8, 12, frame, 72, 8, 4, 0, 128, frame, 80, 8, 12, machine, 88, 8, 4,
            0, 12,
        )
        .expect("encode mixed-index frame double aggregate pair copy");
    assert_eq!(
        mixed_frame_double_pair.len(),
        widths::runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed_width(
            machine, frame, frame, machine, 12,
        )
    );
    assert_eq!(
        &mixed_frame_double_pair[12..20],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat()
    );

    let machine_double_pair =
        encode_runtime_storage_copy_machine_double_indexed_to_machine_double_indexed(
            24, machine, 64, 8, 12, frame, 72, 8, 4, 0, 128, frame, 80, 8, 12, machine, 88, 8, 4,
            0, 12,
        )
        .expect("encode mixed-index machine double pair copy");
    assert_eq!(
        machine_double_pair.len(),
        widths::runtime_storage_copy_machine_double_indexed_to_machine_double_indexed_width(
            machine, frame, frame, machine, 12,
        )
    );
    assert_eq!(
        runtime_storage_copy_machine_double_indexed_to_machine_double_indexed_clobbers(
            machine, frame, frame, machine,
        ),
        RegisterSet::new([
            MachineRegister::Aarch64X(14),
            MachineRegister::Aarch64X(15),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(19),
            MachineRegister::Aarch64X(24),
            MachineRegister::Aarch64X(26),
        ])
    );
}

/// The value compare materializes the expected value into a register and
/// compares register-to-register; its emitted length must equal the width
/// for every operand size, regardless of the expected value's sign or
/// magnitude.
#[test]
fn storage_value_compare_width_matches_emission() {
    for &byte_size in &[1usize, 2, 4, 8] {
        for &expected in &[0i64, 7, -3, -1000, 4_294_967_297, i64::MIN] {
            let bytes = encode_runtime_storage_value_compare_bytes(
                0x20,
                byte_size,
                expected,
                8,
                StateGuardOperator::Equal,
            )
            .unwrap();
            assert_eq!(
                bytes.len(),
                widths::runtime_storage_value_compare_width(0x20, byte_size),
                "byte_size={byte_size}, expected={expected}"
            );
        }
    }
}

/// Negative integer writes store the two's-complement bit pattern truncated
/// to the target width, and the emitted length must equal the width.
#[test]
fn negative_integer_write_width_matches_emission() {
    for &(byte_size, value) in &[
        (1usize, -42i64),
        (2, -1000),
        (4, -70000),
        (8, -42),
        (4, 0x1_0000), // > 16 bits: must not silently truncate
    ] {
        let bytes = encode_runtime_machine_integer_write(0x10, byte_size, value).unwrap();
        assert_eq!(
            bytes.len(),
            widths::runtime_machine_integer_write_width(0x10, byte_size),
            "byte_size={byte_size}, value={value}"
        );
    }
}

/// A 4-byte write of a value above 16 bits must materialize BOTH halfwords
/// (MOVZ + MOVK), not silently truncate to the low 16 bits.
#[test]
fn integer_write_materializes_full_32_bits() {
    let bytes = encode_runtime_machine_integer_write(0x10, 4, 0x0004_0003).unwrap();
    // The two instructions before the trailing store materialize w17.
    let word_at = |from_end: usize| {
        let start = bytes.len() - from_end;
        u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap())
    };
    // MOVZ w17, #3: 0x52800000 | (3 << 5) | 17.
    assert_eq!(word_at(12), 0x5280_0000 | (3 << 5) | 17, "MOVZ w17, #3");
    // MOVK w17, #4, LSL #16: 0x72800000 | (1 << 21) | (4 << 5) | 17.
    assert_eq!(
        word_at(8),
        0x7280_0000 | (1 << 21) | (4 << 5) | 17,
        "MOVK w17, #4, LSL #16"
    );
}

/// The frame-base-indexed setup loads the index as 4 bytes; the integer
/// write width must agree with the encoder.
#[test]
fn frame_base_indexed_integer_write_width_matches_emission() {
    for index_region in [
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        omega_target_operations::RuntimeStorageRegion::Machine,
    ] {
        for &(base, index_off, element_size, field, value_size) in &[
            (0x20usize, 0x48usize, 4usize, 0usize, 4usize),
            (0x20, 0x48, 8, 8, 8),
            (0x20, 0x40, 16, 0, 1),
        ] {
            let bytes = encode_runtime_frame_base_indexed_integer_write_with_index_region(
                base,
                index_region,
                index_off,
                4,
                element_size,
                field,
                value_size,
                7,
            )
            .unwrap();
            assert_eq!(
                bytes.len(),
                widths::runtime_frame_base_indexed_integer_write_with_index_region_width(
                    base,
                    index_region,
                    index_off,
                    4,
                    element_size,
                    field,
                    value_size,
                ),
                "index_region={index_region:?}, value_size={value_size}"
            );
            if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                let index_site = widths::runtime_frame_base_indexed_machine_index_base_offset(base);
                assert_eq!(
                    &bytes[index_site..index_site + 8],
                    [
                        encode_adrp_placeholder(15),
                        encode_add_page_offset_placeholder(15)
                    ]
                    .concat(),
                    "the machine-held index owns an x15 base pair"
                );
            }
        }
    }
}

/// The float storage compare adds two FMOVs + an FCMP; its emitted length
/// must equal the (float-aware) width for both single and double precision.
#[test]
fn float_storage_compare_width_matches_emission() {
    for &byte_size in &[4usize, 8] {
        let bytes = encode_runtime_storage_compare_bytes(
            0x10,
            0x20,
            byte_size,
            8,
            StateGuardOperator::Less,
            true,
        )
        .unwrap();
        assert_eq!(
            bytes.len(),
            widths::runtime_storage_compare_width(0x10, 0x20, byte_size, true),
            "byte_size={byte_size}"
        );
        // The float path must be exactly 8 bytes (two FMOVs) longer than the
        // integer path at the same offsets/width.
        assert_eq!(
            widths::runtime_storage_compare_width(0x10, 0x20, byte_size, true),
            widths::runtime_storage_compare_width(0x10, 0x20, byte_size, false) + 8,
        );
    }
}

/// The float storage compare must emit an FCMP (single `0x1e22_2020` family /
/// double `0x1e62_2020` family) — i.e. ftype follows the operand width — and
/// not an integer `CMP`.
#[test]
fn float_storage_compare_emits_fcmp_of_correct_precision() {
    let single =
        encode_runtime_storage_compare_bytes(0x10, 0x20, 4, 8, StateGuardOperator::Less, true)
            .unwrap();
    let double =
        encode_runtime_storage_compare_bytes(0x10, 0x20, 8, 8, StateGuardOperator::Less, true)
            .unwrap();
    // The FCMP is the instruction immediately before the trailing branch.
    let fcmp_word = |bytes: &[u8]| {
        let start = bytes.len() - 8;
        u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap())
    };
    // FCMP base (Rm/Rn cleared): single 0x1e202000, double 0x1e602000.
    assert_eq!(fcmp_word(&single) & 0xFFE0_FC1F, 0x1e20_2000, "single FCMP");
    assert_eq!(fcmp_word(&double) & 0xFFE0_FC1F, 0x1e60_2000, "double FCMP");
}

/// Build a value-operand arena holding a single storage source operand and
/// return both the arena and the source handle. The convert encoder loads the
/// source via the generic value-operand path; a storage source gives a
/// deterministic, offset-free load width.
fn storage_source(
    byte_size: usize,
) -> (
    psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    omega_target_operations::RuntimeValueOperandHandle,
) {
    let mut arena = psi_arena::Arena::new();
    let handle = arena.insert(omega_target_operations::RuntimeValueOperand::Storage {
        region: omega_target_operations::RuntimeStorageRegion::Machine,
        byte_offset: 0x20,
        byte_size,
    });
    (arena, handle)
}

/// The converting-store encoder length must equal its width function for every
/// (target_offset, source/target width, float/int) combination — layout and
/// relocation placement both rely on the width being exact.
#[test]
fn convert_encoder_length_matches_width() {
    // (target_offset, src_size, tgt_size, src_float, tgt_float, src_signed)
    let cases = [
        // int -> float
        (0x10usize, 4usize, 8usize, false, true, true),
        (0x20, 8, 8, false, true, true),
        (0x18, 4, 4, false, true, true),
        // float -> int
        (0x10, 8, 4, true, false, true),
        (0x20, 8, 8, true, false, true),
        (0x28, 4, 4, true, false, true),
        (0x30, 4, 1, true, false, true),
        // float -> float
        (0x10, 8, 4, true, true, true), // f64 -> f32 (FCVT narrow)
        (0x20, 4, 8, true, true, true), // f32 -> f64 (FCVT widen)
        (0x18, 8, 8, true, true, true), // f64 -> f64 (no-op convert)
        // int -> int
        (0x10, 4, 8, false, false, true),  // signed widen (SXTW)
        (0x20, 4, 8, false, false, false), // unsigned widen (no SXTW)
        (0x28, 8, 4, false, false, true),  // narrow (store truncates)
        (0x30, 4, 4, false, false, true),  // same width
        // a larger, non-trivially-encodable target offset
        (0x4000, 8, 8, false, true, true),
    ];
    for &(target_offset, src_size, tgt_size, src_float, tgt_float, src_signed) in &cases {
        let (arena, source) = storage_source(src_size);
        let bytes = encode_runtime_storage_convert(
            &arena,
            target_offset,
            tgt_size,
            source,
            src_size,
            src_float,
            tgt_float,
            src_signed,
            true,
            false,
            false,
        )
        .unwrap();
        let width = widths::runtime_storage_convert_width(
            &arena,
            target_offset,
            source,
            src_size,
            tgt_size,
            src_float,
            tgt_float,
            src_signed,
            true,
            false,
            false,
        );
        assert_eq!(
            bytes.len(),
            width,
            "len != width for target_offset={target_offset:#x}, src_size={src_size}, tgt_size={tgt_size}, src_float={src_float}, tgt_float={tgt_float}, src_signed={src_signed}"
        );
    }
}

#[test]
fn float_to_int_policy_shapes_match_width_for_signedness_and_narrowing() {
    for target_byte_size in [1usize, 2, 4, 8] {
        for target_signed in [false, true] {
            for (trapping, saturating) in [(true, false), (false, true)] {
                let (arena, source) = storage_source(8);
                let bytes = encode_runtime_storage_convert(
                    &arena,
                    0x10,
                    target_byte_size,
                    source,
                    8,
                    true,
                    false,
                    false,
                    target_signed,
                    trapping,
                    saturating,
                )
                .expect("policy conversion encodes");
                let width = widths::runtime_storage_convert_width(
                    &arena,
                    0x10,
                    source,
                    8,
                    target_byte_size,
                    true,
                    false,
                    false,
                    target_signed,
                    trapping,
                    saturating,
                );
                assert_eq!(
                    bytes.len(),
                    width,
                    "target={target_byte_size} signed={target_signed} trapping={trapping}"
                );
            }
        }
    }
}

/// int -> float must emit SCVTF then FMOV(result -> GPR); float -> int must
/// emit FMOV(bits -> FP) then FCVTZS. Check the opcode families of the two
/// trailing convert instructions (they sit right before the result store).
#[test]
fn convert_emits_expected_conversion_opcodes() {
    // int(w) -> double: SCVTF d0,w17 (0x1e62_0000 family) + FMOV x17,d0
    // (0x9e66_0000 family).
    let (arena, source) = storage_source(4);
    let bytes = encode_runtime_storage_convert(
        &arena, 0x10, 8, source, 4, false, true, true, true, false, false,
    )
    .unwrap();
    // The store is a single 4-byte STR at offset 0x10 (encodable), so the two
    // convert words are at len-12..len-4.
    let word_at = |b: &[u8], from_end: usize| {
        let start = b.len() - from_end;
        u32::from_le_bytes(b[start..start + 4].try_into().unwrap())
    };
    let scvtf = word_at(&bytes, 12);
    let fmov_back = word_at(&bytes, 8);
    // SCVTF d0, w17: base 0x1e620000, Rn=17 -> (17<<5).
    assert_eq!(scvtf, 0x1e62_0000 | (17 << 5), "SCVTF d0, w17");
    // FMOV x17, d0: base 0x9e660000, Rd=17.
    assert_eq!(fmov_back, 0x9e66_0000 | 17, "FMOV x17, d0");

    // unsigned int(x) -> double selects UCVTF, preserving the upper half
    // of u64 instead of treating it as a negative signed integer.
    let (arena, source) = storage_source(8);
    let bytes = encode_runtime_storage_convert(
        &arena, 0x10, 8, source, 8, false, true, false, true, false, false,
    )
    .unwrap();
    let ucvtf = word_at(&bytes, 12);
    assert_eq!(ucvtf, 0x9e63_0000 | (17 << 5), "UCVTF d0, x17");

    // double -> int(w): FMOV d0,x17 (0x9e67_0000) + FCVTZS w17,d0
    // (0x1e38_0000 family).
    let (arena, source) = storage_source(8);
    let bytes = encode_runtime_storage_convert(
        &arena, 0x10, 4, source, 8, true, false, true, true, false, false,
    )
    .unwrap();
    let fmov_in = word_at(&bytes, 12);
    let fcvtzs = word_at(&bytes, 8);
    // FMOV d0, x17: base 0x9e670000, Rn=17 -> (17<<5).
    assert_eq!(fmov_in, 0x9e67_0000 | (17 << 5), "FMOV d0, x17");
    // FCVTZS w17, d0: base 0x1e780000 (double src, 32-bit dst), Rd=17.
    assert_eq!(fcvtzs, 0x1e78_0000 | 17, "FCVTZS w17, d0");
}

/// A signed 32->64 int widening must emit SXTW x17,w17; an unsigned widening
/// (or any non-widening) must emit nothing for the convert step.
#[test]
fn convert_int_widening_uses_sxtw_only_when_signed() {
    let (arena, source) = storage_source(4);
    // signed widen: convert step = SXTW (one 4-byte word). Width must include it.
    let signed_width = widths::runtime_storage_convert_width(
        &arena, 0x10, source, 4, 8, false, false, true, true, false, false,
    );
    let unsigned_width = widths::runtime_storage_convert_width(
        &arena, 0x10, source, 4, 8, false, false, false, false, false, false,
    );
    assert_eq!(
        signed_width,
        unsigned_width + 4,
        "signed widen must be exactly one SXTW longer than unsigned"
    );
    let signed_bytes = encode_runtime_storage_convert(
        &arena, 0x10, 8, source, 4, false, false, true, true, false, false,
    )
    .unwrap();
    // SXTW x17, w17: 0x93407c00 | (17<<5) | 17 — it sits right before the store.
    let store_width = if signed_bytes.len() >= 8 { 4 } else { 0 };
    let _ = store_width;
    let sxtw_start = signed_bytes.len() - 8; // SXTW (4) + STR (4)
    let sxtw = u32::from_le_bytes(signed_bytes[sxtw_start..sxtw_start + 4].try_into().unwrap());
    assert_eq!(sxtw, 0x9340_7c00 | (17 << 5) | 17, "SXTW x17, w17");
}

/// Build a value-operand arena with two immediate operands (a deterministic,
/// relocation-free load width) and return the arena and both handles.
fn immediate_pair(
    left: i64,
    right: i64,
) -> (
    psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    omega_target_operations::RuntimeValueOperandHandle,
    omega_target_operations::RuntimeValueOperandHandle,
) {
    let mut arena = psi_arena::Arena::new();
    let left = arena.insert(omega_target_operations::RuntimeValueOperand::Immediate(
        left,
    ));
    let right = arena.insert(omega_target_operations::RuntimeValueOperand::Immediate(
        right,
    ));
    (arena, left, right)
}

#[test]
fn frame_indexed_binary_write_materializes_a_machine_index_base() {
    let (arena, left, right) = immediate_pair(40, 2);
    let ordinary = encode_runtime_frame_indexed_binary_write(
        &arena,
        24,
        40,
        8,
        4,
        0,
        4,
        left,
        StateGuardOperator::Add,
        right,
    )
    .expect("encode frame-local index binary write");
    let cross_region = encode_runtime_frame_indexed_binary_write_with_index_region(
        &arena,
        24,
        omega_target_operations::RuntimeStorageRegion::Machine,
        40,
        8,
        4,
        0,
        4,
        left,
        StateGuardOperator::Add,
        right,
    )
    .expect("encode machine-indexed frame-descriptor binary write");

    assert_eq!(cross_region.len(), ordinary.len() + 8);
    assert_eq!(
        &cross_region[32..40],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "the extra pair at the published relocation offset must materialize MACHINE storage"
    );
}

#[test]
fn frame_base_indexed_binary_write_materializes_a_machine_index_base() {
    let (arena, left, right) = immediate_pair(40, 2);
    let bytes = encode_runtime_frame_base_indexed_binary_write_with_index_region(
        &arena,
        24,
        omega_target_operations::RuntimeStorageRegion::Machine,
        64,
        8,
        8,
        0,
        8,
        left,
        StateGuardOperator::Add,
        right,
    )
    .expect("encode cross-region inline-frame binary write");
    assert_eq!(
        bytes.len(),
        widths::runtime_frame_base_indexed_binary_write_with_index_region_width(
            &arena,
            24,
            omega_target_operations::RuntimeStorageRegion::Machine,
            64,
            8,
            8,
            0,
            8,
            left,
            StateGuardOperator::Add,
            right,
        )
    );
    let index_site = widths::runtime_frame_base_indexed_machine_index_base_offset(24);
    assert_eq!(
        &bytes[index_site..index_site + 8],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "the machine-held index must own an x15 base pair"
    );
}

#[test]
fn frame_base_indexed_convert_write_materializes_a_machine_index_base() {
    let (arena, source) = storage_source(8);
    let ordinary = encode_runtime_frame_base_indexed_convert_write(
        &arena, 24, 64, 8, 4, 0, 4, source, 8, false, false, true, true, false, false,
    )
    .expect("encode frame-local inline-frame conversion write");
    let cross_region = encode_runtime_frame_base_indexed_convert_write_with_index_region(
        &arena,
        24,
        omega_target_operations::RuntimeStorageRegion::Machine,
        64,
        8,
        4,
        0,
        4,
        source,
        8,
        false,
        false,
        true,
        true,
        false,
        false,
    )
    .expect("encode cross-region inline-frame conversion write");

    assert_eq!(cross_region.len(), ordinary.len() + 8);
    let index_site = widths::runtime_frame_base_indexed_machine_index_base_offset(24);
    assert_eq!(
        &cross_region[index_site..index_site + 8],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "the machine-held conversion index must own an x15 base pair"
    );
}

#[test]
fn frame_indexed_copy_materializes_a_machine_index_base() {
    let ordinary = encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage(
        24, 40, 8, 4, 0, 56, 4,
    )
    .expect("encode frame-local index copy");
    let cross_region =
            encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_with_index_region(
                24,
                omega_target_operations::RuntimeStorageRegion::Machine,
                40,
                8,
                4,
                0,
                56,
                4,
            )
            .expect("encode machine-indexed frame-descriptor copy");

    assert_eq!(cross_region.len(), ordinary.len() + 8);
    assert_eq!(
        &cross_region[32..40],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "the published second-base site must materialize MACHINE storage"
    );
    assert!(
        runtime_storage_copy_from_runtime_frame_indexed_with_index_region_clobbers(
            omega_target_operations::RuntimeStorageRegion::Machine,
        )
        .contains(MachineRegister::Aarch64X(15)),
        "the exact footprint must retain the cross-region base register"
    );
}

#[test]
fn frame_indexed_copy_write_reuses_or_materializes_the_machine_source_base() {
    let ordinary = encode_runtime_storage_copy_to_runtime_frame_indexed(56, 24, 40, 8, 4, 0, 4)
        .expect("encode all-frame indexed copy write");
    let shared_machine = encode_runtime_storage_copy_to_runtime_frame_indexed_with_regions(
        omega_target_operations::RuntimeStorageRegion::Machine,
        56,
        24,
        omega_target_operations::RuntimeStorageRegion::Machine,
        40,
        8,
        4,
        0,
        4,
    )
    .expect("encode indexed copy write sharing one machine base");
    let distinct_machine = encode_runtime_storage_copy_to_runtime_frame_indexed_with_regions(
        omega_target_operations::RuntimeStorageRegion::Machine,
        56,
        24,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        40,
        8,
        4,
        0,
        4,
    )
    .expect("encode indexed copy write with a distinct machine source base");

    assert_eq!(shared_machine.len(), ordinary.len() + 8);
    assert_eq!(distinct_machine.len(), ordinary.len() + 8);
    assert_eq!(
        &shared_machine[32..40],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "a machine index must materialize the reusable machine base at the shared site"
    );
    let distinct_site = widths::runtime_frame_index_setup_width(4, 0);
    assert_eq!(
        &distinct_machine[distinct_site..distinct_site + 8],
        [
            encode_adrp_placeholder(15),
            encode_add_page_offset_placeholder(15)
        ]
        .concat(),
        "a machine source with a frame index needs its own post-address-setup base"
    );
    assert!(
        runtime_storage_copy_to_runtime_frame_indexed_with_regions_clobbers(
            omega_target_operations::RuntimeStorageRegion::Machine,
            omega_target_operations::RuntimeStorageRegion::Machine,
        )
        .contains(MachineRegister::Aarch64X(15))
    );
}

/// The saturating/trapping add/sub/mul encoder length must equal its width
/// function for every (domain, operator, byte_size, signed) combination — the
/// internal `debug_assert_eq!` also fires here. Covers all 1/2/4-byte widths.
#[test]
fn saturating_trapping_binary_write_width_matches_emission() {
    use psi_numerics::arithmetic::ArithmeticDomain;
    let (arena, left, right) = immediate_pair(100, 100);
    for &domain in &[ArithmeticDomain::Saturating, ArithmeticDomain::Trapping] {
        for &operator in &[
            StateGuardOperator::Add,
            StateGuardOperator::Subtract,
            StateGuardOperator::Multiply,
        ] {
            for &byte_size in &[1usize, 2, 4] {
                for &signed in &[false, true] {
                    let bytes = encode_runtime_storage_binary_write(
                        &arena, 0x10, byte_size, left, operator, right, false, domain, signed,
                    )
                    .unwrap();
                    let width = widths::runtime_storage_binary_write_width(
                        &arena, 0x10, byte_size, left, operator, right, false, domain, signed,
                    );
                    assert_eq!(
                        bytes.len(),
                        width,
                        "len != width for domain={domain:?}, operator={operator:?}, byte_size={byte_size}, signed={signed}"
                    );
                }
            }
        }
    }
}

/// The signed saturating add at 1 byte must sign-extend BOTH operands (SXTB
/// Xd,Wn = 0x9340_1C00 family) before the wide ADD, materialize the bounds
/// with MOVZ/MOVK, and clamp with CMP + b.cond + MOV (no BRK).
#[test]
fn signed_saturating_add_byte_sign_extends_and_clamps() {
    use psi_numerics::arithmetic::ArithmeticDomain;
    let (arena, left, right) = immediate_pair(100, 100);
    let bytes = encode_runtime_storage_binary_write(
        &arena,
        0x10,
        1,
        left,
        StateGuardOperator::Add,
        right,
        false,
        ArithmeticDomain::Saturating,
        true,
    )
    .unwrap();
    // IMMEDIATE operands are loaded at their true wide value, so the
    // signed extension is SKIPPED for them (extending from the target
    // width corrupts a wide literal -- the MIN-idiom fix): expect ZERO
    // SXTB here. Storage operands keep their per-side extension (the
    // width twin mirrors the same per-side skip).
    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    // SXTB Xd, Wn family is 0x9340_1C00.
    let sxtb_count = words
        .iter()
        .filter(|w| (*w & 0xFFFF_FC00) == 0x9340_1C00)
        .count();
    assert_eq!(sxtb_count, 0, "immediate operands must not re-extend");
    // Exactly one wide ADD Xd,Xn,Xm (0x8B00_0000 family) — the saturating op.
    let add_count = words
        .iter()
        .filter(|w| (*w & 0xFF20_0000) == 0x8B00_0000)
        .count();
    assert_eq!(add_count, 1, "expected one wide ADD");
    // Saturating must NOT emit a BRK (0xD420_0000 family).
    assert!(
        !words.iter().any(|w| (*w & 0xFFE0_001F) == 0xD420_0000),
        "saturating must not trap"
    );
}

/// Trapping add must emit BRK instructions (0xD420_0000) on the overflow
/// paths, and unsigned must check both the 0 lower bound and the MAX upper
/// bound.
#[test]
fn unsigned_trapping_narrow_brk_per_overflow_direction() {
    // Unsigned wide results overflow in ONE direction per operator, so
    // each narrow unsigned trapping arm emits exactly ONE brk (the old
    // both-checks tail emitted two -- and its SIGNED lower compare
    // misread 2^63+ products); signed arms keep both bound checks.
    use psi_numerics::arithmetic::ArithmeticDomain;
    let brk_count = |bytes: &[u8]| {
        bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .filter(|w| (*w & 0xFFE0_001F) == 0xD420_0000)
            .count()
    };
    for (operator, signed, expected) in [
        (StateGuardOperator::Add, false, 1),
        (StateGuardOperator::Subtract, false, 1),
        (StateGuardOperator::Multiply, false, 1),
        (StateGuardOperator::Add, true, 2),
        (StateGuardOperator::Multiply, true, 2),
    ] {
        let (arena, left, right) = immediate_pair(200, 200);
        let bytes = encode_runtime_storage_binary_write(
            &arena,
            0x10,
            1,
            left,
            operator,
            right,
            false,
            ArithmeticDomain::Trapping,
            signed,
        )
        .unwrap();
        assert_eq!(
            brk_count(&bytes),
            expected,
            "brk count for {operator:?} (signed: {signed})"
        );
    }
}

/// 64-bit saturating/trapping arithmetic (the flag/MULH-based clamps):
/// every (domain x signedness x operator) arm's emitted length must match
/// the width helper, or relocation offsets drift.
#[test]
fn saturating_eight_byte_arithmetic_width_matches_emission() {
    use psi_numerics::arithmetic::ArithmeticDomain;
    for domain in [ArithmeticDomain::Saturating, ArithmeticDomain::Trapping] {
        for signed in [true, false] {
            for operator in [
                StateGuardOperator::Add,
                StateGuardOperator::Subtract,
                StateGuardOperator::Multiply,
                StateGuardOperator::ShiftLeft,
            ] {
                let (arena, left, right) = immediate_pair(5, 5);
                let bytes = encode_runtime_storage_binary_write(
                    &arena, 0x10, 8, left, operator, right, false, domain, signed,
                )
                .unwrap_or_else(|error| {
                    panic!("8-byte {domain:?} {operator:?} signed={signed} should encode: {error}")
                });
                assert_eq!(
                    bytes.len(),
                    widths::runtime_storage_binary_write_width(
                        &arena, 0x10, 8, left, operator, right, false, domain, signed,
                    ),
                    "width drift: {domain:?} {operator:?} signed={signed}"
                );
            }
        }
    }
}

/// Signed Saturating i64 division/remainder must retain both the exact
/// `MIN / -1` fixup and the ordinary SDIV/MSUB path. The encoder and its width
/// twin must agree for both operations, while unsigned Saturating division
/// remains on the ordinary non-overflowing path.
#[test]
fn saturating_eight_byte_divide_remainder_encode_exact_fixups() {
    use psi_numerics::arithmetic::ArithmeticDomain;

    let words = |bytes: &[u8]| {
        bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
            .collect::<Vec<_>>()
    };
    let (mut arena, left, right) = immediate_pair(i64::MIN, -1);
    for operator in [StateGuardOperator::Divide, StateGuardOperator::Modulo] {
        let bytes = encode_runtime_storage_binary_write(
            &arena,
            0x10,
            8,
            left,
            operator,
            right,
            false,
            ArithmeticDomain::Saturating,
            true,
        )
        .expect("signed Saturating i64 divide/remainder should encode");
        assert_eq!(
            bytes.len(),
            widths::runtime_storage_binary_write_width(
                &arena,
                0x10,
                8,
                left,
                operator,
                right,
                false,
                ArithmeticDomain::Saturating,
                true,
            ),
            "{operator:?} width drift"
        );
        let mut helper = Vec::new();
        append_saturating_signed_divide_modulo(
            &mut helper,
            8,
            operator == StateGuardOperator::Modulo,
            17,
            26,
            9,
        )
        .expect("encode the isolated i64 fixup and ordinary path");
        assert_eq!(
            helper.len(),
            if operator == StateGuardOperator::Divide {
                52
            } else {
                40
            }
        );
        let words = words(&helper);
        let sdiv_destination = if operator == StateGuardOperator::Divide {
            17
        } else {
            9
        };
        let sdiv = u32::from_le_bytes(encode_sdiv_x_register(sdiv_destination, 17, 26));
        assert!(
            words.contains(&sdiv),
            "{operator:?} lost its ordinary SDIV path"
        );
        if operator == StateGuardOperator::Divide {
            let negate = u32::from_le_bytes(encode_sub_x_register(17, 31, 17));
            let select_max = u32::from_le_bytes(encode_csinv_x(17, 17, 9, 0b0001));
            assert!(
                words.contains(&negate),
                "i64 divide has no guarded negation"
            );
            assert!(
                words.contains(&select_max),
                "i64 divide has no wrapped-MIN to MAX selection"
            );
        } else {
            let zero = u32::from_le_bytes(encode_movz(17, 0));
            let msub = u32::from_le_bytes(encode_msub_x_register(17, 9, 26, 17));
            assert!(words.contains(&zero), "i64 remainder has no -1 zero result");
            assert!(
                words.contains(&msub),
                "i64 remainder lost its ordinary MSUB path"
            );
        }
    }

    let unsigned = encode_runtime_storage_binary_write(
        &arena,
        0x10,
        8,
        left,
        StateGuardOperator::DivideUnsigned,
        right,
        false,
        ArithmeticDomain::Saturating,
        false,
    )
    .expect("unsigned Saturating division is an ordinary non-overflowing divide");
    let unsigned_words = words(&unsigned);
    assert!(
        unsigned_words.contains(&u32::from_le_bytes(encode_udiv_x_register(17, 17, 26))),
        "unsigned Saturating division must retain UDIV"
    );
    assert!(
        !unsigned_words.contains(&u32::from_le_bytes(encode_sub_x_register(17, 31, 17))),
        "unsigned Saturating division must not enter the signed MIN/-1 fixup"
    );

    for operator in [StateGuardOperator::Divide, StateGuardOperator::Modulo] {
        let operand = arena.insert(omega_target_operations::RuntimeValueOperand::Binary {
            left,
            operator,
            right,
            is_float: false,
            byte_width: 8,
            arithmetic_domain: ArithmeticDomain::Saturating,
            operands_signed: true,
        });
        let mut operand_bytes = Vec::new();
        append_runtime_value_operand(&arena, &mut operand_bytes, 17, &[26, 9], operand)
            .expect("encode the nested signed Saturating i64 operand");
        assert_eq!(
            operand_bytes.len(),
            widths::runtime_value_operand_width(&arena, operand),
            "nested {operator:?} operand width drift"
        );
    }
}

#[test]
fn trapping_float_policy_is_one_result_only_guard() {
    use psi_numerics::arithmetic::ArithmeticDomain;

    for byte_size in [4usize, 8] {
        for operator in [
            StateGuardOperator::Add,
            StateGuardOperator::Min,
            StateGuardOperator::Max,
            StateGuardOperator::Sqrt,
        ] {
            let bytes = float_policy_guard_bytes(
                ArithmeticDomain::Trapping,
                operator,
                byte_size,
                17,
                26,
                None,
                15,
                14,
            )
            .expect("encode result-only policy guard");
            let brk_count = bytes
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                .filter(|word| (*word & 0xFFE0_001F) == 0xD420_0000)
                .count();
            assert_eq!(brk_count, 1, "f{} {operator:?}", byte_size * 8);
            assert_eq!(
                bytes.len(),
                float_policy_guard_width(operator, byte_size, ArithmeticDomain::Trapping)
            );
        }
    }
}

#[test]
fn multiply_then_add_emission_keeps_two_operations_and_width_lockstep() {
    use psi_numerics::arithmetic::ArithmeticDomain;

    for byte_size in [4usize, 8] {
        for domain in [
            ArithmeticDomain::Exact,
            ArithmeticDomain::Saturating,
            ArithmeticDomain::Trapping,
        ] {
            let mut bytes = Vec::new();
            append_runtime_float_binary_operation(
                &mut bytes,
                byte_size,
                17,
                StateGuardOperator::MultiplyThenAdd,
                26,
                domain,
                [15, 14],
            )
            .expect("encode multiply-then-add");
            assert_eq!(
                bytes.len(),
                24 + float_policy_guard_width(
                    StateGuardOperator::MultiplyThenAdd,
                    byte_size,
                    domain,
                ),
                "f{} {domain:?} width",
                byte_size * 8,
            );
            let instructions = bytes
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                .collect::<Vec<_>>();
            assert!(
                instructions.contains(&u32::from_le_bytes(
                    encode_float_multiply(byte_size, 0, 0, 1).expect("encode scalar multiply"),
                )),
                "f{} must contain a scalar multiply",
                byte_size * 8,
            );
            assert!(
                instructions.contains(&u32::from_le_bytes(
                    encode_float_add(byte_size, 0, 0, 1).expect("encode scalar add"),
                )),
                "f{} must contain a separate scalar add",
                byte_size * 8,
            );
        }
    }
}

#[test]
fn directed_operations_balance_fpcr_and_widths() {
    use psi_numerics::arithmetic::ArithmeticDomain;

    for (operator, fpcr, operation) in [
        (
            StateGuardOperator::AddTowardPositive,
            0x0040_0000_u64,
            "add",
        ),
        (
            StateGuardOperator::AddTowardNegative,
            0x0080_0000_u64,
            "add",
        ),
        (StateGuardOperator::AddTowardZero, 0x00c0_0000_u64, "add"),
        (
            StateGuardOperator::SubtractTowardPositive,
            0x0040_0000_u64,
            "subtract",
        ),
        (
            StateGuardOperator::SubtractTowardNegative,
            0x0080_0000_u64,
            "subtract",
        ),
        (
            StateGuardOperator::SubtractTowardZero,
            0x00c0_0000_u64,
            "subtract",
        ),
        (
            StateGuardOperator::MultiplyTowardPositive,
            0x0040_0000_u64,
            "multiply",
        ),
        (
            StateGuardOperator::MultiplyTowardNegative,
            0x0080_0000_u64,
            "multiply",
        ),
        (
            StateGuardOperator::MultiplyTowardZero,
            0x00c0_0000_u64,
            "multiply",
        ),
        (
            StateGuardOperator::DivideTowardPositive,
            0x0040_0000_u64,
            "divide",
        ),
        (
            StateGuardOperator::DivideTowardNegative,
            0x0080_0000_u64,
            "divide",
        ),
        (
            StateGuardOperator::DivideTowardZero,
            0x00c0_0000_u64,
            "divide",
        ),
        (
            StateGuardOperator::SqrtTowardPositive,
            0x0040_0000_u64,
            "sqrt",
        ),
        (
            StateGuardOperator::SqrtTowardNegative,
            0x0080_0000_u64,
            "sqrt",
        ),
        (StateGuardOperator::SqrtTowardZero, 0x00c0_0000_u64, "sqrt"),
    ] {
        for byte_size in [4usize, 8] {
            let mut bytes = Vec::new();
            append_runtime_float_binary_operation(
                &mut bytes,
                byte_size,
                17,
                operator,
                26,
                ArithmeticDomain::Exact,
                [15, 14],
            )
            .expect("encode directed operation");
            assert_eq!(
                bytes.len(),
                widths::runtime_float_binary_operation_width_with_domain(
                    operator,
                    byte_size,
                    ArithmeticDomain::Exact,
                ),
                "f{} {operator:?}",
                byte_size * 8,
            );
            let words = bytes
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(words[2], u32::from_le_bytes(encode_read_fpcr(13)));
            assert_eq!(words[3], u32::from_le_bytes(encode_movz(12, 0)));
            assert_eq!(
                words[4],
                u32::from_le_bytes(encode_movk(12, ((fpcr >> 16) & 0xffff) as u16, 1,))
            );
            assert_eq!(words[5], u32::from_le_bytes(encode_write_fpcr(12)));
            assert_eq!(
                words[6],
                u32::from_le_bytes(match operation {
                    "add" => encode_float_add(byte_size, 0, 0, 1).unwrap(),
                    "subtract" => encode_float_subtract(byte_size, 0, 0, 1).unwrap(),
                    "multiply" => encode_float_multiply(byte_size, 0, 0, 1).unwrap(),
                    "divide" => encode_float_divide(byte_size, 0, 0, 1).unwrap(),
                    "sqrt" => encode_float_sqrt(byte_size, 0, 1).unwrap(),
                    _ => unreachable!(),
                })
            );
            assert_eq!(words[7], u32::from_le_bytes(encode_write_fpcr(13)));
        }
    }
}

#[test]
fn fused_multiply_add_emission_keeps_one_fmadd_and_width_lockstep() {
    use psi_numerics::arithmetic::ArithmeticDomain;

    for byte_size in [4usize, 8] {
        for domain in [
            ArithmeticDomain::Exact,
            ArithmeticDomain::Saturating,
            ArithmeticDomain::Trapping,
        ] {
            let mut bytes = Vec::new();
            append_runtime_float_binary_operation(
                &mut bytes,
                byte_size,
                17,
                StateGuardOperator::FusedMultiplyAdd,
                26,
                domain,
                [15, 14],
            )
            .expect("encode fused multiply-add");
            assert_eq!(
                bytes.len(),
                20 + float_policy_guard_width(
                    StateGuardOperator::FusedMultiplyAdd,
                    byte_size,
                    domain,
                ),
                "f{} {domain:?} width",
                byte_size * 8,
            );
            let instructions = bytes
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                .collect::<Vec<_>>();
            assert!(
                instructions.contains(&u32::from_le_bytes(
                    encode_float_fused_multiply_add(byte_size, 0, 0, 1, 2)
                        .expect("encode scalar FMADD"),
                )),
                "f{} must contain one scalar FMADD",
                byte_size * 8,
            );
            assert!(
                !instructions.contains(&u32::from_le_bytes(
                    encode_float_multiply(byte_size, 0, 0, 1).expect("encode scalar multiply"),
                )),
                "f{} FMA must not contain a separately rounded multiply",
                byte_size * 8,
            );
            assert!(
                !instructions.contains(&u32::from_le_bytes(
                    encode_float_add(byte_size, 0, 0, 1).expect("encode scalar add"),
                )),
                "f{} FMA must not contain a separate add",
                byte_size * 8,
            );
        }
    }
}

#[test]
fn directed_fused_multiply_add_balances_fpcr_and_keeps_one_fmadd() {
    use psi_numerics::arithmetic::ArithmeticDomain;

    for (operator, fpcr) in [
        (
            StateGuardOperator::FusedMultiplyAddTowardPositive,
            0x0040_0000_u64,
        ),
        (
            StateGuardOperator::FusedMultiplyAddTowardNegative,
            0x0080_0000_u64,
        ),
        (
            StateGuardOperator::FusedMultiplyAddTowardZero,
            0x00c0_0000_u64,
        ),
    ] {
        for byte_size in [4usize, 8] {
            let mut bytes = Vec::new();
            append_runtime_float_binary_operation(
                &mut bytes,
                byte_size,
                17,
                operator,
                26,
                ArithmeticDomain::Exact,
                [15, 14],
            )
            .expect("encode directed fused multiply-add");
            assert_eq!(
                bytes.len(),
                crate::aarch64::widths::runtime_float_binary_operation_width_with_domain(
                    operator,
                    byte_size,
                    ArithmeticDomain::Exact,
                )
            );
            let words = bytes
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(words[3], u32::from_le_bytes(encode_read_fpcr(13)));
            assert_eq!(words[4], u32::from_le_bytes(encode_movz(12, 0)));
            assert_eq!(
                words[5],
                u32::from_le_bytes(encode_movk(12, ((fpcr >> 16) & 0xffff) as u16, 1,))
            );
            assert_eq!(words[6], u32::from_le_bytes(encode_write_fpcr(12)));
            assert_eq!(
                words[7],
                u32::from_le_bytes(encode_float_fused_multiply_add(byte_size, 0, 0, 1, 2).unwrap())
            );
            assert_eq!(words[8], u32::from_le_bytes(encode_write_fpcr(13)));
        }
    }
}
