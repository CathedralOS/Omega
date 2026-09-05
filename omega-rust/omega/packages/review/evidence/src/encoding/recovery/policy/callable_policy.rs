//! Typed callable recovery under one bounded policy reader.

mod behavior;
#[cfg(test)]
mod budgets;
#[cfg(test)]
mod signatures;
#[cfg(test)]
mod tests;

use super::{
    Error, PackagePolicyRecoveryLimits,
    behavior::synchronous_invocation,
    contracts::callable_contract,
    identity::{nominal, operator_coordinate, package, type_identity},
    reader::Reader,
    signatures::{conformance_bound, type_parameter},
};
use crate::encoding::{CALLABLE_POLICY_MAGIC, PACKAGE_CALLABLE_POLICY_VERSION};
use crate::record::*;

impl PackagePolicyCallables {
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        let mut reader = Reader::new(bytes, limits)?;
        reader.literal(CALLABLE_POLICY_MAGIC)?;
        if reader.u16()? != PACKAGE_CALLABLE_POLICY_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let package = package(&mut reader)?;
        let target_identity = reader.string()?;
        let target = omega_target::TargetProfile::ALL
            .into_iter()
            .find(|target| target.identity().as_str() == target_identity)
            .ok_or(Error::InvalidValue)?;
        let policy = Self {
            package,
            target,
            callables: reader.sequence(1, callable)?,
        };
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

pub(super) fn callable(reader: &mut Reader<'_>) -> Result<PackagePolicyCallable, Error> {
    Ok(PackagePolicyCallable {
        role: match reader.byte()? {
            0 => PackagePolicyCallableRole::Boundary,
            1 => PackagePolicyCallableRole::Public,
            2 => PackagePolicyCallableRole::Build,
            3 => PackagePolicyCallableRole::PrivateAssumption,
            _ => return Err(Error::InvalidTag),
        },
        identity: nominal(reader)?,
        supply: supply(reader)?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        conformance_bounds: reader.sequence(1, conformance_bound)?,
        parameters: reader.sequence(19, |reader| {
            Ok(PackageReviewCallableParameter {
                name: reader.string()?,
                type_identity: type_identity(reader)?,
                is_const: reader.boolean()?,
                is_mutable: reader.boolean()?,
                is_self: reader.boolean()?,
            })
        })?,
        return_type: reader.option(type_identity)?,
        conformances: reader.sequence(1, callable_conformance)?,
        operator_realizations: reader.sequence(1, |reader| {
            Ok(PackageReviewOperatorRealization {
                coordinate: operator_coordinate(reader)?,
                alias: reader.option(|reader| reader.string())?,
            })
        })?,
        contracts: reader.sequence(4, callable_contract)?,
        declared_service_reach: reader.option(|reader| reader.sequence(41, nominal))?,
        checked_service_reach: match reader.byte()? {
            0 => PackageReviewCheckedServiceReach::NoCheckedBody,
            1 => PackageReviewCheckedServiceReach::CheckedBody {
                realized: reader.sequence(41, nominal)?,
                concrete: reader.sequence(41, nominal)?,
            },
            _ => return Err(Error::InvalidTag),
        },
        unresolved_installation_reaches: reader.sequence(49, |reader| {
            Ok(PackageReviewInstallationReach {
                requirement: nominal(reader)?,
                upper_bound: reader.sequence(41, nominal)?,
            })
        })?,
        declared_synchronous_invocations: reader
            .option(|reader| reader.sequence(1, synchronous_invocation))?,
        realized_synchronous_invocations: reader.sequence(1, synchronous_invocation)?,
        capability_flows: reader.sequence(42, behavior::capability)?,
        reachable_capability_flows: reader.sequence(42, behavior::capability)?,
        declared_may_suspend: reader.option(|reader| reader.boolean())?,
        declared_may_block: reader.option(|reader| reader.boolean())?,
        declared_termination: reader.option(behavior::termination)?,
        checked_may_suspend: reader.boolean()?,
        checked_may_block: reader.boolean()?,
        checked_termination: behavior::termination(reader)?,
        checked_crash: behavior::crash(reader)?,
        mutation: behavior::mutation(reader)?,
    })
}

fn supply(reader: &mut Reader<'_>) -> Result<PackageReviewCallableSupply, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewCallableSupply::CheckedBody,
        1 => PackageReviewCallableSupply::Requirement,
        2 => PackageReviewCallableSupply::Boundary,
        3 => PackageReviewCallableSupply::AdmissionClaim,
        4 => PackageReviewCallableSupply::ExternalRealization,
        5 => PackageReviewCallableSupply::TopLevelRequirement,
        _ => return Err(Error::InvalidTag),
    })
}

fn callable_conformance(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallableConformance, Error> {
    Ok(PackagePolicyCallableConformance {
        trait_identity: nominal(reader)?,
        requirement_identity: nominal(reader)?,
        requirement_lifetime_partition: reader.sequence(4, |reader| reader.u32())?,
        trait_lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        arguments: reader.sequence(8, type_identity)?,
        alias: reader.option(|reader| reader.string())?,
    })
}
