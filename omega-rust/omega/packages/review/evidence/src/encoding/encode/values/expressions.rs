use super::super::declarations::encode_type_identity;
use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering,
    PackageReviewByteSequencePredicate, PackageReviewCastForm,
    PackageReviewCollectionViewOperation, PackageReviewContractBinaryOperator,
    PackageReviewContractCallTarget, PackageReviewContractEvidenceArgument,
    PackageReviewContractExpression, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewFloatLiteral, PackageReviewReferenceAccess,
};

use super::declarations::encode_operator_coordinate;
use super::identity::encode_nominal;

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
