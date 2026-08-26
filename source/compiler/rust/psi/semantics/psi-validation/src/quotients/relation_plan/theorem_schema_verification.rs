//! Exact structural verification of one explicitly selected quotient theorem.
//!
//! This module performs no theorem discovery and proves no implication. It
//! pairs the compiler-derived schema with exact checked-tree coordinates. The
//! resulting certificate is necessary, but deliberately insufficient, for the
//! later quotient lifting and terminal replay stages.

use super::proof_fact_identity::{
    ProofFactIdentityContext, proof_facts_match, static_arguments_match, static_type_identities,
};
use super::theorem::SelectedTheoremTelescope;
use super::theorem_schema::{
    ExpectedTheoremSchema, TheoremApplicationSide, TheoremContractFactLocation,
    TheoremContractOwner, TheoremLegalityPremise, TheoremRepresentativeApplication,
};
use super::{RelationPlanError, RepresentativeTelescope};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::signature::{SignatureContractKind, StateParameter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VerifiedTheoremParameter {
    pub(super) expected_position: usize,
    pub(super) theorem_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VerifiedTheoremFact {
    pub(super) expected_position: usize,
    pub(super) actual: TheoremContractFactLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct VerifiedTheoremSchema {
    pub(super) theorem_machine_symbol: SymbolHandle,
    pub(super) theorem_state_symbol: SymbolHandle,
    pub(super) parameters: Vec<VerifiedTheoremParameter>,
    pub(super) relation_premises: Vec<VerifiedTheoremFact>,
    pub(super) legality_premises: Vec<VerifiedTheoremFact>,
    pub(super) conclusion: TheoremContractFactLocation,
}

#[derive(Clone, Copy)]
struct LocatedFact<'a> {
    location: TheoremContractFactLocation,
    fact: &'a ProofFact,
}

pub(super) fn verify_selected_theorem_schema(
    program: &TypedTrees,
    representative: &RepresentativeTelescope,
    theorem: &SelectedTheoremTelescope,
    expected: &ExpectedTheoremSchema,
) -> Result<VerifiedTheoremSchema, RelationPlanError> {
    let theorem_state = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == theorem.machine_symbol)
        .flat_map(|machine| program.machine_states(machine))
        .find(|state| state.symbol == theorem.state_symbol)
        .ok_or(RelationPlanError::TheoremEntryDoesNotResolveExactly)?;
    let theorem_parameters = program.state_parameters(theorem_state);
    if theorem_parameters.len() != expected.parameters.len() {
        return Err(RelationPlanError::TheoremSchemaParameterArityMismatch);
    }

    let representative_type_bindings = static_type_identities(&representative.static_application);
    let theorem_type_bindings = static_type_identities(&theorem.static_application);
    let mut verified_parameters = Vec::with_capacity(theorem_parameters.len());
    for (position, (actual, expected)) in theorem_parameters
        .iter()
        .zip(&expected.parameters)
        .enumerate()
    {
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
        let actual_identity = program
            .normalized_type_identity_with_binders(actual.type_reference, &theorem_type_bindings);
        if expected_identity != actual_identity {
            return Err(RelationPlanError::TheoremSchemaParameterTypeMismatch(
                position,
            ));
        }
        verified_parameters.push(VerifiedTheoremParameter {
            expected_position: position,
            theorem_symbol: actual.symbol,
        });
    }

    let (mut requires, ensures) = collect_contract_facts(program, theorem)?;
    if requires.len() != expected.relation_premises.len() + expected.legality_premises.len() {
        return Err(RelationPlanError::TheoremSchemaPremiseCountMismatch);
    }
    if ensures.len() != 1 {
        return Err(RelationPlanError::TheoremSchemaConclusionCountMismatch);
    }

    let mut relation_premises = Vec::with_capacity(expected.relation_premises.len());
    for (position, premise) in expected.relation_premises.iter().enumerate() {
        let Some(relation) = program
            .propositions()
            .iter()
            .find(|candidate| candidate.symbol == premise.relation.relation_symbol)
        else {
            return Err(RelationPlanError::TheoremSchemaRelationPremiseMismatch(
                position,
            ));
        };
        let left = &theorem_parameters[premise.left_parameter];
        let right = &theorem_parameters[premise.right_parameter];
        let Some(found) = requires.iter().position(|actual| {
            super::super::fact_is_exact_relation_pair(
                program,
                actual.fact,
                relation,
                left,
                right,
                theorem_parameters,
            )
        }) else {
            return Err(RelationPlanError::TheoremSchemaRelationPremiseMismatch(
                position,
            ));
        };
        let actual = requires.remove(found).location;
        relation_premises.push(VerifiedTheoremFact {
            expected_position: position,
            actual,
        });
    }

    let mut legality_premises = Vec::with_capacity(expected.legality_premises.len());
    for (position, premise) in expected.legality_premises.iter().enumerate() {
        let representative_fact = representative_fact(program, representative, premise).ok_or(
            RelationPlanError::TheoremSchemaLegalityPremiseMismatch(position),
        )?;
        let representative_parameters = &representative.parameters;
        let application = match premise.application {
            TheoremApplicationSide::Left => &expected.left_application,
            TheoremApplicationSide::Right => &expected.right_application,
        };
        let representative_values = representative_parameters
            .iter()
            .zip(&application.arguments)
            .map(|(parameter, theorem_position)| {
                (
                    parameter.symbol,
                    format!("$theorem_parameter_{theorem_position}"),
                )
            })
            .collect::<Vec<_>>();
        let theorem_values = theorem_parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                (parameter.symbol, format!("$theorem_parameter_{position}"))
            })
            .collect::<Vec<_>>();
        let Some(found) = requires.iter().position(|actual| {
            proof_facts_match(
                program,
                representative_fact,
                actual.fact,
                ProofFactIdentityContext {
                    values: &representative_values,
                    static_bindings: &representative.static_application.bindings,
                },
                ProofFactIdentityContext {
                    values: &theorem_values,
                    static_bindings: &theorem.static_application.bindings,
                },
            )
        }) else {
            return Err(RelationPlanError::TheoremSchemaLegalityPremiseMismatch(
                position,
            ));
        };
        let actual = requires.remove(found).location;
        legality_premises.push(VerifiedTheoremFact {
            expected_position: position,
            actual,
        });
    }
    if !requires.is_empty() {
        return Err(RelationPlanError::TheoremSchemaPremiseCountMismatch);
    }

    let conclusion = ensures[0];
    if !conclusion_matches(
        program,
        conclusion.fact,
        representative,
        theorem_parameters,
        expected,
    ) {
        return Err(RelationPlanError::TheoremSchemaConclusionMismatch);
    }

    Ok(VerifiedTheoremSchema {
        theorem_machine_symbol: theorem.machine_symbol,
        theorem_state_symbol: theorem.state_symbol,
        parameters: verified_parameters,
        relation_premises,
        legality_premises,
        conclusion: conclusion.location,
    })
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

