use extents::{
    AddressSpaceId, Extent, ExtentLineageId, ExtentLoan, ExtentProvenanceId, ExtentRights,
    ExtentRootOrigin, MappingEraId,
};

use super::{
    AccessPlanDiagnostic, BoundaryReach, ResourceProfile, ValidatedResourceProfile,
    validate_resource_profile,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceProfileReceiptId(u64);

impl ResourceProfileReceiptId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
        if identity == 0 {
            return Err(AccessPlanDiagnostic(
                "resource-profile receipt identity cannot be zero".into(),
            ));
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Provider-only authority to bind one normalized profile to one exact range
/// and provenance tuple.
#[derive(Debug)]
pub struct ResourceProfileGrant {
    receipt: ResourceProfileReceiptId,
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    origin: ExtentRootOrigin,
    lineage_root: ExtentLineageId,
    required_rights: ExtentRights,
    permitted_reach: BoundaryReach,
}

impl ResourceProfileGrant {
    /// Bind provider supply to one exact granted Extent authority account.
    ///
    /// Taking the opaque Extent instead of a restated geometry/provenance
    /// tuple prevents a profile receipt from being replayed against a
    /// coincident but independently introduced root.
    pub fn from_admitted_provider(
        receipt: ResourceProfileReceiptId,
        extent: &Extent,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        Self::from_bound_extent(
            receipt,
            extent.base(),
            extent.length(),
            extent.address_space(),
            extent.provenance(),
            extent.era(),
            extent.origin(),
            extent.lineage_root(),
            required_rights,
            permitted_reach,
        )
    }

    /// Bind provider supply directly to the exact qualified subrange loan
    /// that will feed placement admission.
    pub fn from_admitted_provider_loan(
        receipt: ResourceProfileReceiptId,
        loan: &ExtentLoan<'_>,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        Self::from_bound_extent(
            receipt,
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.provenance(),
            loan.era(),
            loan.origin(),
            loan.lineage_root(),
            required_rights,
            permitted_reach,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_bound_extent(
        receipt: ResourceProfileReceiptId,
        base: u64,
        length: u64,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        origin: ExtentRootOrigin,
        lineage_root: ExtentLineageId,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        if length == 0 {
            return Err(AccessPlanDiagnostic(
                "resource-profile grant cannot bind an empty range".into(),
            ));
        }
        base.checked_add(length)
            .ok_or_else(|| AccessPlanDiagnostic("resource-profile grant range overflows".into()))?;
        Ok(Self {
            receipt,
            base,
            length,
            address_space,
            provenance,
            era,
            origin,
            lineage_root,
            required_rights,
            permitted_reach,
        })
    }

    pub fn admit(
        self,
        profile: ResourceProfile,
    ) -> Result<AdmittedResourceProfile, ResourceProfileAdmissionError> {
        let validated = match validate_resource_profile(profile.clone(), self.length) {
            Ok(validated) => validated,
            Err(diagnostic) => {
                return Err(ResourceProfileAdmissionError {
                    grant: Box::new(self),
                    profile,
                    diagnostic,
                });
            }
        };
        if let Some(region) = validated
            .regions
            .iter()
            .find(|region| !self.permitted_reach.contains_all(&region.reach))
        {
            return Err(ResourceProfileAdmissionError {
                grant: Box::new(self),
                profile,
                diagnostic: AccessPlanDiagnostic(format!(
                    "resource region {}..{} claims reach outside the provider grant",
                    region.offset,
                    region.offset + region.length
                )),
            });
        }
        Ok(AdmittedResourceProfile {
            receipt: self.receipt,
            base: self.base,
            length: self.length,
            address_space: self.address_space,
            provenance: self.provenance,
            era: self.era,
            origin: self.origin,
            lineage_root: self.lineage_root,
            required_rights: self.required_rights,
            permitted_reach: self.permitted_reach,
            profile: validated,
        })
    }
}

#[derive(Debug)]
pub struct ResourceProfileAdmissionError {
    grant: Box<ResourceProfileGrant>,
    profile: ResourceProfile,
    diagnostic: AccessPlanDiagnostic,
}

impl ResourceProfileAdmissionError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ResourceProfileGrant, ResourceProfile, AccessPlanDiagnostic) {
        (*self.grant, self.profile, self.diagnostic)
    }
}

#[derive(Debug, Clone)]
pub struct AdmittedResourceProfile {
    receipt: ResourceProfileReceiptId,
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    origin: ExtentRootOrigin,
    lineage_root: ExtentLineageId,
    required_rights: ExtentRights,
    permitted_reach: BoundaryReach,
    profile: ValidatedResourceProfile,
}

impl AdmittedResourceProfile {
    pub const fn receipt(&self) -> ResourceProfileReceiptId {
        self.receipt
    }

    pub const fn profile(&self) -> &ValidatedResourceProfile {
        &self.profile
    }

    pub(super) fn restrict_to_loan(
        &self,
        loan: &ExtentLoan<'_>,
    ) -> Result<ValidatedResourceProfile, AccessPlanDiagnostic> {
        if loan.address_space() != self.address_space {
            return Err(AccessPlanDiagnostic(
                "extent address space does not match admitted resource profile".into(),
            ));
        }
        if loan.provenance() != self.provenance {
            return Err(AccessPlanDiagnostic(
                "extent provenance does not match admitted resource profile".into(),
            ));
        }
        if loan.era() != self.era {
            return Err(AccessPlanDiagnostic(
                "extent mapping era does not match admitted resource profile".into(),
            ));
        }
        if loan.origin() != self.origin {
            return Err(AccessPlanDiagnostic(
                "extent sealed root origin does not match admitted resource profile".into(),
            ));
        }
        if loan.lineage_root() != self.lineage_root {
            return Err(AccessPlanDiagnostic(
                "extent root lineage does not match admitted resource profile".into(),
            ));
        }
        if !loan.rights().contains(&self.required_rights) {
            return Err(AccessPlanDiagnostic(
                "extent lacks rights bound into the admitted resource profile".into(),
            ));
        }
        let offset = loan.base().checked_sub(self.base).ok_or_else(|| {
            AccessPlanDiagnostic(
                "extent loan begins before the admitted resource-profile range".into(),
            )
        })?;
        let end = offset.checked_add(loan.length()).ok_or_else(|| {
            AccessPlanDiagnostic("extent loan range overflows resource profile".into())
        })?;
        if end > self.length {
            return Err(AccessPlanDiagnostic(format!(
                "extent loan relative range {offset}..{end} exceeds {}-byte admitted resource profile",
                self.length
            )));
        }
        self.profile
            .restrict(offset, loan.length(), &self.permitted_reach)
    }
}
