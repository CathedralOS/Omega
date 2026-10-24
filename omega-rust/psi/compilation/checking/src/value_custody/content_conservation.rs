//! Source-visible content-conservation contracts.
//!
//! The surface is deliberately closed: exact owner-unique projection calls,
//! proof-only `old(place)`, compiler-owned `separate(...)`, and equality.
//! This module normalizes authored equations for checked facts and semantic
//! identity; it does not infer sealed introductions or custody exits.

mod contexts;
mod normalization;

pub use normalization::{data_variant_name, push_structural_field};

use normalization::{NormalizationContext, normalize_equation};

use arena::Arena;
use diagnostics::Diagnostic;
use language_semantics::content::{
    ContentAlgebraIdentity, ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionPlan, ContentStructuralPlace, conservation_report_fingerprint,
};
use std::collections::HashSet;
use std::sync::Arc;
use symbols::{BuiltinFunction, SymbolHandle};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentConservationSourcePlan {
    pub source_expression: ExpressionHandle,
    pub plan: ContentConservationPlan,
}

/// Formation belongs to this immutable checking invocation. These observations
/// describe proof-only places, not runtime reference values or proved theorems.
#[derive(Debug)]
pub(crate) struct ContentConservationInputs {
    plans: Arc<Vec<ContentConservationSourcePlan>>,
    checking_plans: Vec<ContentConservationSourcePlan>,
    diagnostics: Vec<Diagnostic>,
    formations: Arena<ContentProjectionFormation>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ContentProjectionFormation {
    callable: SymbolHandle,
    source_expression: ExpressionHandle,
    projection: SymbolHandle,
    call: ExpressionHandle,
    argument: ExpressionHandle,
}

/// A successful closed equation lends only its own exact projection operands.
#[derive(Clone, Copy)]
pub(crate) struct ContentDeclarationFormation<'inputs> {
    inputs: &'inputs ContentConservationInputs,
    callable: SymbolHandle,
    source_expression: ExpressionHandle,
}

impl ContentConservationInputs {
    pub(crate) fn declaration_formation(
        &self,
        callable: SymbolHandle,
        source_expression: ExpressionHandle,
    ) -> Option<ContentDeclarationFormation<'_>> {
        self.plans
            .iter()
            .chain(&self.checking_plans)
            .any(|plan| {
                plan.plan.callable == callable && plan.source_expression == source_expression
            })
            .then_some(ContentDeclarationFormation {
                inputs: self,
                callable,
                source_expression,
            })
    }
}

impl ContentDeclarationFormation<'_> {
    pub(crate) fn forms_argument(
        self,
        projection: SymbolHandle,
        call: ExpressionHandle,
        argument: ExpressionHandle,
    ) -> bool {
        self.inputs.formations.iter().any(|(_, formation)| {
            formation.callable == self.callable
                && formation.source_expression == self.source_expression
                && formation.projection == projection
                && formation.call == call
                && formation.argument == argument
        })
    }
}

pub(crate) fn validate_content_conservation_contracts(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let inputs = content_conservation_inputs(program);
    diagnostics.extend(inputs.diagnostics.iter().cloned());
}

/// Produce normalization, scoped operand formation and diagnostics together.
/// Frozen consumers borrow this same result rather than discarding and
/// rebuilding it between source validation and checked fact construction.
pub(crate) fn content_conservation_inputs(
    program: &dyn crate::CheckingInput,
) -> Arc<ContentConservationInputs> {
    crate::frozen_program::frozen_memo(
        program,
        |memos| &memos.content_conservation,
        || {
            let projections =
                crate::value_custody::content_projections::build_content_projection_plans(program);
            let mut diagnostics = Vec::new();
            let mut formations = Arena::default();
            let mut checking_plans = Vec::new();
            let plans = collect_content_conservation_plans(
                program,
                &projections,
                &mut diagnostics,
                &mut formations,
                &mut checking_plans,
            );
            ContentConservationInputs {
                plans: Arc::new(plans),
                checking_plans,
                diagnostics,
                formations,
            }
        },
    )
}

/// The already normalized source contracts for check facts and publication.
pub fn content_conservation_plans(
    program: &dyn crate::CheckingInput,
) -> Arc<Vec<ContentConservationSourcePlan>> {
    Arc::clone(&content_conservation_inputs(program).plans)
}

fn collect_content_conservation_plans(
    program: &dyn crate::CheckingInput,
    projections: &[ContentProjectionPlan],
    diagnostics: &mut Vec<Diagnostic>,
    formations: &mut Arena<ContentProjectionFormation>,
    checking_plans: &mut Vec<ContentConservationSourcePlan>,
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
                formations,
                diagnostics,
            );
        }
    }

    program.for_each_machine_state_contract_frame(|machine, state, contracts, label| {
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
            formations,
            diagnostics,
        );
    });

    let non_contract_nodes = contexts::non_contract_expression_nodes(
        program,
        projections,
        &mut proof_nodes,
        diagnostics,
        checking_plans,
        formations,
    );

    // Arena allocation is not executable ownership: callable application can
    // leave replaced proof copies behind. Inspect live non-contract uses;
    // sharing a handle with a proof occurrence does not excuse an executable use.
    for (handle, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        if !non_contract_nodes.contains(&handle) {
            continue;
        }
        if is_old_call(program, call)
            || is_separate_call(program, call)
            || projection_plan_for_call(program, projections, call).is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "proof-only content operation `{}` is used in executable expression position; `old`, `separate`, and exact `Content<Owner, A>::project` machines are contract-only",
                call.target.as_str(),
            )));
        }
    }

    plans
}

