use super::*;
use omega_effects::provider_plan::{
    ProviderBinding, ServiceEntryAuthorityFlow, ServiceProgressEstablishmentRouteKind,
    ServiceProgressSubject,
};
use psi_checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment,
};

const MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW\0";
pub const PACKAGE_REVIEW_ENCODING_VERSION: u16 = 33;
pub(super) const ROW_MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-ROW\0";
pub const PACKAGE_REVIEW_ROW_ENCODING_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageReviewEncodingLimits {
    maximum_review_bytes: usize,
    maximum_rows: usize,
    maximum_row_key_bytes: usize,
    maximum_row_bytes: usize,
    maximum_total_row_bytes: usize,
}

impl PackageReviewEncodingLimits {
    pub const fn new(
        maximum_review_bytes: usize,
        maximum_rows: usize,
        maximum_row_key_bytes: usize,
        maximum_row_bytes: usize,
        maximum_total_row_bytes: usize,
    ) -> Self {
        Self {
            maximum_review_bytes,
            maximum_rows,
            maximum_row_key_bytes,
            maximum_row_bytes,
            maximum_total_row_bytes,
        }
    }
}

impl Default for PackageReviewEncodingLimits {
    fn default() -> Self {
        Self::new(
            16 * 1024 * 1024,
            65_536,
            1024 * 1024,
            4 * 1024 * 1024,
            16 * 1024 * 1024,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewEncodingError {
    message: &'static str,
}

impl PackageReviewEncodingError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for PackageReviewEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for PackageReviewEncodingError {}

pub(super) fn encode(
    review: &CheckedPackageReviewProjection,
) -> Result<Vec<u8>, PackageReviewEncodingError> {
    encode_with_limits(review, PackageReviewEncodingLimits::default())
}

fn encode_with_limits(
    review: &CheckedPackageReviewProjection,
    limits: PackageReviewEncodingLimits,
) -> Result<Vec<u8>, PackageReviewEncodingError> {
    let mut encoder = Encoder::bounded(limits.maximum_review_bytes);
    encoder.fixed_bytes(MAGIC);
    encoder.u16(PACKAGE_REVIEW_ENCODING_VERSION);
    encoder.package_identity(review.package);
    encoder.string(review.target.target_name())?;
    encoder.sequence(&review.public_traits, encode_trait_shape)?;
    encoder.sequence(&review.public_domains, encode_domain_shape)?;
    encoder.sequence(&review.public_data, encode_data_shape)?;
    encoder.sequence(&review.representation_tcb, encode_representation_tcb)?;
    encoder.sequence(&review.callables, encode_callable)?;
    encoder.sequence(&review.dangerous_authorities, encode_dangerous_authority)?;
    encoder.sequence(
        &review.dangerous_authority_slack,
        encode_dangerous_authority_slack,
    )?;
    encoder.sequence(&review.selected_providers, encode_provider)?;
    encoder.finish()
}

pub(super) fn encode_rows(
    review: &CheckedPackageReviewProjection,
) -> Result<Vec<PackageReviewCanonicalRow>, PackageReviewEncodingError> {
    encode_rows_with_limits(review, PackageReviewEncodingLimits::default())
}

fn encode_rows_with_limits(
    review: &CheckedPackageReviewProjection,
    limits: PackageReviewEncodingLimits,
) -> Result<Vec<PackageReviewCanonicalRow>, PackageReviewEncodingError> {
    let required_rows = review
        .public_traits
        .len()
        .saturating_add(review.public_domains.len())
        .saturating_add(review.public_data.len())
        .saturating_add(review.representation_tcb.len())
        .saturating_add(review.callables.len())
        .saturating_add(
            review
                .callables
                .iter()
                .filter(|callable| callable.supply == MachineSupplyMode::Accepted)
                .count(),
        )
        .saturating_add(review.dangerous_authorities.len())
        .saturating_add(review.dangerous_authority_slack.len())
        .saturating_add(2);
    if required_rows > limits.maximum_rows {
        return Err(PackageReviewEncodingError::new(
            "package review exceeds the canonical row-count ceiling",
        ));
    }
    let mut rows = Vec::new();
    rows.try_reserve(required_rows)
        .map_err(|_| PackageReviewEncodingError::new("package review row allocation failed"))?;
    let mut total_row_bytes = 0usize;
    push_row(
        &mut rows,
        &mut total_row_bytes,
        limits,
        encode_row(
            review,
            limits,
            PackageReviewCanonicalRowKind::ProjectionHeader,
            PackageReviewCanonicalRowRisk::Blocking,
            PackageReviewCanonicalRowSource::compiler_derived(
                PackageReviewSyntheticSourceKind::ProjectionHeader,
            ),
            |_| Ok(()),
            |_| Ok(()),
        )?,
    )?;
    for (index, shape) in review.public_traits.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::PublicTrait,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.public_traits, index)?,
                |encoder| encode_nominal(encoder, &shape.identity),
                |encoder| encode_trait_shape(encoder, shape),
            )?,
        )?;
    }
    for (index, shape) in review.public_domains.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::PublicDomain,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.public_domains, index)?,
                |encoder| encode_nominal(encoder, &shape.identity),
                |encoder| encode_domain_shape(encoder, shape),
            )?,
        )?;
    }
    for (index, shape) in review.public_data.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::PublicData,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.public_data, index)?,
                |encoder| encode_nominal(encoder, &shape.identity),
                |encoder| encode_data_shape(encoder, shape),
            )?,
        )?;
    }
    for (index, row) in review.representation_tcb.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::RepresentationTcb,
                PackageReviewCanonicalRowRisk::AuditRecommended,
                row_source(&review.row_sources.representation_tcb, index)?,
                |encoder| encode_nominal(encoder, &row.declaration),
                |encoder| encode_representation_tcb(encoder, row),
            )?,
        )?;
    }
    for (index, callable) in review.callables.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::Callable,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.callables, index)?,
                |encoder| encode_nominal(encoder, &callable.identity),
                |encoder| encode_callable(encoder, callable),
            )?,
        )?;
        if callable.supply == MachineSupplyMode::Accepted {
            push_row(
                &mut rows,
                &mut total_row_bytes,
                limits,
                encode_row(
                    review,
                    limits,
                    PackageReviewCanonicalRowKind::AcceptedClaim,
                    PackageReviewCanonicalRowRisk::Blocking,
                    row_source(&review.row_sources.callables, index)?,
                    |encoder| encode_nominal(encoder, &callable.identity),
                    |encoder| encode_callable(encoder, callable),
                )?,
            )?;
        }
    }
    for (index, authority) in review.dangerous_authorities.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::DangerousAuthority,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.dangerous_authorities, index)?,
                |encoder| encode_nominal(encoder, &authority.service),
                |encoder| encode_dangerous_authority(encoder, authority),
            )?,
        )?;
    }
    for (index, slack) in review.dangerous_authority_slack.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::DangerousAuthoritySlack,
                PackageReviewCanonicalRowRisk::AuditRecommended,
                row_source(&review.row_sources.dangerous_authority_slack, index)?,
                |encoder| {
                    encode_nominal(encoder, &slack.callable)?;
                    encode_nominal(encoder, &slack.service)
                },
                |encoder| encode_dangerous_authority_slack(encoder, slack),
            )?,
        )?;
    }
    push_row(
        &mut rows,
        &mut total_row_bytes,
        limits,
        encode_row(
            review,
            limits,
            PackageReviewCanonicalRowKind::SelectedProviderSet,
            PackageReviewCanonicalRowRisk::OpaqueBlocking,
            review.row_sources.selected_provider_set.clone(),
            |_| Ok(()),
            |encoder| encoder.sequence(&review.selected_providers, encode_provider),
        )?,
    )?;
    rows.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then(left.key_bytes.cmp(&right.key_bytes))
    });
    if rows
        .windows(2)
        .any(|pair| pair[0].kind == pair[1].kind && pair[0].key_bytes == pair[1].key_bytes)
    {
        return Err(PackageReviewEncodingError::new(
            "package review contains duplicate canonical row keys",
        ));
    }
    Ok(rows)
}

