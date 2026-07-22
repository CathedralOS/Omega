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
            && program.type_reference_table.primitive_type(entry.return_type)
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
            .chain(program.machine_states(relation).iter().map(|state| state.symbol))
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

fn base_data_symbol(program: &TypedTrees, type_reference: TypeReferenceHandle) -> Option<SymbolHandle> {
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
        if proof.symbol == relation.symbol {
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
            if let Some(pair) = relation_pair(program, *expression, relation_targets, relation_name) {
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
            if matches!(program.expression_table.expression(binary.right), ExpressionNode::Boolean(true)) {
                relation_pair(program, binary.left, relation_targets, relation_name)
            } else if matches!(program.expression_table.expression(binary.left), ExpressionNode::Boolean(true)) {
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
    Some((name_operand(program, *left)?, name_operand(program, *right)?))
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
