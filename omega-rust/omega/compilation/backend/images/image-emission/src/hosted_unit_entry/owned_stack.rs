//! Free Unit roots use the same disjoint backing as installed entry receivers.
//! No runtime argument or observation recipe is manufactured. The selected
//! ordinary body's completed demand determines the private stack; the loader's
//! continuation stays on its original stack until normal completion restores it.

use super::{EntryShim, PreparedEntry, UnitEntryForm};
use crate::hosted_entry_storage::{HostedEntryPartitions, HostedEntryStorage};
use diagnostics::Diagnostic;
use object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationRecord, SectionKind,
};

fn invalid() -> Diagnostic {
    Diagnostic::error("free Unit entry lost exact stack, call, or loader-continuation custody")
}

fn call_overhead(target: target::NativeTarget) -> u64 {
    if target.architecture == target::Architecture::Aarch64 {
        0
    } else if target == target::NativeTarget::windows_x64() {
        // Microsoft caller shadow space and the CALL's return address.
        40
    } else {
        8
    }
}

pub(super) fn prepare(
    artifact: &crate::ObjectArtifact,
    form: &UnitEntryForm,
) -> Result<PreparedEntry, Diagnostic> {
    if !crate::function_fragments::replay::has_free_unit_entry(artifact)? {
        return Err(invalid());
    }
    let demand = crate::derive_stack_demand(artifact, artifact.entry)
        .map_err(|error| Diagnostic::error(format!("free Unit entry stack demand: {error}")))?;
    let (object, _, storage) = crate::hosted_entry_storage::provision(
        artifact,
        &demand,
        &[],
        call_overhead(artifact.target),
    )?;
    if artifact.target.architecture == target::Architecture::Aarch64 {
        let mut prepared =
            crate::hosted_receiver::prepare_aarch64_entry(artifact, &[], object, storage)?;
        prepared.shim.surface = form.surface;
        return Ok(prepared);
    }
    prepare_x86_64(artifact, form, object, storage)
}

fn prepare_x86_64(
    artifact: &crate::ObjectArtifact,
    form: &UnitEntryForm,
    mut object: ObjectPlan,
    storage: HostedEntryStorage,
) -> Result<PreparedEntry, Diagnostic> {
    let (bytes, pointers) = encode_x86_64(
        artifact.target,
        artifact.text_bytes.len(),
        artifact.entry_function().text_offset,
        &storage,
    )?;
    let mut text = artifact.text_bytes.clone();
    let (symbol, offset) =
        super::install_entry_shim(&mut object, &mut text, &bytes, form.name, &invalid)?;
    let mut relocations = artifact.relocations.clone();
    for (field, destination) in pointers {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: symbol,
            },
            section: SectionKind::Text,
            offset: offset + field,
            byte_width: 4,
            symbol_handle: destination,
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        });
    }
    Ok(PreparedEntry {
        object,
        text,
        relocations,
        shim: form.surface.shim(symbol, offset),
    })
}

fn encode_x86_64(
    target: target::NativeTarget,
    offset: usize,
    destination: usize,
    storage: &HostedEntryStorage,
) -> Result<(Vec<u8>, Vec<(usize, ObjectSymbolHandle)>), Diagnostic> {
    let mut bytes = Vec::new();
    let mut pointers = Vec::new();
    pointer(
        &mut bytes,
        &mut pointers,
        &[0x48, 0x89, 0x25],
        storage.scratch,
    );
    pointer(
        &mut bytes,
        &mut pointers,
        &[0x48, 0x8d, 0x25],
        storage.stack_top,
    );
    if target == target::NativeTarget::windows_x64() {
        bytes.extend([0x48, 0x83, 0xec, 0x20]);
    }
    bytes.push(0xe8);
    let after_call = offset
        .checked_add(bytes.len())
        .and_then(|n| n.checked_add(4))
        .ok_or_else(invalid)?;
    let displacement =
        i32::try_from(destination as i128 - after_call as i128).map_err(|_| invalid())?;
    bytes.extend(displacement.to_le_bytes());
    pointer(
        &mut bytes,
        &mut pointers,
        &[0x48, 0x8b, 0x25],
        storage.scratch,
    );
    if target == target::NativeTarget::linux_x64() {
        bytes.extend(isa_x86_64::encode_hosted_exit_process_i32(0));
    } else {
        bytes.extend([0x31, 0xc0, 0xc3]);
    }
    Ok((bytes, pointers))
}

