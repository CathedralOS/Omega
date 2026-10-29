//! Retained constant roots become ordinary checked expression bodies here.
//! Open lexical templates retain only descriptions; concrete specializations
//! own separate entries. The next round of the existing specialization loop
//! closes helper calls in those bodies, without another driver pipeline.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::constant_recipes::{RetainedConstantPosition, create_evaluation_entry};

pub(super) fn materialize(program: &mut TypedTrees) -> Result<usize, Vec<Diagnostic>> {
    // Initializer entries are retained before executable preparation. Admit
    // only entries of live declarations, keeping their existing expression
    // and owner rather than constructing a second evaluator projection.
    let mut initializers = Vec::new();
    for declaration in program.const_declarations() {
        if declaration.evaluated_value.is_some()
            || !declaration.evaluation_entry.is_valid()
            || has_open_scope(program, declaration.symbol)
        {
            continue;
        }
        if !program
            .tables
            .machines
            .is_valid(declaration.evaluation_entry)
        {
            return Err(vec![Diagnostic::error(
                "constant initializer lost its retained evaluation entry",
            )]);
        }
        let entry = program.tables.machines.get(declaration.evaluation_entry);
        if program.machine_by_symbol(entry.symbol).is_none() {
            initializers.push(entry.clone());
        }
    }
    let initializer_count = initializers.len();
    for entry in initializers {
        program.push_machine(entry);
    }
    // Private evaluation projections preserve backing arenas, not ownership
    // of every retained recipe. Only live clauses may introduce executable
    // entries; otherwise preparation resurrects excluded declarations.
    let mut owners = typed_trees::references::program_scoped_proof_facts(program)
        .map(|(owner, _, _)| owner)
        .collect::<Vec<_>>();
    owners.sort_unstable_by_key(|owner| (owner.arena_index(), owner.generation()));
    owners.dedup();
    let mut pending = Vec::new();
    for (_, recipe) in program.constant_recipes.iter() {
        let RetainedConstantPosition::ContractExpression(expression) = recipe.position else {
            continue;
        };
        for handle in recipe.contexts.handles() {
            if !program.constant_expression_contexts.is_valid(handle) {
                return Err(vec![Diagnostic::error(
                    "constant entry lost its lexical context",
                )]);
            }
            let context = *program.constant_expression_contexts.get(handle);
            if context.evaluation_entry.is_valid()
                || !owners.contains(&context.owner)
                || has_open_scope(program, context.owner)
            {
                continue;
            }
            pending.push((handle, context, expression));
        }
    }
    let count = initializer_count + pending.len();
    for (handle, context, expression) in pending {
        if !program.expression_table.expression_is_valid(expression) {
            return Err(vec![Diagnostic::error(
                "constant entry lost its complete expression",
            )]);
        }
        // Contract occurrences are excluded from executable specialization.
        // Give the entry its own graph, preserving authored selections while
        // allowing its calls to acquire their ordinary executable identities.
        let expression = program.expression_table.insert_copy(expression);
        let entry = create_evaluation_entry(
            program,
            context.owner,
            format!("@constant-expression-{}", handle.arena_index()),
            expression,
            context.result_type,
        );
        let context = program.constant_expression_contexts.get_mut(handle);
        context.evaluation_entry = entry.symbol;
        context.evaluation_expression = expression;
        program.push_machine(entry);
    }
    Ok(count)
}

fn has_open_scope(program: &TypedTrees, mut owner: SymbolHandle) -> bool {
    while owner.is_valid() {
        if !program.declaration_type_parameters(owner).is_empty()
            || program
                .machine_by_own_or_state_symbol(owner)
                .is_some_and(|machine| program.callable_body_has_open_parameters(machine))
        {
            return true;
        }
        owner = program.declaration_scope_parent(owner);
    }
    false
}
