//! Canonical placed-image evidence wire encoding and bounded decoding.

use super::{NativeEvidenceError, NativePlacedImageEvidence};

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

impl NativePlacedImageEvidence {
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
