//! Microsoft x64 installed-entry bridge; guest streams never carry observations.
use super::{HostedReceiverBinding, ReceiverStorage, finish_receiver_shim, invalid};
use diagnostics::Diagnostic;
use object_file::{ObjectPlan, ObjectSymbolHandle};

pub(super) fn locator(export: &str) -> Result<target::NormalizedForeignLocator, Diagnostic> {
    target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: export.as_bytes().to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .map_err(|error| Diagnostic::error(format!("private entry observer import: {error}")))
}

pub(super) fn prepare(
    artifact: &crate::ObjectArtifact,
    binding: &HostedReceiverBinding,
    mut object: ObjectPlan,
    storage: ReceiverStorage,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    let invalid = || invalid(artifact.target);
    let mut imports = Vec::new();
    for name in [
        "GetEnvironmentVariableA",
        "CreateFileA",
        "WriteFile",
        "FlushFileBuffers",
        "CloseHandle",
    ] {
        let locator = locator(name)?;
        let existing = object_file::object_symbol_handle_by_foreign_locator(&object, &locator);
        let symbol = if object.layout.symbols.is_valid(existing) {
            existing
        } else {
            let symbol = object.layout.symbols.insert(object_file::SymbolPlan {
                name: object_file::normalized_foreign_import_symbol_name(&locator),
                section: object_file::SymbolSection::None,
                offset: 0,
                size: 0,
                kind: object_file::SymbolKind::Import,
                import_library: String::new(),
            });
            object
                .layout
                .normalized_imports
                .push(object_file::NormalizedImportPlan {
                    symbol,
                    locator: locator.into(),
                });
            symbol
        };
        imports.push(symbol);
    }
    let mut bytes = Vec::new();
    let mut fields = Vec::new();
    let mut failures = Vec::new();
    pointer(
        &mut bytes,
        &mut fields,
        &[0x48, 0x89, 0x25],
        storage.scratch,
        0,
    );
    bytes.extend([0x48, 0x83, 0xe4, 0xf0]); // Win32 calls retain the loader's guarded stack.
    bytes.extend([0x48, 0x81, 0xec, 0, 1, 0, 0]); // Separate bridge frame, aligned for all Win32 calls.
    for (index, byte) in b"OMEGA_PRIVATE_OBSERVATION_PIPE\0".iter().enumerate() {
        bytes.extend([0xc6, 0x44, 0x24, 64 + index as u8, *byte]);
    }
    bytes.extend([
        0x48, 0x8d, 0x4c, 0x24, 64, 0x48, 0x8d, 0x54, 0x24, 96, 0x41, 0xb8, 128, 0, 0, 0,
    ]);
    call(&mut bytes, &mut fields, imports[0]);
    bytes.extend([0x85, 0xc0]);
    failure(&mut bytes, &mut failures, 0x84);
    bytes.extend([0x3d, 128, 0, 0, 0]);
    failure(&mut bytes, &mut failures, 0x83);
    bytes.extend([
        0x48, 0x8d, 0x4c, 0x24, 96, 0xba, 0, 0, 0, 0x40, 0x45, 0x31, 0xc0, 0x45, 0x31, 0xc9,
    ]);
    for (offset, value) in [(32, 3u32), (40, 0), (48, 0)] {
        bytes.extend([0x48, 0xc7, 0x44, 0x24, offset]);
        bytes.extend(value.to_le_bytes());
    }
    call(&mut bytes, &mut fields, imports[1]);
    bytes.extend([0x48, 0x83, 0xf8, 0xff]);
    failure(&mut bytes, &mut failures, 0x84);
    pointer(
        &mut bytes,
        &mut fields,
        &[0x48, 0x89, 0x05],
        storage.scratch,
        8,
    );
    pointer(
        &mut bytes,
        &mut fields,
        &[0x48, 0x8d, 0x25],
        storage.stack_top,
        0,
    );
    bytes.extend([0x48, 0x83, 0xec, 64]); // Private semantic stack: shadow space and fifth pointer.
    for (recipe, symbol) in binding.private_entry_ram.iter().zip(&storage.private_ram) {
        initialize(&mut bytes, &mut fields, *symbol, recipe.contents());
    }
    for ordinal in 0..binding.private_entry_ram.len() {
        let index = binding
            .private_entry_ram
            .iter()
            .position(|recipe| recipe.input().runtime_position as usize == ordinal)
            .ok_or_else(invalid)?;
        let prefix = match ordinal {
            0 => [0x48, 0x8d, 0x0d],
            1 => [0x48, 0x8d, 0x15],
            2 => [0x4c, 0x8d, 0x05],
            3 => [0x4c, 0x8d, 0x0d],
            4 => [0x48, 0x8d, 0x05],
            _ => return Err(invalid()),
        };
        pointer(
            &mut bytes,
            &mut fields,
            &prefix,
            storage.private_ram[index],
            0,
        );
        if ordinal == 4 {
            bytes.extend([0x48, 0x89, 0x44, 0x24, 32]);
        }
    }
    call(&mut bytes, &mut fields, artifact.entry_function().symbol);
    pointer(
        &mut bytes,
        &mut fields,
        &[0x48, 0x8b, 0x25],
        storage.scratch,
        0,
    );
    bytes.extend([0x48, 0x83, 0xe4, 0xf0, 0x48, 0x83, 0xec, 64]);
    for ((recipe, original), header) in binding
        .private_entry_ram
        .iter()
        .zip(&storage.private_ram)
        .zip(&storage.observation_headers)
    {
        let contents = recipe.observation_header();
        initialize(&mut bytes, &mut fields, *header, &contents);
        for (symbol, length) in [
            (*header, contents.len()),
            (*original, recipe.contents().len()),
        ] {
            pointer(
                &mut bytes,
                &mut fields,
                &[0x48, 0x8b, 0x0d],
                storage.scratch,
                8,
            );
            pointer(&mut bytes, &mut fields, &[0x48, 0x8d, 0x15], symbol, 0);
            bytes.extend([0x41, 0xb8]);
            bytes.extend((length as u32).to_le_bytes());
            bytes.extend([
                0x4c, 0x8d, 0x4c, 0x24, 48, 0x48, 0xc7, 0x44, 0x24, 32, 0, 0, 0, 0,
            ]);
            call(&mut bytes, &mut fields, imports[2]);
            bytes.extend([0x85, 0xc0]);
            failure(&mut bytes, &mut failures, 0x84);
            bytes.extend([0x81, 0x7c, 0x24, 48]);
            bytes.extend((length as u32).to_le_bytes());
            failure(&mut bytes, &mut failures, 0x85);
        }
    }
    // Flush waits for the authenticated parent to consume the complete pipe before retirement.
    for symbol in &imports[3..] {
        pointer(
            &mut bytes,
            &mut fields,
            &[0x48, 0x8b, 0x0d],
            storage.scratch,
            8,
        );
        call(&mut bytes, &mut fields, *symbol);
        bytes.extend([0x85, 0xc0]);
        failure(&mut bytes, &mut failures, 0x84);
    }
    bytes.extend([0x31, 0xc0, 0xeb, 5]);
    let failed = bytes.len();
    bytes.extend([0xb8, 200, 0, 0, 0]);
    pointer(
        &mut bytes,
        &mut fields,
        &[0x48, 0x8b, 0x25],
        storage.scratch,
        0,
    );
    bytes.push(0xc3);
    for field in failures {
        let delta = i32::try_from(failed as i64 - field as i64 - 4).map_err(|_| invalid())?;
        bytes[field..field + 4].copy_from_slice(&delta.to_le_bytes());
    }
    finish_receiver_shim(
        artifact,
        object,
        &bytes,
        "omega_windows_x86_64_installed_entry_ram",
        &fields,
        |symbol, offset| {
            crate::hosted_unit_entry::EntrySurface::WindowsReceiver.shim(symbol, offset)
        },
    )
}
fn pointer(
    bytes: &mut Vec<u8>,
    fields: &mut Vec<(usize, ObjectSymbolHandle, i64)>,
    prefix: &[u8],
    symbol: ObjectSymbolHandle,
    offset: i64,
) {
    bytes.extend(prefix);
    fields.push((bytes.len(), symbol, offset));
    bytes.extend([0; 4]);
}
fn call(
    bytes: &mut Vec<u8>,
    fields: &mut Vec<(usize, ObjectSymbolHandle, i64)>,
    symbol: ObjectSymbolHandle,
) {
    pointer(bytes, fields, &[0xe8], symbol, 0);
}
fn failure(bytes: &mut Vec<u8>, failures: &mut Vec<usize>, condition: u8) {
    bytes.extend([0x0f, condition]);
    failures.push(bytes.len());
    bytes.extend([0; 4]);
}
fn initialize(
    bytes: &mut Vec<u8>,
    fields: &mut Vec<(usize, ObjectSymbolHandle, i64)>,
    symbol: ObjectSymbolHandle,
    contents: &[u8],
) {
    for (offset, byte) in contents.iter().enumerate() {
        pointer(bytes, fields, &[0xc6, 0x05], symbol, offset as i64 - 1);
        bytes.push(*byte);
    }
}
