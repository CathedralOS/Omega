//! Check the loader's writes, not a regenerated copy of the writer's stream.
//!
//! The supported image uses eager pointer binds and internal pointer rebases.
//! Decode that closed operation language and join each write to independently
//! supplied object facts. A writable segment alone is insufficient: redirecting
//! one operation into its zero-fill tail destroys receiver initialization.
//! Exact rebases also require the preferred pointer value, not just its slot.
//!
//! This establishes conditional fixup correspondence, assuming the selected
//! loader's opcode semantics and provider resolution. It grants no provider or
//! installed-process authority. Segment mapping is checked separately.
//! The opcode meanings are the target's dyld operations:
//! https://github.com/apple-oss-distributions/dyld/blob/main/mach_o/BindOpcodes.cpp
//! and the adjacent RebaseOpcodes.cpp. Supporting another encoding requires
//! checking its writes and state changes here, not accepting unknown opcodes.

use diagnostics::Diagnostic;
use object_file::{
    ObjectPlan, RelocationKind, RelocationPlan, SectionKind, SymbolKind, SymbolSection,
};

use crate::dyld_linking::loader_mapping::{region, wide, word};
use crate::isa::MachoIsa;

/// An internal pointer reconstructed from its object relocation and symbol.
#[derive(Clone, Copy)]
pub struct MachoRebasePointer {
    pub data_offset: usize,
    pub preferred_address: u64,
}

/// One eager import slot and its exact physical locator, in slot order.
#[derive(Clone, Copy)]
pub struct MachoImportPointer<'inputs> {
    pub data_offset: usize,
    pub install_name: &'inputs [u8],
    pub symbol: &'inputs [u8],
}

/// Reconstruct fixup obligations for an AArch64 object and replay bytes.
pub fn validate_macho_aarch64_object_fixups(
    object: &ObjectPlan,
    relocations: &RelocationPlan,
    object_text_bytes: usize,
    object_data_bytes: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    validate_macho_object_fixups(
        object,
        relocations,
        object_text_bytes,
        object_data_bytes,
        output,
        MachoIsa::Aarch64,
    )
}

/// Reconstruct fixup obligations for an x86-64 object and replay bytes.
///
/// Import slot placement follows referenced object symbols, not the bind
/// stream or mutable thunk observations. Normalized locators keep their raw
/// install name and symbol; diagnostic names are used only by the legacy
/// bootstrap form.
pub fn validate_macho_x86_64_object_fixups(
    object: &ObjectPlan,
    relocations: &RelocationPlan,
    object_text_bytes: usize,
    object_data_bytes: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    validate_macho_object_fixups(
        object,
        relocations,
        object_text_bytes,
        object_data_bytes,
        output,
        MachoIsa::X86_64,
    )
}

/// Reconstruct fixup obligations from the source-free object, then replay
/// bytes under `isa`'s thunk contract.
fn validate_macho_object_fixups(
    object: &ObjectPlan,
    relocations: &RelocationPlan,
    object_text_bytes: usize,
    object_data_bytes: usize,
    output: &image::EmittedImageOutput,
    isa: MachoIsa,
) -> Result<(), Diagnostic> {
    if object.target != isa.native_target() || relocations.target != object.target {
        return Err(invalid());
    }
    let layout = output.final_image_layout;
    let referenced_imports = || {
        object.layout.symbols.iter().filter(|(handle, symbol)| {
            symbol.kind == SymbolKind::Import
                && relocations
                    .records()
                    .any(|(_, relocation)| relocation.symbol_handle == *handle)
        })
    };
    let mut rebases = Vec::new();
    for (_, relocation) in relocations.records() {
        if relocation.kind != RelocationKind::Absolute64 {
            continue;
        }
        if relocation.section != SectionKind::Data
            || relocation.byte_width != 8
            || relocation
                .offset
                .checked_add(8)
                .is_none_or(|end| end > object_data_bytes)
            || !object.layout.symbols.is_valid(relocation.symbol_handle)
        {
            return Err(invalid());
        }
        let symbol = object.layout.symbols.get(relocation.symbol_handle);
        let (base, offset) = if symbol.kind == SymbolKind::Import {
            // Taking an import's address points at its local callable thunk,
            // not at the bind slot. Derive that thunk from the same ordered
            // object import domain, independently of mutable region labels.
            let ordinal = referenced_imports()
                .position(|(handle, _)| handle == relocation.symbol_handle)
                .ok_or_else(invalid)?;
            let offset = ordinal
                .checked_mul(isa.import_thunk_size())
                .and_then(|offset| object_text_bytes.checked_add(offset))
                .ok_or_else(invalid)?;
            (layout.text_address, offset)
        } else {
            let base = match symbol.section {
                SymbolSection::Section(SectionKind::Text) => layout.text_address,
                SymbolSection::Section(SectionKind::Data) => layout.data_address,
                SymbolSection::Section(SectionKind::Bss) => layout.bss_address,
                _ => return Err(invalid()),
            };
            (base, symbol.offset)
        };
        let preferred_address = base
            .checked_add(offset as u64)
            .and_then(|address| address.checked_add_signed(relocation.addend))
            .ok_or_else(invalid)?;
        rebases.push(MachoRebasePointer {
            data_offset: relocation.offset,
            preferred_address,
        });
    }
    rebases.sort_unstable_by_key(|pointer| pointer.data_offset);
    let mut imports = Vec::new();
    let mut data_end = object_data_bytes;
    for (handle, symbol) in referenced_imports() {
        let mut locators = object
            .layout
            .normalized_imports
            .iter()
            .filter(|import| import.symbol == handle);
        let (install_name, bind_symbol) = if let Some(import) = locators.next() {
            if locators.next().is_some() || import.locator.target() != isa.target_profile() {
                return Err(invalid());
            }
            let target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name,
                symbol,
            } = import.locator.locator()
            else {
                return Err(invalid());
            };
            (install_name.as_slice(), symbol.as_slice())
        } else {
            (
                calling_conventions::darwin_import_library(&symbol.name).as_bytes(),
                symbol.name.as_bytes(),
            )
        };
        let data_offset = data_end.checked_add(7).ok_or_else(invalid)? & !7;
        data_end = data_offset.checked_add(8).ok_or_else(invalid)?;
        imports.push(MachoImportPointer {
            data_offset,
            install_name,
            symbol: bind_symbol,
        });
    }
    if data_end != output.final_data_bytes.len() {
        return Err(invalid());
    }
    validate_macho_aarch64_loader_fixups(
        &output.bytes,
        &output.final_data_bytes,
        &rebases,
        &imports,
    )
}

