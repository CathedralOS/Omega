//! Exact structural verification of one explicitly selected forward-
//! precondition transport theorem.
//!
//! The transport is authoritative for the complete public-`Q` to
//! representative-`P` lane.  This verifier therefore compares the complete
//! ordered public premise and representative conclusion rosters for both
//! representative applications.  It never calls the automatic implication
//! judgment and never combines theorem and automatic rows.

use super::correspondence_certificate::proof_value_substitutions;
use super::precondition::{
    RepresentativeContractFactLocation, RepresentativeContractOwner,
    RepresentativePreconditionPartition, precondition_fact_at,
};
use super::proof_fact_identity::{
    ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match, static_type_identities,
};
use super::theorem::SelectedTheoremTelescope;
use super::theorem_schema::{
    ExpectedTheoremSchema, TheoremApplicationSide, TheoremContractFactLocation,
    TheoremContractOwner,
};
use super::theorem_schema_verification::VerifiedTheoremParameter;
use super::{DirectLiftRuntimeCorrespondence, RelationPlanError, RepresentativeTelescope};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateParameter};
use typed_trees::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VerifiedTransportFact {
    pub(super) application: TheoremApplicationSide,
    pub(super) source: RepresentativeContractFactLocation,
    pub(super) actual: TheoremContractFactLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct VerifiedForwardPreconditionTransportSchema {
    pub(super) role: typed_trees::expression::QuotientTheoremRole,
    /// Exact selected application, including its complete closed static
    /// application. Certificate composition rejoins this value rather than
    /// treating machine/state identity alone as transport authority.
    pub(super) selected_application: SelectedTheoremTelescope,
    pub(super) theorem_machine_symbol: SymbolHandle,
    pub(super) theorem_state_symbol: SymbolHandle,
    pub(super) parameters: Vec<VerifiedTheoremParameter>,
    pub(super) public_premises: Vec<VerifiedTransportFact>,
    pub(super) representative_conclusions: Vec<VerifiedTransportFact>,
}

#[derive(Clone, Copy)]
struct LocatedFact<'a> {
    location: TheoremContractFactLocation,
    fact: &'a ProofFact,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_forward_precondition_transport_schema(
    program: &TypedTrees,
    public_machine: &Machine,
    public_state: &State,
    representative: &RepresentativeTelescope,
    theorem: &SelectedTheoremTelescope,
    runtime: &DirectLiftRuntimeCorrespondence,
    expected_congruence: &ExpectedTheoremSchema,
    public: &RepresentativePreconditionPartition,
    representative_partition: &RepresentativePreconditionPartition,
) -> Result<VerifiedForwardPreconditionTransportSchema, RelationPlanError> {
    let theorem_state = exact_theorem_state(program, theorem)?;
    let theorem_parameters = program.state_parameters(theorem_state);
    let parameters = verify_parameters(
        program,
        representative,
        theorem,
        theorem_parameters,
        expected_congruence,
    )?;
    let (requires, ensures) = collect_contract_facts(program, theorem)?;
    let public_locations = ordered_locations(public);
    let representative_locations = ordered_locations(representative_partition);
    let expected_public_count = public_locations
        .len()
        .checked_mul(2)
        .ok_or(RelationPlanError::TransportSchemaPremiseCountMismatch)?;
    let expected_representative_count = representative_locations
        .len()
        .checked_mul(2)
        .ok_or(RelationPlanError::TransportSchemaConclusionCountMismatch)?;
    if requires.len() != expected_public_count {
        return Err(RelationPlanError::TransportSchemaPremiseCountMismatch);
    }
    if ensures.len() != expected_representative_count {
        return Err(RelationPlanError::TransportSchemaConclusionCountMismatch);
    }

    let theorem_values = theorem_parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            ProofValueSubstitution::symbolic(
                parameter.symbol,
                format!("$transport_parameter_{position}"),
            )
        })
        .collect::<Vec<_>>();
    let mut public_premises = Vec::with_capacity(requires.len());
    let mut representative_conclusions = Vec::with_capacity(ensures.len());
    let side_values = [TheoremApplicationSide::Left, TheoremApplicationSide::Right]
        .into_iter()
        .map(|application| {
            let application_schema = match application {
                TheoremApplicationSide::Left => &expected_congruence.left_application,
                TheoremApplicationSide::Right => &expected_congruence.right_application,
            };
            if runtime.positions.len() != application_schema.arguments.len() {
                return Err(RelationPlanError::DirectLiftRuntimeArityMismatch);
            }
            let values = proof_value_substitutions(runtime, |position| {
                format!(
                    "$transport_parameter_{}",
                    application_schema.arguments[position]
                )
            });
            Ok((application, values))
        })
        .collect::<Result<Vec<_>, RelationPlanError>>()?;

    // Canonical order is source-fact-major, with the Left and Right
    // applications adjacent for every authored fact. This matches the
    // established congruence legality roster and prevents a producer and
    // consumer from choosing different multi-fact orderings.
    let mut premise_position = 0;
    for source in &public_locations {
        for (application, (public_values, _)) in &side_values {
            let expected = precondition_fact_at(
                program,
                public_machine.contracts,
                public_state.contracts,
                *source,
            )
            .ok_or(RelationPlanError::TransportSchemaPublicPremiseMismatch(
                premise_position,
            ))?;
            let actual = requires[premise_position];
            if !proof_facts_match(
                program,
                expected,
                actual.fact,
                ProofFactIdentityContext {
                    values: public_values,
                    static_bindings: &[],
                },
                ProofFactIdentityContext {
                    values: &theorem_values,
                    static_bindings: &theorem.static_application.bindings,
                },
            ) {
                return Err(RelationPlanError::TransportSchemaPublicPremiseMismatch(
                    premise_position,
                ));
            }
            public_premises.push(VerifiedTransportFact {
                application: *application,
                source: *source,
                actual: actual.location,
            });
            premise_position += 1;
        }
    }
    let mut conclusion_position = 0;
    for source in &representative_locations {
        for (application, (_, representative_values)) in &side_values {
            let expected = precondition_fact_at(
                program,
                representative.machine_contracts,
                representative.state_contracts,
                *source,
            )
            .ok_or(
                RelationPlanError::TransportSchemaRepresentativeConclusionMismatch(
                    conclusion_position,
                ),
            )?;
            let actual = ensures[conclusion_position];
            if !proof_facts_match(
                program,
                expected,
                actual.fact,
                ProofFactIdentityContext {
                    values: representative_values,
                    static_bindings: &representative.static_application.bindings,
                },
                ProofFactIdentityContext {
                    values: &theorem_values,
                    static_bindings: &theorem.static_application.bindings,
                },
            ) {
                return Err(
                    RelationPlanError::TransportSchemaRepresentativeConclusionMismatch(
                        conclusion_position,
                    ),
                );
            }
            representative_conclusions.push(VerifiedTransportFact {
                application: *application,
                source: *source,
                actual: actual.location,
            });
            conclusion_position += 1;
        }
    }

    Ok(VerifiedForwardPreconditionTransportSchema {
        role: typed_trees::expression::QuotientTheoremRole::ForwardPreconditionTransport,
        selected_application: theorem.clone(),
        theorem_machine_symbol: theorem.machine_symbol,
        theorem_state_symbol: theorem.state_symbol,
        parameters,
        public_premises,
        representative_conclusions,
    })
}

