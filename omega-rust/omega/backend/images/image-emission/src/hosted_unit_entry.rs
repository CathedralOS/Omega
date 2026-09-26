//! Physical hosted normal-completion adapters; semantic Unit stays value-free.
//! This compatibility mapping follows completed root cleanup; it is not a source
//! ProcessExit abandonment or closure of the canonical two-surface root contract.
//! Each adapter is selected by the exact target: the Darwin shim exits through
//! the hosted syscall path, while the PE shim relies on the Windows entry
//! convention where the loader maps the entry point's return value to the
//! process exit code. Each adapter discards the semantic root's residual status
//! register so a value-free Unit return publishes physical status zero.

use diagnostics::Diagnostic;
use object_file::{
    ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
};

#[derive(Clone, Copy)]
pub(super) enum EntryShim {
    DarwinReceiver {
        symbol: ObjectSymbolHandle,
        offset: usize,
    },
    LinuxReceiver {
        symbol: ObjectSymbolHandle,
        offset: usize,
    },
    LinuxArm64Receiver {
        symbol: ObjectSymbolHandle,
        offset: usize,
    },
    WindowsReceiver {
        symbol: ObjectSymbolHandle,
        offset: usize,
    },
    DarwinUnit {
        symbol: ObjectSymbolHandle,
        offset: usize,
    },
    WindowsUnit {
        symbol: ObjectSymbolHandle,
        offset: usize,
    },
    LinuxUnit {
        symbol: ObjectSymbolHandle,
        offset: usize,
    },
    LinuxArm64Unit {
        symbol: ObjectSymbolHandle,
        offset: usize,
    },
}

pub(super) struct PreparedEntry {
    pub object: ObjectPlan,
    pub text: Vec<u8>,
    pub relocations: object_file::RelocationPlan,
    pub shim: EntryShim,
}

/// One hosted Unit entry leg: the adapter's encode/decode pair, its shim
/// residency, and the format-family header probe that proves the published
/// image selects exactly this adapter. The four target legs share one
/// mechanics pipeline and differ only in their encoder bytes, their tail
/// decode, and the final header check, so the rows live behind one form
/// rather than four parallel bodies.
struct UnitEntryForm {
    target: fn() -> target::NativeTarget,
    label: &'static str,
    name: &'static str,
    byte_count: usize,
    encode: fn(usize, usize) -> Result<Vec<u8>, Diagnostic>,
    decode: fn(&[u8], usize) -> Option<usize>,
    shim: fn(ObjectSymbolHandle, usize) -> EntryShim,
    header_check: fn(&image::EmittedImageOutput, usize, &[u8]) -> bool,
    header_error: &'static str,
}

/// Select the mechanics row for the artifact's target. `NativeTarget`
/// deliberately collapses hosted Windows and UEFI x86-64 into one PE32+
/// layout. A free Unit entry carries no parameters or entry claims, so the
/// same physical completion applies on both surfaces: the hosted loader maps
/// the entry return value to the process exit code, and the EFI image entry
/// maps it to EFI_STATUS. Returning zero is success under either contract;
/// parameterized EFI arrivals are not free Unit entries.
fn unit_entry_form(target: target::NativeTarget) -> Option<UnitEntryForm> {
    Some(if target == target::NativeTarget::macos_arm64() {
        UnitEntryForm {
            target: target::NativeTarget::macos_arm64,
            label: "Darwin",
            name: "omega_darwin_unit_entry",
            byte_count: 20,
            encode,
            decode,
            shim: |symbol, offset| EntryShim::DarwinUnit { symbol, offset },
            header_check: |output, offset, _expected| main_points_to(&output.bytes, offset),
            header_error: "Mach-O LC_MAIN does not select the exact Unit entry adapter",
        }
    } else if target == target::NativeTarget::windows_x64() {
        UnitEntryForm {
            target: target::NativeTarget::windows_x64,
            label: "Windows",
            name: "omega_windows_unit_entry",
            byte_count: 16,
            encode: encode_windows,
            decode: decode_windows,
            shim: |symbol, offset| EntryShim::WindowsUnit { symbol, offset },
            header_check: |output, offset, expected| {
                pe_entry_points_to(&output.bytes, offset, expected)
            },
            header_error: "PE AddressOfEntryPoint does not select the exact Unit entry adapter",
        }
    } else if target == target::NativeTarget::linux_x64() {
        UnitEntryForm {
            target: target::NativeTarget::linux_x64,
            label: "Linux x86-64",
            name: "omega_linux_x86_64_unit_entry",
            byte_count: 19,
            encode: encode_linux_x86_64,
            decode: decode_linux_x86_64,
            shim: |symbol, offset| EntryShim::LinuxUnit { symbol, offset },
            header_check: |output, offset, expected| {
                elf_entry_points_to(
                    &output.bytes,
                    62,
                    output.final_image_layout.text_address,
                    offset,
                    expected,
                )
            },
            header_error: "ELF e_entry does not select the exact Linux x86-64 Unit entry adapter",
        }
    } else if target == target::NativeTarget::linux_arm64() {
        UnitEntryForm {
            target: target::NativeTarget::linux_arm64,
            label: "Linux ARM64",
            name: "omega_linux_arm64_unit_entry",
            byte_count: 20,
            encode: encode_linux_arm64,
            decode: decode_linux_arm64,
            shim: |symbol, offset| EntryShim::LinuxArm64Unit { symbol, offset },
            header_check: |output, offset, expected| {
                elf_entry_points_to(
                    &output.bytes,
                    183,
                    output.final_image_layout.text_address,
                    offset,
                    expected,
                )
            },
            header_error: "ELF e_entry does not select the exact Linux ARM64 Unit entry adapter",
        }
    } else {
        return None;
    })
}

