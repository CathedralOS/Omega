//! The control-flow-integrity gate between installation and physical
//! invocation
//! (wiki/spec/build/executable_installation.md#control-flow-integrity).
//!
//! An `InstalledCode` proves a realization exists; nothing in that state may
//! yet be called. An entry becomes reachable only through this gate: the
//! receiver names the exact entry and the contract identity the reference
//! must satisfy, and the provider reports that requirement compatibility and
//! instruction-fetch visibility were established over that entry. Only an
//! established bind produces a sealed `InstalledEntryReference`, which
//! retains the satisfier — the exact installed occurrence and its admitted
//! entry — together with the contract identity.
//!
//! The reference borrows the installed realization rather than copying
//! evidence: retirement consumes `InstalledCode`, so Rust's borrow rules keep
//! the realization unretirable for exactly as long as an entry remains
//! possible. Component boundaries still use bindings, not exported local
//! descriptors — this carrier is sealed custody evidence with no serialized
//! form and no callable address. Missing or mismatched evidence rejects and
//! returns every supplied input.

use std::collections::BTreeSet;

use crate::executable_installation::installation::InstalledCodeEvidence;
use crate::executable_installation::{
    ArtifactId, EntryContractDigest, EntryReferenceFactDigest, InstallationDiagnostic,
    InstalledCode, InstalledCodeContext, InstalledCodeId,
};
use installation_evidence::InstalledArtifactOccurrenceDigest;
use layout_plans::{EntryStubId, RelocationTarget};

/// One-shot authority to seal one declared entry of one exact installed
/// realization to one demanded contract identity. Required facts are open
/// provider vocabulary — the receiver names in advance the provider-canonical
/// claims the contracted entry-sealing operation must establish, such as its
/// exact cache-order or fetch-domain completion facts — while requirement
/// compatibility and instruction-fetch visibility remain mandatory gates.
/// An authority demanding no facts admits any receipt that satisfies those
/// lifecycle gates.
#[derive(Debug, PartialEq, Eq)]
pub struct EntryReferenceAuthority {
    installed: InstalledCodeEvidence,
    entry: EntryStubId,
    contract: EntryContractDigest,
    required_facts: BTreeSet<EntryReferenceFactDigest>,
}

impl EntryReferenceAuthority {
    pub fn from_admitted_provider(
        installed: &InstalledCode,
        entry: EntryStubId,
        contract: EntryContractDigest,
    ) -> Self {
        Self {
            installed: InstalledCodeEvidence::from_installed(installed),
            entry,
            contract,
            required_facts: BTreeSet::new(),
        }
    }

    /// Names the provider-canonical facts the entry-sealing receipt must
    /// establish, the same contract installation and retirement authorities
    /// already impose.
    pub fn with_required_facts(
        mut self,
        required_facts: impl IntoIterator<Item = EntryReferenceFactDigest>,
    ) -> Self {
        self.required_facts = required_facts.into_iter().collect();
        self
    }
}

/// Provider result of the one contracted entry-sealing operation. Beyond the
/// mandatory compatibility and instruction-fetch visibility claims it reports
/// the provider-canonical facts the operation established; the gate requires
/// the authority's demanded set to appear here rather than recording
/// assertions unchallenged.
#[derive(Debug, PartialEq, Eq)]
pub struct EntryReferenceReceipt {
    installed: InstalledCodeEvidence,
    entry: EntryStubId,
    contract: EntryContractDigest,
    compatible: bool,
    instruction_fetch_visible: bool,
    established_facts: BTreeSet<EntryReferenceFactDigest>,
}

impl EntryReferenceReceipt {
    /// Bind the provider's report to the exact installed realization, the
    /// declared entry it names, and the contract identity it satisfies.
    pub fn from_provider(
        installed: &InstalledCode,
        entry: EntryStubId,
        contract: EntryContractDigest,
        compatible: bool,
        instruction_fetch_visible: bool,
    ) -> Self {
        Self {
            installed: InstalledCodeEvidence::from_installed(installed),
            entry,
            contract,
            compatible,
            instruction_fetch_visible,
            established_facts: BTreeSet::new(),
        }
    }

