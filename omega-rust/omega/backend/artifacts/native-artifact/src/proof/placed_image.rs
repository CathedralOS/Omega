//! The bounded native evidence a `.proof` sidecar carries for the Native
//! product: the placed-image section.
//!
//! This owner captures and replays placed-image custody. `codec` owns the
//! canonical wire encoding and bounded decode; `container_layout` reads the
//! published container's loader-visible entry and loadable ranges.
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

mod codec;
mod container_layout;

use container_layout::{declared_entry_address, declared_loadable_ranges};

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
        artifact: &crate::NativeArtifact,
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
                (target::Architecture::X86_64, target::ObjectFormat::Coff)
                | (target::Architecture::X86_64, target::ObjectFormat::MachO) => {
                    let bytes = text_bytes
                        .get(region.section_offset..region.section_offset + region.byte_count)
                        .ok_or_else(|| {
                            format!(
                                "import thunk `{}` is out of the declared text extent",
                                region.symbol
                            )
                        })?;
                    if isa_x86_64::decode_x86_64_import_thunk(bytes).is_none() {
                        return Err(format!(
                            "import thunk `{}` does not decode to jmp [rip+disp32]",
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
            (target::Architecture::X86_64, target::ObjectFormat::MachO) => {
                image_macho::validate_macho_x86_64_import_binding_pairing(
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
}
