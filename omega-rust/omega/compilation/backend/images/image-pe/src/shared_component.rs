//! A Windows x86-64 shared component is the ordinary PE32+ layout with
//! `IMAGE_FILE_DLL`, no loader entry, and one export directory.
//!
//! The directory is appended to `.rdata` after the import table. Its RVAs
//! depend only on the text length and that table, so they are known before
//! section planning. The file name the loader searches for is the content
//! hash of these bytes; the directory's own name is a fixed label and is
//! not that hash. A container is not an installed component or a callable
//! address.

use diagnostics::Diagnostic;
use image::{FinalImage, FinalImageSection, FinalImageSymbolHandle};
use object_file::SymbolKind;

use crate::constants::TEXT_RVA;
use crate::{PeImageKind, emit_pe_x86_64};

const COMPONENT_DIRECTORY_NAME: &[u8] = b"omega-component.dll\0";

/// One selected export's linker spelling and original final-image definition.
/// The caller obtains this projection from a complete checked slot application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeComponentExport {
    pub symbol: FinalImageSymbolHandle,
    pub name: Vec<u8>,
    pub slot_commitment: [u8; 32],
}

#[derive(Debug)]
pub struct PeSharedComponentImage {
    image: FinalImage,
    exports: Vec<PeComponentExport>,
    contents: Vec<u8>,
}

impl PeSharedComponentImage {
    pub const fn image(&self) -> &FinalImage {
        &self.image
    }
    pub fn exports(&self) -> &[PeComponentExport] {
        &self.exports
    }
    pub fn contents(&self) -> &[u8] {
        &self.contents
    }
}

#[derive(Debug)]
pub struct PeSharedComponentEmissionError {
    image: FinalImage,
    exports: Vec<PeComponentExport>,
    diagnostic: Diagnostic,
}

image::rejection_carrier!(
    PeSharedComponentEmissionError;
    image: FinalImage,
    exports: Vec<PeComponentExport>,
);

/// Emit a separately loadable component without a process entry point.
/// Failure retains both complete original inputs.
pub fn emit_pe_shared_component(
    image: FinalImage,
    exports: Vec<PeComponentExport>,
) -> Result<PeSharedComponentImage, Box<PeSharedComponentEmissionError>> {
    if image.target != target::NativeTarget::windows_x64() {
        return Err(failure(
            image,
            exports,
            "shared component requires a Windows x86-64 PE container",
        ));
    }
    match emit_pe_x86_64(
        image.clone(),
        3,
        PeImageKind::SharedComponent {
            exports: exports.clone(),
        },
    ) {
        Ok(output) => Ok(PeSharedComponentImage {
            image,
            exports,
            contents: output.bytes,
        }),
        Err(diagnostic) => Err(Box::new(PeSharedComponentEmissionError {
            image,
            exports,
            diagnostic,
        })),
    }
}

