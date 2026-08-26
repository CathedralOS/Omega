//! Source-visible content-conservation contracts.
//!
//! The surface is deliberately closed: exact owner-unique projection calls,
//! proof-only `old(place)`, compiler-owned `separate(...)`, and equality.
//! This module normalizes authored equations for checked facts and semantic
//! identity; it does not infer sealed introductions or custody exits.

mod normalization;

use normalization::{NormalizationContext, normalize_equation};

use psi_diagnostics::Diagnostic;
use psi_language_semantics::content::{
    ContentAlgebraIdentity, ContentConservationOwnerKind, ContentConservationPlan,
    ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion, ContentProjectionPlan,
    ContentStructuralPlace, conservation_fingerprint,
};
use psi_symbols::{BuiltinFunction, SymbolHandle};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use psi_typed_trees::types::TypeReferenceHandle;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentConservationSourcePlan {
    pub source_expression: ExpressionHandle,
    pub plan: ContentConservationPlan,
}

pub(crate) fn validate_content_conservation_contracts(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let projections = crate::content_projections::build_content_projection_plans(program);
    let _ = collect_content_conservation_plans(program, &projections, diagnostics);
}

pub fn build_content_conservation_plans(
    program: &TypedTrees,
) -> Vec<ContentConservationSourcePlan> {
    let projections = crate::content_projections::build_content_projection_plans(program);
    let mut diagnostics = Vec::new();
    collect_content_conservation_plans(program, &projections, &mut diagnostics)
}

fn collect_content_conservation_plans(
    program: &TypedTrees,
    projections: &[ContentProjectionPlan],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ContentConservationSourcePlan> {
    let mut plans = Vec::new();
    let mut proof_nodes = HashSet::new();

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            let contracts = program
                .state_signature_contracts(signature)
                .iter()
                .collect::<Vec<_>>();
            collect_callable_plans(
                program,
                projections,
                ContentConservationOwnerKind::TraitRequirement,
                trait_definition.symbol,
                signature.symbol,
                &format!("{}::{}", trait_definition.name, signature.name),
                program.state_signature_parameters(signature),
                signature.return_type,
                SymbolHandle::invalid(),
                &contracts,
                &mut proof_nodes,
                &mut plans,
                diagnostics,
            );
        }
    }

    for machine in program.machines() {
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            let mut contracts = program.state_contracts(state).iter().collect::<Vec<_>>();
            if state_index == 0 {
                contracts.extend(program.machine_contracts(machine));
            }
            let label = if state_index == 0 {
                machine.name.to_string()
            } else {
                format!("{}::{}", machine.name, state.name)
            };
            collect_callable_plans(
                program,
                projections,
                ContentConservationOwnerKind::Machine,
                machine.symbol,
                state.symbol,
                &label,
                program.state_parameters(state),
                state.return_type,
                machine.attached_data_symbol,
                &contracts,
                &mut proof_nodes,
                &mut plans,
                diagnostics,
            );
        }
    }

    // These calls erase completely. Any occurrence outside a proof fact is a
    // runtime-use attempt, including a direct call to a projection machine.
    for (handle, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        if proof_nodes.contains(&(handle.arena_index(), handle.generation())) {
            continue;
        }
        if is_old_call(program, call)
            || is_separate_call(program, call)
            || projection_plan_for_call(program, projections, call).is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "proof-only content operation `{}` is used in executable expression position; `old`, `separate`, and exact `Content<A>::project` machines are contract-only",
                call.target.as_str(),
            )));
        }
    }

    plans
}