fn invalid() -> Diagnostic {
    Diagnostic::error("Mach-O loader fixups differ from the exact object pointer obligations")
}

struct Stream<'image> {
    remaining: &'image [u8],
}

impl<'image> Stream<'image> {
    fn byte(&mut self) -> Result<u8, Diagnostic> {
        let (byte, remaining) = self.remaining.split_first().ok_or_else(invalid)?;
        self.remaining = remaining;
        Ok(*byte)
    }

    fn require(&mut self, expected: u8) -> Result<(), Diagnostic> {
        if self.byte()? != expected {
            return Err(invalid());
        }
        Ok(())
    }

    fn unsigned(&mut self) -> Result<usize, Diagnostic> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            let payload = u64::from(byte & 0x7f);
            if payload > (u64::MAX >> shift) {
                return Err(invalid());
            }
            value |= payload << shift;
            if byte & 0x80 == 0 {
                if shift != 0 && payload == 0 {
                    return Err(invalid());
                }
                return usize::try_from(value).map_err(|_| invalid());
            }
        }
        Err(invalid())
    }

    fn terminated_bytes(&mut self) -> Result<&'image [u8], Diagnostic> {
        let length = self
            .remaining
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(invalid)?;
        let value = &self.remaining[..length];
        self.remaining = &self.remaining[length + 1..];
        Ok(value)
    }

    fn finish(mut self, has_operations: bool) -> Result<(), Diagnostic> {
        if has_operations {
            self.require(0)?;
        }
        if !self.remaining.is_empty() {
            return Err(invalid());
        }
        Ok(())
    }
}

