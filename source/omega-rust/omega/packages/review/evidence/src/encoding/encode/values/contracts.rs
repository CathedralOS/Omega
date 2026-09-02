use super::super::declarations::encode_type_identity;
use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
    PackageReviewCallableContract, PackageReviewContractEntailmentAssumptionDischarge,
    PackageReviewContractEntailmentOpenObligation, PackageReviewContractEntailmentOpenReason,
    PackageReviewContractFact, PackageReviewContractKind, PackageReviewPropositionApplication,
    PackageReviewPropositionBinderArgumentKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence,
};
use psi_core::{IntegerCarrier, IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType};

use super::declarations::{encode_evidence_interface, encode_proposition_binder};
use super::expressions::encode_contract_expression;
use super::identity::encode_nominal;

pub(crate) fn encode_contract_entailment_open_obligation(
    encoder: &mut Encoder,
    obligation: &PackageReviewContractEntailmentOpenObligation,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &obligation.callable)?;
    encoder.u32(obligation.contract_position);
    encoder.u32(obligation.fact_position);
    encode_contract_entailment_open_obligation_value(encoder, obligation)
}

pub(crate) fn encode_contract_entailment_open_obligation_value(
    encoder: &mut Encoder,
    obligation: &PackageReviewContractEntailmentOpenObligation,
) -> Result<(), PackageReviewEncodingError> {
    encoder.fixed_bytes(&obligation.machine_contract_commitment);
    encode_callable_contract(encoder, &obligation.goal)?;
    encoder.byte(match obligation.reason {
        PackageReviewContractEntailmentOpenReason::UnsupportedEnsuresFact => 0,
        PackageReviewContractEntailmentOpenReason::UnrecognizedInductiveBody => 1,
        PackageReviewContractEntailmentOpenReason::OutsideEntailmentLanguage => 2,
    });
    Ok(())
}

pub(crate) fn encode_contract_entailment_assumption_discharge(
    encoder: &mut Encoder,
    discharge: &PackageReviewContractEntailmentAssumptionDischarge,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &discharge.obligation.callable)?;
    encoder.u32(discharge.obligation.contract_position);
    encoder.u32(discharge.obligation.fact_position);
    encode_contract_entailment_assumption_discharge_value(encoder, discharge)
}

pub(crate) fn encode_contract_entailment_assumption_discharge_value(
    encoder: &mut Encoder,
    discharge: &PackageReviewContractEntailmentAssumptionDischarge,
) -> Result<(), PackageReviewEncodingError> {
    encode_contract_entailment_open_obligation_value(encoder, &discharge.obligation)?;
    encoder.sequence(&discharge.assumptions, encode_assumption_proposition)?;
    encode_assumption_proposition(encoder, &discharge.goal)?;
    encoder.u32(discharge.selected_assumption_position);
    Ok(())
}

fn encode_assumption_proposition(
    encoder: &mut Encoder,
    proposition: &Proposition,
) -> Result<(), PackageReviewEncodingError> {
    match proposition {
        Proposition::Truth => encoder.byte(0),
        Proposition::Falsehood => encoder.byte(1),
        Proposition::Equal(left, right) => {
            encoder.byte(2);
            encode_assumption_scalar(encoder, left)?;
            encode_assumption_scalar(encoder, right)?;
        }
        Proposition::LessThan(left, right) => {
            encoder.byte(3);
            encode_assumption_scalar(encoder, left)?;
            encode_assumption_scalar(encoder, right)?;
        }
        Proposition::LessOrEqual(left, right) => {
            encoder.byte(4);
            encode_assumption_scalar(encoder, left)?;
            encode_assumption_scalar(encoder, right)?;
        }
        Proposition::Conjunction(propositions) => {
            encoder.byte(5);
            encoder.sequence(propositions, encode_assumption_proposition)?;
        }
        Proposition::Disjunction(propositions) => {
            encoder.byte(6);
            encoder.sequence(propositions, encode_assumption_proposition)?;
        }
        _ => {
            return Err(PackageReviewEncodingError::new(
                "contract-entailment discharge contains a proposition outside its canonical package vocabulary",
            ));
        }
    }
    Ok(())
}

