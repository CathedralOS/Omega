//! Executable `.text` and initialized data are placed and replayed the same
//! way: their regions must partition the extent in order, every byte outside a
//! region becomes an explicit gap digest, and each region carries the digest of
//! its own bytes. This module owns that walk and the inventory digests once;
//! each inventory supplies its digest domains, its diagnostic words and the
//! evidence its regions add.

use diagnostics::Diagnostic;

use crate::digest_encoder::DomainDigestEncoder;
use crate::final_image::{FNV_OFFSET, byte_report_fingerprint, digest_bytes, fingerprint_bytes};

pub trait ImageDigest: Copy + Eq {
    fn from_digest(digest: [u8; 32]) -> Self;
    fn as_bytes(&self) -> &[u8; 32];
}

/// A run of extent bytes that no region claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedGap<Digest> {
    pub section_offset: usize,
    pub address: u64,
    pub byte_count: usize,
    pub byte_digest: Digest,
    pub byte_report_fingerprint: u64,
}

/// A region row before or after placement.
pub(crate) trait Region {
    fn section_offset(&self) -> usize;
    fn byte_count(&self) -> usize;
    fn symbol(&self) -> &str;
}

/// A placed region row and the evidence its inventory digests.
pub(crate) trait PlacedRegion: Region {
    type Digest: ImageDigest;
    fn address(&self) -> u64;
    fn byte_digest(&self) -> Self::Digest;
    fn byte_report_fingerprint(&self) -> u64;
    fn origin_tag(&self) -> u8;
    /// Evidence this kind adds after each region's common inventory fields.
    fn digest_more(&self, _digest: &mut DomainDigestEncoder) {}
    fn fingerprint_more(&self, _hash: &mut u64) {}
}

/// What distinguishes one kind of placed extent.
pub(crate) struct ExtentKind {
    /// `executable` or `initialized-data`, as replay diagnostics name it.
    pub(crate) name: &'static str,
    /// `text` or `data`, the final bytes replay checks against.
    pub(crate) noun: &'static str,
    /// Qualifies the whole-extent digest and fingerprint in replay diagnostics.
    pub(crate) whole: &'static str,
    /// How placement words a region that runs past the extent of this length.
    pub(crate) exceeds: fn(usize) -> String,
    /// The earlier region placement names when two regions overlap.
    pub(crate) preceding: &'static str,
    pub(crate) extent_domain: &'static [u8],
    pub(crate) region_domain: &'static [u8],
    pub(crate) gap_domain: &'static [u8],
    pub(crate) inventory_domain: &'static [u8],
}

impl ExtentKind {
    pub(crate) fn extent_digest<D: ImageDigest>(&self, bytes: &[u8]) -> D {
        D::from_digest(digest_bytes(self.extent_domain, bytes))
    }

    fn gap<D: ImageDigest>(
        &self,
        base_address: u64,
        bytes: &[u8],
        section_offset: usize,
        byte_count: usize,
    ) -> Result<PlacedGap<D>, Diagnostic> {
        let address = base_address
            .checked_add(section_offset as u64)
            .ok_or_else(|| {
                Diagnostic::error(format!("final {} gap address overflows", self.name))
            })?;
        let gap_bytes = &bytes[section_offset..section_offset + byte_count];
        Ok(PlacedGap {
            section_offset,
            address,
            byte_count,
            byte_digest: D::from_digest(digest_bytes(self.gap_domain, gap_bytes)),
            byte_report_fingerprint: byte_report_fingerprint(gap_bytes),
        })
    }

