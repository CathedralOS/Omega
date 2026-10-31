//! Closed integer expressions inside ordinary proof clauses are constant work.
//!
//! A parameter beside the expression remains a symbolic runtime subject. Only
//! the closed integer side is evaluated; checking records the relation needed
//! from its retained recipe. Maximal roots preserve the selected arithmetic
//! and every nested invocation. Their exact call roster retains formation
//! obligations until the ordinary selected-provider evaluator admits them.

use crate::build_time_evaluation::expression_table;
use typed_trees::TypedTrees;
use typed_trees::constant_recipes::ConstantExpressionContext;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) struct ContractRoot {
    pub expression: ExpressionHandle,
    pub context: ConstantExpressionContext,
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
                expression_table(program)
                    .expression_handles(application.arguments)
                    .iter()
                    .map(|expression| (owner, *expression)),
            ),
        }
    }
    let mut visited = std::collections::HashSet::new();
    let mut candidates = Vec::new();
    while let Some((owner, expression)) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression)
            || !visited.insert((owner, expression))
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
        let has_recipe = program
            .constant_recipe_for_contract_expression(expression)
            .is_valid();
        let children = subtree(program, expression);
        let has_observation = children.iter().any(|child| {
            matches!(program.expression_table.expression(*child),
                ExpressionNode::Call(call) if call.layout_query.is_some())
        });
        let invocations = children
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
        if !has_recipe && !has_observation && invocations.is_empty() {
            continue;
        }
        let Some(result_type) =
            validation::parameter_expression_result_type_reference(program, owner, &[], expression)
        else {
            continue;
        };
        if !program
            .primitive_type_reference(result_type)
            .is_some_and(|primitive| primitive.is_integer())
        {
            continue;
        }
        candidates.push((
            expression,
            ConstantExpressionContext::new(owner, result_type),
            has_recipe || has_observation,
            invocations,
        ));
    }
    if candidates.is_empty() {
        return Vec::new();
    }
    // Ordinary symbolic applications retain their denotation route. Only a
    // selected-realization dependency requests this deferred value job; an
    // axiom never becomes a demanded integer merely by appearing in a clause.
    let mut admission_and_selections = None;
    let mut roots = Vec::new();
    for (expression, context, has_recipe, invocations) in candidates {
        // Selecting an exact checked body may make the dependency graph look
        // independent. Its retained work root still owes evaluation: body
        // selection settles a dependency, never cancels the value job.
        let keep = if has_recipe {
            true
        } else {
            let (admission, selections) = admission_and_selections.get_or_insert_with(|| {
                (
                    crate::BuildTimeAdmissionPlan::infer(program, None),
                    validation::derive_pre_flow_operator_selections(program),
                )
            });
            invocations.iter().any(|call| {
                admission.closure_needs_execution_preparation(program, call.machine, selections)
            })
        };
        if keep {
            roots.push(ContractRoot {
                expression,
                context,
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
                .map(move |expression| (root.context.owner, expression))
        })
        .collect::<std::collections::HashSet<_>>();
    roots.retain(|root| !nested.contains(&(root.context.owner, root.expression)));
    roots.sort_by_key(|root| {
        (
            root.expression.arena_index(),
            root.expression.generation(),
            root.context.owner.arena_index(),
            root.context.owner.generation(),
        )
    });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(source: &str) -> TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("lex");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
        )
        .expect("resolve");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
    }

    #[test]
    fn constant_entries_do_not_revive_excluded_contract_owners() {
        let mut program = typed(
            "data Cell { code: u64; tag: u64; }
             machine extent() -> u64 { size_of<Cell>() }
             machine unrelated() requires size_of<Cell>() + align_of<Cell>() == 24 {}",
        );
        let extent = program
            .realized_machine_named("extent")
            .expect("extent")
            .symbol;
        super::super::constant_recipes::retain_deferred_constant_recipes(
            &mut program,
            &crate::build_time_evaluation::EvaluationDomain::all(),
        )
        .expect("retain contract jobs");
        assert!(!program.constant_recipes.is_empty());
        let projected = crate::build_time_evaluation::EvaluationDomain::all()
            .demanded_roots(&program, &[extent])
            .execution_program(&program);
        assert!(projected.realized_machine_named("unrelated").is_none());
        assert!(
            !projected.constant_recipes.is_empty(),
            "projection preserves backing arenas"
        );
        let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            projected,
            &typed_trees_to_typed_trees::PreparationRequest::preliminary(),
        )
        .expect("prepare the exact execution projection");
        for (_, recipe) in prepared.constant_recipes.iter() {
            for context in prepared
                .constant_expression_contexts
                .span_or_empty(recipe.contexts)
            {
                assert!(
                    !context.evaluation_entry.is_valid(),
                    "excluded scope must not gain an executable entry"
                );
            }
        }
    }

    #[test]
    fn constant_entries_keep_callee_recipe_scope_separate_from_caller() {
        let mut program = typed(
            "data Cell { code: u64; tag: u64; }
             machine require_size(value: u64)
             requires value >= size_of<Cell>() + align_of<Cell>() {}
             machine program() { require_size(24); }",
        );
        let callee = program
            .realized_machine_named("require_size")
            .expect("callee")
            .symbol;
        let caller = program
            .realized_machine_named("program")
            .expect("caller")
            .symbol;
        super::super::constant_recipes::retain_deferred_constant_recipes(
            &mut program,
            &crate::build_time_evaluation::EvaluationDomain::all(),
        )
        .expect("retain requirement root");
        let (prepared, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
            program,
            &typed_trees_to_typed_trees::PreparationRequest::preliminary(),
        )
        .expect("prepare entries");
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            prepared,
            preparation,
            &typed_trees_to_checked_trees::CheckingRequest::new(),
        )
        .expect("retain conditional call requirement");
        let obligations = checked
            .facts
            .constant_recipe_obligations
            .iter()
            .filter(|obligation| obligation.owner() == caller)
            .collect::<Vec<_>>();
        assert!(
            !obligations.is_empty(),
            "caller must retain the deferred requirement"
        );
        for obligation in obligations {
            assert_eq!(obligation.recipe_owner(), callee);
        }
    }

    #[test]
    fn constant_entries_leave_open_requirement_signatures_descriptive() {
        let mut program =
            typed("trait Geometry { machine check<T>() requires size_of<T>() >= 8; }");
        super::super::constant_recipes::retain_deferred_constant_recipes(
            &mut program,
            &crate::build_time_evaluation::EvaluationDomain::all(),
        )
        .expect("retain generic requirement root");
        assert!(!program.constant_recipes.is_empty());
        let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            program,
            &typed_trees_to_typed_trees::PreparationRequest::preliminary(),
        )
        .expect("prepare descriptions without executing open signatures");
        for (_, recipe) in prepared.constant_recipes.iter() {
            for context in prepared
                .constant_expression_contexts
                .span_or_empty(recipe.contexts)
            {
                assert!(!context.evaluation_entry.is_valid());
            }
        }
    }

    #[test]
    fn constant_entries_are_checked_per_specialized_scope() {
        let mut program = typed(
            "data Cell { code: u64; tag: u64; }
             machine require_geometry<T>()
             requires size_of<Cell>() + align_of<Cell>() == 24 {}
             machine program() {
                 require_geometry<u8>();
                 require_geometry<u64>();
             }",
        );
        let template = program
            .realized_machine_named("require_geometry")
            .expect("template")
            .symbol;
        super::super::constant_recipes::retain_deferred_constant_recipes(
            &mut program,
            &crate::build_time_evaluation::EvaluationDomain::all(),
        )
        .expect("retain complete roots");
        let (prepared, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
            program,
            &typed_trees_to_typed_trees::PreparationRequest::preliminary(),
        )
        .expect("prepare complete entries");
        let contexts = prepared
            .constant_recipes
            .iter()
            .flat_map(|(_, recipe)| {
                prepared
                    .constant_expression_contexts
                    .span_or_empty(recipe.contexts)
            })
            .copied()
            .collect::<Vec<_>>();
        assert!(
            contexts
                .iter()
                .filter(|context| context.owner == template)
                .all(|context| !context.evaluation_entry.is_valid())
        );
        let entries = contexts
            .iter()
            .filter(|context| context.evaluation_entry.is_valid())
            .collect::<Vec<_>>();
        assert_eq!(
            entries.len(),
            2,
            "both concrete applications own complete entries"
        );
        assert_ne!(entries[0].evaluation_entry, entries[1].evaluation_entry);
        assert_ne!(
            entries[0].evaluation_expression,
            entries[1].evaluation_expression
        );
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            prepared,
            preparation,
            &typed_trees_to_checked_trees::CheckingRequest::new(),
        )
        .expect("ordinary checking must accept the conditional entries");
        for context in entries {
            let entry = checked
                .typed
                .constant_evaluation_entry(context.evaluation_entry)
                .expect("checked entry retains exact source ownership");
            assert_eq!(entry.owner, context.owner);
            assert_eq!(entry.evaluation_expression, context.evaluation_expression);
            assert_ne!(entry.source_expression, entry.evaluation_expression);
        }
        assert!(
            !checked.facts.integer_range_obligations.is_empty(),
            "checking must retain addition formation, not assume it succeeded"
        );
    }

    #[test]
    fn constant_expression_contexts_keep_shared_contract_occurrences() {
        let mut program = typed(
            "data Cell { code: u64; }
             machine extent() -> u64 { size_of<Cell>() }
             machine first() requires extent() >= 8 {}
             machine second() {}",
        );
        let first = program
            .realized_machine_named("first")
            .expect("first")
            .clone();
        let second = program
            .realized_machine_named("second")
            .expect("second")
            .symbol;
        program.tables.machines.for_each_mut(|_, machine| {
            if machine.symbol == second {
                machine.contracts = first.contracts;
            }
        });
        let roots = roots(
            &program,
            &crate::build_time_evaluation::EvaluationDomain::all(),
        );
        assert_eq!(
            roots.len(),
            2,
            "one shared expression still has two lexical occurrences"
        );
        assert_eq!(roots[0].expression, roots[1].expression);
        assert_ne!(roots[0].context.owner, roots[1].context.owner);
        for root in roots {
            assert!(root.context.owner == first.symbol || root.context.owner == second);
            assert_eq!(
                program.primitive_type_reference(root.context.result_type),
                Some(typed_trees::types::PrimitiveType::U64),
            );
        }
        let domain = crate::build_time_evaluation::EvaluationDomain::all();
        let plan = super::super::range_endpoints::integer_root_plan(&program, &domain)
            .expect("shared root plan");
        assert_eq!(
            plan.calls.len(),
            1,
            "shared storage has one invocation occurrence"
        );
        super::super::constant_recipes::retain_deferred_constant_recipes(&mut program, &domain)
            .expect("retain complete roots");
        assert_eq!(program.constant_recipes.len(), 1);
        let (_, recipe) = program.constant_recipes.iter().next().expect("recipe");
        assert_eq!(
            expression_table(&program)
                .expression_handles(recipe.invocations)
                .len(),
            1
        );
        assert_eq!(
            program
                .constant_expression_contexts
                .span_or_empty(recipe.contexts)
                .len(),
            2
        );
        let context_count = program.constant_expression_contexts.len();
        super::super::constant_recipes::retain_deferred_constant_recipes(&mut program, &domain)
            .expect("retain roots again");
        assert_eq!(program.constant_expression_contexts.len(), context_count);
    }

    #[test]
    fn constant_expression_contexts_keep_a_root_nested_only_in_another_owner() {
        let mut program = typed(
            "data Cell { code: u64; }
             machine first() requires size_of<Cell>() + 0 >= 8 {}
             machine second() requires size_of<Cell>() >= 8 {}",
        );
        let first = program
            .realized_machine_named("first")
            .expect("first")
            .symbol;
        let second = program
            .realized_machine_named("second")
            .expect("second")
            .symbol;
        let domain = crate::build_time_evaluation::EvaluationDomain::all();
        let initial = roots(&program, &domain);
        let outer = initial
            .iter()
            .find(|root| root.context.owner == first)
            .expect("arithmetic root")
            .expression;
        let ExpressionNode::Binary(arithmetic) = program.expression_table.expression(outer) else {
            panic!("the complete root must include the addition");
        };
        let shared = arithmetic.left;
        let predicate = typed_trees::references::program_scoped_proof_facts(&program)
            .find_map(|(owner, _, fact)| match fact {
                ProofFact::Expression(expression) if owner == second => Some(*expression),
                _ => None,
            })
            .expect("second predicate");
        let ExpressionNode::Binary(comparison) = program.expression_table.expression_mut(predicate)
        else {
            panic!("comparison predicate");
        };
        comparison.left = shared;
        let roots = roots(&program, &domain);
        assert_eq!(roots.len(), 2);
        assert!(
            roots
                .iter()
                .any(|root| root.context.owner == first && root.expression == outer)
        );
        assert!(
            roots
                .iter()
                .any(|root| root.context.owner == second && root.expression == shared)
        );
    }
}