fn encode_assumption_scalar(
    encoder: &mut Encoder,
    scalar: &ScalarTerm,
) -> Result<(), PackageReviewEncodingError> {
    match scalar {
        ScalarTerm::Value { id, scalar_type } => {
            encoder.byte(0);
            encoder.u64(id.get());
            encode_assumption_scalar_type(encoder, *scalar_type)?;
        }
        ScalarTerm::Boolean(value) => {
            encoder.byte(1);
            encoder.boolean(*value);
        }
        ScalarTerm::Integer { scalar_type, value } => {
            encoder.byte(2);
            encode_assumption_integer_type(encoder, *scalar_type);
            match value {
                IntegerValue::Signed(value) => {
                    encoder.byte(0);
                    encoder.i128(*value);
                }
                IntegerValue::Unsigned(value) => {
                    encoder.byte(1);
                    encoder.u128(*value);
                }
            }
        }
        _ => {
            return Err(PackageReviewEncodingError::new(
                "contract-entailment discharge contains a scalar outside its canonical package vocabulary",
            ));
        }
    }
    Ok(())
}

fn encode_assumption_scalar_type(
    encoder: &mut Encoder,
    scalar_type: ScalarType,
) -> Result<(), PackageReviewEncodingError> {
    match scalar_type {
        ScalarType::Boolean => encoder.byte(0),
        ScalarType::Integer(integer_type) => {
            encoder.byte(1);
            encode_assumption_integer_type(encoder, integer_type);
        }
        ScalarType::IeeeFloat(_) => {
            return Err(PackageReviewEncodingError::new(
                "contract-entailment discharge contains a float scalar type",
            ));
        }
    }
    Ok(())
}

fn encode_assumption_integer_type(encoder: &mut Encoder, integer_type: psi_core::IntegerType) {
    encoder.byte(match integer_type.carrier() {
        IntegerCarrier::Fixed => 0,
        IntegerCarrier::Address => 1,
    });
    encoder.byte(match integer_type.sign() {
        IntegerSign::Signed => 0,
        IntegerSign::Unsigned => 1,
    });
    encoder.u16(integer_type.bits());
}

pub(crate) fn encode_callable_contract(
    encoder: &mut Encoder,
    contract: &PackageReviewCallableContract,
) -> Result<(), PackageReviewEncodingError> {
    match (contract.kind, contract.result_case.as_ref()) {
        (PackageReviewContractKind::Requires, None) => encoder.byte(0),
        (PackageReviewContractKind::Ensures, None) => encoder.byte(1),
        (PackageReviewContractKind::Ensures, Some(result_case)) => {
            encoder.byte(2);
            encode_nominal(encoder, &result_case.result_data)?;
            encode_nominal(encoder, &result_case.result_case)?;
        }
        (PackageReviewContractKind::Requires, Some(_)) => {
            return Err(PackageReviewEncodingError::new(
                "requires contract cannot carry a result-case guard",
            ));
        }
    }
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

pub(crate) fn encode_contract_fact(
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
        PackageReviewContractFact::PropositionParameter(application) => {
            encoder.byte(3);
            encoder.u32(application.binder_ordinal);
            encoder.sequence(&application.arguments, encode_contract_expression)
        }
    }
}

pub(crate) fn encode_proposition_application(
    encoder: &mut Encoder,
    application: &PackageReviewPropositionApplication,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &application.declaration)?;
    encoder.sequence(&application.binders, encode_proposition_binder)?;
    encoder.sequence(&application.parameter_types, encode_type_identity)?;
    encoder.sequence(&application.binder_arguments, |encoder, argument| {
        encoder.byte(match argument.kind {
            PackageReviewPropositionBinderArgumentKind::Type => 0,
            PackageReviewPropositionBinderArgumentKind::Const => 1,
            PackageReviewPropositionBinderArgumentKind::Machine => 2,
        });
        match &argument.value {
            PackageReviewPropositionBinderValue::Type(identity) => {
                encoder.byte(0);
                encode_type_identity(encoder, identity)?;
            }
            PackageReviewPropositionBinderValue::Machine(identity) => {
                encoder.byte(4);
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
            encode_evidence_interface(encoder, interface)?;
        }
    }
    Ok(())
}
