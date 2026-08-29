use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::encoder::Encoder;
use crate::evidence::{
    CheckedPackageProviderFamilyReview, CheckedPackageProviderReview,
    PackageReviewCompilerIntrinsicExecution, PackageReviewProviderFamilyApplicationCoverage,
    PackageReviewProviderFamilyCoverage, PackageReviewProviderSelectionAuthority,
    PackageReviewSelectedInstallationReach,
};
use omega_effects::provider_plan::{
    ProviderBinding, ServiceEntryAuthorityFlow, ServiceProgressEstablishmentRouteKind,
    ServiceProgressSubject,
};

use super::identity::encode_nominal;

pub(crate) fn encode_provider(
    encoder: &mut Encoder,
    provider: &CheckedPackageProviderReview,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&provider.plan_name)?;
    encoder.u64(provider.plan_report_fingerprint);
    encoder.optional_package_identity(provider.realizing_package);
    encode_nominal(encoder, &provider.schema_declaration)?;
    encoder.string(&provider.provider_type)?;
    encoder.optional_package_identity(provider.provider_type_package);
    encoder.option(provider.provider_type_declaration.as_ref(), encode_nominal)?;
    encode_service_schema(encoder, &provider.schema)?;
    encoder.string(&provider.target)?;
    encoder.sequence(&provider.rows, encode_provider_row)?;
    encoder.sequence(&provider.row_declarations, |encoder, row| {
        encode_nominal(encoder, &row.requirement)?;
        encode_nominal(encoder, &row.realization)?;
        encoder.option(
            row.compiler_intrinsic_execution.as_ref(),
            encode_compiler_intrinsic_execution,
        )?;
        encoder.option(row.installation_reach.as_ref(), encode_installation_reach)
    })
}

fn encode_installation_reach(
    encoder: &mut Encoder,
    reach: &PackageReviewSelectedInstallationReach,
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(&reach.upper_bound, encode_nominal)?;
    encoder.sequence(&reach.resolved, encode_nominal)
}

pub(crate) fn encode_provider_family(
    encoder: &mut Encoder,
    family: &CheckedPackageProviderFamilyReview,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &family.family_identity)?;
    encode_nominal(encoder, &family.provider_type_declaration)?;
    encoder.string(family.target.target_name())?;
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
        encoder.u64(coordinate.plan_report_fingerprint);
        match &coordinate.application_coverage {
            PackageReviewProviderFamilyApplicationCoverage::NonGeneric => encoder.byte(0),
            PackageReviewProviderFamilyApplicationCoverage::ExactApplications(applications) => {
                encoder.byte(1);
                encoder.sequence(applications, |encoder, application| {
                    encoder.sequence(&application.arguments, |encoder, argument| {
                        encoder.string(argument)
                    })?;
                    encoder.u64(application.report_fingerprint);
                    Ok(())
                })?;
            }
        }
        Ok(())
    })
}

pub(super) fn encode_compiler_intrinsic_execution(
    encoder: &mut Encoder,
    execution: &PackageReviewCompilerIntrinsicExecution,
) -> Result<(), PackageReviewEncodingError> {
    match execution {
        PackageReviewCompilerIntrinsicExecution::BuiltinFunction(function) => {
            encoder.byte(0);
            encoder.u16(u16::try_from(function.ordinal()).map_err(|_| {
                PackageReviewEncodingError::new(
                    "compiler builtin-function ordinal exceeds the portable encoding range",
                )
            })?);
        }
        PackageReviewCompilerIntrinsicExecution::PrimitiveFloatBinary { operation, format } => {
            encoder.byte(3);
            encode_primitive_float_binary_operation(encoder, *operation);
            encode_float_format(encoder, *format);
        }
        PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(format) => {
            encoder.byte(1);
            encode_float_format(encoder, *format);
        }
        PackageReviewCompilerIntrinsicExecution::NamedFloatConversion {
            source,
            target,
            domain,
        } => {
            encoder.byte(2);
            encode_compiler_numeric_type(encoder, *source);
            encode_compiler_numeric_type(encoder, *target);
            encode_compiler_arithmetic_domain(encoder, *domain);
        }
    }
    Ok(())
}

