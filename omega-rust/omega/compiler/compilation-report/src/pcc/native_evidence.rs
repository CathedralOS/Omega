//! The bounded native evidence a `.proof` sidecar carries for the Native
//! product: the placed-image section.
//!
//! `wiki/spec/build/machine_state_evidence.md` describes the self-describing
//! certificate a native admission replays: exact final bytes, placements and
//! the complete executable-region inventory, then normalized instruction rows
//! against closed target instruction specifications, then the composed
//! footprint. This section carries the certificate's first leg plus the
//! import-thunk leg of the second — the declared executable-text and
//! initialized-data extents inside the published container, the complete
//! [`image::PlacedExecutableRegionInventory`] over the text, the complete
//! [`image::PlacedDataRegionInventory`] over the data, and the closed-form
//! realization of every claimed import thunk.
//!
//! What this evidence honestly establishes on its own: every row the section
//! claims about the published bytes is *true of those exact bytes*. The
//! receiver slices both declared extents out of the artifact it holds,
//! recomputes both commitments, and replays
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
//! exactly one committed import-binding slot. That is the first leg that
//! checks what the bytes *do* rather than only where they sit: the indirect
//! target of every Mach-O call through an import is verified, not trusted.
//!
//! What the section still does not establish is everything beyond this:
//! instruction-row semantics inside compiler-function regions, entries and
//! incoming edges, the PE thunk's `.rdata` slot pairing, premise availability
//! and lowering correspondence all still need the native semantic and
//! certification owners. The product leg keeps reporting `Incomplete` for
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
/// Section version 2 adds the placed initialized-data inventory and binds
/// import-thunk rows to the declared target's closed thunk form.
const EVIDENCE_VERSION: u16 = 2;

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

