//! The bounded native evidence a `.proof` sidecar carries for the Native
//! product: the placed-image section.
//!
//! `wiki/spec/build/machine_state_evidence.md` describes the self-describing
//! certificate a native admission replays: exact final bytes, placements and
//! the complete executable-region inventory, then normalized instruction rows
//! against closed target instruction specifications, then the composed
//! footprint. This section carries the certificate's first leg plus the
//! import-thunk leg of the second — the declared executable-text,
//! initialized-data and import-data extents inside the published container,
//! the complete [`image::PlacedExecutableRegionInventory`] over the text, the
//! complete [`image::PlacedDataRegionInventory`] over each data extent, and
//! the closed-form realization of every claimed import thunk.
//!
//! The import-data extent is the writer-owned read-only import table a
//! container carries outside `image.memory.data` — the PE `.rdata` section
//! holding the import address table each thunk's memory operand names. Its
//! placed rows are all `ImportBindingSlot`; descriptors, lookup tables and
//! name bytes sit in unclassified custody gaps rather than pretending to be
//! compiler data. Containers without such an extent declare the canonical
//! empty inventory.
//!
//! What this evidence honestly establishes on its own: every row the section
//! claims about the published bytes is *true of those exact bytes*. The
//! receiver slices every declared extent out of the artifact it holds,
//! recomputes every commitment, and replays
//! [`image::validate_placed_executable_region_inventory`] and
//! [`image::validate_placed_data_region_inventory`] — region and gap digests,
//! addresses, fingerprints and both inventory seals are all recomputed
//! against the real bytes, so a row that lies about the artifact rejects.
//! Producer digests are therefore not the claim: they are annotations the
//! checker verifies.
//!
//! Rows that name an `ImportThunk` are additionally bound to the closed thunk
//! form the declared target realizes: only (x86_64, Coff) `jmp [rip+disp32]`
//! and (aarch64, MachO) `ADRP/LDR/BR X16` sequences are thunk claims at all,
//! decode requires the claimed extent and footprint to equal the closed
//! form's, and replay re-derives the thunk opcodes from the committed bytes —
//! on aarch64 Mach-O through
//! [`image_macho::validate_macho_aarch64_import_binding_pairing`], which also
//! re-decodes each thunk's bound pointer address and requires it to name
//! exactly one committed import-binding slot, and on x86-64 Coff through
//! [`image_pe::validate_pe_x86_64_import_binding_pairing`], which decodes
//! each `jmp [rip+disp32]` to the absolute slot address it loads through and
//! requires exactly one placed `ImportBindingSlot` row at that address
//! inside the import-data extent. That is the first leg that checks what the
//! bytes *do* rather than only where they sit: the indirect target of every
//! call through an import is verified, not trusted.
//!
//! The container's own entry declaration is custody of the same kind:
//! ELF64 `e_entry`, PE32+ `ImageBase + AddressOfEntryPoint`, and Mach-O 64
//! `LC_MAIN`'s entry offset mapped through `__TEXT` are re-derived from the
//! committed bytes alone, and the declared entry virtual address must equal
//! the start address of some placed executable region. A container whose
//! loader-visible entry does not land on a checked instruction boundary is
//! a claim about control flow the bytes do not carry. A container that does
//! not parse as the declared format stays silent here — its extents and
//! thunk legs already bind whatever bytes exist; inventing an entry claim
//! would only shadow them.
//!
//! The declared extents are custody claims the container's own loadable map
//! must ratify: no two declared extents may cover the same bytes, the text
//! extent must sit inside a file range the container marks loadable and
//! executable, the initialized-data extent inside one it marks writable,
//! and an import-data extent inside some loadable range at all — ELF64
//! `PT_LOAD` flags, PE32+ section characteristics, Mach-O 64 `LC_SEGMENT_64`
//! protections. A sidecar that declared its inventory over bytes the loader
//! maps under a different role would verify byte-for-byte while checking
//! nothing the loader executes; the container's own declarations close
//! that gap without trusting a producer annotation.
//!
//! What the section still does not establish is everything beyond this:
//! instruction-row semantics inside compiler-function regions, the edges
//! between instruction rows, premise availability and lowering
//! correspondence all still need the native semantic and certification
//! owners. The product leg keeps reporting `Incomplete` for
//! them rather than letting coverage stand in for behavior, which is the
//! failure the deleted custody scheme committed.
//!
//! The encoding is canonical: decode requires the recognized magic and
//! version, ordered rows, bounded collections, targets that cannot realize
//! the claims they would carry, and a byte-identical re-encode. An
//! unrecognized section is `Unsupported` (the product leg reports
//! `Incomplete`); a recognized-but-malformed or byte-mismatched section is
//! `Malformed`/rejected (the product leg reports `Reject`).

use calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};

/// Magic identifying the placed-image evidence section inside the sidecar's
/// opaque evidence field. An unknown magic or version is an unsupported
/// section, never a malformed one.
const EVIDENCE_MAGIC: &[u8; 8] = b"NPLCIMG1";
/// Section version 3 adds the placed import-data inventory — the writer-owned
/// `.rdata` extent carrying each Coff thunk's import-binding slot — over the
/// version-2 text and data inventories and closed thunk binding.
const EVIDENCE_VERSION: u16 = 3;

/// Hard caps so a hostile section cannot make the decoder allocate without
/// bound. Real inventories are small; these bounds are generous, not tight.
const MAX_INVENTORY_ROWS: u64 = 1 << 20;
const MAX_FOOTPRINT_REGISTERS: u64 = 1 << 10;

/// How a native evidence section fails to decode. The product leg maps
/// `Unsupported` to `Incomplete` (the checker cannot interpret what was
/// offered) and `Malformed` to `Reject` (the offered evidence is invalid).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeEvidenceError {
    Unsupported,
    Malformed(String),
}

/// The placed-image evidence section: the producer's declared executable-text,
/// initialized-data and import-data extents inside the published bytes, the
/// target the bytes were realized for, and the complete placed inventories
/// over all three extents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlacedImageEvidence {
    target: target::NativeTarget,
    /// Byte offset inside the published artifact at which the exact final
    /// `.text` bytes begin. This is a producer annotation the checker
    /// validates against the committed bytes, not a trusted locator.
    text_file_offset: u64,
    inventory: image::PlacedExecutableRegionInventory,
    /// Byte offset inside the published artifact at which the exact final
    /// initialized-data bytes begin. The same producer-annotation standing as
    /// `text_file_offset`; it is `0` when the image carries no data.
    data_file_offset: u64,
    data_inventory: image::PlacedDataRegionInventory,
    /// Byte offset inside the published artifact at which the exact final
    /// writer-owned import-table bytes begin (the PE `.rdata` section). The
    /// same producer-annotation standing as `text_file_offset`; it is `0`
    /// when the container carries no separate import-data extent.
    import_data_file_offset: u64,
    import_data_inventory: image::PlacedDataRegionInventory,
}

