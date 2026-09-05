//! Every file offset and VM address in one pass, once the thunk, rebase and bind
//! sizes are all known.

use crate::code_signature::code_signature_size;
use crate::constants::{
    MACHO_ARM64_PAGE_SIZE, MACHO_CODE_SIGNATURE_COMMAND_SIZE, MACHO_DYLD_INFO_COMMAND_SIZE,
    MACHO_DYSYMTAB_COMMAND_SIZE, MACHO_EXECUTABLE_BASE,
    MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE, MACHO_HEADER_SIZE,
    MACHO_LOAD_DYLINKER_COMMAND_SIZE, MACHO_MAIN_COMMAND_SIZE, MACHO_SECTION_SIZE,
    MACHO_SEGMENT_COMMAND_SIZE, MACHO_SYMTAB_COMMAND_SIZE, MACHO_UUID_COMMAND_SIZE,
};
use crate::layout::{align_to, align_to_u64};
use crate::load_commands::MachoDylib;
use image::{FinalImage, FinalImageLayout};

pub(crate) struct MachOImagePlan {
    pub(crate) has_dyld_info: bool,
    pub(crate) has_data_segment: bool,
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

pub(crate) fn plan_macho_image(
    image: &FinalImage,
    import_count: usize,
    rebase_size: usize,
    bind_size: usize,
    dylibs: &[MachoDylib],
) -> MachOImagePlan {
    let has_imports = import_count > 0;
    let has_dyld_info = rebase_size > 0 || has_imports;
    let data_section_count =
        usize::from(!image.memory.data.is_empty()) + usize::from(image.memory.bss_size > 0);
    let has_data_segment = data_section_count > 0;
    // The 10 always-present commands (2 segments minimum, dylinker, uuid,
    // build_version, main, linkedit, symtab, dysymtab, code_signature) plus one
    // LC_LOAD_DYLIB per linked dylib (≥1: libSystem) plus the optional data
    // segment + dyld_info(bind) commands.
    let dylib_commands_size: usize = dylibs.iter().map(MachoDylib::command_size).sum();
    let command_count =
        10 + dylibs.len() + usize::from(has_data_segment) + usize::from(has_dyld_info);
    let sizeofcmds = MACHO_SEGMENT_COMMAND_SIZE
        + (MACHO_SEGMENT_COMMAND_SIZE + MACHO_SECTION_SIZE)
        + usize::from(has_data_segment)
            * (MACHO_SEGMENT_COMMAND_SIZE + data_section_count * MACHO_SECTION_SIZE)
        + MACHO_LOAD_DYLINKER_COMMAND_SIZE
        + MACHO_UUID_COMMAND_SIZE
        + MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE
        + MACHO_MAIN_COMMAND_SIZE
        + dylib_commands_size
        + usize::from(has_dyld_info) * MACHO_DYLD_INFO_COMMAND_SIZE
        + MACHO_SYMTAB_COMMAND_SIZE
        + MACHO_DYSYMTAB_COMMAND_SIZE
        + MACHO_SEGMENT_COMMAND_SIZE
        + MACHO_CODE_SIGNATURE_COMMAND_SIZE;
    let text_offset = align_to(MACHO_HEADER_SIZE + sizeofcmds, 16);
    let data_offset = align_to(text_offset + image.memory.text.len(), MACHO_ARM64_PAGE_SIZE);
    let text_address = MACHO_EXECUTABLE_BASE + text_offset as u64;
    let data_address = MACHO_EXECUTABLE_BASE + data_offset as u64;
    let bss_address = align_to_u64(
        data_address + image.memory.data.len() as u64,
        image.memory.bss_alignment as u64,
    );
    let text_file_size = if has_data_segment {
        data_offset
    } else {
        align_to(text_offset + image.memory.text.len(), MACHO_ARM64_PAGE_SIZE)
    };
    let data_memory_size = if has_data_segment {
        (bss_address - data_address)
            .checked_add(image.memory.bss_size as u64)
            .expect("Mach-O data memory size overflow")
    } else {
        0
    };
    let data_vm_size = align_to_u64(data_memory_size, MACHO_ARM64_PAGE_SIZE as u64);
    let unsigned_file_end = if has_data_segment {
        data_offset + image.memory.data.len()
    } else {
        text_offset + image.memory.text.len()
    };
    let rebase_offset = align_to(unsigned_file_end, MACHO_ARM64_PAGE_SIZE);
    let bind_offset = rebase_offset + rebase_size;
    let code_signature_offset = align_to(bind_offset + bind_size, MACHO_ARM64_PAGE_SIZE);
    let code_signature_size = code_signature_size(code_signature_offset);
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
    let linkedit_vmsize = align_to(linkedit_filesize, MACHO_ARM64_PAGE_SIZE);

    MachOImagePlan {
        has_dyld_info,
        has_data_segment,
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
    use super::plan_macho_image;
    use crate::constants::{MACHO_ARM64_PAGE_SIZE, MACHO_EXECUTABLE_BASE};
    use arena::Handle;
    use image::FinalImage;

    #[test]
    fn plans_macho_data_bss_and_linkedit_layout() {
        let image = FinalImage::with_capacity(
            FinalImage::default().target,
            image::FinalImageMemory {
                text: vec![0; 3],
                data: vec![0; 5],
                bss_size: 7,
                bss_alignment: 8,
            },
            Handle::invalid(),
            0,
            0,
            0,
        );

        let plan = plan_macho_image(
            &image,
            1,
            0,
            12,
            &[crate::load_commands::MachoDylib::LIBSYSTEM],
        );

        assert!(plan.has_dyld_info);
        assert!(plan.has_data_segment);
        assert_eq!(plan.command_count, 13);
        assert_eq!(plan.data_offset, MACHO_ARM64_PAGE_SIZE);
        assert_eq!(
            plan.text_address,
            MACHO_EXECUTABLE_BASE + plan.text_offset as u64
        );
        assert_eq!(
            plan.data_address,
            MACHO_EXECUTABLE_BASE + MACHO_ARM64_PAGE_SIZE as u64
        );
        assert_eq!(plan.bss_address, plan.data_address + 8);
        assert_eq!(plan.data_vm_size, MACHO_ARM64_PAGE_SIZE as u64);
        assert_eq!(plan.bind_offset, MACHO_ARM64_PAGE_SIZE * 2);
        assert_eq!(plan.code_signature_offset, MACHO_ARM64_PAGE_SIZE * 3);
        assert_eq!(plan.linkedit_vmaddr, plan.data_address + plan.data_vm_size);
        assert!(plan.linkedit_filesize >= plan.code_signature_size);
    }

    #[test]
    fn rebase_only_image_keeps_distinct_linkedit_offsets() {
        let image = FinalImage::with_capacity(
            FinalImage::default().target,
            image::FinalImageMemory {
                text: vec![0; 4],
                data: vec![0; 8],
                ..Default::default()
            },
            Handle::invalid(),
            0,
            0,
            0,
        );

        let plan = plan_macho_image(
            &image,
            0,
            5,
            0,
            &[crate::load_commands::MachoDylib::LIBSYSTEM],
        );

        assert!(plan.has_dyld_info);
        assert_eq!(plan.rebase_offset, MACHO_ARM64_PAGE_SIZE * 2);
        assert_eq!(plan.bind_offset, plan.rebase_offset + 5);
        assert_eq!(plan.linkedit_offset, plan.rebase_offset);
        assert_eq!(plan.code_signature_offset, MACHO_ARM64_PAGE_SIZE * 3);
    }
}
