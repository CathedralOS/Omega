//! First bounded fact-call projection rung.
//!
//! A direct field read from one exact bodyful internal call is a denotational
//! term, not a runtime temporary.  Every broader shape remains fenced here so
//! the structural judge and package-review identity never acquire an implicit
//! execution model.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::data::{DataMember, DataShapeKind};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::proposition::{PropositionBody, PropositionFormula};
use typed_trees::types::TypeReferenceNode;

pub(crate) fn validate_fact_call_projections(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedFactCallProjection> {
    let mut roots = Vec::new();
    let mut proposition_applications = Vec::new();
    for (_, fact) in program.proof_facts.iter() {
        match fact {
            ProofFact::Expression(expression) => roots.push(*expression),
            ProofFact::Membership(membership) => roots.push(membership.value),
            ProofFact::Proposition(application) => {
                proposition_applications.push(application);
                roots.extend_from_slice(
                    program
                        .expression_table
                        .expression_handles(application.arguments),
                );
            }
        }
    }
    for proposition in program.propositions() {
        if let PropositionBody::Transparent { proposition } = &proposition.body {
            match proposition {
                PropositionFormula::Application(application) => roots.extend_from_slice(
                    program
                        .expression_table
                        .expression_handles(application.arguments),
                ),
                PropositionFormula::BooleanExpression(expression) => roots.push(*expression),
            }
        }
    }

    let substituted = proposition_applications
        .into_iter()
        .flat_map(|application| substituted_projection_requests(program, application))
        .collect::<Vec<_>>();
    let has_projection = !substituted.is_empty()
        || roots
            .iter()
            .any(|root| expression_contains_call_projection(program, *root));
    if !has_projection {
        return Vec::new();
    }
    let operational = crate::infer_operational_may(program);
    let service_reaches = crate::infer_service_reaches(program, &operational);
    let mut visited = Vec::new();
    let mut admitted = Vec::new();
    for root in roots {
        validate_expression(
            program,
            root,
            &operational,
            &service_reaches,
            diagnostics,
            &mut visited,
            &mut admitted,
        );
    }
    for (projection_expression, call_expression) in substituted {
        let ExpressionNode::Member(member) =
            program.expression_table.expression(projection_expression)
        else {
            continue;
        };
        let ExpressionNode::Call(call) = program.expression_table.expression(call_expression)
        else {
            continue;
        };
        if let Some(row) = validate_direct_projection(
            program,
            projection_expression,
            call_expression,
            call,
            member,
            &operational,
            &service_reaches,
            diagnostics,
        ) {
            admitted.push(row);
        }
    }
    admitted.sort_by_key(|row| {
        (
            row.projection_expression.arena_index(),
            row.call_expression.arena_index(),
            row.field.arena_index(),
        )
    });
    admitted.dedup();
    admitted
}

fn substituted_projection_requests(
    program: &TypedTrees,
    application: &typed_trees::proposition::PropositionApplication,
) -> Vec<(ExpressionHandle, ExpressionHandle)> {
    let Some(declaration) = program
        .propositions()
        .iter()
        .find(|candidate| candidate.symbol == application.proposition)
    else {
        return Vec::new();
    };
    let PropositionBody::Transparent {
        proposition: PropositionFormula::BooleanExpression(formula),
    } = declaration.body
    else {
        return Vec::new();
    };
    let parameters = program.proposition_parameters(declaration);
    let arguments = program
        .expression_table
        .expression_handles(application.arguments);
    if parameters.len() != arguments.len() {
        return Vec::new();
    }
    let bindings = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.symbol, *argument))
        .collect::<Vec<_>>();
    let mut requests = Vec::new();
    append_substituted_projection_requests(program, formula, &bindings, &mut requests);
    requests
}

