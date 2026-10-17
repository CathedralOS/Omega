use super::super::declarations::encode_type_identity;
use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{PackageReviewContractExpression, PackageReviewContractOperatorMeaning};

use super::declarations::encode_operator_coordinate;

pub(crate) fn encode_contract_expression(
    encoder: &mut Encoder,
    expression: &PackageReviewContractExpression,
) -> Result<(), PackageReviewEncodingError> {
    encoder.nested(|encoder| body::encode_contract_expression_body(encoder, expression))
}

mod body;
mod evidence;
mod static_arguments;

use evidence::encode_contract_evidence_argument;
pub(crate) use static_arguments::encode_contract_static_argument;

pub(crate) fn encode_contract_operator_meaning(
    encoder: &mut Encoder,
    meaning: &PackageReviewContractOperatorMeaning,
) -> Result<(), PackageReviewEncodingError> {
    match meaning {
        PackageReviewContractOperatorMeaning::Builtin => encoder.tag("builtin", 0),
        PackageReviewContractOperatorMeaning::Declared(coordinate) => {
            encoder.tag("declared", 1);
            encoder.field("coordinate", |encoder| {
                encode_operator_coordinate(encoder, coordinate)
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod codec_tests {
    use super::*;

    #[test]
    fn builtin_carrier_member_has_a_closed_wire_identity() {
        let expression = PackageReviewContractExpression::BuiltinTypeMember(
            symbols::BuiltinTypeMember::AddressBound,
        );
        let mut encoder = Encoder::bounded(2);
        encode_contract_expression(&mut encoder, &expression).unwrap();
        assert_eq!(encoder.finish().unwrap(), [25, 0]);
    }

    #[test]
    fn same_spelled_unresolved_nominal_still_requires_ownership() {
        let expression =
            PackageReviewContractExpression::Nominal(crate::record::PackageReviewNominalIdentity {
                owner: crate::record::PackageReviewNominalOwner::Unresolved,
                path: "addr::Bound".to_owned(),
            });
        let mut encoder = Encoder::bounded(64);
        assert!(encode_contract_expression(&mut encoder, &expression).is_err());
    }
}