/// Locate `extent` inside the published container exactly once. A missing or
/// repeated run means the producer cannot make a checkable claim about it,
/// which is a construction failure rather than evidence. An empty extent has
/// no position to commit to and is recorded at offset 0.
fn locate_unique_extent(
    extent: &[u8],
    container: &[u8],
    what: &'static str,
) -> Result<u64, String> {
    if extent.is_empty() {
        return Ok(0);
    }
    if extent.len() > container.len() {
        return Err(format!("{what} is larger than the published container"));
    }
    let mut hits = container
        .windows(extent.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == extent).then_some(offset));
    let offset = hits
        .next()
        .ok_or_else(|| format!("cannot locate {what} inside the published container bytes"))?;
    if hits.next().is_some() {
        return Err(format!(
            "{what} occurs more than once in the published container bytes"
        ));
    }
    Ok(offset as u64)
}

/// Slice the declared `[offset, offset + byte_count)` extent out of the
/// published artifact. An extent that overflows or lies outside the container
/// is a claim about bytes the artifact does not contain.
fn declared_extent<'a>(
    executable_bytes: &'a [u8],
    file_offset: u64,
    byte_count: u64,
    what: &'static str,
) -> Result<&'a [u8], String> {
    let end = file_offset
        .checked_add(byte_count)
        .ok_or_else(|| format!("declared {what} extent overflows"))?;
    let end =
        usize::try_from(end).map_err(|_| format!("declared {what} extent does not fit usize"))?;
    let start = usize::try_from(file_offset)
        .map_err(|_| format!("declared {what} offset does not fit usize"))?;
    executable_bytes.get(start..end).ok_or_else(|| {
        format!(
            "declared {what} extent [{start}..{end}) lies outside the {} published bytes",
            executable_bytes.len()
        )
    })
}

/// The closed import-thunk form a declared target realizes, when it has one:
/// the fixed byte extent and the exact machine-state footprint of the emitted
/// thunk sequence. `(x86_64, Coff)` realizes `jmp [rip+disp32]` — the ISA
/// crate owns that form's extent and footprint; `(aarch64, MachO)` realizes
/// the twelve-byte `ADRP X16, page; LDR X16, [X16, #imm]; BR X16` — X16 as
/// sole scratch plus the instruction pointer. Any other declared target emits
/// no thunk regions, so a row claiming one is not a checkable claim.
fn import_thunk_form(target: target::NativeTarget) -> Option<(usize, StateFootprintEvidence)> {
    match (target.architecture, target.object_format) {
        (target::Architecture::X86_64, target::ObjectFormat::Coff) => Some((
            isa_x86_64::X86_64_IMPORT_THUNK_BYTE_COUNT,
            isa_x86_64::x86_64_import_thunk_footprint(),
        )),
        (target::Architecture::Aarch64, target::ObjectFormat::MachO) => Some((
            12,
            StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::Aarch64X(16)]),
                MachineStateSet::new([MachineState::InstructionPointer]),
            ),
        )),
        _ => None,
    }
}

