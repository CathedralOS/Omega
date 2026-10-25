//! Separately loadable Mach-O component containers with exact symbolic exports.
//!
//! The original FinalImage and caller-selected export roster remain owned beside
//! emitted bytes. MH_DYLIB, its install name and export trie are distinct from a
//! process MH_EXECUTE: no LC_MAIN, process entry or InstalledCode authority is
//! inferred from producing a container. Relocations, dyld bindings and signing
//! use the same format owners as ordinary Mach-O emission.
//!
//! Wire authority: Apple's mach-o/loader.h and dyld MachOAnalyzer; a dylib must
//! carry LC_ID_DYLIB, and exports name offsets relative to its preferred header.

use diagnostics::Diagnostic;
use image::{FinalImage, FinalImageSymbolHandle};

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
}

impl MachoSharedComponentImage {
    pub const fn image(&self) -> &FinalImage {
        &self.image
    }
    pub fn exports(&self) -> &[MachoComponentExport] {
        &self.exports
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

mod exports;

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

/// Emit MH_DYLIB directly from its selected function image. Failure returns the
/// complete original image and export roster, including when relocation,
/// signature construction or independent loader correspondence rejects.
pub fn emit_macho_shared_component(
    image: FinalImage,
    exports: Vec<MachoComponentExport>,
) -> Result<MachoSharedComponentImage, Box<MachoSharedComponentEmissionError>> {
    let result = (|| {
        if exports.is_empty() {
            return Err(invalid("no selected callable exports"));
        }
        selected_addresses(&image, &exports, 0)?;
        let isa = crate::isa::MachoIsa::from_format(
            image.target.object_format,
            image.target.architecture,
        )
        .ok_or_else(|| invalid("image requires an exact macOS ARM64 or x86-64 target"))?;
        if image.target != isa.native_target() {
            return Err(invalid("image target differs from its Mach-O ABI"));
        }
        let mut ordered: Vec<_> = exports.iter().collect();
        ordered.sort_unstable_by(|first, second| first.name.cmp(&second.name));
        let mut identity = optimization_core::digest_encoder::DomainDigestEncoder::new(
            b"omega.macho-component-install-name.v1",
        );
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
        // An install ID names the version-qualified export subject, while the
        // receiver loads the content-addressed filename. Putting that filename
        // inside its own hashed bytes would create a circular image digest.
        let install_name = crate::dyld_linking::load_commands::MachoDylib::from_install_name(
            format!("@rpath/lib{label}.dylib").into_bytes(),
        );
        crate::emit_macho_container_contents(
            image.clone(),
            &label,
            isa,
            Some(&install_name),
            &exports,
        )
        .map(|emitted| emitted.bytes)
    })();
    match result {
        Ok(contents) => Ok(MachoSharedComponentImage {
            image,
            exports,
            contents,
        }),
        Err(diagnostic) => Err(Box::new(MachoSharedComponentEmissionError {
            image,
            exports,
            diagnostic,
        })),
    }
}

/// The reader derives exact export names/addresses from original definitions,
/// not from a regenerated trie or a candidate symbol count. Its payload must
/// occupy the closed linkedit interval after the independently checked fixups.
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
            0x1d => {
                if signature.replace(word(command, 8)? as usize).is_some() {
                    return Err(invalid("duplicate signature command"));
                }
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
    // The writer starts every LINKEDIT blob on an eight-byte boundary, so at
    // most seven zero bytes may separate the fixup interval and the trie.
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
mod tests;
