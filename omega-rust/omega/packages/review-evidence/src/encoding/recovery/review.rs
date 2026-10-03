//! Bounded recovery of a checked package review projection.
//!
//! `canonical_review_bytes` re-encodes the recovered projection and compares
//! against the stored bytes, and the retained canonical rows must equal the
//! projection's own row reconstruction, so any decoder divergence turns into
//! a cache miss instead of silently substituting a different review.

use super::policy::{
    PackagePolicyRecoveryError as Error, PackagePolicyRecoveryLimits,
    baseline::boundary::application as boundary_application,
    behavior::{crash_route, synchronous_invocation, termination},
    calling_application::shapes::{graph as boundary_shape_graph, target as representation_target},
    contracts::callable_contract,
    declarations::{
        conformance_shape, const_shape, data_shape, domain_shape, operator_shape,
        proposition_shape, trait_shape, values::operator_spelling,
    },
    external::locator,
    identity::{nominal, operator_coordinate, package, type_identity},
    intrinsic::execution as compiler_intrinsic_execution,
    physical_calling_policy::placement::value_placement,
    reader::Reader,
    signatures::{conformance_bound, data_properties, machine_contract, type_parameter},
    structural_expressions::boolean_expression,
    terminal_permissions::disposition,
};
use crate::encoding::PACKAGE_REVIEW_ENCODING_VERSION;
use crate::encoding::encode::MAGIC;
use crate::record::package::PackageReviewCanonicalRowSources;
use crate::record::*;
use effects::provider_plan::{
    BoundaryCallingPlanCommitment, EvaluatedBindingEvaluationDigest,
    EvaluatedBindingMaterializationDigest, EvaluatedBindingProducerClosureDigest,
    EvaluatedBindingReceipt, EvaluatedBindingUsage, EvaluatedForeignImport, ProviderBinding,
    ProviderPlanRow, ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod,
    ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind,
    ServiceProgressPremise, ServiceProgressSubject, ServiceResultClaim, ServiceSchema,
    ServiceSchemaDigest,
};
use language_semantics::{
    CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension, DomainPredicateBody,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId,
};
use target::{
    ForeignLocatorCandidate, ForeignLocatorIdentityDigest, TargetProfile, normalize_foreign_locator,
};

// ---------------------------------------------------------------------------
// Entry points