/// Little-endian readers over the published container — the container's own
/// declared fields, not evidence wire bytes. Reads are bounds-checked
/// arithmetic-free: a hostile header can only end the leg, never panic it.
fn container_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn container_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn container_u64(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

/// ELF64 little-endian: `e_entry` at header offset 24 is the loader-visible
/// entry virtual address. A null entry declares none.
fn elf64_entry_address(bytes: &[u8]) -> Option<u64> {
    let identification = bytes.get(..16)?;
    if identification[..4] != [0x7f, b'E', b'L', b'F']
        || identification[4] != 2
        || identification[5] != 1
    {
        return None;
    }
    let entry = container_u64(bytes, 24)?;
    (entry != 0).then_some(entry)
}

/// PE32+: the MZ stub's `e_lfanew` names the PE signature, the optional
/// header carries `AddressOfEntryPoint` as an RVA the loader relocates by
/// `ImageBase`. A null RVA declares no entry.
fn pe32_plus_entry_address(bytes: &[u8]) -> Option<u64> {
    if bytes.get(..2)? != *b"MZ" {
        return None;
    }
    let pe_offset = usize::try_from(container_u32(bytes, 0x3c)?).ok()?;
    if bytes.get(pe_offset..pe_offset.checked_add(4)?)? != *b"PE\0\0" {
        return None;
    }
    let optional = pe_offset.checked_add(24)?;
    if container_u16(bytes, optional)? != 0x20b {
        return None;
    }
    let entry_rva = container_u32(bytes, optional.checked_add(16)?)?;
    if entry_rva == 0 {
        return None;
    }
    let image_base = container_u64(bytes, optional.checked_add(24)?)?;
    image_base.checked_add(u64::from(entry_rva))
}

/// Mach-O 64 little-endian: `LC_MAIN` carries the entry's file offset, which
/// the `__TEXT` segment's `vmaddr`/`fileoff` pair maps to a virtual address.
/// A container with no `LC_MAIN` or no `__TEXT` segment declares no entry
/// claim this leg can check.
fn macho64_entry_address(bytes: &[u8]) -> Option<u64> {
    const LC_SEGMENT_64: u32 = 0x19;
    const LC_MAIN: u32 = 0x8000_0028;
    if container_u32(bytes, 0)? != 0xfeed_facf {
        return None;
    }
    let command_count = usize::try_from(container_u32(bytes, 16)?).ok()?;
    let mut command_offset = 32usize;
    let mut text_segment = None;
    let mut entry_file_offset = None;
    for _ in 0..command_count {
        let command = container_u32(bytes, command_offset)?;
        let command_size =
            usize::try_from(container_u32(bytes, command_offset.checked_add(4)?)?).ok()?;
        if command_size < 8 {
            return None;
        }
        match command {
            LC_SEGMENT_64 => {
                if bytes.get(command_offset.checked_add(8)?..command_offset.checked_add(24)?)?
                    == b"__TEXT\0\0\0\0\0\0\0\0\0\0"
                {
                    let vmaddr = container_u64(bytes, command_offset.checked_add(24)?)?;
                    let file_offset = container_u64(bytes, command_offset.checked_add(40)?)?;
                    text_segment = Some((vmaddr, file_offset));
                }
            }
            LC_MAIN => {
                entry_file_offset = Some(container_u64(bytes, command_offset.checked_add(8)?)?);
            }
            _ => {}
        }
        command_offset = command_offset.checked_add(command_size)?;
    }
    let (vmaddr, file_offset) = text_segment?;
    let in_segment = entry_file_offset?.checked_sub(file_offset)?;
    vmaddr.checked_add(in_segment)
}

/// The entry point the published container declares to its loader,
/// re-derived from the artifact bytes alone for the declared object format.
/// `None` means the bytes carry no checkable entry claim — a container that
/// does not parse as the declared format fails nothing new here, since every
/// other leg still binds whatever bytes sit under the declared extents.
fn declared_entry_address(target: target::NativeTarget, executable_bytes: &[u8]) -> Option<u64> {
    match target.object_format {
        target::ObjectFormat::Elf => elf64_entry_address(executable_bytes),
        target::ObjectFormat::Coff => pe32_plus_entry_address(executable_bytes),
        target::ObjectFormat::MachO => macho64_entry_address(executable_bytes),
    }
}

/// A loadable file range the published container itself declares to its
/// loader, with the access roles that range grants. The roles are the
/// container's own claims — the evidence only checks that its declared
/// extents sit inside ranges carrying the role the extent asserts.
struct ContainerLoadableRange {
    start: u64,
    end: u64,
    executable: bool,
    writable: bool,
}

/// ELF64 little-endian: every `PT_LOAD` program header maps
/// `[p_offset, p_offset + p_filesz)` with `p_flags` access bits (PF_X = 1,
/// PF_W = 2). A container with no `PT_LOAD` entries declares no loadable
/// claim this leg can check.
fn elf64_loadable_ranges(bytes: &[u8]) -> Option<Vec<ContainerLoadableRange>> {
    let identification = bytes.get(..16)?;
    if identification[..4] != [0x7f, b'E', b'L', b'F']
        || identification[4] != 2
        || identification[5] != 1
    {
        return None;
    }
    let header_offset = usize::try_from(container_u64(bytes, 32)?).ok()?;
    let entry_size = usize::from(container_u16(bytes, 54)?);
    if entry_size < 56 {
        return None;
    }
    let count = usize::from(container_u16(bytes, 56)?);
    let mut ranges = Vec::new();
    for index in 0..count {
        let entry = header_offset.checked_add(index.checked_mul(entry_size)?)?;
        if container_u32(bytes, entry)? != 1 {
            continue;
        }
        let flags = container_u32(bytes, entry.checked_add(4)?)?;
        let start = container_u64(bytes, entry.checked_add(8)?)?;
        let end = start.checked_add(container_u64(bytes, entry.checked_add(32)?)?)?;
        if end == start {
            continue;
        }
        ranges.push(ContainerLoadableRange {
            start,
            end,
            executable: flags & 1 != 0,
            writable: flags & 2 != 0,
        });
    }
    Some(ranges)
}

/// PE32+: every section header declares the raw file range it maps
/// (`PointerToRawData`, `SizeOfRawData`) under COFF characteristics
/// (`IMAGE_SCN_MEM_EXECUTE` = 0x2000_0000, `IMAGE_SCN_MEM_WRITE` =
/// 0x8000_0000). A container with no sections carrying raw bytes declares
/// no loadable claim this leg can check.
fn pe32_plus_loadable_ranges(bytes: &[u8]) -> Option<Vec<ContainerLoadableRange>> {
    if bytes.get(..2)? != *b"MZ" {
        return None;
    }
    let pe_offset = usize::try_from(container_u32(bytes, 0x3c)?).ok()?;
    if bytes.get(pe_offset..pe_offset.checked_add(4)?)? != *b"PE\0\0" {
        return None;
    }
    let coff = pe_offset.checked_add(4)?;
    let section_count = usize::from(container_u16(bytes, coff.checked_add(2)?)?);
    let optional_size = usize::from(container_u16(bytes, coff.checked_add(16)?)?);
    let mut section = coff.checked_add(20)?.checked_add(optional_size)?;
    let mut ranges = Vec::new();
    for _ in 0..section_count {
        let raw_size = container_u32(bytes, section.checked_add(16)?)?;
        let start = u64::from(container_u32(bytes, section.checked_add(20)?)?);
        let characteristics = container_u32(bytes, section.checked_add(36)?)?;
        section = section.checked_add(40)?;
        if raw_size == 0 {
            continue;
        }
        let end = start.checked_add(u64::from(raw_size))?;
        ranges.push(ContainerLoadableRange {
            start,
            end,
            executable: characteristics & 0x2000_0000 != 0,
            writable: characteristics & 0x8000_0000 != 0,
        });
    }
    Some(ranges)
}

/// Mach-O 64 little-endian: every `LC_SEGMENT_64` maps `[fileoff, fileoff +
/// filesize)` under its `initprot` bits (VM_PROT_EXECUTE = 4,
/// VM_PROT_WRITE = 2). A container declaring no mapped segment file range
/// carries no loadable claim this leg can check.
fn macho64_loadable_ranges(bytes: &[u8]) -> Option<Vec<ContainerLoadableRange>> {
    const LC_SEGMENT_64: u32 = 0x19;
    if container_u32(bytes, 0)? != 0xfeed_facf {
        return None;
    }
    let command_count = usize::try_from(container_u32(bytes, 16)?).ok()?;
    let mut command_offset = 32usize;
    let mut ranges = Vec::new();
    for _ in 0..command_count {
        let command = container_u32(bytes, command_offset)?;
        let command_size =
            usize::try_from(container_u32(bytes, command_offset.checked_add(4)?)?).ok()?;
        if command_size < 8 {
            return None;
        }
        if command == LC_SEGMENT_64 {
            let start = container_u64(bytes, command_offset.checked_add(40)?)?;
            let end = start.checked_add(container_u64(bytes, command_offset.checked_add(48)?)?)?;
            let initial_protection = container_u32(bytes, command_offset.checked_add(60)?)?;
            if end != start {
                ranges.push(ContainerLoadableRange {
                    start,
                    end,
                    executable: initial_protection & 4 != 0,
                    writable: initial_protection & 2 != 0,
                });
            }
        }
        command_offset = command_offset.checked_add(command_size)?;
    }
    Some(ranges)
}

/// The loadable file ranges the published container declares to its loader,
/// re-derived from the artifact bytes alone for the declared object format.
/// `None` means the bytes carry no checkable loadable claim — a container
/// that does not parse as the declared format fails nothing new here, since
/// every other leg still binds whatever bytes sit under the declared
/// extents.
fn declared_loadable_ranges(
    target: target::NativeTarget,
    executable_bytes: &[u8],
) -> Option<Vec<ContainerLoadableRange>> {
    match target.object_format {
        target::ObjectFormat::Elf => elf64_loadable_ranges(executable_bytes),
        target::ObjectFormat::Coff => pe32_plus_loadable_ranges(executable_bytes),
        target::ObjectFormat::MachO => macho64_loadable_ranges(executable_bytes),
    }
}

/// A declared extent as a file range, `None` for the canonical empty extent.
fn declared_extent_range(file_offset: u64, byte_count: u64) -> Option<(u64, u64)> {
    let end = file_offset.checked_add(byte_count)?;
    (byte_count != 0).then_some((file_offset, end))
}

impl NativePlacedImageEvidence {
    /// Capture the placed-image evidence for one retained native artifact and
    /// the exact bytes about to be published. The final `.text` and
    /// initialized-data runs must be located inside the container
    /// unambiguously: a missing or repeated run means the producer cannot
    /// make a checkable claim about it, which is a construction failure
    /// rather than evidence.
    pub fn from_artifact(
        artifact: &crate::RetainedNativeArtifact,
        executable_bytes: &[u8],
    ) -> Result<Self, String> {
        let output = artifact.image().output();
        let text = output.final_text_bytes.as_slice();
        if text.is_empty() {
            return Err(
                "native PCC evidence requires a non-empty final executable text".to_owned(),
            );
        }
        let text_file_offset =
            locate_unique_extent(text, executable_bytes, "the final executable text")?;
        let data_file_offset = locate_unique_extent(
            output.final_data_bytes.as_slice(),
            executable_bytes,
            "the final initialized data",
        )?;
        let import_data_file_offset = locate_unique_extent(
            output.final_import_data_bytes.as_slice(),
            executable_bytes,
            "the final import data",
        )?;
        Ok(Self {
            target: artifact.target(),
            text_file_offset,
            inventory: output.executable_regions.clone(),
            data_file_offset,
            data_inventory: output.data_regions.clone(),
            import_data_file_offset,
            import_data_inventory: output.import_data_regions.clone(),
        })
    }

    /// Construct a section directly from its parts. Production goes through
    /// [`Self::from_artifact`], which locates the text and data extents; tests
    /// build sections over synthetic inventories.
    #[cfg(test)]
    pub(crate) fn from_parts(
        target: target::NativeTarget,
        text_file_offset: u64,
        inventory: image::PlacedExecutableRegionInventory,
        data_file_offset: u64,
        data_inventory: image::PlacedDataRegionInventory,
        import_data_file_offset: u64,
        import_data_inventory: image::PlacedDataRegionInventory,
    ) -> Self {
        Self {
            target,
            text_file_offset,
            inventory,
            data_file_offset,
            data_inventory,
            import_data_file_offset,
            import_data_inventory,
        }
    }

    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }

    /// The declared file offset of the exact final `.text` bytes.
    pub const fn text_file_offset(&self) -> u64 {
        self.text_file_offset
    }

    /// The complete placed executable-region inventory this section claims
    /// over the declared text extent.
    pub const fn inventory(&self) -> &image::PlacedExecutableRegionInventory {
        &self.inventory
    }

    /// The declared file offset of the exact final initialized-data bytes.
    pub const fn data_file_offset(&self) -> u64 {
        self.data_file_offset
    }

    /// The complete placed initialized-data inventory this section claims
    /// over the declared data extent.
    pub const fn data_inventory(&self) -> &image::PlacedDataRegionInventory {
        &self.data_inventory
    }

    /// The declared file offset of the exact final import-data bytes — the
    /// writer-owned `.rdata` extent a Coff container carries; `0` when the
    /// container has no separate import-data extent.
    pub const fn import_data_file_offset(&self) -> u64 {
        self.import_data_file_offset
    }

    /// The complete placed inventory this section claims over the declared
    /// import-data extent. Every row is an `ImportBindingSlot`; a target
    /// whose container has no separate import-data extent carries the
    /// canonical empty inventory.
    pub const fn import_data_inventory(&self) -> &image::PlacedDataRegionInventory {
        &self.import_data_inventory
    }

    /// Replay the checkable legs of this evidence against the exact published
    /// bytes: every declared extent must lie inside the artifact, every
    /// retained inventory must re-derive byte for byte over the bytes
    /// actually sitting there, every claimed import thunk must decode to the
    /// declared target's closed thunk sequence — on aarch64 Mach-O
    /// additionally binding the decoded pointer load to exactly one committed
    /// import-binding slot — the container's own declared entry point, when
    /// it carries one the declared format's checker can read, must name the
    /// start of a placed region the text inventory just committed — and the
    /// declared extents must be disjoint and must sit inside the loadable
    /// file ranges the container itself marks with the matching role
    /// (executable for text, writable for initialized data, loadable at all
    /// for import data). A section that lies about the artifact fails here,
    /// by name, before any behavioral leg is attempted.
    pub fn replay_against(&self, executable_bytes: &[u8]) -> Result<(), String> {
        let text_bytes = declared_extent(
            executable_bytes,
            self.text_file_offset,
            self.inventory.text_byte_count as u64,
            "executable text",
        )?;
        image::validate_placed_executable_region_inventory(&self.inventory, text_bytes)
            .map_err(|diagnostic| diagnostic.message)?;
        let data_bytes = declared_extent(
            executable_bytes,
            self.data_file_offset,
            self.data_inventory.data_byte_count as u64,
            "initialized data",
        )?;
        image::validate_placed_data_region_inventory(&self.data_inventory, data_bytes)
            .map_err(|diagnostic| diagnostic.message)?;
        let import_data_bytes = declared_extent(
            executable_bytes,
            self.import_data_file_offset,
            self.import_data_inventory.data_byte_count as u64,
            "import data",
        )?;
        image::validate_placed_data_region_inventory(
            &self.import_data_inventory,
            import_data_bytes,
        )
        .map_err(|diagnostic| diagnostic.message)?;

        // The thunk leg replays over bytes whose custody has just been
        // re-derived, so the checked opcodes are the committed ones. The
        // target thunk table is closed at decode, so only realizable pairs
        // reach this point.
        for region in &self.inventory.regions {
            if region.origin != image::FinalExecutableRegionOrigin::ImportThunk {
                continue;
            }
            match (self.target.architecture, self.target.object_format) {
                (target::Architecture::X86_64, target::ObjectFormat::Coff) => {
                    let bytes = text_bytes
                        .get(region.section_offset..region.section_offset + region.byte_count)
                        .ok_or_else(|| {
                            format!(
                                "Coff import thunk `{}` is out of the declared text extent",
                                region.symbol
                            )
                        })?;
                    if isa_x86_64::decode_x86_64_import_thunk(bytes).is_none() {
                        return Err(format!(
                            "Coff import thunk `{}` does not decode to jmp [rip+disp32]",
                            region.symbol
                        ));
                    }
                }
                (target::Architecture::Aarch64, target::ObjectFormat::MachO) => {}
                _ => {
                    return Err("the declared target realizes no import thunk regions".to_owned());
                }
            }
        }
        match (self.target.architecture, self.target.object_format) {
            (target::Architecture::Aarch64, target::ObjectFormat::MachO) => {
                image_macho::validate_macho_aarch64_import_binding_pairing(
                    text_bytes,
                    &self.inventory,
                    &self.data_inventory,
                )
                .map_err(|diagnostic| diagnostic.message)?;
            }
            (target::Architecture::X86_64, target::ObjectFormat::Coff) => {
                image_pe::validate_pe_x86_64_import_binding_pairing(
                    text_bytes,
                    &self.inventory,
                    &self.import_data_inventory,
                )
                .map_err(|diagnostic| diagnostic.message)?;
            }
            _ => {}
        }

        // The container's declared entry is a custody claim over the same
        // committed bytes: it must name the start of a placed region the
        // inventory above just re-derived — a checked instruction boundary
        // inside verified text, not a trusted producer annotation. A
        // container family carrying no checkable entry declaration leaves
        // the leg silent.
        if let Some(entry) = declared_entry_address(self.target, executable_bytes)
            && !self
                .inventory
                .regions
                .iter()
                .any(|region| region.address == entry)
        {
            return Err(format!(
                "the container-declared entry {entry:#x} does not start a placed executable region"
            ));
        }

        // Declared extents are custody claims the container itself must
        // ratify: two extents may not double-cover the same bytes, the text
        // extent must sit inside file ranges the container marks
        // executable, the initialized-data extent inside ranges it marks
        // writable, and an import-data extent inside some loadable range at
        // all. Without this the sidecar could declare coverage over bytes
        // the loader maps under a different role — or does not map — while
        // the unchecked bytes still execute. A container that does not
        // parse as the declared format, or declares no loadable ranges,
        // leaves the leg silent.
        let declared_extents = [
            (
                "executable text",
                declared_extent_range(self.text_file_offset, self.inventory.text_byte_count as u64),
            ),
            (
                "initialized data",
                declared_extent_range(
                    self.data_file_offset,
                    self.data_inventory.data_byte_count as u64,
                ),
            ),
            (
                "import data",
                declared_extent_range(
                    self.import_data_file_offset,
                    self.import_data_inventory.data_byte_count as u64,
                ),
            ),
        ];
        for (first_index, (first_name, first)) in declared_extents.iter().enumerate() {
            let Some((first_start, first_end)) = *first else {
                continue;
            };
            for (second_name, second) in declared_extents.iter().skip(first_index + 1) {
                let Some((second_start, second_end)) = *second else {
                    continue;
                };
                if first_start < second_end && second_start < first_end {
                    return Err(format!(
                        "the declared {first_name} and {second_name} extents overlap"
                    ));
                }
            }
        }
        if let Some(ranges) = declared_loadable_ranges(self.target, executable_bytes)
            && !ranges.is_empty()
        {
            for (name, extent, executable) in [
                ("executable text", declared_extents[0].1, true),
                ("initialized data", declared_extents[1].1, false),
            ] {
                if let Some((start, end)) = extent
                    && !ranges.iter().any(|range| {
                        (if executable {
                            range.executable
                        } else {
                            range.writable
                        }) && range.start <= start
                            && end <= range.end
                    })
                {
                    return Err(format!(
                        "the declared {name} extent sits outside every {} loadable range the container declares",
                        if executable { "executable" } else { "writable" }
                    ));
                }
            }
            if let Some((start, end)) = declared_extents[2].1
                && !ranges
                    .iter()
                    .any(|range| range.start <= start && end <= range.end)
            {
                return Err(
                    "the declared import-data extent sits outside every loadable range the container declares"
                        .to_owned(),
                );
            }
        }
        Ok(())
    }

    /// Serialize the section in its single canonical byte order.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = EvidenceWriter::default();
        writer.bytes(EVIDENCE_MAGIC);
        writer.u16(EVIDENCE_VERSION);
        writer.u8(match self.target.architecture {
            target::Architecture::Aarch64 => 1,
            target::Architecture::X86_64 => 2,
        });
        writer.u8(match self.target.object_format {
            target::ObjectFormat::Elf => 1,
            target::ObjectFormat::MachO => 2,
            target::ObjectFormat::Coff => 3,
        });
        writer.u64(self.target.pointer_size as u64);
        writer.u64(self.target.pointer_alignment as u64);
        writer.u64(self.text_file_offset);
        let inventory = &self.inventory;
        writer.u64(inventory.text_address);
        writer.u64(inventory.text_byte_count as u64);
        writer.bytes(inventory.text_digest.as_bytes());
        writer.u64(inventory.text_report_fingerprint);
        writer.bytes(inventory.inventory_digest.as_bytes());
        writer.u64(inventory.inventory_report_fingerprint);
        writer.u64(inventory.regions.len() as u64);
        for region in &inventory.regions {
            writer.u8(match region.origin {
                image::FinalExecutableRegionOrigin::CompilerFunction => 1,
                image::FinalExecutableRegionOrigin::ImportThunk => 2,
            });
            writer.u64(region.section_offset as u64);
            writer.u64(region.address);
            writer.u64(region.byte_count as u64);
            writer.bytes(region.byte_digest.as_bytes());
            writer.u64(region.byte_report_fingerprint);
            writer.string(&region.symbol);
            match &region.footprint {
                Some(footprint) => {
                    writer.u8(1);
                    writer.u64(footprint.registers().as_slice().len() as u64);
                    for register in footprint.registers().as_slice() {
                        writer.u16(register_code(*register));
                    }
                    writer.u16(footprint.machine_state().bits());
                }
                None => writer.u8(0),
            }
        }
        writer.u64(inventory.unclassified_gaps.len() as u64);
        for gap in &inventory.unclassified_gaps {
            writer.u64(gap.section_offset as u64);
            writer.u64(gap.address);
            writer.u64(gap.byte_count as u64);
            writer.bytes(gap.byte_digest.as_bytes());
            writer.u64(gap.byte_report_fingerprint);
        }
        writer.u64(self.data_file_offset);
        writer.data_inventory(&self.data_inventory);
        writer.u64(self.import_data_file_offset);
        writer.data_inventory(&self.import_data_inventory);
        writer.bytes
    }

    /// Decode one section. A missing magic or unknown version is
    /// `Unsupported`; a recognized section decodes strictly — ordered rows,
    /// bounded collections, in-range tags and a byte-identical re-encode — so
    /// two byte strings never decode to the same section.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, NativeEvidenceError> {
        let mut reader = EvidenceReader::new(bytes);
        if bytes.len() < EVIDENCE_MAGIC.len()
            || reader.take(EVIDENCE_MAGIC.len()).ok() != Some(EVIDENCE_MAGIC.as_slice())
        {
            return Err(NativeEvidenceError::Unsupported);
        }
        if reader.u16()? != EVIDENCE_VERSION {
            return Err(NativeEvidenceError::Unsupported);
        }
        let architecture = match reader.u8()? {
            1 => target::Architecture::Aarch64,
            2 => target::Architecture::X86_64,
            tag => return Err(malformed(format!("unknown target architecture tag {tag}"))),
        };
        let object_format = match reader.u8()? {
            1 => target::ObjectFormat::Elf,
            2 => target::ObjectFormat::MachO,
            3 => target::ObjectFormat::Coff,
            tag => return Err(malformed(format!("unknown object format tag {tag}"))),
        };
        let pointer_size = reader.usize("target pointer size")?;
        let pointer_alignment = reader.usize("target pointer alignment")?;
        // The declared target set is closed — the same (architecture,
        // object-format) pairs and 8-byte pointer axes every realization path
        // can produce. The pointer axes are bound only here: the semantic
        // profile carries the pointer size but no replay leg re-derives the
        // alignment, so a section naming a target nothing realizes is
        // malformed rather than a claim that survives to a downstream leg.
        if pointer_size != 8
            || pointer_alignment != 8
            || !matches!(
                (architecture, object_format),
                (
                    target::Architecture::X86_64,
                    target::ObjectFormat::Elf | target::ObjectFormat::Coff
                ) | (
                    target::Architecture::Aarch64,
                    target::ObjectFormat::Elf | target::ObjectFormat::MachO
                )
            )
        {
            return Err(malformed("native evidence names an undeclared target"));
        }
        let text_file_offset = reader.u64()?;

        let text_address = reader.u64()?;
        let text_byte_count = reader.usize("inventory text byte count")?;
        let text_digest = image::FinalExecutableTextDigest::from_digest(reader.array()?);
        let text_report_fingerprint = reader.u64()?;
        let inventory_digest =
            image::PlacedExecutableRegionInventoryDigest::from_digest(reader.array()?);
        let inventory_report_fingerprint = reader.u64()?;

        let declared_target = target::NativeTarget {
            architecture,
            object_format,
            pointer_size,
            pointer_alignment,
        };
        let region_count = reader.bounded_count("inventory regions", MAX_INVENTORY_ROWS)?;
        let mut regions = Vec::with_capacity(region_count);
        let mut previous_region_offset = None;
        for _ in 0..region_count {
            let origin = match reader.u8()? {
                1 => image::FinalExecutableRegionOrigin::CompilerFunction,
                2 => image::FinalExecutableRegionOrigin::ImportThunk,
                tag => {
                    return Err(malformed(format!(
                        "unknown executable region origin tag {tag}"
                    )));
                }
            };
            let section_offset = reader.usize("region section offset")?;
            if previous_region_offset.is_some_and(|previous| section_offset <= previous) {
                return Err(malformed(
                    "executable regions are not in canonical offset order",
                ));
            }
            previous_region_offset = Some(section_offset);
            let address = reader.u64()?;
            let byte_count = reader.usize("region byte count")?;
            let byte_digest =
                image::PlacedExecutableRegionBytesDigest::from_digest(reader.array()?);
            let byte_report_fingerprint = reader.u64()?;
            let symbol = reader.string("region symbol")?;
            let footprint = match reader.u8()? {
                0 => None,
                1 => Some(reader.footprint(architecture)?),
                tag => {
                    return Err(malformed(format!("invalid footprint presence tag {tag}")));
                }
            };
            // An import thunk is a claim about a closed instruction sequence
            // the declared target emits — its extent and machine-state
            // footprint are fixed by that form, not chosen by the row. A row
            // that misses either is evidence no realization path produces.
            if origin == image::FinalExecutableRegionOrigin::ImportThunk {
                match import_thunk_form(declared_target) {
                    Some((extent, expected_footprint)) => {
                        if byte_count != extent {
                            return Err(malformed(format!(
                                "import thunk `{symbol}` declares {byte_count} bytes but the closed thunk sequence is {extent}"
                            )));
                        }
                        if footprint.as_ref() != Some(&expected_footprint) {
                            return Err(malformed(format!(
                                "import thunk `{symbol}` footprint does not name the closed thunk sequence's exact machine effect"
                            )));
                        }
                    }
                    None => {
                        return Err(malformed(
                            "the declared target realizes no import thunk regions",
                        ));
                    }
                }
            }
            regions.push(image::PlacedExecutableRegion {
                origin,
                section_offset,
                address,
                byte_count,
                byte_digest,
                byte_report_fingerprint,
                symbol,
                footprint,
            });
        }

        let gap_count = reader.bounded_count("inventory gaps", MAX_INVENTORY_ROWS)?;
        let mut unclassified_gaps = Vec::with_capacity(gap_count);
        let mut previous_gap_offset = None;
        for _ in 0..gap_count {
            let section_offset = reader.usize("gap section offset")?;
            if previous_gap_offset.is_some_and(|previous| section_offset <= previous) {
                return Err(malformed(
                    "executable gaps are not in canonical offset order",
                ));
            }
            previous_gap_offset = Some(section_offset);
            unclassified_gaps.push(image::PlacedExecutableGap {
                section_offset,
                address: reader.u64()?,
                byte_count: reader.usize("gap byte count")?,
                byte_digest: image::PlacedExecutableGapBytesDigest::from_digest(reader.array()?),
                byte_report_fingerprint: reader.u64()?,
            });
        }

        let data_file_offset = reader.u64()?;
        let data_inventory = reader.data_inventory("data")?;

        let import_data_file_offset = reader.u64()?;
        let import_data_inventory = reader.data_inventory("import-data")?;
        // The import-data extent is the writer-owned import table (the PE
        // `.rdata` section): its placed rows are all `ImportBindingSlot`. A
        // target that emits no such extent carries the canonical empty
        // inventory — only (x86_64, Coff) realizes one, so a populated
        // inventory under any other target is a claim no realization path
        // produces. An empty extent has no position to commit to and its
        // offset is recorded as 0; a nonzero offset naming an empty extent
        // is never the canonical encoding.
        for region in &import_data_inventory.regions {
            if region.origin != image::FinalDataRegionOrigin::ImportBindingSlot {
                return Err(malformed(format!(
                    "import-data region `{}` is not an import binding slot",
                    region.symbol
                )));
            }
        }
        let import_data_is_empty = import_data_inventory.data_address == 0
            && import_data_inventory.data_byte_count == 0
            && import_data_inventory.regions.is_empty()
            && import_data_inventory.unclassified_gaps.is_empty();
        if import_data_is_empty && import_data_file_offset != 0 {
            return Err(malformed(
                "an empty import-data inventory must declare file offset 0",
            ));
        }
        if !import_data_is_empty
            && !matches!(
                (architecture, object_format),
                (target::Architecture::X86_64, target::ObjectFormat::Coff)
            )
        {
            return Err(malformed(
                "the declared target emits no separate import-data extent",
            ));
        }

        if reader.remaining() != 0 {
            return Err(malformed(format!(
                "native evidence section has {} trailing bytes",
                reader.remaining()
            )));
        }
        let evidence = Self {
            target: declared_target,
            text_file_offset,
            inventory: image::PlacedExecutableRegionInventory {
                text_address,
                text_byte_count,
                text_digest,
                text_report_fingerprint,
                inventory_digest,
                inventory_report_fingerprint,
                regions,
                unclassified_gaps,
            },
            data_file_offset,
            data_inventory,
            import_data_file_offset,
            import_data_inventory,
        };
        if evidence.to_bytes() != bytes {
            return Err(malformed(
                "native evidence section is not in canonical byte order",
            ));
        }
        Ok(evidence)
    }
}

