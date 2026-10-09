//! Where each compiler function landed in the placed image, and the inventory
//! that has to reproduce that placement byte for byte.

use crate::{FinalImage, FinalImageLayout};
use calling_conventions::StateFootprintEvidence;
use diagnostics::Diagnostic;

image_digest!(FinalExecutableTextDigest);
image_digest!(PlacedExecutableRegionBytesDigest);
image_digest!(PlacedExecutableGapBytesDigest);
image_digest!(StateFootprintEvidenceDigest);
image_digest!(PlacedExecutableRegionInventoryDigest);

/// Closed origin vocabulary for bytes in the image's read-only executable
/// mapping. There is no admitted-leaf origin: adding one must also add
/// certificate replay, so admitted leaves are absent by construction until
/// that slice lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalExecutableRegionOrigin {
    CompilerFunction,
    ImportThunk,
    /// Immutable compiler data placed after the compiler's code in the same
    /// read-only mapping: today the byte-literal pool. It is never executed
    /// and carries no machine-state footprint.
    CompilerReadOnlyData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalExecutableRegion {
    pub origin: FinalExecutableRegionOrigin,
    pub section_offset: usize,
    pub byte_count: usize,
    pub symbol: String,
    /// Exact register/machine-state writes derived from final bytes when this
    /// region has a closed format-owned encoding. Compiler functions retain
    /// their composed evidence in the semantic boundary carrier instead.
    pub footprint: Option<calling_conventions::StateFootprintEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedExecutableRegion {
    pub origin: FinalExecutableRegionOrigin,
    pub section_offset: usize,
    pub address: u64,
    pub byte_count: usize,
    /// Collision-resistant commitment to the exact placed region bytes.
    pub byte_digest: PlacedExecutableRegionBytesDigest,
    /// Compact report compatibility only. It never authorizes a region join.
    pub byte_report_fingerprint: u64,
    pub symbol: String,
    pub footprint: Option<calling_conventions::StateFootprintEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedExecutableGap {
    pub section_offset: usize,
    pub address: u64,
    pub byte_count: usize,
    /// Collision-resistant commitment to the exact unclassified gap bytes.
    pub byte_digest: PlacedExecutableGapBytesDigest,
    /// Compact report compatibility only.
    pub byte_report_fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedExecutableRegionInventory {
    pub text_address: u64,
    pub text_byte_count: usize,
    /// Collision-resistant commitment to the complete exact final text.
    pub text_digest: FinalExecutableTextDigest,
    /// Compact report compatibility only.
    pub text_report_fingerprint: u64,
    /// Domain-separated commitment to the exact placed rows, gaps, text
    /// commitment, and structural footprint evidence.
    pub inventory_digest: PlacedExecutableRegionInventoryDigest,
    /// Compact report compatibility only.
    pub inventory_report_fingerprint: u64,
    pub regions: Vec<PlacedExecutableRegion>,
    pub unclassified_gaps: Vec<PlacedExecutableGap>,
}

/// Resolve the currently classified `.text` regions against final image
/// placement. Bounds and overlap failures are hard errors; gaps remain
/// explicit evidence that validation must not claim complete enumeration.
pub fn place_executable_regions(
    image: &FinalImage,
    layout: FinalImageLayout,
) -> Result<PlacedExecutableRegionInventory, Diagnostic> {
    let mut regions = image.executable_regions.clone();
    regions.sort_by_key(|region| region.section_offset);

    let mut placed = Vec::with_capacity(regions.len());
    let mut unclassified_gaps = Vec::new();
    let mut cursor = 0usize;
    for region in regions {
        if region.byte_count == 0 {
            return Err(Diagnostic::error(format!(
                "final executable region `{}` has zero width",
                region.symbol
            )));
        }
        let end = region
            .section_offset
            .checked_add(region.byte_count)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "final executable region `{}` range overflows",
                    region.symbol
                ))
            })?;
        if end > image.memory.text.len() {
            return Err(Diagnostic::error(format!(
                "final executable region `{}` [{}..{}) exceeds .text size {}",
                region.symbol,
                region.section_offset,
                end,
                image.memory.text.len()
            )));
        }
        if region.section_offset < cursor {
            return Err(Diagnostic::error(format!(
                "final executable region `{}` overlaps a preceding .text region",
                region.symbol
            )));
        }
        if region.section_offset > cursor {
            unclassified_gaps.push(placed_gap(
                image,
                layout,
                cursor,
                region.section_offset - cursor,
            )?);
        }
        let address = layout
            .text_address
            .checked_add(region.section_offset as u64)
            .ok_or_else(|| Diagnostic::error("final executable region address overflows"))?;
        cursor = end;
        let region_bytes = &image.memory.text[region.section_offset..end];
        placed.push(PlacedExecutableRegion {
            origin: region.origin,
            section_offset: region.section_offset,
            address,
            byte_count: region.byte_count,
            byte_digest: PlacedExecutableRegionBytesDigest::from_digest(digest_bytes(
                b"omega.placed-executable-region-bytes.sha256.v1\0",
                region_bytes,
            )),
            byte_report_fingerprint: byte_report_fingerprint(region_bytes),
            symbol: region.symbol,
            footprint: region.footprint,
        });
    }
    if cursor < image.memory.text.len() {
        unclassified_gaps.push(placed_gap(
            image,
            layout,
            cursor,
            image.memory.text.len() - cursor,
        )?);
    }

    let text_digest = FinalExecutableTextDigest::from_digest(digest_bytes(
        b"omega.final-executable-text.sha256.v1\0",
        &image.memory.text,
    ));
    let text_report_fingerprint = byte_report_fingerprint(&image.memory.text);
    let inventory_report_fingerprint = executable_inventory_report_fingerprint(
        layout.text_address,
        image.memory.text.len(),
        text_report_fingerprint,
        &placed,
        &unclassified_gaps,
    );
    let inventory_digest = executable_inventory_digest(
        layout.text_address,
        image.memory.text.len(),
        text_digest,
        &placed,
        &unclassified_gaps,
    );
    Ok(PlacedExecutableRegionInventory {
        text_address: layout.text_address,
        text_byte_count: image.memory.text.len(),
        text_digest,
        text_report_fingerprint,
        inventory_digest,
        inventory_report_fingerprint,
        regions: placed,
        unclassified_gaps,
    })
}

