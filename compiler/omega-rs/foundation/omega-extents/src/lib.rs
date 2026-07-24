//! Conservation model for authority over concrete address-space ranges.
//!
//! An `Extent` is not an allocator and an address is not authority. Root
//! extents enter through admitted providers; ordinary operations may only
//! split, attenuate, borrow, or rejoin authority already present.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressSpaceId(u64);

impl AddressSpaceId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "address-space")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtentProvenanceId(u64);

impl ExtentProvenanceId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "extent-provenance")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MappingEraId(u64);

impl MappingEraId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "mapping-era")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtentLineageId(u64);

impl ExtentLineageId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "extent-lineage")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

use std::collections::BTreeSet;

mod mapping;
mod page_table;

pub use mapping::*;
pub use page_table::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtentRightId(u64);

impl ExtentRightId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "extent-right")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// An open, normalized set of grant-established rights.
///
/// The compiler does not bless a READ/WRITE/EXECUTE enumeration here. Target
/// and provider packages define right identities; admission controls which
/// sets may enter a root grant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtentRights(BTreeSet<ExtentRightId>);

impl ExtentRights {
    pub const fn none() -> Self {
        Self(BTreeSet::new())
    }

    pub fn from_normalized_identities(rights: impl IntoIterator<Item = ExtentRightId>) -> Self {
        Self(rights.into_iter().collect())
    }

    pub fn union(&self, other: &Self) -> Self {
        Self(self.0.union(&other.0).copied().collect())
    }

    pub fn contains(&self, required: &Self) -> bool {
        required.0.is_subset(&self.0)
    }

    pub fn identities(&self) -> impl Iterator<Item = ExtentRightId> + '_ {
        self.0.iter().copied()
    }
}

/// Provider-admitted authority to mint exactly one root extent.
///
/// Compiler/provider code constructs this after admission. Omega source never
/// receives a constructor for either this grant or `Extent` itself.
#[derive(Debug, PartialEq, Eq)]
pub struct ExtentRootGrant {
    lineage: ExtentLineageId,
    address_space: AddressSpaceId,
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
}

impl ExtentRootGrant {
    pub const fn from_admitted_provider(
        lineage: ExtentLineageId,
        address_space: AddressSpaceId,
        rights: ExtentRights,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
    ) -> Self {
        Self {
            lineage,
            address_space,
            rights,
            provenance,
            era,
        }
    }

