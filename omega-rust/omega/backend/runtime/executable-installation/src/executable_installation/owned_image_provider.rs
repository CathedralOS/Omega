//! Owned-image provider performing the contracted installation and patching
//! operations over the resident image buffers it holds.
//!
//! The lifecycle transitions in `executable_installation.rs` consume provider
//! evidence; this module is a provider that produces it by doing the work. It
//! owns one resident image per installed realization: `install` copies the
//! validated final bytes into that custody, orders the stores, reads the image
//! back, and suspends its own write authority before reporting the
//! write-to-execute transition; `patch` splices each demanded declared entry
//! site with its bound admitted fragment, re-suspending write authority over
//! the patched image afterward. Receipts are minted only for steps actually
//! performed, and a demand for facts outside this provider's performed set is
//! refused before any mutation.
//!
//! Honest boundaries: an owned buffer cannot be hardware-protected, so this
//! provider reports `WxEnforcement::ConventionOnly` — write authority is the
//! provider's own custody flag, resumed only inside `patch` and re-suspended
//! before the receipt reports it. Cache-order and instruction-fetch claims
//! rest on the performed `SeqCst` fence plus read-back comparison of the
//! stored bytes, never on an unperformed platform cache operation. Patching
//! splices position-independent fragments only: a fragment carrying
//! relocations needs a target resolver this operation does not have, so it
//! refuses rather than patching unresolved bytes. Retirement, quiescence, and
//! quarantine remain separate provider obligations; `release` merely drops
//! resident storage once the caller holds that evidence.

use std::collections::{BTreeMap, BTreeSet};

use crate::executable_installation::code_placement::ValidatedPlacementEvidence;
use crate::executable_installation::installation::InstalledCodeEvidence;
use crate::executable_installation::{
    ArtifactContentDigest, InstallAuthority, InstallationDiagnostic, InstallationFactDigest,
    InstallationReceipt, InstalledCode, InstalledCodeId, ReplacementAuthority,
    ReplacementFactDigest, ReplacementReceipt, ValidatedPlacement, WxEnforcement,
};
use layout_plans::EntryStubId;
use target::Architecture;

/// Canonical fact bytes: the provider stored the validated final bytes into
/// the realization's resident image before reporting anything else.
pub const OWNED_IMAGE_BYTES_STORED: &[u8] = b"omega.owned-image-provider.install.bytes-stored.v1";
/// Canonical fact bytes: the provider suspended write authority over the
/// resident image as the write-to-execute transition it performed.
pub const OWNED_IMAGE_WRITE_TO_EXECUTE: &[u8] =
    b"omega.owned-image-provider.install.write-to-execute.v1";
/// Canonical fact bytes: stored bytes were ordered by a `SeqCst` fence before
/// any read-back or visibility claim.
pub const OWNED_IMAGE_STORE_ORDER_FENCE: &[u8] =
    b"omega.owned-image-provider.store-order.seq-cst-fence.v1";
/// Canonical fact bytes: the stored image was read back and compared equal
/// after ordering, the provider's instruction-fetch visibility step.
pub const OWNED_IMAGE_FETCH_VISIBILITY_READBACK: &[u8] =
    b"omega.owned-image-provider.fetch-visibility.readback.v1";

/// Canonical fact bytes: the provider resumed write authority over the live
/// image for the patch window only.
pub const OWNED_IMAGE_PATCH_WRITE_RESUMPTION: &[u8] =
    b"omega.owned-image-provider.patch.write-resumption.v1";
/// Canonical fact bytes: every demanded site carries its fragment's exact
/// bytes, verified by read-back after the ordering fence.
pub const OWNED_IMAGE_PATCH_SITES_WRITTEN: &[u8] =
    b"omega.owned-image-provider.patch.sites-written.v1";
/// Canonical fact bytes: write authority over the patched image was
/// re-suspended before the receipt reported it.
pub const OWNED_IMAGE_PATCH_WRITE_RESUSPENDED: &[u8] =
    b"omega.owned-image-provider.patch.write-resuspended.v1";
/// Canonical fact bytes: the patched image's read-back is the provider's
/// instruction-fetch visibility step over the patched sites.
pub const OWNED_IMAGE_PATCH_FETCH_VISIBILITY: &[u8] =
    b"omega.owned-image-provider.patch.fetch-visibility.readback.v1";

/// One installed realization's resident image under this provider's custody.
/// `write_suspended` is the provider's convention-enforced write authority:
/// mutation happens only inside a provider operation that deliberately
/// resumes it.
#[derive(Debug)]
struct OwnedImage {
    bytes: Vec<u8>,
    write_suspended: bool,
}

/// A provider that installs into and patches resident image buffers it owns.
///
/// Each `install` mints a fresh installed-code identity and stores the
/// validated final bytes under provider custody; `patch` can then mutate only
/// a realization this provider installed. The provider serves one target
/// architecture and refuses artifacts for any other.
#[derive(Debug)]
pub struct OwnedImageProvider {
    architecture: Architecture,
    images: BTreeMap<InstalledCodeId, OwnedImage>,
    next_installed_identity: u64,
}

