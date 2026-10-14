//! Exact structural verification of one explicitly selected quotient theorem.
//!
//! This module performs no theorem discovery and proves no implication. It
//! pairs the compiler-derived schema with exact checked-tree coordinates. The
//! resulting certificate is necessary, but deliberately insufficient, for the
//! later quotient lifting and terminal replay stages.

use super::proof_fact_identity::{
    ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match, static_arguments_match,
    static_type_identities,
};
use super::theorem::SelectedTheoremTelescope;
use super::theorem_schema::{
    ContractFactLocation, ContractOwner, ExpectedTheoremSchema, TheoremApplicationSide,
    TheoremLegalityPremise, TheoremRepresentativeApplication, collect_contract_facts,
};
use super::{PositionalRelation, RelationPlanError, RepresentativeTelescope};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::{SignatureContractKind, StateParameter};
use typed_trees::type_identity::TypeIdentityRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VerifiedTheoremParameter {
    pub(super) expected_position: usize,
    pub(super) theorem_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VerifiedTheoremFact {
    pub(super) expected_position: usize,
    pub(super) actual: ContractFactLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) struct VerifiedTheoremSchema {
    pub(super) theorem_machine_symbol: SymbolHandle,
    pub(super) theorem_state_symbol: SymbolHandle,
    pub(super) parameters: Vec<VerifiedTheoremParameter>,
    pub(super) relation_premises: Vec<VerifiedTheoremFact>,
    pub(super) legality_premises: Vec<VerifiedTheoremFact>,
    pub(super) conclusion: ContractFactLocation,
}

pub(super) fn verify_selected_theorem_schema(
    program: &dyn crate::CheckingInput,
    representative: &RepresentativeTelescope,
    theorem: &SelectedTheoremTelescope,
    expected: &ExpectedTheoremSchema,
) -> Result<VerifiedTheoremSchema, RelationPlanError> {
    let theorem_state =
        typed_trees::machine::machine_by_symbol(program.machines(), theorem.machine_symbol)
            .and_then(|machine| {
                typed_trees::state::state_by_symbol(
                    program.machine_states(machine),
                    theorem.state_symbol,
                )
            })
            .ok_or(RelationPlanError::TheoremEntryDoesNotResolveExactly)?;
    let theorem_parameters = program.state_parameters(theorem_state);
    let verified_parameters = verify_parameters(
        program,
        representative,
        theorem,
        theorem_parameters,
        expected,
    )?;

    let (mut requires, ensures) = collect_contract_facts(program, theorem)?;
    if requires.len() != expected.relation_premises.len() + expected.legality_premises.len() {
        return Err(RelationPlanError::TheoremSchemaPremiseCountMismatch);
    }
    if ensures.len() != 1 {
        return Err(RelationPlanError::TheoremSchemaConclusionCountMismatch);
    }

    let mut relation_premises = Vec::with_capacity(expected.relation_premises.len());
    for (position, premise) in expected.relation_premises.iter().enumerate() {
        let Some(relation) =
            program.proposition_definition_by_symbol(premise.relation.relation_symbol)
        else {
            return Err(RelationPlanError::TheoremSchemaRelationPremiseMismatch(
                position,
            ));
        };
        let left = &theorem_parameters[premise.left_parameter];
        let right = &theorem_parameters[premise.right_parameter];
        let Some(found) = requires.iter().position(|actual| {
            crate::proof_contracts::quotients::equivalence_selection::fact_is_exact_relation_pair(
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
                ProofValueSubstitution::symbolic(
                    parameter.symbol,
                    format!("$theorem_parameter_{theorem_position}"),
                )
            })
            .collect::<Vec<_>>();
        let theorem_values = theorem_parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                ProofValueSubstitution::symbolic(
                    parameter.symbol,
                    format!("$theorem_parameter_{position}"),
                )
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
fn representative_fact<'a>(
    program: &'a dyn crate::CheckingInput,
    representative: &RepresentativeTelescope,
    premise: &TheoremLegalityPremise,
) -> Option<&'a ProofFact> {
    let span = match premise.fact.owner {
        ContractOwner::Machine => representative.machine_contracts,
        ContractOwner::State => representative.state_contracts,
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
    program: &dyn crate::CheckingInput,
    actual: &ProofFact,
    representative: &RepresentativeTelescope,
    theorem_parameters: &[StateParameter],
    expected: &ExpectedTheoremSchema,
) -> bool {
    let arguments = match expected.result_relation {
        PositionalRelation::Quotient(relation) => {
            let Some(application) = actual.as_proposition() else {
                return false;
            };
            if application.proposition != relation.relation_symbol
                || !application.binder_arguments.is_empty()
            {
                return false;
            }
            let [left, right] = program
                .expression_table
                .expression_handles(application.arguments)
            else {
                return false;
            };
            [*left, *right]
        }
        PositionalRelation::ExactEquality(_) => {
            let Some(expression) = actual.as_expression() else {
                return false;
            };
            let Some(binary) = program.expression_table.binary_node(*expression) else {
                return false;
            };
            if binary.operator != typed_trees::expression::BinaryOperator::Equal {
                return false;
            }
            [binary.left, binary.right]
        }
    };
    matches!(&arguments, [left, right]
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    representative: &RepresentativeTelescope,
    theorem_parameters: &[StateParameter],
    expected: &TheoremRepresentativeApplication,
) -> bool {
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    // The theorem's application must be the representative reached through
    // its ordinary nominal route: a quotient request, a private layout
    // request, or static conformance dispatch (whose rewritten target may
    // equal the representative state while the spelled operation is a public
    // requirement) is a redirected call and rejects.
    if expected.machine_symbol != representative.machine_symbol
        || expected.state_symbol != representative.state_symbol
        || call.target_symbol != expected.state_symbol
        || !call.selects_only_nominal_route()
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
    let expected_arguments = expected.arguments.iter().copied();
    if call.receiver.is_valid() {
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
    program: &dyn crate::CheckingInput,
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

pub(super) fn verify_parameters(
    program: &dyn crate::CheckingInput,
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
            if actual.is_mutable != expected.is_mutable {
                return Err(RelationPlanError::TheoremSchemaParameterModeMismatch(
                    position,
                ));
            }
            let expected_identity = program.type_identity(TypeIdentityRequest {
                binders: &representative_type_bindings,
                ..TypeIdentityRequest::ordinary(expected.type_reference)
            });
            let actual_identity = program.type_identity(TypeIdentityRequest {
                binders: &theorem_type_bindings,
                ..TypeIdentityRequest::ordinary(actual.type_reference)
            });
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