fn push_row(
    rows: &mut Vec<PackageReviewCanonicalRow>,
    total_row_bytes: &mut usize,
    limits: PackageReviewEncodingLimits,
    row: PackageReviewCanonicalRow,
) -> Result<(), PackageReviewEncodingError> {
    *total_row_bytes = total_row_bytes
        .checked_add(row.key_bytes.len())
        .and_then(|total| total.checked_add(row.canonical_bytes.len()))
        .ok_or_else(|| {
            PackageReviewEncodingError::new(
                "package review exceeds the total canonical-row byte ceiling",
            )
        })?;
    if *total_row_bytes > limits.maximum_total_row_bytes {
        return Err(PackageReviewEncodingError::new(
            "package review exceeds the total canonical-row byte ceiling",
        ));
    }
    rows.push(row);
    Ok(())
}

fn encode_row(
    review: &CheckedPackageReviewProjection,
    limits: PackageReviewEncodingLimits,
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    source: PackageReviewCanonicalRowSource,
    encode_key: impl FnOnce(&mut Encoder) -> Result<(), PackageReviewEncodingError>,
    encode_value: impl FnOnce(&mut Encoder) -> Result<(), PackageReviewEncodingError>,
) -> Result<PackageReviewCanonicalRow, PackageReviewEncodingError> {
    let mut key = Encoder::bounded(limits.maximum_row_key_bytes);
    encode_key(&mut key)?;
    let key_bytes = key.finish()?;
    let mut value = Encoder::bounded(limits.maximum_row_bytes);
    encode_value(&mut value)?;
    let value_bytes = value.finish()?;
    let mut canonical = Encoder::bounded(limits.maximum_row_bytes);
    canonical.fixed_bytes(ROW_MAGIC);
    canonical.u16(PACKAGE_REVIEW_ROW_ENCODING_VERSION);
    canonical.u16(PACKAGE_REVIEW_ENCODING_VERSION);
    canonical.package_identity(review.package);
    canonical.string(review.target.target_name())?;
    canonical.byte(canonical_row_kind_tag(kind));
    canonical.byte(canonical_row_risk_tag(risk));
    canonical.bytes(&key_bytes)?;
    canonical.bytes(&value_bytes)?;
    Ok(PackageReviewCanonicalRow {
        kind,
        risk,
        key_bytes,
        canonical_bytes: canonical.finish()?,
        source,
    })
}

fn row_source(
    sources: &[PackageReviewCanonicalRowSource],
    index: usize,
) -> Result<PackageReviewCanonicalRowSource, PackageReviewEncodingError> {
    sources.get(index).cloned().ok_or_else(|| {
        PackageReviewEncodingError::new(
            "package review canonical row has no compiler-issued source disposition",
        )
    })
}

const fn canonical_row_risk_tag(risk: PackageReviewCanonicalRowRisk) -> u8 {
    match risk {
        PackageReviewCanonicalRowRisk::Blocking => 0,
        PackageReviewCanonicalRowRisk::AuditRecommended => 1,
        PackageReviewCanonicalRowRisk::OpaqueBlocking => 2,
    }
}

const fn canonical_row_kind_tag(kind: PackageReviewCanonicalRowKind) -> u8 {
    match kind {
        PackageReviewCanonicalRowKind::ProjectionHeader => 0,
        PackageReviewCanonicalRowKind::PublicTrait => 1,
        PackageReviewCanonicalRowKind::PublicDomain => 2,
        PackageReviewCanonicalRowKind::PublicData => 3,
        PackageReviewCanonicalRowKind::RepresentationTcb => 4,
        PackageReviewCanonicalRowKind::Callable => 5,
        PackageReviewCanonicalRowKind::DangerousAuthority => 6,
        PackageReviewCanonicalRowKind::SelectedProviderSet => 7,
        PackageReviewCanonicalRowKind::AcceptedClaim => 8,
        PackageReviewCanonicalRowKind::DangerousAuthoritySlack => 9,
    }
}

fn encode_representation_tcb(
    encoder: &mut Encoder,
    row: &PackageReviewRepresentationTcb,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &row.declaration)?;
    encoder.byte(match row.abi {
        PackageReviewRepresentationAbiCommitment::Unbound => 0,
    });
    encoder.byte(match row.mechanism {
        PackageReviewRepresentationMechanism::Unbound => 0,
    });
    Ok(())
}

fn encode_dangerous_authority(
    encoder: &mut Encoder,
    authority: &PackageReviewDangerousAuthority,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match authority.class {
        PackageReviewDangerousAuthorityClass::Filesystem => 0,
        PackageReviewDangerousAuthorityClass::MachineControl => 1,
        PackageReviewDangerousAuthorityClass::PortIo => 2,
        PackageReviewDangerousAuthorityClass::InterruptControl => 3,
        PackageReviewDangerousAuthorityClass::InterruptEntry => 4,
        PackageReviewDangerousAuthorityClass::RootMemory => 5,
        PackageReviewDangerousAuthorityClass::Process => 6,
    });
    encode_nominal(encoder, &authority.service)
}

