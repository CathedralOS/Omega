//! Borrow-carrying subrange loans whose polarity derives from the parent
//! borrow.

use crate::extents::extent::Extent;
use crate::extents::extent::diagnostic::ExtentDiagnostic;
use crate::extents::identities::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRights, MappingEraId,
};
use crate::extents::roots::root_origins::{
    ExtentProgramLocalOrigin, ExtentProviderIssuance, ExtentRootOrigin,
};

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
    pub(crate) fn shared(
        extent: &'a Extent,
        offset: u64,
        length: u64,
    ) -> Result<Self, ExtentDiagnostic> {
        let base = validate_subrange(extent, offset, length)?;
        Ok(Self {
            backing: LoanBacking::Shared(extent),
            base,
            length,
        })
    }

    pub(crate) fn exclusive(
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

    pub const fn origin(&self) -> ExtentRootOrigin {
        match &self.backing {
            LoanBacking::Shared(extent) => extent.origin,
            LoanBacking::Exclusive(extent) => extent.origin,
        }
    }

    pub const fn provider_issuance(&self) -> Option<ExtentProviderIssuance> {
        self.origin().provider_issuance()
    }

    pub const fn program_local_origin(&self) -> Option<ExtentProgramLocalOrigin> {
        self.origin().program_local()
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