pub(super) fn prepare(
    artifact: &crate::ObjectArtifact,
) -> Result<Option<PreparedEntry>, Diagnostic> {
    if artifact.hosted_receiver_binding().is_some() {
        return crate::hosted_receiver::prepare(artifact).map(Some);
    }
    if !crate::function_fragments::replay::has_free_unit_entry(artifact)? {
        return Ok(None);
    }
    let Some(form) = unit_entry_form(artifact.target) else {
        return Ok(None);
    };
    let bytes = (form.encode)(
        artifact.text_bytes.len(),
        artifact.entry_function().text_offset,
    )?;
    let (object, text, symbol, offset) = append_entry_shim(artifact, bytes, form.name)?;
    Ok(Some(PreparedEntry {
        object,
        text,
        shim: (form.shim)(symbol, offset),
        relocations: artifact.relocations.clone(),
    }))
}

/// Append `bytes` to `.text`, grow the section, and install a fresh function
/// symbol as the object entry; returns the symbol and the shim's text offset.
/// Shared by the free-Unit adapters and the hosted-receiver bridges; the
/// caller names how a missing text section reports so each surface keeps its
/// own custody diagnostic.
pub(super) fn install_entry_shim(
    object: &mut ObjectPlan,
    text: &mut Vec<u8>,
    bytes: &[u8],
    name: &str,
    missing_text_section: &dyn Fn() -> Diagnostic,
) -> Result<(ObjectSymbolHandle, usize), Diagnostic> {
    let offset = text.len();
    text.extend_from_slice(bytes);
    let section = object
        .layout
        .sections
        .iter()
        .find(|(_, row)| row.kind == SectionKind::Text)
        .map(|(handle, _)| handle)
        .ok_or_else(missing_text_section)?;
    object.layout.sections.get_mut(section).size = text.len();
    let symbol = object.layout.symbols.insert(SymbolPlan {
        name: name.into(),
        section: SymbolSection::Section(SectionKind::Text),
        offset,
        size: bytes.len(),
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = symbol;
    Ok((symbol, offset))
}

fn append_entry_shim(
    artifact: &crate::ObjectArtifact,
    bytes: Vec<u8>,
    name: &str,
) -> Result<(ObjectPlan, Vec<u8>, ObjectSymbolHandle, usize), Diagnostic> {
    let mut object = artifact.object.clone();
    let mut text = artifact.text_bytes.clone();
    let (symbol, offset) = install_entry_shim(&mut object, &mut text, &bytes, name, &|| {
        Diagnostic::error("hosted Unit entry has no text section")
    })?;
    Ok((object, text, symbol, offset))
}

/// `call entry` followed by `exit_group(0)`: the kernel supplies no return
/// continuation at `_start`, so the adapter abandons the residual status and
/// completes the process through the Linux supervisor call.
fn encode_linux_x86_64(offset: usize, destination: usize) -> Result<Vec<u8>, Diagnostic> {
    let displacement = i64::try_from(destination)
        .ok()
        .zip(
            i64::try_from(offset)
                .ok()
                .and_then(|origin| origin.checked_add(5)),
        )
        .and_then(|(destination, origin)| destination.checked_sub(origin))
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| Diagnostic::error("Linux x86-64 Unit entry call is outside rel32 range"))?;
    let mut bytes = vec![0xe8];
    bytes.extend(displacement.to_le_bytes());
    bytes.extend(isa_x86_64::encode_hosted_exit_process_i32(0));
    Ok(bytes)
}

/// AArch64 kernel-arrival adapters share one form: `bl entry` against a
/// fixed exit tail. Both Darwin and Linux supply no return continuation, so
/// only the trailing completion block differs between them.
fn encode_aarch64_call_exit(
    offset: usize,
    destination: usize,
    exit_tail: &[u8],
    adapter: &str,
) -> Result<Vec<u8>, Diagnostic> {
    if !offset.is_multiple_of(4) || !destination.is_multiple_of(4) {
        return Err(Diagnostic::error(format!(
            "{adapter} requires aligned instructions"
        )));
    }
    let displacement = i64::try_from(destination)
        .ok()
        .zip(i64::try_from(offset).ok())
        .and_then(|(destination, origin)| destination.checked_sub(origin))
        .filter(|value| value % 4 == 0 && (-134_217_728..134_217_728).contains(value))
        .ok_or_else(|| Diagnostic::error(format!("{adapter} call is outside aligned BL range")))?;
    let mut bytes: Vec<u8> = (0x9400_0000u32 | ((displacement / 4) as u32 & 0x03ff_ffff))
        .to_le_bytes()
        .to_vec();
    bytes.extend_from_slice(exit_tail);
    Ok(bytes)
}

/// Decode the shared `bl entry` + fixed exit-tail form back to the semantic
/// entry's text offset.
fn decode_aarch64_call_exit(bytes: &[u8], offset: usize, exit_tail: &[u8]) -> Option<usize> {
    if bytes.len() != 4 + exit_tail.len() || !offset.is_multiple_of(4) {
        return None;
    }
    let word = u32::from_le_bytes(bytes[..4].try_into().ok()?);
    if word & 0xfc00_0000 != 0x9400_0000 || bytes[4..] != *exit_tail {
        return None;
    }
    let displacement = ((word << 6) as i32 >> 4) as i64;
    usize::try_from(i64::try_from(offset).ok()?.checked_add(displacement)?).ok()
}

/// `bl entry` followed by `exit_group(0)` under the AArch64 Linux syscall
/// contract (`x8 = 94`); like the kernel-arrival x86-64 surface, there is no
/// return continuation to honor.
fn encode_linux_arm64(offset: usize, destination: usize) -> Result<Vec<u8>, Diagnostic> {
    let exit = isa_aarch64::encode_hosted_exit_process_i32(target::NativeTarget::linux_arm64(), 0)?;
    encode_aarch64_call_exit(offset, destination, &exit, "Linux ARM64 Unit entry")
}

/// `call entry; xor eax,eax; ret` under the ordinary Windows x64 entry ABI.
/// The loader enters with `rsp % 16 == 8`, so `sub rsp, 8` restores the
/// 16-byte call alignment the semantic entry was compiled against.
fn encode_windows(offset: usize, destination: usize) -> Result<Vec<u8>, Diagnostic> {
    let displacement = i64::try_from(destination)
        .ok()
        .zip(
            i64::try_from(offset)
                .ok()
                .and_then(|origin| origin.checked_add(9)),
        )
        .and_then(|(destination, origin)| destination.checked_sub(origin))
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| Diagnostic::error("Windows Unit entry call is outside rel32 range"))?;
    let mut bytes = vec![0x48, 0x83, 0xec, 0x08, 0xe8];
    bytes.extend(displacement.to_le_bytes());
    bytes.extend([0x31, 0xc0, 0x48, 0x83, 0xc4, 0x08, 0xc3]);
    Ok(bytes)
}