fn encode_primitive_float_binary_operation(
    encoder: &mut Encoder,
    operation: omega_provider_planning::plans::CompilerPrimitiveFloatBinaryOperation,
) {
    use omega_provider_planning::plans::CompilerPrimitiveFloatBinaryOperation;

    encoder.byte(match operation {
        CompilerPrimitiveFloatBinaryOperation::Add => 0,
        CompilerPrimitiveFloatBinaryOperation::Subtract => 1,
        CompilerPrimitiveFloatBinaryOperation::Multiply => 2,
        CompilerPrimitiveFloatBinaryOperation::Divide => 3,
        CompilerPrimitiveFloatBinaryOperation::Equal => 4,
        CompilerPrimitiveFloatBinaryOperation::NotEqual => 5,
        CompilerPrimitiveFloatBinaryOperation::Less => 6,
        CompilerPrimitiveFloatBinaryOperation::LessOrEqual => 7,
        CompilerPrimitiveFloatBinaryOperation::Greater => 8,
        CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual => 9,
    });
}

fn encode_float_format(encoder: &mut Encoder, format: psi_numerics::literals::FloatFormat) {
    encoder.byte(match format {
        psi_numerics::literals::FloatFormat::F32 => 0,
        psi_numerics::literals::FloatFormat::F64 => 1,
    });
}

fn encode_compiler_numeric_type(
    encoder: &mut Encoder,
    numeric_type: omega_provider_planning::plans::CompilerNumericType,
) {
    use omega_provider_planning::plans::CompilerNumericType;

    encoder.byte(match numeric_type {
        CompilerNumericType::I8 => 0,
        CompilerNumericType::I16 => 1,
        CompilerNumericType::I32 => 2,
        CompilerNumericType::I64 => 3,
        CompilerNumericType::U8 => 4,
        CompilerNumericType::U16 => 5,
        CompilerNumericType::U32 => 6,
        CompilerNumericType::U64 => 7,
        CompilerNumericType::F32 => 8,
        CompilerNumericType::F64 => 9,
    });
}

fn encode_compiler_arithmetic_domain(
    encoder: &mut Encoder,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
) {
    use psi_numerics::arithmetic::ArithmeticDomain;

    encoder.byte(match domain {
        ArithmeticDomain::Exact => 0,
        ArithmeticDomain::Wrapping => 1,
        ArithmeticDomain::Saturating => 2,
        ArithmeticDomain::Trapping => 3,
    });
}

pub(crate) fn encode_service_schema(
    encoder: &mut Encoder,
    schema: &omega_effects::provider_plan::ServiceSchema,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&schema.trait_name)?;
    encoder.optional_package_identity(schema.trait_package_identity);
    encoder.sequence(&schema.methods, |encoder, method| {
        encoder.string(&method.name)?;
        encoder.string(&method.requirement_owner)?;
        encoder.optional_package_identity(method.requirement_owner_package_identity);
        encoder.string(&method.requirement_identity)?;
        encoder.usize(method.parameter_count)?;
        encoder.sequence(&method.parameter_type_identities, |encoder, identity| {
            encoder.string(identity)
        })?;
        encoder.sequence(&method.entry_claims, |encoder, claim| {
            encoder.usize(claim.parameter_index)?;
            encoder.string(&claim.carrier_identity)?;
            encoder.string(&claim.domain)?;
            encoder.byte(match claim.predicate_body {
                psi_language_semantics::DomainPredicateBody::Bodyless => 0,
                psi_language_semantics::DomainPredicateBody::Present => 1,
            });
            encode_carry_policy(encoder, claim.effective_carry);
            encoder.byte(match claim.authority_flow {
                ServiceEntryAuthorityFlow::Accepts => 0,
            });
            Ok(())
        })?;
        encoder.boolean(method.has_result);
        encoder.option(method.result_type_identity.as_ref(), |encoder, identity| {
            encoder.string(identity)
        })?;
        encoder.sequence(&method.result_claims, |encoder, claim| {
            encoder.string(&claim.domain)?;
            encode_carry_policy(encoder, claim.effective_carry);
            Ok(())
        })?;
        encoder.sequence(&method.service_reach, |encoder, service| {
            encoder.string(service)
        })?;
        encoder.sequence(&method.synchronous_invocations, |encoder, invocation| {
            encoder.string(invocation)
        })?;
        encoder.boolean(method.may_suspend);
        encoder.boolean(method.may_block);
        encoder.boolean(method.terminates_guarantee);
        encoder.sequence(&method.termination_premises, |encoder, premise| {
            encoder.string(&premise.profile)?;
            match premise.subject {
                ServiceProgressSubject::ProviderReceiver => encoder.byte(0),
                ServiceProgressSubject::Parameter(position) => {
                    encoder.byte(1);
                    encoder.usize(position)?;
                }
            }
            encoder.sequence(&premise.subject_projections, |encoder, projection| {
                encoder.string(projection)
            })?;
            encoder.sequence(&premise.establishment_routes, |encoder, route| {
                encoder.byte(match route.kind {
                    ServiceProgressEstablishmentRouteKind::CheckedRequirement => 0,
                    ServiceProgressEstablishmentRouteKind::BoundaryRequirement => 1,
                });
                encoder.string(&route.requirement_identity)
            })
        })?;
        encoder.option(
            method.calling_plan_report_fingerprint.as_ref(),
            |encoder, fingerprint| {
                encoder.u64(*fingerprint);
                Ok(())
            },
        )?;
        encoder.option(
            method.calling_plan_commitment.as_ref(),
            |encoder, commitment| encoder.bytes(&commitment.as_bytes()),
        )
    })
}

