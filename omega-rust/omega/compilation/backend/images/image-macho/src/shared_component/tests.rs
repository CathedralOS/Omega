use super::{MachoComponentExport, emit_macho_shared_component};
use arena::Handle;
use image::{
    FinalImage, FinalImageMemory, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
};
use object_file::{RelocationKind, SymbolKind};

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn wide(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
fn commands(bytes: &[u8]) -> Vec<&[u8]> {
    let mut result = Vec::new();
    let mut cursor = 32;
    for _ in 0..word(bytes, 16) {
        let size = word(bytes, cursor + 4) as usize;
        result.push(&bytes[cursor..cursor + size]);
        cursor += size;
    }
    assert_eq!(cursor, 32 + word(bytes, 20) as usize);
    result
}

fn image(target: target::NativeTarget) -> (FinalImage, Vec<MachoComponentExport>) {
    let text = if target.architecture == target::Architecture::Aarch64 {
        0xd65f_03c0u32.to_le_bytes().to_vec()
    } else {
        vec![0xc3, 0, 0, 0]
    };
    let mut image = FinalImage::with_capacity(
        target,
        FinalImageMemory {
            text,
            data: vec![0; 13],
            bss_size: 24,
            bss_alignment: 16,
        },
        Handle::invalid(),
        2,
        0,
        1,
    );
    let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "selected component entry".into(),
        section: FinalImageSection::Text,
        size: 4,
        kind: SymbolKind::Function,
        ..Default::default()
    });
    image.symbol_table.entry_symbol = entry;
    let storage = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "owned storage".into(),
        section: FinalImageSection::Bss,
        size: 24,
        ..Default::default()
    });
    image
        .relocation_table
        .relocations
        .insert(FinalImageRelocation {
            section: FinalImageSection::Data,
            byte_width: 8,
            symbol_handle: storage,
            kind: RelocationKind::Absolute64,
            ..Default::default()
        });
    (
        image,
        vec![MachoComponentExport {
            symbol: entry,
            name: b"__exact_selected_slot".to_vec(),
            slot_commitment: [7; 32],
        }],
    )
}

#[test]
fn dylib_preserves_original_exports_and_rebases_under_its_own_segment_order() {
    for target in [
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::macos_x64(),
    ] {
        let (original, exports) = image(target);
        let text = original.memory.text.clone();
        let data = original.memory.data.clone();
        let emitted =
            emit_macho_shared_component(original, exports.clone()).expect("emit actual dylib");
        assert_eq!(emitted.image().memory.text, text);
        assert_eq!(emitted.image().memory.data, data);
        assert_eq!(emitted.exports(), exports);
        let bytes = emitted.contents();
        assert_eq!(word(bytes, 12), 6); // MH_DYLIB, never a renamed MH_EXECUTE.
        assert_eq!(word(bytes, 24), 0x85); // Two-level dyld, no executable PIE flag.
        let commands = commands(bytes);
        assert!(
            !commands
                .iter()
                .any(|command| matches!(word(command, 0), 0xe | 0x8000_0028))
        );
        assert!(!commands.iter().any(
            |command| word(command, 0) == 0x19 && &command[8..24] == b"__PAGEZERO\0\0\0\0\0\0"
        ));
        let dyld = commands
            .iter()
            .find(|command| word(command, 0) == 0x8000_0022)
            .unwrap();
        let rebase = word(dyld, 8) as usize;
        assert_eq!(bytes[rebase], 0x11);
        assert_eq!(bytes[rebase + 1], 0x21); // Actual DATA segment index one.
        let text_segment = commands
            .iter()
            .find(|command| {
                word(command, 0) == 0x19 && &command[8..24] == b"__TEXT\0\0\0\0\0\0\0\0\0\0"
            })
            .unwrap();
        let text_offset = word(text_segment, 72 + 48) as usize;
        super::validate_export_trie(bytes, emitted.image(), emitted.exports(), text_offset)
            .expect("independent actual-tree replay");
        let signature = commands
            .iter()
            .find(|command| word(command, 0) == 0x1d)
            .unwrap();
        let signature_start = word(signature, 8) as usize;
        let directory = u32::from_be_bytes(
            bytes[signature_start + 16..signature_start + 20]
                .try_into()
                .unwrap(),
        ) as usize
            + signature_start;
        assert_eq!(
            u64::from_be_bytes(bytes[directory + 80..directory + 88].try_into().unwrap()),
            0
        );
        assert!(
            crate::code_signature_identifier(bytes)
                .unwrap()
                .starts_with("omega-component-")
        );
    }
}

#[test]
fn export_reader_rejects_identity_offset_cycle_truncation_and_unreachable_bytes() {
    let expected = vec![(b"slot".to_vec(), 256)];
    let bytes = super::exports::encode(&expected).unwrap();
    super::exports::validate(&bytes, &expected).unwrap();
    let mut wrong = bytes.clone();
    let last = wrong.len() - 1;
    wrong[last - 2] ^= 1; // Actual exported address, retaining the exact name.
    assert!(super::exports::validate(&wrong, &expected).is_err());
    wrong = bytes.clone();
    wrong[4] = 0; // Root's child points back into root.
    assert!(super::exports::validate(&wrong, &expected).is_err());
    wrong = bytes.clone();
    wrong[last - 3] = 4; // Nonregular export flags.
    assert!(super::exports::validate(&wrong, &expected).is_err());
    for length in 0..bytes.len() {
        assert!(super::exports::validate(&bytes[..length], &expected).is_err());
    }
    wrong = bytes.clone();
    wrong.push(0);
    assert!(super::exports::validate(&wrong, &expected).is_err());
    assert!(super::exports::validate(&bytes, &[(b"other".to_vec(), 256)]).is_err());
    let joined = vec![(b"shared".to_vec(), 8), (b"sharing".to_vec(), 12)];
    super::exports::validate(&super::exports::encode(&joined).unwrap(), &joined).unwrap();
}

#[test]
fn rejected_exports_return_the_complete_original_image_and_roster() {
    let (original, mut exports) = image(target::NativeTarget::macos_arm64());
    let text = original.memory.text.clone();
    let data = original.memory.data.clone();
    exports.push(exports[0].clone());
    let expected = exports.clone();
    let (returned, exports, diagnostic) = emit_macho_shared_component(original, exports)
        .unwrap_err()
        .into_parts();
    assert_eq!(returned.memory.text, text);
    assert_eq!(returned.memory.data, data);
    assert_eq!(exports, expected);
    assert!(diagnostic.message.contains("duplicate"));
}

#[test]
fn dyld_segment_immediates_refuse_out_of_range_indices() {
    let (image, _) = image(target::NativeTarget::macos_arm64());
    assert!(crate::dyld_linking::imports::macho_bind_info_for_segment(&[], 16).is_err());
    assert!(crate::dyld_linking::rebases::macho_rebase_info_for_segment(&image, 16).is_err());
}
