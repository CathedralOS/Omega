//! Lending an extent to a borrower the checker cannot inspect: the admitted
//! grant, the reach receipt that confines the borrower, the linear loan token,
//! and the completion receipt that releases it.

use std::collections::BTreeSet;

use crate::extent::diagnostic::ExtentDiagnostic;
use crate::identities::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRights, MappingEraId,
    normalized_extent_identity,
};
use crate::loans::{ExtentLoan, LoanPolarity};

normalized_extent_identity!(ExternalBorrowerId, "external-borrower");

normalized_extent_identity!(ExternalLoanId, "external-loan");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalLoanDirection {
    /// The external borrower observes memory. CPU reads remain compatible, but
    /// ordinary CPU mutation is excluded by the carried shared borrow.
    DeviceReads,
    /// The external borrower may mutate memory. The carried exclusive borrow
    /// excludes all CPU access until completion.
    DeviceWrites,
}

normalized_extent_identity!(ExternalCompletionFactId, "external-completion-fact");

normalized_extent_identity!(ExternalReachReceiptId, "external-reach receipt");

/// Why the provider may assert that the invisible borrower can reach only the
/// exact range named by one external loan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalReachMechanism {
    /// Admission trusts the borrower's checked/validated descriptor contract.
    AdmittedBorrowerContract,
    /// An IOMMU or equivalent hardware boundary enforces the exact range.
    HardwareIsolation,
}

/// Provider-authored, per-transfer evidence of external-agent confinement.
/// Numeric geometry alone is insufficient: provenance and mapping era bind an
/// identical address range to the exact authority that was lent.
#[derive(Debug, PartialEq, Eq)]
pub struct ExternalReachReceipt {
    identity: ExternalReachReceiptId,
    loan: ExternalLoanId,
    borrower: ExternalBorrowerId,
    direction: ExternalLoanDirection,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    lineage: ExtentLineageId,
    rights: ExtentRights,
    base: u64,
    length: u64,
    mechanism: ExternalReachMechanism,
    confined_to_range: bool,
}

impl ExternalReachReceipt {
    pub fn from_admitted_provider(
        identity: ExternalReachReceiptId,
        loan_identity: ExternalLoanId,
        grant: &ExternalLoanGrant,
        loan: &ExtentLoan<'_>,
        mechanism: ExternalReachMechanism,
        confined_to_range: bool,
    ) -> Self {
        Self {
            identity,
            loan: loan_identity,
            borrower: grant.borrower,
            direction: grant.direction,
            address_space: loan.address_space(),
            provenance: loan.provenance(),
            era: loan.era(),
            lineage: loan.lineage_root(),
            rights: loan.rights().clone(),
            base: loan.base(),
            length: loan.length(),
            mechanism,
            confined_to_range,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionObligations(BTreeSet<ExternalCompletionFactId>);

impl CompletionObligations {
    pub fn from_normalized_facts(
        facts: impl IntoIterator<Item = ExternalCompletionFactId>,
    ) -> Self {
        Self(facts.into_iter().collect())
    }

    pub fn facts(&self) -> impl Iterator<Item = ExternalCompletionFactId> + '_ {
        self.0.iter().copied()
    }
}

/// Reusable admitted policy for lending matching extents to one external
/// borrower. The per-transfer `ExternalLoanId` distinguishes completions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLoanGrant {
    borrower: ExternalBorrowerId,
    direction: ExternalLoanDirection,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    required_rights: ExtentRights,
    completion: CompletionObligations,
}

impl ExternalLoanGrant {
    pub fn from_admitted_provider(
        borrower: ExternalBorrowerId,
        direction: ExternalLoanDirection,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        required_rights: ExtentRights,
        completion: CompletionObligations,
    ) -> Self {
        Self {
            borrower,
            direction,
            address_space,
            provenance,
            required_rights,
            completion,
        }
    }
}

/// Linear proxy for a borrower the Omega checker cannot inspect.
///
/// The token owns the Rust loan that excludes incompatible CPU access. Omega's
/// `[linear]` rule makes completion mandatory in the source language.
#[derive(Debug)]
pub struct ExternalLoan<'extent> {
    identity: ExternalLoanId,
    borrower: ExternalBorrowerId,
    direction: ExternalLoanDirection,
    completion: CompletionObligations,
    reach_receipt: ExternalReachReceiptId,
    reach_mechanism: ExternalReachMechanism,
    loan: ExtentLoan<'extent>,
}