impl OwnedImageProvider {
    /// A provider serving one target architecture. Installed-code identities
    /// are minted from its own issuance counter, never supplied by callers.
    pub const fn for_architecture(architecture: Architecture) -> Self {
        Self {
            architecture,
            images: BTreeMap::new(),
            next_installed_identity: 1,
        }
    }

    /// The installation facts this provider can truthfully establish — the
    /// exact canonical bytes behind each digest, so a receiver can demand them
    /// by identity rather than trusting a reported superset.
    pub fn install_facts() -> BTreeSet<InstallationFactDigest> {
        [
            OWNED_IMAGE_BYTES_STORED,
            OWNED_IMAGE_WRITE_TO_EXECUTE,
            OWNED_IMAGE_STORE_ORDER_FENCE,
            OWNED_IMAGE_FETCH_VISIBILITY_READBACK,
        ]
        .into_iter()
        .map(InstallationFactDigest::from_canonical_bytes)
        .collect()
    }

    /// The replacement facts this provider can truthfully establish.
    pub fn patch_facts() -> BTreeSet<ReplacementFactDigest> {
        [
            OWNED_IMAGE_PATCH_WRITE_RESUMPTION,
            OWNED_IMAGE_PATCH_SITES_WRITTEN,
            OWNED_IMAGE_PATCH_WRITE_RESUSPENDED,
            OWNED_IMAGE_PATCH_FETCH_VISIBILITY,
        ]
        .into_iter()
        .map(ReplacementFactDigest::from_canonical_bytes)
        .collect()
    }

    /// The resident bytes of one realization installed through this provider,
    /// including every committed patch. A read-only verification view: the
    /// provider's custody flag, not this accessor, is the write authority.
    pub fn installed_image(&self, installed: InstalledCodeId) -> Option<&[u8]> {
        self.images
            .get(&installed)
            .map(|image| image.bytes.as_slice())
    }

    /// Whether write authority over one resident image is currently suspended.
    pub fn write_suspended(&self, installed: InstalledCodeId) -> Option<bool> {
        self.images
            .get(&installed)
            .map(|image| image.write_suspended)
    }

    /// Drop the resident image of a drained realization. This is storage
    /// release only — it mints no retirement or quarantine evidence, and the
    /// caller must already hold the lifecycle outcome before calling it.
    pub fn release(&mut self, installed: InstalledCodeId) -> bool {
        self.images.remove(&installed).is_some()
    }

    /// Perform the contracted write-to-execute operation: copy the validated
    /// final bytes into a fresh resident image, order the stores, read the
    /// image back, and suspend write authority before reporting. The minted
    /// receipt reports `ConventionOnly` enforcement — an owned buffer carries
    /// no hardware protection — and establishes exactly the facts this
    /// provider performed; a demand beyond them refuses before any storage is
    /// allocated.
    pub fn install(
        &mut self,
        validated: &ValidatedPlacement,
        authority: &InstallAuthority,
    ) -> Result<(InstalledCodeId, InstallationReceipt), InstallationDiagnostic> {
        let artifact = &validated.frozen.artifact.artifact;
        if artifact.architecture() != self.architecture {
            return Err(InstallationDiagnostic(format!(
                "owned-image provider for {:?} cannot install {:?} artifact {:?}",
                self.architecture,
                artifact.architecture(),
                artifact.identity()
            )));
        }
        let established = Self::install_facts();
        if !authority.required_facts.is_subset(&established) {
            return Err(InstallationDiagnostic(
                "owned-image provider cannot establish the demanded installation facts".into(),
            ));
        }
        if authority.validated != ValidatedPlacementEvidence::from_validated(validated) {
            return Err(InstallationDiagnostic(
                "install authority is not scoped to the handed validated placement".into(),
            ));
        }

        let installed = self.next_installed_identity();
        let source = validated.frozen.materialized.bytes();
        let mut bytes = vec![0; source.len()];
        bytes.copy_from_slice(source);
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        let mut image = OwnedImage {
            bytes,
            write_suspended: false,
        };
        if image.bytes.as_slice() != source {
            return Err(InstallationDiagnostic(
                "installed image read-back does not match the validated final bytes".into(),
            ));
        }
        image.write_suspended = true;
        self.images.insert(installed, image);
        Ok((
            installed,
            InstallationReceipt::from_provider(
                installed,
                validated,
                true,
                WxEnforcement::ConventionOnly,
            )
            .with_established_facts(established),
        ))
    }

