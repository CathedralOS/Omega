//! N6 proof quotient admission.
//!
//! A quotient declaration names an ordinary proof-only carrier family and an
//! ordinary boolean relation machine. Admission is structural: the relation
//! must have a pure checked two-carrier signature, and separately checked proof
//! machines must expose reflexivity, symmetry, and transitivity through their
//! ordinary `requires`/`ensures` contracts. No privileged law names are used.

use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::proof_only::ProofOnlyClassification;
use omega_typed_trees::signature::SignatureContractKind;
use omega_typed_trees::state::State;
use omega_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use std::collections::HashSet;

pub(crate) fn validate_quotients(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for definition in program.data_definitions() {
        let Some(quotient) = &definition.quotient else {
            continue;
        };
        let relation_name = quotient
            .relation
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");

        let Some(carrier_symbol) = base_data_symbol(program, quotient.carrier) else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` carrier `{}` must name a data type or generic data family",
                definition.name,
                program.display_type_reference_with_constraints(quotient.carrier),
            )));
            continue;
        };
        let Some(carrier) = program
            .data_definitions()
            .iter()
            .find(|candidate| candidate.symbol == carrier_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` has unknown carrier `{}`",
                definition.name,
                program.display_type_reference_with_constraints(quotient.carrier),
            )));
            continue;
        };
        if !proof_only.is_proof_only(carrier.symbol) {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` carrier `{}` has a runtime layout; quotient carriers must be proof-only",
                definition.name, carrier.name,
            )));
        }

        let Some(relation) = find_machine(program, quotient.relation_symbol) else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` names unknown equivalence relation machine `{relation_name}`",
                definition.name,
            )));
            continue;
        };
        let Some(entry) = program.machine_states(relation).first() else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` relation `{relation_name}` has no callable entry state",
                definition.name,
            )));
            continue;
        };
        let parameters = program.state_parameters(entry);
        let signature_matches = relation.attached_data.is_none()
            && !relation.boundary
            && parameters.len() == 2
            && parameters.iter().all(|parameter| {
                base_data_symbol(program, parameter.type_reference) == Some(carrier_symbol)
            })
            && program
                .type_reference_table
                .primitive_type(entry.return_type)
                == Some(PrimitiveType::Bool)
            && program.machine_effects(relation).is_empty();
        if !signature_matches {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` relation `{relation_name}` must be a free checked pure machine `(a: {}, b: {}) -> bool`",
                definition.name, carrier.name, carrier.name,
            )));
            continue;
        }

        let relation_targets: HashSet<u32> = std::iter::once(relation.symbol)
            .chain(
                program
                    .machine_states(relation)
                    .iter()
                    .map(|state| state.symbol),
            )
            .map(|symbol| symbol.arena_index())
            .collect();
        let laws = discover_equivalence_laws(program, relation, &relation_targets);
        for (present, law) in [
            (laws.reflexive, "reflexivity"),
            (laws.symmetric, "symmetry"),
            (laws.transitive, "transitivity"),
        ] {
            if !present {
                diagnostics.push(Diagnostic::error(format!(
                    "quotient data `{}` cannot admit `{relation_name}` as an equivalence: missing a structurally matching {law} proof machine",
                    definition.name,
                )));
            }
        }
    }
}

fn base_data_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
        TypeReferenceNode::Constrained { base_type, .. } => base_data_symbol(program, *base_type),
        _ => None,
    }
}

fn find_machine(program: &TypedTrees, symbol: SymbolHandle) -> Option<&Machine> {
    if !symbol.is_valid() {
        return None;
    }
    program.machines().iter().find(|machine| {
        machine.symbol == symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == symbol)
    })
}

#[derive(Default)]
struct EquivalenceLaws {
    reflexive: bool,
    symmetric: bool,
    transitive: bool,
}

