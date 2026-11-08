/*
We emit separately loadable Mach-O `MH_DYLIB` shared components with
deterministic ULEB128 export tries (`LC_DYLD_EXPORTS_TRIE`) and `LC_ID_DYLIB`
identities while replaying the emitted trie to verify that every exported
symbol resolves to its exact relocated `.text` offset.
*/

use diagnostics::Diagnostic;
use image::{FinalImage, FinalImageSymbolHandle};
use optimization_core::digest_encoder::DomainDigestEncoder;

use crate::dyld_linking::load_commands::MachoDylib;
use crate::emit_macho_container_contents;
use crate::isa::MachoIsa;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachoComponentExport {
    pub symbol: FinalImageSymbolHandle,
    pub name: Vec<u8>,
    pub slot_commitment: [u8; 32],
}

#[derive(Debug)]
pub struct MachoSharedComponentImage {
    image: FinalImage,
    exports: Vec<MachoComponentExport>,
    contents: Vec<u8>,
    final_text_bytes: Vec<u8>,
}

impl MachoSharedComponentImage {
    pub const fn image(&self) -> &FinalImage {
        &self.image
    }
    pub fn exports(&self) -> &[MachoComponentExport] {
        &self.exports
    }
    pub fn final_text_bytes(&self) -> &[u8] {
        &self.final_text_bytes
    }
    pub fn contents(&self) -> &[u8] {
        &self.contents
    }
}

#[derive(Debug)]
pub struct MachoSharedComponentEmissionError {
    image: FinalImage,
    exports: Vec<MachoComponentExport>,
    diagnostic: Diagnostic,
}

image::rejection_carrier!(
    MachoSharedComponentEmissionError;
    image: FinalImage,
    exports: Vec<MachoComponentExport>,
);

mod exports {
    use crate::dyld_linking::write_uleb128;
    use diagnostics::Diagnostic;

    #[derive(Default)]
    struct Node {
        address: Option<u64>,
        children: Vec<(u8, usize)>,
    }

    fn invalid(reason: &str) -> Diagnostic {
        Diagnostic::error(format!("Mach-O component export trie: {reason}"))
    }

    pub(super) fn encode(exports: &[(Vec<u8>, u64)]) -> Result<Vec<u8>, Diagnostic> {
        let mut nodes = vec![Node::default()];
        for (name, address) in exports {
            if name.is_empty() || name.contains(&0) {
                return Err(invalid(
                    "export name is absent or outside the supported byte extent",
                ));
            }
            let mut node = 0;
            for byte in name {
                let next = if let Some((_, index)) =
                    nodes[node].children.iter().find(|(edge, _)| edge == byte)
                {
                    *index
                } else {
                    let index = nodes.len();
                    nodes.push(Node::default());
                    nodes[node].children.push((*byte, index));
                    index
                };
                node = next;
            }
            if nodes[node].address.replace(*address).is_some() {
                return Err(invalid("duplicate exported name"));
            }
        }
        for node in &mut nodes {
            node.children.sort_unstable_by_key(|(byte, _)| *byte);
        }
        let encode_node = |node: &Node, offsets: &[usize]| -> Result<Vec<u8>, Diagnostic> {
            let mut terminal = Vec::new();
            if let Some(address) = node.address {
                write_uleb128(&mut terminal, 0); // Regular symbol, no reexport/resolver flags.
                write_uleb128(&mut terminal, address);
            }
            let mut bytes = Vec::new();
            write_uleb128(&mut bytes, terminal.len() as u64);
            bytes.extend(terminal);
            bytes.push(
                u8::try_from(node.children.len())
                    .map_err(|_| invalid("node child count overflows"))?,
            );
            for (edge, child) in &node.children {
                bytes.extend([*edge, 0]);
                write_uleb128(&mut bytes, offsets[*child] as u64);
            }
            Ok(bytes)
        };
        let mut offsets = vec![0; nodes.len()];
        for _ in 0..nodes.len().saturating_mul(10).saturating_add(1) {
            let mut next = Vec::with_capacity(nodes.len());
            let mut bytes = Vec::new();
            for node in &nodes {
                next.push(bytes.len());
                bytes.extend(encode_node(node, &offsets)?);
            }
            if next == offsets {
                return Ok(bytes);
            }
            offsets = next;
        }
        Err(invalid(
            "child-offset layout did not reach its finite fixed point",
        ))
    }

