use super::*;
use psi_core::PackageKeyIdentity;

pub(crate) const MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW\0";
pub const PACKAGE_REVIEW_ENCODING_VERSION: u16 = 78;
pub(crate) const ROW_MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-ROW\0";
pub const PACKAGE_REVIEW_ROW_ENCODING_VERSION: u16 = 36;

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
    pub(crate) const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for PackageReviewEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for PackageReviewEncodingError {}

pub(crate) fn encode(
    review: &CheckedPackageReviewProjection,
) -> Result<Vec<u8>, PackageReviewEncodingError> {
    encode_with_limits(review, PackageReviewEncodingLimits::default())
}

pub(crate) fn encode_with_limits(
    review: &CheckedPackageReviewProjection,
    limits: PackageReviewEncodingLimits,
) -> Result<Vec<u8>, PackageReviewEncodingError> {
    let mut encoder = Encoder::bounded(limits.maximum_review_bytes);
    encoder.fixed_bytes(MAGIC);
    encoder.u16(PACKAGE_REVIEW_ENCODING_VERSION);
    encoder.package_identity(review.package);
    encoder.string(review.target.target_name())?;
    encoder.sequence(&review.public_traits, encode_trait_shape)?;
    encoder.sequence(&review.public_conformances, encode_conformance_shape)?;
    encoder.sequence(&review.public_domains, encode_domain_shape)?;
    encoder.sequence(&review.public_propositions, encode_proposition_shape)?;
    encoder.sequence(&review.public_consts, encode_const_shape)?;
    encoder.sequence(&review.public_operators, encode_operator_shape)?;
    encoder.sequence(&review.public_data, encode_data_shape)?;
    encoder.sequence(&review.representation_tcb, encode_representation_tcb)?;
    encoder.sequence(&review.semantic_dependencies, encode_semantic_dependency)?;
    encoder.sequence(&review.callables, encode_callable)?;
    encoder.sequence(
        &review.external_executable_supply,
        encode_external_executable_supply,
    )?;
    encoder.sequence(&review.dangerous_authorities, encode_dangerous_authority)?;
    encoder.sequence(
        &review.dangerous_authority_slack,
        encode_dangerous_authority_slack,
    )?;
    encoder.sequence(&review.selected_providers, encode_provider)?;
    encoder.finish()
}

pub(crate) fn encode_rows(
    review: &CheckedPackageReviewProjection,
) -> Result<Vec<PackageReviewCanonicalRow>, PackageReviewEncodingError> {
    encode_rows_with_limits(review, PackageReviewEncodingLimits::default())
}

pub(crate) fn encode_rows_with_limits(
    review: &CheckedPackageReviewProjection,
    limits: PackageReviewEncodingLimits,
) -> Result<Vec<PackageReviewCanonicalRow>, PackageReviewEncodingError> {
    let required_rows = review
        .public_traits
        .len()
        .saturating_add(review.public_conformances.len())
        .saturating_add(review.public_domains.len())
        .saturating_add(review.public_propositions.len())
        .saturating_add(review.public_consts.len())
        .saturating_add(review.public_operators.len())
        .saturating_add(review.public_data.len())
        .saturating_add(review.representation_tcb.len())
        .saturating_add(review.semantic_dependencies.len())
        .saturating_add(review.callables.len())
        .saturating_add(review.external_executable_supply.len())
        .saturating_add(
            review
                .callables
                .iter()
                .filter(|callable| callable.supply == PackageReviewCallableSupply::Accepted)
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
    for (index, shape) in review.public_conformances.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::PublicConformance,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.public_conformances, index)?,
                |encoder| encode_nominal(encoder, &shape.identity),
                |encoder| encode_conformance_shape(encoder, shape),
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
    for (index, shape) in review.public_propositions.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::PublicProposition,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.public_propositions, index)?,
                |encoder| encode_nominal(encoder, &shape.identity),
                |encoder| encode_proposition_shape(encoder, shape),
            )?,
        )?;
    }
    for (index, shape) in review.public_consts.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::PublicConst,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.public_consts, index)?,
                |encoder| encode_nominal(encoder, &shape.identity),
                |encoder| encode_const_shape(encoder, shape),
            )?,
        )?;
    }
    for (index, shape) in review.public_operators.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::PublicOperator,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.public_operators, index)?,
                |encoder| encode_operator_coordinate(encoder, &shape.coordinate),
                |encoder| encode_operator_shape(encoder, shape),
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
    for (index, dependency) in review.semantic_dependencies.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::SemanticDependency,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.semantic_dependencies, index)?,
                |encoder| encode_semantic_dependency_key(encoder, dependency),
                |encoder| encode_semantic_dependency(encoder, dependency),
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
        if callable.supply == PackageReviewCallableSupply::Accepted {
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
    for (index, supply) in review.external_executable_supply.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::ExternalExecutableSupply,
                PackageReviewCanonicalRowRisk::OpaqueBlocking,
                row_source(&review.row_sources.external_executable_supply, index)?,
                |encoder| encode_external_executable_supply_key(encoder, supply),
                |encoder| encode_external_executable_supply(encoder, supply),
            )?,
        )?;
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

