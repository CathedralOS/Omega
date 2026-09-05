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
    encoder.field("package", |encoder| {
        encoder.package_identity(policy.package);
        Ok(())
    })?;
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
    encoder.field("realizing_package", |encoder| {
        encoder.optional_package_identity(plan.realizing_package);
        Ok(())
    })?;
    encoder.field("schema_declaration", |encoder| {
        encode_nominal(encoder, &plan.schema_declaration)
    })?;
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
    encoder.field("requirement", |encoder| {
        encode_nominal(encoder, &row.requirement)
    })?;
    encoder.field("realization", |encoder| {
        encode_nominal(encoder, &row.realization)
    })?;
    encoder.field("requirement_lifetime_partition", |encoder| {
        encoder.sequence(&row.requirement_lifetime_partition, |encoder, ordinal| {
            encoder.field("ordinal", |encoder| {
                encoder.u32(*ordinal);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("binding", |encoder| {
        bindings::binding(encoder, &row.binding)
    })?;
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
    })
}

fn family(
    encoder: &mut Encoder,
    family: &PackagePolicyProviderFamily,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("family_identity", |encoder| {
        encode_nominal(encoder, &family.family_identity)
    })?;
    encoder.field("provider_type_declaration", |encoder| {
        encode_nominal(encoder, &family.provider_type_declaration)
    })?;
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
            encoder.field("operator_declaration", |encoder| {
                encode_nominal(encoder, &coordinate.operator_declaration)
            })?;
            encoder.field("plan_index", |encoder| {
                encoder.u32(coordinate.plan_index);
                Ok(())
            })?;
            Ok(())
        })
    })
}