/// Darwin Unit exit tail: `movz x0, #0; movz x16, #0x30; svc #0x80; brk #0`
/// — the hosted syscall path completing the process with status zero.
const DARWIN_UNIT_EXIT_TAIL: [u8; 16] = [
    0x00, 0x00, 0x80, 0xd2, // movz x0, #0
    0x30, 0x00, 0x80, 0xd2, // movz x16, #0x30
    0x01, 0x10, 0x00, 0xd4, // svc #0x80
    0x00, 0x00, 0x20, 0xd4, // brk #0
];

fn encode(offset: usize, destination: usize) -> Result<Vec<u8>, Diagnostic> {
    // The root has settled its normal-return frontier before physical
    // completion. No extra frame or source-level process-abandonment effect
    // is introduced.
    encode_aarch64_call_exit(
        offset,
        destination,
        &DARWIN_UNIT_EXIT_TAIL,
        "Darwin Unit entry",
    )
}

fn decode(bytes: &[u8], offset: usize) -> Option<usize> {
    decode_aarch64_call_exit(bytes, offset, &DARWIN_UNIT_EXIT_TAIL)
}

fn decode_linux_x86_64(bytes: &[u8], offset: usize) -> Option<usize> {
    if bytes.len() != 19 || bytes[0] != 0xe8 {
        return None;
    }
    if bytes[5..] != *isa_x86_64::encode_hosted_exit_process_i32(0).as_slice() {
        return None;
    }
    let displacement = i64::from(i32::from_le_bytes(bytes[1..5].try_into().ok()?));
    usize::try_from(
        i64::try_from(offset)
            .ok()?
            .checked_add(5)?
            .checked_add(displacement)?,
    )
    .ok()
}

