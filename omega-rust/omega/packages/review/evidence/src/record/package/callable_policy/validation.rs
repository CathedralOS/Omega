//! Structural policy consistency, not compiler proof or acceptance authority.

mod behavior;
pub(in crate::record) mod signature;
pub(in crate::record) mod signature_contracts;
pub(in crate::record) mod signature_expressions;
mod signature_parameters;
mod structural;

use super::*;
use crate::record::*;

impl PackagePolicyCallables {
    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        if self
            .callables
            .windows(2)
            .any(|pair| pair[0].identity >= pair[1].identity)
        {
            return Err("callable policy identities are repeated or out of order");
        }
        if self
            .callables
            .iter()
            .filter(|callable| callable.role == PackagePolicyCallableRole::Build)
            .count()
            > 1
        {
            return Err("callable policy contains multiple selected builds");
        }
        for callable in &self.callables {
            nominal(&callable.identity)?;
            if callable.identity.owner != PackageReviewNominalOwner::Package(self.package) {
                return Err("callable policy contains a foreign-owned callable");
            }
            if callable.role == PackagePolicyCallableRole::PrivateAssumption
                && callable.supply != PackageReviewCallableSupply::AdmissionClaim
            {
                return Err("private assumption role has another callable supply");
            }
            if callable.role == PackagePolicyCallableRole::PrivateExternal
                && callable.supply != PackageReviewCallableSupply::ExternalRealization
            {
                return Err("private external role has another callable supply");
            }
            let boundary_supply = matches!(
                callable.supply,
                PackageReviewCallableSupply::Boundary
                    | PackageReviewCallableSupply::TopLevelRequirement
                    | PackageReviewCallableSupply::AdmissionClaim
            );
            if (callable.role == PackagePolicyCallableRole::Boundary && !boundary_supply)
                || (callable.role == PackagePolicyCallableRole::Public && boundary_supply)
            {
                return Err("callable role differs from its boundary supply classification");
            }
            if matches!(
                callable.role,
                PackagePolicyCallableRole::Public
                    | PackagePolicyCallableRole::Boundary
                    | PackagePolicyCallableRole::PrivateAssumption
                    | PackagePolicyCallableRole::PrivateExternal
            ) && (callable.declared_service_reach.is_none()
                || callable.declared_synchronous_invocations.is_none()
                || callable.declared_may_suspend.is_none()
                || callable.declared_may_block.is_none())
            {
                return Err("published callable policy lacks a published authority ceiling");
            }
            if matches!(
                callable.role,
                PackagePolicyCallableRole::Boundary
                    | PackagePolicyCallableRole::PrivateAssumption
                    | PackagePolicyCallableRole::PrivateExternal
            ) && callable.declared_termination.is_none()
            {
                return Err("bodyless or boundary callable lacks its termination interface");
            }
            if callable.role == PackagePolicyCallableRole::Public
                && !callable.unresolved_installation_reaches.is_empty()
            {
                return Err("ordinary public callable retains unresolved installation authority");
            }
            if (callable.declared_may_suspend == Some(false) && callable.checked_may_suspend)
                || (callable.declared_may_block == Some(false) && callable.checked_may_block)
            {
                return Err("checked callable behavior exceeds its declared ceiling");
            }
            match (&callable.checked_service_reach, callable.supply) {
                (
                    PackageReviewCheckedServiceReach::NoCheckedBody,
                    PackageReviewCallableSupply::CheckedBody,
                ) => return Err("checked callable supply has no checked body reach"),
                (PackageReviewCheckedServiceReach::CheckedBody { .. }, supply)
                    if !matches!(
                        supply,
                        PackageReviewCallableSupply::CheckedBody
                            | PackageReviewCallableSupply::Boundary
                    ) =>
                {
                    return Err("bodyless callable supply carries checked body reach");
                }
                _ => {}
            }
            ordered(&callable.conformances)?;
            ordered(&callable.operator_realizations)?;
            signature::validate(callable)?;
            behavior::validate(callable)?;
        }
        Ok(())
    }
}

fn nominal(identity: &PackageReviewNominalIdentity) -> Result<(), &'static str> {
    if identity.path.is_empty() || identity.owner == PackageReviewNominalOwner::Unresolved {
        return Err("callable policy nominal is empty or unresolved");
    }
    Ok(())
}

fn ordered<T: Ord>(values: &[T]) -> Result<(), &'static str> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("callable policy set is repeated or out of order");
    }
    Ok(())
}

fn nominal_set(values: &[PackageReviewNominalIdentity]) -> Result<(), &'static str> {
    ordered(values)?;
    for value in values {
        nominal(value)?;
    }
    Ok(())
}
