mod copies;
mod sections;

use crate::model::FinalImage;
use crate::symbols::final_image_symbol_handle;
use omega_core::arena::Arena;
use omega_object_file::{ObjectPlan, RelocationPlan, SectionKind, SymbolKind};
use omega_target::NativeTarget;

pub struct FinalImageInput<'a> {
    pub target: NativeTarget,
    pub object: &'a ObjectPlan,
    pub relocations: &'a RelocationPlan,
    pub text_bytes: &'a [u8],
    pub data_bytes: &'a [u8],
}

pub fn build_final_image(input: FinalImageInput<'_>) -> FinalImage {
    let import_count = input
        .object
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind == SymbolKind::Import)
        .count();
    let mut image = FinalImage {
        target: input.target,
        entry_symbol: final_image_symbol_handle(input.object.entry_symbol),
        text: input.text_bytes.to_vec(),
        data: input.data_bytes.to_vec(),
        bss_size: sections::section_size(input.object, SectionKind::Bss),
        bss_alignment: sections::section_alignment(input.object, SectionKind::Bss),
        symbols: Arena::with_capacity(input.object.symbols.len()),
        imports: Arena::with_capacity(import_count),
        relocations: Arena::with_capacity(input.relocations.records.len()),
    };

    copies::copy_object_symbols(&mut image, input.object);
    copies::copy_object_imports(&mut image, input.object);
    copies::copy_object_relocations(&mut image, input.relocations);

    image
}

#[cfg(test)]
mod tests {
    use super::{FinalImageInput, build_final_image};
    use crate::{FinalImageLayout, FinalImageSection, final_image_symbol_address};
    use omega_core::arena::{Arena, Handle};
    use omega_object_file::{
        ObjectPlan, RelocationKind, RelocationPlan, RelocationRecord, SectionKind, SectionPlan,
        SymbolKind, SymbolPlan, SymbolSection,
    };
    use omega_target::NativeTarget;

    #[test]
    fn builds_final_image_from_object_symbols_imports_and_relocations() {
        let target = NativeTarget::host();
        let mut object = ObjectPlan {
            target,
            sections: Arena::new(),
            symbols: Arena::new(),
            entry_symbol: Handle::invalid(),
        };
        object.sections.insert(SectionPlan {
            kind: SectionKind::Text,
            size: 8,
            alignment: 16,
        });
        object.sections.insert(SectionPlan {
            kind: SectionKind::Data,
            size: 3,
            alignment: 4,
        });
        object.sections.insert(SectionPlan {
            kind: SectionKind::Bss,
            size: 24,
            alignment: 8,
        });

        let entry_symbol = object.symbols.insert(SymbolPlan {
            name: "_start".into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 4,
            size: 4,
            kind: SymbolKind::Function,
        });
        let import_symbol = object.symbols.insert(SymbolPlan {
            name: "host_write".into(),
            section: SymbolSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        object.symbols.insert(SymbolPlan {
            name: "payload".into(),
            section: SymbolSection::Section(SectionKind::Data),
            offset: 2,
            size: 1,
            kind: SymbolKind::Object,
        });
        object.entry_symbol = entry_symbol;

        let mut relocations = RelocationPlan {
            target,
            records: Arena::new(),
        };
        relocations.records.insert(RelocationRecord {
            function_symbol_handle: entry_symbol,
            selected_instruction_index: 0,
            text_offset: 4,
            byte_width: 4,
            symbol_handle: import_symbol,
            kind: RelocationKind::X86_64Relative32,
        });

        let image = build_final_image(FinalImageInput {
            target,
            object: &object,
            relocations: &relocations,
            text_bytes: &[0xaa; 8],
            data_bytes: &[1, 2, 3],
        });

        assert_eq!(image.target, target);
        assert_eq!(image.text, vec![0xaa; 8]);
        assert_eq!(image.data, vec![1, 2, 3]);
        assert_eq!((image.bss_size, image.bss_alignment), (24, 8));
        assert_eq!(image.symbols.len(), 3);
        assert_eq!(image.imports.len(), 1);
        assert_eq!(image.relocations.len(), 1);

        let entry = image.symbols.get(image.entry_symbol);
        assert_eq!(entry.name, "_start");
        assert_eq!(entry.section, FinalImageSection::Text);
        assert_eq!(entry.offset, 4);

        let import = image.imports.iter().next().expect("expected import").1;
        assert_eq!(image.symbols.get(import.symbol_handle).name, "host_write");
        assert_eq!(
            image.relocations.iter().next().unwrap().1.symbol_handle,
            import.symbol_handle
        );
        assert_eq!(
            final_image_symbol_address(
                &image,
                image.entry_symbol,
                &FinalImageLayout {
                    text_address: 0x1000,
                    data_address: 0x2000,
                    bss_address: 0x3000,
                }
            ),
            Some(0x1004)
        );
    }
}
