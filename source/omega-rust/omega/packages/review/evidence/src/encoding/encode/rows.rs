use super::declarations::{
    encode_conformance_shape, encode_dangerous_authority, encode_dangerous_authority_slack,
    encode_data_shape, encode_domain_shape, encode_representation_tcb,
    encode_representation_tcb_key, encode_semantic_dependency, encode_semantic_dependency_key,
    encode_terminal_authority_permission, encode_terminal_authority_permission_key,
    encode_trait_shape,
};
use super::values::callables::{
    encode_callable, encode_external_executable_supply, encode_external_executable_supply_key,
};
use super::values::contracts::encode_contract_entailment_open_obligation_value;
use super::values::declarations::{
    encode_const_shape, encode_operator_coordinate, encode_operator_shape, encode_proposition_shape,
};
use super::values::identity::encode_nominal;
use super::values::providers::{
    encode_boundary_application_realization, encode_boundary_application_realization_key,
    encode_provider, encode_provider_family,
};
use super::{PackageReviewEncodingError, PackageReviewEncodingLimits};
use crate::record::{
    CheckedPackageReviewProjection, PackageReviewCallableSupply, PackageReviewCanonicalRow,
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
    PackageReviewSyntheticSourceKind,
};

mod framing;

pub(crate) use framing::encode_subject_row;
use framing::{encode_row, push_row, row_source};

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
        .saturating_add(review.contract_entailment_open_obligations.len())
        .saturating_add(review.external_executable_supply.len())
        .saturating_add(
            review
                .callables
                .iter()
                .filter(|callable| callable.supply == PackageReviewCallableSupply::AdmissionClaim)
                .count(),
        )
        .saturating_add(review.dangerous_authorities.len())
        .saturating_add(review.dangerous_authority_slack.len())
        .saturating_add(review.terminal_authority_permissions.len())
        .saturating_add(review.boundary_application_realizations.len())
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
                |encoder| encode_representation_tcb_key(encoder, row),
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
        if callable.supply == PackageReviewCallableSupply::AdmissionClaim {
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
    for (index, obligation) in review
        .contract_entailment_open_obligations
        .iter()
        .enumerate()
    {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(
                    &review.row_sources.contract_entailment_open_obligations,
                    index,
                )?,
                |encoder| {
                    encode_nominal(encoder, &obligation.callable)?;
                    encoder.u32(obligation.contract_position);
                    encoder.u32(obligation.fact_position);
                    Ok(())
                },
                |encoder| encode_contract_entailment_open_obligation_value(encoder, obligation),
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
    for (index, permission) in review.terminal_authority_permissions.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::TerminalAuthorityPermission,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.terminal_authority_permissions, index)?,
                |encoder| encode_terminal_authority_permission_key(encoder, permission),
                |encoder| encode_terminal_authority_permission(encoder, permission),
            )?,
        )?;
    }
    for (index, realization) in review.boundary_application_realizations.iter().enumerate() {
        push_row(
            &mut rows,
            &mut total_row_bytes,
            limits,
            encode_row(
                review,
                limits,
                PackageReviewCanonicalRowKind::BoundaryApplicationRealization,
                PackageReviewCanonicalRowRisk::Blocking,
                row_source(&review.row_sources.boundary_application_realizations, index)?,
                |encoder| encode_boundary_application_realization_key(encoder, realization),
                |encoder| encode_boundary_application_realization(encoder, realization),
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
            |encoder| {
                encoder.sequence(&review.selected_providers, encode_provider)?;
                encoder.sequence(&review.selected_provider_families, encode_provider_family)
            },
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
