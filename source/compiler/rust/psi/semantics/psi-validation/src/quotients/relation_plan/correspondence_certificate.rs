//! Non-executable composition of direct quotient correspondence evidence.
//!
//! The lift rung here is intentionally bounded to exact direct public
//! arguments, including omission, permutation, repeated occurrences, and
//! exact closed literal substitution, plus exact structural inclusion and one
//! strict integer-expression entailment rung. It is not the general
//! membership/proposition implication or adapted-argument judgment.

use super::precondition::{
    DefinePreconditionCorrespondence, RepresentativeContractFactLocation,
    RepresentativeContractOwner, RepresentativePreconditionPartition, precondition_fact_at,
};
use super::proof_fact_identity::{
    ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match,
};
use super::runtime_correspondence::{
    DefineRuntimeCorrespondence, DirectLiftArgumentSource, DirectLiftRuntimeCorrespondence,
};
use super::theorem_schema::{
    ExpectedTheoremSchema, TheoremApplicationSide, TheoremContractFactLocation,
    TheoremContractOwner,
};
use super::theorem_schema_verification::VerifiedTheoremSchema;
use super::{RelationPlanError, RepresentativeTelescope};
use crate::contract_entailment::{
    StrictArithmeticBindingValue, StrictArithmeticImplicationJudgment,
    StrictArithmeticSymbolBinding, strict_arithmetic_expression_implication,
};
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_numerics::literals::LandedIntegerType;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DirectLiftPreconditionProof {
    ExactMatch {
        public: RepresentativeContractFactLocation,
    },
    ArithmeticEntailment {
        /// Complete authored dependent-Q roster, in source contract order.
        /// The strict kernel consumes this exact list; it does not report a
        /// smaller opaque premise subset.
        premises: Vec<RepresentativeContractFactLocation>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirectLiftPreconditionFactRow {
    pub(super) application: TheoremApplicationSide,
    pub(super) proof: DirectLiftPreconditionProof,
    pub(super) representative: RepresentativeContractFactLocation,
    pub(super) theorem: TheoremContractFactLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct DirectLiftPreconditionImplication {
    pub(super) rows: Vec<DirectLiftPreconditionFactRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum QuotientCorrespondenceEvidence {
    DirectLift {
        runtime: DirectLiftRuntimeCorrespondence,
        precondition: DirectLiftPreconditionImplication,
    },
    Define {
        runtime: DefineRuntimeCorrespondence,
        precondition: DefinePreconditionCorrespondence,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct QuotientCorrespondenceCertificate {
    pub(super) theorem: VerifiedTheoremSchema,
    pub(super) evidence: QuotientCorrespondenceEvidence,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_direct_lift_precondition_implication(
    program: &TypedTrees,
    public_machine: &Machine,
    public_state: &State,
    representative: &RepresentativeTelescope,
    public: &RepresentativePreconditionPartition,
    representative_partition: &RepresentativePreconditionPartition,
    runtime: &DirectLiftRuntimeCorrespondence,
    expected_theorem: &ExpectedTheoremSchema,
    verified_theorem: &VerifiedTheoremSchema,
) -> Result<DirectLiftPreconditionImplication, RelationPlanError> {
    let mut rows = Vec::with_capacity(representative_partition.dependent.len() * 2);
    for application in [TheoremApplicationSide::Left, TheoremApplicationSide::Right] {
        let application_schema = match application {
            TheoremApplicationSide::Left => &expected_theorem.left_application,
            TheoremApplicationSide::Right => &expected_theorem.right_application,
        };
        if runtime.positions.len() != application_schema.arguments.len() {
            return Err(RelationPlanError::DirectLiftRuntimeArityMismatch);
        }
        let mut public_values = Vec::new();
        let mut representative_values = Vec::with_capacity(runtime.positions.len());
        for (position, theorem_position) in
            runtime.positions.iter().zip(&application_schema.arguments)
        {
            match &position.source {
                DirectLiftArgumentSource::PublicParameter(public_parameter) => {
                    let value = public_values
                        .iter()
                        .find_map(|value: &ProofValueSubstitution| {
                            (value.symbol == *public_parameter).then(|| value.clone())
                        })
                        .unwrap_or_else(|| {
                            let value = ProofValueSubstitution::symbolic(
                                *public_parameter,
                                format!("$theorem_parameter_{theorem_position}"),
                            );
                            public_values.push(value.clone());
                            value
                        });
                    representative_values.push(value.rebound(position.representative_parameter));
                }
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::Boolean(value),
                ) => representative_values.push(ProofValueSubstitution::boolean(
                    position.representative_parameter,
                    *value,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::Integer { spelling, landing },
                ) => representative_values.push(ProofValueSubstitution::integer(
                    position.representative_parameter,
                    spelling,
                    *landing,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::Float { spelling, landing },
                ) => representative_values.push(ProofValueSubstitution::float(
                    position.representative_parameter,
                    spelling,
                    *landing,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::ByteString { bytes, .. },
                ) => representative_values.push(ProofValueSubstitution::byte_string(
                    position.representative_parameter,
                    bytes,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::FixedByteArray {
                        bytes, ..
                    },
                ) => representative_values.push(ProofValueSubstitution::fixed_byte_array(
                    position.representative_parameter,
                    bytes,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::BooleanArray {
                        values, ..
                    },
                ) => representative_values.push(ProofValueSubstitution::boolean_array(
                    position.representative_parameter,
                    values,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::NestedFixedByteArray {
                        rows,
                        ..
                    },
                ) => representative_values.push(ProofValueSubstitution::nested_fixed_byte_array(
                    position.representative_parameter,
                    rows,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::NestedBooleanArray {
                        rows,
                        ..
                    },
                ) => representative_values.push(ProofValueSubstitution::nested_boolean_array(
                    position.representative_parameter,
                    rows,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::BooleanTensor3 {
                        planes, ..
                    },
                ) => representative_values.push(ProofValueSubstitution::boolean_tensor3(
                    position.representative_parameter,
                    planes,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::RecursivePrimitiveArray {
                        elements,
                        ..
                    },
                ) => representative_values.push(ProofValueSubstitution::recursive_primitive_array(
                    position.representative_parameter,
                    elements,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::IntegerArray {
                        elements, ..
                    },
                ) => representative_values.push(ProofValueSubstitution::integer_array(
                    position.representative_parameter,
                    elements
                        .iter()
                        .map(|element| (element.spelling.clone(), element.landing)),
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::NestedIntegerArray {
                        rows,
                        ..
                    },
                ) => representative_values.push(ProofValueSubstitution::nested_integer_array(
                    position.representative_parameter,
                    rows,
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::FloatArray {
                        elements, ..
                    },
                ) => representative_values.push(ProofValueSubstitution::float_array(
                    position.representative_parameter,
                    elements
                        .iter()
                        .map(|element| (element.spelling.clone(), element.landing)),
                )),
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::NestedFloatArray {
                        rows, ..
                    },
                ) => representative_values.push(ProofValueSubstitution::nested_float_array(
                    position.representative_parameter,
                    rows,
                )),
            }
        }
        let arithmetic_bindings = arithmetic_bindings_for_application(
            program,
            representative,
            runtime,
            expected_theorem,
            &application_schema.arguments,
        );
        let public_expression_hypotheses = public
            .dependent
            .iter()
            .map(|location| {
                let ProofFact::Expression(expression) = precondition_fact_at(
                    program,
                    public_machine.contracts,
                    public_state.contracts,
                    *location,
                )?
                else {
                    return None;
                };
                Some(*expression)
            })
            .collect::<Option<Vec<_>>>();

        for (representative_position, representative_location) in
            representative_partition.dependent.iter().enumerate()
        {
            let representative_fact = precondition_fact_at(
                program,
                representative.machine_contracts,
                representative.state_contracts,
                *representative_location,
            )
            .ok_or_else(|| implication_error(application, representative_position))?;
            let exact_public = public.dependent.iter().copied().find(|location| {
                precondition_fact_at(
                    program,
                    public_machine.contracts,
                    public_state.contracts,
                    *location,
                )
                .is_some_and(|public_fact| {
                    proof_facts_match(
                        program,
                        public_fact,
                        representative_fact,
                        ProofFactIdentityContext {
                            values: &public_values,
                            static_bindings: &[],
                        },
                        ProofFactIdentityContext {
                            values: &representative_values,
                            static_bindings: &representative.static_application.bindings,
                        },
                    )
                })
            });
            let proof = if let Some(public) = exact_public {
                DirectLiftPreconditionProof::ExactMatch { public }
            } else {
                let (Some(hypotheses), ProofFact::Expression(goal)) =
                    (public_expression_hypotheses.as_deref(), representative_fact)
                else {
                    return Err(implication_error(application, representative_position));
                };
                if strict_arithmetic_expression_implication(
                    program,
                    public_machine,
                    hypotheses,
                    *goal,
                    &arithmetic_bindings,
                ) != StrictArithmeticImplicationJudgment::Proven
                {
                    return Err(implication_error(application, representative_position));
                }
                DirectLiftPreconditionProof::ArithmeticEntailment {
                    premises: public.dependent.clone(),
                }
            };
            let theorem = verified_legality_coordinate(
                expected_theorem,
                verified_theorem,
                application,
                *representative_location,
            )
            .ok_or(RelationPlanError::DirectLiftTheoremLegalityMismatch)?;
            rows.push(DirectLiftPreconditionFactRow {
                application,
                proof,
                representative: *representative_location,
                theorem,
            });
        }
    }
    Ok(DirectLiftPreconditionImplication { rows })
}

fn arithmetic_bindings_for_application(
    program: &TypedTrees,
    representative: &RepresentativeTelescope,
    runtime: &DirectLiftRuntimeCorrespondence,
    expected: &ExpectedTheoremSchema,
    theorem_arguments: &[usize],
) -> Vec<StrictArithmeticSymbolBinding> {
    let mut bindings = Vec::new();
    for ((position, representative_parameter), theorem_position) in runtime
        .positions
        .iter()
        .zip(&representative.parameters)
        .zip(theorem_arguments)
    {
        let Some(parameter) = expected.parameters.get(*theorem_position) else {
            continue;
        };
        let Some((primitive, unsigned)) =
            exact_integer_parameter(program, parameter.type_reference)
        else {
            continue;
        };
        match &position.source {
            DirectLiftArgumentSource::PublicParameter(public_symbol) => {
                let value = bindings
                    .iter()
                    .find_map(|binding: &StrictArithmeticSymbolBinding| {
                        (binding.symbol == *public_symbol).then(|| binding.value.clone())
                    })
                    .unwrap_or_else(|| StrictArithmeticBindingValue::Atom {
                        identity: format!("$theorem_parameter_{theorem_position}"),
                        unsigned,
                    });
                push_arithmetic_binding(&mut bindings, *public_symbol, value.clone());
                push_arithmetic_binding(&mut bindings, representative_parameter.symbol, value);
            }
            DirectLiftArgumentSource::Literal(
                super::runtime_correspondence::ClosedLiftLiteral::Integer { spelling, landing },
            ) if landing.domain == ArithmeticDomain::Exact
                && landed_primitive(landing.landed_type) == primitive =>
            {
                if let Some(value) = integer_spelling_value(spelling) {
                    push_arithmetic_binding(
                        &mut bindings,
                        representative_parameter.symbol,
                        StrictArithmeticBindingValue::Integer(value),
                    );
                }
            }
            _ => {}
        }
    }
    for binding in &representative.static_application.bindings {
        if binding.kind != super::RepresentativeStaticBindingKind::Const {
            continue;
        }
        let Some(value) = binding
            .argument
            .const_literal
            .as_ref()
            .and_then(|literal| literal.value_bignum())
        else {
            continue;
        };
        push_arithmetic_binding(
            &mut bindings,
            binding.parameter,
            StrictArithmeticBindingValue::Integer(value),
        );
    }
    bindings
}

fn push_arithmetic_binding(
    bindings: &mut Vec<StrictArithmeticSymbolBinding>,
    symbol: SymbolHandle,
    value: StrictArithmeticBindingValue,
) {
    if !bindings.iter().any(|binding| binding.symbol == symbol) {
        bindings.push(StrictArithmeticSymbolBinding { symbol, value });
    }
}

fn exact_integer_parameter(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<(PrimitiveType, bool)> {
    let primitive = program
        .type_reference_table
        .primitive_type(type_reference)?;
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
            | PrimitiveType::Addr
    ) || program.arithmetic_domain_for_type_reference(type_reference) != ArithmeticDomain::Exact
    {
        return None;
    }
    Some((primitive, !primitive.is_signed_integer()))
}

fn landed_primitive(landed: LandedIntegerType) -> PrimitiveType {
    match landed {
        LandedIntegerType::I8 => PrimitiveType::I8,
        LandedIntegerType::I16 => PrimitiveType::I16,
        LandedIntegerType::I32 => PrimitiveType::I32,
        LandedIntegerType::I64 => PrimitiveType::I64,
        LandedIntegerType::U8 => PrimitiveType::U8,
        LandedIntegerType::U16 => PrimitiveType::U16,
        LandedIntegerType::U32 => PrimitiveType::U32,
        LandedIntegerType::U64 => PrimitiveType::U64,
        LandedIntegerType::Addr => PrimitiveType::Addr,
    }
}

fn integer_spelling_value(spelling: &str) -> Option<psi_numerics::bignum::BigInt> {
    use psi_numerics::literals::{IntegerLiteral, IntegerRadix};

    let (negative, unsigned) = spelling
        .strip_prefix('-')
        .map_or((false, spelling), |unsigned| (true, unsigned));
    let (radix, digits) = if let Some(digits) = unsigned.strip_prefix("0b") {
        (IntegerRadix::Binary, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0o") {
        (IntegerRadix::Octal, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0x") {
        (IntegerRadix::Hexadecimal, digits)
    } else {
        (IntegerRadix::Decimal, unsigned)
    };
    IntegerLiteral::from_parts(negative, radix, digits)
        .ok()?
        .value_bignum()
}

fn implication_error(
    application: TheoremApplicationSide,
    representative_position: usize,
) -> RelationPlanError {
    match application {
        TheoremApplicationSide::Left => {
            RelationPlanError::DirectLiftLeftPreconditionNotImplied(representative_position)
        }
        TheoremApplicationSide::Right => {
            RelationPlanError::DirectLiftRightPreconditionNotImplied(representative_position)
        }
    }
}

fn verified_legality_coordinate(
    expected: &ExpectedTheoremSchema,
    verified: &VerifiedTheoremSchema,
    application: TheoremApplicationSide,
    representative: RepresentativeContractFactLocation,
) -> Option<TheoremContractFactLocation> {
    let expected_position = expected.legality_premises.iter().position(|premise| {
        premise.application == application && premise.fact == theorem_location(representative)
    })?;
    verified
        .legality_premises
        .iter()
        .find_map(|fact| (fact.expected_position == expected_position).then_some(fact.actual))
}

fn theorem_location(location: RepresentativeContractFactLocation) -> TheoremContractFactLocation {
    TheoremContractFactLocation {
        owner: match location.owner {
            RepresentativeContractOwner::Machine => TheoremContractOwner::Machine,
            RepresentativeContractOwner::State => TheoremContractOwner::State,
        },
        contract_position: location.contract_position,
        fact_position: location.fact_position,
    }
}

pub(super) fn compose_lift_correspondence_certificate(
    theorem: &Result<VerifiedTheoremSchema, RelationPlanError>,
    runtime: &DirectLiftRuntimeCorrespondence,
    precondition: &DirectLiftPreconditionImplication,
) -> Option<QuotientCorrespondenceCertificate> {
    Some(QuotientCorrespondenceCertificate {
        theorem: theorem.as_ref().ok()?.clone(),
        evidence: QuotientCorrespondenceEvidence::DirectLift {
            runtime: runtime.clone(),
            precondition: precondition.clone(),
        },
    })
}

pub(super) fn compose_define_correspondence_certificate(
    theorem: &Result<VerifiedTheoremSchema, RelationPlanError>,
    runtime: &DefineRuntimeCorrespondence,
    precondition: &DefinePreconditionCorrespondence,
) -> Option<QuotientCorrespondenceCertificate> {
    Some(QuotientCorrespondenceCertificate {
        theorem: theorem.as_ref().ok()?.clone(),
        evidence: QuotientCorrespondenceEvidence::Define {
            runtime: runtime.clone(),
            precondition: precondition.clone(),
        },
    })
}
