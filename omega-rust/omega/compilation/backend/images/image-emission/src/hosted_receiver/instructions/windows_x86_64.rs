//! Independent reader for Microsoft x64 entry construction and exact pipe writes.
use super::{HostedReceiverPartitions, expected_addresses, rip_relative};
use diagnostics::Diagnostic;

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    address: u64,
    position: usize,
    failure: u64,
}
impl Reader<'_> {
    fn reject(reason: &str) -> Diagnostic {
        Diagnostic::error(format!("Windows installed-entry replay: {reason}"))
    }
    fn bytes(&mut self, expected: &[u8], reason: &str) -> Result<(), Diagnostic> {
        let end = self
            .position
            .checked_add(expected.len())
            .ok_or_else(|| Self::reject(reason))?;
        if self.bytes.get(self.position..end) != Some(expected) {
            return Err(Self::reject(reason));
        }
        self.position = end;
        Ok(())
    }
    fn pointer(&mut self, prefix: &[u8], target: u64, reason: &str) -> Result<(), Diagnostic> {
        self.bytes(prefix, reason)?;
        if rip_relative(self.bytes, self.address, self.position) != Some(target) {
            return Err(Self::reject(reason));
        }
        self.position += 4;
        Ok(())
    }
    fn failure(&mut self, condition: u8) -> Result<(), Diagnostic> {
        self.pointer(
            &[0x0f, condition],
            self.failure,
            "observation failure does not restore the original continuation",
        )
    }
    fn initialize(&mut self, original: u64, contents: &[u8]) -> Result<(), Diagnostic> {
        for (index, byte) in contents.iter().enumerate() {
            let target = original
                .checked_add(index as u64)
                .and_then(|value| value.checked_sub(1))
                .ok_or_else(|| Self::reject("initialized byte address overflow"))?;
            self.pointer(
                &[0xc6, 0x05],
                target,
                "initialization targets another residence",
            )?;
            self.bytes(&[*byte], "initialized value differs from checked recipe")?;
        }
        Ok(())
    }
}

