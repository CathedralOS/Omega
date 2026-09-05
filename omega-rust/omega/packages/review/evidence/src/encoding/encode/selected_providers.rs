//! Full inert selected-provider policy, sharing one bounded writer.

mod authority;
mod bindings;
mod service;
mod signature;
pub(super) use service::method as encode_service_method;

use super::{
    encoder::Encoder,
    values::{identity::encode_nominal, providers::encode_compiler_intrinsic_execution},
};
use crate::encoding::{
    PACKAGE_SELECTED_PROVIDER_POLICY_VERSION, PackageReviewEncodingError,
    SELECTED_PROVIDER_POLICY_MAGIC,
};
use crate::record::*;

impl PackagePolicySelectedProviders {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        self.validate_canonical_structure()
            .map_err(PackageReviewEncodingError::new)?;
        let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
        encoder.fixed_bytes(SELECTED_PROVIDER_POLICY_MAGIC);
        encoder.u16(PACKAGE_SELECTED_PROVIDER_POLICY_VERSION);
        policy(&mut encoder, self)?;
        encoder.finish()
    }
}

pub(super) fn policy(
    encoder: &mut Encoder,
    policy: &PackagePolicySelectedProviders,
) -> Result<(), PackageReviewEncodingError> {
    encoder.package_identity(policy.package);
    encoder.string(policy.target.identity().as_str())?;
    encoder.sequence(&policy.plans, plan)?;
    encoder.sequence(&policy.families, family)
}

fn plan(
    encoder: &mut Encoder,
    plan: &PackagePolicyProviderPlan,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&plan.plan_name)?;
    encoder.optional_package_identity(plan.realizing_package);
    encode_nominal(encoder, &plan.schema_declaration)?;
    encoder.string(&plan.provider_type)?;
    encoder.option(plan.provider_type_declaration.as_ref(), encode_nominal)?;
    encoder.string(&plan.target)?;
    encoder.sequence(&plan.methods, service::method)?;
    encoder.sequence(&plan.rows, row)?;
    encoder.sequence(&plan.grants, |encoder, grant| {
        encoder.byte(match grant {
            PackageReviewProviderGrantSelectorKind::PlanName => 0,
            PackageReviewProviderGrantSelectorKind::ProviderSlot => 1,
        });
        Ok(())
    })
}

fn row(
    encoder: &mut Encoder,
    row: &PackagePolicyProviderRow,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&row.method)?;
    encode_nominal(encoder, &row.requirement)?;
    encode_nominal(encoder, &row.realization)?;
    encoder.sequence(&row.requirement_lifetime_partition, |encoder, ordinal| {
        encoder.u32(*ordinal);
        Ok(())
    })?;
    bindings::binding(encoder, &row.binding)?;
    encoder.option(
        row.compiler_intrinsic_execution.as_ref(),
        encode_compiler_intrinsic_execution,
    )?;
    encoder.option(row.installation_reach.as_ref(), |encoder, reach| {
        encoder.sequence(&reach.upper_bound, encode_nominal)?;
        encoder.sequence(&reach.resolved, encode_nominal)
    })
}

fn family(
    encoder: &mut Encoder,
    family: &PackagePolicyProviderFamily,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &family.family_identity)?;
    encode_nominal(encoder, &family.provider_type_declaration)?;
    encoder.string(family.target.identity().as_str())?;
    encoder.byte(match family.authority {
        PackageReviewProviderSelectionAuthority::BuildOverride => 0,
        PackageReviewProviderSelectionAuthority::TargetDefault => 1,
    });
    encoder.byte(match family.coverage {
        PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily => 0,
    });
    encoder.sequence(&family.coordinates, |encoder, coordinate| {
        encoder.string(&coordinate.requirement_identity)?;
        encode_nominal(encoder, &coordinate.operator_declaration)?;
        encoder.u32(coordinate.plan_index);
        Ok(())
    })
}
