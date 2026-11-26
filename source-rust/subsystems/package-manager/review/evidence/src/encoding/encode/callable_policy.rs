// Canonical encoding for complete callable policy rows without private derivation coordinates.

use super::declarations::encode_type_identity;
use super::encoder::Encoder;
use super::public_api::type_parameter as encode_type_parameter;
use super::values::encode_boolean_expression;
use super::values::expressions::encode_contract_expression;
use super::values::identity::encode_nominal;
use crate::encoding::encode::declarations::encode_conformance_bound;
use crate::encoding::encode::type_identity_in_scope;
use crate::encoding::encode::values::contracts::encode_callable_contract;
use crate::encoding::encode::values::declarations::{
    encode_operator_coordinate, operator_spelling_name, operator_spelling_tag,
};
use crate::encoding::encode::values::effects::{
    encode_installation_reach, encode_synchronous_invocation,
};
use crate::encoding::encode::values::identity::{
    encode_service, encode_supply, encode_target_context,
};
use crate::encoding::{
    CALLABLE_POLICY_MAGIC, PACKAGE_CALLABLE_POLICY_VERSION, PackageReviewEncodingError,
};
use crate::record::{
    PackagePolicyCallable, PackagePolicyCallableConformance, PackagePolicyCallableRole,
    PackagePolicyCallables, PackagePolicyCapabilityFlow, PackagePolicyCrash,
    PackagePolicyCrashGuard, PackagePolicyCrashRoute, PackagePolicyInferredCrash,
    PackagePolicyMutation, PackagePolicyTermination, PackageReviewCheckedServiceReach,
    PackageReviewCrashCause, PackageReviewCrashInterface, PackageReviewProgressSubject,
    PackageReviewWriteFrameCompleteness,
};

impl PackagePolicyCallables {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        Encoder::versioned_policy_bytes(
            CALLABLE_POLICY_MAGIC,
            crate::encoding::CALLABLE_POLICY_WITH_NOMINAL_SERVICES,
            PACKAGE_CALLABLE_POLICY_VERSION,
            self,
            policy,
        )
    }
}

pub(super) fn policy(
    encoder: &mut Encoder,
    policy: &PackagePolicyCallables,
) -> Result<(), PackageReviewEncodingError> {
    encoder.package_identity_field("package", policy.package)?;
    encoder.string_field("target", policy.target.identity().as_str())?;
    encoder.sequence_field("callables", &policy.callables, encode_callable)
}

pub(super) fn encode_callable_conformance(
    encoder: &mut Encoder,
    conformance: &PackagePolicyCallableConformance,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("trait", &conformance.trait_identity, encode_nominal)?;
    encoder.field_by(
        "requirement",
        &conformance.requirement_identity,
        encode_nominal,
    )?;
    encoder.field("requirement_lifetime_partition", |encoder| {
        encoder.sequence(
            &conformance.requirement_lifetime_partition,
            |encoder, ordinal| {
                encoder.u32(*ordinal);
                Ok(())
            },
        )
    })?;
    encoder.field("trait_lifetime_arguments", |encoder| {
        encoder.sequence(&conformance.trait_lifetime_arguments, |encoder, ordinal| {
            encoder.u32(*ordinal);
            Ok(())
        })
    })?;
    encoder.sequence_field("arguments", &conformance.arguments, encode_type_identity)?;
    encoder.field("alias", |encoder| {
        encoder.option(conformance.alias.as_deref(), |encoder, alias| {
            encoder.string(alias)
        })
    })
}

// Complete named callable row encoding sharing the enclosing policy budget.