impl<'extent> ExternalLoan<'extent> {
    pub const fn identity(&self) -> ExternalLoanId {
        self.identity
    }

    pub const fn borrower(&self) -> ExternalBorrowerId {
        self.borrower
    }

    pub const fn direction(&self) -> ExternalLoanDirection {
        self.direction
    }

    pub const fn reach_receipt(&self) -> ExternalReachReceiptId {
        self.reach_receipt
    }

    pub const fn reach_mechanism(&self) -> ExternalReachMechanism {
        self.reach_mechanism
    }

    pub const fn base(&self) -> u64 {
        self.loan.base()
    }

    pub const fn length(&self) -> u64 {
        self.loan.length()
    }

    pub fn complete(
        self,
        receipt: ExternalCompletionReceipt,
    ) -> Result<ExternalLoanCompletion, Box<ExternalCompletionError<'extent>>> {
        let mismatch = if receipt.loan != self.identity {
            Some("completion receipt names a different external loan")
        } else if receipt.borrower != self.borrower {
            Some("completion receipt names a different external borrower")
        } else if receipt.direction != self.direction {
            Some("completion receipt names a different transfer direction")
        } else if receipt.reach_receipt != self.reach_receipt {
            Some("completion receipt names different confinement evidence")
        } else if receipt.address_space != self.loan.address_space() {
            Some("completion receipt names a different address space")
        } else if receipt.provenance != self.loan.provenance() {
            Some("completion receipt names different extent provenance")
        } else if receipt.era != self.loan.era() {
            Some("completion receipt names a stale mapping era")
        } else if receipt.lineage != self.loan.lineage_root() {
            Some("completion receipt names a different extent authority lineage")
        } else if receipt.rights != *self.loan.rights() {
            Some("completion receipt names different attenuated extent rights")
        } else if receipt.base != self.loan.base() || receipt.length != self.loan.length() {
            Some("completion receipt names a different lent extent range")
        } else if !receipt.borrow_released {
            Some("completion receipt does not establish external-borrow release")
        } else if !self.completion.0.is_subset(&receipt.established_facts) {
            Some("completion receipt lacks facts required by the external-loan grant")
        } else {
            None
        };

        if let Some(message) = mismatch {
            return Err(Box::new(ExternalCompletionError {
                loan: self,
                receipt,
                diagnostic: ExtentDiagnostic(message.into()),
            }));
        }

        Ok(ExternalLoanCompletion {
            loan: self.identity,
            borrower: self.borrower,
            reach_receipt: self.reach_receipt,
            base: self.loan.base(),
            length: self.loan.length(),
        })
    }
}