fn malformed(reason: impl Into<String>) -> NativeEvidenceError {
    NativeEvidenceError::Malformed(reason.into())
}

/// The closed-register code a footprint register encodes as. The scheme is
/// the calling-conventions vocabulary's own ordering: the x86-64 GPR block,
/// then each architecture's vector/general blocks at fixed bases. An unknown
/// code rejects on decode; the vocabulary is closed.
fn register_code(register: MachineRegister) -> u16 {
    match register {
        MachineRegister::X86Rax => 0,
        MachineRegister::X86Rcx => 1,
        MachineRegister::X86Rdx => 2,
        MachineRegister::X86Rbx => 3,
        MachineRegister::X86Rsp => 4,
        MachineRegister::X86Rbp => 5,
        MachineRegister::X86Rsi => 6,
        MachineRegister::X86Rdi => 7,
        MachineRegister::X86R8 => 8,
        MachineRegister::X86R9 => 9,
        MachineRegister::X86R10 => 10,
        MachineRegister::X86R11 => 11,
        MachineRegister::X86R12 => 12,
        MachineRegister::X86R13 => 13,
        MachineRegister::X86R14 => 14,
        MachineRegister::X86R15 => 15,
        MachineRegister::X86Xmm(index) => 0x100 + u16::from(index),
        MachineRegister::Aarch64X(index) => 0x200 + u16::from(index),
        MachineRegister::Aarch64V(index) => 0x300 + u16::from(index),
    }
}

