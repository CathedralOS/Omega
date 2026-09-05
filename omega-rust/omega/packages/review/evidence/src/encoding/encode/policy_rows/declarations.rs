use super::super::{
    public_api::declarations as encode,
    values::{declarations::*, identity::encode_nominal},
};
use super::*;

pub(super) fn project(
    builder: &mut Builder,
    api: &PackagePolicyPublicApi,
) -> Result<(), PackageReviewEncodingError> {
    let PackagePolicyPublicApi {
        traits,
        conformances,
        domains,
        propositions,
        consts,
        operators,
        data,
    } = api;
    for value in traits {
        builder.push(
            PackagePolicyRowKind::PublicTrait,
            false,
            false,
            |encoder| encode_nominal(encoder, &value.identity),
            |encoder| encode::trait_shape(encoder, value),
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
    Ok(())
}