fn decode_linux_arm64(bytes: &[u8], offset: usize) -> Option<usize> {
    let exit =
        isa_aarch64::encode_hosted_exit_process_i32(target::NativeTarget::linux_arm64(), 0).ok()?;
    decode_aarch64_call_exit(bytes, offset, &exit)
}

fn decode_windows(bytes: &[u8], offset: usize) -> Option<usize> {
    if bytes.len() != 16
        || bytes[..5] != [0x48, 0x83, 0xec, 0x08, 0xe8]
        || bytes[9..] != [0x31, 0xc0, 0x48, 0x83, 0xc4, 0x08, 0xc3]
    {
        return None;
    }
    let displacement = i64::from(i32::from_le_bytes(bytes[5..9].try_into().ok()?));
    usize::try_from(
        i64::try_from(offset)
            .ok()?
            .checked_add(9)?
            .checked_add(displacement)?,
    )
    .ok()
}

pub(super) fn unique_region(
    object: &ObjectPlan,
    symbol: ObjectSymbolHandle,
    offset: usize,
    byte_count: usize,
    output: &image::EmittedImageOutput,
) -> bool {
    let name = object_file::object_symbol_name(object, symbol);
    output
        .executable_regions
        .regions
        .iter()
        .filter(|region| {
            region.origin == image::FinalExecutableRegionOrigin::CompilerFunction
                && region.symbol == name
                && region.section_offset == offset
                && region.byte_count == byte_count
        })
        .count()
        == 1
}

/// Shared Unit entry custody check: the adapter decodes back to the exact
/// semantic entry, owns one executable region, and the format header selects
/// it. Each leg only rebinds its byte width, decoder, and final header probe.
fn validate_unit_entry(
    form: &UnitEntryForm,
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    symbol: ObjectSymbolHandle,
    offset: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let end = offset
        .checked_add(form.byte_count)
        .ok_or_else(|| Diagnostic::error(format!("{} entry range overflow", form.label)))?;
    if artifact.target != (form.target)()
        || !crate::function_fragments::replay::has_free_unit_entry(artifact)?
        || offset != artifact.text_bytes.len()
        || object.layout.entry_symbol != symbol
        || text
            .get(offset..end)
            .and_then(|bytes| (form.decode)(bytes, offset))
            != Some(artifact.entry_function().text_offset)
        || output.final_text_bytes.get(offset..end) != text.get(offset..end)
    {
        return Err(Diagnostic::error(format!(
            "{} Unit entry lost exact source, call, or completion custody",
            form.label
        )));
    }
    if !unique_region(object, symbol, offset, form.byte_count, output) {
        return Err(Diagnostic::error(format!(
            "{} Unit entry has no unique executable region",
            form.label
        )));
    }
    if !(form.header_check)(output, offset, &text[offset..end]) {
        return Err(Diagnostic::error(form.header_error));
    }
    Ok(())
}

pub(super) fn validate_darwin(
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    symbol: ObjectSymbolHandle,
    offset: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    validate_unit_entry(
        &unit_entry_form(target::NativeTarget::macos_arm64())
            .ok_or_else(|| Diagnostic::error("Darwin Unit entry has no mechanics row"))?,
        artifact,
        object,
        text,
        symbol,
        offset,
        output,
    )
}

pub(super) fn validate_windows(
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    symbol: ObjectSymbolHandle,
    offset: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    validate_unit_entry(
        &unit_entry_form(target::NativeTarget::windows_x64())
            .ok_or_else(|| Diagnostic::error("Windows Unit entry has no mechanics row"))?,
        artifact,
        object,
        text,
        symbol,
        offset,
        output,
    )
}

