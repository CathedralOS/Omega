//! Complete callable policy, with no private derivation coordinates.

mod behavior;

use super::{
    declarations::{encode_conformance_bound, encode_type_identity, encode_type_parameter},
    encoder::Encoder,
    values::{
        contracts::encode_callable_contract,
        declarations::encode_operator_coordinate,
        effects::{encode_installation_reach, encode_synchronous_invocation},
        identity::{encode_nominal, encode_supply},
    },
};
use crate::encoding::{
    CALLABLE_POLICY_MAGIC, PACKAGE_CALLABLE_POLICY_VERSION, PackageReviewEncodingError,
};
use crate::record::{
    PackagePolicyCallable, PackagePolicyCallableConformance, PackagePolicyCallableRole,
    PackagePolicyCallables, PackageReviewCheckedServiceReach,
};

impl PackagePolicyCallables {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        self.validate_canonical_structure()
            .map_err(PackageReviewEncodingError::new)?;
        let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
        encoder.fixed_bytes(CALLABLE_POLICY_MAGIC);
        encoder.u16(PACKAGE_CALLABLE_POLICY_VERSION);
        encoder.package_identity(self.package);
        encoder.string(self.target.identity().as_str())?;
        encoder.sequence(&self.callables, encode_callable)?;
        encoder.finish()
    }
}

pub(in crate::encoding) fn encode_callable(
    encoder: &mut Encoder,
    callable: &PackagePolicyCallable,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match callable.role {
        PackagePolicyCallableRole::Boundary => 0,
        PackagePolicyCallableRole::Public => 1,
        PackagePolicyCallableRole::Build => 2,
        PackagePolicyCallableRole::PrivateAssumption => 3,
    });
    encode_nominal(encoder, &callable.identity)?;
    encode_supply(encoder, callable.supply)?;
    encoder.usize(callable.lifetime_parameter_count)?;
    encoder.sequence(&callable.type_parameters, encode_type_parameter)?;
    encoder.sequence(&callable.conformance_bounds, encode_conformance_bound)?;
    encoder.sequence(&callable.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encoder.option(callable.return_type.as_ref(), encode_type_identity)?;
    encoder.sequence(&callable.conformances, encode_callable_conformance)?;
    encoder.sequence(&callable.operator_realizations, |encoder, realization| {
        encode_operator_coordinate(encoder, &realization.coordinate)?;
        encoder.option(realization.alias.as_deref(), |encoder, alias| {
            encoder.string(alias)
        })
    })?;
    encoder.sequence(&callable.contracts, encode_callable_contract)?;
    encoder.option(
        callable.declared_service_reach.as_deref(),
        |encoder, row| encoder.sequence(row, encode_nominal),
    )?;
    match &callable.checked_service_reach {
        PackageReviewCheckedServiceReach::NoCheckedBody => encoder.byte(0),
        PackageReviewCheckedServiceReach::CheckedBody { realized, concrete } => {
            encoder.byte(1);
            encoder.sequence(realized, encode_nominal)?;
            encoder.sequence(concrete, encode_nominal)?;
        }
    }
    encoder.sequence(
        &callable.unresolved_installation_reaches,
        encode_installation_reach,
    )?;
    encoder.option(
        callable.declared_synchronous_invocations.as_deref(),
        |encoder, row| encoder.sequence(row, encode_synchronous_invocation),
    )?;
    encoder.sequence(
        &callable.realized_synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.sequence(&callable.capability_flows, behavior::capability)?;
    encoder.sequence(&callable.reachable_capability_flows, behavior::capability)?;
    encoder.option(callable.declared_may_suspend.as_ref(), |encoder, value| {
        encoder.boolean(*value);
        Ok(())
    })?;
    encoder.option(callable.declared_may_block.as_ref(), |encoder, value| {
        encoder.boolean(*value);
        Ok(())
    })?;
    encoder.option(
        callable.declared_termination.as_ref(),
        behavior::termination,
    )?;
    encoder.boolean(callable.checked_may_suspend);
    encoder.boolean(callable.checked_may_block);
    behavior::termination(encoder, &callable.checked_termination)?;
    behavior::crash(encoder, &callable.checked_crash)?;
    behavior::mutation(encoder, &callable.mutation)
}

fn encode_callable_conformance(
    encoder: &mut Encoder,
    conformance: &PackagePolicyCallableConformance,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &conformance.trait_identity)?;
    encode_nominal(encoder, &conformance.requirement_identity)?;
    encoder.sequence(
        &conformance.requirement_lifetime_partition,
        |encoder, ordinal| {
            encoder.u32(*ordinal);
            Ok(())
        },
    )?;
    encoder.sequence(&conformance.trait_lifetime_arguments, |encoder, ordinal| {
        encoder.u32(*ordinal);
        Ok(())
    })?;
    encoder.sequence(&conformance.arguments, encode_type_identity)?;
    encoder.option(conformance.alias.as_deref(), |encoder, alias| {
        encoder.string(alias)
    })
}