pub(in crate::encoding) fn encode_callable(
    encoder: &mut Encoder,
    callable: &PackagePolicyCallable,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("role", |encoder| {
        match callable.role {
            PackagePolicyCallableRole::Boundary => encoder.tag("boundary", 0),
            PackagePolicyCallableRole::Public => encoder.tag("public", 1),
            PackagePolicyCallableRole::Build => encoder.tag("build", 2),
            PackagePolicyCallableRole::PrivateAssumption => encoder.tag("private_assumption", 3),
            PackagePolicyCallableRole::PrivateExternal => encoder.tag("private_external", 4),
        }
        Ok(())
    })?;
    encoder.field_by("identity", &callable.identity, encode_nominal)?;
    encoder.field("target_context", |encoder| {
        encode_target_context(encoder, &callable.target_context)
    })?;
    encoder.field("supply", |encoder| encode_supply(encoder, callable.supply))?;
    encoder.field("spelling", |encoder| {
        encoder.option(callable.spelling.as_ref(), |encoder, spelling| {
            encoder.tag(
                operator_spelling_name(*spelling),
                operator_spelling_tag(*spelling),
            );
            Ok(())
        })
    })?;
    encoder.usize_field(
        "lifetime_parameter_count",
        callable.lifetime_parameter_count,
    )?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&callable.type_parameters, encode_type_parameter)
    })?;
    encoder.field("conformance_bounds", |encoder| {
        encoder.sequence(&callable.conformance_bounds, encode_conformance_bound)
    })?;
    let signature_type = |encoder: &mut Encoder, identity: &_| {
        type_identity_in_scope(encoder, identity, Some(callable.lifetime_parameter_count))
    };
    encoder.field("parameters", |encoder| {
        encoder.sequence(&callable.parameters, |encoder, parameter| {
            encoder.field("name", |encoder| encoder.string(&parameter.name))?;
            encoder.field_by("type", &parameter.type_identity, signature_type)?;
            encoder.boolean_field("const", parameter.is_const)?;
            encoder.boolean_field("mutable", parameter.is_mutable)?;
            Ok(())
        })
    })?;
    encoder.option_field("return_type", callable.return_type.as_ref(), signature_type)?;
    encoder.field("conformances", |encoder| {
        encoder.sequence(&callable.conformances, encode_callable_conformance)
    })?;
    encoder.field("operator_realizations", |encoder| {
        encoder.sequence(&callable.operator_realizations, |encoder, realization| {
            encoder.field_by(
                "coordinate",
                &realization.coordinate,
                encode_operator_coordinate,
            )?;
            encoder.field("alias", |encoder| {
                encoder.option(realization.alias.as_deref(), |encoder, alias| {
                    encoder.string(alias)
                })
            })
        })
    })?;
    encoder.field("requirement_realizations", |encoder| {
        encoder.sequence(
            &callable.requirement_realizations,
            |encoder, realization| {
                encoder.field_by(
                    "requirement_identity",
                    &realization.requirement_identity,
                    encode_nominal,
                )?;
                encoder.field("alias", |encoder| {
                    encoder.option(realization.alias.as_deref(), |encoder, alias| {
                        encoder.string(alias)
                    })
                })
            },
        )
    })?;
    encoder.sequence_field("contracts", &callable.contracts, encode_callable_contract)?;
    encoder.field("declared_service_reach", |encoder| {
        encoder.option(
            callable.declared_service_reach.as_deref(),
            |encoder, row| encoder.sequence(row, encode_service),
        )
    })?;
    encoder.field("service_reach_dependency", |encoder| {
        encoder.field("concrete", |encoder| {
            encoder.sequence(&callable.service_reach_dependency.concrete, encode_service)
        })?;
        encoder.field("parameters", |encoder| {
            encoder.sequence(
                &callable.service_reach_dependency.parameters,
                |encoder, ordinal| {
                    encoder.u32(*ordinal);
                    Ok(())
                },
            )
        })
    })?;
    encoder.field("checked_service_reach", |encoder| {
        match &callable.checked_service_reach {
            PackageReviewCheckedServiceReach::NoCheckedBody => encoder.tag("no_checked_body", 0),
            PackageReviewCheckedServiceReach::CheckedBody { realized, concrete } => {
                encoder.tag("checked_body", 1);
                encoder.sequence_field("realized", realized, encode_service)?;
                encoder.sequence_field("concrete", concrete, encode_service)?;
            }
        }
        Ok(())
    })?;
    encoder.field("unresolved_installation_reaches", |encoder| {
        encoder.sequence(
            &callable.unresolved_installation_reaches,
            encode_installation_reach,
        )
    })?;
    encoder.field("declared_synchronous_invocations", |encoder| {
        encoder.option(
            callable.declared_synchronous_invocations.as_deref(),
            |encoder, row| encoder.sequence(row, encode_synchronous_invocation),
        )
    })?;
    encoder.field("realized_synchronous_invocations", |encoder| {
        encoder.sequence(
            &callable.realized_synchronous_invocations,
            encode_synchronous_invocation,
        )
    })?;
    encoder.field("capability_flows", |encoder| {
        encoder.sequence(&callable.capability_flows, capability)
    })?;
    encoder.field("reachable_capability_flows", |encoder| {
        encoder.sequence(&callable.reachable_capability_flows, capability)
    })?;
    encoder.field("declared_may_suspend", |encoder| {
        encoder.option(callable.declared_may_suspend.as_ref(), |encoder, value| {
            encoder.boolean(*value);
            Ok(())
        })
    })?;
    encoder.field("declared_may_block", |encoder| {
        encoder.option(callable.declared_may_block.as_ref(), |encoder, value| {
            encoder.boolean(*value);
            Ok(())
        })
    })?;
    encoder.option_field(
        "declared_termination",
        callable.declared_termination.as_ref(),
        termination,
    )?;
    encoder.boolean_field("checked_may_suspend", callable.checked_may_suspend)?;
    encoder.boolean_field("checked_may_block", callable.checked_may_block)?;
    encoder.field_by(
        "checked_termination",
        &callable.checked_termination,
        termination,
    )?;
    encoder.field_by("checked_crash", &callable.checked_crash, crash)?;
    encoder.field_by("mutation", &callable.mutation, mutation)
}