    /// Records the provider-canonical facts the operation established.
    pub fn with_established_facts(
        mut self,
        established_facts: impl IntoIterator<Item = EntryReferenceFactDigest>,
    ) -> Self {
        self.established_facts = established_facts.into_iter().collect();
        self
    }
}

/// Sealed requirement-compatible entry reference: the satisfier (this exact
/// installed occurrence and its admitted entry) and the contract identity it
/// satisfies, held under a borrow of the installed realization. Callers
/// receive a sealed target identity, never a resolved executable address.
#[derive(Debug)]
pub struct InstalledEntryReference<'installed> {
    installed: &'installed InstalledCode,
    entry: EntryStubId,
    contract: EntryContractDigest,
}

impl InstalledEntryReference<'_> {
    /// The declared entry this reference seals.
    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    /// The contract identity this entry satisfies.
    pub const fn contract(&self) -> EntryContractDigest {
        self.contract
    }

    pub fn installed_code(&self) -> InstalledCodeId {
        self.installed.identity()
    }

    pub fn artifact(&self) -> ArtifactId {
        self.installed.artifact()
    }

    /// Opaque complete installed-occurrence evidence for downstream equality
    /// gates. The context contains no executable address or constructor.
    pub fn installed_context(&self) -> InstalledCodeContext {
        self.installed.receipt_context()
    }

    pub fn occurrence_digest(&self) -> InstalledArtifactOccurrenceDigest {
        self.installed.occurrence_digest()
    }

    /// The sealed relocation target naming this entry. The numeric address
    /// remains private to writer execution; consumers bind this identity,
    /// never a transplanted offset.
    pub fn selected_target(&self) -> RelocationTarget {
        RelocationTarget::Entry(self.entry)
    }
}

/// Every input, returned when the entry could not be sealed. The installed
/// realization was borrowed rather than consumed, so only the authority and
/// receipt need returning.
#[derive(Debug)]
pub struct EntryReferenceError {
    authority: EntryReferenceAuthority,
    receipt: EntryReferenceReceipt,
    diagnostic: InstallationDiagnostic,
}

impl EntryReferenceError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (EntryReferenceAuthority, EntryReferenceReceipt) {
        (self.authority, self.receipt)
    }
}

impl InstalledCode {
    /// Seal one declared entry of this exact installed realization into a
    /// requirement-compatible entry reference. The authority must be scoped
    /// to this realization and the demanded entry must be a declared entry of
    /// the admitted artifact; the receipt must bind the same triple and
    /// establish requirement compatibility, instruction-fetch visibility over
    /// the entry, and every fact the authority demands. Any refusal returns
    /// the authority and receipt unchanged.
    pub fn seal_entry_reference(
        &self,
        authority: EntryReferenceAuthority,
        receipt: EntryReferenceReceipt,
    ) -> Result<InstalledEntryReference<'_>, Box<EntryReferenceError>> {
        let evidence = InstalledCodeEvidence::from_installed(self);
        let mismatch = if authority.installed != evidence {
            Some("entry-reference authority is not scoped to this installed code".into())
        } else if self
            .validated
            .frozen
            .artifact
            .artifact
            .entry(authority.entry)
            .is_none()
        {
            Some(format!(
                "entry {:?} is not a declared entry of the installed artifact",
                authority.entry
            ))
        } else if receipt.installed != evidence
            || receipt.entry != authority.entry
            || receipt.contract != authority.contract
        {
            Some(
                "entry-reference receipt does not bind this exact installed code, entry, and contract"
                    .into(),
            )
        } else if !receipt.compatible {
            Some("entry-reference receipt does not establish requirement compatibility".into())
        } else if !receipt.instruction_fetch_visible {
            Some(
                "entry-reference receipt does not establish instruction-fetch visibility over the entry"
                    .into(),
            )
        } else if !authority
            .required_facts
            .is_subset(&receipt.established_facts)
        {
            Some("entry-reference receipt lacks required completion facts".into())
        } else {
            None
        };
        if let Some(message) = mismatch {
            return Err(Box::new(EntryReferenceError {
                authority,
                receipt,
                diagnostic: InstallationDiagnostic(message),
            }));
        }
        Ok(InstalledEntryReference {
            installed: self,
            entry: authority.entry,
            contract: authority.contract,
        })
    }
}