fn encode_dangerous_authority_slack(
    encoder: &mut Encoder,
    slack: &PackageReviewDangerousAuthoritySlack,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match slack.class {
        PackageReviewDangerousAuthorityClass::Filesystem => 0,
        PackageReviewDangerousAuthorityClass::MachineControl => 1,
        PackageReviewDangerousAuthorityClass::PortIo => 2,
        PackageReviewDangerousAuthorityClass::InterruptControl => 3,
        PackageReviewDangerousAuthorityClass::InterruptEntry => 4,
        PackageReviewDangerousAuthorityClass::RootMemory => 5,
        PackageReviewDangerousAuthorityClass::Process => 6,
    });
    encode_nominal(encoder, &slack.callable)?;
    encode_nominal(encoder, &slack.service)
}

fn encode_trait_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewTraitShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.boolean(shape.is_boundary);
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encoder.sequence(&shape.conformance_bounds, encode_conformance_bound)?;
    encoder.sequence(&shape.parents, encode_trait_parent)?;
    encoder.sequence(&shape.requirements, encode_trait_requirement)
}

fn encode_conformance_bound(
    encoder: &mut Encoder,
    bound: &PackageReviewConformanceBound,
) -> Result<(), PackageReviewEncodingError> {
    match bound.binder_ordinal {
        None => encoder.byte(0),
        Some(ordinal) => {
            encoder.byte(1);
            encoder.u32(ordinal);
        }
    }
    encoder.u32(bound.subject_parameter);
    match (&bound.selected_conformance, &bound.selected_carrier) {
        (None, None) => encoder.byte(0),
        (Some(conformance), Some(carrier)) => {
            encoder.byte(1);
            encode_nominal(encoder, conformance)?;
            encode_nominal(encoder, carrier)?;
            encoder.sequence(&bound.selected_carrier_arguments, encode_type_identity)?;
        }
        _ => {
            return Err(PackageReviewEncodingError::new(
                "selected conformance review row has an incomplete carrier identity",
            ));
        }
    }
    encode_nominal(encoder, &bound.trait_identity)?;
    encoder.sequence(&bound.arguments, encode_type_identity)
}

fn encode_trait_parent(
    encoder: &mut Encoder,
    parent: &PackageReviewTraitParent,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match parent.kind {
        psi_typed_trees::trait_definition::TraitCompositionKind::Policy => 0,
        psi_typed_trees::trait_definition::TraitCompositionKind::ServiceReach => 1,
    });
    encode_nominal(encoder, &parent.identity)?;
    encoder.sequence(&parent.lifetime_arguments, |encoder, argument| {
        encoder.u32(*argument);
        Ok(())
    })?;
    encoder.sequence(&parent.arguments, encode_type_identity)
}

fn encode_trait_requirement(
    encoder: &mut Encoder,
    requirement: &PackageReviewTraitRequirement,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &requirement.identity)?;
    match requirement.spelling {
        None => encoder.byte(0),
        Some(spelling) => {
            encoder.byte(1);
            encoder.byte(match spelling {
                psi_language_core::OperatorSpelling::Add => 0,
                psi_language_core::OperatorSpelling::Subtract => 1,
                psi_language_core::OperatorSpelling::Multiply => 2,
                psi_language_core::OperatorSpelling::Divide => 3,
                psi_language_core::OperatorSpelling::Modulo => 4,
                psi_language_core::OperatorSpelling::Equal => 5,
                psi_language_core::OperatorSpelling::NotEqual => 6,
                psi_language_core::OperatorSpelling::Less => 7,
                psi_language_core::OperatorSpelling::LessEqual => 8,
                psi_language_core::OperatorSpelling::Greater => 9,
                psi_language_core::OperatorSpelling::GreaterEqual => 10,
                psi_language_core::OperatorSpelling::Index => 11,
                psi_language_core::OperatorSpelling::Range => 12,
            });
        }
    }
    encoder.boolean(requirement.has_default_realization);
    encoder.usize(requirement.lifetime_parameter_count)?;
    encoder.sequence(&requirement.type_parameters, encode_type_parameter)?;
    encoder.sequence(&requirement.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &requirement.return_type)?;
    encoder.sequence(&requirement.contracts, encode_callable_contract)?;
    encoder.sequence(&requirement.published_crash, encode_crash_route)?;
    encoder.sequence(&requirement.service_reach, encode_nominal)?;
    encoder.boolean(requirement.service_reach_is_installation_bound);
    encoder.sequence(
        &requirement.synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.boolean(requirement.suspends);
    encoder.boolean(requirement.blocks);
    encode_termination(encoder, &requirement.termination)?;
    Ok(())
}

fn encode_domain_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewDomainShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encode_type_identity(encoder, &shape.target_type)?;
    encoder.sequence(&shape.index_arguments, encode_type_identity)?;
    encoder.byte(match shape.predicate_body {
        psi_language_semantics::DomainPredicateBody::Bodyless => 0,
        psi_language_semantics::DomainPredicateBody::Present => 1,
    });
    encoder.sequence(&shape.predicate_facts, encode_contract_fact)?;
    match &shape.alias_expansion {
        None => encoder.byte(0),
        Some(atoms) => {
            encoder.byte(1);
            encoder.sequence(atoms, encode_nominal)?;
        }
    }
    match shape.classification {
        None => encoder.byte(0),
        Some(PackageReviewDomainClassification::ProgressProfile) => encoder.byte(1),
    }
    encoder.sequence(
        &shape.establishment_routes,
        encode_domain_establishment_route,
    )
}

fn encode_domain_establishment_route(
    encoder: &mut Encoder,
    route: &PackageReviewDomainEstablishmentRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match route.kind {
        PackageReviewDomainEstablishmentKind::CheckedRequirement => 0,
        PackageReviewDomainEstablishmentKind::BoundaryRequirement => 1,
    });
    encode_nominal(encoder, &route.trait_identity)?;
    encode_nominal(encoder, &route.requirement_identity)
}

fn encode_data_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewDataShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.byte(match shape.supply {
        psi_language_semantics::DataSupplyMode::CheckedShape => 0,
        psi_language_semantics::DataSupplyMode::BoundaryOpaque => 1,
    });
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encode_data_properties(encoder, shape.properties);
    encoder.boolean(shape.zero_gated);
    encoder.sequence(&shape.retired_identities, |encoder, identity| {
        encoder.u64(*identity);
        Ok(())
    })?;
    encoder.sequence(&shape.members, encode_data_member)
}