fn append_substituted_projection_requests(
    program: &TypedTrees,
    expression: ExpressionHandle,
    bindings: &[(symbols::SymbolHandle, ExpressionHandle)],
    requests: &mut Vec<(ExpressionHandle, ExpressionHandle)>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            if let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver)
                && let Some((_, actual)) = bindings
                    .iter()
                    .find(|(parameter, _)| *parameter == path.symbol)
                && matches!(
                    program.expression_table.expression(*actual),
                    ExpressionNode::Call(_)
                )
            {
                requests.push((expression, *actual));
            }
            append_substituted_projection_requests(program, member.receiver, bindings, requests);
        }
        ExpressionNode::Atomic(atomic) => {
            append_substituted_projection_requests(program, atomic.value, bindings, requests);
            append_substituted_projection_requests(program, atomic.result, bindings, requests);
        }
        ExpressionNode::Binary(binary) => {
            append_substituted_projection_requests(program, binary.left, bindings, requests);
            append_substituted_projection_requests(program, binary.right, bindings, requests);
        }
        ExpressionNode::Borrow(inner) => {
            append_substituted_projection_requests(program, inner.target, bindings, requests)
        }
        ExpressionNode::Cast(cast) => {
            append_substituted_projection_requests(program, cast.value, bindings, requests)
        }
        ExpressionNode::Call(call) => {
            append_substituted_projection_requests(program, call.receiver, bindings, requests);
            for argument in program.expression_table.expression_handles(call.arguments) {
                append_substituted_projection_requests(program, *argument, bindings, requests);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            append_substituted_projection_requests(program, indexed.collection, bindings, requests);
            append_substituted_projection_requests(program, indexed.index, bindings, requests);
        }
        ExpressionNode::Range(range) => {
            append_substituted_projection_requests(program, range.start, bindings, requests);
            append_substituted_projection_requests(program, range.end, bindings, requests);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                append_substituted_projection_requests(program, field.value, bindings, requests);
            }
        }
        ExpressionNode::Unary(unary) => {
            append_substituted_projection_requests(program, unary.operand, bindings, requests)
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                append_substituted_projection_requests(program, *value, bindings, requests);
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFactCallProjection {
    pub projection_expression: ExpressionHandle,
    pub call_expression: ExpressionHandle,
    pub target_machine: symbols::SymbolHandle,
    pub target_state: symbols::SymbolHandle,
    pub machine_arguments: Box<[typed_trees::expression::StaticMachineArgument]>,
    pub result_type: typed_trees::types::TypeReferenceHandle,
    pub field: symbols::SymbolHandle,
}

fn validate_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
    visited: &mut Vec<ExpressionHandle>,
    admitted: &mut Vec<ValidatedFactCallProjection>,
) {
    if !expression.is_valid() || visited.contains(&expression) {
        return;
    }
    visited.push(expression);
    macro_rules! recurse {
        ($child:expr, $diagnostics:expr, $visited:expr) => {
            validate_expression(
                program,
                $child,
                operational,
                service_reaches,
                $diagnostics,
                $visited,
                admitted,
            )
        };
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            match program.expression_table.expression(member.receiver) {
                ExpressionNode::Call(call) => {
                    if let Some(row) = validate_direct_projection(
                        program,
                        expression,
                        member.receiver,
                        call,
                        member,
                        operational,
                        service_reaches,
                        diagnostics,
                    ) {
                        admitted.push(row);
                    }
                }
                _ if expression_contains_call(program, member.receiver) => diagnostics.push(
                    Diagnostic::error(
                        "fact-position call projection must be one direct plain-record field; nested or adapted projection is not admitted",
                    ),
                ),
                _ => {}
            }
            recurse!(member.receiver, diagnostics, visited);
        }
        ExpressionNode::Atomic(atomic) => {
            recurse!(atomic.value, diagnostics, visited);
            recurse!(atomic.result, diagnostics, visited);
        }
        ExpressionNode::Binary(binary) => {
            recurse!(binary.left, diagnostics, visited);
            recurse!(binary.right, diagnostics, visited);
        }
        ExpressionNode::Borrow(inner) => recurse!(inner.target, diagnostics, visited),
        ExpressionNode::Cast(cast) => recurse!(cast.value, diagnostics, visited),
        ExpressionNode::Call(call) => {
            recurse!(call.receiver, diagnostics, visited);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse!(*argument, diagnostics, visited);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            recurse!(indexed.collection, diagnostics, visited);
            recurse!(indexed.index, diagnostics, visited);
        }
        ExpressionNode::Range(range) => {
            recurse!(range.start, diagnostics, visited);
            recurse!(range.end, diagnostics, visited);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                recurse!(field.value, diagnostics, visited);
            }
        }
        ExpressionNode::Unary(unary) => recurse!(unary.operand, diagnostics, visited),
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                recurse!(*value, diagnostics, visited);
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