fn representative_fact<'a>(
    program: &'a TypedTrees,
    representative: &RepresentativeTelescope,
    premise: &TheoremLegalityPremise,
) -> Option<&'a ProofFact> {
    let span = match premise.fact.owner {
        TheoremContractOwner::Machine => representative.machine_contracts,
        TheoremContractOwner::State => representative.state_contracts,
    };
    let contract = program
        .signature_contracts
        .span_or_empty(span)
        .get(premise.fact.contract_position)?;
    (contract.kind == SignatureContractKind::Requires)
        .then(|| {
            program
                .proof_facts
                .span_or_empty(contract.facts)
                .get(premise.fact.fact_position)
        })
        .flatten()
}

fn conclusion_matches(
    program: &TypedTrees,
    actual: &ProofFact,
    representative: &RepresentativeTelescope,
    theorem_parameters: &[StateParameter],
    expected: &ExpectedTheoremSchema,
) -> bool {
    let ProofFact::Proposition(application) = actual else {
        return false;
    };
    if application.proposition != expected.result_relation.relation_symbol
        || !application.binder_arguments.is_empty()
    {
        return false;
    }
    let arguments = program
        .expression_table
        .expression_handles(application.arguments);
    matches!(arguments, [left, right]
    if representative_application_matches(
        program,
        *left,
        representative,
        theorem_parameters,
        &expected.left_application,
    ) && representative_application_matches(
        program,
        *right,
        representative,
        theorem_parameters,
        &expected.right_application,
    ))
}

fn representative_application_matches(
    program: &TypedTrees,
    expression: ExpressionHandle,
    representative: &RepresentativeTelescope,
    theorem_parameters: &[StateParameter],
    expected: &TheoremRepresentativeApplication,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    if expected.machine_symbol != representative.machine_symbol
        || expected.state_symbol != representative.state_symbol
        || call.target_symbol != expected.state_symbol
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
        || !call.evidence_arguments.is_empty()
        || call.machine_arguments.len() != representative.static_application.bindings.len()
        || !call
            .machine_arguments
            .iter()
            .zip(&representative.static_application.bindings)
            .all(|(actual, expected)| static_arguments_match(actual, &expected.argument))
    {
        return false;
    }
    let mut expected_arguments = expected.arguments.iter().copied();
    let receiver_matches = representative
        .parameters
        .first()
        .is_some_and(|parameter| parameter.is_self);
    if receiver_matches {
        let Some(position) = expected_arguments.next() else {
            return false;
        };
        if !expression_is_parameter(program, call.receiver, theorem_parameters, position) {
            return false;
        }
    } else if call.receiver.is_valid() {
        return false;
    }
    let arguments = program.expression_table.expression_handles(call.arguments);
    arguments.len() == expected_arguments.len()
        && arguments
            .iter()
            .zip(expected_arguments)
            .all(|(actual, position)| {
                expression_is_parameter(program, *actual, theorem_parameters, position)
            })
}

fn expression_is_parameter(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    position: usize,
) -> bool {
    let Some(parameter) = parameters.get(position) else {
        return false;
    };
    matches!(program.expression_table.expression(expression),
        ExpressionNode::Name(path) if parameter.symbol.is_valid() && path.symbol == parameter.symbol)
}
