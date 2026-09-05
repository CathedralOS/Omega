//! Recovery of contract meaning and proposition interfaces, not discharges.

use super::Error;
use super::expressions::expression;
use super::identity::{nominal, type_identity};
use super::reader::Reader;
use super::signatures::data_properties;
use crate::record::*;

#[cfg(test)]
mod tests;

pub(super) fn callable_contract(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewCallableContract, Error> {
    let (kind, result_case) = match reader.byte()? {
        0 => (PackageReviewContractKind::Requires, None),
        1 => (PackageReviewContractKind::Ensures, None),
        2 => (
            PackageReviewContractKind::Ensures,
            Some(PackageReviewResultCaseIdentity {
                result_data: nominal(reader)?,
                result_case: nominal(reader)?,
            }),
        ),
        _ => return Err(Error::InvalidTag),
    };
    Ok(PackageReviewCallableContract {
        kind,
        result_case,
        binding: reader.option(|reader| reader.string())?,
        evidence_lane_position: reader.option(|reader| reader.u32())?,
        fact: contract_fact(reader)?,
    })
}

fn contract_fact(reader: &mut Reader<'_>) -> Result<PackageReviewContractFact, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewContractFact::Expression(expression(reader)?),
        1 => PackageReviewContractFact::Membership {
            value: expression(reader)?,
            domain: nominal(reader)?,
        },
        2 => PackageReviewContractFact::Proposition(proposition_application(reader)?),
        3 => PackageReviewContractFact::PropositionParameter(
            PackageReviewPropositionParameterApplication {
                binder_ordinal: reader.u32()?,
                arguments: reader.sequence(1, expression)?,
            },
        ),
        _ => return Err(Error::InvalidTag),
    })
}

fn proposition_application(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewPropositionApplication, Error> {
    Ok(PackageReviewPropositionApplication {
        declaration: nominal(reader)?,
        binders: reader.sequence(3, proposition_binder)?,
        parameter_types: reader.sequence(8, type_identity)?,
        binder_arguments: reader.sequence(2, binder_argument)?,
        arguments: reader.sequence(1, expression)?,
        evidence: match reader.byte()? {
            0 => PackageReviewPropositionEvidence::FactOnly,
            1 => PackageReviewPropositionEvidence::Witness(evidence_interface(reader)?),
            _ => return Err(Error::InvalidTag),
        },
    })
}

fn binder_argument(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewPropositionBinderArgument, Error> {
    let kind = match reader.byte()? {
        0 => PackageReviewPropositionBinderArgumentKind::Type,
        1 => PackageReviewPropositionBinderArgumentKind::Const,
        2 => PackageReviewPropositionBinderArgumentKind::Machine,
        _ => return Err(Error::InvalidTag),
    };
    let value = match reader.byte()? {
        0 => PackageReviewPropositionBinderValue::Type(type_identity(reader)?),
        1 => PackageReviewPropositionBinderValue::GenericBinder(reader.u32()?),
        2 => PackageReviewPropositionBinderValue::Integer(reader.string()?),
        3 => PackageReviewPropositionBinderValue::EvidenceProjection {
            source_kind: match reader.byte()? {
                0 => PackageReviewContractKind::Requires,
                1 => PackageReviewContractKind::Ensures,
                _ => return Err(Error::InvalidTag),
            },
            source_lane_position: reader.u32()?,
            declaring_trait: nominal(reader)?,
            declaring_trait_arguments: reader.sequence(8, type_identity)?,
            requirement: nominal(reader)?,
        },
        4 => PackageReviewPropositionBinderValue::Machine(nominal(reader)?),
        _ => return Err(Error::InvalidTag),
    };
    Ok(PackageReviewPropositionBinderArgument { kind, value })
}

fn proposition_binder(reader: &mut Reader<'_>) -> Result<PackageReviewPropositionBinder, Error> {
    let kind = match reader.byte()? {
        0 => PackageReviewPropositionBinderKind::Type,
        1 => PackageReviewPropositionBinderKind::Const(type_identity(reader)?),
        2 => PackageReviewPropositionBinderKind::Machine,
        _ => return Err(Error::InvalidTag),
    };
    Ok(PackageReviewPropositionBinder {
        kind,
        bounds: data_properties(reader)?,
    })
}

fn evidence_interface(reader: &mut Reader<'_>) -> Result<PackageReviewEvidenceInterface, Error> {
    Ok(PackageReviewEvidenceInterface {
        trait_identity: nominal(reader)?,
        lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        arguments: reader.sequence(8, type_identity)?,
        requirements: reader.sequence(1, |reader| {
            Ok(PackageReviewEvidenceRequirement {
                declaring_trait: nominal(reader)?,
                declaring_trait_lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
                declaring_trait_arguments: reader.sequence(8, type_identity)?,
                requirement: nominal(reader)?,
            })
        })?,
    })
}
