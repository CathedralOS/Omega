//! The replacement join over the patch and drain transitions
//! (wiki/spec/build/executable_installation.md#visibility-and-retirement).
//!
//! Replacing a live installed realization routes new calls first and drains
//! the superseded era afterward. The provider must patch every site the
//! authority demands with the admitted fragment bound to it — the only code
//! sites an artifact declares are its entries, so no other offset may carry a
//! patch — while write authority is re-suspended and instruction-fetch
//! visibility completes over the patched bytes. Only an established patch
//! reaches the drain: retirement returns the placement for reuse, an
//! incomplete drain with trapping evidence parks in quarantine, and any
//! refusal returns every supplied input, leaving the realization's custody
//! untouched.

use std::collections::{BTreeMap, BTreeSet};

use crate::executable_installation::installation::InstalledCodeEvidence;
use crate::executable_installation::{
    AdmittedArtifact, ArtifactContentDigest, InstallationDiagnostic, InstalledCode,
    MappingQuarantineReceipt, QuarantinedInstallation, ReplacementFactDigest, RetiredInstallation,
    RetirementAuthority, RetirementReceipt, UninstallOutcome, uninstall_installed,
};
use layout_plans::EntryStubId;

/// One-shot authority to replace one exact installed realization: the named
/// successor must already be installed, each demanded site must be a declared
/// entry of the superseded artifact, and the provider must establish the
/// demanded completion facts. Sites bind the complete admitted fragment, not a
/// compact identity, so a substituted redirect cannot borrow another
/// fragment's admission.
#[derive(Debug)]
pub struct ReplacementAuthority {
    superseded: InstalledCodeEvidence,
    successor: InstalledCodeEvidence,
    sites: BTreeMap<EntryStubId, AdmittedArtifact>,
    required_facts: BTreeSet<ReplacementFactDigest>,
}

impl ReplacementAuthority {
    /// Bind the exact superseded and successor realizations plus the admitted
    /// fragment each declared site must carry. Required facts are open
    /// provider vocabulary — the receiver names in advance the claims the
    /// contracted patch operation must establish, such as its per-site cache
    /// and instruction-fetch completion facts.
    pub fn from_admitted_provider(
        superseded: &InstalledCode,
        successor: &InstalledCode,
        sites: impl IntoIterator<Item = (EntryStubId, AdmittedArtifact)>,
        required_facts: impl IntoIterator<Item = ReplacementFactDigest>,
    ) -> Self {
        Self {
            superseded: InstalledCodeEvidence::from_installed(superseded),
            successor: InstalledCodeEvidence::from_installed(successor),
            sites: sites.into_iter().collect(),
            required_facts: required_facts.into_iter().collect(),
        }
    }
}

/// Provider result of the contracted patch operation: every demanded declared
/// site carries its admitted fragment's exact content, the patched bytes are
/// visible to instruction fetch, and the temporary write authority needed to
/// patch live code is re-suspended.
#[derive(Debug, PartialEq, Eq)]
pub struct ReplacementReceipt {
    superseded: InstalledCodeEvidence,
    successor: InstalledCodeEvidence,
    patched: BTreeMap<EntryStubId, ArtifactContentDigest>,
    visibility_complete: bool,
    write_resuspended: bool,
    established_facts: BTreeSet<ReplacementFactDigest>,
}

impl ReplacementReceipt {
    /// Bind the provider's report to both exact realizations, the
    /// site-to-content patch map it applied, and the facts it established.
    pub fn from_provider(
        superseded: &InstalledCode,
        successor: &InstalledCode,
        patched: impl IntoIterator<Item = (EntryStubId, ArtifactContentDigest)>,
        visibility_complete: bool,
        write_resuspended: bool,
        established_facts: impl IntoIterator<Item = ReplacementFactDigest>,
    ) -> Self {
        Self {
            superseded: InstalledCodeEvidence::from_installed(superseded),
            successor: InstalledCodeEvidence::from_installed(successor),
            patched: patched.into_iter().collect(),
            visibility_complete,
            write_resuspended,
            established_facts: established_facts.into_iter().collect(),
        }
    }
}

/// The two endings a replacement produces once its patch is established: the
/// superseded realization drained and its placement returned, or the same
/// range held reserved and trapping under quarantine. The accepted patch
/// receipt stays attached to either ending.
#[derive(Debug)]
pub enum ReplacementOutcome {
    Retired {
        retired: RetiredInstallation,
        patch: ReplacementReceipt,
    },
    Quarantined {
        quarantined: QuarantinedInstallation,
        patch: ReplacementReceipt,
    },
}

/// Every input, returned when the patch could not be established or the drain
/// produced no ending. `patch_diagnostic` is present when the patch leg
/// refused; the drain diagnostics record the legs that ran afterward.
#[derive(Debug)]
pub struct ReplacementError {
    installed: InstalledCode,
    authority: ReplacementAuthority,
    patch: ReplacementReceipt,
    retirement_authority: RetirementAuthority,
    retirement: RetirementReceipt,
    quarantine: Option<MappingQuarantineReceipt>,
    patch_diagnostic: Option<InstallationDiagnostic>,
    retirement_diagnostic: Option<InstallationDiagnostic>,
    quarantine_diagnostic: Option<InstallationDiagnostic>,
}