pub(in crate::hosted_receiver) fn validate(
    bytes: &[u8],
    address: u64,
    selected_entry: u64,
    output: &image::EmittedImageOutput,
    partitions: HostedReceiverPartitions,
    recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
) -> Result<(), Diagnostic> {
    let invalid = || Reader::reject("invalid bridge partitions or import residency");
    let (saved, stack_top, receiver) =
        expected_addresses(output.final_image_layout.bss_address, partitions, &invalid)?;
    if partitions.stack_byte_count < 80 {
        return Err(invalid());
    }
    let mut imports = Vec::new();
    for export in [
        "GetEnvironmentVariableA",
        "CreateFileA",
        "WriteFile",
        "FlushFileBuffers",
        "CloseHandle",
    ] {
        let locator = target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::PeByName {
                library: b"kernel32.dll".to_vec(),
                export: export.as_bytes().to_vec(),
            },
            target::TargetProfile::WindowsX64,
        )
        .map_err(|_| invalid())?;
        let name = object_file::normalized_foreign_import_symbol_name(&locator);
        let mut matching = output.executable_regions.regions.iter().filter(|region| {
            region.origin == image::FinalExecutableRegionOrigin::ImportThunk
                && region.symbol == name
        });
        let region = matching.next().ok_or_else(invalid)?;
        if matching.next().is_some() {
            return Err(invalid());
        }
        imports.push(
            output
                .final_image_layout
                .text_address
                .checked_add(region.section_offset as u64)
                .ok_or_else(invalid)?,
        );
    }
    let failed = bytes.len().checked_sub(13).ok_or_else(invalid)?;
    let mut reader = Reader {
        bytes,
        address,
        position: 0,
        failure: address.checked_add(failed as u64).ok_or_else(invalid)?,
    };
    reader.pointer(&[0x48, 0x89, 0x25], saved, "loader stack was not preserved")?;
    reader.bytes(
        &[0x48, 0x83, 0xe4, 0xf0],
        "Win32 calls do not retain the guarded loader stack",
    )?;
    reader.bytes(
        &[0x48, 0x81, 0xec, 0, 1, 0, 0],
        "bridge stack frame is not bounded",
    )?;
    for (index, byte) in b"OMEGA_PRIVATE_OBSERVATION_PIPE\0".iter().enumerate() {
        reader.bytes(
            &[0xc6, 0x44, 0x24, 64 + index as u8, *byte],
            "private pipe variable changed",
        )?;
    }
    reader.bytes(
        &[
            0x48, 0x8d, 0x4c, 0x24, 64, 0x48, 0x8d, 0x54, 0x24, 96, 0x41, 0xb8, 128, 0, 0, 0,
        ],
        "pipe name is not bounded bridge-local storage",
    )?;
    reader.pointer(&[0xe8], imports[0], "pipe name lookup calls another import")?;
    reader.bytes(&[0x85, 0xc0], "missing pipe name accepted")?;
    reader.failure(0x84)?;
    reader.bytes(&[0x3d, 128, 0, 0, 0], "truncated pipe name accepted")?;
    reader.failure(0x83)?;
    reader.bytes(
        &[
            0x48, 0x8d, 0x4c, 0x24, 96, 0xba, 0, 0, 0, 0x40, 0x45, 0x31, 0xc0, 0x45, 0x31, 0xc9,
        ],
        "pipe open lost its private write-only mode",
    )?;
    for (offset, value) in [(32, 3u32), (40, 0), (48, 0)] {
        reader.bytes(
            &[0x48, 0xc7, 0x44, 0x24, offset],
            "pipe open stack argument changed",
        )?;
        reader.bytes(
            &value.to_le_bytes(),
            "pipe open creates or inherits unrelated storage",
        )?;
    }
    reader.pointer(&[0xe8], imports[1], "pipe open calls another import")?;
    reader.bytes(&[0x48, 0x83, 0xf8, 0xff], "invalid pipe handle accepted")?;
    reader.failure(0x84)?;
    reader.pointer(
        &[0x48, 0x89, 0x05],
        saved + 8,
        "pipe handle overwrote loader continuation",
    )?;
    reader.pointer(
        &[0x48, 0x8d, 0x25],
        stack_top,
        "callee stack is not its disjoint partition",
    )?;
    reader.bytes(
        &[0x48, 0x83, 0xec, 64],
        "semantic call lost its shadow/stack arguments",
    )?;
    let mut end = receiver;
    let mut originals = Vec::new();
    let mut headers = Vec::new();
    for recipe in recipes {
        if !recipe.alignment().is_power_of_two() {
            return Err(invalid());
        }
        let start = end
            .checked_add(recipe.alignment() - 1)
            .map(|value| value & !(recipe.alignment() - 1))
            .ok_or_else(invalid)?;
        reader.initialize(start, recipe.contents())?;
        let header = start
            .checked_add(recipe.contents().len() as u64)
            .ok_or_else(invalid)?;
        originals.push(start);
        headers.push(header);
        end = header
            .checked_add(recipe.observation_header().len() as u64)
            .ok_or_else(invalid)?;
    }
    if end
        != output
            .final_image_layout
            .bss_address
            .checked_add(partitions.end_offset)
            .ok_or_else(invalid)?
    {
        return Err(invalid());
    }
    for ordinal in 0..recipes.len() {
        let index = recipes
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
        reader.pointer(
            &prefix,
            originals[index],
            "semantic argument names another original allocation",
        )?;
        if ordinal == 4 {
            reader.bytes(
                &[0x48, 0x89, 0x44, 0x24, 32],
                "fifth argument is outside its ABI stack slot",
            )?;
        }
    }
    reader.pointer(
        &[0xe8],
        selected_entry,
        "bridge calls another semantic entry",
    )?;
    reader.pointer(
        &[0x48, 0x8b, 0x25],
        saved,
        "observer did not restore loader stack",
    )?;
    reader.bytes(
        &[0x48, 0x83, 0xe4, 0xf0, 0x48, 0x83, 0xec, 64],
        "observer Win32 calls lost their guarded stack frame",
    )?;
    for ((recipe, original), header) in recipes.iter().zip(originals).zip(headers) {
        let contents = recipe.observation_header();
        reader.initialize(header, &contents)?;
        for (source, length) in [
            (header, contents.len()),
            (original, recipe.contents().len()),
        ] {
            reader.pointer(
                &[0x48, 0x8b, 0x0d],
                saved + 8,
                "observation uses another pipe handle",
            )?;
            reader.pointer(
                &[0x48, 0x8d, 0x15],
                source,
                "observation reads another allocation",
            )?;
            reader.bytes(&[0x41, 0xb8], "observation length register changed")?;
            reader.bytes(&(length as u32).to_le_bytes(), "observation extent changed")?;
            reader.bytes(
                &[
                    0x4c, 0x8d, 0x4c, 0x24, 48, 0x48, 0xc7, 0x44, 0x24, 32, 0, 0, 0, 0,
                ],
                "write completion is not synchronous bridge-local storage",
            )?;
            reader.pointer(&[0xe8], imports[2], "observation calls another writer")?;
            reader.bytes(&[0x85, 0xc0], "failed write accepted")?;
            reader.failure(0x84)?;
            reader.bytes(&[0x81, 0x7c, 0x24, 48], "missing completed byte count")?;
            reader.bytes(&(length as u32).to_le_bytes(), "partial write accepted")?;
            reader.failure(0x85)?;
        }
    }
    for import in &imports[3..] {
        reader.pointer(
            &[0x48, 0x8b, 0x0d],
            saved + 8,
            "retirement uses another handle",
        )?;
        reader.pointer(&[0xe8], *import, "retirement calls another import")?;
        reader.bytes(&[0x85, 0xc0], "failed retirement accepted")?;
        reader.failure(0x84)?;
    }
    reader.bytes(&[0x31, 0xc0, 0xeb, 5], "normal completion is not zero")?;
    if reader.position != failed {
        return Err(invalid());
    }
    reader.bytes(
        &[0xb8, 200, 0, 0, 0],
        "observation failure reported success",
    )?;
    reader.pointer(
        &[0x48, 0x8b, 0x25],
        saved,
        "original loader stack was not restored",
    )?;
    reader.bytes(&[0xc3], "bridge does not resume its loader")?;
    if reader.position != bytes.len() {
        return Err(Reader::reject("extra instructions after bridge completion"));
    }
    Ok(())
}