    /// Perform the contracted patch operation: resume write authority over the
    /// superseded realization's resident image, splice every demanded declared
    /// site with its bound admitted fragment, order and read back the patched
    /// bytes, then re-suspend write authority before reporting. The patch is
    /// staged and committed atomically, so a refused site leaves the resident
    /// image byte-identical. Both realizations must be resident here — the
    /// provider cannot patch code it does not hold, nor route calls to a
    /// successor it never installed.
    pub fn patch(
        &mut self,
        superseded: &InstalledCode,
        successor: &InstalledCode,
        authority: &ReplacementAuthority,
    ) -> Result<ReplacementReceipt, InstallationDiagnostic> {
        let established = Self::patch_facts();
        if !authority.required_facts.is_subset(&established) {
            return Err(InstallationDiagnostic(
                "owned-image provider cannot establish the demanded replacement facts".into(),
            ));
        }
        if superseded.identity() == successor.identity() {
            return Err(InstallationDiagnostic(
                "an installed realization cannot replace itself".into(),
            ));
        }
        if authority.superseded != InstalledCodeEvidence::from_installed(superseded)
            || authority.successor != InstalledCodeEvidence::from_installed(successor)
        {
            return Err(InstallationDiagnostic(
                "replacement authority is not scoped to the handed realizations".into(),
            ));
        }
        if !self.images.contains_key(&successor.identity()) {
            return Err(InstallationDiagnostic(
                "provider holds no resident image for the successor realization".into(),
            ));
        }
        let image = self.images.get(&superseded.identity()).ok_or_else(|| {
            InstallationDiagnostic(
                "provider holds no resident image for the superseded realization".into(),
            )
        })?;
        let artifact = &superseded.validated.frozen.artifact.artifact;
        let image_length = image.bytes.len() as u64;

        // Validate every demanded site before mutating anything: each site
        // must be a declared entry of the superseded artifact, and the bound
        // fragment must be position-independent and fit the site's extent —
        // the bytes from that entry's offset up to the next declared entry,
        // or the image end.
        let mut splices: Vec<(EntryStubId, u64, &[u8], ArtifactContentDigest)> =
            Vec::with_capacity(authority.sites.len());
        for (site, fragment) in &authority.sites {
            let Some(entry) = artifact.entry(*site) else {
                return Err(InstallationDiagnostic(format!(
                    "patch site {site:?} is not a declared entry of the superseded artifact"
                )));
            };
            if entry.code_offset > image_length {
                return Err(InstallationDiagnostic(format!(
                    "patch site {site:?} lies outside the resident image"
                )));
            }
            let fragment_artifact = fragment.artifact();
            if fragment_artifact.architecture() != self.architecture {
                return Err(InstallationDiagnostic(format!(
                    "owned-image provider for {:?} cannot patch in a {:?} fragment {:?}",
                    self.architecture,
                    fragment_artifact.architecture(),
                    fragment_artifact.identity()
                )));
            }
            if !fragment_artifact.relocations().is_empty() {
                return Err(InstallationDiagnostic(format!(
                    "patch fragment {:?} carries relocations; this provider splices position-independent fragments only",
                    fragment_artifact.identity()
                )));
            }
            let code = fragment_artifact.code();
            let site_end = artifact
                .entries()
                .iter()
                .map(|candidate| candidate.code_offset)
                .filter(|offset| *offset > entry.code_offset)
                .min()
                .unwrap_or(image_length);
            let extent = site_end - entry.code_offset;
            if code.len() as u64 > extent {
                return Err(InstallationDiagnostic(format!(
                    "patch fragment {:?} ({} bytes) does not fit the {extent}-byte extent of declared site {site:?}",
                    fragment_artifact.identity(),
                    code.len()
                )));
            }
            splices.push((*site, entry.code_offset, code, fragment_artifact.content()));
        }

        // Resume write authority, stage the patched image, order the stores,
        // verify every splice by read-back, then commit and re-suspend.
        let image = self
            .images
            .get_mut(&superseded.identity())
            .expect("superseded image residency was checked above");
        image.write_suspended = false;
        let mut staged = image.bytes.clone();
        for (_, offset, code, _) in &splices {
            let offset = usize::try_from(*offset)
                .expect("declared entry offsets are bounded by artifact byte length");
            staged[offset..offset + code.len()].copy_from_slice(code);
        }
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        for (site, offset, code, _) in &splices {
            let offset = usize::try_from(*offset)
                .expect("declared entry offsets are bounded by artifact byte length");
            if staged[offset..offset + code.len()] != **code {
                image.write_suspended = true;
                return Err(InstallationDiagnostic(format!(
                    "read-back of patched site {site:?} does not match the admitted fragment"
                )));
            }
        }
        image.bytes = staged;
        image.write_suspended = true;

        Ok(ReplacementReceipt::from_provider(
            superseded,
            successor,
            splices
                .into_iter()
                .map(|(site, _, _, content)| (site, content)),
            true,
            true,
            established,
        ))
    }

    fn next_installed_identity(&mut self) -> InstalledCodeId {
        let identity = self.next_installed_identity;
        self.next_installed_identity += 1;
        InstalledCodeId::from_normalized_identity(identity)
            .expect("provider issuance counter never produces zero")
    }
}

#[cfg(test)]
mod tests;