/// Recovers the projection stored in `canonical_review_bytes` form.
///
/// `canonical_rows` are the framed ledger rows retained beside the review
/// bytes; the recovered projection's row reconstruction must equal them
/// before the value is trusted — that check also binds the `row_sources`
/// audit metadata to the decoded bodies.
pub fn recover_package_review_projection(
    canonical_review_bytes: &[u8],
    canonical_rows: &[PackageReviewCanonicalRow],
    limits: PackagePolicyRecoveryLimits,
) -> Result<CheckedPackageReviewProjection, Error> {
    let mut reader = Reader::new(canonical_review_bytes, limits)?;
    reader.literal(MAGIC)?;
    if reader.u16()? != PACKAGE_REVIEW_ENCODING_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    let mut projection = projection(&mut reader)?;
    reader.finish()?;
    reader.canonical_scratch(canonical_review_bytes.len())?;
    projection.row_sources = row_sources(canonical_rows)?;
    if projection
        .canonical_review_bytes()
        .map_err(|_| Error::NonCanonicalEncoding)?
        != canonical_review_bytes
    {
        return Err(Error::NonCanonicalEncoding);
    }
    if projection
        .canonical_rows()
        .map_err(|_| Error::NonCanonicalEncoding)?
        .as_slice()
        != canonical_rows
    {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(projection)
}

fn empty_row_sources() -> PackageReviewCanonicalRowSources {
    PackageReviewCanonicalRowSources {
        public_traits: Vec::new(),
        public_conformances: Vec::new(),
        public_domains: Vec::new(),
        public_propositions: Vec::new(),
        public_consts: Vec::new(),
        public_operators: Vec::new(),
        public_data: Vec::new(),
        representation_tcb: Vec::new(),
        semantic_dependencies: Vec::new(),
        callables: Vec::new(),
        contract_entailment_open_obligations: Vec::new(),
        contract_entailment_assumption_discharges: Vec::new(),
        contract_entailment_exit_membership_discharges: Vec::new(),
        external_executable_supply: Vec::new(),
        dangerous_authorities: Vec::new(),
        dangerous_authority_slack: Vec::new(),
        terminal_authority_permissions: Vec::new(),
        boundary_application_realizations: Vec::new(),
        boundary_application_demands: Vec::new(),
        selected_provider_set: PackageReviewCanonicalRowSource::compiler_derived(
            PackageReviewSyntheticSourceKind::EmptySelectedProviderSet,
        ),
    }
}

/// Rebuilds the positional row-source ledger from the stored canonical rows.
/// Rows emit group-by-group in projection order, so each row's source lands
/// at the next slot of its kind's ledger; `AcceptedClaim` rows alias their
/// callable's source and carry no ledger slot of their own.
fn row_sources(
    rows: &[PackageReviewCanonicalRow],
) -> Result<PackageReviewCanonicalRowSources, Error> {
    let mut sources = empty_row_sources();
    for row in rows {
        match row.kind() {
            PackageReviewCanonicalRowKind::PublicTrait => {
                sources.public_traits.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::PublicConformance => {
                sources.public_conformances.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::PublicDomain => {
                sources.public_domains.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::PublicProposition => {
                sources.public_propositions.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::PublicConst => {
                sources.public_consts.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::PublicOperator => {
                sources.public_operators.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::PublicData => {
                sources.public_data.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::RepresentationTcb => {
                sources.representation_tcb.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::SemanticDependency => {
                sources.semantic_dependencies.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::Callable => {
                sources.callables.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::AcceptedClaim => {}
            PackageReviewCanonicalRowKind::ExternalExecutableSupply => {
                sources
                    .external_executable_supply
                    .push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation => {
                sources
                    .contract_entailment_open_obligations
                    .push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::ContractEntailmentAssumptionDischarge => {
                sources
                    .contract_entailment_assumption_discharges
                    .push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::ContractEntailmentExitMembershipDischarge => {
                sources
                    .contract_entailment_exit_membership_discharges
                    .push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::DangerousAuthority => {
                sources.dangerous_authorities.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::DangerousAuthoritySlack => {
                sources.dangerous_authority_slack.push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::TerminalAuthorityPermission => {
                sources
                    .terminal_authority_permissions
                    .push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::BoundaryApplicationRealization => {
                sources
                    .boundary_application_realizations
                    .push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::BoundaryApplicationDemand => {
                sources
                    .boundary_application_demands
                    .push(row.source().clone());
            }
            PackageReviewCanonicalRowKind::SelectedProviderSet => {
                sources.selected_provider_set = row.source().clone();
            }
            PackageReviewCanonicalRowKind::ProjectionHeader
            | PackageReviewCanonicalRowKind::NonExecutableQuotientCorrespondence
            | PackageReviewCanonicalRowKind::AssumedBindingContract => {}
        }
    }
    Ok(sources)
}

fn projection(reader: &mut Reader<'_>) -> Result<CheckedPackageReviewProjection, Error> {
    Ok(CheckedPackageReviewProjection {
        package: package(reader)?,
        target: target_profile(reader)?,
        public_traits: reader.sequence(58, trait_shape)?,
        public_conformances: reader.sequence(34, conformance_shape)?,
        public_domains: reader.sequence(30, domain_shape)?,
        public_propositions: reader.sequence(30, proposition_shape)?,
        public_consts: reader.sequence(30, const_shape)?,
        public_operators: reader.sequence(30, operator_shape)?,
        public_data: reader.sequence(30, data_shape)?,
        representation_tcb: reader.sequence(30, representation_tcb)?,
        semantic_dependencies: reader.sequence(30, semantic_dependency)?,
        callables: reader.sequence(30, callable)?,
        external_executable_supply: reader.sequence(30, external_executable_supply)?,
        contract_entailment_open_obligations: reader
            .sequence(30, contract_entailment_open_obligation)?,
        contract_entailment_assumption_discharges: reader
            .sequence(30, contract_entailment_assumption_discharge)?,
        contract_entailment_exit_membership_discharges: reader
            .sequence(30, contract_entailment_exit_membership_discharge)?,
        dangerous_authorities: reader.sequence(30, dangerous_authority)?,
        dangerous_authority_slack: reader.sequence(30, dangerous_authority_slack)?,
        terminal_authority_permissions: reader.sequence(30, terminal_authority_permission)?,
        selected_providers: reader.sequence(30, provider)?,
        selected_provider_families: reader.sequence(30, provider_family)?,
        boundary_application_realizations: reader.sequence(30, boundary_application_realization)?,
        boundary_application_demands: reader.sequence(30, boundary_application_demand)?,
        row_sources: empty_row_sources(),
    })
}

fn target_profile(reader: &mut Reader<'_>) -> Result<TargetProfile, Error> {
    let target_name = reader.string()?;
    TargetProfile::ALL
        .iter()
        .copied()
        .find(|profile| profile.target_name() == target_name)
        .ok_or(Error::InvalidIdentity)
}

// ---------------------------------------------------------------------------
// Dependency trust

fn semantic_dependency(reader: &mut Reader<'_>) -> Result<PackageReviewSemanticDependency, Error> {
    Ok(PackageReviewSemanticDependency {
        consumer: nominal(reader)?,
        dependency: nominal(reader)?,
        kind: match reader.byte()? {
            0 => PackageReviewSemanticDependencyKind::NominalIdentity,
            1 => PackageReviewSemanticDependencyKind::Layout,
            2 => PackageReviewSemanticDependencyKind::OwnershipBehavior,
            3 => PackageReviewSemanticDependencyKind::AutomaticCleanup,
            4 => PackageReviewSemanticDependencyKind::AutomaticCleanupMachine,
            _ => return Err(Error::InvalidTag),
        },
        exposure: match reader.byte()? {
            0 => PackageReviewSemanticDependencyExposure::PrivateImplementation,
            1 => PackageReviewSemanticDependencyExposure::PublicInterface,
            _ => return Err(Error::InvalidTag),
        },
    })
}

fn opaque_representation_origin(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewOpaqueRepresentationApplicationOrigin, Error> {
    Ok(match reader.byte()? {
        1 => PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance,
        _ => return Err(Error::InvalidTag),
    })
}

fn opaque_representation_lifecycle(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewOpaqueRepresentationLifecycleDisposition, Error> {
    Ok(match reader.byte()? {
        1 => PackageReviewOpaqueRepresentationLifecycleDisposition::Inert,
        _ => return Err(Error::InvalidTag),
    })
}

fn opaque_representation_copy_disposition(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewOpaqueRepresentationCopyDisposition, Error> {
    Ok(match reader.byte()? {
        1 => PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly,
        2 => PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
        _ => return Err(Error::InvalidTag),
    })
}

fn representation_tcb(reader: &mut Reader<'_>) -> Result<PackageReviewRepresentationTcb, Error> {
    let declaration = nominal(reader)?;
    let kind = match reader.byte()? {
        0 => PackageReviewRepresentationTcbKind::Unbound,
        1 => PackageReviewRepresentationTcbKind::ProducerAvailability {
            conformance: nominal(reader)?,
            carrier: nominal(reader)?,
        },
        2 => PackageReviewRepresentationTcbKind::SelectedCopyReceipt {
            conformance: nominal(reader)?,
            carrier: nominal(reader)?,
            representation_schema_version: reader.u16()?,
            origin: opaque_representation_origin(reader)?,
            lifecycle: opaque_representation_lifecycle(reader)?,
            copy_disposition: opaque_representation_copy_disposition(reader)?,
            conformance_application_commitment: reader.digest()?,
            selected_application_commitment: reader.digest()?,
        },
        3 => PackageReviewRepresentationTcbKind::ConsumerDemand {
            boundary_trait: nominal(reader)?,
            boundary_arguments: reader.sequence(9, type_identity)?,
            requirement: nominal(reader)?,
            requirement_identity: reader.string()?,
            target: representation_target(reader)?,
            conformance: nominal(reader)?,
            carrier: nominal(reader)?,
            representation_schema_version: reader.u16()?,
            origin: opaque_representation_origin(reader)?,
            lifecycle: opaque_representation_lifecycle(reader)?,
            copy_disposition: opaque_representation_copy_disposition(reader)?,
            shape_graph: boundary_shape_graph(reader)?,
            occurrences: reader.sequence(12, opaque_occurrence)?,
            calling_policy: boundary_calling_policy(reader)?,
            conformance_application_commitment: reader.digest()?,
            selected_application_commitment: reader.digest()?,
            boundary_plan_commitment: reader.digest()?,
        },
        _ => return Err(Error::InvalidTag),
    };
    Ok(PackageReviewRepresentationTcb { declaration, kind })
}

fn opaque_occurrence(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewOpaqueRepresentationOccurrence, Error> {
    Ok(PackageReviewOpaqueRepresentationOccurrence {
        carrier_shape_root: reader.u16()?,
        role: match reader.byte()? {
            0 => PackageReviewOpaqueRepresentationMovementRole::Parameter {
                formal_ordinal: reader.u32()?,
                native_ordinal: reader.u32()?,
            },
            1 => PackageReviewOpaqueRepresentationMovementRole::Result,
            _ => return Err(Error::InvalidTag),
        },
        path: reader.sequence(3, |reader| {
            Ok(match reader.byte()? {
                0 => PackageReviewOpaqueRepresentationPathElement::FixedArrayElement,
                1 => PackageReviewOpaqueRepresentationPathElement::RecordField {
                    ordinal: reader.u16()?,
                },
                _ => return Err(Error::InvalidTag),
            })
        })?,
        placement: value_placement(reader)?,
    })
}

fn boundary_calling_policy(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewBoundaryCallingPolicy, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewBoundaryCallingPolicy::MicrosoftX64,
        1 => PackageReviewBoundaryCallingPolicy::SystemVAMD64,
        2 => PackageReviewBoundaryCallingPolicy::Aapcs64,
        3 => PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64,
        4 => PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64,
        _ => return Err(Error::InvalidTag),
    })
}

fn dangerous_authority_class(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDangerousAuthorityClass, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewDangerousAuthorityClass::Filesystem,
        1 => PackageReviewDangerousAuthorityClass::MachineControl,
        2 => PackageReviewDangerousAuthorityClass::PortIo,
        3 => PackageReviewDangerousAuthorityClass::InterruptControl,
        4 => PackageReviewDangerousAuthorityClass::InterruptEntry,
        5 => PackageReviewDangerousAuthorityClass::RootMemory,
        6 => PackageReviewDangerousAuthorityClass::Process,
        _ => return Err(Error::InvalidTag),
    })
}

fn dangerous_authority(reader: &mut Reader<'_>) -> Result<PackageReviewDangerousAuthority, Error> {
    Ok(PackageReviewDangerousAuthority {
        class: dangerous_authority_class(reader)?,
        service: nominal(reader)?,
    })
}

fn dangerous_authority_slack(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDangerousAuthoritySlack, Error> {
    Ok(PackageReviewDangerousAuthoritySlack {
        class: dangerous_authority_class(reader)?,
        callable: nominal(reader)?,
        service: nominal(reader)?,
    })
}

fn terminal_authority_permission(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewTerminalAuthorityPermission, Error> {
    Ok(PackageReviewTerminalAuthorityPermission {
        service: nominal(reader)?,
        service_schema: ServiceSchemaDigest::from_digest(reader.digest()?),
        requirement_identity: reader.string()?,
        permitted: disposition(reader)?,
    })
}

// ---------------------------------------------------------------------------
// Callables

fn callable(reader: &mut Reader<'_>) -> Result<CheckedPackageCallableReview, Error> {
    Ok(CheckedPackageCallableReview {
        role: match reader.byte()? {
            0 => PackageReviewCallableRole::Boundary,
            1 => PackageReviewCallableRole::Public,
            2 => PackageReviewCallableRole::Build,
            _ => return Err(Error::InvalidTag),
        },
        identity: nominal(reader)?,
        supply: match reader.byte()? {
            0 => PackageReviewCallableSupply::CheckedBody,
            1 => PackageReviewCallableSupply::Requirement,
            2 => PackageReviewCallableSupply::Boundary,
            3 => PackageReviewCallableSupply::AdmissionClaim,
            4 => PackageReviewCallableSupply::ExternalRealization,
            5 => PackageReviewCallableSupply::TopLevelRequirement,
            _ => return Err(Error::InvalidTag),
        },
        spelling: reader.option(operator_spelling)?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(9, type_parameter)?,
        conformance_bounds: reader.sequence(16, conformance_bound)?,
        parameters: reader.sequence(20, callable_parameter)?,
        return_type: type_identity(reader)?,
        conformances: reader.sequence(20, callable_conformance)?,
        operator_realizations: reader.sequence(20, |reader| {
            Ok(PackageReviewOperatorRealization {
                coordinate: operator_coordinate(reader)?,
                alias: reader.option(|reader| reader.string())?,
            })
        })?,
        requirement_realizations: reader.sequence(20, |reader| {
            Ok(PackageReviewRequirementRealization {
                requirement_identity: nominal(reader)?,
                alias: reader.option(|reader| reader.string())?,
            })
        })?,
        contracts: reader.sequence(20, callable_contract)?,
        declared_service_reach: reader.option(|reader| reader.sequence(30, nominal))?,
        checked_service_reach: match reader.byte()? {
            0 => PackageReviewCheckedServiceReach::NoCheckedBody,
            1 => PackageReviewCheckedServiceReach::CheckedBody {
                realized: reader.sequence(30, nominal)?,
                concrete: reader.sequence(30, nominal)?,
            },
            _ => return Err(Error::InvalidTag),
        },
        unresolved_installation_reaches: reader.sequence(30, installation_reach)?,
        declared_synchronous_invocations: reader
            .option(|reader| reader.sequence(30, synchronous_invocation))?,
        realized_synchronous_invocations: reader.sequence(30, synchronous_invocation)?,
        capability_flows: reader.sequence(30, capability_flow)?,
        checked_may_suspend: reader.boolean()?,
        checked_may_block: reader.boolean()?,
        checked_termination: termination(reader)?,
        checked_crash: crash(reader)?,
        mutation: reader.sequence(30, mutation)?,
    })
}

fn callable_parameter(reader: &mut Reader<'_>) -> Result<PackageReviewCallableParameter, Error> {
    Ok(PackageReviewCallableParameter {
        name: reader.string()?,
        type_identity: type_identity(reader)?,
        is_const: reader.boolean()?,
        is_mutable: reader.boolean()?,
        is_self: reader.boolean()?,
    })
}

fn callable_conformance(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewCallableConformance, Error> {
    Ok(PackageReviewCallableConformance {
        trait_identity: nominal(reader)?,
        requirement_identity: nominal(reader)?,
        requirement_lifetime_partition: reader.sequence(4, |reader| reader.u32())?,
        arguments: reader.sequence(9, type_identity)?,
        alias: reader.option(|reader| reader.string())?,
    })
}

fn installation_reach(reader: &mut Reader<'_>) -> Result<PackageReviewInstallationReach, Error> {
    Ok(PackageReviewInstallationReach {
        requirement: nominal(reader)?,
        upper_bound: reader.sequence(30, nominal)?,
    })
}

fn capability_flow(reader: &mut Reader<'_>) -> Result<PackageReviewCapabilityFlow, Error> {
    Ok(PackageReviewCapabilityFlow {
        capability: nominal(reader)?,
        kind: match reader.byte()? {
            0 => flow_effects::CapabilityFlowKind::Uses,
            1 => flow_effects::CapabilityFlowKind::Returns,
            2 => flow_effects::CapabilityFlowKind::Acquires,
            3 => flow_effects::CapabilityFlowKind::Stores,
            4 => flow_effects::CapabilityFlowKind::Derives,
            _ => return Err(Error::InvalidTag),
        },
        state: nominal(reader)?,
        statement_index: reader.usize()?,
        call_ordinal: reader.usize()?,
        via_state: reader.option(nominal)?,
    })
}

fn mutation(reader: &mut Reader<'_>) -> Result<PackageReviewMutation, Error> {
    Ok(PackageReviewMutation {
        state: nominal(reader)?,
        completeness: match reader.byte()? {
            0 => PackageReviewWriteFrameCompleteness::Complete,
            1 => PackageReviewWriteFrameCompleteness::Opaque,
            _ => return Err(Error::InvalidTag),
        },
        paths: reader.sequence(4, |reader| reader.string())?,
    })
}

// ---------------------------------------------------------------------------
// Crashes

fn crash(reader: &mut Reader<'_>) -> Result<PackageReviewCrash, Error> {
    Ok(PackageReviewCrash {
        interface: match reader.byte()? {
            0 => PackageReviewCrashInterface::InternalInferred,
            1 => PackageReviewCrashInterface::PublishedCeiling,
            _ => return Err(Error::InvalidTag),
        },
        published: reader.sequence(20, crash_route)?,
        structural_runtime_requirements: reader
            .option(|reader| reader.sequence(20, boolean_expression))?,
        checked_sites: reader.sequence(20, crash_site)?,
        checked_calls: reader.sequence(20, crash_call)?,
        checked_operators: reader.sequence(20, crash_operator_site)?,
    })
}

fn crash_cause(reader: &mut Reader<'_>) -> Result<PackageReviewCrashCause, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewCrashCause::Trap,
        1 => PackageReviewCrashCause::Abort,
        _ => return Err(Error::InvalidTag),
    })
}

fn crash_predicate(reader: &mut Reader<'_>) -> Result<PackageReviewCrashPredicate, Error> {
    Ok(PackageReviewCrashPredicate {
        canonical_bytes: reader.bytes()?,
    })
}

fn permission_claim(reader: &mut Reader<'_>) -> Result<PackageReviewPermissionClaim, Error> {
    Ok(PackageReviewPermissionClaim {
        machine: nominal(reader)?,
        state: nominal(reader)?,
        source: match reader.byte()? {
            0 => PackageReviewPermissionSource::StateEntry,
            1 => PackageReviewPermissionSource::Statement {
                statement_ordinal: reader.u64()?,
            },
            2 => PackageReviewPermissionSource::Call {
                statement_ordinal: reader.u64()?,
                call_ordinal: reader.u64()?,
                target: nominal(reader)?,
            },
            3 => PackageReviewPermissionSource::StateExit,
            _ => return Err(Error::InvalidTag),
        },
        ordinal: reader.u32()?,
    })
}

fn crash_site(reader: &mut Reader<'_>) -> Result<PackageReviewCrashSite, Error> {
    Ok(PackageReviewCrashSite {
        state: nominal(reader)?,
        statement_ordinal: reader.u32()?,
        cause: crash_cause(reader)?,
        path_guard_conjuncts: reader.sequence(12, crash_predicate)?,
        path_guard_consequences: reader.sequence(12, crash_predicate)?,
        guard_covering_buckets: reader.sequence(4, |reader| reader.u32())?,
        frontier_lower_bound: reader.sequence(20, permission_claim)?,
    })
}

fn crash_call(reader: &mut Reader<'_>) -> Result<PackageReviewCrashCall, Error> {
    Ok(PackageReviewCrashCall {
        state: nominal(reader)?,
        statement_ordinal: reader.u32()?,
        call_ordinal: reader.u32()?,
        target_machine: nominal(reader)?,
        target_state: nominal(reader)?,
        path_guard_conjuncts: reader.sequence(12, crash_predicate)?,
        path_guard_consequences: reader.sequence(12, crash_predicate)?,
        surviving_buckets: reader.sequence(20, crash_route)?,
    })
}

fn crash_operator_site(reader: &mut Reader<'_>) -> Result<PackageReviewCrashOperatorSite, Error> {
    Ok(PackageReviewCrashOperatorSite {
        state: nominal(reader)?,
        statement_ordinal: reader.u32()?,
        selected_operator: nominal(reader)?,
        published: reader.sequence(20, crash_route)?,
        surviving: reader.sequence(20, crash_route)?,
    })
}

// ---------------------------------------------------------------------------
// Contract entailment

fn contract_entailment_open_obligation(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewContractEntailmentOpenObligation, Error> {
    Ok(PackageReviewContractEntailmentOpenObligation {
        callable: nominal(reader)?,
        contract_position: reader.u32()?,
        fact_position: reader.u32()?,
        machine_contract_commitment: reader.digest()?,
        goal: callable_contract(reader)?,
        reason: match reader.byte()? {
            0 => PackageReviewContractEntailmentOpenReason::UnsupportedEnsuresFact,
            1 => PackageReviewContractEntailmentOpenReason::UnrecognizedInductiveBody,
            2 => PackageReviewContractEntailmentOpenReason::OutsideEntailmentLanguage,
            _ => return Err(Error::InvalidTag),
        },
    })
}

fn contract_entailment_assumption_discharge(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewContractEntailmentAssumptionDischarge, Error> {
    Ok(PackageReviewContractEntailmentAssumptionDischarge {
        obligation: contract_entailment_open_obligation(reader)?,
        assumptions: reader.sequence(30, assumption_proposition)?,
        goal: assumption_proposition(reader)?,
        selected_assumption_position: reader.u32()?,
    })
}

fn contract_entailment_exit_membership_discharge(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewContractEntailmentExitMembershipDischarge, Error> {
    Ok(PackageReviewContractEntailmentExitMembershipDischarge {
        obligation: contract_entailment_open_obligation(reader)?,
        covered_exits: reader.sequence(12, |reader| {
            Ok(PackageReviewContractEntailmentCoveredExit {
                state: nominal(reader)?,
                statement_index: reader.u32()?,
            })
        })?,
    })
}

/// Decodes one proposition of the entailment vocabulary; the encoder only
/// admits the closed {truth, falsehood, comparison, junction} fragment, so any
/// other form is a store integrity failure.
fn assumption_proposition(reader: &mut Reader<'_>) -> Result<Proposition, Error> {
    Ok(match reader.byte()? {
        0 => Proposition::Truth,
        1 => Proposition::Falsehood,
        2 => Proposition::Equal(
            assumption_scalar_term(reader)?,
            assumption_scalar_term(reader)?,
        ),
        3 => Proposition::LessThan(
            assumption_scalar_term(reader)?,
            assumption_scalar_term(reader)?,
        ),
        4 => Proposition::LessOrEqual(
            assumption_scalar_term(reader)?,
            assumption_scalar_term(reader)?,
        ),
        5 => Proposition::Conjunction(reader.sequence(6, assumption_proposition)?),
        6 => Proposition::Disjunction(reader.sequence(6, assumption_proposition)?),
        _ => return Err(Error::InvalidTag),
    })
}

fn assumption_scalar_term(reader: &mut Reader<'_>) -> Result<ScalarTerm, Error> {
    Ok(match reader.byte()? {
        0 => ScalarTerm::Value {
            id: ValueId::new(reader.u64()?).ok_or(Error::InvalidValue)?,
            scalar_type: assumption_scalar_type(reader)?,
        },
        1 => ScalarTerm::Boolean(reader.boolean()?),
        2 => ScalarTerm::Integer {
            scalar_type: assumption_integer_type(reader)?,
            value: match reader.byte()? {
                0 => IntegerValue::Signed(i128::from_le_bytes(reader.array::<16>()?)),
                1 => IntegerValue::Unsigned(u128::from_le_bytes(reader.array::<16>()?)),
                _ => return Err(Error::InvalidTag),
            },
        },
        _ => return Err(Error::InvalidTag),
    })
}

fn assumption_scalar_type(reader: &mut Reader<'_>) -> Result<ScalarType, Error> {
    Ok(match reader.byte()? {
        0 => ScalarType::Boolean,
        1 => ScalarType::Integer(assumption_integer_type(reader)?),
        _ => return Err(Error::InvalidTag),
    })
}

/// Wire layout: carrier byte, sign byte, bits. `IntegerType::address` forces
/// the unsigned axis on reconstruction, so a signed address carrier is a
/// non-canonical store, not a decoding of a different type.
fn assumption_integer_type(reader: &mut Reader<'_>) -> Result<IntegerType, Error> {
    let carrier = reader.byte()?;
    let sign = reader.byte()?;
    let bits = reader.u16()?;
    match carrier {
        0 => IntegerType::new(
            match sign {
                0 => IntegerSign::Signed,
                1 => IntegerSign::Unsigned,
                _ => return Err(Error::InvalidTag),
            },
            bits,
        )
        .map_err(|_| Error::InvalidValue),
        1 => {
            if sign != 1 {
                return Err(Error::NonCanonicalEncoding);
            }
            IntegerType::address(bits).map_err(|_| Error::InvalidValue)
        }
        _ => Err(Error::InvalidTag),
    }
}

// ---------------------------------------------------------------------------
// External executable supply

fn external_executable_supply(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewExternalExecutableSupply, Error> {
    Ok(PackageReviewExternalExecutableSupply {
        callable: nominal(reader)?,
        signature: external_signature(reader)?,
        requirement: external_requirement(reader)?,
        binding: external_binding(reader)?,
    })
}

fn external_signature(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewExternalCallableSignature, Error> {
    Ok(PackageReviewExternalCallableSignature {
        lifetime_parameter_count: reader.usize()?,
        static_parameters: reader.sequence(9, external_static_parameter)?,
        conformance_bounds: reader.sequence(16, conformance_bound)?,
        parameters: reader.sequence(20, |reader| {
            Ok(PackageReviewExternalCallableParameter {
                type_identity: type_identity(reader)?,
                is_const: reader.boolean()?,
                is_mutable: reader.boolean()?,
                is_self: reader.boolean()?,
            })
        })?,
        return_type: type_identity(reader)?,
    })
}

fn external_static_parameter(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewExternalStaticParameter, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewExternalStaticParameter::Type {
            properties: data_properties(reader)?,
        },
        1 => PackageReviewExternalStaticParameter::Const {
            type_identity: type_identity(reader)?,
        },
        2 => PackageReviewExternalStaticParameter::Machine {
            contract: machine_contract(reader)?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

fn external_requirement(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewExternalRequirement, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewExternalRequirement::Trait(callable_conformance(reader)?),
        1 => PackageReviewExternalRequirement::Operator {
            coordinate: operator_coordinate(reader)?,
            alias: reader.option(|reader| reader.string())?,
        },
        2 => PackageReviewExternalRequirement::TopLevelRequirement {
            identity: nominal(reader)?,
            signature: external_signature(reader)?,
            alias: reader.option(|reader| reader.string())?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

fn external_binding(reader: &mut Reader<'_>) -> Result<PackageReviewExternalBinding, Error> {
    Ok(match reader.byte()? {
        0 => return Err(Error::RetiredVocabulary),
        1 => PackageReviewExternalBinding::Syscall {
            number: reader.i64()?,
        },
        2 => PackageReviewExternalBinding::CompilerIntrinsic,
        3 => PackageReviewExternalBinding::VtableSlot {
            index: reader.i64()?,
        },
        4 => PackageReviewExternalBinding::VtableField {
            field: reader.string()?,
        },
        5 => PackageReviewExternalBinding::TableFunction {
            field: reader.string()?,
        },
        6 => PackageReviewExternalBinding::NormalizedImport(evaluated_import(reader)?),
        7 => PackageReviewExternalBinding::NormalizedSyscall(evaluated_syscall(reader)?),
        _ => return Err(Error::InvalidTag),
    })
}

fn evaluated_usage(reader: &mut Reader<'_>) -> Result<PackageReviewEvaluatedBindingUsage, Error> {
    Ok(PackageReviewEvaluatedBindingUsage {
        usage_schema_version: reader.u32()?,
        step_schedule_marker: reader.u32()?,
        fuel_units: reader.u64()?,
        fuel_ceiling: reader.u64()?,
        build_log_bytes: reader.u64()?,
        filesystem_operation_attempts: reader.u64()?,
        peak_live_cells: reader.u64()?,
        peak_live_text_bytes: reader.u64()?,
        result_cells: reader.u64()?,
        result_text_bytes: reader.u64()?,
    })
}

fn evaluated_import(reader: &mut Reader<'_>) -> Result<PackageReviewEvaluatedImport, Error> {
    Ok(PackageReviewEvaluatedImport {
        target: reader.string()?,
        locator: locator(reader)?,
        locator_identity_digest: reader.digest()?,
        producer: nominal(reader)?,
        producer_package: reader.option(package)?,
        producer_callable_identity: reader.string()?,
        producer_closure_digest: reader.digest()?,
        evaluator_semantics_marker: reader.u32()?,
        evaluation_usage: evaluated_usage(reader)?,
        evaluation_digest: reader.digest()?,
        materializer_schema_version: reader.u32()?,
        materialization_digest: reader.digest()?,
        receipt_locator_identity_digest: reader.digest()?,
        receipt_identity_digest: reader.digest()?,
    })
}

fn evaluated_syscall(reader: &mut Reader<'_>) -> Result<PackageReviewEvaluatedSyscall, Error> {
    Ok(PackageReviewEvaluatedSyscall {
        target: reader.string()?,
        number: reader.i64()?,
        binding_identity_digest: reader.digest()?,
        producer: nominal(reader)?,
        producer_package: reader.option(package)?,
        producer_callable_identity: reader.string()?,
        producer_closure_digest: reader.digest()?,
        evaluator_semantics_marker: reader.u32()?,
        evaluation_usage: evaluated_usage(reader)?,
        evaluation_digest: reader.digest()?,
        materializer_schema_version: reader.u32()?,
        materialization_digest: reader.digest()?,
        receipt_binding_identity_digest: reader.digest()?,
        receipt_identity_digest: reader.digest()?,
    })
}

// ---------------------------------------------------------------------------
// Selected providers

fn provider(reader: &mut Reader<'_>) -> Result<CheckedPackageProviderReview, Error> {
    Ok(CheckedPackageProviderReview {
        plan_name: reader.string()?,
        plan_report_fingerprint: reader.u64()?,
        grants: reader.sequence(10, |reader| {
            Ok(PackageReviewSelectedProviderGrant {
                selector_kind: match reader.byte()? {
                    0 => PackageReviewProviderGrantSelectorKind::PlanName,
                    1 => PackageReviewProviderGrantSelectorKind::ProviderSlot,
                    _ => return Err(Error::InvalidTag),
                },
                selected_plan_digest: reader.digest()?,
            })
        })?,
        realizing_package: reader.option(package)?,
        schema_declaration: nominal(reader)?,
        provider_type: reader.string()?,
        provider_type_package: reader.option(package)?,
        provider_type_declaration: reader.option(nominal)?,
        schema: service_schema(reader)?,
        target: reader.string()?,
        rows: reader.sequence(30, provider_row)?,
        row_declarations: reader.sequence(30, provider_row_declaration)?,
    })
}

fn service_schema(reader: &mut Reader<'_>) -> Result<ServiceSchema, Error> {
    Ok(ServiceSchema {
        trait_name: reader.string()?,
        trait_package_identity: reader.option(package)?,
        methods: reader.sequence(30, service_method)?,
    })
}

fn service_method(reader: &mut Reader<'_>) -> Result<ServiceMethod, Error> {
    Ok(ServiceMethod {
        name: reader.string()?,
        requirement_owner: reader.string()?,
        requirement_owner_package_identity: reader.option(package)?,
        requirement_identity: reader.string()?,
        parameter_count: reader.usize()?,
        parameter_type_identities: reader.sequence(4, |reader| reader.string())?,
        entry_claims: reader.sequence(12, service_entry_claim)?,
        completion: match reader.byte()? {
            0 => semantic_vocabulary::BoundaryCompletion::NormalReturn,
            1 => semantic_vocabulary::BoundaryCompletion::External(
                semantic_vocabulary::ExternalCompletionKind::ProcessExit,
            ),
            _ => return Err(Error::InvalidTag),
        },
        has_result: reader.boolean()?,
        result_type_identity: reader.option(|reader| reader.string())?,
        result_claims: reader.sequence(8, service_result_claim)?,
        service_reach: reader.sequence(30, |reader| reader.string())?,
        synchronous_invocations: reader.sequence(30, |reader| reader.string())?,
        may_suspend: reader.boolean()?,
        may_block: reader.boolean()?,
        terminates_guarantee: reader.boolean()?,
        termination_premises: reader.sequence(30, service_progress_premise)?,
        calling_plan_report_fingerprint: reader.option(|reader| reader.u64())?,
        calling_plan_commitment: reader.option(|reader| {
            let bytes: [u8; 32] = reader
                .bytes()?
                .try_into()
                .map_err(|_| Error::InvalidValue)?;
            Ok(BoundaryCallingPlanCommitment::from_digest(bytes))
        })?,
    })
}

fn service_entry_claim(reader: &mut Reader<'_>) -> Result<ServiceEntryClaim, Error> {
    Ok(ServiceEntryClaim {
        parameter_index: reader.usize()?,
        carrier_identity: reader.string()?,
        domain: reader.string()?,
        predicate_body: match reader.byte()? {
            0 => DomainPredicateBody::Bodyless,
            1 => DomainPredicateBody::Present,
            _ => return Err(Error::InvalidTag),
        },
        effective_carry: carry_policy(reader)?,
        authority_flow: match reader.byte()? {
            0 => ServiceEntryAuthorityFlow::Accepts,
            _ => return Err(Error::InvalidTag),
        },
    })
}

fn carry_policy(reader: &mut Reader<'_>) -> Result<CarryPolicy, Error> {
    Ok(CarryPolicy {
        suspension: match reader.byte()? {
            0 => CarrySuspension::Forbidden,
            1 => CarrySuspension::Allowed,
            _ => return Err(Error::InvalidTag),
        },
        cpu: match reader.byte()? {
            0 => CarryCpu::Origin,
            1 => CarryCpu::Any,
            _ => return Err(Error::InvalidTag),
        },
        host_thread: match reader.byte()? {
            0 => CarryHostThread::Origin,
            1 => CarryHostThread::Any,
            _ => return Err(Error::InvalidTag),
        },
        address: match reader.byte()? {
            0 => CarryAddress::Stable,
            1 => CarryAddress::Movable,
            _ => return Err(Error::InvalidTag),
        },
    })
}

fn service_result_claim(reader: &mut Reader<'_>) -> Result<ServiceResultClaim, Error> {
    Ok(ServiceResultClaim {
        domain: reader.string()?,
        effective_carry: carry_policy(reader)?,
    })
}

fn service_progress_premise(reader: &mut Reader<'_>) -> Result<ServiceProgressPremise, Error> {
    Ok(ServiceProgressPremise {
        profile: reader.string()?,
        subject: match reader.byte()? {
            0 => ServiceProgressSubject::ProviderReceiver,
            1 => ServiceProgressSubject::Parameter(reader.usize()?),
            _ => return Err(Error::InvalidTag),
        },
        subject_projections: reader.sequence(4, |reader| reader.string())?,
        establishment_routes: reader.sequence(8, |reader| {
            Ok(ServiceProgressEstablishmentRoute {
                kind: match reader.byte()? {
                    0 => ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                    1 => ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                    _ => return Err(Error::InvalidTag),
                },
                requirement_identity: reader.string()?,
            })
        })?,
    })
}

fn provider_row(reader: &mut Reader<'_>) -> Result<ProviderPlanRow, Error> {
    Ok(ProviderPlanRow {
        method: reader.string()?,
        requirement_identity: reader.string()?,
        requirement_lifetime_partition: reader.sequence(4, |reader| reader.u32())?,
        binding: provider_binding(reader)?,
    })
}

fn provider_binding(reader: &mut Reader<'_>) -> Result<ProviderBinding, Error> {
    Ok(match reader.byte()? {
        0 => return Err(Error::RetiredVocabulary),
        1 => ProviderBinding::Syscall {
            number: reader.i64()?,
        },
        2 => ProviderBinding::CompilerIntrinsic {
            machine: reader.string()?,
        },
        3 => ProviderBinding::VtableSlot {
            index: reader.i64()?,
        },
        4 => ProviderBinding::VtableField {
            table: reader.string()?,
            field: reader.string()?,
        },
        5 => ProviderBinding::TableFunction {
            table: reader.string()?,
            field: reader.string()?,
        },
        6 => ProviderBinding::CheckedAdapter {
            machine_identity: reader.string()?,
            machine_package_identity: reader.option(package)?,
        },
        7 => ProviderBinding::Import {
            evaluated: normalized_import(reader)?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

/// Rebuilds a normalized evaluated import: the stored candidate is
/// renormalized (which recomputes its identity), the recomputed locator digest
/// must equal both stored copies, and the retained-evidence ctor revalidates
/// the locator/receipt join.
fn normalized_import(reader: &mut Reader<'_>) -> Result<EvaluatedForeignImport, Error> {
    let target_name = reader.string()?;
    let target = TargetProfile::ALL
        .iter()
        .copied()
        .find(|profile| profile.target_name() == target_name)
        .ok_or(Error::InvalidIdentity)?;
    let stored_compatibility_fingerprint = reader.u64()?;
    let candidate = foreign_locator_candidate(reader)?;
    let stored_locator_identity_digest = reader.digest()?;
    let normalized =
        normalize_foreign_locator(candidate, target).map_err(|_| Error::InvalidValue)?;
    if normalized.non_authoritative_compatibility_fingerprint() != stored_compatibility_fingerprint
        || normalized.identity_digest().as_bytes() != stored_locator_identity_digest
    {
        return Err(Error::InvalidIdentity);
    }
    let receipt = binding_receipt(reader, normalized.identity_digest())?;
    EvaluatedForeignImport::from_retained_evidence(normalized, receipt)
        .map_err(|_| Error::InvalidValue)
}

fn foreign_locator_candidate(reader: &mut Reader<'_>) -> Result<ForeignLocatorCandidate, Error> {
    Ok(match reader.byte()? {
        0 => ForeignLocatorCandidate::PeByName {
            library: reader.bytes()?,
            export: reader.bytes()?,
        },
        1 => ForeignLocatorCandidate::PeByOrdinal {
            library: reader.bytes()?,
            ordinal: reader.u16()?,
        },
        2 => ForeignLocatorCandidate::ElfVersioned {
            object: reader.bytes()?,
            symbol: reader.bytes()?,
            version: reader.bytes()?,
        },
        3 => ForeignLocatorCandidate::MachODylibSymbol {
            install_name: reader.bytes()?,
            symbol: reader.bytes()?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

fn binding_receipt(
    reader: &mut Reader<'_>,
    locator_identity_digest: ForeignLocatorIdentityDigest,
) -> Result<EvaluatedBindingReceipt, Error> {
    let producer_package = reader.option(package)?;
    let producer_callable_identity = reader.string()?;
    let producer_closure_digest =
        EvaluatedBindingProducerClosureDigest::from_bytes(reader.digest()?)
            .map_err(|_| Error::InvalidValue)?;
    let evaluator_semantics_marker = reader.u32()?;
    let evaluation_usage = EvaluatedBindingUsage::from_evaluator(
        reader.u32()?,
        reader.u32()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
    )
    .map_err(|_| Error::InvalidValue)?;
    let evaluation_digest = EvaluatedBindingEvaluationDigest::from_bytes(reader.digest()?)
        .map_err(|_| Error::InvalidValue)?;
    let materializer_schema_version = reader.u32()?;
    let materialization_digest =
        EvaluatedBindingMaterializationDigest::from_bytes(reader.digest()?)
            .map_err(|_| Error::InvalidValue)?;
    let stored_locator_identity_digest = reader.digest()?;
    let stored_receipt_identity_digest = reader.digest()?;
    if stored_locator_identity_digest != locator_identity_digest.as_bytes() {
        return Err(Error::InvalidIdentity);
    }
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        producer_package,
        producer_callable_identity,
        producer_closure_digest,
        evaluator_semantics_marker,
        evaluation_usage,
        evaluation_digest,
        materializer_schema_version,
        materialization_digest,
        locator_identity_digest,
    )
    .map_err(|_| Error::InvalidValue)?;
    if receipt.identity_digest() != stored_receipt_identity_digest {
        return Err(Error::InvalidIdentity);
    }
    Ok(receipt)
}

fn provider_row_declaration(
    reader: &mut Reader<'_>,
) -> Result<CheckedPackageProviderRowIdentity, Error> {
    Ok(CheckedPackageProviderRowIdentity {
        requirement: nominal(reader)?,
        realization: reader.option(nominal)?,
        compiler_intrinsic_execution: reader.option(compiler_intrinsic_execution)?,
        installation_reach: reader.option(|reader| {
            Ok(PackageReviewSelectedInstallationReach {
                upper_bound: reader.sequence(30, nominal)?,
                resolved: reader.sequence(30, nominal)?,
            })
        })?,
    })
}

fn provider_family(reader: &mut Reader<'_>) -> Result<CheckedPackageProviderFamilyReview, Error> {
    Ok(CheckedPackageProviderFamilyReview {
        family_identity: nominal(reader)?,
        provider_type_declaration: nominal(reader)?,
        target: target_profile(reader)?,
        authority: match reader.byte()? {
            0 => PackageReviewProviderSelectionAuthority::BuildOverride,
            1 => PackageReviewProviderSelectionAuthority::TargetDefault,
            _ => return Err(Error::InvalidTag),
        },
        coverage: match reader.byte()? {
            0 => PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily,
            _ => return Err(Error::InvalidTag),
        },
        coordinates: reader.sequence(30, |reader| {
            Ok(CheckedPackageProviderFamilyCoordinateReview {
                requirement_identity: reader.string()?,
                operator_declaration: nominal(reader)?,
                plan_report_fingerprint: reader.u64()?,
            })
        })?,
    })
}

// ---------------------------------------------------------------------------
// Boundary applications

fn boundary_application_realization(
    reader: &mut Reader<'_>,
) -> Result<CheckedPackageBoundaryApplicationRealizationReview, Error> {
    Ok(CheckedPackageBoundaryApplicationRealizationReview {
        operator_declaration: nominal(reader)?,
        requirement_identity: reader.string()?,
        application: boundary_application(reader)?,
        selected_plan_digest: reader.digest()?,
        realization: match reader.byte()? {
            0 => PackageReviewBoundaryApplicationRealization::NongenericCheckedBody {
                realization_machine: nominal(reader)?,
                realization_state: nominal(reader)?,
                realization_contract_commitment: reader.digest()?,
            },
            2 => PackageReviewBoundaryApplicationRealization::SpecializedCheckedBody {
                realization_template: nominal(reader)?,
                realization_machine: nominal(reader)?,
                realization_state: nominal(reader)?,
                specialization_commitment: reader.digest()?,
                realization_contract_commitment: reader.digest()?,
            },
            1 => PackageReviewBoundaryApplicationRealization::ExactCompilerIntrinsic {
                execution: compiler_intrinsic_execution(reader)?,
            },
            _ => return Err(Error::InvalidTag),
        },
    })
}

fn boundary_application_demand(
    reader: &mut Reader<'_>,
) -> Result<CheckedPackageBoundaryApplicationDemandReview, Error> {
    Ok(CheckedPackageBoundaryApplicationDemandReview {
        operator_coordinate: operator_coordinate(reader)?,
        requirement_identity: reader.string()?,
        producer_callable: nominal(reader)?,
        arguments: reader.sequence(9, |reader| {
            Ok(match reader.byte()? {
                0 => PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                    requirement_binder_ordinal: reader.u32()?,
                    producer_binder_ordinal: reader.u32()?,
                },
                _ => return Err(Error::InvalidTag),
            })
        })?,
    })
}