fn discover_equivalence_laws(
    program: &TypedTrees,
    relation: &Machine,
    relation_targets: &HashSet<u32>,
) -> EquivalenceLaws {
    let mut laws = EquivalenceLaws::default();
    for proof in program.machines() {
        if proof.symbol == relation.symbol
            || proof.boundary
            || !program.machine_effects(proof).is_empty()
        {
            continue;
        }
        let requires = relation_pairs_in_contracts(
            program,
            proof,
            SignatureContractKind::Requires,
            relation_targets,
            relation.name.as_str(),
        );
        let ensures = relation_pairs_in_contracts(
            program,
            proof,
            SignatureContractKind::Ensures,
            relation_targets,
            relation.name.as_str(),
        );

        laws.reflexive |= ensures.iter().any(|(left, right)| left == right);
        laws.symmetric |= requires.iter().any(|(left, right)| {
            ensures
                .iter()
                .any(|(out_left, out_right)| out_left == right && out_right == left)
        });
        laws.transitive |= requires.iter().any(|(left, middle)| {
            requires.iter().any(|(next_left, right)| {
                next_left == middle
                    && ensures
                        .iter()
                        .any(|(out_left, out_right)| out_left == left && out_right == right)
            })
        });
    }
    laws
}

fn relation_pairs_in_contracts(
    program: &TypedTrees,
    machine: &Machine,
    kind: SignatureContractKind,
    relation_targets: &HashSet<u32>,
    relation_name: &str,
) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for contract in program
        .machine_contracts(machine)
        .iter()
        .filter(|contract| contract.kind == kind)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some(pair) = relation_pair(program, *expression, relation_targets, relation_name)
            {
                pairs.push(pair);
            }
        }
    }
    pairs
}

fn relation_pair(
    program: &TypedTrees,
    expression: ExpressionHandle,
    relation_targets: &HashSet<u32>,
    relation_name: &str,
) -> Option<(String, String)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            relation_call_pair(program, call, relation_targets, relation_name)
        }
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                relation_pair(program, binary.left, relation_targets, relation_name)
            } else if matches!(
                program.expression_table.expression(binary.left),
                ExpressionNode::Boolean(true)
            ) {
                relation_pair(program, binary.right, relation_targets, relation_name)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn relation_call_pair(
    program: &TypedTrees,
    call: &omega_typed_trees::expression::TableCallExpression,
    relation_targets: &HashSet<u32>,
    relation_name: &str,
) -> Option<(String, String)> {
    if !relation_targets.contains(&call.target_symbol.arena_index())
        && call.target.as_str() != relation_name
    {
        return None;
    }
    let arguments = program.expression_table.expression_handles(call.arguments);
    let [left, right] = arguments else {
        return None;
    };
    Some((
        name_operand(program, *left)?,
        name_operand(program, *right)?,
    ))
}

fn name_operand(program: &TypedTrees, expression: ExpressionHandle) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => Some(
            program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::"),
        ),
        ExpressionNode::Mutable(inner) => name_operand(program, *inner),
        _ => None,
    }
}

/// A receiver-free representative operation whose entire carrier argument
/// list has been lifted to one quotient. The candidate exists independently
/// of certification so callers can replace a generic nominal-mismatch cascade
/// with the precise missing-respect diagnostic.
pub(crate) struct QuotientLiftCandidate<'program> {
    pub(crate) quotient: &'program omega_typed_trees::data::DataDefinition,
    pub(crate) operation: &'program Machine,
    pub(crate) certified: bool,
}

pub(crate) fn quotient_lift_candidate<'program>(
    program: &'program TypedTrees,
    argument_types: &[Option<TypeReferenceHandle>],
    state: &'program State,
) -> Option<QuotientLiftCandidate<'program>> {
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.is_empty() || parameters.len() != argument_types.len() {
        return None;
    }

    let first_argument = argument_types.first().copied().flatten()?;
    let quotient = quotient_for_type(program, first_argument)?;
    let quotient_metadata = quotient.quotient.as_ref()?;
    let carrier = base_data_symbol(program, quotient_metadata.carrier)?;
    if base_data_symbol(program, state.return_type) != Some(carrier) {
        return None;
    }
    for (parameter, argument_type) in parameters.iter().zip(argument_types) {
        if base_data_symbol(program, parameter.type_reference) != Some(carrier) {
            return None;
        }
        let argument_quotient = quotient_for_type(program, (*argument_type)?)?;
        if argument_quotient.symbol != quotient.symbol {
            return None;
        }
    }

    let operation = program.machines().iter().find(|machine| {
        machine.attached_data.is_none()
            && program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
    })?;
    let certified = !operation.boundary
        && program.machine_effects(operation).is_empty()
        && operation_respects_quotient(program, operation, state, quotient);
    Some(QuotientLiftCandidate {
        quotient,
        operation,
        certified,
    })
}

fn quotient_for_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&omega_typed_trees::data::DataDefinition> {
    let symbol = base_data_symbol(program, type_reference)?;
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol && definition.quotient.is_some())
}