/// The placed-image evidence section: the producer's declared executable-text
/// and initialized-data extents inside the published bytes, the target the
/// bytes were realized for, and the complete placed inventories over both
/// extents.
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
/// thunk sequence. `(x86_64, Coff)` realizes `jmp [rip+disp32]` — six bytes
/// writing only the instruction pointer; `(aarch64, MachO)` realizes the
/// twelve-byte `ADRP X16, page; LDR X16, [X16, #imm]; BR X16` — X16 as sole
/// scratch plus the instruction pointer. Any other declared target emits no
/// thunk regions, so a row claiming one is not a checkable claim.
fn import_thunk_form(target: target::NativeTarget) -> Option<(usize, StateFootprintEvidence)> {
    match (target.architecture, target.object_format) {
        (target::Architecture::X86_64, target::ObjectFormat::Coff) => Some((
            6,
            StateFootprintEvidence::new(
                RegisterSet::default(),
                MachineStateSet::new([MachineState::InstructionPointer]),
            ),
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
        Ok(Self {
            target: artifact.target(),
            text_file_offset,
            inventory: output.executable_regions.clone(),
            data_file_offset,
            data_inventory: output.data_regions.clone(),
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
    ) -> Self {
        Self {
            target,
            text_file_offset,
            inventory,
            data_file_offset,
            data_inventory,
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

    /// Replay the checkable legs of this evidence against the exact published
    /// bytes: both declared extents must lie inside the artifact, both
    /// retained inventories must re-derive byte for byte over the bytes
    /// actually sitting there, and every claimed import thunk must decode to
    /// the declared target's closed thunk sequence — on aarch64 Mach-O
    /// additionally binding the decoded pointer load to exactly one committed
    /// import-binding slot. A section that lies about the artifact fails
    /// here, by name, before any behavioral leg is attempted.
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
                        .get(region.section_offset..region.section_offset + 6)
                        .ok_or_else(|| {
                            format!(
                                "Coff import thunk `{}` is out of the declared text extent",
                                region.symbol
                            )
                        })?;
                    if bytes[..2] != [0xff, 0x25] {
                        return Err(format!(
                            "Coff import thunk `{}` does not match jmp [rip+disp32]",
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
        if matches!(
            (self.target.architecture, self.target.object_format),
            (target::Architecture::Aarch64, target::ObjectFormat::MachO)
        ) {
            image_macho::validate_macho_aarch64_import_binding_pairing(
                text_bytes,
                &self.inventory,
                &self.data_inventory,
            )
            .map_err(|diagnostic| diagnostic.message)?;
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
        let data_inventory = &self.data_inventory;
        writer.u64(data_inventory.data_address);
        writer.u64(data_inventory.data_byte_count as u64);
        writer.bytes(data_inventory.data_digest.as_bytes());
        writer.u64(data_inventory.data_report_fingerprint);
        writer.bytes(data_inventory.inventory_digest.as_bytes());
        writer.u64(data_inventory.inventory_report_fingerprint);
        writer.u64(data_inventory.regions.len() as u64);
        for region in &data_inventory.regions {
            writer.u8(match region.origin {
                image::FinalDataRegionOrigin::CompilerData => 1,
                image::FinalDataRegionOrigin::ImportBindingSlot => 2,
                image::FinalDataRegionOrigin::AlignmentPadding => 3,
            });
            writer.u64(region.section_offset as u64);
            writer.u64(region.address);
            writer.u64(region.byte_count as u64);
            writer.bytes(region.byte_digest.as_bytes());
            writer.u64(region.byte_report_fingerprint);
            writer.string(&region.symbol);
        }
        writer.u64(data_inventory.unclassified_gaps.len() as u64);
        for gap in &data_inventory.unclassified_gaps {
            writer.u64(gap.section_offset as u64);
            writer.u64(gap.address);
            writer.u64(gap.byte_count as u64);
            writer.bytes(gap.byte_digest.as_bytes());
            writer.u64(gap.byte_report_fingerprint);
        }
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
        let data_address = reader.u64()?;
        let data_byte_count = reader.usize("data inventory byte count")?;
        let data_digest = image::FinalInitializedDataDigest::from_digest(reader.array()?);
        let data_report_fingerprint = reader.u64()?;
        let data_inventory_digest =
            image::PlacedDataRegionInventoryDigest::from_digest(reader.array()?);
        let data_inventory_report_fingerprint = reader.u64()?;

        let data_region_count =
            reader.bounded_count("data inventory regions", MAX_INVENTORY_ROWS)?;
        let mut data_regions = Vec::with_capacity(data_region_count);
        let mut previous_data_offset = None;
        for _ in 0..data_region_count {
            let origin = match reader.u8()? {
                1 => image::FinalDataRegionOrigin::CompilerData,
                2 => image::FinalDataRegionOrigin::ImportBindingSlot,
                3 => image::FinalDataRegionOrigin::AlignmentPadding,
                tag => {
                    return Err(malformed(format!("unknown data region origin tag {tag}")));
                }
            };
            let section_offset = reader.usize("data region section offset")?;
            if previous_data_offset.is_some_and(|previous| section_offset <= previous) {
                return Err(malformed("data regions are not in canonical offset order"));
            }
            previous_data_offset = Some(section_offset);
            let address = reader.u64()?;
            let byte_count = reader.usize("data region byte count")?;
            let byte_digest = image::PlacedDataRegionBytesDigest::from_digest(reader.array()?);
            let byte_report_fingerprint = reader.u64()?;
            let symbol = reader.string("data region symbol")?;
            data_regions.push(image::PlacedDataRegion {
                origin,
                section_offset,
                address,
                byte_count,
                byte_digest,
                byte_report_fingerprint,
                symbol,
            });
        }

        let data_gap_count = reader.bounded_count("data inventory gaps", MAX_INVENTORY_ROWS)?;
        let mut data_gaps = Vec::with_capacity(data_gap_count);
        let mut previous_data_gap_offset = None;
        for _ in 0..data_gap_count {
            let section_offset = reader.usize("data gap section offset")?;
            if previous_data_gap_offset.is_some_and(|previous| section_offset <= previous) {
                return Err(malformed("data gaps are not in canonical offset order"));
            }
            previous_data_gap_offset = Some(section_offset);
            data_gaps.push(image::PlacedDataGap {
                section_offset,
                address: reader.u64()?,
                byte_count: reader.usize("data gap byte count")?,
                byte_digest: image::PlacedDataGapBytesDigest::from_digest(reader.array()?),
                byte_report_fingerprint: reader.u64()?,
            });
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
            data_inventory: image::PlacedDataRegionInventory {
                data_address,
                data_byte_count,
                data_digest,
                data_report_fingerprint,
                inventory_digest: data_inventory_digest,
                inventory_report_fingerprint: data_inventory_report_fingerprint,
                regions: data_regions,
                unclassified_gaps: data_gaps,
            },
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