pub(crate) fn push_row(
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

pub(crate) fn encode_row(
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

pub(crate) fn row_source(
    sources: &[PackageReviewCanonicalRowSource],
    index: usize,
) -> Result<PackageReviewCanonicalRowSource, PackageReviewEncodingError> {
    sources.get(index).cloned().ok_or_else(|| {
        PackageReviewEncodingError::new(
            "package review canonical row has no compiler-issued source disposition",
        )
    })
}

pub(crate) const fn canonical_row_risk_tag(risk: PackageReviewCanonicalRowRisk) -> u8 {
    match risk {
        PackageReviewCanonicalRowRisk::Blocking => 0,
        PackageReviewCanonicalRowRisk::AuditRecommended => 1,
        PackageReviewCanonicalRowRisk::OpaqueBlocking => 2,
    }
}

pub(crate) const fn canonical_row_kind_tag(kind: PackageReviewCanonicalRowKind) -> u8 {
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
        PackageReviewCanonicalRowKind::SemanticDependency => 10,
        PackageReviewCanonicalRowKind::PublicProposition => 11,
        PackageReviewCanonicalRowKind::PublicConst => 12,
        PackageReviewCanonicalRowKind::PublicOperator => 13,
        PackageReviewCanonicalRowKind::PublicConformance => 14,
        PackageReviewCanonicalRowKind::ExternalExecutableSupply => 15,
    }
}

pub(crate) fn encode_conformance_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewConformanceShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    match &shape.subject {
        PackageReviewConformanceSubject::Subjectless => encoder.byte(0),
        PackageReviewConformanceSubject::TypeParameter(ordinal) => {
            encoder.byte(1);
            encoder.u32(*ordinal);
        }
        PackageReviewConformanceSubject::Nominal(identity) => {
            encoder.byte(2);
            encode_nominal(encoder, identity)?;
        }
    }
    encode_evidence_interface(encoder, &shape.interface)
}

pub(crate) fn encode_semantic_dependency_key(
    encoder: &mut Encoder,
    dependency: &PackageReviewSemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &dependency.consumer)?;
    encode_nominal(encoder, &dependency.dependency)?;
    encoder.byte(semantic_dependency_kind_tag(dependency.kind));
    Ok(())
}

pub(crate) fn encode_semantic_dependency(
    encoder: &mut Encoder,
    dependency: &PackageReviewSemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encode_semantic_dependency_key(encoder, dependency)?;
    encoder.byte(match dependency.exposure {
        PackageReviewSemanticDependencyExposure::PrivateImplementation => 0,
        PackageReviewSemanticDependencyExposure::PublicInterface => 1,
    });
    Ok(())
}

pub(crate) const fn semantic_dependency_kind_tag(kind: PackageReviewSemanticDependencyKind) -> u8 {
    match kind {
        PackageReviewSemanticDependencyKind::NominalIdentity => 0,
        PackageReviewSemanticDependencyKind::Layout => 1,
        PackageReviewSemanticDependencyKind::OwnershipBehavior => 2,
        PackageReviewSemanticDependencyKind::AutomaticCleanup => 3,
        PackageReviewSemanticDependencyKind::AutomaticCleanupMachine => 4,
    }
}