#[allow(clippy::too_many_arguments)]
fn collect_callable_plans(
    program: &TypedTrees,
    projections: &[ContentProjectionPlan],
    owner_kind: ContentConservationOwnerKind,
    owner: SymbolHandle,
    callable: SymbolHandle,
    label: &str,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    self_data: SymbolHandle,
    contracts: &[&SignatureContract],
    proof_nodes: &mut HashSet<(u32, u32)>,
    plans: &mut Vec<ContentConservationSourcePlan>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in contracts {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                collect_expression_nodes(program, *expression, proof_nodes);
            } else if let ProofFact::Membership(membership) = fact {
                collect_expression_nodes(program, membership.value, proof_nodes);
            }
        }
    }

    let mut by_algebra: HashMap<String, ExpressionHandle> = HashMap::new();
    for contract in contracts {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if !expression_uses_content_surface(program, projections, *expression) {
                continue;
            }
            if contract.kind != SignatureContractKind::Ensures {
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` uses proof-only content operations outside an `ensures` contract; callable-entry/current conservation relates callable outcomes",
                )));
                continue;
            }

            let context = NormalizationContext {
                program,
                projections,
                parameters,
                return_type,
                self_data,
                contracts,
            };
            let (algebra, equation) = match normalize_equation(&context, *expression) {
                Ok(normalized) => normalized,
                Err(message) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "callable `{label}` has an invalid content-conservation contract: {message}",
                    )));
                    continue;
                }
            };
            let algebra_key = algebra_key(&algebra);
            if let Some(first) = by_algebra.insert(algebra_key, *expression) {
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` publishes more than one content-conservation equation for the same algebra (facts `{}` and `{}`); author one normalized equation per outcome row and algebra",
                    program.expression_table.display_name(first),
                    program.expression_table.display_name(*expression),
                )));
                continue;
            }
            let fingerprint = conservation_fingerprint(&algebra, &equation);
            plans.push(ContentConservationSourcePlan {
                source_expression: *expression,
                plan: ContentConservationPlan {
                    owner_kind,
                    owner,
                    callable,
                    algebra,
                    equation,
                    fingerprint,
                },
            });
        }
    }
}

fn projection_plan_for_call<'a>(
    program: &TypedTrees,
    projections: &'a [ContentProjectionPlan],
    call: &TableCallExpression,
) -> Option<&'a ContentProjectionPlan> {
    if !call.receiver.is_valid() {
        return None;
    }
    projections.iter().find(|projection| {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == projection.machine)
        else {
            return false;
        };
        let target_matches = call.target_symbol == machine.symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == call.target_symbol)
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.name.as_str() == call.target.as_str());
        target_matches && receiver_selects_domain(program, call.receiver, projection.domain)
    })
}

fn receiver_selects_domain(
    program: &TypedTrees,
    receiver: ExpressionHandle,
    domain: SymbolHandle,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(receiver) else {
        return false;
    };
    if path.symbol == domain || path.head_symbol == domain {
        return true;
    }
    let Some(spelled) = program
        .expression_table
        .name_path_members(path.members)
        .last()
    else {
        return false;
    };
    let expected = domain_label(program, domain);
    spelled.as_str() == expected.rsplit("::").next().unwrap_or(expected.as_str())
}

fn is_old_call(program: &TypedTrees, call: &TableCallExpression) -> bool {
    is_builtin_call(program, call, BuiltinFunction::ContentOld)
}

fn is_separate_call(program: &TypedTrees, call: &TableCallExpression) -> bool {
    is_builtin_call(program, call, BuiltinFunction::ContentSeparate)
}

fn is_builtin_call(
    program: &TypedTrees,
    call: &TableCallExpression,
    function: BuiltinFunction,
) -> bool {
    !call.receiver.is_valid()
        && call.target.as_str() == function.name()
        && (!call.target_symbol.is_valid()
            || (program.symbols.get(call.target_symbol).kind
                == psi_symbols::SymbolKind::BuiltinFunction
                && program.symbols.name(call.target_symbol) == function.name()))
}

