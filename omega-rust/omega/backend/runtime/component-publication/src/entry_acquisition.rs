//! Acquiring an era's concrete runtime entry: the live-entry-gated seal of
//! one declared entry of the era's retained installed realization into a
//! requirement-compatible `InstalledEntryReference`.
//!
//! `enter` on the era ledger is the only mint of `ActiveComponentEraEntry`;
//! holding that token proves the named era has an active invocation. This
//! module joins the token to the era's retained `InstalledCode`, so
//! acquisition can only ever name the installation the live entry belongs
//! to. The executable-installation control-flow-integrity gate then decides
//! the seal itself — declared entry, requirement compatibility,
//! instruction-fetch visibility, and every completion fact the authority
//! demands — and refusal returns the authority and receipt unchanged.

use effects::ActiveComponentEraEntry;
use executable_installation::{
    EntryReferenceAuthority, EntryReferenceReceipt, InstalledEntryReference,
};

use crate::RunnableComponentEraLedger;

impl RunnableComponentEraLedger {
    /// Seal one declared entry of the era this live entry token names into a
    /// requirement-compatible runtime entry reference.
    ///
    /// Only the era the token belongs to can supply the installed
    /// realization, so a published, retained era is the reachability
    /// precondition. The authority must be scoped to that exact retained
    /// `InstalledCode` — `retained_component(entry.era_identity())` exposes
    /// it for authority and receipt construction — and the receipt must
    /// bind the same installed occurrence, entry, and contract while
    /// establishing requirement compatibility, instruction-fetch
    /// visibility, and the demanded completion facts.
    ///
    /// The returned reference borrows the ledger's retained installation,
    /// so it cannot outlive this borrow of the era evidence.
    pub fn acquire_installed_entry<'ledger>(
        &'ledger self,
        entry: &ActiveComponentEraEntry,
        authority: EntryReferenceAuthority,
        receipt: EntryReferenceReceipt,
    ) -> Result<InstalledEntryReference<'ledger>, Box<ComponentEntryAcquisitionError>> {
        let Some(component) = self.runnable.get(&entry.era_identity()) else {
            return Err(Box::new(ComponentEntryAcquisitionError {
                authority,
                receipt,
                diagnostic:
                    "component era entry names an era with no retained runnable installation evidence"
                        .into(),
            }));
        };
        component
            .installed()
            .seal_entry_reference(authority, receipt)
            .map_err(|error| {
                let diagnostic = error.diagnostic().to_string();
                let (authority, receipt) = error.into_parts();
                Box::new(ComponentEntryAcquisitionError {
                    authority,
                    receipt,
                    diagnostic,
                })
            })
    }
}

/// Rejection returning every acquisition input: either the entry token's
/// era retains no runnable installation evidence, or the
/// control-flow-integrity seal refused for the reason the diagnostic names.
/// The installed realization was borrowed rather than consumed, so only the
/// authority and receipt need returning for a corrected retry.
#[derive(Debug)]
pub struct ComponentEntryAcquisitionError {
    authority: EntryReferenceAuthority,
    receipt: EntryReferenceReceipt,
    diagnostic: String,
}

impl ComponentEntryAcquisitionError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (EntryReferenceAuthority, EntryReferenceReceipt) {
        (self.authority, self.receipt)
    }
}

impl std::fmt::Display for ComponentEntryAcquisitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ComponentEntryAcquisitionError {}
