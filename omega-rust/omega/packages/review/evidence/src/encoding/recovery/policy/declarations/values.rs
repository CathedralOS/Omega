use super::super::{
    Error,
    contracts::{contract_fact, evidence_interface, proposition_binder},
    identity::{nominal, type_identity},
    reader::Reader,
};
#[cfg(test)]
use super::super::{
    behavior::crash_route, contracts::callable_contract, identity::operator_coordinate,
    signatures::type_parameter,
};
use crate::record::*;

pub(in crate::encoding::recovery::policy) fn proposition_shape(
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

pub(in crate::encoding::recovery::policy) fn const_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewConstShape, Error> {
    Ok(PackageReviewConstShape {
        identity: nominal(reader)?,
        declared_type: type_identity(reader)?,
        canonical_value_encoding: reader.string()?,
    })
}

#[cfg(test)]
pub(in crate::encoding::recovery::policy) fn operator_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewOperatorShape, Error> {
    Ok(PackageReviewOperatorShape {
        coordinate: operator_coordinate(reader)?,
        is_boundary: reader.boolean()?,
        spelling: reader.option(operator_spelling)?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        parameters: reader.sequence(19, |reader| {
            Ok(PackageReviewCallableParameter {
                name: reader.string()?,
                type_identity: type_identity(reader)?,
                is_const: reader.boolean()?,
                is_mutable: reader.boolean()?,
                is_self: reader.boolean()?,
            })
        })?,
        return_type: type_identity(reader)?,
        contracts: reader.sequence(4, callable_contract)?,
        published_crash: reader.sequence(9, crash_route)?,
    })
}

pub(in crate::encoding::recovery::policy) fn operator_spelling(
    reader: &mut Reader<'_>,
) -> Result<psi_language_core::OperatorSpelling, Error> {
    use psi_language_core::OperatorSpelling;
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