fn operation_respects_quotient(
    program: &TypedTrees,
    operation: &Machine,
    operation_state: &State,
    quotient: &omega_typed_trees::data::DataDefinition,
) -> bool {
    let relation = quotient
        .quotient
        .as_ref()
        .expect("quotient lift candidate has quotient metadata");
    for proof in program.machines() {
        if proof.symbol == operation.symbol
            || proof.boundary
            || !program.machine_effects(proof).is_empty()
        {
            continue;
        }
        let requires = contract_expression_handles(program, proof, SignatureContractKind::Requires);
        let ensures = contract_expression_handles(program, proof, SignatureContractKind::Ensures);
        for ensured in ensures {
            let Some(relation_call) = fact_call(program, ensured) else {
                continue;
            };
            if !call_matches_relation(program, relation_call, relation.relation_symbol) {
                continue;
            }
            let relation_arguments = program
                .expression_table
                .expression_handles(relation_call.arguments);
            let [left_result, right_result] = relation_arguments else {
                continue;
            };
            let (ExpressionNode::Call(left_call), ExpressionNode::Call(right_call)) = (
                program.expression_table.expression(*left_result),
                program.expression_table.expression(*right_result),
            ) else {
                continue;
            };
            if !call_matches_operation(left_call, operation, operation_state)
                || !call_matches_operation(right_call, operation, operation_state)
            {
                continue;
            }
            let left_arguments = program
                .expression_table
                .expression_handles(left_call.arguments);
            let right_arguments = program
                .expression_table
                .expression_handles(right_call.arguments);
            let operation_arity = program
                .state_parameters(operation_state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count();
            if left_arguments.len() != operation_arity
                || right_arguments.len() != operation_arity
                || left_arguments.is_empty()
            {
                continue;
            }
            let mut varies = false;
            let respected = left_arguments
                .iter()
                .zip(right_arguments)
                .all(|(left, right)| {
                    if program
                        .expression_table
                        .expressions_structurally_equal(*left, *right)
                    {
                        return true;
                    }
                    varies = true;
                    requires.iter().any(|required| {
                        relation_fact_matches_pair(
                            program,
                            *required,
                            relation.relation_symbol,
                            *left,
                            *right,
                        )
                    })
                });
            if respected && varies {
                return true;
            }
        }
    }
    false
}

fn contract_expression_handles(
    program: &TypedTrees,
    machine: &Machine,
    kind: SignatureContractKind,
) -> Vec<ExpressionHandle> {
    program
        .machine_contracts(machine)
        .iter()
        .filter(|contract| contract.kind == kind)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .collect()
}

fn fact_call(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<&omega_typed_trees::expression::TableCallExpression> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => Some(call),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                fact_call(program, binary.left)
            } else if matches!(
                program.expression_table.expression(binary.left),
                ExpressionNode::Boolean(true)
            ) {
                fact_call(program, binary.right)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn call_matches_relation(
    program: &TypedTrees,
    call: &omega_typed_trees::expression::TableCallExpression,
    relation_symbol: SymbolHandle,
) -> bool {
    if call.target_symbol == relation_symbol {
        return true;
    }
    find_machine(program, relation_symbol).is_some_and(|relation| {
        call.target.as_str() == relation.name.as_str()
            || program
                .machine_states(relation)
                .iter()
                .any(|state| state.symbol == call.target_symbol)
    })
}

fn call_matches_operation(
    call: &omega_typed_trees::expression::TableCallExpression,
    operation: &Machine,
    state: &State,
) -> bool {
    call.target_symbol == operation.symbol
        || call.target_symbol == state.symbol
        || call.target.as_str() == operation.name.as_str()
        || call.target.as_str() == state.name.as_str()
}

fn relation_fact_matches_pair(
    program: &TypedTrees,
    expression: ExpressionHandle,
    relation_symbol: SymbolHandle,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let Some(call) = fact_call(program, expression) else {
        return false;
    };
    if !call_matches_relation(program, call, relation_symbol) {
        return false;
    }
    matches!(
        program.expression_table.expression_handles(call.arguments),
        [required_left, required_right]
            if (program.expression_table.expressions_structurally_equal(*required_left, left)
                && program.expression_table.expressions_structurally_equal(*required_right, right))
                || (program.expression_table.expressions_structurally_equal(*required_left, right)
                    && program.expression_table.expressions_structurally_equal(*required_right, left))
    )
}