pub(crate) fn export_directory_bytes(
    image: &FinalImage,
    exports: &[PeComponentExport],
    directory_rva: u32,
) -> Result<Vec<u8>, Diagnostic> {
    if exports.is_empty() {
        return Err(Diagnostic::error(
            "shared component has no selected exports",
        ));
    }
    let mut order: Vec<usize> = (0..exports.len()).collect();
    order.sort_by(|&left, &right| exports[left].name.cmp(&exports[right].name));
    for (position, &index) in order.iter().enumerate() {
        let export = &exports[index];
        if export.name.is_empty()
            || export.name.contains(&0)
            || export.slot_commitment == [0; 32]
            || order[..position]
                .iter()
                .any(|&prior| exports[prior].name == export.name)
            || exports
                .iter()
                .filter(|prior| prior.slot_commitment == export.slot_commitment)
                .count()
                != 1
            || !image.symbol_table.symbols.is_valid(export.symbol)
        {
            return Err(Diagnostic::error(
                "shared component has absent, duplicate or invalid export custody",
            ));
        }
        let symbol = image.symbol_table.symbols.get(export.symbol);
        if symbol.kind != SymbolKind::Function
            || symbol.section != FinalImageSection::Text
            || symbol.size == 0
            || symbol
                .offset
                .checked_add(symbol.size)
                .is_none_or(|end| end > image.memory.text.len())
        {
            return Err(Diagnostic::error(
                "component export is not a complete emitted text function",
            ));
        }
    }

    let count = u32::try_from(exports.len())
        .map_err(|_| Diagnostic::error("shared component export count overflows"))?;
    let functions_at = 40usize;
    let names_at = functions_at + exports.len() * 4;
    let ordinals_at = names_at + exports.len() * 4;
    let strings_at = ordinals_at + exports.len() * 2;
    let mut bytes = Vec::with_capacity(strings_at + COMPONENT_DIRECTORY_NAME.len());
    bytes.resize(strings_at, 0);
    let mut name_offsets = Vec::with_capacity(exports.len());
    for &index in &order {
        name_offsets.push(bytes.len());
        bytes.extend_from_slice(&exports[index].name);
        bytes.push(0);
    }
    let dll_name_at = bytes.len();
    bytes.extend_from_slice(COMPONENT_DIRECTORY_NAME);

    let functions_rva = directory_rva
        .checked_add(u32::try_from(functions_at).expect("export header fits"))
        .ok_or_else(|| Diagnostic::error("PE export function table RVA overflows"))?;
    let names_rva = directory_rva
        .checked_add(u32::try_from(names_at).expect("export header fits"))
        .ok_or_else(|| Diagnostic::error("PE export name table RVA overflows"))?;
    let ordinals_rva = directory_rva
        .checked_add(u32::try_from(ordinals_at).expect("export header fits"))
        .ok_or_else(|| Diagnostic::error("PE export ordinal table RVA overflows"))?;
    let dll_name_rva = directory_rva
        .checked_add(u32::try_from(dll_name_at).expect("export name fits"))
        .ok_or_else(|| Diagnostic::error("PE export directory name RVA overflows"))?;

    put_u32(&mut bytes, 0, 0);
    put_u32(&mut bytes, 4, 0);
    put_u16(&mut bytes, 8, 0);
    put_u16(&mut bytes, 10, 0);
    put_u32(&mut bytes, 12, dll_name_rva);
    put_u32(&mut bytes, 16, 1);
    put_u32(&mut bytes, 20, count);
    put_u32(&mut bytes, 24, count);
    put_u32(&mut bytes, 28, functions_rva);
    put_u32(&mut bytes, 32, names_rva);
    put_u32(&mut bytes, 36, ordinals_rva);

    for (sorted, &index) in order.iter().enumerate() {
        let symbol = image.symbol_table.symbols.get(exports[index].symbol);
        let function_rva = TEXT_RVA
            .checked_add(
                u32::try_from(symbol.offset)
                    .map_err(|_| Diagnostic::error("component export offset exceeds a PE RVA"))?,
            )
            .ok_or_else(|| Diagnostic::error("component export RVA overflows"))?;
        let at = functions_at + sorted * 4;
        bytes[at..at + 4].copy_from_slice(&function_rva.to_le_bytes());
        let name_rva = directory_rva
            .checked_add(u32::try_from(name_offsets[sorted]).expect("export name fits"))
            .ok_or_else(|| Diagnostic::error("PE export name RVA overflows"))?;
        let name_at = names_at + sorted * 4;
        bytes[name_at..name_at + 4].copy_from_slice(&name_rva.to_le_bytes());
        let ordinal_at = ordinals_at + sorted * 2;
        bytes[ordinal_at..ordinal_at + 2].copy_from_slice(
            &u16::try_from(sorted)
                .expect("export count fits")
                .to_le_bytes(),
        );
    }
    let _ = directory_rva
        .checked_add(
            u32::try_from(bytes.len())
                .map_err(|_| Diagnostic::error("PE export directory size overflows"))?,
        )
        .ok_or_else(|| Diagnostic::error("PE export directory end overflows"))?;
    Ok(bytes)
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn failure(
    image: FinalImage,
    exports: Vec<PeComponentExport>,
    message: &str,
) -> Box<PeSharedComponentEmissionError> {
    Box::new(PeSharedComponentEmissionError {
        image,
        exports,
        diagnostic: Diagnostic::error(message),
    })
}

#[cfg(test)]
mod tests {
    use super::{PeComponentExport, emit_pe_shared_component};
    use arena::Handle;
    use image::{FinalImage, FinalImageSection, FinalImageSymbol};
    use object_file::SymbolKind;

    #[test]
    fn emitted_component_names_its_export() {
        let (image, symbol) = answer_image();
        let exported = emit_pe_shared_component(image, vec![answer_export(symbol)])
            .expect("windows component emits");
        let bytes = exported.contents();
        assert_eq!(&bytes[0..2], b"MZ");
        assert!(
            bytes
                .windows(b"answer\0".len())
                .any(|window| window == b"answer\0")
        );
        assert!(
            bytes
                .windows(b"omega-component.dll\0".len())
                .any(|window| window == b"omega-component.dll\0")
        );
        let pe = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
        let characteristics = u16::from_le_bytes(bytes[pe + 22..pe + 24].try_into().unwrap());
        assert_eq!(characteristics & 0x2000, 0x2000);
        let entry = u32::from_le_bytes(bytes[pe + 24 + 16..pe + 24 + 20].try_into().unwrap());
        assert_eq!(entry, 0);
    }

    #[cfg(windows)]
    #[test]
    fn loaded_component_returns_its_authored_value() {
        let (image, symbol) = answer_image();
        let exported = emit_pe_shared_component(image, vec![answer_export(symbol)])
            .expect("windows component emits");
        let path =
            std::env::temp_dir().join(format!("omega_component_answer_{}.dll", std::process::id()));
        std::fs::write(&path, exported.contents()).unwrap();
        let value = call_answer(&path).expect("component export loaded");
        let _ = std::fs::remove_file(&path);
        assert_eq!(value, 42);
    }

    fn answer_export(symbol: image::FinalImageSymbolHandle) -> PeComponentExport {
        PeComponentExport {
            symbol,
            name: b"answer".to_vec(),
            slot_commitment: [9; 32],
        }
    }

    fn answer_image() -> (FinalImage, image::FinalImageSymbolHandle) {
        let mut image = FinalImage::with_capacity(
            target::NativeTarget::windows_x64(),
            image::FinalImageMemory {
                text: vec![0xb8, 0x2a, 0x00, 0x00, 0x00, 0xc3],
                ..Default::default()
            },
            Handle::invalid(),
            1,
            0,
            0,
        );
        let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "answer".into(),
            section: FinalImageSection::Text,
            offset: 0,
            size: 6,
            kind: SymbolKind::Function,
        });
        image.symbol_table.entry_symbol = symbol;
        (image, symbol)
    }

    #[cfg(windows)]
    fn call_answer(path: &std::path::Path) -> Result<u32, String> {
        unsafe extern "system" {
            fn LoadLibraryA(name: *const u8) -> *mut core::ffi::c_void;
            fn GetProcAddress(
                module: *mut core::ffi::c_void,
                name: *const u8,
            ) -> *mut core::ffi::c_void;
            fn FreeLibrary(module: *mut core::ffi::c_void) -> i32;
        }
        let mut file = path.to_string_lossy().into_owned().into_bytes();
        file.push(0);
        unsafe {
            let module = LoadLibraryA(file.as_ptr());
            if module.is_null() {
                return Err("LoadLibrary failed".into());
            }
            let answer = GetProcAddress(module, b"answer\0".as_ptr());
            if answer.is_null() {
                FreeLibrary(module);
                return Err("GetProcAddress failed".into());
            }
            let function: extern "C" fn() -> u32 = std::mem::transmute(answer);
            let value = function();
            FreeLibrary(module);
            Ok(value)
        }
    }
}
