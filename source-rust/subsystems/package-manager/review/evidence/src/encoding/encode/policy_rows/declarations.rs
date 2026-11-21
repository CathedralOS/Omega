// Policy-row projections for public API declarations (traits, conformances, domains, propositions, consts, operators, and data).

use super::super::{
    public_api::declarations as encode,
    values::{declarations::*, identity::encode_nominal},
};
use super::Builder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::PackagePolicyPublicApi;
use crate::record::PackagePolicyRowKind;

pub(super) fn project(
    builder: &mut Builder,
    api: &PackagePolicyPublicApi,
    baseline_schema: u16,
) -> Result<(), PackageReviewEncodingError> {
    let PackagePolicyPublicApi {
        traits,
        conformances,
        domains,
        propositions,
        consts,
        operators,
        data,
        type_aliases: _,
    } = api;
    let trait_schema = baseline_schema.min(crate::encoding::BASELINE_WITH_STRUCTURAL_PARAMETERS);
    for value in traits {
        builder.push_versioned(
            (PackagePolicyRowKind::PublicTrait, trait_schema),
            false,
            false,
            |encoder| encode_nominal(encoder, &value.identity),
            |encoder| encode::trait_shape(encoder, value, trait_schema),
        )?;
    }
    for value in conformances {
        builder.push(
            PackagePolicyRowKind::PublicConformance,
            false,
            false,
            |encoder| encode_nominal(encoder, &value.identity),
            |encoder| encode::conformance_shape(encoder, value),
        )?;
    }
    for value in domains {
        builder.push(
            PackagePolicyRowKind::PublicDomain,
            false,
            false,
            |encoder| encode_nominal(encoder, &value.identity),
            |encoder| encode::domain_shape(encoder, value),
        )?;
    }
    for value in propositions {
        builder.push(
            PackagePolicyRowKind::PublicProposition,
            false,
            false,
            |encoder| encode_nominal(encoder, &value.identity),
            |encoder| encode_proposition_shape(encoder, value),
        )?;
    }
    for value in consts {
        builder.push(
            PackagePolicyRowKind::PublicConst,
            false,
            false,
            |encoder| encode_nominal(encoder, &value.identity),
            |encoder| encode_const_shape(encoder, value),
        )?;
    }
    for value in operators {
        builder.push(
            PackagePolicyRowKind::PublicOperator,
            false,
            false,
            |encoder| encode_operator_coordinate(encoder, &value.coordinate),
            |encoder| encode::operator_shape(encoder, value),
        )?;
    }
    for value in data {
        builder.push(
            PackagePolicyRowKind::PublicData,
            false,
            false,
            |encoder| encode_nominal(encoder, &value.identity),
            |encoder| encode::data_shape(encoder, value),
        )?;
    }
    for alias in api
        .type_aliases_for_version(baseline_schema)
        .map_err(PackageReviewEncodingError::new)?
        .into_iter()
        .flatten()
    {
        builder.push_versioned(
            (
                PackagePolicyRowKind::PublicTypeAlias,
                crate::encoding::BASELINE_WITH_TYPE_ALIASES,
            ),
            false,
            false,
            |encoder| encode_nominal(encoder, &alias.identity),
            |encoder| encode::type_alias(encoder, alias),
        )?;
    }
    Ok(())
}