    /// Place `regions`, sorted by offset, over `bytes` at `base_address`.
    /// `extent_name` names the extent in placement diagnostics. Each placed
    /// region is built by `place` from its row, address and byte evidence.
    pub(crate) fn place<R: Region, P, D: ImageDigest, G: ImageDigest>(
        &self,
        extent_name: &str,
        bytes: &[u8],
        base_address: u64,
        regions: Vec<R>,
        place: impl Fn(R, u64, D, u64) -> P,
    ) -> Result<(Vec<P>, Vec<PlacedGap<G>>), Diagnostic> {
        let mut placed = Vec::with_capacity(regions.len());
        let mut gaps = Vec::new();
        let mut cursor = 0usize;
        for row in regions {
            let region = |claim: &str| {
                Diagnostic::error(format!(
                    "final {extent_name} region `{}` {claim}",
                    row.symbol()
                ))
            };
            if row.byte_count() == 0 {
                return Err(region("has zero width"));
            }
            let start = row.section_offset();
            let end = start
                .checked_add(row.byte_count())
                .ok_or_else(|| region("range overflows"))?;
            if end > bytes.len() {
                return Err(region(&format!(
                    "[{start}..{end}) {}",
                    (self.exceeds)(bytes.len())
                )));
            }
            if start < cursor {
                return Err(region(&format!("overlaps {}", self.preceding)));
            }
            if start > cursor {
                gaps.push(self.gap(base_address, bytes, cursor, start - cursor)?);
            }
            let address = base_address.checked_add(start as u64).ok_or_else(|| {
                Diagnostic::error(format!("final {extent_name} region address overflows"))
            })?;
            cursor = end;
            let region_bytes = &bytes[start..end];
            let digest = D::from_digest(digest_bytes(self.region_domain, region_bytes));
            let fingerprint = byte_report_fingerprint(region_bytes);
            placed.push(place(row, address, digest, fingerprint));
        }
        if cursor < bytes.len() {
            gaps.push(self.gap(base_address, bytes, cursor, bytes.len() - cursor)?);
        }
        Ok((placed, gaps))
    }

