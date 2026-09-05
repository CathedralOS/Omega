use super::{encode_carry_policy, encode_compiler_intrinsic_execution};
use crate::encoding::encode::values::identity::encode_nominal;
use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::{
    CheckedPackageProviderFamilyReview, CheckedPackageProviderReview,
    PackageReviewProviderFamilyCoverage, PackageReviewProviderGrantSelectorKind,
    PackageReviewProviderSelectionAuthority, PackageReviewSelectedInstallationReach,
};
use omega_effects::provider_plan::{
    ProviderBinding, ServiceEntryAuthorityFlow, ServiceProgressEstablishmentRouteKind,
    ServiceProgressSubject,
};

pub(crate) fn encode_provider(
    encoder: &mut Encoder,
    provider: &CheckedPackageProviderReview,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&provider.plan_name)?;
    encoder.u64(provider.plan_report_fingerprint);
    encoder.sequence(&provider.grants, |encoder, grant| {
        encoder.byte(match grant.selector_kind {
            PackageReviewProviderGrantSelectorKind::PlanName => 0,
            PackageReviewProviderGrantSelectorKind::ProviderSlot => 1,
        });
        encoder.fixed_bytes(&grant.selected_plan_digest);
        Ok(())
    })?;
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
        Ok(())
    })
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

pub(crate) fn encode_provider_row(
    encoder: &mut Encoder,
    row: &omega_effects::provider_plan::ProviderPlanRow,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&row.method)?;
    encoder.string(&row.requirement_identity)?;
    encoder.sequence(&row.requirement_lifetime_partition, |encoder, ordinal| {
        encoder.u32(*ordinal);
        Ok(())
    })?;
    match &row.binding {
        ProviderBinding::Import { evaluated } => {
            let locator = evaluated.locator();
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
                omega_effects::ForeignLocatorCandidate::MachODylibSymbol {
                    install_name,
                    symbol,
                } => {
                    encoder.byte(3);
                    encoder.bytes(install_name)?;
                    encoder.bytes(symbol)?;
                }
            }
            encoder.fixed_bytes(&locator.identity_digest().as_bytes());
            encode_evaluated_binding_receipt(encoder, evaluated.receipt())?;
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

fn encode_evaluated_binding_receipt(
    encoder: &mut Encoder,
    receipt: &omega_effects::provider_plan::EvaluatedBindingReceipt,
) -> Result<(), PackageReviewEncodingError> {
    encoder.optional_package_identity(receipt.producer_package());
    encoder.string(receipt.producer_callable_identity())?;
    encoder.fixed_bytes(&receipt.producer_closure_digest().as_bytes());
    encoder.u32(receipt.evaluator_semantics_marker());
    let usage = receipt.evaluation_usage();
    encoder.u32(usage.usage_schema_version());
    encoder.u32(usage.step_schedule_marker());
    encoder.u64(usage.fuel_units());
    encoder.u64(usage.fuel_ceiling());
    encoder.u64(usage.build_log_bytes());
    encoder.u64(usage.filesystem_operation_attempts());
    encoder.u64(usage.peak_live_cells());
    encoder.u64(usage.peak_live_text_bytes());
    encoder.u64(usage.result_cells());
    encoder.u64(usage.result_text_bytes());
    encoder.fixed_bytes(&receipt.evaluation_digest().as_bytes());
    encoder.u32(receipt.materializer_schema_version());
    encoder.fixed_bytes(&receipt.materialization_digest().as_bytes());
    encoder.fixed_bytes(&receipt.locator_identity_digest().as_bytes());
    encoder.fixed_bytes(&receipt.identity_digest());
    encoder.check()
}
