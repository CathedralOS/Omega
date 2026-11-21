/*
We compute the single-pass byte and virtual-address layout (`MachoContainerPlan`)
for Mach-O `MH_EXECUTE` and `MH_DYLIB` containers across `__PAGEZERO`,
`__TEXT`, optional `__DATA`, and `__LINKEDIT` (rebase, bind, export trie, and
code-signature payloads).
*/

pub(crate) mod bytes {
    pub(crate) use image::codec_bytes::{write_u32, write_u64};

    pub(crate) fn write_be_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_be_bytes());
    }

    pub(crate) fn write_be_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend(value.to_be_bytes());
    }

    pub(crate) fn write_fixed_string_16(bytes: &mut Vec<u8>, value: &str) {
        let value_bytes = value.as_bytes();
        assert!(
            value_bytes.len() <= 16,
            "fixed Mach-O string is longer than 16 bytes"
        );
        bytes.extend(value_bytes);
        bytes.resize(bytes.len() + (16 - value_bytes.len()), 0);
    }

    pub(crate) fn write_reserved_u32s(bytes: &mut Vec<u8>, count: usize) {
        bytes.resize(bytes.len() + count * 4, 0);
    }
}

pub(crate) mod constants {
    pub(crate) const MACHO_EXECUTABLE_BASE: u64 = 0x1_0000_0000;
    pub(super) const MACHO_HEADER_SIZE: usize = 32;
    pub(crate) const MACHO_SEGMENT_COMMAND_SIZE: usize = 72;
    pub(crate) const MACHO_SECTION_SIZE: usize = 80;
    pub(crate) const MACHO_LOAD_DYLINKER_COMMAND_SIZE: usize = 32;
    pub(crate) const MACHO_UUID_COMMAND_SIZE: usize = 24;
    pub(crate) const MACHO_MAIN_COMMAND_SIZE: usize = 24;
    pub(crate) const MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE: usize = 32;
    pub(crate) const MACHO_DYLD_INFO_COMMAND_SIZE: usize = 48;
    pub(crate) const MACHO_SYMTAB_COMMAND_SIZE: usize = 24;
    pub(crate) const MACHO_DYSYMTAB_COMMAND_SIZE: usize = 80;
    pub(crate) const MACHO_CODE_SIGNATURE_COMMAND_SIZE: usize = 16;
    pub(crate) const MACHO_HEADER_FLAGS_NOUNDEFS: u32 = 0x1;
    pub(crate) const MACHO_HEADER_FLAGS_DYLDLINK: u32 = 0x4;
    pub(crate) const MACHO_HEADER_FLAGS_TWOLEVEL: u32 = 0x80;
    pub(crate) const MACHO_HEADER_FLAGS_PIE: u32 = 0x20_0000;
}

pub(crate) mod layout {
    pub(crate) fn alignment_power(alignment: usize) -> u32 {
        alignment.max(1).trailing_zeros()
    }

    pub(crate) use image::{align_to, align_to_u64};
}

pub(crate) mod plan {
    use crate::code_signature::code_signature_size;
    use crate::dyld_linking::load_commands::MachoDylib;
    use crate::file_layout::constants::{
        MACHO_CODE_SIGNATURE_COMMAND_SIZE, MACHO_DYLD_INFO_COMMAND_SIZE,
        MACHO_DYSYMTAB_COMMAND_SIZE, MACHO_EXECUTABLE_BASE,
        MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE, MACHO_HEADER_SIZE,
        MACHO_LOAD_DYLINKER_COMMAND_SIZE, MACHO_MAIN_COMMAND_SIZE, MACHO_SECTION_SIZE,
        MACHO_SEGMENT_COMMAND_SIZE, MACHO_SYMTAB_COMMAND_SIZE, MACHO_UUID_COMMAND_SIZE,
    };
    use crate::file_layout::layout::{align_to, align_to_u64};
    use crate::isa::MachoIsa;
    use image::{FinalImage, FinalImageLayout};