pub(super) fn validate_linux_x86_64(
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    symbol: ObjectSymbolHandle,
    offset: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    validate_unit_entry(
        &unit_entry_form(target::NativeTarget::linux_x64())
            .ok_or_else(|| Diagnostic::error("Linux x86-64 Unit entry has no mechanics row"))?,
        artifact,
        object,
        text,
        symbol,
        offset,
        output,
    )
}

pub(super) fn validate_linux_arm64(
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    symbol: ObjectSymbolHandle,
    offset: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    validate_unit_entry(
        &unit_entry_form(target::NativeTarget::linux_arm64())
            .ok_or_else(|| Diagnostic::error("Linux ARM64 Unit entry has no mechanics row"))?,
        artifact,
        object,
        text,
        symbol,
        offset,
        output,
    )
}

/// Independently parse the emitted PE32+ headers: `AddressOfEntryPoint` must
/// select the shim inside `.text`, and the raw section bytes must carry it.
pub(super) fn pe_entry_points_to(bytes: &[u8], shim_offset: usize, expected_shim: &[u8]) -> bool {
    fn word16(bytes: &[u8], offset: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
        ))
    }
    fn word32(bytes: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
        ))
    }
    if bytes.get(0..2) != Some(b"MZ") {
        return false;
    }
    let Some(pe) = word32(bytes, 0x3c).and_then(|pe| usize::try_from(pe).ok()) else {
        return false;
    };
    let Some(signature_end) = pe.checked_add(4) else {
        return false;
    };
    if bytes.get(pe..signature_end) != Some(b"PE\0\0") {
        return false;
    }
    let Some(coff) = pe.checked_add(4) else {
        return false;
    };
    let Some(sections) = word16(bytes, coff + 2) else {
        return false;
    };
    let Some(optional_size) = word16(bytes, coff + 16) else {
        return false;
    };
    let Some(optional) = coff.checked_add(20) else {
        return false;
    };
    if word16(bytes, optional) != Some(0x20b) {
        return false;
    }
    let Some(entry_rva) = word32(bytes, optional + 16) else {
        return false;
    };
    let Some(first_section) = optional.checked_add(usize::from(optional_size)) else {
        return false;
    };
    let mut text = None;
    for index in 0..usize::from(sections) {
        let Some(header) = first_section
            .checked_add(index.saturating_mul(40))
            .filter(|header| header.checked_add(40).is_some_and(|end| end <= bytes.len()))
        else {
            return false;
        };
        if bytes.get(header..header + 8) != Some(b".text\0\0\0") {
            continue;
        }
        if text.is_some() {
            return false;
        }
        let Some(virtual_address) = word32(bytes, header + 12) else {
            return false;
        };
        let Some(raw_size) = word32(bytes, header + 16) else {
            return false;
        };
        let Some(raw_pointer) = word32(bytes, header + 20) else {
            return false;
        };
        text = Some((virtual_address, raw_size, raw_pointer));
    }
    let Some((virtual_address, raw_size, raw_pointer)) = text else {
        return false;
    };
    if u64::from(entry_rva) != u64::from(virtual_address).saturating_add(shim_offset as u64) {
        return false;
    }
    let Some(raw_start) = (raw_pointer as usize).checked_add(shim_offset) else {
        return false;
    };
    let Some(raw_end) = raw_start.checked_add(expected_shim.len()) else {
        return false;
    };
    if raw_end - (raw_pointer as usize) > raw_size as usize {
        return false;
    }
    bytes.get(raw_start..raw_end) == Some(expected_shim)
}