    /// Replay a retained inventory against the final `bytes`: the whole-extent
    /// evidence, every region row, the gap partition, and the inventory's
    /// digest and fingerprint over all of them.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn replay<R: PlacedRegion, E: ImageDigest, G: ImageDigest, I: ImageDigest>(
        &self,
        base_address: u64,
        byte_count: usize,
        extent_digest: E,
        extent_fingerprint: u64,
        regions: &[R],
        gaps: &[PlacedGap<G>],
        inventory_digest: I,
        inventory_fingerprint: u64,
        bytes: &[u8],
    ) -> Result<(), Diagnostic> {
        let (name, noun, whole) = (self.name, self.noun, self.whole);
        if byte_count != bytes.len() {
            return Err(Diagnostic::error(format!(
                "final {name} inventory records {byte_count} {noun} byte(s), but final {noun} contains {}",
                bytes.len()
            )));
        }
        if extent_digest != self.extent_digest(bytes) {
            return Err(Diagnostic::error(format!(
                "final {name} inventory {whole}digest does not match final {noun}"
            )));
        }
        if extent_fingerprint != byte_report_fingerprint(bytes) {
            return Err(Diagnostic::error(format!(
                "final {name} inventory {whole}fingerprint does not match final {noun}"
            )));
        }

        let mut expected_gaps = Vec::new();
        let mut cursor = 0usize;
        for row in regions {
            let region = |claim: &str| {
                Diagnostic::error(format!("placed {name} region `{}` {claim}", row.symbol()))
            };
            if row.byte_count() == 0 {
                return Err(region("has zero width"));
            }
            let start = row.section_offset();
            let end = start
                .checked_add(row.byte_count())
                .filter(|end| *end <= bytes.len())
                .ok_or_else(|| region(&format!("exceeds final {noun}")))?;
            if start < cursor {
                return Err(region("is out of order or overlaps a preceding region"));
            }
            if start > cursor {
                expected_gaps.push(self.gap(base_address, bytes, cursor, start - cursor)?);
            }
            let expected_address = base_address.checked_add(start as u64).ok_or_else(|| {
                Diagnostic::error(format!("placed {name} region address overflows"))
            })?;
            if row.address() != expected_address {
                return Err(region(&format!(
                    "address does not match its final {noun} offset"
                )));
            }
            let region_bytes = &bytes[start..end];
            if *row.byte_digest().as_bytes() != digest_bytes(self.region_domain, region_bytes) {
                return Err(region(&format!("byte digest does not match final {noun}")));
            }
            if row.byte_report_fingerprint() != byte_report_fingerprint(region_bytes) {
                return Err(region(&format!(
                    "byte fingerprint does not match final {noun}"
                )));
            }
            cursor = end;
        }
        if cursor < bytes.len() {
            expected_gaps.push(self.gap(base_address, bytes, cursor, bytes.len() - cursor)?);
        }
        if gaps != expected_gaps.as_slice() {
            return Err(Diagnostic::error(format!(
                "final {name} inventory gap partition does not match final {noun} regions"
            )));
        }

        let fingerprint = inventory_report_fingerprint(
            base_address,
            byte_count,
            extent_fingerprint,
            regions,
            gaps,
        );
        if inventory_fingerprint != fingerprint {
            return Err(Diagnostic::error(format!(
                "final {name} inventory fingerprint does not match its retained rows"
            )));
        }
        let digest: I =
            self.inventory_digest(base_address, byte_count, extent_digest, regions, gaps);
        if inventory_digest != digest {
            return Err(Diagnostic::error(format!(
                "final {name} inventory digest does not match its retained rows"
            )));
        }
        Ok(())
    }

    pub(crate) fn inventory_digest<
        R: PlacedRegion,
        E: ImageDigest,
        G: ImageDigest,
        I: ImageDigest,
    >(
        &self,
        base_address: u64,
        byte_count: usize,
        extent_digest: E,
        regions: &[R],
        gaps: &[PlacedGap<G>],
    ) -> I {
        let mut digest = DomainDigestEncoder::new(self.inventory_domain);
        digest.raw(&base_address.to_le_bytes());
        digest.raw(&(byte_count as u64).to_le_bytes());
        digest.raw(extent_digest.as_bytes());
        digest.raw(&(regions.len() as u64).to_le_bytes());
        for region in regions {
            digest.byte(region.origin_tag());
            digest.u64(region.section_offset() as u64);
            digest.raw(&region.address().to_le_bytes());
            digest.u64(region.byte_count() as u64);
            digest.raw(region.byte_digest().as_bytes());
            digest.u64(region.symbol().len() as u64);
            digest.raw(region.symbol().as_bytes());
            region.digest_more(&mut digest);
        }
        digest.raw(&(gaps.len() as u64).to_le_bytes());
        for gap in gaps {
            digest.u64(gap.section_offset as u64);
            digest.raw(&gap.address.to_le_bytes());
            digest.u64(gap.byte_count as u64);
            digest.raw(gap.byte_digest.as_bytes());
        }
        I::from_digest(digest.finish())
    }
}

pub(crate) fn inventory_report_fingerprint<R: PlacedRegion, G>(
    base_address: u64,
    byte_count: usize,
    extent_fingerprint: u64,
    regions: &[R],
    gaps: &[PlacedGap<G>],
) -> u64 {
    let mut hash = FNV_OFFSET;
    fingerprint_bytes(&mut hash, &base_address.to_le_bytes());
    fingerprint_bytes(&mut hash, &(byte_count as u64).to_le_bytes());
    fingerprint_bytes(&mut hash, &extent_fingerprint.to_le_bytes());
    for region in regions {
        fingerprint_bytes(&mut hash, &[region.origin_tag()]);
        fingerprint_bytes(&mut hash, &(region.section_offset() as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &region.address().to_le_bytes());
        fingerprint_bytes(&mut hash, &(region.byte_count() as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &region.byte_report_fingerprint().to_le_bytes());
        fingerprint_bytes(&mut hash, region.symbol().as_bytes());
        fingerprint_bytes(&mut hash, &[0]);
        region.fingerprint_more(&mut hash);
    }
    for gap in gaps {
        fingerprint_bytes(&mut hash, &(gap.section_offset as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &gap.address.to_le_bytes());
        fingerprint_bytes(&mut hash, &(gap.byte_count as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &gap.byte_report_fingerprint.to_le_bytes());
    }
    hash
}