    fn unsigned(bytes: &[u8], cursor: &mut usize) -> Result<u64, Diagnostic> {
        let mut value = 0u64;
        for shift in (0..70).step_by(7) {
            let byte = *bytes
                .get(*cursor)
                .ok_or_else(|| invalid("truncated ULEB"))?;
            *cursor += 1;
            if shift == 63 && byte & 0x7e != 0 {
                return Err(invalid("ULEB overflow"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                if shift != 0 && byte == 0 {
                    return Err(invalid("noncanonical ULEB"));
                }
                return Ok(value);
            }
        }
        Err(invalid("ULEB overflow"))
    }

    pub(super) fn validate(bytes: &[u8], expected: &[(Vec<u8>, u64)]) -> Result<(), Diagnostic> {
        if bytes.is_empty() {
            return Err(invalid("missing root node"));
        }
        let mut visited = vec![false; bytes.len()];
        let mut pending = vec![(0usize, Vec::<u8>::new())];
        let mut extents = Vec::new();
        let mut actual = Vec::new();
        while let Some((offset, prefix)) = pending.pop() {
            let seen = visited
                .get_mut(offset)
                .ok_or_else(|| invalid("child lies outside trie"))?;
            if std::mem::replace(seen, true) {
                return Err(invalid("cycle or multiply-owned node"));
            }
            let mut cursor = offset;
            let terminal_size = usize::try_from(unsigned(bytes, &mut cursor)?)
                .map_err(|_| invalid("terminal extent overflows"))?;
            let terminal_end = cursor
                .checked_add(terminal_size)
                .filter(|end| *end <= bytes.len())
                .ok_or_else(|| invalid("terminal lies outside trie"))?;
            if terminal_size != 0 {
                let terminal = &bytes[..terminal_end];
                if prefix.is_empty() || unsigned(terminal, &mut cursor)? != 0 {
                    return Err(invalid("nonregular or empty export"));
                }
                let address = unsigned(terminal, &mut cursor)?;
                if cursor != terminal_end {
                    return Err(invalid("extra terminal payload"));
                }
                actual.push((prefix.clone(), address));
            }
            cursor = terminal_end;
            let count = *bytes
                .get(cursor)
                .ok_or_else(|| invalid("missing child count"))?;
            cursor += 1;
            let mut first_bytes = [false; 256];
            for _ in 0..count {
                let start = cursor;
                while *bytes
                    .get(cursor)
                    .ok_or_else(|| invalid("unterminated edge"))?
                    != 0
                {
                    cursor += 1;
                }
                if start == cursor || first_bytes[usize::from(bytes[start])] {
                    return Err(invalid("absent or ambiguous edge"));
                }
                first_bytes[usize::from(bytes[start])] = true;
                let mut name = prefix.clone();
                name.extend_from_slice(&bytes[start..cursor]);
                if name.len() > bytes.len() {
                    return Err(invalid("expanded name exceeds its byte extent"));
                }
                cursor += 1;
                let child = usize::try_from(unsigned(bytes, &mut cursor)?)
                    .map_err(|_| invalid("child offset overflows"))?;
                pending.push((child, name));
            }
            extents.push((offset, cursor));
        }
        extents.sort_unstable();
        let mut end = 0;
        for (start, next) in extents {
            if start != end {
                return Err(invalid("overlapping, unreachable or gapped node bytes"));
            }
            end = next;
        }
        if end != bytes.len() {
            return Err(invalid("unreachable trailing bytes"));
        }
        actual.sort_unstable();
        let mut expected = expected.to_vec();
        expected.sort_unstable();
        if actual != expected {
            return Err(invalid(
                "names or offsets differ from original selected exports",
            ));
        }
        Ok(())
    }
}

fn invalid(reason: &str) -> Diagnostic {
    Diagnostic::error(format!("Mach-O shared component: {reason}"))
}

fn selected_addresses(
    image: &FinalImage,
    exports: &[MachoComponentExport],
    text_offset: usize,
) -> Result<Vec<(Vec<u8>, u64)>, Diagnostic> {
    let mut addresses = Vec::with_capacity(exports.len());
    for (position, export) in exports.iter().enumerate() {
        if export.name.is_empty()
            || export.name.contains(&0)
            || export.slot_commitment == [0; 32]
            || exports[..position].iter().any(|prior| {
                prior.name == export.name || prior.slot_commitment == export.slot_commitment
            })
            || !image.symbol_table.symbols.is_valid(export.symbol)
        {
            return Err(invalid(
                "absent, duplicate or invalid selected export custody",
            ));
        }
        let symbol = image.symbol_table.symbols.get(export.symbol);
        if symbol.kind != object_file::SymbolKind::Function
            || symbol.section != image::FinalImageSection::Text
            || symbol.size == 0
            || symbol
                .offset
                .checked_add(symbol.size)
                .is_none_or(|end| end > image.memory.text.len())
        {
            return Err(invalid(
                "selected export is not a complete emitted text function",
            ));
        }
        let address = text_offset
            .checked_add(symbol.offset)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| invalid("export address overflows"))?;
        addresses.push((export.name.clone(), address));
    }
    Ok(addresses)
}

pub(super) fn export_trie(
    image: &FinalImage,
    exports: &[MachoComponentExport],
    text_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if exports.is_empty() {
        return Ok(Vec::new());
    }
    exports::encode(&selected_addresses(image, exports, text_offset)?)
}

pub fn emit_macho_shared_component(
    image: FinalImage,
    exports: Vec<MachoComponentExport>,
) -> Result<MachoSharedComponentImage, Box<MachoSharedComponentEmissionError>> {
    let result = (|| {
        if exports.is_empty() {
            return Err(invalid("no selected callable exports"));
        }
        selected_addresses(&image, &exports, 0)?;
        let isa = MachoIsa::from_format(image.target.object_format, image.target.architecture)
            .ok_or_else(|| invalid("image requires an exact macOS ARM64 or x86-64 target"))?;
        if image.target != isa.native_target() {
            return Err(invalid("image target differs from its Mach-O ABI"));
        }
        let mut ordered: Vec<_> = exports.iter().collect();
        ordered.sort_unstable_by(|first, second| first.name.cmp(&second.name));
        let mut identity = DomainDigestEncoder::new(b"omega.macho-component-install-name.v1");
        identity.bytes(isa.target_profile().target_name().as_bytes());
        for export in ordered {
            identity.bytes(&export.name);
            identity.raw(&export.slot_commitment);
        }
        let mut label = String::from("omega-component-");
        use std::fmt::Write;
        for byte in identity.finish() {
            write!(&mut label, "{byte:02x}").expect("writing into String");
        }
        let install_name =
            MachoDylib::from_install_name(format!("@rpath/lib{label}.dylib").into_bytes());
        emit_macho_container_contents(image.clone(), &label, isa, Some(&install_name), &exports)
            .map(|emitted| (emitted.bytes, emitted.image.memory.text))
    })();
    match result {
        Ok((contents, final_text_bytes)) => Ok(MachoSharedComponentImage {
            image,
            exports,
            contents,
            final_text_bytes,
        }),
        Err(diagnostic) => Err(Box::new(MachoSharedComponentEmissionError {
            image,
            exports,
            diagnostic,
        })),
    }
}

pub(super) fn validate_export_trie(
    bytes: &[u8],
    image: &FinalImage,
    exports: &[MachoComponentExport],
    text_offset: usize,
) -> Result<(), Diagnostic> {
    use crate::dyld_linking::loader_mapping::{region, wide, word};
    let commands = region(
        bytes,
        32,
        usize::try_from(word(bytes, 20)?).map_err(|_| invalid("command extent overflows"))?,
    )?;
    let mut cursor = 0usize;
    let mut selected = None;
    let mut fixup_end = None;
    let mut linkedit = None;
    let mut signature = None;
    for _ in 0..word(bytes, 16)? {
        let size = usize::try_from(word(
            commands,
            cursor
                .checked_add(4)
                .ok_or_else(|| invalid("command offset overflows"))?,
        )?)
        .map_err(|_| invalid("command size overflows"))?;
        if size < 8 || !size.is_multiple_of(8) {
            return Err(invalid("invalid command extent"));
        }
        let command = region(commands, cursor, size)?;
        match word(command, 0)? {
            0x8000_0033 => {
                if size != 16
                    || selected
                        .replace((word(command, 8)? as usize, word(command, 12)? as usize))
                        .is_some()
                {
                    return Err(invalid("duplicate or malformed export trie command"));
                }
            }
            0x8000_0022 => {
                let end = (word(command, 16)? as usize)
                    .checked_add(word(command, 20)? as usize)
                    .ok_or_else(|| invalid("fixup extent overflows"))?;
                if fixup_end.replace(end).is_some() {
                    return Err(invalid("duplicate fixup command"));
                }
            }
            0x19 if region(command, 8, 16)? == b"__LINKEDIT\0\0\0\0\0\0" => {
                if linkedit
                    .replace(
                        usize::try_from(wide(command, 40)?)
                            .map_err(|_| invalid("linkedit offset overflows"))?,
                    )
                    .is_some()
                {
                    return Err(invalid("duplicate linkedit segment"));
                }
            }
            0x1d if signature.replace(word(command, 8)? as usize).is_some() => {
                return Err(invalid("duplicate signature command"));
            }
            _ => {}
        }
        cursor = cursor
            .checked_add(size)
            .ok_or_else(|| invalid("command offset overflows"))?;
    }
    if cursor != commands.len() {
        return Err(invalid("trailing load-command bytes"));
    }
    let (offset, count) = selected.ok_or_else(|| invalid("missing export trie"))?;
    let start = fixup_end
        .or(linkedit)
        .ok_or_else(|| invalid("missing linkedit interval"))?;
    let end = offset
        .checked_add(count)
        .ok_or_else(|| invalid("export extent overflows"))?;
    let signed_end = signature.ok_or_else(|| invalid("missing signature interval"))?;
    if offset < start
        || offset - start >= 8
        || !offset.is_multiple_of(8)
        || end > signed_end
        || region(bytes, start, offset - start)?
            .iter()
            .any(|byte| *byte != 0)
        || region(bytes, end, signed_end - end)?
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(invalid(
            "export trie differs from the exact closed linkedit interval",
        ));
    }
    exports::validate(
        region(bytes, offset, count)?,
        &selected_addresses(image, exports, text_offset)?,
    )
}

#[cfg(test)]
mod tests {
    use super::{MachoComponentExport, emit_macho_shared_component};
    use image::{
        FinalImage, FinalImageMemory, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
    };
    use object_file::{RelocationKind, SymbolKind};

    fn word(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
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
        let mut image = crate::test_support::image(
            target,
            FinalImageMemory {
                text,
                data: vec![0; 13],
                bss_size: 24,
                bss_alignment: 16,
            },
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
            assert!(
                !commands.iter().any(|command| word(command, 0) == 0x19
                    && &command[8..24] == b"__PAGEZERO\0\0\0\0\0\0")
            );
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
}
