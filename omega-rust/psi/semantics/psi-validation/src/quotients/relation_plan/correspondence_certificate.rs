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
use super::transport_schema::VerifiedForwardPreconditionTransportSchema;
use super::{PlannedQuotientTheoremEvidence, RelationPlanError, RepresentativeTelescope};
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
pub(super) enum FixedRepresentativeCallProof {
    ExactMatch {
        public: RepresentativeContractFactLocation,
    },
    ArithmeticEntailment {
        /// Complete authored fixed-Q roster, in source contract order. The
        /// strict kernel consumes this exact list and accepts only when every
        /// row is an integer expression inside its language.
        premises: Vec<RepresentativeContractFactLocation>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FixedRepresentativeCallFactRow {
    /// One proof discharges the one representative call performed at runtime.
    pub(super) proof: FixedRepresentativeCallProof,
    pub(super) representative: RepresentativeContractFactLocation,
    /// The selected theorem independently retains the same legality fact for
    /// both hypothetical representative applications. Both coordinates are
    /// retained here so replay cannot collapse or substitute either side.
    pub(super) theorem_left: TheoremContractFactLocation,
    pub(super) theorem_right: TheoremContractFactLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct FixedRepresentativeCallPreconditions {
    pub(super) rows: Vec<FixedRepresentativeCallFactRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum QuotientCorrespondenceEvidence {
    DirectLift {
        runtime: DirectLiftRuntimeCorrespondence,
        precondition: DirectLiftPreconditionImplication,
        fixed: FixedRepresentativeCallPreconditions,
    },
    DirectLiftWithTransport {
        runtime: DirectLiftRuntimeCorrespondence,
        /// Exact role-specific proof of the whole Q => P lane. Automatic
        /// implication rows are structurally absent from this variant.
        transport: VerifiedForwardPreconditionTransportSchema,
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
        let (public_values, representative_values) =
            proof_value_substitutions(runtime, |position| {
                format!(
                    "$theorem_parameter_{}",
                    application_schema.arguments[position]
                )
            });
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

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_fixed_representative_call_preconditions(
    program: &TypedTrees,
    public_machine: &Machine,
    public_state: &State,
    representative: &RepresentativeTelescope,
    public: &RepresentativePreconditionPartition,
    representative_partition: &RepresentativePreconditionPartition,
    runtime: &DirectLiftRuntimeCorrespondence,
    expected_theorem: &ExpectedTheoremSchema,
    verified_theorem: &VerifiedTheoremSchema,
) -> Result<FixedRepresentativeCallPreconditions, RelationPlanError> {
    let (public_values, representative_values) =
        proof_value_substitutions(runtime, |position| format!("$runtime_parameter_{position}"));
    let arithmetic_bindings = arithmetic_bindings_for_runtime(program, representative, runtime);
    let public_expression_hypotheses = public
        .fixed
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
    let mut rows = Vec::with_capacity(representative_partition.fixed.len());
    for (representative_position, representative_location) in
        representative_partition.fixed.iter().enumerate()
    {
        let representative_fact = precondition_fact_at(
            program,
            representative.machine_contracts,
            representative.state_contracts,
            *representative_location,
        )
        .ok_or(RelationPlanError::DirectLiftFixedPreconditionNotImplied(
            representative_position,
        ))?;
        let exact_public = public.fixed.iter().copied().find(|location| {
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
            FixedRepresentativeCallProof::ExactMatch { public }
        } else {
            let (Some(hypotheses), ProofFact::Expression(goal)) =
                (public_expression_hypotheses.as_deref(), representative_fact)
            else {
                return Err(RelationPlanError::DirectLiftFixedPreconditionNotImplied(
                    representative_position,
                ));
            };
            if strict_arithmetic_expression_implication(
                program,
                public_machine,
                hypotheses,
                *goal,
                &arithmetic_bindings,
            ) != StrictArithmeticImplicationJudgment::Proven
            {
                return Err(RelationPlanError::DirectLiftFixedPreconditionNotImplied(
                    representative_position,
                ));
            }
            FixedRepresentativeCallProof::ArithmeticEntailment {
                premises: public.fixed.clone(),
            }
        };
        let theorem_left = verified_legality_coordinate(
            expected_theorem,
            verified_theorem,
            TheoremApplicationSide::Left,
            *representative_location,
        )
        .ok_or(RelationPlanError::DirectLiftFixedTheoremLegalityMismatch)?;
        let theorem_right = verified_legality_coordinate(
            expected_theorem,
            verified_theorem,
            TheoremApplicationSide::Right,
            *representative_location,
        )
        .ok_or(RelationPlanError::DirectLiftFixedTheoremLegalityMismatch)?;
        rows.push(FixedRepresentativeCallFactRow {
            proof,
            representative: *representative_location,
            theorem_left,
            theorem_right,
        });
    }
    Ok(FixedRepresentativeCallPreconditions { rows })
}

pub(super) fn proof_value_substitutions(
    runtime: &DirectLiftRuntimeCorrespondence,
    public_identity: impl Fn(usize) -> String,
) -> (Vec<ProofValueSubstitution>, Vec<ProofValueSubstitution>) {
    let mut public_values = Vec::new();
    let mut representative_values = Vec::with_capacity(runtime.positions.len());
    for (position_index, position) in runtime.positions.iter().enumerate() {
        let value = match &position.source {
            DirectLiftArgumentSource::PublicParameter(public_parameter) => {
                let value = public_values
                    .iter()
                    .find_map(|value: &ProofValueSubstitution| {
                        (value.symbol == *public_parameter).then(|| value.clone())
                    })
                    .unwrap_or_else(|| {
                        let value = ProofValueSubstitution::symbolic(
                            *public_parameter,
                            public_identity(position_index),
                        );
                        public_values.push(value.clone());
                        value
                    });
                value.rebound(position.representative_parameter)
            }
            DirectLiftArgumentSource::Literal(literal) => {
                proof_value_for_literal(position.representative_parameter, literal)
            }
        };
        representative_values.push(value);
    }
    (public_values, representative_values)
}

fn proof_value_for_literal(
    symbol: SymbolHandle,
    literal: &super::runtime_correspondence::ClosedLiftLiteral,
) -> ProofValueSubstitution {
    use super::runtime_correspondence::ClosedLiftLiteral;

    match literal {
        ClosedLiftLiteral::Boolean(value) => ProofValueSubstitution::boolean(symbol, *value),
        ClosedLiftLiteral::Integer { spelling, landing } => {
            ProofValueSubstitution::integer(symbol, spelling, *landing)
        }
        ClosedLiftLiteral::Float { spelling, landing } => {
            ProofValueSubstitution::float(symbol, spelling, *landing)
        }
        ClosedLiftLiteral::ByteString { bytes, .. } => {
            ProofValueSubstitution::byte_string(symbol, bytes)
        }
        ClosedLiftLiteral::FixedByteArray { bytes, .. } => {
            ProofValueSubstitution::fixed_byte_array(symbol, bytes)
        }
        ClosedLiftLiteral::BooleanArray { values, .. } => {
            ProofValueSubstitution::boolean_array(symbol, values)
        }
        ClosedLiftLiteral::NestedFixedByteArray { rows, .. } => {
            ProofValueSubstitution::nested_fixed_byte_array(symbol, rows)
        }
        ClosedLiftLiteral::NestedBooleanArray { rows, .. } => {
            ProofValueSubstitution::nested_boolean_array(symbol, rows)
        }
        ClosedLiftLiteral::BooleanTensor3 { planes, .. } => {
            ProofValueSubstitution::boolean_tensor3(symbol, planes)
        }
        ClosedLiftLiteral::RecursivePrimitiveArray { elements, .. } => {
            ProofValueSubstitution::recursive_primitive_array(symbol, elements)
        }
        ClosedLiftLiteral::IntegerArray { elements, .. } => ProofValueSubstitution::integer_array(
            symbol,
            elements
                .iter()
                .map(|element| (element.spelling.clone(), element.landing)),
        ),
        ClosedLiftLiteral::NestedIntegerArray { rows, .. } => {
            ProofValueSubstitution::nested_integer_array(symbol, rows)
        }
        ClosedLiftLiteral::FloatArray { elements, .. } => ProofValueSubstitution::float_array(
            symbol,
            elements
                .iter()
                .map(|element| (element.spelling.clone(), element.landing)),
        ),
        ClosedLiftLiteral::NestedFloatArray { rows, .. } => {
            ProofValueSubstitution::nested_float_array(symbol, rows)
        }
    }
}

fn arithmetic_bindings_for_runtime(
    program: &TypedTrees,
    representative: &RepresentativeTelescope,
    runtime: &DirectLiftRuntimeCorrespondence,
) -> Vec<StrictArithmeticSymbolBinding> {
    let mut bindings = Vec::new();
    for (position_index, (position, representative_parameter)) in runtime
        .positions
        .iter()
        .zip(&representative.parameters)
        .enumerate()
    {
        let Some((primitive, unsigned)) =
            exact_integer_parameter(program, representative_parameter.type_reference)
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
                        identity: format!("$runtime_parameter_{position_index}"),
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
    append_static_integer_bindings(&mut bindings, representative);
    bindings
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
    append_static_integer_bindings(&mut bindings, representative);
    bindings
}

fn append_static_integer_bindings(
    bindings: &mut Vec<StrictArithmeticSymbolBinding>,
    representative: &RepresentativeTelescope,
) {
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
            bindings,
            binding.parameter,
            StrictArithmeticBindingValue::Integer(value),
        );
    }
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
    fixed: &FixedRepresentativeCallPreconditions,
    representative_partition: &RepresentativePreconditionPartition,
) -> Option<QuotientCorrespondenceCertificate> {
    if fixed
        .rows
        .iter()
        .map(|row| row.representative)
        .ne(representative_partition.fixed.iter().copied())
    {
        return None;
    }
    Some(QuotientCorrespondenceCertificate {
        theorem: theorem.as_ref().ok()?.clone(),
        evidence: QuotientCorrespondenceEvidence::DirectLift {
            runtime: runtime.clone(),
            precondition: precondition.clone(),
            fixed: fixed.clone(),
        },
    })
}

pub(super) fn compose_lift_transport_correspondence_certificate(
    congruence: &Result<VerifiedTheoremSchema, RelationPlanError>,
    congruence_evidence: &PlannedQuotientTheoremEvidence,
    transport: &Result<VerifiedForwardPreconditionTransportSchema, RelationPlanError>,
    transport_evidence: &PlannedQuotientTheoremEvidence,
    runtime: &DirectLiftRuntimeCorrespondence,
) -> Option<QuotientCorrespondenceCertificate> {
    let verified_congruence = congruence.as_ref().ok()?;
    let verified_transport = transport.as_ref().ok()?;
    if !theorem_evidence_is_eligible(
        congruence_evidence,
        psi_typed_trees::expression::QuotientTheoremRole::Congruence,
        verified_congruence.theorem_machine_symbol,
        verified_congruence.theorem_state_symbol,
    ) || !theorem_evidence_is_eligible(
        transport_evidence,
        verified_transport.role,
        verified_transport.theorem_machine_symbol,
        verified_transport.theorem_state_symbol,
    ) || transport_evidence.selected_application != verified_transport.selected_application
    {
        return None;
    }
    Some(QuotientCorrespondenceCertificate {
        theorem: verified_congruence.clone(),
        evidence: QuotientCorrespondenceEvidence::DirectLiftWithTransport {
            runtime: runtime.clone(),
            transport: verified_transport.clone(),
        },
    })
}

fn theorem_evidence_is_eligible(
    evidence: &PlannedQuotientTheoremEvidence,
    role: psi_typed_trees::expression::QuotientTheoremRole,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> bool {
    evidence.role == role
        && evidence.selected_application.machine_symbol == machine_symbol
        && evidence.selected_application.state_symbol == state_symbol
        && evidence.termination.is_some_and(|termination| {
            termination.machine_symbol == machine_symbol && termination.state_symbol == state_symbol
        })
        && evidence.purity.is_some_and(|purity| {
            purity.machine_symbol == machine_symbol && purity.state_symbol == state_symbol
        })
        && evidence.crash_free
}

pub(super) fn compose_define_correspondence_certificate(
    theorem: &Result<VerifiedTheoremSchema, RelationPlanError>,
    runtime: &DefineRuntimeCorrespondence,
    precondition: &DefinePreconditionCorrespondence,
    representative_partition: &RepresentativePreconditionPartition,
) -> Option<QuotientCorrespondenceCertificate> {
    if precondition
        .fixed
        .iter()
        .map(|pair| pair.representative)
        .ne(representative_partition.fixed.iter().copied())
    {
        return None;
    }
    Some(QuotientCorrespondenceCertificate {
        theorem: theorem.as_ref().ok()?.clone(),
        evidence: QuotientCorrespondenceEvidence::Define {
            runtime: runtime.clone(),
            precondition: precondition.clone(),
        },
    })
}
