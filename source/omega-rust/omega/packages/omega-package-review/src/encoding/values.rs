use super::*;
use omega_effects::provider_plan::{
    ProviderBinding, ServiceEntryAuthorityFlow, ServiceProgressEstablishmentRouteKind,
    ServiceProgressSubject,
};
use psi_checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment,
};
pub(crate) fn encode_callable(
    encoder: &mut Encoder,
    callable: &CheckedPackageCallableReview,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match callable.role {
        PackageReviewCallableRole::Boundary => 0,
        PackageReviewCallableRole::Public => 1,
        PackageReviewCallableRole::Build => 2,
    });
    encode_nominal(encoder, &callable.identity)?;
    encode_supply(encoder, callable.supply)?;
    encoder.usize(callable.lifetime_parameter_count)?;
    encoder.sequence(&callable.type_parameters, encode_type_parameter)?;
    encoder.sequence(&callable.conformance_bounds, encode_conformance_bound)?;
    encoder.sequence(&callable.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &callable.return_type)?;
    encoder.sequence(&callable.conformances, encode_callable_conformance)?;
    encoder.sequence(&callable.operator_realizations, |encoder, realization| {
        encode_operator_coordinate(encoder, &realization.coordinate)?;
        encoder.option(realization.alias.as_deref(), |encoder, alias| {
            encoder.string(alias)
        })
    })?;
    encoder.sequence(&callable.contracts, encode_callable_contract)?;
    encoder.option(
        callable.declared_service_reach.as_deref(),
        |encoder, row| encoder.sequence(row, encode_nominal),
    )?;
    match &callable.checked_service_reach {
        PackageReviewCheckedServiceReach::NoCheckedBody => encoder.byte(0),
        PackageReviewCheckedServiceReach::CheckedBody { realized, concrete } => {
            encoder.byte(1);
            encoder.sequence(realized, encode_nominal)?;
            encoder.sequence(concrete, encode_nominal)?;
        }
    }
    encoder.sequence(
        &callable.unresolved_installation_reaches,
        encode_installation_reach,
    )?;
    encoder.option(
        callable.declared_synchronous_invocations.as_deref(),
        |encoder, invocations| encoder.sequence(invocations, encode_synchronous_invocation),
    )?;
    encoder.sequence(
        &callable.realized_synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.sequence(&callable.capability_flows, encode_capability_flow)?;
    encoder.boolean(callable.checked_may_suspend);
    encoder.boolean(callable.checked_may_block);
    encode_termination(encoder, &callable.checked_termination)?;
    encode_crash(encoder, &callable.checked_crash)?;
    encoder.sequence(&callable.mutation, encode_mutation)
}

pub(crate) fn encode_callable_conformance(
    encoder: &mut Encoder,
    conformance: &PackageReviewCallableConformance,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &conformance.trait_identity)?;
    encode_nominal(encoder, &conformance.requirement_identity)?;
    encoder.sequence(&conformance.arguments, encode_type_identity)?;
    encoder.option(conformance.alias.as_deref(), |encoder, alias| {
        encoder.string(alias)
    })
}

pub(crate) fn encode_external_executable_supply_key(
    encoder: &mut Encoder,
    supply: &PackageReviewExternalExecutableSupply,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &supply.callable)?;
    match &supply.requirement {
        PackageReviewExternalRequirement::Trait(conformance) => {
            encoder.byte(0);
            encode_callable_conformance(encoder, conformance)
        }
        PackageReviewExternalRequirement::Operator(operator) => {
            encoder.byte(1);
            encode_operator_coordinate(encoder, operator)
        }
    }
}

pub(crate) fn encode_external_executable_supply(
    encoder: &mut Encoder,
    supply: &PackageReviewExternalExecutableSupply,
) -> Result<(), PackageReviewEncodingError> {
    encode_external_executable_supply_key(encoder, supply)?;
    match &supply.binding {
        PackageReviewExternalBinding::Import { library, symbol } => {
            encoder.byte(0);
            encoder.string(library)?;
            encoder.string(symbol)?;
        }
        PackageReviewExternalBinding::Syscall { number } => {
            encoder.byte(1);
            encoder.i64(*number);
        }
        PackageReviewExternalBinding::CompilerIntrinsic => encoder.byte(2),
        PackageReviewExternalBinding::VtableSlot { index } => {
            encoder.byte(3);
            encoder.i64(*index);
        }
        PackageReviewExternalBinding::VtableField { field } => {
            encoder.byte(4);
            encoder.string(field)?;
        }
        PackageReviewExternalBinding::TableFunction { field } => {
            encoder.byte(5);
            encoder.string(field)?;
        }
    }
    Ok(())
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
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Type => 0,
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Const => 1,
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine => 2,
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

pub(crate) fn encode_proposition_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewPropositionShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.sequence(&shape.binders, encode_proposition_binder)?;
    encoder.sequence(&shape.parameter_types, encode_type_identity)?;
    match &shape.body {
        PackageReviewPublicPropositionBody::Primitive => encoder.byte(0),
        PackageReviewPublicPropositionBody::Witness(interface) => {
            encoder.byte(1);
            encode_evidence_interface(encoder, interface)?;
        }
        PackageReviewPublicPropositionBody::Transparent(expansion) => {
            encoder.byte(2);
            encode_contract_fact(encoder, expansion)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_const_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewConstShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encode_type_identity(encoder, &shape.declared_type)?;
    encoder.string(&shape.canonical_value_encoding)
}

pub(crate) fn encode_operator_coordinate(
    encoder: &mut Encoder,
    coordinate: &PackageReviewOperatorCoordinate,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &coordinate.identity)?;
    encoder.string(&coordinate.parameter_dispatch)?;
    encoder.string(&coordinate.result_dispatch)
}

pub(crate) fn encode_operator_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewOperatorShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_operator_coordinate(encoder, &shape.coordinate)?;
    encoder.boolean(shape.is_boundary);
    encoder.option(shape.spelling.as_ref(), |encoder, spelling| {
        encoder.byte(operator_spelling_tag(*spelling));
        Ok(())
    })?;
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encoder.sequence(&shape.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &shape.return_type)?;
    encoder.sequence(&shape.contracts, encode_callable_contract)?;
    encoder.sequence(&shape.published_crash, encode_crash_route)
}