/// Independently parse the emitted ELF64 headers: `e_entry` must select the
/// shim inside an executable `PT_LOAD`, and the mapped file bytes must carry it.
/// Applies to both the static lane and the section-headered dynamic lane.
pub(super) fn elf_entry_points_to(
    bytes: &[u8],
    machine: u16,
    text_address: u64,
    shim_offset: usize,
    expected_shim: &[u8],
) -> bool {
    fn word16(bytes: &[u8], offset: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
        ))
    }
    fn word64(bytes: &[u8], offset: usize) -> Option<u64> {
        Some(u64::from_le_bytes(
            bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
        ))
    }
    if bytes.get(0..4) != Some(&[0x7f, b'E', b'L', b'F'])
        || bytes.get(4) != Some(&2)
        || bytes.get(5) != Some(&1)
        || word16(bytes, 16) != Some(2)
        || word16(bytes, 18) != Some(machine)
    {
        return false;
    }
    let Some(entry) = word64(bytes, 24) else {
        return false;
    };
    if entry != text_address.saturating_add(shim_offset as u64) {
        return false;
    }
    let Some(program_headers) = word64(bytes, 32).and_then(|v| usize::try_from(v).ok()) else {
        return false;
    };
    let Some(header_size) = word16(bytes, 54).map(usize::from) else {
        return false;
    };
    let Some(header_count) = word16(bytes, 56).map(usize::from) else {
        return false;
    };
    let shim_file_offset = (0..header_count).find_map(|index| {
        let header = program_headers.checked_add(index.checked_mul(header_size)?)?;
        let kind = bytes
            .get(header..header.checked_add(4)?)
            .and_then(|v| TryInto::<[u8; 4]>::try_into(v).ok())
            .map(u32::from_le_bytes)?;
        let flags = bytes
            .get(header + 4..header.checked_add(8)?)
            .and_then(|v| TryInto::<[u8; 4]>::try_into(v).ok())
            .map(u32::from_le_bytes)?;
        let segment_offset = word64(bytes, header + 8)?;
        let segment_address = word64(bytes, header + 16)?;
        let segment_file_size = word64(bytes, header + 32)?;
        if kind != 1 || flags & 1 == 0 {
            return None;
        }
        if entry < segment_address || entry >= segment_address.checked_add(segment_file_size)? {
            return None;
        }
        let file_offset = segment_offset.checked_add(entry.checked_sub(segment_address)?)?;
        usize::try_from(file_offset).ok()
    });
    let Some(shim_file_offset) = shim_file_offset else {
        return false;
    };
    let Some(shim_file_end) = shim_file_offset.checked_add(expected_shim.len()) else {
        return false;
    };
    bytes.get(shim_file_offset..shim_file_end) == Some(expected_shim)
}

pub(super) fn main_points_to(bytes: &[u8], shim_offset: usize) -> bool {
    fn word(bytes: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
        ))
    }
    fn wide(bytes: &[u8], offset: usize) -> Option<u64> {
        Some(u64::from_le_bytes(
            bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
        ))
    }
    if word(bytes, 0) != Some(0xfeed_facf) || word(bytes, 4) != Some(0x0100_000c) {
        return false;
    }
    let Some(count) = word(bytes, 16) else {
        return false;
    };
    let Some(commands_end) = word(bytes, 20)
        .and_then(|size| 32usize.checked_add(size as usize))
        .filter(|end| *end <= bytes.len())
    else {
        return false;
    };
    let mut cursor = 32usize;
    let mut entry = None;
    let mut text = None;
    for _ in 0..count {
        let Some(command) = word(bytes, cursor) else {
            return false;
        };
        let Some(size) = word(bytes, cursor + 4).and_then(|size| usize::try_from(size).ok()) else {
            return false;
        };
        let Some(end) = cursor
            .checked_add(size)
            .filter(|end| size >= 8 && size.is_multiple_of(8) && *end <= commands_end)
        else {
            return false;
        };
        if command == 0x8000_0028 {
            if entry.is_some() || size != 24 {
                return false;
            }
            entry = wide(bytes, cursor + 8);
        }
        if command == 0x19 {
            if size < 72 {
                return false;
            }
            let Some(sections) = word(bytes, cursor + 64) else {
                return false;
            };
            if (sections as usize)
                .checked_mul(80)
                .and_then(|size| size.checked_add(72))
                != Some(size)
            {
                return false;
            }
            for section in 0..sections as usize {
                let Some(section) = section
                    .checked_mul(80)
                    .and_then(|delta| cursor.checked_add(72)?.checked_add(delta))
                    .filter(|section| {
                        section
                            .checked_add(80)
                            .is_some_and(|end_section| end_section <= end)
                    })
                else {
                    return false;
                };
                if bytes.get(section..section + 16) == Some(b"__text\0\0\0\0\0\0\0\0\0\0") {
                    if text.is_some() {
                        return false;
                    }
                    text = word(bytes, section + 48).map(u64::from);
                }
            }
        }
        cursor = end;
    }
    cursor == commands_end
        && entry.is_some()
        && entry == text.and_then(|text| text.checked_add(shim_offset as u64))
}

