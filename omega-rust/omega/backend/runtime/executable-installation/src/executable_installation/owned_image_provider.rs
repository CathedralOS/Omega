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
//! the patched image afterward; `seal_entry` performs the entry-sealing work
//! `InstalledCode::seal_entry_reference` contracts for, and `call` is the
//! provider's call path — the only route that turns a sealed
//! `InstalledEntryReference` into the resident entry content a physical call
//! enters; `retire` unwinds the write-to-execute transition for a drained
//! realization, and `quarantine` parks a drain that cannot complete as an
//! unserved trapping reservation — both reporting the executor quiescence
//! the `&mut self` receiver establishes structurally. Receipts are minted
//! only for steps actually performed, and a demand for facts outside this
//! provider's performed set is refused before any mutation.
//!
//! Honest boundaries: an owned buffer cannot be hardware-protected, so this
//! provider reports `WxEnforcement::ConventionOnly` — write authority is the
//! provider's own custody flag, resumed only inside `patch` and re-suspended
//! before the receipt reports it. Cache-order and instruction-fetch claims
//! rest on the performed `SeqCst` fence plus read-back comparison of the
//! stored bytes, never on an unperformed platform cache operation. Patching
//! splices position-independent fragments only: a fragment carrying
//! relocations needs a target resolver this operation does not have, so it
//! refuses rather than patching unresolved bytes. `seal_entry` replays the
//! exact committed content of the demanded entry extent — installed bytes,
//! or the fragment committed at a patched site — so a drifted or substituted
//! entry cannot seal. `call` hands the caller's executor the resident entry
//! bytes under seal, not a runnable address: the physical control transfer
//! remains the consuming platform executor's obligation, and holding the
//! returned `ResidentEntryCall` keeps the image borrowed so no `patch` or
//! `release` can run while a call is in flight — and that same exclusive
//! borrow is the executor-quiescence evidence `retire` and `quarantine`
//! report. Retirement clears the image's execute-enabled flag and restores
//! write authority; quarantine marks the image trapping — every serve path
//! refuses, and `release` refuses while the range stays reserved — so a stale
//! entry attempt faults at the provider rather than being served or
//! silently freed. Which ending a failed drain takes, and the attributed
//! residual holder the quarantine receipt names, stay caller obligations.

use std::collections::{BTreeMap, BTreeSet};