impl ReplacementError {
    /// The patch leg's refusal — when present, the drain never ran.
    pub const fn patch_diagnostic(&self) -> Option<&InstallationDiagnostic> {
        self.patch_diagnostic.as_ref()
    }

    /// The drain's retirement refusal, present once the patch was established
    /// and the drain could not complete.
    pub const fn retirement_diagnostic(&self) -> Option<&InstallationDiagnostic> {
        self.retirement_diagnostic.as_ref()
    }

    /// The drain's quarantine refusal, present only when a quarantine receipt
    /// was supplied and itself rejected.
    pub const fn quarantine_diagnostic(&self) -> Option<&InstallationDiagnostic> {
        self.quarantine_diagnostic.as_ref()
    }

    /// All inputs back, unchanged: the installed custody, both authorities,
    /// the patch report, and both drain receipts.
    pub fn into_parts(
        self,
    ) -> (
        InstalledCode,
        ReplacementAuthority,
        ReplacementReceipt,
        RetirementAuthority,
        RetirementReceipt,
        Option<MappingQuarantineReceipt>,
    ) {
        (
            self.installed,
            self.authority,
            self.patch,
            self.retirement_authority,
            self.retirement,
            self.quarantine,
        )
    }
}

/// Replace one installed realization: the provider patches every demanded
/// declared site with its admitted fragment, establishes visibility and
/// write re-suspension over the patched bytes, then the superseded custody
/// drains toward retirement or quarantine exactly as `uninstall_installed`.
/// A refused patch leaves the realization live; a refused drain returns every
/// input unless the supplied quarantine evidence parks it.
pub fn replace_installed(
    installed: InstalledCode,
    successor: &InstalledCode,
    authority: ReplacementAuthority,
    patch: ReplacementReceipt,
    retirement_authority: RetirementAuthority,
    retirement: RetirementReceipt,
    quarantine: Option<MappingQuarantineReceipt>,
) -> Result<ReplacementOutcome, Box<ReplacementError>> {
    let evidence = InstalledCodeEvidence::from_installed(&installed);
    let successor_evidence = InstalledCodeEvidence::from_installed(successor);
    let artifact = &installed.validated.frozen.artifact.artifact;
    let demanded: BTreeMap<EntryStubId, ArtifactContentDigest> = authority
        .sites
        .iter()
        .map(|(site, fragment)| (*site, fragment.artifact().content()))
        .collect();
    let mismatch = if authority.superseded != evidence {
        Some("replacement authority is not scoped to this installed code".into())
    } else if authority.successor != successor_evidence {
        Some("replacement authority does not name this successor realization".into())
    } else if evidence == successor_evidence {
        Some("an installed realization cannot replace itself".into())
    } else if let Some(site) = authority
        .sites
        .keys()
        .find(|site| artifact.entry(**site).is_none())
    {
        Some(format!(
            "replacement site {site:?} is not a declared entry of the installed artifact"
        ))
    } else if patch.superseded != evidence || patch.successor != successor_evidence {
        Some("replacement receipt does not bind this exact superseded and successor pair".into())
    } else if patch.patched != demanded {
        Some(
            "replacement receipt does not patch every demanded declared site with its admitted fragment"
                .into(),
        )
    } else if !patch.visibility_complete {
        Some(
            "replacement did not complete instruction-fetch visibility over the patched sites"
                .into(),
        )
    } else if !patch.write_resuspended {
        Some("replacement leaves residual write authority over the patched realization".into())
    } else if !authority.required_facts.is_subset(&patch.established_facts) {
        Some("replacement receipt lacks required completion facts".into())
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(ReplacementError {
            installed,
            authority,
            patch,
            retirement_authority,
            retirement,
            quarantine,
            patch_diagnostic: Some(InstallationDiagnostic(message)),
            retirement_diagnostic: None,
            quarantine_diagnostic: None,
        }));
    }

    match uninstall_installed(installed, retirement_authority, retirement, quarantine) {
        Ok(UninstallOutcome::Retired(retired)) => {
            Ok(ReplacementOutcome::Retired { retired, patch })
        }
        Ok(UninstallOutcome::Quarantined(quarantined)) => {
            Ok(ReplacementOutcome::Quarantined { quarantined, patch })
        }
        Err(error) => {
            let retirement_diagnostic = error.retirement_diagnostic().clone();
            let quarantine_diagnostic = error.quarantine_diagnostic().cloned();
            let (installed, retirement_authority, retirement, quarantine) = (*error).into_parts();
            Err(Box::new(ReplacementError {
                installed,
                authority,
                patch,
                retirement_authority,
                retirement,
                quarantine,
                patch_diagnostic: None,
                retirement_diagnostic: Some(retirement_diagnostic),
                quarantine_diagnostic,
            }))
        }
    }
}