pub fn begin_external_loan<'extent>(
    loan: ExtentLoan<'extent>,
    identity: ExternalLoanId,
    grant: &ExternalLoanGrant,
    reach_receipt: Option<ExternalReachReceipt>,
) -> Result<ExternalLoan<'extent>, Box<ExternalLoanStartError<'extent>>> {
    let mismatch = if loan.address_space() != grant.address_space {
        Some("extent address space does not match external-loan grant")
    } else if loan.provenance() != grant.provenance {
        Some("extent provenance does not match external-loan grant")
    } else if !loan.rights().contains(&grant.required_rights) {
        Some("extent lacks rights required by external-loan grant")
    } else if grant.direction == ExternalLoanDirection::DeviceReads
        && loan.polarity() != LoanPolarity::Shared
    {
        Some("device-read lending requires a shared extent loan")
    } else if grant.direction == ExternalLoanDirection::DeviceWrites
        && loan.polarity() != LoanPolarity::Exclusive
    {
        Some("device-write lending requires an exclusive extent loan")
    } else {
        None
    };

    if let Some(message) = mismatch {
        return Err(Box::new(ExternalLoanStartError {
            loan,
            reach_receipt,
            diagnostic: ExtentDiagnostic(message.into()),
        }));
    }

    let reach_mismatch = match reach_receipt.as_ref() {
        None => Some("external borrower requires exact-range reach evidence or hardware isolation"),
        Some(receipt) if receipt.loan != identity => {
            Some("external-reach receipt names a different external loan")
        }
        Some(receipt) if receipt.borrower != grant.borrower => {
            Some("external-reach receipt names a different borrower")
        }
        Some(receipt) if receipt.direction != grant.direction => {
            Some("external-reach receipt names a different transfer direction")
        }
        Some(receipt) if receipt.address_space != loan.address_space() => {
            Some("external-reach receipt names a different address space")
        }
        Some(receipt) if receipt.provenance != loan.provenance() => {
            Some("external-reach receipt names different extent provenance")
        }
        Some(receipt) if receipt.era != loan.era() => {
            Some("external-reach receipt names a stale mapping era")
        }
        Some(receipt) if receipt.lineage != loan.lineage_root() => {
            Some("external-reach receipt names a different extent authority lineage")
        }
        Some(receipt) if receipt.rights != *loan.rights() => {
            Some("external-reach receipt names different attenuated extent rights")
        }
        Some(receipt) if receipt.base != loan.base() || receipt.length != loan.length() => {
            Some("external borrower reach is not the exact lent extent range")
        }
        Some(receipt) if !receipt.confined_to_range => {
            Some("external-reach receipt does not establish range confinement")
        }
        Some(_) => None,
    };
    if let Some(message) = reach_mismatch {
        return Err(Box::new(ExternalLoanStartError {
            loan,
            reach_receipt,
            diagnostic: ExtentDiagnostic(message.into()),
        }));
    }
    let reach_receipt = reach_receipt.expect("validated external-reach receipt");

    Ok(ExternalLoan {
        identity,
        borrower: grant.borrower,
        direction: grant.direction,
        completion: grant.completion.clone(),
        reach_receipt: reach_receipt.identity,
        reach_mechanism: reach_receipt.mechanism,
        loan,
    })
}

/// Provider-authored evidence that the invisible borrower released its loan.
/// Construction belongs to the admitted boundary provider, never the borrower.
#[derive(Debug, PartialEq, Eq)]
pub struct ExternalCompletionReceipt {
    loan: ExternalLoanId,
    borrower: ExternalBorrowerId,
    direction: ExternalLoanDirection,
    reach_receipt: ExternalReachReceiptId,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    lineage: ExtentLineageId,
    rights: ExtentRights,
    base: u64,
    length: u64,
    borrow_released: bool,
    established_facts: BTreeSet<ExternalCompletionFactId>,
}

impl ExternalCompletionReceipt {
    /// Bind completion evidence to the exact live external loan instead of
    /// asking the provider to restate its replay-sensitive authority facts.
    pub fn from_admitted_provider(
        loan: &ExternalLoan<'_>,
        borrow_released: bool,
        established_facts: impl IntoIterator<Item = ExternalCompletionFactId>,
    ) -> Self {
        Self {
            loan: loan.identity,
            borrower: loan.borrower,
            direction: loan.direction,
            reach_receipt: loan.reach_receipt,
            address_space: loan.loan.address_space(),
            provenance: loan.loan.provenance(),
            era: loan.loan.era(),
            lineage: loan.loan.lineage_root(),
            rights: loan.loan.rights().clone(),
            base: loan.loan.base(),
            length: loan.loan.length(),
            borrow_released,
            established_facts: established_facts.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalLoanCompletion {
    pub loan: ExternalLoanId,
    pub borrower: ExternalBorrowerId,
    pub reach_receipt: ExternalReachReceiptId,
    pub base: u64,
    pub length: u64,
}

#[derive(Debug)]
pub struct ExternalLoanStartError<'extent> {
    loan: ExtentLoan<'extent>,
    reach_receipt: Option<ExternalReachReceipt>,
    diagnostic: ExtentDiagnostic,
}

impl<'extent> ExternalLoanStartError<'extent> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_loan(self) -> ExtentLoan<'extent> {
        self.loan
    }

    pub fn into_parts(self) -> (ExtentLoan<'extent>, Option<ExternalReachReceipt>) {
        (self.loan, self.reach_receipt)
    }
}

#[derive(Debug)]
pub struct ExternalCompletionError<'extent> {
    loan: ExternalLoan<'extent>,
    receipt: ExternalCompletionReceipt,
    diagnostic: ExtentDiagnostic,
}

impl<'extent> ExternalCompletionError<'extent> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ExternalLoan<'extent>, ExternalCompletionReceipt) {
        (self.loan, self.receipt)
    }
}

#[cfg(test)]
mod tests;