#[allow(clippy::too_many_arguments)]
fn collect_callable_plans(
    program: &dyn crate::CheckingInput,
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
    formations: &mut Arena<ContentProjectionFormation>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in contracts {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let Some(expression) = fact.as_expression() {
                collect_expression_nodes(program, *expression, proof_nodes);
            } else if let Some(membership) = fact.as_membership() {
                collect_expression_nodes(program, membership.value, proof_nodes);
            }
        }
    }

    let first_callable_plan = plans.len();
    for contract in contracts {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(expression) = fact.as_expression() else {
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
                callable,
                source_expression: *expression,
                projections,
                parameters,
                return_type,
                self_data,
                contracts,
            };
            let mut projection_formations = Vec::new();
            let (algebra, equation) = match normalize_equation(
                &context,
                *expression,
                &mut projection_formations,
            ) {
                Ok(normalized) => normalized,
                Err(message) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "callable `{label}` has an invalid content-conservation contract: {message}",
                    )));
                    continue;
                }
            };
            // An algebra defines how content combines, not which qualification
            // owns it. Several independent claims on one carrier may use the
            // same interval or count algebra and must conserve independently.
            // Repeated equations for an overlapping owner remain ambiguous;
            // the later joint transaction still proves exact parent spending.
            if let Some(first) = plans[first_callable_plan..].iter().find(|previous| {
                previous.plan.algebra == algebra
                    && [
                        previous.plan.equation.left(),
                        previous.plan.equation.right(),
                    ]
                    .into_iter()
                    .any(|previous_term| {
                        [equation.left(), equation.right()]
                            .into_iter()
                            .any(|term| share_projection_owner(previous_term, term))
                    })
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` publishes more than one content-conservation equation for the same qualification owner and algebra (facts `{}` and `{}`); author one normalized equation per outcome row and qualification owner",
                    program.expression_table.display_name(first.source_expression),
                    program.expression_table.display_name(*expression),
                )));
                continue;
            }
            // Partial/invalid equations publish no operand formation rows.
            formations.insert_many(projection_formations);
            let report_fingerprint = conservation_report_fingerprint(&algebra, &equation);
            plans.push(ContentConservationSourcePlan {
                source_expression: *expression,
                plan: ContentConservationPlan {
                    owner_kind,
                    owner,
                    callable,
                    algebra,
                    equation,
                    report_fingerprint,
                },
            });
        }
    }
}

fn share_projection_owner(left: &ContentConservationTerm, right: &ContentConservationTerm) -> bool {
    match (left, right) {
        (ContentConservationTerm::Separate(children), other)
        | (other, ContentConservationTerm::Separate(children)) => children
            .iter()
            .any(|child| share_projection_owner(child, other)),
        (
            ContentConservationTerm::Projection {
                domain: left_domain,
                semantic_domain: left_instance,
                projection_machine: left_projection,
                ..
            },
            ContentConservationTerm::Projection {
                domain: right_domain,
                semantic_domain: right_instance,
                projection_machine: right_projection,
                ..
            },
        ) => {
            left_domain == right_domain
                && left_instance == right_instance
                && left_projection == right_projection
        }
    }
}

fn projection_plan_for_call<'a>(
    program: &dyn crate::CheckingInput,
    projections: &'a [ContentProjectionPlan],
    call: &TableCallExpression,
) -> Option<&'a ContentProjectionPlan> {
    // The selected declaration owns projection meaning. Receiver spellings
    // and same-named state bodies cannot substitute for that exact identity.
    projections.iter().find(|projection| {
        program.machine(projection.machine).is_some_and(|machine| {
            call.target_symbol == machine.symbol
                || typed_trees::state::state_by_symbol(
                    program.machine_states(machine),
                    call.target_symbol,
                )
                .is_some()
        })
    })
}

fn is_old_call(program: &dyn crate::CheckingInput, call: &TableCallExpression) -> bool {
    is_builtin_call(program, call, BuiltinFunction::ContentOld)
}

fn is_separate_call(program: &dyn crate::CheckingInput, call: &TableCallExpression) -> bool {
    is_builtin_call(program, call, BuiltinFunction::ContentSeparate)
}

fn is_builtin_call(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
    function: BuiltinFunction,
) -> bool {
    // Resolution decides: a same-spelled authored machine shadows the
    // builtin, and an unresolved call is no compiler function.
    !call.receiver.is_valid()
        && program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            == Some(function)
}

fn expression_uses_content_surface(
    program: &dyn crate::CheckingInput,
    projections: &[ContentProjectionPlan],
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            crate::expression_traversal::match_children(program, *dispatch)
                .any(|child| expression_uses_content_surface(program, projections, child))
        }
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
        ExpressionNode::SavedSubject(_) | typed_trees::literal_expression_pattern!() => false,
    }
}

fn collect_expression_nodes(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    nodes: &mut HashSet<(u32, u32)>,
) {
    if !expression.is_valid() || !nodes.insert((expression.arena_index(), expression.generation()))
    {
        return;
    }
    crate::expression_traversal::children(
        program,
        program.expression_table.expression(expression),
        |child| collect_expression_nodes(program, child, nodes),
    );
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

fn projection_label(
    program: &dyn crate::CheckingInput,
    projection: &ContentProjectionPlan,
) -> String {
    program
        .machine(projection.machine)
        .map(|machine| machine.name.to_string())
        .unwrap_or_else(|| format!("projection#{}", projection.machine.arena_index()))
}

fn domain_label(program: &dyn crate::CheckingInput, domain: SymbolHandle) -> String {
    program
        .domain_definition(domain)
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