/// Independently replay a placed executable inventory against the final text
/// bytes it claims to classify. This prevents a stored summary, span address,
/// or fingerprint from becoming authority merely because it survived image
/// construction.
pub fn validate_placed_executable_region_inventory(
    inventory: &PlacedExecutableRegionInventory,
    final_text_bytes: &[u8],
) -> Result<(), Diagnostic> {
    if inventory.text_byte_count != final_text_bytes.len() {
        return Err(Diagnostic::error(format!(
            "final executable inventory records {} text byte(s), but final text contains {}",
            inventory.text_byte_count,
            final_text_bytes.len()
        )));
    }
    let text_digest = FinalExecutableTextDigest::from_digest(digest_bytes(
        b"omega.final-executable-text.sha256.v1\0",
        final_text_bytes,
    ));
    if inventory.text_digest != text_digest {
        return Err(Diagnostic::error(
            "final executable inventory text digest does not match final text",
        ));
    }
    let text_report_fingerprint = byte_report_fingerprint(final_text_bytes);
    if inventory.text_report_fingerprint != text_report_fingerprint {
        return Err(Diagnostic::error(
            "final executable inventory text fingerprint does not match final text",
        ));
    }

    let mut expected_gaps = Vec::new();
    let mut cursor = 0usize;
    for region in &inventory.regions {
        if region.byte_count == 0 {
            return Err(Diagnostic::error(format!(
                "placed executable region `{}` has zero width",
                region.symbol
            )));
        }
        let end = region
            .section_offset
            .checked_add(region.byte_count)
            .filter(|end| *end <= final_text_bytes.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "placed executable region `{}` exceeds final text",
                    region.symbol
                ))
            })?;
        if region.section_offset < cursor {
            return Err(Diagnostic::error(format!(
                "placed executable region `{}` is out of order or overlaps a preceding region",
                region.symbol
            )));
        }
        if region.section_offset > cursor {
            expected_gaps.push(placed_gap_from_bytes(
                inventory.text_address,
                final_text_bytes,
                cursor,
                region.section_offset - cursor,
            )?);
        }
        let expected_address = inventory
            .text_address
            .checked_add(region.section_offset as u64)
            .ok_or_else(|| Diagnostic::error("placed executable region address overflows"))?;
        if region.address != expected_address {
            return Err(Diagnostic::error(format!(
                "placed executable region `{}` address does not match its final text offset",
                region.symbol
            )));
        }
        let region_bytes = &final_text_bytes[region.section_offset..end];
        let expected_region_digest = PlacedExecutableRegionBytesDigest::from_digest(digest_bytes(
            b"omega.placed-executable-region-bytes.sha256.v1\0",
            region_bytes,
        ));
        if region.byte_digest != expected_region_digest {
            return Err(Diagnostic::error(format!(
                "placed executable region `{}` byte digest does not match final text",
                region.symbol
            )));
        }
        if region.byte_report_fingerprint != byte_report_fingerprint(region_bytes) {
            return Err(Diagnostic::error(format!(
                "placed executable region `{}` byte fingerprint does not match final text",
                region.symbol
            )));
        }
        cursor = end;
    }
    if cursor < final_text_bytes.len() {
        expected_gaps.push(placed_gap_from_bytes(
            inventory.text_address,
            final_text_bytes,
            cursor,
            final_text_bytes.len() - cursor,
        )?);
    }
    if inventory.unclassified_gaps != expected_gaps {
        return Err(Diagnostic::error(
            "final executable inventory gap partition does not match final text regions",
        ));
    }

    let inventory_report_fingerprint = executable_inventory_report_fingerprint(
        inventory.text_address,
        inventory.text_byte_count,
        inventory.text_report_fingerprint,
        &inventory.regions,
        &inventory.unclassified_gaps,
    );
    if inventory.inventory_report_fingerprint != inventory_report_fingerprint {
        return Err(Diagnostic::error(
            "final executable inventory fingerprint does not match its retained rows",
        ));
    }
    let inventory_digest = executable_inventory_digest(
        inventory.text_address,
        inventory.text_byte_count,
        inventory.text_digest,
        &inventory.regions,
        &inventory.unclassified_gaps,
    );
    if inventory.inventory_digest != inventory_digest {
        return Err(Diagnostic::error(
            "final executable inventory digest does not match its retained rows",
        ));
    }
    Ok(())
}