fn encode_type_parameter(
    encoder: &mut Encoder,
    parameter: &PackageReviewTypeParameter,
) -> Result<(), PackageReviewEncodingError> {
    match &parameter.kind {
        PackageReviewTypeParameterKind::Type => encoder.byte(0),
        PackageReviewTypeParameterKind::Const(type_identity) => {
            encoder.byte(1);
            encode_type_identity(encoder, type_identity)?;
        }
    }
    encode_data_properties(encoder, parameter.bounds);
    Ok(())
}

fn encode_data_properties(
    encoder: &mut Encoder,
    properties: psi_typed_trees::data::DataProperties,
) {
    encoder.byte(match properties.multiplicity {
        psi_language_semantics::Multiplicity::Unrestricted => 0,
        psi_language_semantics::Multiplicity::Affine => 1,
        psi_language_semantics::Multiplicity::Linear => 2,
    });
    match properties.carry {
        None => encoder.byte(0),
        Some(carry) => {
            encoder.byte(1);
            encoder.byte(match carry.suspension {
                psi_language_semantics::CarrySuspension::Forbidden => 0,
                psi_language_semantics::CarrySuspension::Allowed => 1,
            });
            encoder.byte(match carry.cpu {
                psi_language_semantics::CarryCpu::Origin => 0,
                psi_language_semantics::CarryCpu::Any => 1,
            });
            encoder.byte(match carry.host_thread {
                psi_language_semantics::CarryHostThread::Origin => 0,
                psi_language_semantics::CarryHostThread::Any => 1,
            });
            encoder.byte(match carry.address {
                psi_language_semantics::CarryAddress::Stable => 0,
                psi_language_semantics::CarryAddress::Movable => 1,
            });
        }
    }
}

fn encode_data_member(
    encoder: &mut Encoder,
    member: &PackageReviewDataMember,
) -> Result<(), PackageReviewEncodingError> {
    match member {
        PackageReviewDataMember::Field(field) => {
            encoder.byte(0);
            encode_data_field(encoder, field)?;
        }
        PackageReviewDataMember::Variant {
            identity,
            name,
            payload,
            retired_payload_identities,
        } => {
            encoder.byte(1);
            encode_optional_u64(encoder, *identity);
            encoder.string(name)?;
            encoder.sequence(payload, encode_data_field)?;
            encoder.sequence(retired_payload_identities, |encoder, identity| {
                encoder.u64(*identity);
                Ok(())
            })?;
        }
    }
    Ok(())
}

fn encode_data_field(
    encoder: &mut Encoder,
    field: &PackageReviewDataField,
) -> Result<(), PackageReviewEncodingError> {
    encode_optional_u64(encoder, field.identity);
    encoder.string(&field.name)?;
    encode_relevance(encoder, field.relevance);
    encode_type_identity(encoder, &field.type_identity)
}

fn encode_type_identity(
    encoder: &mut Encoder,
    identity: &PackageReviewTypeIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&identity.canonical)
}

fn encode_relevance(encoder: &mut Encoder, relevance: psi_language_core::BindingRelevance) {
    encoder.byte(match relevance {
        psi_language_core::BindingRelevance::Relevant => 0,
        psi_language_core::BindingRelevance::Erased => 1,
    });
}

fn encode_optional_u64(encoder: &mut Encoder, value: Option<u64>) {
    match value {
        None => encoder.byte(0),
        Some(value) => {
            encoder.byte(1);
            encoder.u64(value);
        }
    }
}

struct Encoder {
    output: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
}

impl Encoder {
    fn bounded(maximum_bytes: usize) -> Self {
        Self {
            output: Vec::new(),
            maximum_bytes,
            exceeded: false,
        }
    }

    fn finish(self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        if self.exceeded {
            Err(PackageReviewEncodingError::new(
                "package review exceeds its canonical encoding byte ceiling",
            ))
        } else {
            Ok(self.output)
        }
    }

