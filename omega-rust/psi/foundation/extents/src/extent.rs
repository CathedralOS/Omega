//! The extent itself: the opaque authority carrier, its conserved split,
//! partition, attenuation and merge operations, and the errors that hand the
//! authority back on failure.

pub(crate) mod diagnostic;

use crate::extent::diagnostic::ExtentDiagnostic;
use crate::identities::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRights, MappingEraId,
};
use crate::loans::ExtentLoan;
use crate::roots::root_origins::{
    ExtentProgramLocalOrigin, ExtentProviderIssuance, ExtentRootOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SplitBranch {
    Lower,
    Upper,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Lineage {
    pub(crate) root: ExtentLineageId,
    pub(crate) path: Vec<SplitBranch>,
}

/// Whether this extent's backing stays exclusive to one custody lineage or
/// the provider admitted peer aliases the checker cannot see.
///
/// Exclusive claims over the same stable backing descend from one custody
/// root; a writable peer cannot be wished into an exclusive borrow, so a
/// `PeerShared` extent refuses [`Extent::loan_mut`]. Shared reads of hostile
/// backing remain the consumer's copy-and-validate responsibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtentSharingMode {
    Exclusive,
    PeerShared,
}

/// Opaque authority over one concrete address-space range.
///
/// This Rust carrier is deliberately non-`Clone`; Omega's `[linear]` checker
/// supplies the source-language must-consume rule. Every consuming operation
/// below returns the authority on failure rather than silently dropping it.
#[derive(Debug, PartialEq, Eq)]
pub struct Extent {
    pub(crate) base: u64,
    pub(crate) length: u64,
    pub(crate) address_space: AddressSpaceId,
    pub(crate) rights: ExtentRights,
    pub(crate) provenance: ExtentProvenanceId,
    pub(crate) era: MappingEraId,
    pub(crate) origin: ExtentRootOrigin,
    pub(crate) lineage: Lineage,
    pub(crate) sharing: ExtentSharingMode,
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

    pub const fn origin(&self) -> ExtentRootOrigin {
        self.origin
    }

    pub const fn provider_issuance(&self) -> Option<ExtentProviderIssuance> {
        self.origin.provider_issuance()
    }

    pub const fn program_local_origin(&self) -> Option<ExtentProgramLocalOrigin> {
        self.origin.program_local()
    }

    pub const fn lineage_root(&self) -> ExtentLineageId {
        self.lineage.root
    }

    /// Whether the backing may have writable peer aliases outside the
    /// checker's ledger.
    pub const fn sharing(&self) -> ExtentSharingMode {
        self.sharing
    }

    /// Whether this value is the exact unsplit root of its passive lineage.
    /// This is report/retirement structure only; it never establishes origin.
    pub fn is_lineage_root(&self) -> bool {
        self.lineage.path.is_empty()
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

        Ok(self.split_at_validated(lower_length))
    }

    fn split_at_validated(self, lower_length: u64) -> (Self, Self) {
        debug_assert!(lower_length > 0 && lower_length < self.length);

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
            origin: self.origin,
            lineage: Lineage {
                root: self.lineage.root,
                path: lower_path,
            },
            sharing: self.sharing,
        };
        let upper = Self {
            base: upper_base,
            length: upper_length,
            address_space: self.address_space,
            rights: self.rights,
            provenance: self.provenance,
            era: self.era,
            origin: self.origin,
            lineage: Lineage {
                root: self.lineage.root,
                path: upper_path,
            },
            sharing: self.sharing,
        };
        (lower, upper)
    }

    /// Extract one independently owned subextent while retaining every byte
    /// of the parent authority in an explicit conserved partition.
    ///
    /// Borrowed layout/static views should use [`Extent::loan`] instead. This
    /// operation is for an allocation or other subresource that genuinely
    /// leaves the parent's ownership domain.
    pub fn partition_owned(
        self,
        offset: u64,
        length: u64,
    ) -> Result<OwnedExtentPartition, OwnedPartitionError> {
        if length == 0 {
            return Err(OwnedPartitionError {
                extent: self,
                diagnostic: ExtentDiagnostic(
                    "owned subextent must carry nonempty authority".into(),
                ),
            });
        }
        let Some(end) = offset.checked_add(length) else {
            return Err(OwnedPartitionError {
                extent: self,
                diagnostic: ExtentDiagnostic("owned subextent range overflows".into()),
            });
        };
        if end > self.length {
            let parent_length = self.length;
            return Err(OwnedPartitionError {
                extent: self,
                diagnostic: ExtentDiagnostic(format!(
                    "owned subextent {offset}..{end} exceeds {parent_length}-byte parent"
                )),
            });
        }

        let (before, selected, after) = match (offset, end == self.length) {
            (0, true) => (None, self, None),
            (0, false) => {
                let (selected, after) = self.split_at_validated(length);
                (None, selected, Some(after))
            }
            (_, true) => {
                let (before, selected) = self.split_at_validated(offset);
                (Some(before), selected, None)
            }
            (_, false) => {
                let (before, tail) = self.split_at_validated(offset);
                let (selected, after) = tail.split_at_validated(length);
                (Some(before), selected, Some(after))
            }
        };
        Ok(OwnedExtentPartition {
            before,
            selected,
            after,
        })
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
            origin: lower.origin,
            lineage: Lineage {
                root: lower.lineage.root,
                path: parent_path,
            },
            sharing: lower.sharing,
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
        if self.sharing == ExtentSharingMode::PeerShared {
            return Err(ExtentDiagnostic(
                "a writable peer cannot mint an exclusive borrow".into(),
            ));
        }
        ExtentLoan::exclusive(self, offset, length)
    }
}