fn placed_gap(
    image: &FinalImage,
    layout: FinalImageLayout,
    section_offset: usize,
    byte_count: usize,
) -> Result<PlacedExecutableGap, Diagnostic> {
    placed_gap_from_bytes(
        layout.text_address,
        &image.memory.text,
        section_offset,
        byte_count,
    )
}

fn placed_gap_from_bytes(
    text_address: u64,
    text_bytes: &[u8],
    section_offset: usize,
    byte_count: usize,
) -> Result<PlacedExecutableGap, Diagnostic> {
    let address = text_address
        .checked_add(section_offset as u64)
        .ok_or_else(|| Diagnostic::error("final executable gap address overflows"))?;
    Ok(PlacedExecutableGap {
        section_offset,
        address,
        byte_count,
        byte_digest: PlacedExecutableGapBytesDigest::from_digest(digest_bytes(
            b"omega.placed-executable-gap-bytes.sha256.v1\0",
            &text_bytes[section_offset..section_offset + byte_count],
        )),
        byte_report_fingerprint: byte_report_fingerprint(
            &text_bytes[section_offset..section_offset + byte_count],
        ),
    })
}

pub(crate) fn executable_inventory_digest(
    text_address: u64,
    text_byte_count: usize,
    text_digest: FinalExecutableTextDigest,
    regions: &[PlacedExecutableRegion],
    gaps: &[PlacedExecutableGap],
) -> PlacedExecutableRegionInventoryDigest {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(b"omega.placed-executable-region-inventory.sha256.v1\0");
    digest.raw(&text_address.to_le_bytes());
    digest.raw(&u64::try_from(text_byte_count) .expect("final executable text length fits u64") .to_le_bytes(),);
    digest.raw(text_digest.as_bytes());
    digest.raw(&u64::try_from(regions.len()) .expect("placed executable region count fits u64") .to_le_bytes(),);
    for region in regions {
        digest.byte(match region.origin {
            FinalExecutableRegionOrigin::CompilerFunction => 1,
            FinalExecutableRegionOrigin::ImportThunk => 2,
            FinalExecutableRegionOrigin::CompilerReadOnlyData => 3,
        });
        digest.u64(region.section_offset as u64);
        digest.raw(&region.address.to_le_bytes());
        digest.u64(region.byte_count as u64);
        digest.raw(region.byte_digest.as_bytes());
        digest.u64(region.symbol.len() as u64);
        digest.raw(region.symbol.as_bytes());
        match &region.footprint {
            Some(footprint) => {
                digest.byte(1);
                digest.raw(state_footprint_evidence_digest(footprint).as_bytes());
            }
            None => digest.byte(0),
        }
    }
    digest.raw(&u64::try_from(gaps.len()) .expect("placed executable gap count fits u64") .to_le_bytes(),);
    for gap in gaps {
        digest.u64(gap.section_offset as u64);
        digest.raw(&gap.address.to_le_bytes());
        digest.u64(gap.byte_count as u64);
        digest.raw(gap.byte_digest.as_bytes());
    }
    PlacedExecutableRegionInventoryDigest::from_digest(digest.finish())
}

