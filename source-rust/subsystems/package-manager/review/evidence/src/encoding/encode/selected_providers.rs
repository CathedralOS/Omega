// Full inert selected-provider policy encoding sharing one bounded writer.

use super::encoder::Encoder;
use super::values::external_policy::encode_producer;
use super::values::identity::encode_nominal;
use super::values::providers::encode_compiler_intrinsic_execution;
use crate::encoding::encode::public_api::{formal, type_parameter as encode_type_parameter};
use crate::encoding::encode::values::effects::encode_synchronous_invocation;
use crate::encoding::encode::values::identity::encode_service;
use crate::encoding::{
    PACKAGE_SELECTED_PROVIDER_POLICY_VERSION, PackageReviewEncodingError,
    SELECTED_PROVIDER_POLICY_MAGIC,
};
use crate::record::{
    PackagePolicyProviderFamily, PackagePolicyProviderPlan, PackagePolicyProviderRow,
    PackagePolicySelectedProviders, PackagePolicyServiceAuthority, PackagePolicyServiceSignature,
    PackageReviewProviderFamilyCoverage, PackageReviewProviderGrantSelectorKind,
    PackageReviewProviderSelectionAuthority,
};
pub(super) use bindings::binding as encode_binding;
pub(super) use service::method as encode_service_method;
use service_surface::{ServiceProgressEstablishmentRouteKind, ServiceProgressSubject};
mod bindings;
mod service;

impl PackagePolicySelectedProviders {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        let version = if self.has_application_grants() {
            crate::encoding::PROVIDER_POLICY_WITH_APPLICATION_GRANTS
        } else {
            crate::encoding::PROVIDER_POLICY_WITHOUT_APPLICATION_GRANTS
        };
        Encoder::versioned_policy_bytes(
            SELECTED_PROVIDER_POLICY_MAGIC,
            version,
            PACKAGE_SELECTED_PROVIDER_POLICY_VERSION,
            self,
            policy,
        )
    }
}

pub(super) fn policy(
    encoder: &mut Encoder,
    policy: &PackagePolicySelectedProviders,
) -> Result<(), PackageReviewEncodingError> {
    encoder.package_identity_field("package", policy.package)?;
    encoder.string_field("target", policy.target.identity().as_str())?;
    encoder.field("plans", |encoder| encoder.sequence(&policy.plans, plan))?;
    encoder.sequence_field("families", &policy.families, family)
}

