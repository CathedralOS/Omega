//! Full inert selected-provider policy, sharing one bounded writer.

use crate::record::{
    PackagePolicyProviderFamily, PackagePolicyProviderPlan, PackagePolicyProviderRow,
    PackagePolicySelectedProviders, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderGrantSelectorKind, PackageReviewProviderSelectionAuthority,
};
mod authority;
mod bindings;
mod service;
mod signature;
pub(super) use service::method as encode_service_method;

use super::{
    encoder::{Encoder, encode_canonical_component},
    values::{identity::encode_nominal, providers::encode_compiler_intrinsic_execution},
};
use crate::encoding::{
    PACKAGE_SELECTED_PROVIDER_POLICY_VERSION, PackageReviewEncodingError,
    SELECTED_PROVIDER_POLICY_MAGIC,
};

impl PackagePolicySelectedProviders {
    encode_canonical_component!(
        SELECTED_PROVIDER_POLICY_MAGIC,
        PACKAGE_SELECTED_PROVIDER_POLICY_VERSION,
        policy,
    );
}

pub(super) fn policy(
    encoder: &mut Encoder,
    policy: &PackagePolicySelectedProviders,
) -> Result<(), PackageReviewEncodingError> {
    encoder.package_identity_field("package", policy.package)?;
    encoder.field("target", |encoder| {
        encoder.string(policy.target.identity().as_str())
    })?;
    encoder.field("plans", |encoder| encoder.sequence(&policy.plans, plan))?;
    encoder.field("families", |encoder| {
        encoder.sequence(&policy.families, family)
    })
}

fn plan(
    encoder: &mut Encoder,
    plan: &PackagePolicyProviderPlan,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("plan_name", |encoder| encoder.string(&plan.plan_name))?;
    encoder.optional_package_identity_field("realizing_package", plan.realizing_package)?;
    encoder.field_by(
        "schema_declaration",
        &plan.schema_declaration,
        encode_nominal,
    )?;
    encoder.field("provider_type", |encoder| {
        encoder.string(&plan.provider_type)
    })?;
    encoder.field("provider_type_declaration", |encoder| {
        encoder.option(plan.provider_type_declaration.as_ref(), encode_nominal)
    })?;
    encoder.field("target", |encoder| encoder.string(&plan.target))?;
    encoder.field("methods", |encoder| {
        encoder.sequence(&plan.methods, service::method)
    })?;
    encoder.field("rows", |encoder| encoder.sequence(&plan.rows, row))?;
    encoder.field("grants", |encoder| {
        encoder.sequence(&plan.grants, |encoder, grant| {
            encoder.field("grant", |encoder| {
                match grant {
                    PackageReviewProviderGrantSelectorKind::PlanName => encoder.tag("plan_name", 0),
                    PackageReviewProviderGrantSelectorKind::ProviderSlot => {
                        encoder.tag("provider_slot", 1)
                    }
                };
                Ok(())
            })?;
            Ok(())
        })
    })
}

fn row(
    encoder: &mut Encoder,
    row: &PackagePolicyProviderRow,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("method", |encoder| encoder.string(&row.method))?;
    encoder.field_by("requirement", &row.requirement, encode_nominal)?;
    encoder.field("realization", |encoder| {
        encoder.option(row.realization.as_ref(), encode_nominal)
    })?;
    encoder.field("requirement_lifetime_partition", |encoder| {
        encoder.sequence(&row.requirement_lifetime_partition, |encoder, ordinal| {
            encoder.u32_field("ordinal", *ordinal)?;
            Ok(())
        })
    })?;
    encoder.field_by("binding", &row.binding, bindings::binding)?;
    encoder.field("compiler_intrinsic_execution", |encoder| {
        encoder.option(
            row.compiler_intrinsic_execution.as_ref(),
            encode_compiler_intrinsic_execution,
        )
    })?;
    encoder.field("installation_reach", |encoder| {
        encoder.option(row.installation_reach.as_ref(), |encoder, reach| {
            encoder.field("upper_bound", |encoder| {
                encoder.sequence(&reach.upper_bound, encode_nominal)
            })?;
            encoder.field("resolved", |encoder| {
                encoder.sequence(&reach.resolved, encode_nominal)
            })
        })
    })?;
    encoder.u64_field("admitted_same_stack_bytes", row.admitted_same_stack_bytes)
}

fn family(
    encoder: &mut Encoder,
    family: &PackagePolicyProviderFamily,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("family_identity", &family.family_identity, encode_nominal)?;
    encoder.field_by(
        "provider_type_declaration",
        &family.provider_type_declaration,
        encode_nominal,
    )?;
    encoder.field("target", |encoder| {
        encoder.string(family.target.identity().as_str())
    })?;
    encoder.field("authority", |encoder| {
        match family.authority {
            PackageReviewProviderSelectionAuthority::BuildOverride => {
                encoder.tag("build_override", 0)
            }
            PackageReviewProviderSelectionAuthority::TargetDefault => {
                encoder.tag("target_default", 1)
            }
        };
        Ok(())
    })?;
    encoder.field("coverage", |encoder| {
        match family.coverage {
            PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily => {
                encoder.tag("complete_declaration_family", 0)
            }
        };
        Ok(())
    })?;
    encoder.field("coordinates", |encoder| {
        encoder.sequence(&family.coordinates, |encoder, coordinate| {
            encoder.field("requirement_identity", |encoder| {
                encoder.string(&coordinate.requirement_identity)
            })?;
            encoder.field_by(
                "operator_declaration",
                &coordinate.operator_declaration,
                encode_nominal,
            )?;
            encoder.u32_field("plan_index", coordinate.plan_index)?;
            Ok(())
        })
    })
}