pub(crate) fn state_footprint_evidence_digest(
    footprint: &StateFootprintEvidence,
) -> StateFootprintEvidenceDigest {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(b"omega.state-footprint-evidence.sha256.v1\0");
    digest.u64(footprint.registers().as_slice().len() as u64);
    for register in footprint.registers().as_slice() {
        let (class, index) = register.wire_class_index();
        digest.byte(class);
        digest.raw(&index.to_le_bytes());
    }
    digest.raw(&footprint.machine_state().bits().to_le_bytes());
    StateFootprintEvidenceDigest::from_digest(digest.finish())
}

pub(crate) fn digest_bytes(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(domain);
    digest.u64(bytes.len() as u64);
    digest.raw(&bytes);
    digest.finish()
}

pub(crate) fn byte_report_fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    fingerprint_bytes(&mut hash, bytes);
    hash
}

pub(crate) fn executable_inventory_report_fingerprint(
    text_address: u64,
    text_byte_count: usize,
    text_report_fingerprint: u64,
    regions: &[PlacedExecutableRegion],
    gaps: &[PlacedExecutableGap],
) -> u64 {
    let mut hash = FNV_OFFSET;
    fingerprint_bytes(&mut hash, &text_address.to_le_bytes());
    fingerprint_bytes(&mut hash, &(text_byte_count as u64).to_le_bytes());
    fingerprint_bytes(&mut hash, &text_report_fingerprint.to_le_bytes());
    for region in regions {
        fingerprint_bytes(
            &mut hash,
            &[match region.origin {
                FinalExecutableRegionOrigin::CompilerFunction => 1,
                FinalExecutableRegionOrigin::ImportThunk => 2,
                FinalExecutableRegionOrigin::CompilerReadOnlyData => 3,
            }],
        );
        fingerprint_bytes(&mut hash, &(region.section_offset as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &region.address.to_le_bytes());
        fingerprint_bytes(&mut hash, &(region.byte_count as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &region.byte_report_fingerprint.to_le_bytes());
        fingerprint_bytes(&mut hash, region.symbol.as_bytes());
        fingerprint_bytes(&mut hash, &[0]);
        match &region.footprint {
            Some(footprint) => {
                fingerprint_bytes(&mut hash, &[1]);
                fingerprint_bytes(
                    &mut hash,
                    &footprint.evidence_report_fingerprint().to_le_bytes(),
                );
            }
            None => fingerprint_bytes(&mut hash, &[0]),
        }
    }
    for gap in gaps {
        fingerprint_bytes(&mut hash, &(gap.section_offset as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &gap.address.to_le_bytes());
        fingerprint_bytes(&mut hash, &(gap.byte_count as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &gap.byte_report_fingerprint.to_le_bytes());
    }
    hash
}

pub(crate) const FNV_OFFSET: u64 = semantic_vocabulary::Fnv1a::OFFSET_BASIS;

pub(crate) fn fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
    let mut fingerprint = semantic_vocabulary::Fnv1a::resume(*hash);
    fingerprint.bytes(bytes);
    *hash = fingerprint.finish();
}
