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
}
