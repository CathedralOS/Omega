//! N6 proof quotient migration boundary.
//!
//! The declaration validator below is the legacy pilot: it still accepts an
//! ordinary boolean relation and structurally discovers reflexivity, symmetry,
//! and transitivity contracts. That is not the settled quotient-formation
//! authority, which must select one exact named `Equivalence` conformance from
//! the declaration's static `where` surface.
//!
//! Sealed `Quotient::lift`/`define` requests retain their exact source-selected
//! operation and conformance identities, but this module rejects executable
//! admission until formation, correspondence, and contract obligations are
//! checked. Bare representative calls never discover structural respect proof
//! machines and never acquire lift authority.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::proof_only::ProofOnlyClassification;
use psi_typed_trees::signature::SignatureContractKind;
use psi_typed_trees::state::State;
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use std::collections::HashSet;

pub(crate) fn validate_quotients(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, expression) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        if let Some(request) = &call.quotient_operation {
            let operation = match request.kind {
                psi_typed_trees::expression::QuotientOperationKind::Lift => "lift",
                psi_typed_trees::expression::QuotientOperationKind::Define => "define",
            };
            diagnostics.push(Diagnostic::error(format!(
                "`Quotient::{operation}` retains its exact representative operation and named conformance, but executable quotient operations are not admitted until quotient formation, correspondence, and result-flow obligations are independently checked",
            )));
        }
    }

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
            && relation.supply_mode.is_checked_body()
            && parameters.len() == 2
            && parameters.iter().all(|parameter| {
                base_data_symbol(program, parameter.type_reference) == Some(carrier_symbol)
            })
            && program
                .type_reference_table
                .primitive_type(entry.return_type)
                == Some(PrimitiveType::Bool)
            && authored_behavior_is_empty(program, relation);
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
        TypeReferenceNode::Reference { referee, .. } => base_data_symbol(program, *referee),
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
            || !proof.supply_mode.is_checked_body()
            || !authored_behavior_is_empty(program, proof)
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
    call: &psi_typed_trees::expression::TableCallExpression,
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

/// A bare representative call whose operands happen to be quotient values.
/// This shape is retained only to replace generic nominal mismatch cascades
/// with the settled explicit-wrapper diagnostic. It carries no admission.
pub(crate) struct LegacyQuotientCallCandidate<'program> {
    pub(crate) quotient: &'program psi_typed_trees::data::DataDefinition,
    pub(crate) operation: &'program Machine,
}

pub(crate) fn legacy_quotient_call_candidate<'program>(
    program: &'program TypedTrees,
    receiver_type: Option<TypeReferenceHandle>,
    argument_types: &[Option<TypeReferenceHandle>],
    state: &'program State,
) -> Option<LegacyQuotientCallCandidate<'program>> {
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != argument_types.len() {
        return None;
    }

    let operation = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
    })?;
    let is_attached = operation.attached_data.is_some();
    if is_attached != receiver_type.is_some() {
        return None;
    }

    let first_operand = receiver_type.or_else(|| argument_types.first().copied().flatten())?;
    let quotient = quotient_for_type(program, first_operand)?;
    let quotient_metadata = quotient.quotient.as_ref()?;
    let carrier = base_data_symbol(program, quotient_metadata.carrier)?;
    if base_data_symbol(program, state.return_type) != Some(carrier) {
        return None;
    }
    if let Some(receiver_type) = receiver_type {
        if quotient_for_type(program, receiver_type)?.symbol != quotient.symbol {
            return None;
        }
        let attached_carrier = operation.attached_data.as_ref().and_then(|attached| {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == attached.as_str())
        })?;
        if attached_carrier.symbol != carrier {
            return None;
        }
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

    Some(LegacyQuotientCallCandidate {
        quotient,
        operation,
    })
}

/// Identify a bare attached representative call solely for a precise
/// migration diagnostic. This does not resolve the call, validate arguments,
/// inspect proof machines, or grant any lift authority.
pub(crate) fn legacy_attached_quotient_call_candidate<'program>(
    program: &'program TypedTrees,
    receiver_type: TypeReferenceHandle,
    target: &str,
) -> Option<LegacyQuotientCallCandidate<'program>> {
    let quotient = quotient_for_type(program, receiver_type)?;
    let carrier_symbol = base_data_symbol(program, quotient.quotient.as_ref()?.carrier)?;
    let carrier = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == carrier_symbol)?;
    let operation = program.machines().iter().find(|machine| {
        machine
            .attached_data
            .as_ref()
            .is_some_and(|attached| attached.as_str() == carrier.name.as_str())
            && program
                .machine_states(machine)
                .iter()
                .any(|state| state.name.as_str() == target)
    })?;
    Some(LegacyQuotientCallCandidate {
        quotient,
        operation,
    })
}

fn quotient_for_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&psi_typed_trees::data::DataDefinition> {
    let symbol = base_data_symbol(program, type_reference)?;
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol && definition.quotient.is_some())
}

#[cfg(test)]
mod tests {
    use super::validate_quotients;
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::expression::{
        ExpressionHandle, ExpressionNode, QuotientOperationKind, QuotientOperationRequest,
        StaticMachineArgument, TableCallExpression,
    };
    use psi_typed_trees::name::Identifier;

    fn static_argument(name: &'static str) -> StaticMachineArgument {
        StaticMachineArgument {
            path: vec![Identifier::generated_static(name)].into_boxed_slice(),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: SymbolHandle::invalid(),
        }
    }

    #[test]
    fn retained_sealed_request_is_not_executable_admission() {
        let mut program = TypedTrees::default();
        let arguments = program
            .expression_table
            .insert_expression_handles(std::iter::empty());
        program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol: SymbolHandle::invalid(),
                target: Identifier::generated_static("lift"),
                machine_arguments: Box::default(),
                quotient_operation: Some(QuotientOperationRequest {
                    kind: QuotientOperationKind::Lift,
                    representative_operation: static_argument("representative"),
                    respect_conformance: static_argument("ExactRespect"),
                }),
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: Default::default(),
            }));
        let proof_only = psi_typed_trees::proof_only::classify(&program);
        let mut diagnostics = Vec::new();

        validate_quotients(&program, &proof_only, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("executable quotient operations are not admitted")
        );
    }
}

fn authored_behavior_is_empty(program: &TypedTrees, machine: &Machine) -> bool {
    program
        .service_reach_rows
        .services(machine.service_reach_row)
        .is_empty()
        && !machine.suspends
        && !machine.blocks
}