// Normalized operational facts (capabilities, crashes, mutations, and termination) sharing the enclosing callable writer budget.

fn capability(
    encoder: &mut Encoder,
    flow: &PackagePolicyCapabilityFlow,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("capability", &flow.capability, encode_nominal)?;
    encoder.field("kind", |encoder| {
        match flow.kind {
            psi_effect_contracts::CapabilityFlowKind::Uses => encoder.tag("uses", 0),
            psi_effect_contracts::CapabilityFlowKind::Returns => encoder.tag("returns", 1),
            psi_effect_contracts::CapabilityFlowKind::Acquires => encoder.tag("acquires", 2),
            psi_effect_contracts::CapabilityFlowKind::Stores => encoder.tag("stores", 3),
            psi_effect_contracts::CapabilityFlowKind::Derives => encoder.tag("derives", 4),
        }
        Ok(())
    })
}

fn crash(
    encoder: &mut Encoder,
    crash: &PackagePolicyCrash,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("interface", |encoder| {
        match crash.interface {
            PackageReviewCrashInterface::InternalInferred => encoder.tag("internal_inferred", 0),
            PackageReviewCrashInterface::PublishedCeiling => encoder.tag("published_ceiling", 1),
        }
        Ok(())
    })?;
    encoder.sequence_field("published", &crash.published, crash_route)?;
    encoder.field("structural_runtime_requirements", |encoder| {
        encoder.option(
            crash.structural_runtime_requirements.as_deref(),
            |encoder, requirements| encoder.sequence(requirements, encode_boolean_expression),
        )
    })?;
    encoder.field("inferred", |encoder| {
        match &crash.inferred {
            PackagePolicyInferredCrash::Unknown => encoder.tag("unknown", 0),
            PackagePolicyInferredCrash::Complete { causes } => {
                encoder.tag("complete", 1);
                encoder.field("causes", |encoder| {
                    encoder.sequence(causes, |encoder, cause| {
                        match cause {
                            PackageReviewCrashCause::Trap => encoder.tag("trap", 0),
                            PackageReviewCrashCause::Abort => encoder.tag("abort", 1),
                        }
                        Ok(())
                    })
                })?;
            }
        }
        Ok(())
    })
}

