//! Full typed selected-provider recovery with one aggregate reader budget.

use crate::record::{
    PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate, PackagePolicyProviderPlan,
    PackagePolicyProviderRow, PackagePolicySelectedProviders, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderGrantSelectorKind, PackageReviewProviderSelectionAuthority,
    PackageReviewSelectedInstallationReach,
};
pub(in crate::encoding::recovery) mod authority;
pub(in crate::encoding::recovery) mod bindings;
pub(in crate::encoding::recovery) mod service;
pub(in crate::encoding::recovery) mod signature;
pub(in crate::encoding::recovery) use service::method as service_method;

use super::{
    Error, PackagePolicyRecoveryLimits,
    identity::{nominal, package},
    intrinsic,
    reader::Reader,
};
use crate::encoding::{PACKAGE_SELECTED_PROVIDER_POLICY_VERSION, SELECTED_PROVIDER_POLICY_MAGIC};

impl PackagePolicySelectedProviders {
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        super::recover_component(
            bytes,
            limits,
            SELECTED_PROVIDER_POLICY_MAGIC,
            PACKAGE_SELECTED_PROVIDER_POLICY_VERSION,
            policy,
            |policy| {
                policy
                    .validate_canonical_structure()
                    .map_err(|_| Error::InvalidValue)
            },
            Self::canonical_bytes,
        )
    }
}

pub(in crate::encoding::recovery) fn policy(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicySelectedProviders, Error> {
    Ok(PackagePolicySelectedProviders {
        package: package(reader)?,
        target: target(reader)?,
        plans: reader.sequence(1, plan)?,
        families: reader.sequence(1, family)?,
    })
}

pub(in crate::encoding::recovery) fn target(
    reader: &mut Reader<'_>,
) -> Result<target::TargetProfile, Error> {
    let identity = reader.string()?;
    target::TargetProfile::ALL
        .into_iter()
        .find(|profile| profile.identity().as_str() == identity)
        .ok_or(Error::InvalidValue)
}

pub(in crate::encoding::recovery) fn plan(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyProviderPlan, Error> {
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

pub(in crate::encoding::recovery) fn row(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyProviderRow, Error> {
    Ok(PackagePolicyProviderRow {
        method: reader.string()?,
        requirement: nominal(reader)?,
        realization: reader.option(nominal)?,
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

pub(in crate::encoding::recovery) fn family(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyProviderFamily, Error> {
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