    fn append(&mut self, bytes: &[u8]) {
        if self.exceeded {
            return;
        }
        let Some(required) = self.output.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return;
        };
        if required > self.maximum_bytes || self.output.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.output.extend_from_slice(bytes);
    }

    fn fixed_bytes(&mut self, value: &[u8]) {
        self.append(value);
    }

    fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }

    fn boolean(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.append(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.append(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) -> Result<(), PackageReviewEncodingError> {
        self.u64(u64::try_from(value).map_err(|_| {
            PackageReviewEncodingError::new(
                "package review value exceeds the portable encoding range",
            )
        })?);
        Ok(())
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), PackageReviewEncodingError> {
        self.usize(value.len())?;
        self.append(value);
        self.check()
    }

    fn string(&mut self, value: &str) -> Result<(), PackageReviewEncodingError> {
        self.bytes(value.as_bytes())
    }

    fn sequence<T>(
        &mut self,
        values: &[T],
        encode_value: impl Fn(&mut Self, &T) -> Result<(), PackageReviewEncodingError>,
    ) -> Result<(), PackageReviewEncodingError> {
        self.usize(values.len())?;
        for value in values {
            encode_value(self, value)?;
        }
        Ok(())
    }

    fn option<T: ?Sized>(
        &mut self,
        value: Option<&T>,
        encode_value: impl Fn(&mut Self, &T) -> Result<(), PackageReviewEncodingError>,
    ) -> Result<(), PackageReviewEncodingError> {
        match value {
            None => self.byte(0),
            Some(value) => {
                self.byte(1);
                encode_value(self, value)?;
            }
        }
        Ok(())
    }

    fn package_identity(&mut self, identity: PackageKeyIdentity) {
        self.append(&identity.digest());
    }

    fn optional_package_identity(&mut self, identity: Option<PackageKeyIdentity>) {
        match identity {
            None => self.byte(0),
            Some(identity) => {
                self.byte(1);
                self.package_identity(identity);
            }
        }
    }

    fn check(&self) -> Result<(), PackageReviewEncodingError> {
        if self.exceeded {
            Err(PackageReviewEncodingError::new(
                "package review exceeds its canonical encoding byte ceiling",
            ))
        } else {
            Ok(())
        }
    }
}

fn encode_callable(
    encoder: &mut Encoder,
    callable: &CheckedPackageCallableReview,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match callable.role {
        PackageReviewCallableRole::Boundary => 0,
        PackageReviewCallableRole::Public => 1,
        PackageReviewCallableRole::Build => 2,
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
    encode_type_identity(encoder, &callable.return_type)?;
    encoder.sequence(&callable.conformances, |encoder, conformance| {
        encode_nominal(encoder, &conformance.trait_identity)?;
        encode_nominal(encoder, &conformance.requirement_identity)?;
        encoder.sequence(&conformance.arguments, encode_type_identity)?;
        encoder.option(conformance.alias.as_deref(), |encoder, alias| {
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
        |encoder, invocations| encoder.sequence(invocations, encode_synchronous_invocation),
    )?;
    encoder.sequence(
        &callable.realized_synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.sequence(&callable.capability_flows, encode_capability_flow)?;
    encoder.boolean(callable.checked_may_suspend);
    encoder.boolean(callable.checked_may_block);
    encode_termination(encoder, &callable.checked_termination)?;
    encode_crash(encoder, &callable.checked_crash)?;
    encoder.sequence(&callable.mutation, encode_mutation)
}

fn encode_callable_contract(
    encoder: &mut Encoder,
    contract: &PackageReviewCallableContract,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match contract.kind {
        PackageReviewContractKind::Requires => 0,
        PackageReviewContractKind::Ensures => 1,
        PackageReviewContractKind::Boundary => 2,
    });
    encoder.option(contract.binding.as_deref(), |encoder, binding| {
        encoder.string(binding)
    })?;
    encoder.option(
        contract.evidence_lane_position.as_ref(),
        |encoder, position| {
            encoder.u32(*position);
            Ok(())
        },
    )?;
    encode_contract_fact(encoder, &contract.fact)
}

fn encode_contract_fact(
    encoder: &mut Encoder,
    fact: &PackageReviewContractFact,
) -> Result<(), PackageReviewEncodingError> {
    match fact {
        PackageReviewContractFact::Expression(expression) => {
            encoder.byte(0);
            encode_contract_expression(encoder, expression)
        }
        PackageReviewContractFact::Membership { value, domain } => {
            encoder.byte(1);
            encode_contract_expression(encoder, value)?;
            encode_nominal(encoder, domain)
        }
        PackageReviewContractFact::Proposition(application) => {
            encoder.byte(2);
            encode_proposition_application(encoder, application)
        }
    }
}

fn encode_proposition_application(
    encoder: &mut Encoder,
    application: &PackageReviewPropositionApplication,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &application.declaration)?;
    encoder.sequence(&application.binders, |encoder, binder| {
        match &binder.kind {
            PackageReviewPropositionBinderKind::Type => encoder.byte(0),
            PackageReviewPropositionBinderKind::Const(type_identity) => {
                encoder.byte(1);
                encode_type_identity(encoder, type_identity)?;
            }
            PackageReviewPropositionBinderKind::Machine => encoder.byte(2),
        }
        encode_data_properties(encoder, binder.bounds);
        Ok(())
    })?;
    encoder.sequence(&application.parameter_types, encode_type_identity)?;
    encoder.sequence(&application.binder_arguments, |encoder, argument| {
        encoder.byte(match argument.kind {
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Type => 0,
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Const => 1,
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine => 2,
        });
        match &argument.value {
            PackageReviewPropositionBinderValue::Nominal(identity) => {
                encoder.byte(0);
                encode_nominal(encoder, identity)?;
            }
            PackageReviewPropositionBinderValue::GenericBinder(position) => {
                encoder.byte(1);
                encoder.u32(*position);
            }
            PackageReviewPropositionBinderValue::Integer(value) => {
                encoder.byte(2);
                encoder.string(value)?;
            }
            PackageReviewPropositionBinderValue::EvidenceProjection {
                source_kind,
                source_lane_position,
                declaring_trait,
                declaring_trait_arguments,
                requirement,
            } => {
                encoder.byte(3);
                encoder.byte(match source_kind {
                    PackageReviewContractKind::Requires => 0,
                    PackageReviewContractKind::Ensures => 1,
                    PackageReviewContractKind::Boundary => 2,
                });
                encoder.u32(*source_lane_position);
                encode_nominal(encoder, declaring_trait)?;
                encoder.sequence(declaring_trait_arguments, encode_type_identity)?;
                encode_nominal(encoder, requirement)?;
            }
        }
        Ok(())
    })?;
    encoder.sequence(&application.arguments, encode_contract_expression)?;
    match &application.evidence {
        PackageReviewPropositionEvidence::FactOnly => encoder.byte(0),
        PackageReviewPropositionEvidence::Witness(interface) => {
            encoder.byte(1);
            encode_nominal(encoder, &interface.trait_identity)?;
            encoder.sequence(&interface.arguments, encode_type_identity)?;
            encoder.sequence(&interface.requirements, |encoder, requirement| {
                encode_nominal(encoder, &requirement.declaring_trait)?;
                encoder.sequence(&requirement.declaring_trait_arguments, encode_type_identity)?;
                encode_nominal(encoder, &requirement.requirement)
            })?;
        }
    }
    Ok(())
}

fn encode_contract_expression(
    encoder: &mut Encoder,
    expression: &PackageReviewContractExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        PackageReviewContractExpression::Boolean(value) => {
            encoder.byte(0);
            encoder.boolean(*value);
        }
        PackageReviewContractExpression::Integer(value) => {
            encoder.byte(1);
            encoder.string(value)?;
        }
        PackageReviewContractExpression::DomainSubject => encoder.byte(10),
        PackageReviewContractExpression::Parameter(position) => {
            encoder.byte(2);
            encoder.u32(*position);
        }
        PackageReviewContractExpression::Result => encoder.byte(3),
        PackageReviewContractExpression::GenericBinder(position) => {
            encoder.byte(4);
            encoder.u32(*position);
        }
        PackageReviewContractExpression::Nominal(identity) => {
            encoder.byte(5);
            encode_nominal(encoder, identity)?;
        }
        PackageReviewContractExpression::Member {
            receiver,
            member,
            case_variant,
        } => {
            encoder.byte(8);
            encode_contract_expression(encoder, receiver)?;
            encode_nominal(encoder, member)?;
            encoder.option(case_variant.as_ref(), encode_nominal)?;
        }
        PackageReviewContractExpression::Cast {
            value,
            target,
            arithmetic_domain,
            semantic_domain,
            semantic_domain_arguments,
            form,
        } => {
            encoder.byte(9);
            encode_contract_expression(encoder, value)?;
            encode_type_identity(encoder, target)?;
            encoder.byte(match arithmetic_domain {
                PackageReviewArithmeticDomain::Exact => 0,
                PackageReviewArithmeticDomain::Wrapping => 1,
                PackageReviewArithmeticDomain::Saturating => 2,
                PackageReviewArithmeticDomain::Trapping => 3,
            });
            encoder.option(semantic_domain.as_ref(), encode_nominal)?;
            encoder.sequence(semantic_domain_arguments, encode_type_identity)?;
            encoder.byte(match form {
                PackageReviewCastForm::Value => 0,
                PackageReviewCastForm::RecastShared => 1,
                PackageReviewCastForm::RecastMutable => 2,
            });
        }
        PackageReviewContractExpression::Binary {
            operator,
            left,
            right,
        } => {
            encoder.byte(6);
            encoder.byte(match operator {
                PackageReviewContractBinaryOperator::Add => 0,
                PackageReviewContractBinaryOperator::And => 1,
                PackageReviewContractBinaryOperator::BitwiseAnd => 2,
                PackageReviewContractBinaryOperator::BitwiseOr => 3,
                PackageReviewContractBinaryOperator::BitwiseXor => 4,
                PackageReviewContractBinaryOperator::Divide => 5,
                PackageReviewContractBinaryOperator::Equal => 6,
                PackageReviewContractBinaryOperator::Greater => 7,
                PackageReviewContractBinaryOperator::GreaterOrEqual => 8,
                PackageReviewContractBinaryOperator::Less => 9,
                PackageReviewContractBinaryOperator::LessOrEqual => 10,
                PackageReviewContractBinaryOperator::Modulo => 11,
                PackageReviewContractBinaryOperator::Multiply => 12,
                PackageReviewContractBinaryOperator::NotEqual => 13,
                PackageReviewContractBinaryOperator::Or => 14,
                PackageReviewContractBinaryOperator::ShiftLeft => 15,
                PackageReviewContractBinaryOperator::ShiftRight => 16,
                PackageReviewContractBinaryOperator::Subtract => 17,
            });
            encode_contract_expression(encoder, left)?;
            encode_contract_expression(encoder, right)?;
        }
        PackageReviewContractExpression::Unary { operator, operand } => {
            encoder.byte(7);
            encoder.byte(match operator {
                PackageReviewContractUnaryOperator::BitwiseNot => 0,
                PackageReviewContractUnaryOperator::LogicalNot => 1,
            });
            encode_contract_expression(encoder, operand)?;
        }
    }
    Ok(())
}

fn encode_synchronous_invocation(
    encoder: &mut Encoder,
    invocation: &PackageReviewSynchronousInvocation,
) -> Result<(), PackageReviewEncodingError> {
    match invocation {
        PackageReviewSynchronousInvocation::Parameter(position) => {
            encoder.byte(0);
            encoder.u32(*position);
        }
        PackageReviewSynchronousInvocation::Service(service) => {
            encoder.byte(1);
            encode_nominal(encoder, service)?;
        }
    }
    Ok(())
}

fn encode_nominal(
    encoder: &mut Encoder,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), PackageReviewEncodingError> {
    match identity.owner {
        PackageReviewNominalOwner::Package(package) => {
            encoder.byte(0);
            encoder.package_identity(package);
        }
        PackageReviewNominalOwner::ToolchainSource(source) => {
            encoder.byte(1);
            encoder.fixed_bytes(&source.digest());
        }
        PackageReviewNominalOwner::ToolchainUnbound => encoder.byte(2),
        PackageReviewNominalOwner::Unresolved => encoder.byte(3),
    }
    encoder.string(&identity.path)
}

fn encode_supply(
    encoder: &mut Encoder,
    supply: MachineSupplyMode,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match supply {
        MachineSupplyMode::CheckedBody => 0,
        MachineSupplyMode::Requirement => 1,
        MachineSupplyMode::Boundary => 2,
        MachineSupplyMode::Accepted => 3,
        MachineSupplyMode::ExternalRealization { .. } => {
            return Err(PackageReviewEncodingError::new(
                "reviewed callable unexpectedly carries an interner-backed external realization",
            ));
        }
    });
    Ok(())
}

fn encode_installation_reach(
    encoder: &mut Encoder,
    reach: &PackageReviewInstallationReach,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &reach.requirement)?;
    encoder.sequence(&reach.upper_bound, encode_nominal)
}

fn encode_capability_flow(
    encoder: &mut Encoder,
    flow: &PackageReviewCapabilityFlow,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &flow.capability)?;
    encoder.byte(match flow.kind {
        psi_effects::CapabilityFlowKind::Uses => 0,
        psi_effects::CapabilityFlowKind::Returns => 1,
        psi_effects::CapabilityFlowKind::Acquires => 2,
        psi_effects::CapabilityFlowKind::Stores => 3,
        psi_effects::CapabilityFlowKind::Derives => 4,
    });
    encode_nominal(encoder, &flow.state)?;
    encoder.usize(flow.statement_index)?;
    encoder.usize(flow.call_ordinal)?;
    encoder.option(flow.via_state.as_ref(), encode_nominal)
}

