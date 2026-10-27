use super::super::declarations::{encode_carry_permission, encode_type_identity};
use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
    PackageReviewCallableContract, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewPropositionApplication, PackageReviewPropositionBinderArgumentKind,
    PackageReviewPropositionBinderValue, PackageReviewPropositionEvidence,
};

use super::declarations::{encode_evidence_interface, encode_proposition_binder};
use super::expressions::encode_contract_expression;
use super::identity::encode_nominal;

pub(crate) fn encode_callable_contract(
    encoder: &mut Encoder,
    contract: &PackageReviewCallableContract,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("kind", |encoder| {
        match (contract.kind, contract.result_case.as_ref()) {
            (PackageReviewContractKind::Requires, None) => encoder.tag("requires", 0),
            (PackageReviewContractKind::Ensures, None) => encoder.tag("ensures", 1),
            (PackageReviewContractKind::Ensures, Some(result_case)) => {
                encoder.tag("ensures_case", 2);
                encoder.field_by("result_data", &result_case.result_data, encode_nominal)?;
                encoder.field_by("result_case", &result_case.result_case, encode_nominal)?;
            }
            (PackageReviewContractKind::Requires, Some(_)) => {
                return Err(PackageReviewEncodingError::new(
                    "requires contract cannot carry a result-case guard",
                ));
            }
        };
        Ok(())
    })?;
    encoder.field("binding", |encoder| {
        encoder.option(contract.binding.as_deref(), |encoder, binding| {
            encoder.field("binding", |encoder| encoder.string(binding))
        })
    })?;
    encoder.field("evidence_lane_position", |encoder| {
        encoder.option(
            contract.evidence_lane_position.as_ref(),
            |encoder, position| {
                encoder.u32_field("position", *position)?;
                Ok(())
            },
        )
    })?;
    encoder.field_by("fact", &contract.fact, encode_contract_fact)
}

pub(crate) fn encode_contract_fact(
    encoder: &mut Encoder,
    fact: &PackageReviewContractFact,
) -> Result<(), PackageReviewEncodingError> {
    match fact {
        PackageReviewContractFact::Expression(expression) => {
            encoder.tag("expression", 0);
            encoder.field_by("expression", expression, encode_contract_expression)
        }
        PackageReviewContractFact::CarryMembership { value, permission } => {
            encoder.tag("carry_membership", 4);
            encoder.field_by("value", value, encode_contract_expression)?;
            encoder.field("permission", |encoder| {
                encode_carry_permission(encoder, *permission);
                Ok(())
            })
        }
        PackageReviewContractFact::Membership { value, domain } => {
            encoder.tag("membership", 1);
            encoder.field_by("value", value, encode_contract_expression)?;
            encoder.field("domain", |encoder| encode_nominal(encoder, domain))
        }
        PackageReviewContractFact::Proposition(application) => {
            encoder.tag("proposition", 2);
            encoder.field_by("application", application, encode_proposition_application)
        }
        PackageReviewContractFact::PropositionParameter(application) => {
            encoder.tag("proposition_parameter", 3);
            encoder.u32_field("binder_ordinal", application.binder_ordinal)?;
            encoder.field("arguments", |encoder| {
                encoder.sequence(&application.arguments, encode_contract_expression)
            })
        }
    }
}

pub(crate) fn encode_proposition_application(
    encoder: &mut Encoder,
    application: &PackageReviewPropositionApplication,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("declaration", &application.declaration, encode_nominal)?;
    encoder.field("binders", |encoder| {
        encoder.sequence(&application.binders, encode_proposition_binder)
    })?;
    encoder.field("parameter_types", |encoder| {
        encoder.sequence(&application.parameter_types, encode_type_identity)
    })?;
    encoder.field("binder_arguments", |encoder| {
        encoder.sequence(&application.binder_arguments, |encoder, argument| {
            encoder.field("kind", |encoder| {
                match argument.kind {
                    PackageReviewPropositionBinderArgumentKind::Type => encoder.tag("type", 0),
                    PackageReviewPropositionBinderArgumentKind::Const => encoder.tag("const", 1),
                    PackageReviewPropositionBinderArgumentKind::Machine => {
                        encoder.tag("machine", 2)
                    }
                };
                Ok(())
            })?;
            encoder.field("value", |encoder| {
                match &argument.value {
                    PackageReviewPropositionBinderValue::Type(identity) => {
                        encoder.tag("type", 0);
                        encoder.field_by("identity", identity, encode_type_identity)?;
                    }
                    PackageReviewPropositionBinderValue::Machine(identity) => {
                        encoder.tag("machine", 4);
                        encoder.field("identity", |encoder| encode_nominal(encoder, identity))?;
                    }
                    PackageReviewPropositionBinderValue::GenericBinder(position) => {
                        encoder.tag("generic_binder", 1);
                        encoder.u32_field("position", *position)?;
                    }
                    PackageReviewPropositionBinderValue::Integer(value) => {
                        encoder.tag("integer", 2);
                        encoder.field("value", |encoder| encoder.string(value))?;
                    }
                    PackageReviewPropositionBinderValue::EvidenceProjection {
                        source_kind,
                        source_lane_position,
                        declaring_trait,
                        declaring_trait_arguments,
                        requirement,
                    } => {
                        encoder.tag("evidence_projection", 3);
                        encoder.field("source_kind", |encoder| {
                            match source_kind {
                                PackageReviewContractKind::Requires => encoder.tag("requires", 0),
                                PackageReviewContractKind::Ensures => encoder.tag("ensures", 1),
                            };
                            Ok(())
                        })?;
                        encoder.u32_field("source_lane_position", *source_lane_position)?;
                        encoder.field_by("declaring_trait", declaring_trait, encode_nominal)?;
                        encoder.field("declaring_trait_arguments", |encoder| {
                            encoder.sequence(declaring_trait_arguments, encode_type_identity)
                        })?;
                        encoder.field_by("requirement", requirement, encode_nominal)?;
                    }
                };
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("arguments", |encoder| {
        encoder.sequence(&application.arguments, encode_contract_expression)
    })?;
    encoder.field("evidence", |encoder| {
        match &application.evidence {
            PackageReviewPropositionEvidence::FactOnly => encoder.tag("fact_only", 0),
            PackageReviewPropositionEvidence::Witness(interface) => {
                encoder.tag("witness", 1);
                encoder.field_by("interface", interface, encode_evidence_interface)?;
            }
        };
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod codec_tests {
    use super::*;
    use crate::record::PackageReviewContractExpression;

    #[test]
    fn closed_carry_fact_has_stable_distinct_wire_tags() {
        for (tag, permission) in language_semantics::CarryPermission::ALL
            .into_iter()
            .enumerate()
        {
            let fact = PackageReviewContractFact::CarryMembership {
                value: PackageReviewContractExpression::Result,
                permission,
            };
            let mut encoder = Encoder::bounded(3);
            encode_contract_fact(&mut encoder, &fact).unwrap();
            assert_eq!(encoder.finish().unwrap(), [4, 3, tag as u8]);
        }
    }
}