use crate::executable_installation::code_placement::ValidatedPlacementEvidence;
use crate::executable_installation::installation::InstalledCodeEvidence;
use crate::executable_installation::{
    Artifact, ArtifactContentDigest, EntryContractDigest, EntryReferenceAuthority,
    EntryReferenceFactDigest, EntryReferenceReceipt, InstallAuthority, InstallationDiagnostic,
    InstallationFactDigest, InstallationReceipt, InstalledCode, InstalledCodeId,
    InstalledEntryReference, MappingQuarantineCause, MappingQuarantineId, MappingQuarantineReceipt,
    ReplacementAuthority, ReplacementFactDigest, ReplacementReceipt, RetirementAuthority,
    RetirementFactDigest, RetirementReceipt, ValidatedPlacement, WxEnforcement,
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

/// Canonical fact bytes: the demanded entry is a declared entry of the
/// installed artifact whose resident image this provider holds.
pub const OWNED_IMAGE_SEAL_DECLARED_ENTRY: &[u8] =
    b"omega.owned-image-provider.seal-entry.declared-entry.v1";
/// Canonical fact bytes: the resident entry extent read back equal to the
/// exact committed content — the installed bytes, or the fragment committed
/// at a patched site — the provider's requirement-compatibility step.
pub const OWNED_IMAGE_SEAL_COMMITTED_CONTENT: &[u8] =
    b"omega.owned-image-provider.seal-entry.committed-content-readback.v1";
/// Canonical fact bytes: the provider's `SeqCst` fence and read-back over the
/// entry extent are its instruction-fetch visibility step for the seal.
pub const OWNED_IMAGE_SEAL_FETCH_VISIBILITY: &[u8] =
    b"omega.owned-image-provider.seal-entry.fetch-visibility-readback.v1";

/// Canonical fact bytes: no call is in flight at retirement — the `&mut self`
/// receiver is the quiescence evidence, since a live `ResidentEntryCall`
/// borrows the provider and would make this invocation impossible.
pub const OWNED_IMAGE_RETIRE_EXECUTORS_QUIESCED: &[u8] =
    b"omega.owned-image-provider.retire.executors-quiesced.v1";
/// Canonical fact bytes: the provider cleared the image's execute-enabled
/// convention flag, so `call` and `seal_entry` refuse the realization — an
/// owned buffer's execute authority is the flag, never a hardware mapping.
pub const OWNED_IMAGE_RETIRE_EXECUTE_DISABLED: &[u8] =
    b"omega.owned-image-provider.retire.execute-disabled.v1";
/// Canonical fact bytes: the provider's custody write flag was restored,
/// returning the realization's convention to writeable-not-executable.
pub const OWNED_IMAGE_RETIRE_WRITE_AUTHORITY_RESTORED: &[u8] =
    b"omega.owned-image-provider.retire.write-authority-restored.v1";

/// One installed realization's resident image under this provider's custody.
/// `write_suspended` is the provider's convention-enforced write authority:
/// mutation happens only inside a provider operation that deliberately
/// resumes it. `execute_enabled` is the matching execute authority: install
/// sets it with the write suspension, `retire` clears it and restores write
/// authority — the W+NX transition in reverse — and `call`/`seal_entry`
/// consult it. `quarantined` marks a mapping whose drain could not complete:
/// the provider stops serving it for every operation and `release` cannot
/// free it, keeping the range reserved — the owned-buffer analogue of an
/// unmapped, trapping reservation. `patched_sites` retains the exact bytes
/// each patch committed at a declared entry, so `seal_entry` replays
/// committed content rather than the superseded pre-patch bytes.
#[derive(Debug)]
struct OwnedImage {
    bytes: Vec<u8>,
    write_suspended: bool,
    execute_enabled: bool,
    quarantined: bool,
    patched_sites: BTreeMap<EntryStubId, Vec<u8>>,
}

/// The bounded extent one declared entry occupies inside a resident image:
/// its code offset up to the next declared entry, or the image end. The
/// extent is the only region a patch may write at that site and the only
/// region a seal or call may name, so all three operations share it.
fn declared_entry_extent(
    artifact: &Artifact,
    image_length: u64,
    entry: EntryStubId,
) -> Option<(u64, u64)> {
    let declared = artifact.entry(entry)?;
    let end = artifact
        .entries()
        .iter()
        .map(|candidate| candidate.code_offset)
        .filter(|offset| *offset > declared.code_offset)
        .min()
        .unwrap_or(image_length);
    Some((declared.code_offset, end))
}

/// The call edge a sealed [`InstalledEntryReference`] unlocks: the exact
/// resident extent a physical call enters, bound to the installed
/// occurrence, entry, and contract the reference seals. Holding the value
/// keeps the provider's image borrowed, so no `patch`, `release`, or
/// `install` can run against this provider while a call is in flight — the
/// caller drops it when its executor returns. This provider hands over
/// resident bytes, not a runnable address: an owned buffer carries no
/// hardware execute authority, so the physical control transfer remains the
/// consuming executor's obligation.
#[derive(Debug)]
pub struct ResidentEntryCall<'provider> {
    installed_code: InstalledCodeId,
    entry: EntryStubId,
    contract: EntryContractDigest,
    code: &'provider [u8],
}

impl ResidentEntryCall<'_> {
    /// The installed realization the sealed call targets.
    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    /// The declared entry the sealed call enters.
    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    /// The contract identity the sealed reference satisfies.
    pub const fn contract(&self) -> EntryContractDigest {
        self.contract
    }

    /// The resident bytes the sealed call enters: exactly the committed
    /// content of the entry's declared extent.
    pub const fn code(&self) -> &[u8] {
        self.code
    }
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
    next_quarantine_identity: u64,
}