fn encode_termination(
    encoder: &mut Encoder,
    termination: &PackageReviewTermination,
) -> Result<(), PackageReviewEncodingError> {
    match termination {
        PackageReviewTermination::NoGuarantee => encoder.byte(0),
        PackageReviewTermination::Terminates { premises } => {
            encoder.byte(1);
            encoder.sequence(premises, |encoder, premise| {
                encode_nominal(encoder, &premise.profile)?;
                match &premise.subject {
                    PackageReviewProgressSubject::Declaration(identity) => {
                        encoder.byte(0);
                        encode_nominal(encoder, identity)?;
                    }
                    PackageReviewProgressSubject::Receiver => encoder.byte(1),
                    PackageReviewProgressSubject::Parameter(position) => {
                        encoder.byte(2);
                        encoder.u32(*position);
                    }
                }
                encoder.sequence(&premise.projections, encode_nominal)
            })?;
        }
    }
    Ok(())
}

fn encode_mutation(
    encoder: &mut Encoder,
    mutation: &PackageReviewMutation,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &mutation.state)?;
    encoder.byte(match mutation.completeness {
        psi_facts::WriteFrameCompleteness::Complete => 0,
        psi_facts::WriteFrameCompleteness::Opaque => 1,
    });
    encoder.sequence(&mutation.paths, |encoder, path| encoder.string(path))
}

fn encode_crash(
    encoder: &mut Encoder,
    crash: &PackageReviewCrash,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match crash.interface {
        PackageReviewCrashInterface::InternalInferred => 0,
        PackageReviewCrashInterface::PublishedCeiling => 1,
    });
    encoder.sequence(&crash.published, encode_crash_route)?;
    encoder.option(
        crash.structural_runtime_requirements.as_deref(),
        |encoder, requirements| encoder.sequence(requirements, encode_boolean_expression),
    )?;
    encoder.sequence(&crash.checked_sites, encode_crash_site)?;
    encoder.sequence(&crash.checked_calls, encode_crash_call)
}

