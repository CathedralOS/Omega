//! Non-executable composition of direct quotient correspondence evidence.
//!
//! The lift rung here is intentionally bounded to exact direct public
//! arguments, including omission, permutation, repeated occurrences, and
//! exact closed scalar literal substitution, plus structural fact inclusion.
//! It is not the general implication or adapted-argument judgment.

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
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DirectLiftPreconditionFactRow {
    pub(super) application: TheoremApplicationSide,
    pub(super) public: RepresentativeContractFactLocation,
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
            }
        }

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
            let Some(public_location) = public.dependent.iter().copied().find(|location| {
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
            }) else {
                return Err(implication_error(application, representative_position));
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
                public: public_location,
                representative: *representative_location,
                theorem,
            });
        }
    }
    Ok(DirectLiftPreconditionImplication { rows })
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