    pub fn mint(self, base: u64, length: u64) -> Result<Extent, MintError> {
        if let Err(diagnostic) = validate_range(base, length) {
            return Err(MintError {
                grant: self,
                diagnostic,
            });
        }
        Ok(Extent {
            base,
            length,
            address_space: self.address_space,
            rights: self.rights,
            provenance: self.provenance,
            era: self.era,
            lineage: Lineage {
                root: self.lineage,
                path: Vec::new(),
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitBranch {
    Lower,
    Upper,
}

#[derive(Debug, PartialEq, Eq)]
struct Lineage {
    root: ExtentLineageId,
    path: Vec<SplitBranch>,
}

/// Opaque authority over one concrete address-space range.
///
/// This Rust carrier is deliberately non-`Clone`; Omega's `[linear]` checker
/// supplies the source-language must-consume rule. Every consuming operation
/// below returns the authority on failure rather than silently dropping it.
#[derive(Debug, PartialEq, Eq)]
pub struct Extent {
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    lineage: Lineage,
}

impl Extent {
    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn end(&self) -> u64 {
        self.base + self.length
    }

    pub const fn address_space(&self) -> AddressSpaceId {
        self.address_space
    }

    pub const fn rights(&self) -> &ExtentRights {
        &self.rights
    }

    pub const fn provenance(&self) -> ExtentProvenanceId {
        self.provenance
    }

    pub const fn era(&self) -> MappingEraId {
        self.era
    }

    pub const fn lineage_root(&self) -> ExtentLineageId {
        self.lineage.root
    }

    pub fn split_at(self, lower_length: u64) -> Result<(Self, Self), SplitError> {
        if lower_length == 0 || lower_length >= self.length {
            return Err(SplitError {
                extent: self,
                diagnostic: ExtentDiagnostic(
                    "split point must produce two nonempty child extents".into(),
                ),
            });
        }

        let upper_base = self.base + lower_length;
        let upper_length = self.length - lower_length;
        let mut lower_path = self.lineage.path.clone();
        lower_path.push(SplitBranch::Lower);
        let mut upper_path = self.lineage.path;
        upper_path.push(SplitBranch::Upper);

        let lower = Self {
            base: self.base,
            length: lower_length,
            address_space: self.address_space,
            rights: self.rights.clone(),
            provenance: self.provenance,
            era: self.era,
            lineage: Lineage {
                root: self.lineage.root,
                path: lower_path,
            },
        };
        let upper = Self {
            base: upper_base,
            length: upper_length,
            address_space: self.address_space,
            rights: self.rights,
            provenance: self.provenance,
            era: self.era,
            lineage: Lineage {
                root: self.lineage.root,
                path: upper_path,
            },
        };
        Ok((lower, upper))
    }

    pub fn attenuate(self, rights: ExtentRights) -> Result<Self, AttenuationError> {
        if !self.rights.contains(&rights) {
            return Err(AttenuationError {
                extent: self,
                diagnostic: ExtentDiagnostic("attenuation cannot add extent rights".into()),
            });
        }
        Ok(Self { rights, ..self })
    }

    pub fn merge(self, other: Self) -> Result<Self, Box<MergeError>> {
        if let Err(diagnostic) = validate_merge(&self, &other) {
            return Err(Box::new(MergeError {
                first: self,
                second: other,
                diagnostic,
            }));
        }

        let (lower, upper) = if self.base < other.base {
            (self, other)
        } else {
            (other, self)
        };
        let mut parent_path = lower.lineage.path;
        parent_path.pop();
        Ok(Self {
            base: lower.base,
            length: lower.length + upper.length,
            address_space: lower.address_space,
            rights: lower.rights,
            provenance: lower.provenance,
            era: lower.era,
            lineage: Lineage {
                root: lower.lineage.root,
                path: parent_path,
            },
        })
    }

    pub fn loan(&self, offset: u64, length: u64) -> Result<ExtentLoan<'_>, ExtentDiagnostic> {
        ExtentLoan::shared(self, offset, length)
    }

    pub fn loan_mut(
        &mut self,
        offset: u64,
        length: u64,
    ) -> Result<ExtentLoan<'_>, ExtentDiagnostic> {
        ExtentLoan::exclusive(self, offset, length)
    }
}

fn validate_merge(first: &Extent, second: &Extent) -> Result<(), ExtentDiagnostic> {
    if first.address_space != second.address_space
        || first.rights != second.rights
        || first.provenance != second.provenance
        || first.era != second.era
    {
        return Err(ExtentDiagnostic(
            "merge requires identical space, rights, provenance, and era".into(),
        ));
    }
    if first.lineage.root != second.lineage.root {
        return Err(ExtentDiagnostic(
            "numeric adjacency cannot merge independent authority lineages".into(),
        ));
    }
    let Some((first_branch, first_parent)) = first.lineage.path.split_last() else {
        return Err(ExtentDiagnostic(
            "root extents have no merge sibling".into(),
        ));
    };
    let Some((second_branch, second_parent)) = second.lineage.path.split_last() else {
        return Err(ExtentDiagnostic(
            "root extents have no merge sibling".into(),
        ));
    };
    if first_parent != second_parent || first_branch == second_branch {
        return Err(ExtentDiagnostic(
            "merge requires the two children of one conserved split".into(),
        ));
    }

    let (lower, upper) = if first.base < second.base {
        (first, second)
    } else {
        (second, first)
    };
    let Some(lower_end) = lower.base.checked_add(lower.length) else {
        return Err(ExtentDiagnostic("lower extent range overflows".into()));
    };
    if lower_end != upper.base
        || !matches!(lower.lineage.path.last(), Some(SplitBranch::Lower))
        || !matches!(upper.lineage.path.last(), Some(SplitBranch::Upper))
    {
        return Err(ExtentDiagnostic(
            "merge children do not restore their split geometry".into(),
        ));
    }
    lower
        .length
        .checked_add(upper.length)
        .ok_or_else(|| ExtentDiagnostic("merged extent length overflows".into()))?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoanPolarity {
    Shared,
    Exclusive,
}

enum LoanBacking<'a> {
    Shared(&'a Extent),
    Exclusive(&'a mut Extent),
}

/// One borrow-carrying subrange carrier. Its polarity derives from the parent
/// borrow rather than becoming a second nominal loan type.
pub struct ExtentLoan<'a> {
    backing: LoanBacking<'a>,
    base: u64,
    length: u64,
}

impl<'a> ExtentLoan<'a> {
    fn shared(extent: &'a Extent, offset: u64, length: u64) -> Result<Self, ExtentDiagnostic> {
        let base = validate_subrange(extent, offset, length)?;
        Ok(Self {
            backing: LoanBacking::Shared(extent),
            base,
            length,
        })
    }

    fn exclusive(
        extent: &'a mut Extent,
        offset: u64,
        length: u64,
    ) -> Result<Self, ExtentDiagnostic> {
        let base = validate_subrange(extent, offset, length)?;
        Ok(Self {
            backing: LoanBacking::Exclusive(extent),
            base,
            length,
        })
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn polarity(&self) -> LoanPolarity {
        match self.backing {
            LoanBacking::Shared(_) => LoanPolarity::Shared,
            LoanBacking::Exclusive(_) => LoanPolarity::Exclusive,
        }
    }

    pub const fn address_space(&self) -> AddressSpaceId {
        match &self.backing {
            LoanBacking::Shared(extent) => extent.address_space,
            LoanBacking::Exclusive(extent) => extent.address_space,
        }
    }

    pub const fn rights(&self) -> &ExtentRights {
        match &self.backing {
            LoanBacking::Shared(extent) => &extent.rights,
            LoanBacking::Exclusive(extent) => &extent.rights,
        }
    }

    pub const fn provenance(&self) -> ExtentProvenanceId {
        match &self.backing {
            LoanBacking::Shared(extent) => extent.provenance,
            LoanBacking::Exclusive(extent) => extent.provenance,
        }
    }

    pub const fn era(&self) -> MappingEraId {
        match &self.backing {
            LoanBacking::Shared(extent) => extent.era,
            LoanBacking::Exclusive(extent) => extent.era,
        }
    }

    pub const fn lineage_root(&self) -> ExtentLineageId {
        match &self.backing {
            LoanBacking::Shared(extent) => extent.lineage.root,
            LoanBacking::Exclusive(extent) => extent.lineage.root,
        }
    }
}

impl std::fmt::Debug for ExtentLoan<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExtentLoan")
            .field("base", &self.base)
            .field("length", &self.length)
            .field("polarity", &self.polarity())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalBorrowerId(u64);

impl ExternalBorrowerId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "external-borrower")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalLoanId(u64);

impl ExternalLoanId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "external-loan")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalLoanDirection {
    /// The external borrower observes memory. CPU reads remain compatible, but
    /// ordinary CPU mutation is excluded by the carried shared borrow.
    DeviceReads,
    /// The external borrower may mutate memory. The carried exclusive borrow
    /// excludes all CPU access until completion.
    DeviceWrites,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalCompletionFactId(u64);

impl ExternalCompletionFactId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "external-completion-fact")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalReachReceiptId(u64);

impl ExternalReachReceiptId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "external-reach receipt")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

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
    base: u64,
    length: u64,
    mechanism: ExternalReachMechanism,
    confined_to_range: bool,
}

impl ExternalReachReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn from_admitted_provider(
        identity: ExternalReachReceiptId,
        loan: ExternalLoanId,
        borrower: ExternalBorrowerId,
        direction: ExternalLoanDirection,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        base: u64,
        length: u64,
        mechanism: ExternalReachMechanism,
        confined_to_range: bool,
    ) -> Self {
        Self {
            identity,
            loan,
            borrower,
            direction,
            address_space,
            provenance,
            era,
            base,
            length,
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

fn validate_subrange(extent: &Extent, offset: u64, length: u64) -> Result<u64, ExtentDiagnostic> {
    if length == 0 {
        return Err(ExtentDiagnostic("extent loan cannot be empty".into()));
    }
    let end = offset
        .checked_add(length)
        .ok_or_else(|| ExtentDiagnostic("extent loan range overflows".into()))?;
    if end > extent.length {
        return Err(ExtentDiagnostic(format!(
            "extent loan {offset}..{end} exceeds {}-byte parent",
            extent.length
        )));
    }
    extent
        .base
        .checked_add(offset)
        .ok_or_else(|| ExtentDiagnostic("extent loan base overflows".into()))
}

fn validate_range(base: u64, length: u64) -> Result<(), ExtentDiagnostic> {
    if length == 0 {
        return Err(ExtentDiagnostic(
            "root extent must carry nonempty authority".into(),
        ));
    }
    base.checked_add(length)
        .ok_or_else(|| ExtentDiagnostic("extent range overflows address width".into()))?;
    Ok(())
}

fn nonzero_identity(identity: u64, name: &str) -> Result<(), ExtentDiagnostic> {
    if identity == 0 {
        return Err(ExtentDiagnostic(format!(
            "normalized {name} identity cannot be zero"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtentDiagnostic(pub String);

impl std::fmt::Display for ExtentDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ExtentDiagnostic {}

#[derive(Debug, PartialEq, Eq)]
pub struct MintError {
    grant: ExtentRootGrant,
    diagnostic: ExtentDiagnostic,
}

impl MintError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_grant(self) -> ExtentRootGrant {
        self.grant
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SplitError {
    extent: Extent,
    diagnostic: ExtentDiagnostic,
}

impl SplitError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_extent(self) -> Extent {
        self.extent
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AttenuationError {
    extent: Extent,
    diagnostic: ExtentDiagnostic,
}

impl AttenuationError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_extent(self) -> Extent {
        self.extent
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MergeError {
    first: Extent,
    second: Extent,
    diagnostic: ExtentDiagnostic,
}

impl MergeError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_extents(self) -> (Extent, Extent) {
        (self.first, self.second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized identity")
    }

    fn rights(identities: &[u64]) -> ExtentRights {
        ExtentRights::from_normalized_identities(
            identities
                .iter()
                .copied()
                .map(|identity| id(identity, ExtentRightId::from_normalized_identity)),
        )
    }

    fn grant(lineage: u64, base: u64, length: u64) -> Extent {
        root_grant(lineage).mint(base, length).expect("root extent")
    }

    fn root_grant(lineage: u64) -> ExtentRootGrant {
        ExtentRootGrant::from_admitted_provider(
            id(lineage, ExtentLineageId::from_normalized_identity),
            id(10, AddressSpaceId::from_normalized_identity),
            rights(&[100, 101]),
            id(20, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
    }

    #[test]
    fn split_and_merge_conserve_one_authority_lineage() {
        let root = grant(1, 0x1000, 0x1000);
        let (lower, upper) = root.split_at(0x400).expect("split");
        assert_eq!((lower.base(), lower.length()), (0x1000, 0x400));
        assert_eq!((upper.base(), upper.length()), (0x1400, 0xc00));

        let restored = upper
            .merge(lower)
            .expect("sibling merge is order independent");
        assert_eq!((restored.base(), restored.length()), (0x1000, 0x1000));
        assert_eq!(restored.lineage_root().normalized_identity(), 1);
    }

    #[test]
    fn nested_children_must_rejoin_before_their_parent_can_merge() {
        let (lower, upper) = grant(1, 0, 100).split_at(40).expect("root split");
        let (lower_a, lower_b) = lower.split_at(10).expect("nested split");

        let error = lower_a.merge(upper).expect_err("not siblings");
        let (lower_a, upper) = (*error).into_extents();
        let lower = lower_a.merge(lower_b).expect("restore lower child");
        let root = lower.merge(upper).expect("restore root");
        assert_eq!((root.base(), root.length()), (0, 100));
    }

    #[test]
    fn adjacency_never_merges_independent_grants() {
        let first = grant(1, 0, 64);
        let second = grant(2, 64, 64);
        let error = first.merge(second).expect_err("adjacency is not authority");
        assert!(error.diagnostic().0.contains("independent authority"));
        let (first, second) = (*error).into_extents();
        assert_eq!(first.length() + second.length(), 128);
    }

    #[test]
    fn attenuation_never_restores_or_widens_rights() {
        let extent = grant(1, 0, 64)
            .attenuate(rights(&[100]))
            .expect("remove write authority");
        let error = extent
            .attenuate(rights(&[100, 101]))
            .expect_err("cannot restore removed right");
        assert!(error.diagnostic().0.contains("cannot add"));
        assert_eq!(error.into_extent().rights(), &rights(&[100]));
    }

    #[test]
    fn incompatible_sibling_facts_cannot_launder_authority() {
        let (lower, upper) = grant(1, 0, 64).split_at(32).expect("split");
        let lower = lower
            .attenuate(rights(&[100]))
            .expect("attenuate lower child");
        let error = lower.merge(upper).expect_err("rights differ");
        assert!(error.diagnostic().0.contains("identical space, rights"));
    }

    #[test]
    fn loans_are_bounded_and_derive_parent_borrow_polarity() {
        let mut extent = grant(1, 0x1000, 64);
        let shared = extent.loan(4, 8).expect("shared loan");
        assert_eq!((shared.base(), shared.length()), (0x1004, 8));
        assert_eq!(shared.polarity(), LoanPolarity::Shared);
        drop(shared);

        let exclusive = extent.loan_mut(16, 8).expect("exclusive loan");
        assert_eq!(exclusive.polarity(), LoanPolarity::Exclusive);
        drop(exclusive);

        assert!(extent.loan(60, 8).is_err());
    }

    #[test]
    fn failed_split_returns_the_original_authority() {
        let extent = grant(1, 0, 64);
        let error = extent.split_at(64).expect_err("empty upper child");
        assert_eq!(error.into_extent().length(), 64);
    }

    #[test]
    fn failed_mint_returns_the_admitted_root_grant() {
        let error = root_grant(1).mint(u64::MAX, 2).expect_err("overflow");
        assert!(error.diagnostic().0.contains("overflows"));
        let extent = error.into_grant().mint(0, 64).expect("retry valid mint");
        assert_eq!(extent.length(), 64);
    }

    fn external_grant(
        direction: ExternalLoanDirection,
        completion: CompletionObligations,
    ) -> ExternalLoanGrant {
        ExternalLoanGrant::from_admitted_provider(
            id(500, ExternalBorrowerId::from_normalized_identity),
            direction,
            id(10, AddressSpaceId::from_normalized_identity),
            id(20, ExtentProvenanceId::from_normalized_identity),
            rights(&[100]),
            completion,
        )
    }

    fn loan_id(identity: u64) -> ExternalLoanId {
        id(identity, ExternalLoanId::from_normalized_identity)
    }

    fn completion_fact(identity: u64) -> ExternalCompletionFactId {
        id(identity, ExternalCompletionFactId::from_normalized_identity)
    }

    fn reach_receipt(
        loan: ExternalLoanId,
        direction: ExternalLoanDirection,
        base: u64,
        length: u64,
        mechanism: ExternalReachMechanism,
    ) -> ExternalReachReceipt {
        ExternalReachReceipt::from_admitted_provider(
            id(800, ExternalReachReceiptId::from_normalized_identity),
            loan,
            id(500, ExternalBorrowerId::from_normalized_identity),
            direction,
            id(10, AddressSpaceId::from_normalized_identity),
            id(20, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
            base,
            length,
            mechanism,
            true,
        )
    }

    #[test]
    fn external_read_loan_requires_completion_facts_before_releasing_borrow() {
        let extent = grant(1, 0x1000, 64);
        let loan = extent.loan(0, 32).expect("shared DMA source");
        let transfer = begin_external_loan(
            loan,
            loan_id(600),
            &external_grant(
                ExternalLoanDirection::DeviceReads,
                CompletionObligations::from_normalized_facts([
                    completion_fact(700),
                    completion_fact(701),
                ]),
            ),
            Some(reach_receipt(
                loan_id(600),
                ExternalLoanDirection::DeviceReads,
                0x1000,
                32,
                ExternalReachMechanism::AdmittedBorrowerContract,
            )),
        )
        .expect("admitted external read");
        assert_eq!(transfer.direction(), ExternalLoanDirection::DeviceReads);

        let incomplete = ExternalCompletionReceipt::from_admitted_provider(
            &transfer,
            true,
            [completion_fact(701)],
        );
        let error = transfer
            .complete(incomplete)
            .expect_err("required device fence is missing");
        assert!(error.diagnostic().0.contains("lacks facts"));
        let (transfer, _) = (*error).into_parts();

        let complete = ExternalCompletionReceipt::from_admitted_provider(
            &transfer,
            true,
            [completion_fact(700), completion_fact(701)],
        );
        let completion = transfer.complete(complete).expect("completed DMA read");
        assert_eq!((completion.base, completion.length), (0x1000, 32));
    }

    #[test]
    fn external_write_loan_derives_exclusive_cpu_exclusion() {
        let mut extent = grant(1, 0x2000, 64);
        let shared = extent.loan(0, 16).expect("shared loan");
        let error = begin_external_loan(
            shared,
            loan_id(601),
            &external_grant(
                ExternalLoanDirection::DeviceWrites,
                CompletionObligations::default(),
            ),
            Some(reach_receipt(
                loan_id(601),
                ExternalLoanDirection::DeviceWrites,
                0x2000,
                16,
                ExternalReachMechanism::HardwareIsolation,
            )),
        )
        .expect_err("device mutation needs exclusive custody");
        assert!(error.diagnostic().0.contains("exclusive"));
        drop((*error).into_loan());

        let exclusive = extent.loan_mut(0, 16).expect("exclusive loan");
        let transfer = begin_external_loan(
            exclusive,
            loan_id(602),
            &external_grant(
                ExternalLoanDirection::DeviceWrites,
                CompletionObligations::default(),
            ),
            Some(reach_receipt(
                loan_id(602),
                ExternalLoanDirection::DeviceWrites,
                0x2000,
                16,
                ExternalReachMechanism::HardwareIsolation,
            )),
        )
        .expect("admitted external write");
        let receipt = ExternalCompletionReceipt::from_admitted_provider(&transfer, true, []);
        let completion = transfer.complete(receipt).expect("completed DMA write");
        assert_eq!(completion.borrower.normalized_identity(), 500);
        assert_eq!(completion.reach_receipt.normalized_identity(), 800);
    }

    #[test]
    fn external_agent_reach_must_equal_the_lent_extent_and_fail_closed() {
        let extent = grant(1, 0x3000, 128);
        let loan = extent.loan(32, 32).expect("DMA subrange");
        let missing = begin_external_loan(
            loan,
            loan_id(603),
            &external_grant(
                ExternalLoanDirection::DeviceReads,
                CompletionObligations::default(),
            ),
            None,
        )
        .expect_err("an invisible borrower without reach evidence must reject");
        assert!(missing.diagnostic().0.contains("reach evidence"));
        let loan = (*missing).into_loan();

        let overbroad = begin_external_loan(
            loan,
            loan_id(603),
            &external_grant(
                ExternalLoanDirection::DeviceReads,
                CompletionObligations::default(),
            ),
            Some(reach_receipt(
                loan_id(603),
                ExternalLoanDirection::DeviceReads,
                0x3000,
                128,
                ExternalReachMechanism::HardwareIsolation,
            )),
        )
        .expect_err("whole-parent reach exceeds the exact lent subrange");
        assert!(overbroad.diagnostic().0.contains("exact lent extent range"));
        let loan = (*overbroad).into_loan();

        let transfer = begin_external_loan(
            loan,
            loan_id(603),
            &external_grant(
                ExternalLoanDirection::DeviceReads,
                CompletionObligations::default(),
            ),
            Some(reach_receipt(
                loan_id(603),
                ExternalLoanDirection::DeviceReads,
                0x3020,
                32,
                ExternalReachMechanism::HardwareIsolation,
            )),
        )
        .expect("exact subrange isolation");
        assert_eq!((transfer.base(), transfer.length()), (0x3020, 32));
        assert_eq!(
            transfer.reach_mechanism(),
            ExternalReachMechanism::HardwareIsolation
        );
    }

    #[test]
    fn external_completion_cannot_replay_after_lent_authority_drift() {
        let first_extent = grant(1, 0x4000, 64);
        let first = begin_external_loan(
            first_extent.loan(0, 16).expect("first DMA range"),
            loan_id(604),
            &external_grant(
                ExternalLoanDirection::DeviceReads,
                CompletionObligations::default(),
            ),
            Some(reach_receipt(
                loan_id(604),
                ExternalLoanDirection::DeviceReads,
                0x4000,
                16,
                ExternalReachMechanism::HardwareIsolation,
            )),
        )
        .expect("first external loan");
        let stale = ExternalCompletionReceipt::from_admitted_provider(&first, true, []);

        let second_extent = grant(2, 0x5000, 64);
        let second = begin_external_loan(
            second_extent.loan(0, 16).expect("second DMA range"),
            loan_id(604),
            &external_grant(
                ExternalLoanDirection::DeviceReads,
                CompletionObligations::default(),
            ),
            Some(reach_receipt(
                loan_id(604),
                ExternalLoanDirection::DeviceReads,
                0x5000,
                16,
                ExternalReachMechanism::HardwareIsolation,
            )),
        )
        .expect("second external loan");

        let error = second
            .complete(stale)
            .expect_err("completion for the prior range must not replay");
        assert!(error.diagnostic().0.contains("lent extent range"));
    }
}