fn validate_direct_projection(
    program: &TypedTrees,
    projection_expression: ExpressionHandle,
    call_expression: ExpressionHandle,
    call: &TableCallExpression,
    member: &typed_trees::expression::TableMemberExpression,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ValidatedFactCallProjection> {
    let reject = |reason: &str, diagnostics: &mut Vec<Diagnostic>| {
        diagnostics.push(Diagnostic::error(format!(
            "fact-position projection from call `{}` is not denotational: {reason}",
            call.target
        )));
    };
    if call.receiver.is_valid()
        || call.quotient_operation.is_some()
        || !call.evidence_arguments.is_empty()
        || program
            .expression_table
            .expression_handles(call.arguments)
            .iter()
            .any(|argument| expression_contains_call(program, *argument))
    {
        reject(
            "only one direct free call with non-call runtime arguments is admitted",
            diagnostics,
        );
        return None;
    }

    let (machine, state) = validate_checked_call_candidate(
        program,
        call,
        operational,
        service_reaches,
        &reject,
        diagnostics,
    )?;

    let (data_symbol, qualified) = match program
        .type_reference_table
        .type_reference(state.return_type)
    {
        TypeReferenceNode::Named { symbol, .. } => (*symbol, false),
        TypeReferenceNode::Generic { base_symbol, .. } => (*base_symbol, false),
        TypeReferenceNode::Constrained { .. } => (symbols::SymbolHandle::invalid(), true),
        _ => (symbols::SymbolHandle::invalid(), false),
    };
    if qualified {
        reject(
            "content-bearing or otherwise qualified results are not admitted",
            diagnostics,
        );
        return None;
    }
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == data_symbol)
    else {
        reject("the result is not one nominal plain record", diagnostics);
        return None;
    };
    if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || DataShapeKind::Record
            != typed_trees::data::DataDefinition::shape_kind_from_members(
                program.data_members(data),
            )
    {
        reject(
            "sum, mixed, empty, and opaque result shapes are not admitted",
            diagnostics,
        );
        return None;
    }
    let field = (member.case_variant.is_none())
        .then(|| {
            program.data_members(data).iter().find_map(|candidate| {
                let DataMember::Field(field) = candidate else {
                    return None;
                };
                (field.name.as_str() == member.member.as_str()).then_some(field.symbol)
            })
        })
        .flatten();
    let Some(field) = field else {
        reject(
            "the selected member is not one exact direct record field",
            diagnostics,
        );
        return None;
    };
    Some(ValidatedFactCallProjection {
        projection_expression,
        call_expression,
        target_machine: machine.symbol,
        target_state: state.symbol,
        machine_arguments: call.machine_arguments.clone(),
        result_type: state.return_type,
        field,
    })
}