fn encode_crash_route(
    encoder: &mut Encoder,
    route: &PackageReviewCrashRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match route.cause {
        psi_checked_trees::CrashCause::Trap => 0,
        psi_checked_trees::CrashCause::Abort => 1,
    });
    encoder.sequence(&route.alternative_guards, |encoder, guard| {
        match guard {
            PackageReviewCrashRouteGuard::Truth => encoder.byte(0),
            PackageReviewCrashRouteGuard::Predicate(predicate) => {
                encoder.byte(1);
                encoder.bytes(&predicate.canonical_bytes)?;
            }
        }
        Ok(())
    })
}

fn encode_crash_site(
    encoder: &mut Encoder,
    site: &PackageReviewCrashSite,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &site.state)?;
    encoder.u32(site.statement_ordinal);
    encoder.byte(match site.cause {
        psi_checked_trees::CrashCause::Trap => 0,
        psi_checked_trees::CrashCause::Abort => 1,
    });
    encoder.sequence(&site.path_guard_conjuncts, encode_crash_predicate)?;
    encoder.sequence(&site.path_guard_consequences, encode_crash_predicate)?;
    encoder.sequence(&site.guard_covering_buckets, |encoder, bucket| {
        encoder.u32(*bucket);
        Ok(())
    })?;
    encoder.sequence(&site.frontier_lower_bound, encode_permission_claim)
}

fn encode_crash_predicate(
    encoder: &mut Encoder,
    predicate: &PackageReviewCrashPredicate,
) -> Result<(), PackageReviewEncodingError> {
    encoder.bytes(&predicate.canonical_bytes)
}

fn encode_permission_claim(
    encoder: &mut Encoder,
    claim: &PackageReviewPermissionClaim,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &claim.machine)?;
    encode_nominal(encoder, &claim.state)?;
    match &claim.source {
        PackageReviewPermissionSource::StateEntry => encoder.byte(0),
        PackageReviewPermissionSource::Statement { statement_ordinal } => {
            encoder.byte(1);
            encoder.u64(*statement_ordinal);
        }
        PackageReviewPermissionSource::Call {
            statement_ordinal,
            call_ordinal,
            target,
        } => {
            encoder.byte(2);
            encoder.u64(*statement_ordinal);
            encoder.u64(*call_ordinal);
            encode_nominal(encoder, target)?;
        }
        PackageReviewPermissionSource::StateExit => encoder.byte(3),
    }
    encoder.u32(claim.ordinal);
    Ok(())
}

fn encode_crash_call(
    encoder: &mut Encoder,
    call: &PackageReviewCrashCall,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &call.state)?;
    encoder.u32(call.statement_ordinal);
    encoder.u32(call.call_ordinal);
    encode_nominal(encoder, &call.target_machine)?;
    encode_nominal(encoder, &call.target_state)?;
    encoder.sequence(&call.path_guard_conjuncts, encode_crash_predicate)?;
    encoder.sequence(&call.path_guard_consequences, encode_crash_predicate)?;
    encoder.sequence(&call.surviving_buckets, encode_crash_route)
}

fn encode_boolean_expression(
    encoder: &mut Encoder,
    expression: &CheckedBooleanExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        CheckedBooleanExpression::Constant(value) => {
            encoder.byte(0);
            encoder.boolean(*value);
        }
        CheckedBooleanExpression::Parameter { position } => {
            encoder.byte(1);
            encoder.usize(*position)?;
        }
        CheckedBooleanExpression::Local { position } => {
            encoder.byte(2);
            encoder.usize(*position)?;
        }
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => {
            encoder.byte(3);
            encoder.u32(*parameter_position);
            encode_structural_path(encoder, path)?;
        }
        CheckedBooleanExpression::Not(operand) => {
            encoder.byte(4);
            encode_boolean_expression(encoder, operand)?;
        }
        CheckedBooleanExpression::Equal { left, right } => {
            encoder.byte(5);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            encoder.byte(6);
            encoder.byte(integer_comparison_tag(*kind));
            encode_scalar_expression(encoder, left)?;
            encode_scalar_expression(encoder, right)?;
        }
        CheckedBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.byte(7);
            encoder.byte(match kind {
                psi_checked_trees::CheckedIeeeFloatComparisonKind::Equal => 0,
                psi_checked_trees::CheckedIeeeFloatComparisonKind::NotEqual => 1,
            });
            encode_primitive_type(encoder, *primitive_type);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
        }
        CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
            encoder.byte(8);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
        }
        CheckedBooleanExpression::PayloadlessSumEqual { left, right, cases } => {
            encoder.byte(9);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
            encoder.sequence(cases, |encoder, case| encoder.string(case))?;
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            encoder.byte(10);
            encode_structural_field(encoder, subject)?;
            encoder.string(case)?;
        }
        CheckedBooleanExpression::And { left, right } => {
            encoder.byte(11);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
        CheckedBooleanExpression::Or { left, right } => {
            encoder.byte(12);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
    }
    Ok(())
}