pub(in crate::encoding) fn crash_route(
    encoder: &mut Encoder,
    route: &PackagePolicyCrashRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("cause", |encoder| {
        match route.cause {
            PackageReviewCrashCause::Trap => encoder.tag("trap", 0),
            PackageReviewCrashCause::Abort => encoder.tag("abort", 1),
        }
        Ok(())
    })?;
    encoder.field("alternative_guards", |encoder| {
        encoder.sequence(&route.alternative_guards, |encoder, guard| {
            match guard {
                PackagePolicyCrashGuard::Truth => encoder.tag("truth", 0),
                PackagePolicyCrashGuard::Expression(expression) => {
                    encoder.tag("expression", 1);
                    encoder.field_by("expression", expression, encode_contract_expression)?;
                }
            }
            Ok(())
        })
    })
}

fn mutation(
    encoder: &mut Encoder,
    mutation: &PackagePolicyMutation,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("completeness", |encoder| {
        match mutation.completeness {
            PackageReviewWriteFrameCompleteness::Complete => encoder.tag("complete", 0),
            PackageReviewWriteFrameCompleteness::Opaque => encoder.tag("opaque", 1),
        }
        Ok(())
    })?;
    encoder.field("paths", |encoder| {
        encoder.sequence(&mutation.paths, |encoder, path| encoder.string(path))
    })
}

pub(in crate::encoding) fn termination(
    encoder: &mut Encoder,
    termination: &PackagePolicyTermination,
) -> Result<(), PackageReviewEncodingError> {
    match termination {
        PackagePolicyTermination::NoGuarantee => encoder.tag("no_guarantee", 0),
        PackagePolicyTermination::Terminates { premises } => {
            encoder.tag("terminates", 1);
            encoder.field("premises", |encoder| {
                encoder.sequence(premises, |encoder, premise| {
                    encoder.field("profile", |encoder| {
                        encode_nominal(encoder, &premise.profile)
                    })?;
                    encoder.field("subject", |encoder| {
                        match &premise.subject {
                            PackageReviewProgressSubject::Declaration(identity) => {
                                encoder.tag("declaration", 0);
                                encoder.field("identity", |encoder| {
                                    encode_nominal(encoder, identity)
                                })?;
                            }
                            PackageReviewProgressSubject::Parameter(position) => {
                                encoder.tag("parameter", 2);
                                encoder.field("position", |encoder| {
                                    encoder.u32(*position);
                                    Ok(())
                                })?;
                            }
                        }
                        Ok(())
                    })?;
                    encoder.field("projections", |encoder| {
                        encoder.sequence(&premise.projections, encode_nominal)
                    })?;
                    encoder.field("establishment_routes", |encoder| {
                        encoder.sequence(&premise.establishment_routes, |encoder, route| {
                            use service_surface::ServiceProgressEstablishmentRouteKind as Kind;
                            encoder.field("kind", |encoder| {
                                match route.kind {
                                    Kind::CheckedRequirement => {
                                        encoder.tag("checked_requirement", 0)
                                    }
                                    Kind::BoundaryRequirement => {
                                        encoder.tag("boundary_requirement", 1)
                                    }
                                }
                                Ok(())
                            })?;
                            encoder.field("requirement_owner", |encoder| {
                                encode_nominal(encoder, &route.requirement_owner)
                            })?;
                            encoder.field("requirement", |encoder| {
                                encode_nominal(encoder, &route.requirement)
                            })
                        })
                    })
                })
            })?;
        }
    }
    Ok(())
}