/// One exact owned extraction and all authority needed to account for its
/// parent. Private fields prevent callers from silently dropping a remainder
/// while inspecting the partition; `into_parts` explicitly transfers every
/// resulting claim.
#[derive(Debug)]
pub struct OwnedExtentPartition {
    before: Option<Extent>,
    selected: Extent,
    after: Option<Extent>,
}

impl OwnedExtentPartition {
    pub const fn before(&self) -> Option<&Extent> {
        self.before.as_ref()
    }

    pub const fn selected(&self) -> &Extent {
        &self.selected
    }

    pub const fn after(&self) -> Option<&Extent> {
        self.after.as_ref()
    }

    pub fn into_parts(self) -> (Option<Extent>, Extent, Option<Extent>) {
        (self.before, self.selected, self.after)
    }

    /// Recompose an unmodified partition into its exact parent authority.
    pub fn rejoin(self) -> Extent {
        let mut restored = self.selected;
        if let Some(after) = self.after {
            restored = restored
                .merge(after)
                .expect("private owned partition retains exact upper sibling");
        }
        if let Some(before) = self.before {
            restored = before
                .merge(restored)
                .expect("private owned partition retains exact lower sibling");
        }
        restored
    }
}

fn validate_merge(first: &Extent, second: &Extent) -> Result<(), ExtentDiagnostic> {
    if first.address_space != second.address_space
        || first.rights != second.rights
        || first.provenance != second.provenance
        || first.era != second.era
        || first.sharing != second.sharing
    {
        return Err(ExtentDiagnostic(
            "merge requires identical space, rights, provenance, era, and sharing".into(),
        ));
    }
    if first.lineage.root != second.lineage.root {
        return Err(ExtentDiagnostic(
            "numeric adjacency cannot merge independent authority lineages".into(),
        ));
    }
    if first.origin != second.origin {
        return Err(ExtentDiagnostic(
            "merge requires identical exact root-origin evidence".into(),
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

#[derive(Debug, PartialEq, Eq)]
pub struct SplitError {
    extent: Extent,
    diagnostic: ExtentDiagnostic,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OwnedPartitionError {
    extent: Extent,
    diagnostic: ExtentDiagnostic,
}

impl OwnedPartitionError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_extent(self) -> Extent {
        self.extent
    }
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
