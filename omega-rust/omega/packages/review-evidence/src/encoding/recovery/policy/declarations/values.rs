use super::super::{
    Error,
    contracts::{contract_fact, evidence_interface, proposition_binder},
    identity::{nominal, type_identity},
    reader::Reader,
};
use crate::record::{
    PackageReviewConstShape, PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
};

pub(in crate::encoding::recovery) fn proposition_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewPropositionShape, Error> {
    Ok(PackageReviewPropositionShape {
        identity: nominal(reader)?,
        binders: reader.sequence(3, proposition_binder)?,
        parameter_types: reader.sequence(8, type_identity)?,
        body: match reader.byte()? {
            0 => PackageReviewPublicPropositionBody::Primitive,
            1 => PackageReviewPublicPropositionBody::Witness(evidence_interface(reader)?),
            2 => PackageReviewPublicPropositionBody::Transparent(contract_fact(reader)?),
            _ => return Err(Error::InvalidTag),
        },
    })
}

pub(in crate::encoding::recovery) fn const_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewConstShape, Error> {
    Ok(PackageReviewConstShape {
        identity: nominal(reader)?,
        declared_type: type_identity(reader)?,
        canonical_value_encoding: reader.string()?,
    })
}

pub(in crate::encoding::recovery) fn operator_spelling(
    reader: &mut Reader<'_>,
) -> Result<language_core::OperatorSpelling, Error> {
    use language_core::OperatorSpelling;
    Ok(match reader.byte()? {
        0 => OperatorSpelling::Add,
        1 => OperatorSpelling::Subtract,
        2 => OperatorSpelling::Multiply,
        3 => OperatorSpelling::Divide,
        4 => OperatorSpelling::Modulo,
        5 => OperatorSpelling::Equal,
        6 => OperatorSpelling::NotEqual,
        7 => OperatorSpelling::Less,
        8 => OperatorSpelling::LessEqual,
        9 => OperatorSpelling::Greater,
        10 => OperatorSpelling::GreaterEqual,
        11 => OperatorSpelling::Index,
        12 => OperatorSpelling::Range,
        _ => return Err(Error::InvalidTag),
    })
}