fn pointer(
    bytes: &mut Vec<u8>,
    pointers: &mut Vec<(usize, ObjectSymbolHandle)>,
    opcode: &[u8],
    destination: ObjectSymbolHandle,
) {
    bytes.extend_from_slice(opcode);
    pointers.push((bytes.len(), destination));
    bytes.extend([0; 4]);
}

pub(super) fn validate(
    form: &UnitEntryForm,
    target: target::NativeTarget,
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    relocations: &object_file::RelocationPlan,
    shim: EntryShim,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let expected = prepare(artifact, form)?;
    let (symbol, offset) = shim.symbol_offset();
    if target != artifact.target
        || shim != expected.shim
        || object != &expected.object
        || text != expected.text
        || relocations != &expected.relocations
        || offset != artifact.text_bytes.len()
    {
        return Err(invalid());
    }
    let byte_count = text.len().checked_sub(offset).ok_or_else(invalid)?;
    if !super::unique_region(object, symbol, offset, byte_count, output) {
        return Err(invalid());
    }
    let bytes = output
        .final_text_bytes
        .get(offset..offset + byte_count)
        .ok_or_else(invalid)?;
    if !(form.header_check)(output, offset, bytes) {
        return Err(Diagnostic::error(form.header_error));
    }
    let demand = crate::derive_stack_demand(artifact, artifact.entry)
        .map_err(|error| Diagnostic::error(format!("free Unit entry stack demand: {error}")))?;
    let original_bss = artifact
        .object
        .layout
        .sections
        .iter()
        .filter(|(_, section)| section.kind == SectionKind::Bss)
        .try_fold(None, |previous, (_, section)| {
            if previous.is_some() {
                Err(invalid())
            } else {
                Ok(Some(section.size as u64))
            }
        })?
        .unwrap_or(0);
    let partitions = crate::hosted_entry_storage::partitions(
        target,
        &demand,
        &[],
        call_overhead(target),
        original_bss,
    )?;
    crate::hosted_entry_storage::validate_partitions(
        target,
        &demand,
        partitions,
        original_bss,
        output,
    )?;
    let address = output
        .final_image_layout
        .text_address
        .checked_add(offset as u64)
        .ok_or_else(invalid)?;
    let entry = output
        .final_image_layout
        .text_address
        .checked_add(artifact.entry_function().text_offset as u64)
        .ok_or_else(invalid)?;
    if target.architecture == target::Architecture::Aarch64 {
        crate::hosted_receiver::validate_aarch64_entry(
            target,
            bytes,
            address,
            entry,
            output.final_image_layout.bss_address,
            partitions,
            &[],
        )
    } else {
        validate_x86_64(
            target,
            bytes,
            address,
            entry,
            output.final_image_layout.bss_address,
            partitions,
        )
    }
}

