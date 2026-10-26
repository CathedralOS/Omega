//! macOS ARM64 realization of explicitly installed private entry RAM.
//!
//! The loader continuation and the checked activation stack have distinct BSS
//! residences. Every argument is a sealed recipe's original allocation. Only
//! after normal return does the adapter observe those same bytes on FD 3.

use super::{HostedReceiverBinding, ReceiverStorage, install_entry_text, invalid};
use diagnostics::Diagnostic;
use object_file::{ObjectPlan, ObjectSymbolHandle};

pub(super) fn prepare(
    artifact: &crate::ObjectArtifact,
    binding: &HostedReceiverBinding,
    object: ObjectPlan,
    storage: ReceiverStorage,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    let invalid = || invalid(artifact.target);
    let mut words = Vec::new();
    let mut addresses = Vec::new();
    address(&mut words, &mut addresses, 9, storage.scratch);
    words.extend([0x9100_03ea, 0xa900_792a]); // SP/LR -> exact scratch.
    address(&mut words, &mut addresses, 10, storage.stack_top);
    words.push(0x9100_015f); // Switch before any callee spill.
    for (recipe, symbol) in binding.private_entry_ram.iter().zip(&storage.private_ram) {
        initialize(&mut words, &mut addresses, *symbol, recipe.contents());
    }
    for ordinal in 0..binding.private_entry_ram.len() {
        let index = binding
            .private_entry_ram
            .iter()
            .position(|recipe| recipe.input().runtime_position as usize == ordinal)
            .ok_or_else(invalid)?;
        if ordinal >= 8 {
            return Err(invalid());
        }
        address(
            &mut words,
            &mut addresses,
            ordinal as u32,
            storage.private_ram[index],
        );
    }
    let entry = artifact.entry_function();
    let start = artifact.text_bytes.len();
    let call = start
        .checked_add(words.len().checked_mul(4).ok_or_else(invalid)?)
        .ok_or_else(invalid)?;
    let displacement = entry.text_offset as i128 - call as i128;
    if !start.is_multiple_of(4)
        || displacement % 4 != 0
        || !(-134_217_728..134_217_728).contains(&displacement)
    {
        return Err(invalid());
    }
    words.push(0x9400_0000 | ((displacement / 4) as u32 & 0x03ff_ffff));
    for ((recipe, original), header) in binding
        .private_entry_ram
        .iter()
        .zip(&storage.private_ram)
        .zip(&storage.observation_headers)
    {
        let contents = recipe.observation_header();
        initialize(&mut words, &mut addresses, *header, &contents);
        for (symbol, length) in [
            (*header, contents.len()),
            (*original, recipe.contents().len()),
        ] {
            words.push(0xd280_0060); // FD 3, independent of guest streams.
            address(&mut words, &mut addresses, 1, symbol);
            unsigned_count(&mut words, 2, length)?;
            words.extend([0xd280_0090, 0xd400_1001, 0x5400_00a2]); // write; carry -> failure.
            unsigned_count(&mut words, 9, length)?;
            words.extend([0xeb09_001f, 0x5400_00a0]); // Exact completed byte count.
            words.extend([0xd280_1900, 0xd280_0030, 0xd400_1001, 0xd420_0000]); // exit(200).
        }
    }
    address(&mut words, &mut addresses, 9, storage.scratch);
    words.extend([0xa940_792a, 0x9100_015f, 0x5280_0000, 0xd65f_03c0]);
    use object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
    let bytes = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    let (object, text, symbol, offset) = install_entry_text(
        object,
        artifact,
        &bytes,
        "omega_macos_arm64_installed_entry_ram",
    )?;
    let mut relocations = artifact.relocations.clone();
    for (instruction, destination) in addresses {
        for (relative, kind) in [
            (0, RelocationKind::Aarch64Page21),
            (1, RelocationKind::Aarch64PageOffset12),
        ] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: symbol,
                },
                section: SectionKind::Text,
                offset: offset + (instruction + relative) * 4,
                byte_width: 4,
                symbol_handle: destination,
                addend: 0,
                kind,
            });
        }
    }
    Ok(crate::hosted_unit_entry::PreparedEntry {
        object,
        text,
        relocations,
        shim: crate::hosted_unit_entry::EntrySurface::DarwinReceiver.shim(symbol, offset),
    })
}

fn address(
    words: &mut Vec<u32>,
    addresses: &mut Vec<(usize, ObjectSymbolHandle)>,
    register: u32,
    symbol: ObjectSymbolHandle,
) {
    addresses.push((words.len(), symbol));
    words.extend([
        0x9000_0000 | register,
        0x9100_0000 | (register << 5) | register,
    ]);
}

fn initialize(
    words: &mut Vec<u32>,
    addresses: &mut Vec<(usize, ObjectSymbolHandle)>,
    symbol: ObjectSymbolHandle,
    bytes: &[u8],
) {
    address(words, addresses, 9, symbol);
    for byte in bytes {
        words.extend([
            0x5280_000a | (u32::from(*byte) << 5),
            0x3900_012a,
            0x9100_0529,
        ]);
    }
}

fn unsigned_count(words: &mut Vec<u32>, register: u32, count: usize) -> Result<(), Diagnostic> {
    let count = u32::try_from(count).map_err(|_| invalid(target::NativeTarget::macos_arm64()))?;
    words.extend([
        0xd280_0000 | ((count & 0xffff) << 5) | register,
        0xf2a0_0000 | ((count >> 16) << 5) | register,
    ]);
    Ok(())
}