impl OwnedImageProvider {
    /// A provider serving one target architecture. Installed-code and
    /// quarantine identities are minted from its own issuance counters,
    /// never supplied by callers.
    pub const fn for_architecture(architecture: Architecture) -> Self {
        Self {
            architecture,
            images: BTreeMap::new(),
            next_installed_identity: 1,
            next_quarantine_identity: 1,
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

    /// The entry-sealing facts this provider can truthfully establish.
    pub fn seal_facts() -> BTreeSet<EntryReferenceFactDigest> {
        [
            OWNED_IMAGE_SEAL_DECLARED_ENTRY,
            OWNED_IMAGE_SEAL_COMMITTED_CONTENT,
            OWNED_IMAGE_SEAL_FETCH_VISIBILITY,
        ]
        .into_iter()
        .map(EntryReferenceFactDigest::from_canonical_bytes)
        .collect()
    }

    /// The retirement facts this provider can truthfully establish.
    pub fn retire_facts() -> BTreeSet<RetirementFactDigest> {
        [
            OWNED_IMAGE_RETIRE_EXECUTORS_QUIESCED,
            OWNED_IMAGE_RETIRE_EXECUTE_DISABLED,
            OWNED_IMAGE_RETIRE_WRITE_AUTHORITY_RESTORED,
        ]
        .into_iter()
        .map(RetirementFactDigest::from_canonical_bytes)
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

    /// Whether execute authority over one resident image is currently held.
    /// Cleared by `retire` and `quarantine`; `call` and `seal_entry` refuse
    /// without it.
    pub fn execute_enabled(&self, installed: InstalledCodeId) -> Option<bool> {
        self.images
            .get(&installed)
            .map(|image| image.execute_enabled)
    }

    /// Whether one resident image is held as a quarantined trapping
    /// reservation — set by `quarantine`, never cleared: the range stays
    /// reserved until a wider isolation domain retires it, which this
    /// provider has no evidence of.
    pub fn quarantined(&self, installed: InstalledCodeId) -> Option<bool> {
        self.images.get(&installed).map(|image| image.quarantined)
    }

    /// Drop the resident image of a drained realization. This is storage
    /// release only — it mints no retirement or quarantine evidence, and the
    /// caller must already hold the lifecycle outcome before calling it. Two
    /// refusals keep the lifecycle honest: a quarantined image's range stays
    /// reserved until a wider isolation domain retires, and an image still
    /// carrying execution state — execute authority granted or write
    /// authority suspended — refuses because freeing it would reclaim
    /// storage under a mapping no retirement proved unreachable. Only an
    /// image whose execution state `retire` already unwound is freeable.
    pub fn release(&mut self, installed: InstalledCodeId) -> Result<bool, InstallationDiagnostic> {
        let Some(image) = self.images.get(&installed) else {
            return Ok(false);
        };
        if image.quarantined {
            return Err(InstallationDiagnostic(
                "a quarantined range stays reserved; releasing it is not this provider's to decide"
                    .into(),
            ));
        }
        if image.execute_enabled || image.write_suspended {
            return Err(InstallationDiagnostic(
                "the range still carries execution state; retire the realization before releasing \
                 its storage"
                    .into(),
            ));
        }
        Ok(self.images.remove(&installed).is_some())
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
            execute_enabled: true,
            quarantined: false,
            patched_sites: BTreeMap::new(),
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
    /// image byte-identical. Both realizations must be resident here and
    /// still live — the provider cannot patch code it does not hold, cannot
    /// patch a drained superseded (live-site patching has no live site left,
    /// and resuming write authority would undo the retirement that restored
    /// it), and cannot route calls to a quarantined or retired successor.
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
        match self.images.get(&successor.identity()) {
            Some(successor_image) if successor_image.quarantined => {
                return Err(InstallationDiagnostic(
                    "cannot route calls to a quarantined successor realization".into(),
                ));
            }
            Some(successor_image) if !successor_image.execute_enabled => {
                return Err(InstallationDiagnostic(
                    "cannot route calls to a successor whose execute authority was removed".into(),
                ));
            }
            Some(_) => {}
            None => {
                return Err(InstallationDiagnostic(
                    "provider holds no resident image for the successor realization".into(),
                ));
            }
        }
        let image = self.images.get(&superseded.identity()).ok_or_else(|| {
            InstallationDiagnostic(
                "provider holds no resident image for the superseded realization".into(),
            )
        })?;
        if image.quarantined {
            return Err(InstallationDiagnostic(
                "cannot patch the trapping reservation of a quarantined realization".into(),
            ));
        }
        if !image.execute_enabled {
            return Err(InstallationDiagnostic(
                "cannot patch a realization after its execute authority was removed".into(),
            ));
        }
        let artifact = &superseded.validated.frozen.artifact.artifact;
        let image_length = image.bytes.len() as u64;

        // Validate every demanded site before mutating anything: each site
        // must be a declared entry of the superseded artifact, and the bound
        // fragment must be position-independent and fit the site's extent.
        let mut splices: Vec<(EntryStubId, u64, u64, &[u8], ArtifactContentDigest)> =
            Vec::with_capacity(authority.sites.len());
        for (site, fragment) in &authority.sites {
            let Some((start, end)) = declared_entry_extent(artifact, image_length, *site) else {
                return Err(InstallationDiagnostic(format!(
                    "patch site {site:?} is not a declared entry of the superseded artifact"
                )));
            };
            if start > image_length {
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
            let extent = end - start;
            if code.len() as u64 > extent {
                return Err(InstallationDiagnostic(format!(
                    "patch fragment {:?} ({} bytes) does not fit the {extent}-byte extent of declared site {site:?}",
                    fragment_artifact.identity(),
                    code.len()
                )));
            }
            splices.push((*site, start, end, code, fragment_artifact.content()));
        }

        // Resume write authority, stage the patched image, order the stores,
        // verify every splice by read-back, then commit and re-suspend.
        let image = self
            .images
            .get_mut(&superseded.identity())
            .expect("superseded image residency was checked above");
        image.write_suspended = false;
        let mut staged = image.bytes.clone();
        for (_, offset, _, code, _) in &splices {
            let offset = usize::try_from(*offset)
                .expect("declared entry offsets are bounded by artifact byte length");
            staged[offset..offset + code.len()].copy_from_slice(code);
        }
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        for (site, offset, _, code, _) in &splices {
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
        // Retain the exact committed content of each patched extent so a
        // later seal replays what the patch committed, not the superseded
        // bytes the artifact was installed with.
        for (site, start, end, _, _) in &splices {
            let start = usize::try_from(*start)
                .expect("declared entry offsets are bounded by artifact byte length");
            let end =
                usize::try_from(*end).expect("entry extents are bounded by artifact byte length");
            image
                .patched_sites
                .insert(*site, image.bytes[start..end].to_vec());
        }
        image.write_suspended = true;

        Ok(ReplacementReceipt::from_provider(
            superseded,
            successor,
            splices
                .into_iter()
                .map(|(site, _, _, _, content)| (site, content)),
            true,
            true,
            established,
        ))
    }

    /// Perform the contracted entry-sealing operation over the resident
    /// image: confirm the demanded entry is a declared entry of the installed
    /// artifact, order the read-back, and replay the entry extent against the
    /// exact committed content — the installed bytes, or the fragment
    /// committed at a patched site. The minted receipt reports requirement
    /// compatibility and instruction-fetch visibility only after that
    /// comparison succeeds; a drifted or substituted extent refuses before
    /// any receipt leaves the provider. A demand for facts outside this
    /// provider's performed set refuses first.
    pub fn seal_entry(
        &self,
        installed: &InstalledCode,
        authority: &EntryReferenceAuthority,
    ) -> Result<EntryReferenceReceipt, InstallationDiagnostic> {
        let established = Self::seal_facts();
        if !authority.required_facts.is_subset(&established) {
            return Err(InstallationDiagnostic(
                "owned-image provider cannot establish the demanded entry-sealing facts".into(),
            ));
        }
        if authority.installed != InstalledCodeEvidence::from_installed(installed) {
            return Err(InstallationDiagnostic(
                "entry-sealing authority is not scoped to the handed installed code".into(),
            ));
        }
        let image = self.images.get(&installed.identity()).ok_or_else(|| {
            InstallationDiagnostic(
                "provider holds no resident image for the installed realization".into(),
            )
        })?;
        if image.quarantined {
            return Err(InstallationDiagnostic(
                "cannot seal an entry of a quarantined realization".into(),
            ));
        }
        if !image.execute_enabled {
            return Err(InstallationDiagnostic(
                "cannot seal an entry after the realization's execute authority was removed".into(),
            ));
        }
        if !image.write_suspended {
            return Err(InstallationDiagnostic(
                "cannot seal an entry while the provider holds write authority over the image"
                    .into(),
            ));
        }
        let artifact = &installed.validated.frozen.artifact.artifact;
        let Some((start, end)) =
            declared_entry_extent(artifact, image.bytes.len() as u64, authority.entry)
        else {
            return Err(InstallationDiagnostic(format!(
                "entry {:?} is not a declared entry of the installed artifact",
                authority.entry
            )));
        };
        let start = usize::try_from(start)
            .expect("declared entry offsets are bounded by artifact byte length");
        let end = usize::try_from(end).expect("entry extents are bounded by artifact byte length");
        let expected: &[u8] = image
            .patched_sites
            .get(&authority.entry)
            .map(Vec::as_slice)
            .unwrap_or(&installed.validated.frozen.materialized.bytes()[start..end]);
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        if image.bytes[start..end] != *expected {
            return Err(InstallationDiagnostic(format!(
                "resident extent of entry {:?} does not replay the bytes committed at install or patch",
                authority.entry
            )));
        }
        Ok(EntryReferenceReceipt::from_provider(
            installed,
            authority.entry,
            authority.contract,
            true,
            true,
        )
        .with_established_facts(established))
    }

    /// The provider's call path over one sealed [`InstalledEntryReference`]:
    /// the reference must seal the handed installed occurrence, the
    /// realization must be resident here, and the sealed entry must occupy a
    /// declared extent — then the caller receives a [`ResidentEntryCall`]
    /// carrying exactly the resident bytes the call enters. While the caller
    /// holds it, the image borrow keeps `patch`, `release`, and `install`
    /// from running against this provider, so an in-flight call is itself the
    /// quiescence witness a later retirement asks about.
    pub fn call<'provider>(
        &'provider self,
        installed: &InstalledCode,
        reference: &InstalledEntryReference<'_>,
    ) -> Result<ResidentEntryCall<'provider>, InstallationDiagnostic> {
        if reference.installed_context() != installed.receipt_context() {
            return Err(InstallationDiagnostic(
                "the sealed reference does not seal the handed installed code".into(),
            ));
        }
        let image = self
            .images
            .get(&reference.installed_code())
            .ok_or_else(|| {
                InstallationDiagnostic(
                    "provider holds no resident image for the sealed realization".into(),
                )
            })?;
        if image.quarantined {
            return Err(InstallationDiagnostic(
                "cannot invoke an entry of a quarantined realization".into(),
            ));
        }
        if !image.execute_enabled {
            return Err(InstallationDiagnostic(
                "cannot invoke an entry after the realization's execute authority was removed"
                    .into(),
            ));
        }
        if !image.write_suspended {
            return Err(InstallationDiagnostic(
                "cannot invoke an entry while the provider holds write authority over the image"
                    .into(),
            ));
        }
        let artifact = &installed.validated.frozen.artifact.artifact;
        let Some((start, end)) =
            declared_entry_extent(artifact, image.bytes.len() as u64, reference.entry())
        else {
            return Err(InstallationDiagnostic(format!(
                "the sealed entry {:?} is not a declared entry of the installed artifact",
                reference.entry()
            )));
        };
        let start = usize::try_from(start)
            .expect("declared entry offsets are bounded by artifact byte length");
        let end = usize::try_from(end).expect("entry extents are bounded by artifact byte length");
        Ok(ResidentEntryCall {
            installed_code: reference.installed_code(),
            entry: reference.entry(),
            contract: reference.contract(),
            code: &image.bytes[start..end],
        })
    }

    /// Perform the contracted retirement operation over the resident image:
    /// remove the realization's execute authority and restore its write
    /// authority — the write-to-execute transition unwound — then mint the
    /// [`RetirementReceipt`] reporting executor quiescence, execute removal,
    /// and restored write authority. Quiescence is structural, not reported
    /// on faith: `retire` takes `&mut self`, and a live [`ResidentEntryCall`]
    /// borrows the provider, so no call can be in flight here. Retired
    /// storage stays resident until the caller, holding the lifecycle's
    /// retired or quarantined outcome, invokes `release`.
    pub fn retire(
        &mut self,
        installed: &InstalledCode,
        authority: &RetirementAuthority,
    ) -> Result<RetirementReceipt, InstallationDiagnostic> {
        let established = Self::retire_facts();
        if !authority.required_facts.is_subset(&established) {
            return Err(InstallationDiagnostic(
                "owned-image provider cannot establish the demanded retirement facts".into(),
            ));
        }
        if authority.installed != InstalledCodeEvidence::from_installed(installed) {
            return Err(InstallationDiagnostic(
                "retirement authority is not scoped to the handed installed code".into(),
            ));
        }
        let image = self.images.get_mut(&installed.identity()).ok_or_else(|| {
            InstallationDiagnostic(
                "provider holds no resident image for the installed realization".into(),
            )
        })?;
        if image.quarantined {
            return Err(InstallationDiagnostic(
                "the realization's mapping is already quarantined".into(),
            ));
        }
        image.execute_enabled = false;
        image.write_suspended = false;
        Ok(RetirementReceipt::from_provider(
            installed,
            true,
            true,
            true,
            established,
        ))
    }

    /// Perform the quarantine transition a failed drain asks for: stop
    /// serving the realization's mapping — execute authority off, write
    /// authority restored, and every serve path refusing — while retaining
    /// the image so the range stays reserved. The receipt reports exactly
    /// those performed changes; the attributed `cause` is the caller's
    /// evidence of who still holds authority over the drained mapping, so an
    /// unattributed cause refuses before any mutation. Quiescence is
    /// structural: `&mut self` is unreachable while a [`ResidentEntryCall`]
    /// borrows the provider.
    pub fn quarantine(
        &mut self,
        installed: &InstalledCode,
        cause: MappingQuarantineCause,
    ) -> Result<MappingQuarantineReceipt, InstallationDiagnostic> {
        if !cause.is_attributed() {
            return Err(InstallationDiagnostic(
                "cannot quarantine a mapping without an attributed residual holder".into(),
            ));
        }
        let image = self.images.get_mut(&installed.identity()).ok_or_else(|| {
            InstallationDiagnostic(
                "provider holds no resident image for the installed realization".into(),
            )
        })?;
        if image.quarantined {
            return Err(InstallationDiagnostic(
                "the realization's mapping is already quarantined".into(),
            ));
        }
        image.execute_enabled = false;
        image.write_suspended = false;
        image.quarantined = true;
        let quarantine = self.next_quarantine_identity();
        Ok(MappingQuarantineReceipt::from_provider(
            installed, quarantine, true, true, true, cause,
        ))
    }

    fn next_installed_identity(&mut self) -> InstalledCodeId {
        let identity = self.next_installed_identity;
        self.next_installed_identity += 1;
        InstalledCodeId::from_normalized_identity(identity)
            .expect("provider issuance counter never produces zero")
    }

    fn next_quarantine_identity(&mut self) -> MappingQuarantineId {
        let identity = self.next_quarantine_identity;
        self.next_quarantine_identity += 1;
        MappingQuarantineId::from_normalized_identity(identity)
            .expect("provider issuance counter never produces zero")
    }
}

#[cfg(test)]
mod tests;