fn encode_scalar_expression(
    encoder: &mut Encoder,
    expression: &CheckedScalarExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => {
            encoder.byte(0);
            encoder.usize(*position)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => {
            encoder.byte(1);
            encoder.usize(*position)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => {
            encoder.byte(2);
            encoder.u32(*parameter_position);
            encode_structural_path(encoder, path)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        CheckedScalarExpression::IntegerLiteral { literal } => {
            encoder.byte(3);
            encoder.string(literal.text())?;
            let landing = literal.landing();
            encoder.option(landing.as_ref(), |encoder, landing| {
                encoder.string(landing.landed_type.name())?;
                encoder.string(landing.domain.name())
            })?;
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.byte(4);
            encoder.byte(integer_binary_tag(*kind));
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, left)?;
            encode_scalar_expression(encoder, right)?;
        }
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => {
            encoder.byte(5);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
        }
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => {
            encoder.byte(6);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
        }
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => {
            encoder.byte(7);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
            encoder.string(&range.minimum.to_string())?;
            encoder.string(&range.maximum.to_string())?;
        }
        CheckedScalarExpression::Boolean(expression) => {
            encoder.byte(8);
            encode_boolean_expression(encoder, expression)?;
        }
    }
    Ok(())
}

fn encode_structural_field(
    encoder: &mut Encoder,
    field: &CheckedStructuralParameterField,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u32(field.parameter_position);
    encode_structural_path(encoder, &field.path)
}

fn encode_structural_path(
    encoder: &mut Encoder,
    path: &[CheckedStructuralPredicatePathSegment],
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(path, |encoder, segment| {
        match segment {
            CheckedStructuralPredicatePathSegment::Field(field) => {
                encoder.byte(0);
                encoder.string(field)?;
            }
            CheckedStructuralPredicatePathSegment::Case(case) => {
                encoder.byte(1);
                encoder.string(case)?;
            }
        }
        Ok(())
    })
}

fn encode_primitive_type(
    encoder: &mut Encoder,
    primitive_type: psi_typed_trees::types::PrimitiveType,
) {
    encoder.byte(match primitive_type {
        psi_typed_trees::types::PrimitiveType::Bool => 0,
        psi_typed_trees::types::PrimitiveType::F32 => 1,
        psi_typed_trees::types::PrimitiveType::F64 => 2,
        psi_typed_trees::types::PrimitiveType::I8 => 3,
        psi_typed_trees::types::PrimitiveType::I16 => 4,
        psi_typed_trees::types::PrimitiveType::I32 => 5,
        psi_typed_trees::types::PrimitiveType::I64 => 6,
        psi_typed_trees::types::PrimitiveType::U8 => 7,
        psi_typed_trees::types::PrimitiveType::U16 => 8,
        psi_typed_trees::types::PrimitiveType::U32 => 9,
        psi_typed_trees::types::PrimitiveType::U64 => 10,
        psi_typed_trees::types::PrimitiveType::Addr => 11,
    });
}

const fn integer_comparison_tag(kind: CheckedIntegerComparisonKind) -> u8 {
    match kind {
        CheckedIntegerComparisonKind::Equal => 0,
        CheckedIntegerComparisonKind::LessThan => 1,
        CheckedIntegerComparisonKind::LessOrEqual => 2,
    }
}

const fn integer_binary_tag(kind: CheckedIntegerBinaryKind) -> u8 {
    match kind {
        CheckedIntegerBinaryKind::ExactAdd => 0,
        CheckedIntegerBinaryKind::ExactSubtract => 1,
        CheckedIntegerBinaryKind::ExactMultiply => 2,
        CheckedIntegerBinaryKind::ExactDivide => 3,
        CheckedIntegerBinaryKind::ExactRemainder => 4,
        CheckedIntegerBinaryKind::WrappingDivide => 5,
        CheckedIntegerBinaryKind::WrappingRemainder => 6,
        CheckedIntegerBinaryKind::SaturatingDivide => 7,
        CheckedIntegerBinaryKind::SaturatingRemainder => 8,
        CheckedIntegerBinaryKind::WrappingAdd => 9,
        CheckedIntegerBinaryKind::SaturatingAdd => 10,
        CheckedIntegerBinaryKind::WrappingSubtract => 11,
        CheckedIntegerBinaryKind::SaturatingSubtract => 12,
        CheckedIntegerBinaryKind::WrappingMultiply => 13,
        CheckedIntegerBinaryKind::SaturatingMultiply => 14,
        CheckedIntegerBinaryKind::BitwiseAnd => 15,
        CheckedIntegerBinaryKind::BitwiseOr => 16,
        CheckedIntegerBinaryKind::BitwiseXor => 17,
        CheckedIntegerBinaryKind::WrappingShiftLeft => 18,
        CheckedIntegerBinaryKind::WrappingShiftRight => 19,
        CheckedIntegerBinaryKind::ExactShiftLeft => 20,
        CheckedIntegerBinaryKind::ExactShiftRight => 21,
    }
}

fn encode_provider(
    encoder: &mut Encoder,
    provider: &CheckedPackageProviderReview,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&provider.plan_name)?;
    encoder.u64(provider.plan_fingerprint);
    encoder.optional_package_identity(provider.realizing_package);
    encoder.string(&provider.provider_type)?;
    encoder.optional_package_identity(provider.provider_type_package);
    encode_service_schema(encoder, &provider.schema)?;
    encoder.string(&provider.target)?;
    encoder.sequence(&provider.rows, encode_provider_row)
}

fn encode_service_schema(
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
            method.calling_plan_fingerprint.as_ref(),
            |encoder, fingerprint| {
                encoder.u64(*fingerprint);
                Ok(())
            },
        )
    })
}

fn encode_carry_policy(encoder: &mut Encoder, policy: psi_language_semantics::CarryPolicy) {
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

fn encode_provider_row(
    encoder: &mut Encoder,
    row: &omega_effects::provider_plan::ProviderPlanRow,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&row.method)?;
    encoder.string(&row.requirement_identity)?;
    match &row.binding {
        ProviderBinding::Import { library, symbol } => {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_review() -> CheckedPackageReviewProjection {
        CheckedPackageReviewProjection {
            package: PackageKeyIdentity::from_digest([1; 32]).expect("nonzero package identity"),
            target: omega_target::TargetProfile::WindowsX64,
            public_traits: Vec::new(),
            public_domains: Vec::new(),
            public_data: Vec::new(),
            representation_tcb: Vec::new(),
            callables: Vec::new(),
            dangerous_authorities: Vec::new(),
            dangerous_authority_slack: Vec::new(),
            selected_providers: Vec::new(),
            row_sources: PackageReviewCanonicalRowSources {
                public_traits: Vec::new(),
                public_domains: Vec::new(),
                public_data: Vec::new(),
                representation_tcb: Vec::new(),
                callables: Vec::new(),
                dangerous_authorities: Vec::new(),
                dangerous_authority_slack: Vec::new(),
                selected_provider_set: PackageReviewCanonicalRowSource::compiler_derived(
                    PackageReviewSyntheticSourceKind::EmptySelectedProviderSet,
                ),
            },
        }
    }

    #[test]
    fn bounded_encoders_reject_instead_of_returning_partial_evidence() {
        let review = empty_review();
        assert!(encode(&review).is_ok());
        assert!(encode_rows(&review).is_ok());

        assert!(
            encode_with_limits(
                &review,
                PackageReviewEncodingLimits::new(1, 2, 64, 256, 512)
            )
            .is_err()
        );
        assert!(
            encode_rows_with_limits(
                &review,
                PackageReviewEncodingLimits::new(256, 1, 64, 256, 512),
            )
            .is_err()
        );
        assert!(
            encode_rows_with_limits(
                &review,
                PackageReviewEncodingLimits::new(256, 2, 64, 1, 512),
            )
            .is_err()
        );
        assert!(
            encode_rows_with_limits(
                &review,
                PackageReviewEncodingLimits::new(256, 2, 64, 256, 1),
            )
            .is_err()
        );
    }
}
