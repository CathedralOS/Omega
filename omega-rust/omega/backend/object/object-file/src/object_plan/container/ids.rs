//! The OMGOBJ tag tables, 1-based so a zero word is never a valid discriminant,
//! and the only one of the crate's three architecture/format mappings under test.

use crate::{RelocationKind, SectionKind, SymbolKind};
use target::{Architecture, ObjectFormat};

pub(super) fn architecture_id(architecture: Architecture) -> u32 {
    match architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }
}

pub(super) fn object_format_id(object_format: ObjectFormat) -> u32 {
    match object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }
}

pub(super) fn symbol_kind_id(symbol_kind: SymbolKind) -> u32 {
    match symbol_kind {
        SymbolKind::Function => 1,
        SymbolKind::Import => 2,
        SymbolKind::Object => 3,
    }
}

pub(super) fn section_kind_id(section_kind: SectionKind) -> u32 {
    match section_kind {
        SectionKind::Text => 1,
        SectionKind::Data => 2,
        SectionKind::Bss => 3,
    }
}

pub(super) fn relocation_kind_id(relocation_kind: RelocationKind) -> u32 {
    match relocation_kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        architecture_id, object_format_id, relocation_kind_id, section_kind_id, symbol_kind_id,
    };
    use crate::{RelocationKind, SectionKind, SymbolKind};
    use target::{Architecture, ObjectFormat};

    #[test]
    fn maps_object_container_ids_stably() {
        assert_eq!(architecture_id(Architecture::Aarch64), 1);
        assert_eq!(architecture_id(Architecture::X86_64), 2);
        assert_eq!(object_format_id(ObjectFormat::Elf), 1);
        assert_eq!(object_format_id(ObjectFormat::MachO), 2);
        assert_eq!(object_format_id(ObjectFormat::Coff), 3);
        assert_eq!(symbol_kind_id(SymbolKind::Function), 1);
        assert_eq!(symbol_kind_id(SymbolKind::Import), 2);
        assert_eq!(symbol_kind_id(SymbolKind::Object), 3);
        assert_eq!(section_kind_id(SectionKind::Text), 1);
        assert_eq!(section_kind_id(SectionKind::Data), 2);
        assert_eq!(section_kind_id(SectionKind::Bss), 3);
        assert_eq!(relocation_kind_id(RelocationKind::Aarch64Page21), 1);
        assert_eq!(relocation_kind_id(RelocationKind::Aarch64PageOffset12), 2);
        assert_eq!(relocation_kind_id(RelocationKind::Aarch64Branch26), 3);
        assert_eq!(relocation_kind_id(RelocationKind::Absolute64), 4);
        assert_eq!(relocation_kind_id(RelocationKind::X86_64Relative32), 5);
    }
}