fn expression_uses_content_surface(
    program: &TypedTrees,
    projections: &[ContentProjectionPlan],
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            is_old_call(program, call)
                || is_separate_call(program, call)
                || projection_plan_for_call(program, projections, call).is_some()
                || expression_uses_content_surface(program, projections, call.receiver)
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| {
                        expression_uses_content_surface(program, projections, *argument)
                    })
        }
        ExpressionNode::Atomic(atomic) => {
            expression_uses_content_surface(program, projections, atomic.value)
        }
        ExpressionNode::Binary(binary) => {
            expression_uses_content_surface(program, projections, binary.left)
                || expression_uses_content_surface(program, projections, binary.right)
        }
        ExpressionNode::Unary(unary) => {
            expression_uses_content_surface(program, projections, unary.operand)
        }
        ExpressionNode::Cast(cast) => {
            expression_uses_content_surface(program, projections, cast.value)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_uses_content_surface(program, projections, indexed.collection)
                || expression_uses_content_surface(program, projections, indexed.index)
        }
        ExpressionNode::Member(member) => {
            expression_uses_content_surface(program, projections, member.receiver)
        }
        ExpressionNode::Borrow(inner) => {
            expression_uses_content_surface(program, projections, inner.target)
        }
        ExpressionNode::Range(range) => {
            expression_uses_content_surface(program, projections, range.start)
                || expression_uses_content_surface(program, projections, range.end)
        }
        ExpressionNode::ArrayLiteral(items) => program
            .expression_table
            .expression_handles(*items)
            .iter()
            .any(|item| expression_uses_content_surface(program, projections, *item)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_uses_content_surface(program, projections, field.value)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn collect_expression_nodes(
    program: &TypedTrees,
    expression: ExpressionHandle,
    nodes: &mut HashSet<(u32, u32)>,
) {
    if !expression.is_valid() || !nodes.insert((expression.arena_index(), expression.generation()))
    {
        return;
    }
    let recurse =
        |child, nodes: &mut HashSet<(u32, u32)>| collect_expression_nodes(program, child, nodes);
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            recurse(atomic.value, nodes);
            recurse(atomic.result, nodes);
        }
        ExpressionNode::Binary(binary) => {
            recurse(binary.left, nodes);
            recurse(binary.right, nodes);
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand, nodes),
        ExpressionNode::Cast(cast) => recurse(cast.value, nodes),
        ExpressionNode::Call(call) => {
            recurse(call.receiver, nodes);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument, nodes);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection, nodes);
            recurse(indexed.index, nodes);
        }
        ExpressionNode::Member(member) => recurse(member.receiver, nodes),
        ExpressionNode::Borrow(inner) => recurse(inner.target, nodes),
        ExpressionNode::Range(range) => {
            recurse(range.start, nodes);
            recurse(range.end, nodes);
        }
        ExpressionNode::ArrayLiteral(items) => {
            for item in program.expression_table.expression_handles(*items) {
                recurse(*item, nodes);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                recurse(field.value, nodes);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn algebra_key(algebra: &ContentAlgebraIdentity) -> String {
    match algebra {
        ContentAlgebraIdentity::IntervalSet { coordinate_space } => {
            format!("IntervalSet<{coordinate_space}>")
        }
        ContentAlgebraIdentity::CountedQuantity { unit } => {
            format!("CountedQuantity<{unit}>")
        }
    }
}

fn projection_label(program: &TypedTrees, projection: &ContentProjectionPlan) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == projection.machine)
        .map(|machine| machine.name.to_string())
        .unwrap_or_else(|| format!("projection#{}", projection.machine.arena_index()))
}

fn domain_label(program: &TypedTrees, domain: SymbolHandle) -> String {
    program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == domain)
        .map(|definition| definition.name.to_string())
        .unwrap_or_else(|| format!("domain#{}", domain.arena_index()))
}

fn structural_place_label(place: &ContentStructuralPlace) -> String {
    let mut label = match &place.root {
        ContentPlaceRoot::Parameter { name, .. } => name.clone(),
        ContentPlaceRoot::Result => "result".to_owned(),
    };
    for segment in &place.segments {
        match segment {
            ContentPlaceSegment::Case(case) => {
                label.push_str("::");
                label.push_str(&case.name);
            }
            ContentPlaceSegment::Field(field) => {
                label.push('.');
                label.push_str(&field.name);
            }
            ContentPlaceSegment::FixedIndex(index) => {
                label.push('[');
                label.push_str(&index.to_string());
                label.push(']');
            }
        }
    }
    if place.version == ContentPlaceVersion::Entry {
        format!("old(&{label})")
    } else {
        format!("&{label}")
    }
}