fn decode_register(code: u16) -> Result<MachineRegister, NativeEvidenceError> {
    let register = match code {
        0 => MachineRegister::X86Rax,
        1 => MachineRegister::X86Rcx,
        2 => MachineRegister::X86Rdx,
        3 => MachineRegister::X86Rbx,
        4 => MachineRegister::X86Rsp,
        5 => MachineRegister::X86Rbp,
        6 => MachineRegister::X86Rsi,
        7 => MachineRegister::X86Rdi,
        8 => MachineRegister::X86R8,
        9 => MachineRegister::X86R9,
        10 => MachineRegister::X86R10,
        11 => MachineRegister::X86R11,
        12 => MachineRegister::X86R12,
        13 => MachineRegister::X86R13,
        14 => MachineRegister::X86R14,
        15 => MachineRegister::X86R15,
        0x100..=0x1ff => MachineRegister::X86Xmm((code - 0x100) as u8),
        0x200..=0x2ff => MachineRegister::Aarch64X((code - 0x200) as u8),
        0x300..=0x3ff => MachineRegister::Aarch64V((code - 0x300) as u8),
        _ => {
            return Err(malformed(format!(
                "unknown footprint register code {code:#06x}"
            )));
        }
    };
    Ok(register)
}

/// Machine-state bits carry the closed [`MachineState`] vocabulary: only bits
/// 0 through 8 exist, so a set bit outside that range is malformed rather
/// than silently dropped.
fn decode_machine_state_set(bits: u16) -> Result<MachineStateSet, NativeEvidenceError> {
    const STATES: [MachineState; 9] = [
        MachineState::GeneralRegisters,
        MachineState::VectorRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::SegmentState,
        MachineState::ControlState,
        MachineState::DebugState,
        MachineState::ExtendedState,
    ];
    if bits & !0x1ff != 0 {
        return Err(malformed(format!(
            "machine-state bits {bits:#06x} name state classes outside the closed vocabulary"
        )));
    }
    Ok(MachineStateSet::new(
        STATES
            .iter()
            .enumerate()
            .filter(|(position, _)| bits & (1 << position) != 0)
            .map(|(_, state)| *state),
    ))
}