/// Independent fixed-form reader: every pointer names the exact backing
/// partition, the CALL selects the semantic entry, and SP is restored before
/// returning to the loader. No writer or relocation patcher answers this query.
fn validate_x86_64(
    target: target::NativeTarget,
    bytes: &[u8],
    address: u64,
    entry: u64,
    bss: u64,
    partitions: HostedEntryPartitions,
) -> Result<(), Diagnostic> {
    let continuation = bss
        .checked_add(partitions.saved_continuation_offset)
        .ok_or_else(invalid)?;
    let stack_top = bss
        .checked_add(partitions.stack_offset)
        .and_then(|v| v.checked_add(partitions.stack_byte_count))
        .ok_or_else(invalid)?;
    let mut cursor = 0;
    read_pointer(
        bytes,
        address,
        &mut cursor,
        &[0x48, 0x89, 0x25],
        continuation,
    )?;
    read_pointer(bytes, address, &mut cursor, &[0x48, 0x8d, 0x25], stack_top)?;
    if target == target::NativeTarget::windows_x64() {
        read_exact(bytes, &mut cursor, &[0x48, 0x83, 0xec, 0x20])?;
    }
    read_pointer(bytes, address, &mut cursor, &[0xe8], entry)?;
    read_pointer(
        bytes,
        address,
        &mut cursor,
        &[0x48, 0x8b, 0x25],
        continuation,
    )?;
    let tail = if target == target::NativeTarget::linux_x64() {
        isa_x86_64::encode_hosted_exit_process_i32(0)
    } else {
        vec![0x31, 0xc0, 0xc3]
    };
    read_exact(bytes, &mut cursor, &tail)?;
    if cursor != bytes.len() {
        return Err(invalid());
    }
    Ok(())
}

fn read_exact(bytes: &[u8], cursor: &mut usize, expected: &[u8]) -> Result<(), Diagnostic> {
    let end = cursor.checked_add(expected.len()).ok_or_else(invalid)?;
    if bytes.get(*cursor..end) != Some(expected) {
        return Err(invalid());
    }
    *cursor = end;
    Ok(())
}

fn read_pointer(
    bytes: &[u8],
    address: u64,
    cursor: &mut usize,
    opcode: &[u8],
    expected: u64,
) -> Result<(), Diagnostic> {
    read_exact(bytes, cursor, opcode)?;
    let displacement = crate::codec_bytes::u32_at(bytes, *cursor).ok_or_else(invalid)? as i32;
    *cursor = cursor.checked_add(4).ok_or_else(invalid)?;
    if address
        .checked_add(*cursor as u64)
        .and_then(|v| v.checked_add_signed(i64::from(displacement)))
        != Some(expected)
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_switch_replay_rejects_changed_backing_call_restore_and_completion() {
        let mut symbols = arena::Arena::new();
        let scratch = symbols.insert(object_file::SymbolPlan::default());
        let stack_top = symbols.insert(object_file::SymbolPlan::default());
        let storage = HostedEntryStorage {
            scratch,
            stack_top,
            private_ram: Vec::new(),
            observation_headers: Vec::new(),
        };
        let address = 0x10000u64;
        let entry = 0x8000u64;
        let bss = 0x30000u64;
        let partitions = HostedEntryPartitions {
            saved_continuation_offset: 0,
            stack_offset: 16,
            stack_byte_count: 9 * 1024 * 1024,
            allocation_offset: 16 + 9 * 1024 * 1024,
            end_offset: 16 + 9 * 1024 * 1024,
        };
        for target in [
            target::NativeTarget::windows_x64(),
            target::NativeTarget::linux_x64(),
            target::NativeTarget::macos_x64(),
        ] {
            let (mut bytes, pointers) =
                encode_x86_64(target, address as usize, entry as usize, &storage).unwrap();
            for (field, symbol) in pointers {
                let destination = if symbol == scratch {
                    bss
                } else {
                    bss + partitions.allocation_offset
                };
                let displacement =
                    i32::try_from(destination as i128 - (address + field as u64 + 4) as i128)
                        .unwrap();
                bytes[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
            }
            validate_x86_64(target, &bytes, address, entry, bss, partitions).unwrap();
            for bit in 0..bytes.len() * 8 {
                let mut changed = bytes.clone();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    validate_x86_64(target, &changed, address, entry, bss, partitions).is_err(),
                    "{target:?} bit {bit}"
                );
            }
            assert!(validate_x86_64(target, &bytes, address, entry, bss + 16, partitions).is_err());
            assert!(validate_x86_64(target, &bytes, address, entry + 4, bss, partitions).is_err());
            assert!(
                validate_x86_64(
                    target,
                    &bytes[..bytes.len() - 1],
                    address,
                    entry,
                    bss,
                    partitions
                )
                .is_err()
            );
        }
    }
}
