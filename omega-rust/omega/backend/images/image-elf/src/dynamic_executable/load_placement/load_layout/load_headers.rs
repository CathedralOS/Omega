//! ELF load program headers, placed dynamic sections and memory
//! placements.

use crate::dynamic_executable::section_headers::relative_section_layout::ElfRelativeSectionPayloadRegion;

/// Ordered program-header role closed by the dynamic load-layout carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfLoadProgramHeaderKind {
    Interpreter = 1,
    LoadReadOnly = 2,
    LoadReadExecute = 3,
    LoadReadWrite = 4,
    Dynamic = 5,
}

/// Public observation vocabulary for the closed placed section roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfPlacedDynamicSectionKind {
    Null = 0,
    Interpreter = 1,
    DynamicString = 2,
    DynamicSymbol = 3,
    SystemVHash = 4,
    GnuSymbolVersion = 5,
    GnuVersionRequirement = 6,
    GnuHash = 7,
    ProcedureLinkage = 8,
    ProcedureGot = 9,
    ProcedureRelocation = 10,
    GeneralRelocation = 11,
    DynamicTable = 12,
    SectionNameTable = 13,
}

/// Absolute geometry for one future `Elf64_Phdr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfLoadProgramHeader {
    pub(crate) kind: ElfLoadProgramHeaderKind,
    pub(crate) flags: u32,
    pub(crate) file_offset: u64,
    pub(crate) virtual_address: u64,
    pub(crate) physical_address: u64,
    pub(crate) file_size: u64,
    pub(crate) memory_size: u64,
    pub(crate) alignment: u64,
}

impl ElfLoadProgramHeader {
    pub const fn kind(&self) -> ElfLoadProgramHeaderKind {
        self.kind
    }

    pub const fn flags(&self) -> u32 {
        self.flags
    }

    pub const fn file_offset(&self) -> u64 {
        self.file_offset
    }

    pub const fn virtual_address(&self) -> u64 {
        self.virtual_address
    }

    pub const fn physical_address(&self) -> u64 {
        self.physical_address
    }

    pub const fn file_size(&self) -> u64 {
        self.file_size
    }

    pub const fn memory_size(&self) -> u64 {
        self.memory_size
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }
}

/// Absolute placement of one row in the closed fourteen-section roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfPlacedDynamicSection {
    pub(crate) index: u32,
    pub(crate) kind: ElfPlacedDynamicSectionKind,
    pub(crate) region: Option<ElfRelativeSectionPayloadRegion>,
    pub(crate) file_offset: u64,
    pub(crate) virtual_address: Option<u64>,
    pub(crate) byte_size: u64,
    pub(crate) alignment: u64,
}

impl ElfPlacedDynamicSection {
    pub const fn index(&self) -> u32 {
        self.index
    }

    pub const fn kind(&self) -> ElfPlacedDynamicSectionKind {
        self.kind
    }

    pub const fn region(&self) -> Option<ElfRelativeSectionPayloadRegion> {
        self.region
    }

    pub const fn file_offset(&self) -> u64 {
        self.file_offset
    }

    pub const fn virtual_address(&self) -> Option<u64> {
        self.virtual_address
    }

    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }
}

/// Meaning of a retained section-header placement resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfSectionPlacementResolutionKind {
    VirtualAddress = 1,
    FileOffset = 2,
}

/// One resolved value for an existing typed section-header fixup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfResolvedSectionHeaderPlacement {
    pub(crate) row_index: u32,
    pub(crate) section_kind: ElfPlacedDynamicSectionKind,
    pub(crate) byte_offset: usize,
    pub(crate) byte_width: u8,
    pub(crate) kind: ElfSectionPlacementResolutionKind,
    pub(crate) value: u64,
}

/// Placement of the retained non-section `FinalImage` memory domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfLoadImageMemoryPlacement {
    pub(crate) text_file_offset: u64,
    pub(crate) text_virtual_address: u64,
    pub(crate) text_size: u64,
    pub(crate) data_file_offset: u64,
    pub(crate) data_virtual_address: u64,
    pub(crate) data_size: u64,
    pub(crate) bss_virtual_address: u64,
    pub(crate) bss_size: u64,
    pub(crate) bss_alignment: u64,
}

impl ElfLoadImageMemoryPlacement {
    pub const fn text_file_offset(&self) -> u64 {
        self.text_file_offset
    }

    pub const fn text_virtual_address(&self) -> u64 {
        self.text_virtual_address
    }

    pub const fn text_size(&self) -> u64 {
        self.text_size
    }

    pub const fn data_file_offset(&self) -> u64 {
        self.data_file_offset
    }

    pub const fn data_virtual_address(&self) -> u64 {
        self.data_virtual_address
    }

    pub const fn data_size(&self) -> u64 {
        self.data_size
    }

    pub const fn bss_virtual_address(&self) -> u64 {
        self.bss_virtual_address
    }

    pub const fn bss_size(&self) -> u64 {
        self.bss_size
    }

    pub const fn bss_alignment(&self) -> u64 {
        self.bss_alignment
    }
}

impl ElfResolvedSectionHeaderPlacement {
    pub const fn row_index(&self) -> u32 {
        self.row_index
    }

    pub const fn section_kind(&self) -> ElfPlacedDynamicSectionKind {
        self.section_kind
    }

    pub const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub const fn byte_width(&self) -> u8 {
        self.byte_width
    }

    pub const fn kind(&self) -> ElfSectionPlacementResolutionKind {
        self.kind
    }

    pub const fn value(&self) -> u64 {
        self.value
    }
}