#[derive(Default)]
struct EvidenceWriter {
    bytes: Vec<u8>,
}

impl EvidenceWriter {
    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn string(&mut self, value: &str) {
        self.u64(value.len() as u64);
        self.bytes(value.as_bytes());
    }

    /// Serialize one placed data-region inventory — the shared encoding the
    /// initialized-data and import-data extents both carry.
    fn data_inventory(&mut self, inventory: &image::PlacedDataRegionInventory) {
        self.u64(inventory.data_address);
        self.u64(inventory.data_byte_count as u64);
        self.bytes(inventory.data_digest.as_bytes());
        self.u64(inventory.data_report_fingerprint);
        self.bytes(inventory.inventory_digest.as_bytes());
        self.u64(inventory.inventory_report_fingerprint);
        self.u64(inventory.regions.len() as u64);
        for region in &inventory.regions {
            self.u8(match region.origin {
                image::FinalDataRegionOrigin::CompilerData => 1,
                image::FinalDataRegionOrigin::ImportBindingSlot => 2,
                image::FinalDataRegionOrigin::AlignmentPadding => 3,
            });
            self.u64(region.section_offset as u64);
            self.u64(region.address);
            self.u64(region.byte_count as u64);
            self.bytes(region.byte_digest.as_bytes());
            self.u64(region.byte_report_fingerprint);
            self.string(&region.symbol);
        }
        self.u64(inventory.unclassified_gaps.len() as u64);
        for gap in &inventory.unclassified_gaps {
            self.u64(gap.section_offset as u64);
            self.u64(gap.address);
            self.u64(gap.byte_count as u64);
            self.bytes(gap.byte_digest.as_bytes());
            self.u64(gap.byte_report_fingerprint);
        }
    }
}