/// Shared source admission for denotational calls. Final checked termination
/// remains mandatory for every candidate, including integer embeddings.
pub(crate) fn validate_checked_call_candidate<'program>(
    program: &'program TypedTrees,
    call: &TableCallExpression,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    reject: &impl Fn(&str, &mut Vec<Diagnostic>),
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(
    &'program typed_trees::machine::Machine,
    &'program typed_trees::state::State,
)> {
    let matches = program
        .machines()
        .iter()
        .flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .filter(|state| state.symbol == call.target_symbol)
                .map(move |state| (machine, state))
        })
        .collect::<Vec<_>>();
    let [(machine, state)] = matches.as_slice() else {
        reject(
            "the call target does not resolve to one exact entry",
            diagnostics,
        );
        return None;
    };
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
    {
        reject(
            "bodyless, boundary, accepted, and external targets are not admitted",
            diagnostics,
        );
        return None;
    }
    // Termination is finalized after checked control-flow/ranking analysis.
    // This pass retains an otherwise eligible candidate; checked lowering
    // admits the certificate only when the exact machine summary is an
    // unconditional `Terminates` theorem.
    let has_requires = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .any(|contract| {
            matches!(
                contract.kind,
                typed_trees::signature::SignatureContractKind::Requires
            ) && !program.proof_facts.span_or_empty(contract.facts).is_empty()
        });
    if has_requires {
        reject("the selected entry has preconditions", diagnostics);
        return None;
    }
    let proof_only = typed_trees::proof_only::classify(program);
    if program.state_parameters(state).iter().any(|parameter| {
        crate::quotients::type_has_forbidden_denotational_content(
            program,
            &proof_only,
            parameter.type_reference,
        )
    }) {
        reject(
            "runtime arguments are not copy-only and custody-free",
            diagnostics,
        );
        return None;
    }
    if !crate::denotational_calls::has_pure_effect_closure(
        machine.symbol,
        state.symbol,
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_mutable),
        operational,
        service_reaches,
    ) {
        reject(
            "the selected call closure is effectful, reaching, blocking, suspending, mutable, or unresolved",
            diagnostics,
        );
        return None;
    }
    if !crate::denotational_calls::has_observation_free_checked_closure(
        program,
        machine.symbol,
        operational,
    ) {
        reject(
            "the selected call closure observes or mutates hidden state, uses atomic/external machinery, or contains a non-checked target",
            diagnostics,
        );
        return None;
    }
    if !crate::denotational_calls::has_no_crash_routes(program, machine.symbol, operational) {
        reject("the selected call closure has a crash route", diagnostics);
        return None;
    }
    Some((*machine, *state))
}

pub(crate) fn expression_contains_call_projection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => expression_contains_call(program, member.receiver),
        ExpressionNode::Atomic(atomic) => {
            expression_contains_call_projection(program, atomic.value)
                || expression_contains_call_projection(program, atomic.result)
        }
        ExpressionNode::Binary(binary) => {
            expression_contains_call_projection(program, binary.left)
                || expression_contains_call_projection(program, binary.right)
        }
        ExpressionNode::Borrow(inner) => expression_contains_call_projection(program, inner.target),
        ExpressionNode::Cast(cast) => expression_contains_call_projection(program, cast.value),
        ExpressionNode::Call(call) => {
            expression_contains_call_projection(program, call.receiver)
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_contains_call_projection(program, *argument))
        }
        ExpressionNode::Indexed(indexed) => {
            expression_contains_call_projection(program, indexed.collection)
                || expression_contains_call_projection(program, indexed.index)
        }
        ExpressionNode::Range(range) => {
            expression_contains_call_projection(program, range.start)
                || expression_contains_call_projection(program, range.end)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_contains_call_projection(program, field.value)),
        ExpressionNode::Unary(unary) => expression_contains_call_projection(program, unary.operand),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_contains_call_projection(program, *value)),
        _ => false,
    }
}

fn expression_contains_call(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Member(member) => expression_contains_call(program, member.receiver),
        ExpressionNode::Borrow(inner) => expression_contains_call(program, inner.target),
        ExpressionNode::Cast(cast) => expression_contains_call(program, cast.value),
        _ => false,
    }
}