fn plan(
    encoder: &mut Encoder,
    plan: &PackagePolicyProviderPlan,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("plan_name", |encoder| encoder.string(&plan.plan_name))?;
    encoder.optional_package_identity_field("realizing_package", plan.realizing_package)?;
    encoder.field("schema_declaration", |encoder| {
        super::values::identity::encode_service_receiver(
            encoder,
            &plan.schema_declaration,
            plan.schema_application.as_ref(),
        )
    })?;
    encoder.string_field("provider_type", &plan.provider_type)?;
    encoder.option_field(
        "provider_type_declaration",
        plan.provider_type_declaration.as_ref(),
        encode_nominal,
    )?;
    encoder.field("target", |encoder| encoder.string(&plan.target))?;
    encoder.sequence_field("methods", &plan.methods, service::method)?;
    encoder.field("rows", |encoder| encoder.sequence(&plan.rows, row))?;
    encoder.field("grants", |encoder| {
        encoder.sequence(&plan.grants, |encoder, grant| {
            encoder.field("grant", |encoder| {
                match grant {
                    PackageReviewProviderGrantSelectorKind::PlanName => encoder.tag("plan_name", 0),
                    PackageReviewProviderGrantSelectorKind::ProviderSlot => {
                        encoder.tag("provider_slot", 1)
                    }
                    PackageReviewProviderGrantSelectorKind::ProviderApplication(application) => {
                        encoder.tag("provider_application", 2);
                        encoder.option_field(
                            "application",
                            application.as_ref(),
                            |encoder, value| encoder.string(value),
                        )?;
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
    encoder.option_field(
        "requirement_application",
        row.requirement_application.as_ref(),
        |encoder, application| encoder.string(application),
    )?;
    encoder.option_field("realization", row.realization.as_ref(), encode_nominal)?;
    encoder.option_field("native_build", row.native_build.as_ref(), encode_producer)?;
    encoder.field("requirement_lifetime_partition", |encoder| {
        encoder.sequence(&row.requirement_lifetime_partition, |encoder, ordinal| {
            encoder.u32_field("ordinal", *ordinal)?;
            Ok(())
        })
    })?;
    encoder.field_by("binding", &row.binding, bindings::binding)?;
    encoder.option_field(
        "compiler_intrinsic_execution",
        row.compiler_intrinsic_execution.as_ref(),
        encode_compiler_intrinsic_execution,
    )?;
    encoder.field("installation_reach", |encoder| {
        encoder.option(row.installation_reach.as_ref(), |encoder, reach| {
            encoder.sequence_field("upper_bound", &reach.upper_bound, encode_service)?;
            encoder.sequence_field("resolved", &reach.resolved, encode_service)
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
    encoder.string_field("target", family.target.identity().as_str())?;
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
            encoder.string_field("requirement_identity", &coordinate.requirement_identity)?;
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

// Canonical encoding for service-method operational authority, invocations, and progress premises.

fn authority(
    encoder: &mut Encoder,
    authority: &PackagePolicyServiceAuthority,
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence_field("service_reach", &authority.service_reach, encode_service)?;
    encoder.field("synchronous_invocations", |encoder| {
        encoder.sequence(
            &authority.synchronous_invocations,
            encode_synchronous_invocation,
        )
    })?;
    encoder.field("progress_premises", |encoder| {
        encoder.sequence(&authority.progress_premises, |encoder, premise| {
            encoder.field_by("profile", &premise.profile, encode_nominal)?;
            encoder.field("subject", |encoder| {
                match premise.subject {
                    ServiceProgressSubject::Parameter(position) => {
                        encoder.tag("parameter", 1);
                        encoder.field("position", |encoder| encoder.usize(position))?;
                    }
                };
                Ok(())
            })?;
            encoder.field("subject_projections", |encoder| {
                encoder.sequence(&premise.subject_projections, encode_nominal)
            })?;
            encoder.field("establishment_routes", |encoder| {
                encoder.sequence(&premise.establishment_routes, |encoder, route| {
                    encoder.field("kind", |encoder| {
                        match route.kind {
                            ServiceProgressEstablishmentRouteKind::CheckedRequirement => {
                                encoder.tag("checked_requirement", 0)
                            }
                            ServiceProgressEstablishmentRouteKind::BoundaryRequirement => {
                                encoder.tag("boundary_requirement", 1)
                            }
                        };
                        Ok(())
                    })?;
                    encoder.field_by(
                        "requirement_owner",
                        &route.requirement_owner,
                        encode_nominal,
                    )?;
                    encoder.field_by("requirement", &route.requirement, encode_nominal)
                })
            })
        })
    })
}

// Canonical encoding for selected-provider service signatures, schema arguments, and formals.

fn signature(
    encoder: &mut Encoder,
    signature: &PackagePolicyServiceSignature,
) -> Result<(), PackageReviewEncodingError> {
    let lifetime_count = signature
        .schema_lifetime_parameter_count
        .checked_add(signature.application_lifetime_parameter_count)
        .and_then(|count| count.checked_add(signature.requirement_lifetime_parameter_count))
        .ok_or_else(|| {
            PackageReviewEncodingError::new("selected service lifetime scope overflows")
        })? as usize;
    encoder.field("schema_arguments", |encoder| {
        encoder.sequence(&signature.schema_arguments, Encoder::static_argument)
    })?;
    encoder.u32_field(
        "schema_lifetime_parameter_count",
        signature.schema_lifetime_parameter_count,
    )?;
    encoder.field("schema_lifetime_arguments", |encoder| {
        encoder.sequence(&signature.schema_lifetime_arguments, |encoder, ordinal| {
            encoder.u32(*ordinal);
            Ok(())
        })
    })?;
    encoder.u32_field(
        "application_lifetime_parameter_count",
        signature.application_lifetime_parameter_count,
    )?;
    encoder.field("requirement_arguments", |encoder| {
        encoder.sequence(&signature.requirement_arguments, Encoder::static_argument)
    })?;
    encoder.field("requirement_lifetime_arguments", |encoder| {
        encoder.sequence(
            &signature.requirement_lifetime_arguments,
            |encoder, ordinal| {
                encoder.u32_field("ordinal", *ordinal)?;
                Ok(())
            },
        )
    })?;
    encoder.u32_field(
        "requirement_lifetime_parameter_count",
        signature.requirement_lifetime_parameter_count,
    )?;
    encoder.field("static_parameters", |encoder| {
        encoder.sequence(&signature.static_parameters, encode_type_parameter)
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(&signature.parameters, |encoder, parameter| {
            formal(
                encoder,
                &parameter.name,
                &parameter.type_identity,
                parameter.is_const,
                parameter.is_mutable,
                lifetime_count,
            )
        })
    })?;
    encoder.option_field("result", signature.result.as_ref(), |encoder, identity| {
        super::type_identity_in_scope(encoder, identity, Some(lifetime_count))
    })
}