pub(crate) const fn operator_spelling_tag(spelling: psi_language_core::OperatorSpelling) -> u8 {
    match spelling {
        psi_language_core::OperatorSpelling::Add => 0,
        psi_language_core::OperatorSpelling::Subtract => 1,
        psi_language_core::OperatorSpelling::Multiply => 2,
        psi_language_core::OperatorSpelling::Divide => 3,
        psi_language_core::OperatorSpelling::Modulo => 4,
        psi_language_core::OperatorSpelling::Equal => 5,
        psi_language_core::OperatorSpelling::NotEqual => 6,
        psi_language_core::OperatorSpelling::Less => 7,
        psi_language_core::OperatorSpelling::LessEqual => 8,
        psi_language_core::OperatorSpelling::Greater => 9,
        psi_language_core::OperatorSpelling::GreaterEqual => 10,
        psi_language_core::OperatorSpelling::Index => 11,
        psi_language_core::OperatorSpelling::Range => 12,
    }
}

pub(crate) fn encode_proposition_binder(
    encoder: &mut Encoder,
    binder: &PackageReviewPropositionBinder,
) -> Result<(), PackageReviewEncodingError> {
    match &binder.kind {
        PackageReviewPropositionBinderKind::Type => encoder.byte(0),
        PackageReviewPropositionBinderKind::Const(type_identity) => {
            encoder.byte(1);
            encode_type_identity(encoder, type_identity)?;
        }
        PackageReviewPropositionBinderKind::Machine => encoder.byte(2),
    }
    encode_data_properties(encoder, binder.bounds);
    Ok(())
}

pub(crate) fn encode_evidence_interface(
    encoder: &mut Encoder,
    interface: &PackageReviewEvidenceInterface,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &interface.trait_identity)?;
    encoder.sequence(&interface.arguments, encode_type_identity)?;
    encoder.sequence(&interface.requirements, |encoder, requirement| {
        encode_nominal(encoder, &requirement.declaring_trait)?;
        encoder.sequence(&requirement.declaring_trait_arguments, encode_type_identity)?;
        encode_nominal(encoder, &requirement.requirement)
    })
}