fn exact_theorem_state<'a>(
    program: &'a TypedTrees,
    theorem: &SelectedTheoremTelescope,
) -> Result<&'a State, RelationPlanError> {
    let mut matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == theorem.machine_symbol)
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.symbol == theorem.state_symbol);
    let Some(state) = matches.next() else {
        return Err(RelationPlanError::TheoremEntryDoesNotResolveExactly);
    };
    if matches.next().is_some() {
        return Err(RelationPlanError::TheoremEntryDoesNotResolveExactly);
    }
    Ok(state)
}

fn verify_parameters(
    program: &TypedTrees,
    representative: &RepresentativeTelescope,
    theorem: &SelectedTheoremTelescope,
    theorem_parameters: &[StateParameter],
    expected: &ExpectedTheoremSchema,
) -> Result<Vec<VerifiedTheoremParameter>, RelationPlanError> {
    if theorem_parameters.len() != expected.parameters.len() {
        return Err(RelationPlanError::TheoremSchemaParameterArityMismatch);
    }
    let representative_type_bindings = static_type_identities(&representative.static_application);
    let theorem_type_bindings = static_type_identities(&theorem.static_application);
    theorem_parameters
        .iter()
        .zip(&expected.parameters)
        .enumerate()
        .map(|(position, (actual, expected))| {
            if actual.is_const {
                return Err(RelationPlanError::TheoremSchemaConstParameter(position));
            }
            if actual.is_self {
                return Err(RelationPlanError::TheoremSchemaAttachedReceiver(position));
            }
            if actual.is_mutable != expected.is_mutable {
                return Err(RelationPlanError::TheoremSchemaParameterModeMismatch(
                    position,
                ));
            }
            let expected_identity = program.normalized_type_identity_with_binders(
                expected.type_reference,
                &representative_type_bindings,
            );
            let actual_identity = program.normalized_type_identity_with_binders(
                actual.type_reference,
                &theorem_type_bindings,
            );
            if expected_identity != actual_identity {
                return Err(RelationPlanError::TheoremSchemaParameterTypeMismatch(
                    position,
                ));
            }
            Ok(VerifiedTheoremParameter {
                expected_position: position,
                theorem_symbol: actual.symbol,
            })
        })
        .collect()
}

fn collect_contract_facts<'a>(
    program: &'a TypedTrees,
    theorem: &SelectedTheoremTelescope,
) -> Result<(Vec<LocatedFact<'a>>, Vec<LocatedFact<'a>>), RelationPlanError> {
    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    for (owner, span) in [
        (TheoremContractOwner::Machine, theorem.machine_contracts),
        (TheoremContractOwner::State, theorem.state_contracts),
    ] {
        for (contract_position, contract) in program
            .signature_contracts
            .span_or_empty(span)
            .iter()
            .enumerate()
        {
            if contract.binding.is_some() {
                return Err(RelationPlanError::TheoremSchemaNamedEvidenceLane);
            }
            let destination = match contract.kind {
                SignatureContractKind::Requires => &mut requires,
                SignatureContractKind::Ensures => &mut ensures,
                SignatureContractKind::EnsuresForResultCase { .. }
                | SignatureContractKind::Crashes { .. } => {
                    return Err(RelationPlanError::TheoremSchemaUnexpectedContractKind);
                }
            };
            for (fact_position, fact) in program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .enumerate()
            {
                destination.push(LocatedFact {
                    location: TheoremContractFactLocation {
                        owner,
                        contract_position,
                        fact_position,
                    },
                    fact,
                });
            }
        }
    }
    Ok((requires, ensures))
}

fn ordered_locations(
    partition: &RepresentativePreconditionPartition,
) -> Vec<RepresentativeContractFactLocation> {
    let mut locations = Vec::with_capacity(partition.dependent.len() + partition.fixed.len());
    locations.extend_from_slice(&partition.dependent);
    locations.extend_from_slice(&partition.fixed);
    locations.sort_by_key(|location| {
        (
            match location.owner {
                RepresentativeContractOwner::Machine => 0,
                RepresentativeContractOwner::State => 1,
            },
            location.contract_position,
            location.fact_position,
        )
    });
    locations
}
