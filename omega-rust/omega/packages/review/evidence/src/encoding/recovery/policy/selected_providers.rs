//! Full typed selected-provider recovery with one aggregate reader budget.

mod authority;
#[cfg(test)]
mod authority_tests;
#[cfg(test)]
mod binding_tests;
mod bindings;
#[cfg(test)]
mod budgets;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
pub(super) use fixtures::complete as row_fixture;
#[cfg(test)]
pub(super) use fixtures::method as service_method_fixture;
mod service;
#[cfg(test)]
mod service_tests;
mod signature;
pub(super) use service::method as service_method;
#[cfg(test)]
mod tests;

use super::{
    Error, PackagePolicyRecoveryLimits,
    identity::{nominal, package},
    intrinsic,
    reader::Reader,
};
use crate::encoding::{PACKAGE_SELECTED_PROVIDER_POLICY_VERSION, SELECTED_PROVIDER_POLICY_MAGIC};
use crate::record::*;

impl PackagePolicySelectedProviders {
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        let mut reader = Reader::new(bytes, limits)?;
        reader.literal(SELECTED_PROVIDER_POLICY_MAGIC)?;
        if reader.u16()? != PACKAGE_SELECTED_PROVIDER_POLICY_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let policy = policy(&mut reader)?;
        reader.finish()?;
        policy
            .validate_canonical_structure()
            .map_err(|_| Error::InvalidValue)?;
        reader.canonical_scratch(bytes.len())?;
        if policy
            .canonical_bytes()
            .map_err(|_| Error::NonCanonicalEncoding)?
            != bytes
        {
            return Err(Error::NonCanonicalEncoding);
        }
        Ok(policy)
    }
}

pub(super) fn policy(reader: &mut Reader<'_>) -> Result<PackagePolicySelectedProviders, Error> {
    Ok(PackagePolicySelectedProviders {
        package: package(reader)?,
        target: target(reader)?,
        plans: reader.sequence(1, plan)?,
        families: reader.sequence(1, family)?,
    })
}

pub(super) fn target(reader: &mut Reader<'_>) -> Result<omega_target::TargetProfile, Error> {
    let identity = reader.string()?;
    omega_target::TargetProfile::ALL
        .into_iter()
        .find(|profile| profile.identity().as_str() == identity)
        .ok_or(Error::InvalidValue)
}

fn plan(reader: &mut Reader<'_>) -> Result<PackagePolicyProviderPlan, Error> {
    Ok(PackagePolicyProviderPlan {
        plan_name: reader.string()?,
        realizing_package: reader.option(package)?,
        schema_declaration: nominal(reader)?,
        provider_type: reader.string()?,
        provider_type_declaration: reader.option(nominal)?,
        target: reader.string()?,
        methods: reader.sequence(1, service::method)?,
        rows: reader.sequence(1, row)?,
        grants: reader.sequence(1, |reader| {
            Ok(match reader.byte()? {
                0 => PackageReviewProviderGrantSelectorKind::PlanName,
                1 => PackageReviewProviderGrantSelectorKind::ProviderSlot,
                _ => return Err(Error::InvalidTag),
            })
        })?,
    })
}

fn row(reader: &mut Reader<'_>) -> Result<PackagePolicyProviderRow, Error> {
    Ok(PackagePolicyProviderRow {
        method: reader.string()?,
        requirement: nominal(reader)?,
        realization: nominal(reader)?,
        requirement_lifetime_partition: reader.sequence(4, Reader::u32)?,
        binding: bindings::binding(reader)?,
        compiler_intrinsic_execution: reader.option(intrinsic::execution)?,
        installation_reach: reader.option(|reader| {
            Ok(PackageReviewSelectedInstallationReach {
                upper_bound: reader.sequence(41, nominal)?,
                resolved: reader.sequence(41, nominal)?,
            })
        })?,
    })
}

fn family(reader: &mut Reader<'_>) -> Result<PackagePolicyProviderFamily, Error> {
    Ok(PackagePolicyProviderFamily {
        family_identity: nominal(reader)?,
        provider_type_declaration: nominal(reader)?,
        target: target(reader)?,
        authority: match reader.byte()? {
            0 => PackageReviewProviderSelectionAuthority::BuildOverride,
            1 => PackageReviewProviderSelectionAuthority::TargetDefault,
            _ => return Err(Error::InvalidTag),
        },
        coverage: match reader.byte()? {
            0 => PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily,
            _ => return Err(Error::InvalidTag),
        },
        coordinates: reader.sequence(53, |reader| {
            Ok(PackagePolicyProviderFamilyCoordinate {
                requirement_identity: reader.string()?,
                operator_declaration: nominal(reader)?,
                plan_index: reader.u32()?,
            })
        })?,
    })
}