pub(crate) fn encode_contract_expression(
    encoder: &mut Encoder,
    expression: &PackageReviewContractExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        PackageReviewContractExpression::Boolean(value) => {
            encoder.byte(0);
            encoder.boolean(*value);
        }
        PackageReviewContractExpression::Integer(value) => {
            encoder.byte(1);
            encoder.string(value)?;
        }
        PackageReviewContractExpression::Array(values) => {
            encoder.byte(14);
            encoder.sequence(values, encode_contract_expression)?;
        }
        PackageReviewContractExpression::Constructor { data, case, fields } => {
            encoder.byte(15);
            encode_nominal(encoder, data)?;
            encoder.option(case.as_ref(), encode_nominal)?;
            encoder.sequence(fields, |encoder, field| {
                encode_nominal(encoder, &field.field)?;
                encode_contract_expression(encoder, &field.value)
            })?;
        }
        PackageReviewContractExpression::Indexed {
            meaning,
            collection,
            index,
        } => {
            encoder.byte(16);
            encode_contract_operator_meaning(encoder, meaning)?;
            encode_contract_expression(encoder, collection)?;
            encode_contract_expression(encoder, index)?;
        }
        PackageReviewContractExpression::Range {
            start,
            end,
            end_inclusive,
        } => {
            encoder.byte(17);
            encoder.option(start.as_deref(), encode_contract_expression)?;
            encoder.option(end.as_deref(), encode_contract_expression)?;
            encoder.boolean(*end_inclusive);
        }
        PackageReviewContractExpression::ByteSequence(value) => {
            encoder.byte(12);
            encoder.bytes(value)?;
        }
        PackageReviewContractExpression::DomainSubject => encoder.byte(10),
        PackageReviewContractExpression::Parameter(position) => {
            encoder.byte(2);
            encoder.u32(*position);
        }
        PackageReviewContractExpression::Result => encoder.byte(3),
        PackageReviewContractExpression::GenericBinder(position) => {
            encoder.byte(4);
            encoder.u32(*position);
        }
        PackageReviewContractExpression::Nominal(identity) => {
            encoder.byte(5);
            encode_nominal(encoder, identity)?;
        }
        PackageReviewContractExpression::ZeroValue(type_identity) => {
            encoder.byte(13);
            encode_type_identity(encoder, type_identity)?;
        }
        PackageReviewContractExpression::CollectionLength { collection } => {
            encoder.byte(18);
            encode_contract_expression(encoder, collection)?;
        }
        PackageReviewContractExpression::Member {
            receiver,
            member,
            case_variant,
        } => {
            encoder.byte(8);
            encode_contract_expression(encoder, receiver)?;
            encode_nominal(encoder, member)?;
            encoder.option(case_variant.as_ref(), encode_nominal)?;
        }
        PackageReviewContractExpression::Cast {
            value,
            target,
            arithmetic_domain,
            semantic_domain,
            semantic_domain_arguments,
            form,
        } => {
            encoder.byte(9);
            encode_contract_expression(encoder, value)?;
            encode_type_identity(encoder, target)?;
            encoder.byte(match arithmetic_domain {
                PackageReviewArithmeticDomain::Exact => 0,
                PackageReviewArithmeticDomain::Wrapping => 1,
                PackageReviewArithmeticDomain::Saturating => 2,
                PackageReviewArithmeticDomain::Trapping => 3,
            });
            encoder.option(semantic_domain.as_ref(), encode_nominal)?;
            encoder.sequence(semantic_domain_arguments, encode_type_identity)?;
            encoder.byte(match form {
                PackageReviewCastForm::Value => 0,
                PackageReviewCastForm::RecastShared => 1,
                PackageReviewCastForm::RecastMutable => 2,
            });
        }
        PackageReviewContractExpression::Call {
            receiver,
            target,
            static_arguments,
            arguments,
        } => {
            encoder.byte(11);
            encoder.option(receiver.as_deref(), encode_contract_expression)?;
            match target {
                PackageReviewContractCallTarget::Nominal(identity) => {
                    encoder.byte(0);
                    encode_nominal(encoder, identity)?;
                }
                PackageReviewContractCallTarget::BuiltinFunction(function) => {
                    encoder.byte(2);
                    encoder.u16(u16::try_from(function.ordinal()).map_err(|_| {
                        PackageReviewEncodingError::new(
                            "compiler builtin-function ordinal exceeds the portable encoding range",
                        )
                    })?);
                }
                PackageReviewContractCallTarget::ByteSequencePredicate(predicate) => {
                    encoder.byte(1);
                    encoder.byte(match predicate {
                        PackageReviewByteSequencePredicate::ValidUtf8 => 0,
                        PackageReviewByteSequencePredicate::NoNul => 1,
                        PackageReviewByteSequencePredicate::AsciiOnly => 2,
                        PackageReviewByteSequencePredicate::NonEmpty => 3,
                    });
                }
            }
            encoder.sequence(static_arguments, encode_contract_static_argument)?;
            encoder.sequence(arguments, encode_contract_expression)?;
        }
        PackageReviewContractExpression::Binary {
            meaning,
            operator,
            left,
            right,
        } => {
            encoder.byte(6);
            encode_contract_operator_meaning(encoder, meaning)?;
            encoder.byte(match operator {
                PackageReviewContractBinaryOperator::Add => 0,
                PackageReviewContractBinaryOperator::And => 1,
                PackageReviewContractBinaryOperator::BitwiseAnd => 2,
                PackageReviewContractBinaryOperator::BitwiseOr => 3,
                PackageReviewContractBinaryOperator::BitwiseXor => 4,
                PackageReviewContractBinaryOperator::Divide => 5,
                PackageReviewContractBinaryOperator::Equal => 6,
                PackageReviewContractBinaryOperator::Greater => 7,
                PackageReviewContractBinaryOperator::GreaterOrEqual => 8,
                PackageReviewContractBinaryOperator::Less => 9,
                PackageReviewContractBinaryOperator::LessOrEqual => 10,
                PackageReviewContractBinaryOperator::Modulo => 11,
                PackageReviewContractBinaryOperator::Multiply => 12,
                PackageReviewContractBinaryOperator::NotEqual => 13,
                PackageReviewContractBinaryOperator::Or => 14,
                PackageReviewContractBinaryOperator::ShiftLeft => 15,
                PackageReviewContractBinaryOperator::ShiftRight => 16,
                PackageReviewContractBinaryOperator::Subtract => 17,
            });
            encode_contract_expression(encoder, left)?;
            encode_contract_expression(encoder, right)?;
        }
        PackageReviewContractExpression::Unary { operator, operand } => {
            encoder.byte(7);
            encoder.byte(match operator {
                PackageReviewContractUnaryOperator::BitwiseNot => 0,
                PackageReviewContractUnaryOperator::LogicalNot => 1,
            });
            encode_contract_expression(encoder, operand)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_contract_operator_meaning(
    encoder: &mut Encoder,
    meaning: &PackageReviewContractOperatorMeaning,
) -> Result<(), PackageReviewEncodingError> {
    match meaning {
        PackageReviewContractOperatorMeaning::Builtin => encoder.byte(0),
        PackageReviewContractOperatorMeaning::Declared(coordinate) => {
            encoder.byte(1);
            encode_operator_coordinate(encoder, coordinate)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_contract_static_argument(
    encoder: &mut Encoder,
    argument: &PackageReviewContractStaticArgument,
) -> Result<(), PackageReviewEncodingError> {
    match argument {
        PackageReviewContractStaticArgument::Type(identity) => {
            encoder.byte(0);
            encode_type_identity(encoder, identity)?;
        }
        PackageReviewContractStaticArgument::GenericTypeBinder(position) => {
            encoder.byte(5);
            encoder.u32(*position);
        }
        PackageReviewContractStaticArgument::GenericType {
            base,
            lifetime_arguments,
            arguments,
        } => {
            encoder.byte(1);
            encode_type_identity(encoder, base)?;
            encoder.sequence(lifetime_arguments, |encoder, argument| {
                encoder.u32(*argument);
                Ok(())
            })?;
            encoder.sequence(arguments, encode_contract_static_argument)?;
        }
        PackageReviewContractStaticArgument::ConstInteger(value) => {
            encoder.byte(2);
            encoder.string(value)?;
        }
        PackageReviewContractStaticArgument::GenericConstBinder(position) => {
            encoder.byte(6);
            encoder.u32(*position);
        }
        PackageReviewContractStaticArgument::GenericMachineBinder(position) => {
            encoder.byte(3);
            encoder.u32(*position);
        }
        PackageReviewContractStaticArgument::ConcreteMachine(identity) => {
            encoder.byte(4);
            encode_nominal(encoder, identity)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_synchronous_invocation(
    encoder: &mut Encoder,
    invocation: &PackageReviewSynchronousInvocation,
) -> Result<(), PackageReviewEncodingError> {
    match invocation {
        PackageReviewSynchronousInvocation::Parameter(position) => {
            encoder.byte(0);
            encoder.u32(*position);
        }
        PackageReviewSynchronousInvocation::Service(service) => {
            encoder.byte(1);
            encode_nominal(encoder, service)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_nominal(
    encoder: &mut Encoder,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), PackageReviewEncodingError> {
    match identity.owner {
        PackageReviewNominalOwner::Package(package) => {
            encoder.byte(0);
            encoder.package_identity(package);
        }
        PackageReviewNominalOwner::ToolchainSource(source) => {
            encoder.byte(1);
            encoder.fixed_bytes(&source.digest());
        }
        PackageReviewNominalOwner::Unresolved => {
            return Err(PackageReviewEncodingError::new(
                "package review cannot encode unresolved nominal ownership",
            ));
        }
    }
    encoder.string(&identity.path)
}

pub(crate) fn encode_supply(
    encoder: &mut Encoder,
    supply: PackageReviewCallableSupply,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match supply {
        PackageReviewCallableSupply::CheckedBody => 0,
        PackageReviewCallableSupply::Requirement => 1,
        PackageReviewCallableSupply::Boundary => 2,
        PackageReviewCallableSupply::Accepted => 3,
        PackageReviewCallableSupply::ExternalRealization => 4,
    });
    Ok(())
}

pub(crate) fn encode_installation_reach(
    encoder: &mut Encoder,
    reach: &PackageReviewInstallationReach,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &reach.requirement)?;
    encoder.sequence(&reach.upper_bound, encode_nominal)
}

pub(crate) fn encode_capability_flow(
    encoder: &mut Encoder,
    flow: &PackageReviewCapabilityFlow,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &flow.capability)?;
    encoder.byte(match flow.kind {
        psi_effects::CapabilityFlowKind::Uses => 0,
        psi_effects::CapabilityFlowKind::Returns => 1,
        psi_effects::CapabilityFlowKind::Acquires => 2,
        psi_effects::CapabilityFlowKind::Stores => 3,
        psi_effects::CapabilityFlowKind::Derives => 4,
    });
    encode_nominal(encoder, &flow.state)?;
    encoder.usize(flow.statement_index)?;
    encoder.usize(flow.call_ordinal)?;
    encoder.option(flow.via_state.as_ref(), encode_nominal)
}

pub(crate) fn encode_termination(
    encoder: &mut Encoder,
    termination: &PackageReviewTermination,
) -> Result<(), PackageReviewEncodingError> {
    match termination {
        PackageReviewTermination::NoGuarantee => encoder.byte(0),
        PackageReviewTermination::Terminates { premises } => {
            encoder.byte(1);
            encoder.sequence(premises, |encoder, premise| {
                encode_nominal(encoder, &premise.profile)?;
                match &premise.subject {
                    PackageReviewProgressSubject::Declaration(identity) => {
                        encoder.byte(0);
                        encode_nominal(encoder, identity)?;
                    }
                    PackageReviewProgressSubject::Receiver => encoder.byte(1),
                    PackageReviewProgressSubject::Parameter(position) => {
                        encoder.byte(2);
                        encoder.u32(*position);
                    }
                }
                encoder.sequence(&premise.projections, encode_nominal)
            })?;
        }
    }
    Ok(())
}

pub(crate) fn encode_mutation(
    encoder: &mut Encoder,
    mutation: &PackageReviewMutation,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &mutation.state)?;
    encoder.byte(match mutation.completeness {
        psi_facts::WriteFrameCompleteness::Complete => 0,
        psi_facts::WriteFrameCompleteness::Opaque => 1,
    });
    encoder.sequence(&mutation.paths, |encoder, path| encoder.string(path))
}

pub(crate) fn encode_crash(
    encoder: &mut Encoder,
    crash: &PackageReviewCrash,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match crash.interface {
        PackageReviewCrashInterface::InternalInferred => 0,
        PackageReviewCrashInterface::PublishedCeiling => 1,
    });
    encoder.sequence(&crash.published, encode_crash_route)?;
    encoder.option(
        crash.structural_runtime_requirements.as_deref(),
        |encoder, requirements| encoder.sequence(requirements, encode_boolean_expression),
    )?;
    encoder.sequence(&crash.checked_sites, encode_crash_site)?;
    encoder.sequence(&crash.checked_calls, encode_crash_call)
}

pub(crate) fn encode_crash_route(
    encoder: &mut Encoder,
    route: &PackageReviewCrashRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match route.cause {
        psi_checked_trees::CrashCause::Trap => 0,
        psi_checked_trees::CrashCause::Abort => 1,
    });
    encoder.sequence(&route.alternative_guards, |encoder, guard| {
        match guard {
            PackageReviewCrashRouteGuard::Truth => encoder.byte(0),
            PackageReviewCrashRouteGuard::Predicate(predicate) => {
                encoder.byte(1);
                encoder.bytes(&predicate.canonical_bytes)?;
            }
            PackageReviewCrashRouteGuard::Expression(expression) => {
                encoder.byte(2);
                encode_contract_expression(encoder, expression)?;
            }
        }
        Ok(())
    })
}

pub(crate) fn encode_crash_site(
    encoder: &mut Encoder,
    site: &PackageReviewCrashSite,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &site.state)?;
    encoder.u32(site.statement_ordinal);
    encoder.byte(match site.cause {
        psi_checked_trees::CrashCause::Trap => 0,
        psi_checked_trees::CrashCause::Abort => 1,
    });
    encoder.sequence(&site.path_guard_conjuncts, encode_crash_predicate)?;
    encoder.sequence(&site.path_guard_consequences, encode_crash_predicate)?;
    encoder.sequence(&site.guard_covering_buckets, |encoder, bucket| {
        encoder.u32(*bucket);
        Ok(())
    })?;
    encoder.sequence(&site.frontier_lower_bound, encode_permission_claim)
}

pub(crate) fn encode_crash_predicate(
    encoder: &mut Encoder,
    predicate: &PackageReviewCrashPredicate,
) -> Result<(), PackageReviewEncodingError> {
    encoder.bytes(&predicate.canonical_bytes)
}

pub(crate) fn encode_permission_claim(
    encoder: &mut Encoder,
    claim: &PackageReviewPermissionClaim,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &claim.machine)?;
    encode_nominal(encoder, &claim.state)?;
    match &claim.source {
        PackageReviewPermissionSource::StateEntry => encoder.byte(0),
        PackageReviewPermissionSource::Statement { statement_ordinal } => {
            encoder.byte(1);
            encoder.u64(*statement_ordinal);
        }
        PackageReviewPermissionSource::Call {
            statement_ordinal,
            call_ordinal,
            target,
        } => {
            encoder.byte(2);
            encoder.u64(*statement_ordinal);
            encoder.u64(*call_ordinal);
            encode_nominal(encoder, target)?;
        }
        PackageReviewPermissionSource::StateExit => encoder.byte(3),
    }
    encoder.u32(claim.ordinal);
    Ok(())
}

pub(crate) fn encode_crash_call(
    encoder: &mut Encoder,
    call: &PackageReviewCrashCall,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &call.state)?;
    encoder.u32(call.statement_ordinal);
    encoder.u32(call.call_ordinal);
    encode_nominal(encoder, &call.target_machine)?;
    encode_nominal(encoder, &call.target_state)?;
    encoder.sequence(&call.path_guard_conjuncts, encode_crash_predicate)?;
    encoder.sequence(&call.path_guard_consequences, encode_crash_predicate)?;
    encoder.sequence(&call.surviving_buckets, encode_crash_route)
}

pub(crate) fn encode_boolean_expression(
    encoder: &mut Encoder,
    expression: &CheckedBooleanExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        CheckedBooleanExpression::Constant(value) => {
            encoder.byte(0);
            encoder.boolean(*value);
        }
        CheckedBooleanExpression::Parameter { position } => {
            encoder.byte(1);
            encoder.usize(*position)?;
        }
        CheckedBooleanExpression::Local { position } => {
            encoder.byte(2);
            encoder.usize(*position)?;
        }
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => {
            encoder.byte(3);
            encoder.u32(*parameter_position);
            encode_structural_path(encoder, path)?;
        }
        CheckedBooleanExpression::Not(operand) => {
            encoder.byte(4);
            encode_boolean_expression(encoder, operand)?;
        }
        CheckedBooleanExpression::Equal { left, right } => {
            encoder.byte(5);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            encoder.byte(6);
            encoder.byte(integer_comparison_tag(*kind));
            encode_scalar_expression(encoder, left)?;
            encode_scalar_expression(encoder, right)?;
        }
        CheckedBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.byte(7);
            encoder.byte(match kind {
                psi_checked_trees::CheckedIeeeFloatComparisonKind::Equal => 0,
                psi_checked_trees::CheckedIeeeFloatComparisonKind::NotEqual => 1,
            });
            encode_primitive_type(encoder, *primitive_type);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
        }
        CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
            encoder.byte(8);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
        }
        CheckedBooleanExpression::PayloadlessSumEqual { left, right, cases } => {
            encoder.byte(9);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
            encoder.sequence(cases, |encoder, case| encoder.string(case))?;
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            encoder.byte(10);
            encode_structural_field(encoder, subject)?;
            encoder.string(case)?;
        }
        CheckedBooleanExpression::And { left, right } => {
            encoder.byte(11);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
        CheckedBooleanExpression::Or { left, right } => {
            encoder.byte(12);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_scalar_expression(
    encoder: &mut Encoder,
    expression: &CheckedScalarExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => {
            encoder.byte(0);
            encoder.usize(*position)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => {
            encoder.byte(1);
            encoder.usize(*position)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => {
            encoder.byte(2);
            encoder.u32(*parameter_position);
            encode_structural_path(encoder, path)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        CheckedScalarExpression::IntegerLiteral { literal } => {
            encoder.byte(3);
            encoder.string(literal.text())?;
            let landing = literal.landing();
            encoder.option(landing.as_ref(), |encoder, landing| {
                encoder.string(landing.landed_type.name())?;
                encoder.string(landing.domain.name())
            })?;
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.byte(4);
            encoder.byte(integer_binary_tag(*kind));
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, left)?;
            encode_scalar_expression(encoder, right)?;
        }
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => {
            encoder.byte(5);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
        }
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => {
            encoder.byte(6);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
        }
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => {
            encoder.byte(7);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
            encoder.string(&range.minimum.to_string())?;
            encoder.string(&range.maximum.to_string())?;
        }
        CheckedScalarExpression::Boolean(expression) => {
            encoder.byte(8);
            encode_boolean_expression(encoder, expression)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_structural_field(
    encoder: &mut Encoder,
    field: &CheckedStructuralParameterField,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u32(field.parameter_position);
    encode_structural_path(encoder, &field.path)
}

pub(crate) fn encode_structural_path(
    encoder: &mut Encoder,
    path: &[CheckedStructuralPredicatePathSegment],
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(path, |encoder, segment| {
        match segment {
            CheckedStructuralPredicatePathSegment::Field(field) => {
                encoder.byte(0);
                encoder.string(field)?;
            }
            CheckedStructuralPredicatePathSegment::Case(case) => {
                encoder.byte(1);
                encoder.string(case)?;
            }
        }
        Ok(())
    })
}

pub(crate) fn encode_primitive_type(
    encoder: &mut Encoder,
    primitive_type: psi_typed_trees::types::PrimitiveType,
) {
    encoder.byte(match primitive_type {
        psi_typed_trees::types::PrimitiveType::Bool => 0,
        psi_typed_trees::types::PrimitiveType::F32 => 1,
        psi_typed_trees::types::PrimitiveType::F64 => 2,
        psi_typed_trees::types::PrimitiveType::I8 => 3,
        psi_typed_trees::types::PrimitiveType::I16 => 4,
        psi_typed_trees::types::PrimitiveType::I32 => 5,
        psi_typed_trees::types::PrimitiveType::I64 => 6,
        psi_typed_trees::types::PrimitiveType::U8 => 7,
        psi_typed_trees::types::PrimitiveType::U16 => 8,
        psi_typed_trees::types::PrimitiveType::U32 => 9,
        psi_typed_trees::types::PrimitiveType::U64 => 10,
        psi_typed_trees::types::PrimitiveType::Addr => 11,
    });
}

pub(crate) const fn integer_comparison_tag(kind: CheckedIntegerComparisonKind) -> u8 {
    match kind {
        CheckedIntegerComparisonKind::Equal => 0,
        CheckedIntegerComparisonKind::LessThan => 1,
        CheckedIntegerComparisonKind::LessOrEqual => 2,
    }
}

pub(crate) const fn integer_binary_tag(kind: CheckedIntegerBinaryKind) -> u8 {
    match kind {
        CheckedIntegerBinaryKind::ExactAdd => 0,
        CheckedIntegerBinaryKind::ExactSubtract => 1,
        CheckedIntegerBinaryKind::ExactMultiply => 2,
        CheckedIntegerBinaryKind::ExactDivide => 3,
        CheckedIntegerBinaryKind::ExactRemainder => 4,
        CheckedIntegerBinaryKind::WrappingDivide => 5,
        CheckedIntegerBinaryKind::WrappingRemainder => 6,
        CheckedIntegerBinaryKind::SaturatingDivide => 7,
        CheckedIntegerBinaryKind::SaturatingRemainder => 8,
        CheckedIntegerBinaryKind::WrappingAdd => 9,
        CheckedIntegerBinaryKind::SaturatingAdd => 10,
        CheckedIntegerBinaryKind::WrappingSubtract => 11,
        CheckedIntegerBinaryKind::SaturatingSubtract => 12,
        CheckedIntegerBinaryKind::WrappingMultiply => 13,
        CheckedIntegerBinaryKind::SaturatingMultiply => 14,
        CheckedIntegerBinaryKind::BitwiseAnd => 15,
        CheckedIntegerBinaryKind::BitwiseOr => 16,
        CheckedIntegerBinaryKind::BitwiseXor => 17,
        CheckedIntegerBinaryKind::WrappingShiftLeft => 18,
        CheckedIntegerBinaryKind::WrappingShiftRight => 19,
        CheckedIntegerBinaryKind::ExactShiftLeft => 20,
        CheckedIntegerBinaryKind::ExactShiftRight => 21,
    }
}

pub(crate) fn encode_provider(
    encoder: &mut Encoder,
    provider: &CheckedPackageProviderReview,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&provider.plan_name)?;
    encoder.u64(provider.plan_fingerprint);
    encoder.optional_package_identity(provider.realizing_package);
    encode_nominal(encoder, &provider.schema_declaration)?;
    encoder.string(&provider.provider_type)?;
    encoder.optional_package_identity(provider.provider_type_package);
    encoder.option(provider.provider_type_declaration.as_ref(), encode_nominal)?;
    encode_service_schema(encoder, &provider.schema)?;
    encoder.string(&provider.target)?;
    encoder.sequence(&provider.rows, encode_provider_row)?;
    encoder.sequence(&provider.row_declarations, |encoder, row| {
        encode_nominal(encoder, &row.requirement)?;
        encode_nominal(encoder, &row.realization)?;
        encoder.option(
            row.compiler_intrinsic_builtin.as_ref(),
            |encoder, function| {
                encoder.u16(u16::try_from(function.ordinal()).map_err(|_| {
                    PackageReviewEncodingError::new(
                        "compiler builtin-function ordinal exceeds the portable encoding range",
                    )
                })?);
                Ok(())
            },
        )
    })
}

pub(crate) fn encode_service_schema(
    encoder: &mut Encoder,
    schema: &omega_effects::provider_plan::ServiceSchema,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&schema.trait_name)?;
    encoder.optional_package_identity(schema.trait_package_identity);
    encoder.sequence(&schema.methods, |encoder, method| {
        encoder.string(&method.name)?;
        encoder.string(&method.requirement_owner)?;
        encoder.optional_package_identity(method.requirement_owner_package_identity);
        encoder.string(&method.requirement_identity)?;
        encoder.usize(method.parameter_count)?;
        encoder.sequence(&method.parameter_type_identities, |encoder, identity| {
            encoder.string(identity)
        })?;
        encoder.sequence(&method.entry_claims, |encoder, claim| {
            encoder.usize(claim.parameter_index)?;
            encoder.string(&claim.carrier_identity)?;
            encoder.string(&claim.domain)?;
            encoder.byte(match claim.predicate_body {
                psi_language_semantics::DomainPredicateBody::Bodyless => 0,
                psi_language_semantics::DomainPredicateBody::Present => 1,
            });
            encode_carry_policy(encoder, claim.effective_carry);
            encoder.byte(match claim.authority_flow {
                ServiceEntryAuthorityFlow::Accepts => 0,
            });
            Ok(())
        })?;
        encoder.boolean(method.has_result);
        encoder.option(method.result_type_identity.as_ref(), |encoder, identity| {
            encoder.string(identity)
        })?;
        encoder.sequence(&method.result_claims, |encoder, claim| {
            encoder.string(&claim.domain)?;
            encode_carry_policy(encoder, claim.effective_carry);
            Ok(())
        })?;
        encoder.sequence(&method.service_reach, |encoder, service| {
            encoder.string(service)
        })?;
        encoder.sequence(&method.synchronous_invocations, |encoder, invocation| {
            encoder.string(invocation)
        })?;
        encoder.boolean(method.may_suspend);
        encoder.boolean(method.may_block);
        encoder.boolean(method.terminates_guarantee);
        encoder.sequence(&method.termination_premises, |encoder, premise| {
            encoder.string(&premise.profile)?;
            match premise.subject {
                ServiceProgressSubject::ProviderReceiver => encoder.byte(0),
                ServiceProgressSubject::Parameter(position) => {
                    encoder.byte(1);
                    encoder.usize(position)?;
                }
            }
            encoder.sequence(&premise.subject_projections, |encoder, projection| {
                encoder.string(projection)
            })?;
            encoder.sequence(&premise.establishment_routes, |encoder, route| {
                encoder.byte(match route.kind {
                    ServiceProgressEstablishmentRouteKind::CheckedRequirement => 0,
                    ServiceProgressEstablishmentRouteKind::BoundaryRequirement => 1,
                });
                encoder.string(&route.requirement_identity)
            })
        })?;
        encoder.option(
            method.calling_plan_fingerprint.as_ref(),
            |encoder, fingerprint| {
                encoder.u64(*fingerprint);
                Ok(())
            },
        )
    })
}

pub(crate) fn encode_carry_policy(
    encoder: &mut Encoder,
    policy: psi_language_semantics::CarryPolicy,
) {
    encoder.byte(match policy.suspension {
        psi_language_semantics::CarrySuspension::Forbidden => 0,
        psi_language_semantics::CarrySuspension::Allowed => 1,
    });
    encoder.byte(match policy.cpu {
        psi_language_semantics::CarryCpu::Origin => 0,
        psi_language_semantics::CarryCpu::Any => 1,
    });
    encoder.byte(match policy.host_thread {
        psi_language_semantics::CarryHostThread::Origin => 0,
        psi_language_semantics::CarryHostThread::Any => 1,
    });
    encoder.byte(match policy.address {
        psi_language_semantics::CarryAddress::Stable => 0,
        psi_language_semantics::CarryAddress::Movable => 1,
    });
}

pub(crate) fn encode_provider_row(
    encoder: &mut Encoder,
    row: &omega_effects::provider_plan::ProviderPlanRow,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&row.method)?;
    encoder.string(&row.requirement_identity)?;
    match &row.binding {
        ProviderBinding::Import { locator } => {
            encoder.byte(7);
            encoder.string(locator.target().target_name())?;
            encoder.u64(locator.normalized_identity());
            match locator.locator() {
                omega_effects::ForeignLocatorCandidate::PeByName { library, export } => {
                    encoder.byte(0);
                    encoder.bytes(library)?;
                    encoder.bytes(export)?;
                }
                omega_effects::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                    encoder.byte(1);
                    encoder.bytes(library)?;
                    encoder.u16(*ordinal);
                }
                omega_effects::ForeignLocatorCandidate::ElfVersioned {
                    object,
                    symbol,
                    version,
                } => {
                    encoder.byte(2);
                    encoder.bytes(object)?;
                    encoder.bytes(symbol)?;
                    encoder.bytes(version)?;
                }
            }
        }
        ProviderBinding::StringBackedImportBootstrap { library, symbol } => {
            encoder.byte(0);
            encoder.string(library)?;
            encoder.string(symbol)?;
        }
        ProviderBinding::Syscall { number } => {
            encoder.byte(1);
            encoder.i64(*number);
        }
        ProviderBinding::CompilerIntrinsic { machine } => {
            encoder.byte(2);
            encoder.string(machine)?;
        }
        ProviderBinding::VtableSlot { index } => {
            encoder.byte(3);
            encoder.i64(*index);
        }
        ProviderBinding::VtableField { table, field } => {
            encoder.byte(4);
            encoder.string(table)?;
            encoder.string(field)?;
        }
        ProviderBinding::TableFunction { table, field } => {
            encoder.byte(5);
            encoder.string(table)?;
            encoder.string(field)?;
        }
        ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } => {
            encoder.byte(6);
            encoder.string(machine_identity)?;
            encoder.optional_package_identity(*machine_package_identity);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::PackageKeyIdentity;

    pub(crate) fn normalized_import_row(
        export: &[u8],
    ) -> omega_effects::provider_plan::ProviderPlanRow {
        omega_effects::provider_plan::ProviderPlanRow {
            method: "write".to_owned(),
            requirement_identity: "Console::write#exact".to_owned(),
            binding: ProviderBinding::Import {
                locator: omega_effects::normalize_foreign_locator(
                    omega_effects::ForeignLocatorCandidate::PeByName {
                        library: b"kernel32.dll".to_vec(),
                        export: export.to_vec(),
                    },
                    omega_target::TargetProfile::WindowsX64,
                )
                .expect("normalized import fixture"),
            },
        }
    }

    #[test]
    pub(crate) fn normalized_import_review_encoding_retains_exact_atomic_locator() {
        fn encoded(export: &[u8]) -> Vec<u8> {
            let mut encoder = Encoder::bounded(1024);
            encode_provider_row(&mut encoder, &normalized_import_row(export))
                .expect("encode normalized import");
            encoder.finish().expect("bounded encoding")
        }

        let write = encoded(b"WriteFile");
        let read = encoded(b"ReadFile");
        assert_ne!(write, read);
        assert!(
            write
                .windows(b"kernel32.dll".len())
                .any(|bytes| bytes == b"kernel32.dll")
        );
        assert!(
            write
                .windows(b"WriteFile".len())
                .any(|bytes| bytes == b"WriteFile")
        );
    }

    pub(crate) fn empty_review() -> CheckedPackageReviewProjection {
        CheckedPackageReviewProjection {
            package: PackageKeyIdentity::from_digest([1; 32]).expect("nonzero package identity"),
            target: omega_target::TargetProfile::WindowsX64,
            public_traits: Vec::new(),
            public_conformances: Vec::new(),
            public_domains: Vec::new(),
            public_propositions: Vec::new(),
            public_consts: Vec::new(),
            public_operators: Vec::new(),
            public_data: Vec::new(),
            representation_tcb: Vec::new(),
            semantic_dependencies: Vec::new(),
            callables: Vec::new(),
            external_executable_supply: Vec::new(),
            dangerous_authorities: Vec::new(),
            dangerous_authority_slack: Vec::new(),
            selected_providers: Vec::new(),
            row_sources: PackageReviewCanonicalRowSources {
                public_traits: Vec::new(),
                public_conformances: Vec::new(),
                public_domains: Vec::new(),
                public_propositions: Vec::new(),
                public_consts: Vec::new(),
                public_operators: Vec::new(),
                public_data: Vec::new(),
                representation_tcb: Vec::new(),
                semantic_dependencies: Vec::new(),
                callables: Vec::new(),
                external_executable_supply: Vec::new(),
                dangerous_authorities: Vec::new(),
                dangerous_authority_slack: Vec::new(),
                selected_provider_set: PackageReviewCanonicalRowSource::compiler_derived(
                    PackageReviewSyntheticSourceKind::EmptySelectedProviderSet,
                ),
            },
        }
    }

    #[test]
    pub(crate) fn bounded_encoders_reject_instead_of_returning_partial_evidence() {
        let review = empty_review();
        assert!(encode(&review).is_ok());
        assert!(encode_rows(&review).is_ok());

        assert!(
            encode_with_limits(
                &review,
                PackageReviewEncodingLimits::new(1, 2, 64, 256, 512)
            )
            .is_err()
        );
        assert!(
            encode_rows_with_limits(
                &review,
                PackageReviewEncodingLimits::new(256, 1, 64, 256, 512),
            )
            .is_err()
        );
        assert!(
            encode_rows_with_limits(
                &review,
                PackageReviewEncodingLimits::new(256, 2, 64, 1, 512),
            )
            .is_err()
        );
        assert!(
            encode_rows_with_limits(
                &review,
                PackageReviewEncodingLimits::new(256, 2, 64, 256, 1),
            )
            .is_err()
        );
    }

    #[test]
    pub(crate) fn canonical_encoding_rejects_unresolved_nominal_ownership() {
        let identity = PackageReviewNominalIdentity {
            owner: PackageReviewNominalOwner::Unresolved,
            path: "source_free::nominal".to_owned(),
        };
        let error = encode_nominal(&mut Encoder::bounded(1024), &identity)
            .expect_err("unresolved ownership must not enter canonical review bytes");
        assert_eq!(
            error.to_string(),
            "package review cannot encode unresolved nominal ownership"
        );
    }
}