    pub(crate) struct MachOImagePlan {
        pub(crate) has_dyld_info: bool,
        pub(crate) has_data_segment: bool,
        pub(crate) data_segment_index: Option<u8>,
        pub(crate) command_count: usize,
        pub(crate) sizeofcmds: usize,
        pub(crate) text_offset: usize,
        pub(crate) data_offset: usize,
        pub(crate) text_address: u64,
        pub(crate) data_address: u64,
        pub(crate) bss_address: u64,
        pub(crate) text_file_size: usize,
        pub(crate) data_vm_size: u64,
        pub(crate) rebase_offset: usize,
        pub(crate) bind_offset: usize,
        pub(crate) export_offset: usize,
        pub(crate) code_signature_offset: usize,
        pub(crate) code_signature_size: usize,
        pub(crate) linkedit_vmaddr: u64,
        pub(crate) linkedit_offset: usize,
        pub(crate) linkedit_filesize: usize,
        pub(crate) linkedit_vmsize: usize,
    }

    impl MachOImagePlan {
        pub(crate) fn final_image_layout(&self) -> FinalImageLayout {
            FinalImageLayout {
                text_address: self.text_address,
                data_address: self.data_address,
                bss_address: self.bss_address,
            }
        }
    }

    pub(crate) fn macho_data_segment_index(image: &FinalImage, dylib: bool) -> Option<u8> {
        if image.memory.data.is_empty() && image.memory.bss_size == 0 {
            return None;
        }
        let pagezero_segments = u8::from(!dylib);
        Some(pagezero_segments + 1) // TEXT precedes DATA.
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn plan_macho_container(
        image: &FinalImage,
        import_count: usize,
        rebase_size: usize,
        bind_size: usize,
        dylibs: &[MachoDylib],
        code_signature_identifier: &str,
        isa: MachoIsa,
        identity: Option<&MachoDylib>,
        export_size: usize,
    ) -> MachOImagePlan {
        let page_size = usize::try_from(isa.page_size()).expect("Mach-O page size");
        let has_imports = import_count > 0;
        let has_dyld_info = rebase_size > 0 || has_imports;
        let data_section_count =
            usize::from(!image.memory.data.is_empty()) + usize::from(image.memory.bss_size > 0);
        let has_data_segment = data_section_count > 0;
        let dylib_commands_size: usize = dylibs.iter().map(MachoDylib::command_size).sum();
        let (root_commands, root_size) = if let Some(identity) = identity {
            (2, identity.command_size() + 16) // LC_ID_DYLIB and LC_DYLD_EXPORTS_TRIE.
        } else {
            (
                3,
                MACHO_SEGMENT_COMMAND_SIZE
                    + MACHO_LOAD_DYLINKER_COMMAND_SIZE
                    + MACHO_MAIN_COMMAND_SIZE,
            )
        };
        let command_count = 7
            + root_commands
            + dylibs.len()
            + usize::from(has_data_segment)
            + usize::from(has_dyld_info);
        let sizeofcmds = root_size
            + (MACHO_SEGMENT_COMMAND_SIZE + MACHO_SECTION_SIZE)
            + usize::from(has_data_segment)
                * (MACHO_SEGMENT_COMMAND_SIZE + data_section_count * MACHO_SECTION_SIZE)
            + MACHO_UUID_COMMAND_SIZE
            + MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE
            + dylib_commands_size
            + usize::from(has_dyld_info) * MACHO_DYLD_INFO_COMMAND_SIZE
            + MACHO_SYMTAB_COMMAND_SIZE
            + MACHO_DYSYMTAB_COMMAND_SIZE
            + MACHO_SEGMENT_COMMAND_SIZE
            + MACHO_CODE_SIGNATURE_COMMAND_SIZE;
        let text_offset = align_to(MACHO_HEADER_SIZE + sizeofcmds, 16);
        let data_offset = align_to(text_offset + image.memory.text.len(), page_size);
        let text_address = MACHO_EXECUTABLE_BASE + text_offset as u64;
        let data_address = MACHO_EXECUTABLE_BASE + data_offset as u64;
        let bss_address = align_to_u64(
            data_address + image.memory.data.len() as u64,
            image.memory.bss_alignment as u64,
        );
        let text_file_size = if has_data_segment {
            data_offset
        } else {
            align_to(text_offset + image.memory.text.len(), page_size)
        };
        let data_memory_size = if has_data_segment {
            (bss_address - data_address)
                .checked_add(image.memory.bss_size as u64)
                .expect("Mach-O data memory size overflow")
        } else {
            0
        };
        let data_vm_size = align_to_u64(data_memory_size, isa.page_size());
        let unsigned_file_end = if has_data_segment {
            data_offset + image.memory.data.len()
        } else {
            text_offset + image.memory.text.len()
        };
        let rebase_offset = align_to(unsigned_file_end, page_size);
        let bind_offset = align_to(rebase_offset + rebase_size, 8);
        let export_offset = align_to(bind_offset + bind_size, 8);
        let code_signature_offset = align_to(export_offset + export_size, page_size);
        let code_signature_size =
            code_signature_size(code_signature_offset, code_signature_identifier, isa);
        let linkedit_vmaddr = if has_data_segment {
            data_address
                .checked_add(data_vm_size)
                .expect("Mach-O LINKEDIT vm address overflow")
        } else {
            MACHO_EXECUTABLE_BASE
                .checked_add(bind_offset as u64)
                .expect("Mach-O LINKEDIT vm address overflow")
        };
        let linkedit_offset = rebase_offset;
        let linkedit_filesize = code_signature_offset + code_signature_size - linkedit_offset;
        let linkedit_vmsize = align_to(linkedit_filesize, page_size);

        MachOImagePlan {
            has_dyld_info,
            has_data_segment,
            data_segment_index: macho_data_segment_index(image, identity.is_some()),
            command_count,
            sizeofcmds,
            text_offset,
            data_offset,
            text_address,
            data_address,
            bss_address,
            text_file_size,
            data_vm_size,
            rebase_offset,
            bind_offset,
            export_offset,
            code_signature_offset,
            code_signature_size,
            linkedit_vmaddr,
            linkedit_offset,
            linkedit_filesize,
            linkedit_vmsize,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::plan_macho_container;
        use crate::file_layout::constants::MACHO_EXECUTABLE_BASE;
        use crate::isa::MachoIsa;
        use image::FinalImage;

        const PAGE_SIZE: usize = 0x4000;

        #[test]
        fn plans_macho_data_bss_and_linkedit_layout() {
            let image = crate::test_support::image(
                FinalImage::default().target,
                image::FinalImageMemory {
                    text: vec![0; 3],
                    data: vec![0; 5],
                    bss_size: 7,
                    bss_alignment: 8,
                },
            );

            let plan = plan_macho_container(
                &image,
                1,
                0,
                12,
                &[crate::dyld_linking::load_commands::MachoDylib::LIBSYSTEM],
                "omega-program",
                MachoIsa::Aarch64,
                None,
                0,
            );

            assert!(plan.has_dyld_info);
            assert!(plan.has_data_segment);
            assert_eq!(plan.command_count, 13);
            assert_eq!(plan.data_offset, PAGE_SIZE);
            assert_eq!(
                plan.text_address,
                MACHO_EXECUTABLE_BASE + plan.text_offset as u64
            );
            assert_eq!(plan.data_address, MACHO_EXECUTABLE_BASE + PAGE_SIZE as u64);
            assert_eq!(plan.bss_address, plan.data_address + 8);
            assert_eq!(plan.data_vm_size, PAGE_SIZE as u64);
            assert_eq!(plan.bind_offset, PAGE_SIZE * 2);
            assert_eq!(plan.code_signature_offset, PAGE_SIZE * 3);
            assert_eq!(plan.linkedit_vmaddr, plan.data_address + plan.data_vm_size);
            assert!(plan.linkedit_filesize >= plan.code_signature_size);
        }

        #[test]
        fn rebase_only_image_keeps_distinct_linkedit_offsets() {
            let image = crate::test_support::image(
                FinalImage::default().target,
                image::FinalImageMemory {
                    text: vec![0; 4],
                    data: vec![0; 8],
                    ..Default::default()
                },
            );

            let plan = plan_macho_container(
                &image,
                0,
                5,
                0,
                &[crate::dyld_linking::load_commands::MachoDylib::LIBSYSTEM],
                "omega-program",
                MachoIsa::Aarch64,
                None,
                0,
            );

            assert!(plan.has_dyld_info);
            assert_eq!(plan.rebase_offset, PAGE_SIZE * 2);
            assert_eq!(plan.bind_offset, plan.rebase_offset + 8);
            assert_eq!(plan.linkedit_offset, plan.rebase_offset);
            assert_eq!(plan.code_signature_offset, PAGE_SIZE * 3);
        }
    }
}