#[cfg(test)]
mod tests {
    use super::{
        decode, decode_linux_arm64, decode_linux_x86_64, decode_windows, elf_entry_points_to,
        encode, encode_linux_arm64, encode_linux_x86_64, encode_windows, main_points_to,
        pe_entry_points_to,
    };
    #[test]
    fn darwin_main_requires_exact_adapter_offset_and_target_header() {
        let mut bytes = vec![0u8; 208];
        for (offset, word) in [
            (0, 0xfeed_facfu32),
            (4, 0x0100_000c),
            (16, 2),
            (20, 176),
            (32, 0x19),
            (36, 152),
            (96, 1),
            (152, 4096),
            (184, 0x8000_0028),
            (188, 24),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes[104..120].copy_from_slice(b"__text\0\0\0\0\0\0\0\0\0\0");
        bytes[192..200].copy_from_slice(&4160u64.to_le_bytes());
        assert!(main_points_to(&bytes, 64));
        assert!(!main_points_to(&bytes, 68));
        for offset in [0, 4, 16, 20, 36, 96, 104, 152, 184, 188, 192] {
            let mut hostile = bytes.clone();
            hostile[offset] ^= 1;
            assert!(!main_points_to(&hostile, 64), "offset {offset}");
        }
        assert!(!main_points_to(&bytes[..207], 64));
    }
    #[test]
    fn darwin_unit_entry_overwrites_residual_status_without_an_extra_frame() {
        let bytes = encode(64, 0).unwrap();
        assert_eq!(decode(&bytes, 64), Some(0));
        assert_eq!(
            &bytes[4..],
            isa_aarch64::encode_hosted_exit_process_i32(target::NativeTarget::macos_arm64(), 0)
                .unwrap()
                .as_slice()
        );
        for bit in 0..160 {
            let mut hostile = bytes.clone();
            hostile[bit / 8] ^= 1 << (bit % 8);
            assert_ne!(decode(&hostile, 64), Some(0), "bit {bit}");
        }
        assert_ne!(decode(&bytes, 68), Some(0));
        assert!(encode(1, 0).is_err());
        assert!(encode(134_217_732, 0).is_err());
        assert!(decode(&bytes[..16], 64).is_none());
    }
    #[test]
    fn windows_unit_entry_calls_entry_and_returns_zero_status() {
        let bytes = encode_windows(64, 0).unwrap();
        assert_eq!(decode_windows(&bytes, 64), Some(0));
        // x86 needs no instruction alignment; the shim may start at any offset.
        let unaligned = encode_windows(0x1f, 0).unwrap();
        assert_eq!(decode_windows(&unaligned, 0x1f), Some(0));
        for bit in 0..128 {
            let mut hostile = unaligned.clone();
            hostile[bit / 8] ^= 1 << (bit % 8);
            assert_ne!(decode_windows(&hostile, 0x1f), Some(0), "bit {bit}");
        }
        assert_ne!(decode_windows(&unaligned, 0x20), Some(0));
        assert!(decode_windows(&unaligned[..12], 0x1f).is_none());
        assert!(encode_windows(usize::MAX - 4, 0).is_err());
        assert!(encode_windows(0x1_0000_0000, 0).is_err());
    }
    #[test]
    fn windows_pe_entry_selects_the_exact_adapter_bytes() {
        let shim = encode_windows(0x20, 0).unwrap();
        let mut bytes = vec![0u8; 0x400];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        let coff = 0x84;
        bytes[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
        bytes[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes());
        let optional = coff + 20;
        bytes[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        bytes[optional + 16..optional + 20].copy_from_slice(&0x1020u32.to_le_bytes());
        let header = optional + 240;
        bytes[header..header + 8].copy_from_slice(b".text\0\0\0");
        bytes[header + 12..header + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        bytes[header + 16..header + 20].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[header + 20..header + 24].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[0x220..0x230].copy_from_slice(&shim);
        assert!(pe_entry_points_to(&bytes, 0x20, &shim));
        assert!(!pe_entry_points_to(&bytes, 0x24, &shim));
        assert!(!pe_entry_points_to(
            &bytes,
            0x20,
            &encode_windows(0x20, 4).unwrap()
        ));
        for offset in [
            0,
            0x3c,
            0x80,
            coff + 16,
            optional,
            optional + 16,
            header,
            header + 12,
            header + 20,
            0x220,
        ] {
            let mut hostile = bytes.clone();
            hostile[offset] ^= 1;
            assert!(
                !pe_entry_points_to(&hostile, 0x20, &shim),
                "offset {offset:#x}"
            );
        }
        assert!(!pe_entry_points_to(&bytes[..0x22f], 0x20, &shim));
    }
    #[test]
    fn linux_x86_64_unit_entry_calls_entry_and_exits_with_zero_status() {
        let bytes = encode_linux_x86_64(64, 0).unwrap();
        assert_eq!(decode_linux_x86_64(&bytes, 64), Some(0));
        assert_eq!(
            &bytes[5..],
            isa_x86_64::encode_hosted_exit_process_i32(0).as_slice()
        );
        let unaligned = encode_linux_x86_64(0x1f, 0).unwrap();
        assert_eq!(decode_linux_x86_64(&unaligned, 0x1f), Some(0));
        for bit in 0..152 {
            let mut hostile = unaligned.clone();
            hostile[bit / 8] ^= 1 << (bit % 8);
            assert_ne!(decode_linux_x86_64(&hostile, 0x1f), Some(0), "bit {bit}");
        }
        assert_eq!(decode_linux_x86_64(&unaligned, 0x20), Some(1));
        assert!(decode_linux_x86_64(&unaligned[..15], 0x1f).is_none());
        assert!(encode_linux_x86_64(usize::MAX - 4, 0).is_err());
        assert!(encode_linux_x86_64(0x1_0000_0000, 0).is_err());
    }
    #[test]
    fn linux_arm64_unit_entry_calls_entry_and_exits_with_zero_status() {
        let bytes = encode_linux_arm64(64, 0).unwrap();
        assert_eq!(decode_linux_arm64(&bytes, 64), Some(0));
        assert_eq!(
            &bytes[4..],
            isa_aarch64::encode_hosted_exit_process_i32(target::NativeTarget::linux_arm64(), 0)
                .unwrap()
                .as_slice()
        );
        for bit in 0..160 {
            let mut hostile = bytes.clone();
            hostile[bit / 8] ^= 1 << (bit % 8);
            assert_ne!(decode_linux_arm64(&hostile, 64), Some(0), "bit {bit}");
        }
        assert_ne!(decode_linux_arm64(&bytes, 68), Some(0));
        assert!(encode_linux_arm64(1, 0).is_err());
        assert!(encode_linux_arm64(134_217_732, 0).is_err());
        assert!(decode_linux_arm64(&bytes[..16], 64).is_none());
    }
    #[test]
    fn linux_elf_entry_selects_the_exact_adapter_bytes() {
        // Static-lane geometry: 64-byte ELF header, two 56-byte phdrs, text
        // mapped from file offset 0 at IMAGE_BASE = 0x400000.
        let shim = encode_linux_x86_64(0x140, 0).unwrap();
        let text_address = 0x401000u64;
        let mut bytes = vec![0u8; 0x2000];
        bytes[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
        bytes[24..32].copy_from_slice(&(text_address + 0x140).to_le_bytes());
        bytes[32..40].copy_from_slice(&64u64.to_le_bytes());
        bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
        bytes[56..58].copy_from_slice(&2u16.to_le_bytes());
        // RX PT_LOAD: whole file mapped at 0x400000.
        bytes[64..68].copy_from_slice(&1u32.to_le_bytes());
        bytes[68..72].copy_from_slice(&5u32.to_le_bytes());
        bytes[72..80].copy_from_slice(&0u64.to_le_bytes());
        bytes[80..88].copy_from_slice(&0x400000u64.to_le_bytes());
        bytes[96..104].copy_from_slice(&0x1200u64.to_le_bytes());
        // RW PT_LOAD for .data.
        bytes[120..124].copy_from_slice(&1u32.to_le_bytes());
        bytes[124..128].copy_from_slice(&6u32.to_le_bytes());
        bytes[128..136].copy_from_slice(&0x2000u64.to_le_bytes());
        bytes[136..144].copy_from_slice(&0x402000u64.to_le_bytes());
        bytes[152..160].copy_from_slice(&0x100u64.to_le_bytes());
        bytes[0x1140..0x1140 + 19].copy_from_slice(&shim);
        assert!(elf_entry_points_to(&bytes, 62, text_address, 0x140, &shim));
        assert!(!elf_entry_points_to(&bytes, 62, text_address, 0x144, &shim));
        assert!(!elf_entry_points_to(
            &bytes,
            183,
            text_address,
            0x140,
            &shim
        ));
        assert!(!elf_entry_points_to(
            &bytes,
            62,
            text_address,
            0x140,
            &encode_linux_x86_64(0x140, 4).unwrap()
        ));
        for offset in [0, 4, 16, 18, 24, 64, 68, 0x1140] {
            let mut hostile = bytes.clone();
            hostile[offset] ^= 1;
            assert!(
                !elf_entry_points_to(&hostile, 62, text_address, 0x140, &shim),
                "offset {offset:#x}"
            );
        }
        assert!(!elf_entry_points_to(
            &bytes[..0x1140 + 18],
            62,
            text_address,
            0x140,
            &shim
        ));
    }
}