/// Replay the writer-supported fixup language against exact expected pointers.
///
/// Rebase inputs must be strictly ordered by data offset; import inputs follow
/// increasing image-added slot order and lie beyond every rebase slot. Callers
/// reconstruct these from the object, not the untrusted opcode stream. Validate
/// loader mapping separately to bind `final_data` to the actual file and VM map.
pub fn validate_macho_aarch64_loader_fixups(
    bytes: &[u8],
    final_data: &[u8],
    rebases: &[MachoRebasePointer],
    imports: &[MachoImportPointer<'_>],
) -> Result<(), Diagnostic> {
    let commands = region(bytes, 32, word(bytes, 20)? as usize)?;
    let mut cursor = 0usize;
    let mut segment_count = 0usize;
    let mut data_segment = None;
    let mut linkedit = None;
    let mut signature = None;
    let mut dyld_info = None;
    // The emitter's immediate dylib ordinal is four bits. Keep that finite
    // roster borrowed; no image-sized parser tree or name index is needed.
    let mut dylibs: [Option<&[u8]>; 15] = [None; 15];
    let mut dylib_count = 0usize;
    for _ in 0..word(bytes, 16)? {
        let command_size = word(commands, cursor.checked_add(4).ok_or_else(invalid)?)? as usize;
        if command_size < 8 || !command_size.is_multiple_of(8) {
            return Err(invalid());
        }
        let command = region(commands, cursor, command_size)?;
        match word(command, 0)? {
            0x19 => {
                let name = region(command, 8, 16)?;
                if name == b"__DATA\0\0\0\0\0\0\0\0\0\0"
                    && data_segment.replace(segment_count).is_some()
                {
                    return Err(invalid());
                }
                if name == b"__LINKEDIT\0\0\0\0\0\0"
                    && linkedit
                        .replace((wide(command, 40)?, wide(command, 48)?))
                        .is_some()
                {
                    return Err(invalid());
                }
                segment_count += 1;
            }
            0xc => {
                if command_size < 32 || dylib_count == dylibs.len() || word(command, 8)? != 24 {
                    return Err(invalid());
                }
                let mut path = Stream {
                    remaining: region(command, 24, command_size - 24)?,
                };
                dylibs[dylib_count] = Some(path.terminated_bytes()?);
                dylib_count += 1;
            }
            0x8000_0022 => {
                if command_size != 48 || dyld_info.replace(command).is_some() {
                    return Err(invalid());
                }
            }
            0x1d => {
                if command_size != 16 || signature.is_some() {
                    return Err(invalid());
                }
                signature = Some(word(command, 8)? as usize);
            }
            _ => {}
        }
        cursor = cursor.checked_add(command_size).ok_or_else(invalid)?;
    }
    if cursor != commands.len() {
        return Err(invalid());
    }
    let Some(info) = dyld_info else {
        return if rebases.is_empty() && imports.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        };
    };
    // Weak, lazy, and export payloads would introduce unchecked loader behavior.
    if info[24..].iter().any(|byte| *byte != 0) || data_segment != Some(2) {
        return Err(invalid());
    }
    let (linkedit_offset, linkedit_size) = linkedit.ok_or_else(invalid)?;
    let linkedit_end = linkedit_offset
        .checked_add(linkedit_size)
        .ok_or_else(invalid)?;
    let rebase_offset = word(info, 8)? as usize;
    let rebase_size = word(info, 12)? as usize;
    let bind_offset = word(info, 16)? as usize;
    let bind_size = word(info, 20)? as usize;
    let fixup_end = bind_offset.checked_add(bind_size).ok_or_else(invalid)?;
    if rebase_offset as u64 != linkedit_offset
        || rebase_offset.checked_add(rebase_size) != Some(bind_offset)
        || fixup_end > signature.ok_or_else(invalid)?
        || fixup_end as u64 > linkedit_end
    {
        return Err(invalid());
    }
    let mut rebase_stream = Stream {
        remaining: region(bytes, rebase_offset, rebase_size)?,
    };
    if !rebases.is_empty() {
        rebase_stream.require(0x11)?; // SET_TYPE POINTER.
    }
    let mut previous_end = 0usize;
    for pointer in rebases {
        validate_slot(final_data, pointer.data_offset, previous_end)?;
        rebase_stream.require(0x22)?; // SET_SEGMENT DATA, then offset.
        if rebase_stream.unsigned()? != pointer.data_offset
            || wide(final_data, pointer.data_offset)? != pointer.preferred_address
        {
            return Err(invalid());
        }
        rebase_stream.require(0x51)?; // DO_REBASE once, eight-byte pointer.
        previous_end = pointer.data_offset + 8;
    }
    rebase_stream.finish(!rebases.is_empty())?;

    let mut bind_stream = Stream {
        remaining: region(bytes, bind_offset, bind_size)?,
    };
    for pointer in imports {
        validate_slot(final_data, pointer.data_offset, previous_end)?;
        let ordinal = bind_stream.byte()?;
        if ordinal & 0xf0 != 0x10 || ordinal & 0xf == 0 {
            return Err(invalid());
        }
        let install_name = dylibs[usize::from((ordinal & 0xf) - 1)].ok_or_else(invalid)?;
        bind_stream.require(0x40)?; // SET_SYMBOL, no weak/special flags.
        if install_name != pointer.install_name
            || bind_stream.terminated_bytes()? != pointer.symbol
            || pointer.symbol.is_empty()
            || wide(final_data, pointer.data_offset)? != 0
        {
            return Err(invalid());
        }
        bind_stream.require(0x51)?; // SET_TYPE POINTER.
        bind_stream.require(0x72)?; // SET_SEGMENT DATA, then offset.
        if bind_stream.unsigned()? != pointer.data_offset {
            return Err(invalid());
        }
        bind_stream.require(0x90)?; // DO_BIND once, eight-byte pointer.
        previous_end = pointer.data_offset + 8;
    }
    bind_stream.finish(!imports.is_empty())
}

fn validate_slot(bytes: &[u8], offset: usize, previous_end: usize) -> Result<(), Diagnostic> {
    if offset < previous_end || !offset.is_multiple_of(8) {
        return Err(invalid());
    }
    region(bytes, offset, 8)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Stream;

    #[test]
    fn fixup_offsets_are_bounded_and_canonical() {
        for (encoded, expected) in [(&[0][..], 0), (&[0x7f][..], 127), (&[0x80, 1][..], 128)] {
            let mut stream = Stream { remaining: encoded };
            assert_eq!(stream.unsigned().unwrap(), expected);
            assert!(stream.remaining.is_empty());
        }
        for encoded in [
            &[0x80][..],
            &[0x80, 0][..],
            &[0xff, 0][..],
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 2][..],
            &[0x80; 11][..],
        ] {
            assert!(Stream { remaining: encoded }.unsigned().is_err());
        }
    }
}
