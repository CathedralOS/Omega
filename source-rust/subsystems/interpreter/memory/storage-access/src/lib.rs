//! Bind physical resource profiles to the exact admitted live extent and loan.
//! Failures return the grant; restriction conserves its scope, lineage and rights.
#![forbid(unsafe_code)]
use extent_contracts::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRights, MappingEraId,
};
use extents::{Extent, ExtentLoan, ExtentRootOrigin};

use access_admission::ValidatedResourceProfile;
use access_admission::validate_resource_profile;
use access_vocabulary::AccessPlanDiagnostic;
use access_vocabulary::BoundaryReach;
use access_vocabulary::ResourceProfile;
use access_vocabulary::ResourceProfileReceiptId;

#[derive(Debug, Clone)]
struct BoundProfileAuthority {
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

#[derive(Debug)]
pub struct ResourceProfileGrant(BoundProfileAuthority);

trait ExtentMetadata {
    fn base(&self) -> u64;
    fn length(&self) -> u64;
    fn address_space(&self) -> AddressSpaceId;
    fn provenance(&self) -> ExtentProvenanceId;
    fn era(&self) -> MappingEraId;
    fn origin(&self) -> ExtentRootOrigin;
    fn lineage_root(&self) -> ExtentLineageId;
}

impl ExtentMetadata for Extent {
    fn base(&self) -> u64 {
        Extent::base(self)
    }
    fn length(&self) -> u64 {
        Extent::length(self)
    }
    fn address_space(&self) -> AddressSpaceId {
        Extent::address_space(self)
    }
    fn provenance(&self) -> ExtentProvenanceId {
        Extent::provenance(self)
    }
    fn era(&self) -> MappingEraId {
        Extent::era(self)
    }
    fn origin(&self) -> ExtentRootOrigin {
        Extent::origin(self)
    }
    fn lineage_root(&self) -> ExtentLineageId {
        Extent::lineage_root(self)
    }
}

impl ExtentMetadata for ExtentLoan<'_> {
    fn base(&self) -> u64 {
        ExtentLoan::base(self)
    }
    fn length(&self) -> u64 {
        ExtentLoan::length(self)
    }
    fn address_space(&self) -> AddressSpaceId {
        ExtentLoan::address_space(self)
    }
    fn provenance(&self) -> ExtentProvenanceId {
        ExtentLoan::provenance(self)
    }
    fn era(&self) -> MappingEraId {
        ExtentLoan::era(self)
    }
    fn origin(&self) -> ExtentRootOrigin {
        ExtentLoan::origin(self)
    }
    fn lineage_root(&self) -> ExtentLineageId {
        ExtentLoan::lineage_root(self)
    }
}

impl ResourceProfileGrant {
    fn from_carrier(
        receipt: ResourceProfileReceiptId,
        carrier: &impl ExtentMetadata,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        carrier
            .base()
            .checked_add(carrier.length())
            .ok_or_else(|| AccessPlanDiagnostic("resource-profile grant range overflows".into()))?;
        Ok(Self(BoundProfileAuthority {
            receipt,
            base: carrier.base(),
            length: carrier.length(),
            address_space: carrier.address_space(),
            provenance: carrier.provenance(),
            era: carrier.era(),
            origin: carrier.origin(),
            lineage_root: carrier.lineage_root(),
            required_rights,
            permitted_reach,
        }))
    }

    pub fn from_admitted_provider(
        receipt: ResourceProfileReceiptId,
        extent: &Extent,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        Self::from_carrier(receipt, extent, required_rights, permitted_reach)
    }

    pub fn from_admitted_provider_loan(
        receipt: ResourceProfileReceiptId,
        loan: &ExtentLoan<'_>,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        Self::from_carrier(receipt, loan, required_rights, permitted_reach)
    }

    pub fn admit(
        self,
        profile: ResourceProfile,
    ) -> Result<AdmittedResourceProfile, ResourceProfileAdmissionError> {
        let validated = match validate_resource_profile(profile.clone(), self.0.length) {
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
            .regions()
            .iter()
            .find(|region| !self.0.permitted_reach.contains_all(&region.reach))
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
            authority: self.0,
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
    authority: BoundProfileAuthority,
    profile: ValidatedResourceProfile,
}

impl AdmittedResourceProfile {
    pub const fn receipt(&self) -> ResourceProfileReceiptId {
        self.authority.receipt
    }

    pub const fn profile(&self) -> &ValidatedResourceProfile {
        &self.profile
    }

    pub fn initialization_compatibility(
        &self,
        loan: &ExtentLoan<'_>,
        plan: &access_admission::ValidatedPlacement,
    ) -> Result<access_admission::InitializationResourceCompatibility, AccessPlanDiagnostic> {
        if loan.polarity() != extent_contracts::LoanPolarity::Exclusive {
            return Err(AccessPlanDiagnostic(
                "initialization requires an exclusive extent loan".into(),
            ));
        }
        let restricted = self.restrict_to_loan(loan)?;
        let compatibility = access_admission::validate_initialization_resources(plan, &restricted)?;
        let base = compatibility.placement().base_congruence();
        if !base.admits(loan.base()) {
            return Err(AccessPlanDiagnostic(format!(
                "initialization base {} must satisfy base mod {} = {}",
                loan.base(),
                base.modulus(),
                base.residue()
            )));
        }
        Ok(compatibility)
    }

    pub fn restrict_to_loan(
        &self,
        loan: &ExtentLoan<'_>,
    ) -> Result<ValidatedResourceProfile, AccessPlanDiagnostic> {
        let auth = &self.authority;
        for (mismatched, message) in [
            (
                loan.address_space() != auth.address_space,
                "extent address space does not match admitted resource profile",
            ),
            (
                loan.provenance() != auth.provenance,
                "extent provenance does not match admitted resource profile",
            ),
            (
                loan.era() != auth.era,
                "extent mapping era does not match admitted resource profile",
            ),
            (
                loan.origin() != auth.origin,
                "extent sealed root origin does not match admitted resource profile",
            ),
            (
                loan.lineage_root() != auth.lineage_root,
                "extent root lineage does not match admitted resource profile",
            ),
            (
                !loan.rights().contains(&auth.required_rights),
                "extent lacks rights bound into the admitted resource profile",
            ),
        ] {
            if mismatched {
                return Err(AccessPlanDiagnostic(message.into()));
            }
        }
        let offset = loan.base().checked_sub(auth.base).ok_or_else(|| {
            AccessPlanDiagnostic(
                "extent loan begins before the admitted resource-profile range".into(),
            )
        })?;
        let end = offset.checked_add(loan.length()).ok_or_else(|| {
            AccessPlanDiagnostic("extent loan range overflows resource profile".into())
        })?;
        if end > auth.length {
            return Err(AccessPlanDiagnostic(format!(
                "extent loan relative range {offset}..{end} exceeds {}-byte admitted resource profile",
                auth.length
            )));
        }
        self.profile
            .restrict(offset, loan.length(), &auth.permitted_reach)
    }
}
#[cfg(test)]
mod tests;

mod authorization;
pub use authorization::{AccessAuthorization, AuthorizedFieldAccess};
