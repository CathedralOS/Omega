//! Closed integer expressions inside ordinary proof clauses are constant work.
//!
//! A parameter beside the expression remains a symbolic runtime subject. Only
//! the closed integer side is evaluated; checking records the relation needed
//! from its retained recipe. Maximal roots preserve the selected arithmetic
//! and every nested invocation. Their exact call roster retains formation
//! obligations until the ordinary selected-provider evaluator admits them.

use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) struct ContractRoot {
    pub expression: ExpressionHandle,
    pub invocations: Vec<super::range_endpoints::PendingEndpoint>,
}

pub(super) fn roots(
    program: &TypedTrees,
    domain: &crate::build_time_evaluation::EvaluationDomain,
) -> Vec<ContractRoot> {
    let mut pending = Vec::new();
    for (owner, _, fact) in typed_trees::references::program_scoped_proof_facts(program) {
        if !domain.contains(owner) {
            continue;
        }
        match fact {
            ProofFact::Expression(expression) => pending.push((owner, *expression)),
            ProofFact::Membership(membership) => pending.push((owner, membership.value)),
            ProofFact::Proposition(application) => pending.extend(
                program
                    .expression_table
                    .expression_handles(application.arguments)
                    .iter()
                    .map(|expression| (owner, *expression)),
            ),
        }
    }
    let mut visited = std::collections::HashSet::new();
    let mut candidates = Vec::new();
    while let Some((owner, expression)) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) || !visited.insert(expression)
        {
            continue;
        }
        pending.extend(
            crate::machine_execution::admission::expression_children(program, expression)
                .into_iter()
                .map(|child| (owner, child)),
        );
        if !closed(program, expression, &mut Vec::new()) {
            continue;
        }
        let integer =
            validation::parameter_expression_result_type_reference(program, owner, &[], expression)
                .and_then(|reference| program.primitive_type_reference(reference))
                .is_some_and(|primitive| primitive.is_integer());
        if integer {
            candidates.push(expression);
        }
    }
    if candidates.is_empty() {
        return Vec::new();
    }
    // Ordinary symbolic applications retain their denotation route. Only a
    // selected-realization dependency requests this deferred value job; an
    // axiom never becomes a demanded integer merely by appearing in a clause.
    let admission = crate::BuildTimeAdmissionPlan::infer(program, None);
    let selections = validation::derive_pre_flow_operator_selections(program);
    let mut roots = Vec::new();
    for expression in candidates {
        let invocations = subtree(program, expression)
            .into_iter()
            .filter_map(|call_expression| {
                let (machine, static_application) =
                    super::range_endpoints::selected_endpoint_machine(program, call_expression)?;
                Some(super::range_endpoints::PendingEndpoint {
                    expression: call_expression,
                    root: expression,
                    machine: machine.symbol,
                    static_application,
                    contract_expression: true,
                })
            })
            .collect::<Vec<_>>();
        // Selecting an exact checked body may make the dependency graph look
        // independent. Its retained work root still owes evaluation: body
        // selection settles a dependency, never cancels the value job.
        if program
            .constant_recipe_for_contract_expression(expression)
            .is_valid()
            || invocations.iter().any(|call| {
                admission.closure_needs_execution_preparation(program, call.machine, &selections)
            })
        {
            roots.push(ContractRoot {
                expression,
                invocations,
            });
        }
    }
    // A surrounding arithmetic expression owns its children's admissions.
    // Folding a child separately would erase its exact evaluation dependency
    // before the enclosing root is admitted and transactionally published.
    let nested = roots
        .iter()
        .flat_map(|root| {
            subtree(program, root.expression)
                .into_iter()
                .filter(move |expression| *expression != root.expression)
        })
        .collect::<std::collections::HashSet<_>>();
    roots.retain(|root| !nested.contains(&root.expression));
    roots.sort_by_key(|root| (root.expression.arena_index(), root.expression.generation()));
    roots
}

fn subtree(program: &TypedTrees, expression: ExpressionHandle) -> Vec<ExpressionHandle> {
    let mut pending = vec![expression];
    let mut result = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) || result.contains(&expression)
        {
            continue;
        }
        result.push(expression);
        pending.extend(crate::machine_execution::admission::expression_children(
            program, expression,
        ));
    }
    result
}

fn closed(
    program: &TypedTrees,
    expression: ExpressionHandle,
    active: &mut Vec<ExpressionHandle>,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return false;
    }
    active.push(expression);
    let result = match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Name(name) => program
            .const_declaration_by_symbol(name.symbol)
            .is_some_and(|declaration| {
                closed(program, declaration.materialized_initializer, active)
            }),
        ExpressionNode::Call(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::Cast(_) => {
            crate::machine_execution::admission::expression_children(program, expression)
                .into_iter()
                .all(|child| closed(program, child, active))
        }
        _ => false,
    };
    active.pop();
    result
}