pub(crate) fn encode_representation_tcb(
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

pub(crate) fn encode_dangerous_authority(
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

pub(crate) fn encode_dangerous_authority_slack(
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

pub(crate) fn encode_trait_shape(
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

pub(crate) fn encode_conformance_bound(
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
    match (&bound.selected_conformance, &bound.selected_subject) {
        (None, None)
            if bound.selected_lifetime_arguments.is_empty()
                && bound.selected_arguments.is_empty() =>
        {
            encoder.byte(0)
        }
        (Some(conformance), Some(subject)) => {
            encoder.byte(1);
            encode_nominal(encoder, conformance)?;
            encoder.sequence(&bound.selected_lifetime_arguments, |encoder, argument| {
                encoder.u32(*argument);
                Ok(())
            })?;
            encoder.sequence(&bound.selected_arguments, encode_contract_static_argument)?;
            encode_contract_static_argument(encoder, subject)?;
        }
        _ => {
            return Err(PackageReviewEncodingError::new(
                "selected conformance review row has an incomplete application identity",
            ));
        }
    }
    encode_nominal(encoder, &bound.trait_identity)?;
    encoder.sequence(&bound.arguments, encode_type_identity)
}

pub(crate) fn encode_trait_parent(
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

pub(crate) fn encode_trait_requirement(
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

pub(crate) fn encode_domain_shape(
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
            encoder.sequence(atoms, encode_domain_alias_atom)?;
        }
    }
    match shape.classification {
        None => encoder.byte(0),
        Some(PackageReviewDomainClassification::ProgressProfile) => encoder.byte(1),
    }
    encoder.sequence(&shape.semantic_roles, |encoder, role| {
        encoder.byte(match role {
            PackageReviewDomainSemanticRole::DenotationDimension => 0,
            PackageReviewDomainSemanticRole::ArithmeticPolicy => 1,
        });
        Ok(())
    })?;
    encoder.sequence(
        &shape.establishment_routes,
        encode_domain_establishment_route,
    )
}

pub(crate) fn encode_domain_alias_atom(
    encoder: &mut Encoder,
    atom: &PackageReviewDomainAliasAtom,
) -> Result<(), PackageReviewEncodingError> {
    match atom {
        PackageReviewDomainAliasAtom::Declared(identity) => {
            encoder.byte(0);
            encode_nominal(encoder, identity)
        }
        PackageReviewDomainAliasAtom::Carry(permission) => {
            encoder.byte(1);
            encoder.byte(match permission {
                psi_language_semantics::CarryPermission::AcrossSuspend => 0,
                psi_language_semantics::CarryPermission::AnyCpu => 1,
                psi_language_semantics::CarryPermission::AnyThread => 2,
                psi_language_semantics::CarryPermission::MovableAddress => 3,
            });
            Ok(())
        }
    }
}

pub(crate) fn encode_domain_establishment_route(
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

pub(crate) fn encode_data_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewDataShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    match &shape.kind {
        PackageReviewDataKind::Ordinary => encoder.byte(0),
        PackageReviewDataKind::Quotient { carrier, relation } => {
            encoder.byte(1);
            encode_type_identity(encoder, carrier)?;
            encode_nominal(encoder, relation)?;
        }
    }
    encoder.byte(match shape.supply {
        psi_language_semantics::DataSupplyMode::CheckedShape => 0,
        psi_language_semantics::DataSupplyMode::BoundaryOpaque => 1,
    });
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encode_data_properties(encoder, shape.properties);
    encoder.boolean(shape.zero_gated);
    encoder.sequence(&shape.invariants, encode_contract_fact)?;
    encoder.sequence(&shape.retired_identities, |encoder, identity| {
        encoder.u64(*identity);
        Ok(())
    })?;
    encoder.sequence(&shape.members, encode_data_member)
}

pub(crate) fn encode_type_parameter(
    encoder: &mut Encoder,
    parameter: &PackageReviewTypeParameter,
) -> Result<(), PackageReviewEncodingError> {
    match &parameter.kind {
        PackageReviewTypeParameterKind::Type => encoder.byte(0),
        PackageReviewTypeParameterKind::Const(type_identity) => {
            encoder.byte(1);
            encode_type_identity(encoder, type_identity)?;
        }
        PackageReviewTypeParameterKind::Machine(contract) => {
            encoder.byte(2);
            encode_machine_parameter_contract(encoder, contract)?;
        }
        PackageReviewTypeParameterKind::Proposition(signature) => {
            encoder.byte(3);
            encoder.sequence(&signature.parameters, |encoder, parameter| {
                encode_type_identity(encoder, &parameter.type_identity)
            })?;
        }
    }
    encode_data_properties(encoder, parameter.bounds);
    Ok(())
}

pub(crate) fn encode_machine_parameter_contract(
    encoder: &mut Encoder,
    contract: &PackageReviewMachineParameterContract,
) -> Result<(), PackageReviewEncodingError> {
    match contract {
        PackageReviewMachineParameterContract::Structural(signature) => {
            encoder.byte(0);
            encode_machine_parameter_signature(encoder, signature)
        }
        PackageReviewMachineParameterContract::Nominal {
            trait_identity,
            requirement_identity,
        } => {
            encoder.byte(1);
            encode_nominal(encoder, trait_identity)?;
            encode_nominal(encoder, requirement_identity)
        }
        PackageReviewMachineParameterContract::RequirementIdentity => {
            encoder.byte(2);
            Ok(())
        }
    }
}

pub(crate) fn encode_machine_parameter_signature(
    encoder: &mut Encoder,
    signature: &PackageReviewMachineParameterSignature,
) -> Result<(), PackageReviewEncodingError> {
    encoder.usize(signature.lifetime_parameter_count)?;
    encoder.sequence(&signature.type_parameters, encode_type_parameter)?;
    encoder.sequence(&signature.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &signature.return_type)?;
    encoder.sequence(&signature.contracts, encode_callable_contract)?;
    encoder.sequence(&signature.published_crash, encode_crash_route)?;
    encoder.sequence(&signature.service_reach, encode_nominal)?;
    encoder.boolean(signature.service_reach_is_installation_bound);
    encoder.sequence(
        &signature.synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.boolean(signature.suspends);
    encoder.boolean(signature.blocks);
    encode_termination(encoder, &signature.termination)
}

pub(crate) fn encode_data_properties(
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

pub(crate) fn encode_data_member(
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

pub(crate) fn encode_data_field(
    encoder: &mut Encoder,
    field: &PackageReviewDataField,
) -> Result<(), PackageReviewEncodingError> {
    encode_optional_u64(encoder, field.identity);
    encoder.string(&field.name)?;
    encode_relevance(encoder, field.relevance);
    encode_type_identity(encoder, &field.type_identity)
}

pub(crate) fn encode_type_identity(
    encoder: &mut Encoder,
    identity: &PackageReviewTypeIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&identity.canonical)
}

pub(crate) fn encode_relevance(
    encoder: &mut Encoder,
    relevance: psi_language_core::BindingRelevance,
) {
    encoder.byte(match relevance {
        psi_language_core::BindingRelevance::Relevant => 0,
        psi_language_core::BindingRelevance::Erased => 1,
    });
}

pub(crate) fn encode_optional_u64(encoder: &mut Encoder, value: Option<u64>) {
    match value {
        None => encoder.byte(0),
        Some(value) => {
            encoder.byte(1);
            encoder.u64(value);
        }
    }
}

pub(crate) struct Encoder {
    output: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
}

impl Encoder {
    pub(crate) fn bounded(maximum_bytes: usize) -> Self {
        Self {
            output: Vec::new(),
            maximum_bytes,
            exceeded: false,
        }
    }

    pub(crate) fn finish(self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        if self.exceeded {
            Err(PackageReviewEncodingError::new(
                "package review exceeds its canonical encoding byte ceiling",
            ))
        } else {
            Ok(self.output)
        }
    }

    pub(crate) fn append(&mut self, bytes: &[u8]) {
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

    pub(crate) fn fixed_bytes(&mut self, value: &[u8]) {
        self.append(value);
    }

    pub(crate) fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }

    pub(crate) fn boolean(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn i64(&mut self, value: i64) {
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn usize(&mut self, value: usize) -> Result<(), PackageReviewEncodingError> {
        self.u64(u64::try_from(value).map_err(|_| {
            PackageReviewEncodingError::new(
                "package review value exceeds the portable encoding range",
            )
        })?);
        Ok(())
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) -> Result<(), PackageReviewEncodingError> {
        self.usize(value.len())?;
        self.append(value);
        self.check()
    }

    pub(crate) fn string(&mut self, value: &str) -> Result<(), PackageReviewEncodingError> {
        self.bytes(value.as_bytes())
    }

    pub(crate) fn sequence<T>(
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

    pub(crate) fn option<T: ?Sized>(
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

    pub(crate) fn package_identity(&mut self, identity: PackageKeyIdentity) {
        self.append(&identity.digest());
    }

    pub(crate) fn optional_package_identity(&mut self, identity: Option<PackageKeyIdentity>) {
        match identity {
            None => self.byte(0),
            Some(identity) => {
                self.byte(1);
                self.package_identity(identity);
            }
        }
    }

    pub(crate) fn check(&self) -> Result<(), PackageReviewEncodingError> {
        if self.exceeded {
            Err(PackageReviewEncodingError::new(
                "package review exceeds its canonical encoding byte ceiling",
            ))
        } else {
            Ok(())
        }
    }
}