struct EvidenceReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> EvidenceReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.cursor
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], NativeEvidenceError> {
        if self.remaining() < count {
            return Err(malformed("native evidence section is truncated"));
        }
        let slice = &self.bytes[self.cursor..self.cursor + count];
        self.cursor += count;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, NativeEvidenceError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, NativeEvidenceError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two bytes"),
        ))
    }

    fn u64(&mut self) -> Result<u64, NativeEvidenceError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight bytes"),
        ))
    }

    fn usize(&mut self, label: &'static str) -> Result<usize, NativeEvidenceError> {
        usize::try_from(self.u64()?).map_err(|_| malformed(format!("{label} does not fit usize")))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], NativeEvidenceError> {
        Ok(self.take(N)?.try_into().expect("fixed-size array"))
    }

    fn bounded_count(
        &mut self,
        label: &'static str,
        limit: u64,
    ) -> Result<usize, NativeEvidenceError> {
        let count = self.u64()?;
        if count > limit {
            return Err(malformed(format!(
                "{label} count {count} exceeds the bound"
            )));
        }
        usize::try_from(count).map_err(|_| malformed(format!("{label} count does not fit usize")))
    }

    fn string(&mut self, label: &'static str) -> Result<String, NativeEvidenceError> {
        let len = usize::try_from(self.u64()?)
            .map_err(|_| malformed(format!("{label} length does not fit usize")))?;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|_| malformed(format!("{label} is not valid UTF-8")))
    }

    /// Decode one placed data-region inventory — the shared encoding the
    /// initialized-data and import-data extents both carry. `what` names the
    /// extent in diagnostics (`"data"` / `"import-data"`).
    fn data_inventory(
        &mut self,
        what: &'static str,
    ) -> Result<image::PlacedDataRegionInventory, NativeEvidenceError> {
        let data_address = self.u64()?;
        let data_byte_count = self.usize(if what == "data" {
            "data inventory byte count"
        } else {
            "import-data inventory byte count"
        })?;
        let data_digest = image::FinalInitializedDataDigest::from_digest(self.array()?);
        let data_report_fingerprint = self.u64()?;
        let inventory_digest = image::PlacedDataRegionInventoryDigest::from_digest(self.array()?);
        let inventory_report_fingerprint = self.u64()?;

        let region_count = self.bounded_count(
            if what == "data" {
                "data inventory regions"
            } else {
                "import-data inventory regions"
            },
            MAX_INVENTORY_ROWS,
        )?;
        let mut regions = Vec::with_capacity(region_count);
        let mut previous_offset = None;
        for _ in 0..region_count {
            let origin = match self.u8()? {
                1 => image::FinalDataRegionOrigin::CompilerData,
                2 => image::FinalDataRegionOrigin::ImportBindingSlot,
                3 => image::FinalDataRegionOrigin::AlignmentPadding,
                tag => {
                    return Err(malformed(format!("unknown {what} region origin tag {tag}")));
                }
            };
            let section_offset = self.usize(if what == "data" {
                "data region section offset"
            } else {
                "import-data region section offset"
            })?;
            if previous_offset.is_some_and(|previous| section_offset <= previous) {
                return Err(malformed(format!(
                    "{what} regions are not in canonical offset order"
                )));
            }
            previous_offset = Some(section_offset);
            let address = self.u64()?;
            let byte_count = self.usize(if what == "data" {
                "data region byte count"
            } else {
                "import-data region byte count"
            })?;
            let byte_digest = image::PlacedDataRegionBytesDigest::from_digest(self.array()?);
            let byte_report_fingerprint = self.u64()?;
            let symbol = self.string(if what == "data" {
                "data region symbol"
            } else {
                "import-data region symbol"
            })?;
            regions.push(image::PlacedDataRegion {
                origin,
                section_offset,
                address,
                byte_count,
                byte_digest,
                byte_report_fingerprint,
                symbol,
            });
        }

        let gap_count = self.bounded_count(
            if what == "data" {
                "data inventory gaps"
            } else {
                "import-data inventory gaps"
            },
            MAX_INVENTORY_ROWS,
        )?;
        let mut unclassified_gaps = Vec::with_capacity(gap_count);
        let mut previous_gap_offset = None;
        for _ in 0..gap_count {
            let section_offset = self.usize(if what == "data" {
                "data gap section offset"
            } else {
                "import-data gap section offset"
            })?;
            if previous_gap_offset.is_some_and(|previous| section_offset <= previous) {
                return Err(malformed(format!(
                    "{what} gaps are not in canonical offset order"
                )));
            }
            previous_gap_offset = Some(section_offset);
            unclassified_gaps.push(image::PlacedDataGap {
                section_offset,
                address: self.u64()?,
                byte_count: self.usize(if what == "data" {
                    "data gap byte count"
                } else {
                    "import-data gap byte count"
                })?,
                byte_digest: image::PlacedDataGapBytesDigest::from_digest(self.array()?),
                byte_report_fingerprint: self.u64()?,
            });
        }

        Ok(image::PlacedDataRegionInventory {
            data_address,
            data_byte_count,
            data_digest,
            data_report_fingerprint,
            inventory_digest,
            inventory_report_fingerprint,
            regions,
            unclassified_gaps,
        })
    }

    fn footprint(
        &mut self,
        architecture: target::Architecture,
    ) -> Result<StateFootprintEvidence, NativeEvidenceError> {
        let register_count = self.bounded_count("footprint registers", MAX_FOOTPRINT_REGISTERS)?;
        let mut registers = Vec::with_capacity(register_count);
        for _ in 0..register_count {
            let register = decode_register(self.u16()?)?;
            // The footprint vocabulary is closed per declared architecture:
            // a register no target realization can touch is not a claim the
            // checker can discharge downstream.
            if register.architecture() != architecture {
                return Err(malformed(format!(
                    "footprint register {register:?} belongs to another architecture"
                )));
            }
            registers.push(register);
        }
        let machine_state = decode_machine_state_set(self.u16()?)?;
        Ok(StateFootprintEvidence::new(
            RegisterSet::new(registers),
            machine_state,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EVIDENCE_MAGIC, NativeEvidenceError, NativePlacedImageEvidence, decode_machine_state_set,
    };
    use calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
    use image::{
        FinalExecutableRegion, FinalExecutableRegionOrigin, FinalImage, FinalImageLayout,
        FinalImageMemory,
    };

    /// Build a real placed inventory over a small text through the production
    /// `place_executable_regions`, so test evidence rows carry honest digests.
    fn placed_inventory(text: &[u8]) -> image::PlacedExecutableRegionInventory {
        let mut image = FinalImage::with_capacity(
            target::NativeTarget::host(),
            FinalImageMemory {
                text: text.to_vec(),
                ..FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.extend([
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 4,
                symbol: "entry".into(),
                footprint: Some(calling_conventions::StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86Rdi]),
                    MachineStateSet::new([MachineState::Flags]),
                )),
            },
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 8,
                byte_count: 4,
                symbol: "host_call".into(),
                footprint: None,
            },
        ]);
        image::place_executable_regions(
            &image,
            FinalImageLayout {
                text_address: 0x4010_0000,
                ..FinalImageLayout::default()
            },
        )
        .expect("the fixture regions place")
    }

    /// An empty placed data inventory — the honest shape for a text-only
    /// fixture that carries no initialized data.
    fn empty_data_inventory() -> image::PlacedDataRegionInventory {
        image::place_data_regions(
            &FinalImage::with_capacity(
                target::NativeTarget::host(),
                FinalImageMemory::default(),
                Default::default(),
                0,
                0,
                0,
            ),
            FinalImageLayout::default(),
        )
        .expect("the empty data inventory places")
    }

    fn evidence_over(text: &[u8], text_file_offset: u64) -> NativePlacedImageEvidence {
        NativePlacedImageEvidence::from_parts(
            target::NativeTarget::host(),
            text_file_offset,
            placed_inventory(text),
            0,
            empty_data_inventory(),
            0,
            empty_data_inventory(),
        )
    }

    #[test]
    fn section_round_trips_canonically() {
        let text = [0xabu8; 12];
        let evidence = evidence_over(&text, 96);
        let bytes = evidence.to_bytes();
        assert_eq!(
            NativePlacedImageEvidence::from_bytes(&bytes).expect("decode"),
            evidence
        );
    }

    #[test]
    fn decode_distinguishes_unsupported_from_malformed() {
        assert_eq!(
            NativePlacedImageEvidence::from_bytes(&[]),
            Err(NativeEvidenceError::Unsupported)
        );
        assert_eq!(
            NativePlacedImageEvidence::from_bytes(b"other format bytes"),
            Err(NativeEvidenceError::Unsupported)
        );
        // Recognized magic, unknown version: unsupported, not malformed.
        let mut unknown_version = evidence_over(&[0xabu8; 12], 0).to_bytes();
        unknown_version[EVIDENCE_MAGIC.len()] = 9;
        assert_eq!(
            NativePlacedImageEvidence::from_bytes(&unknown_version),
            Err(NativeEvidenceError::Unsupported)
        );
        // Recognized magic and version, truncated body: malformed evidence.
        let mut truncated = evidence_over(&[0xabu8; 12], 0).to_bytes();
        truncated.truncate(EVIDENCE_MAGIC.len() + 4);
        assert!(matches!(
            NativePlacedImageEvidence::from_bytes(&truncated),
            Err(NativeEvidenceError::Malformed(_))
        ));
    }

    #[test]
    fn decode_rejects_trailing_bytes_and_unordered_rows() {
        let text = [0xabu8; 12];
        let bytes = evidence_over(&text, 0).to_bytes();
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            NativePlacedImageEvidence::from_bytes(&trailing),
            Err(NativeEvidenceError::Malformed(_))
        ));

        // Two regions written out of offset order reject rather than being
        // silently re-sorted into a canonical-looking section.
        let mut image = FinalImage::with_capacity(
            target::NativeTarget::host(),
            FinalImageMemory {
                text: vec![0xabu8; 12],
                ..FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.extend([
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 8,
                byte_count: 4,
                symbol: "second".into(),
                footprint: None,
            },
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 4,
                symbol: "first".into(),
                footprint: None,
            },
        ]);
        // place_executable_regions sorts by offset, so hand the decoder an
        // evidence whose wire rows were written unsorted directly.
        let inventory = image::place_executable_regions(&image, FinalImageLayout::default())
            .expect("the fixture regions place");
        let mut inventory = inventory;
        inventory.regions.swap(0, 1);
        let evidence = NativePlacedImageEvidence::from_parts(
            target::NativeTarget::host(),
            0,
            inventory,
            0,
            empty_data_inventory(),
            0,
            empty_data_inventory(),
        );
        assert!(matches!(
            NativePlacedImageEvidence::from_bytes(&evidence.to_bytes()),
            Err(NativeEvidenceError::Malformed(_))
        ));
    }

    #[test]
    fn replay_proves_the_declared_extent_and_inventory_against_real_bytes() {
        let text = [0xabu8; 12];
        let mut container = vec![0xffu8; 96];
        container.extend_from_slice(&text);
        container.extend_from_slice(&[0x00u8; 32]);
        let evidence = evidence_over(&text, 96);
        evidence
            .replay_against(&container)
            .expect("honest evidence replays against the bytes it describes");

        // A declared extent that points at different bytes rejects.
        let shifted = evidence_over(&text, 95);
        assert!(
            shifted.replay_against(&container).is_err(),
            "an extent whose bytes do not carry the committed text must fail"
        );
        // An extent outside the artifact rejects.
        let outside = evidence_over(&text, container.len() as u64);
        assert!(outside.replay_against(&container).is_err());
        // The same section against different artifact bytes rejects.
        let mut other_container = container.clone();
        other_container[96] ^= 0xff;
        assert!(evidence.replay_against(&other_container).is_err());
    }

    #[test]
    fn machine_state_bits_outside_the_vocabulary_reject() {
        assert!(decode_machine_state_set(0x1ff).is_ok());
        assert!(decode_machine_state_set(0x200).is_err());
    }
}

#[cfg(test)]
#[path = "native_evidence/custody_tests.rs"]
mod custody_tests;