pub(crate) fn encode_carry_policy(
    encoder: &mut Encoder,
    policy: psi_language_semantics::CarryPolicy,
) {
    encoder.byte(match policy.suspension {
        psi_language_semantics::CarrySuspension::Forbidden => 0,
        psi_language_semantics::CarrySuspension::Allowed => 1,
    });
    encoder.byte(match policy.cpu {
        psi_language_semantics::CarryCpu::Origin => 0,
        psi_language_semantics::CarryCpu::Any => 1,
    });
    encoder.byte(match policy.host_thread {
        psi_language_semantics::CarryHostThread::Origin => 0,
        psi_language_semantics::CarryHostThread::Any => 1,
    });
    encoder.byte(match policy.address {
        psi_language_semantics::CarryAddress::Stable => 0,
        psi_language_semantics::CarryAddress::Movable => 1,
    });
}

pub(crate) fn encode_provider_row(
    encoder: &mut Encoder,
    row: &omega_effects::provider_plan::ProviderPlanRow,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&row.method)?;
    encoder.string(&row.requirement_identity)?;
    match &row.binding {
        ProviderBinding::Import { locator } => {
            encoder.byte(7);
            encoder.string(locator.target().target_name())?;
            encoder.u64(locator.non_authoritative_compatibility_fingerprint());
            match locator.locator() {
                omega_effects::ForeignLocatorCandidate::PeByName { library, export } => {
                    encoder.byte(0);
                    encoder.bytes(library)?;
                    encoder.bytes(export)?;
                }
                omega_effects::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                    encoder.byte(1);
                    encoder.bytes(library)?;
                    encoder.u16(*ordinal);
                }
                omega_effects::ForeignLocatorCandidate::ElfVersioned {
                    object,
                    symbol,
                    version,
                } => {
                    encoder.byte(2);
                    encoder.bytes(object)?;
                    encoder.bytes(symbol)?;
                    encoder.bytes(version)?;
                }
            }
        }
        ProviderBinding::StringBackedImportBootstrap { library, symbol } => {
            encoder.byte(0);
            encoder.string(library)?;
            encoder.string(symbol)?;
        }
        ProviderBinding::Syscall { number } => {
            encoder.byte(1);
            encoder.i64(*number);
        }
        ProviderBinding::CompilerIntrinsic { machine } => {
            encoder.byte(2);
            encoder.string(machine)?;
        }
        ProviderBinding::VtableSlot { index } => {
            encoder.byte(3);
            encoder.i64(*index);
        }
        ProviderBinding::VtableField { table, field } => {
            encoder.byte(4);
            encoder.string(table)?;
            encoder.string(field)?;
        }
        ProviderBinding::TableFunction { table, field } => {
            encoder.byte(5);
            encoder.string(table)?;
            encoder.string(field)?;
        }
        ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } => {
            encoder.byte(6);
            encoder.string(machine_identity)?;
            encoder.optional_package_identity(*machine_package_identity);
        }
    }
    Ok(())
}
