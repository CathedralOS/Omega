//! Retained constant roots become ordinary checked expression bodies here.
//! Open expression inputs retain only descriptions; concrete specializations
//! own separate entries. A closed expression inside a generic owner still owes
//! its formation and evaluation obligations. The next specialization round
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
    let operands = typed_trees::references::collect_program_operand_owners(program);
    let mut pending = Vec::new();
    for (_, recipe) in program.constant_recipes.iter() {
        let expression = recipe.position.expression();
        if !expression.is_valid() {
            continue;
        }
        if program
            .expression_table
            .expression_handles(recipe.invocations)
            .iter()
            .any(|invocation| !program.expression_table.expression_is_valid(*invocation))
        {
            return Err(vec![Diagnostic::error(
                "constant entry lost its invocation correspondence",
            )]);
        }
        for handle in recipe.contexts.handles() {
            if !program.constant_expression_contexts.is_valid(handle) {
                return Err(vec![Diagnostic::error(
                    "constant entry lost its lexical context",
                )]);
            }
            let context = *program.constant_expression_contexts.get(handle);
            let live = match recipe.position {
                RetainedConstantPosition::ContractExpression(_) => owners.contains(&context.owner),
                RetainedConstantPosition::ConstApplication { site, .. } => operands
                    .type_owners(site)
                    .any(|owner| owner == context.owner),
                RetainedConstantPosition::RangeEndpoint(root) => operands
                    .expression_owners(root)
                    .any(|owner| owner == context.owner),
                _ => false,
            };
            if context.evaluation_entry.is_valid()
                || !live
                || !has_closed_inputs(program, expression, context.result_type)
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
        // Static occurrences are excluded from executable specialization.
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

/// Lexical containment is not a free-variable dependency: `limit()` inside
/// `store<T>` needs no `T`. Use the shared reference walker so static arguments,
/// dependent carriers and nested type payloads participate in closure too.
/// This prepares an ordinary body, never authorizes its execution or effects.
fn has_closed_inputs(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    result_type: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let mut inputs = ClosedInputs {
        program,
        closed: true,
        pending_binders: Vec::new(),
        bound_scopes: Vec::new(),
    };
    typed_trees::references::visit_expression(program, &mut inputs, expression);
    typed_trees::references::visit_type_reference(program, &mut inputs, result_type);
    inputs.closed
}

/// Applied callable descriptions can mention their own formal parameters.
/// Those are bound inside the signature, not free inputs to this expression.
struct ClosedInputs<'program> {
    program: &'program TypedTrees,
    closed: bool,
    pending_binders: Vec<SymbolHandle>,
    bound_scopes: Vec<Vec<SymbolHandle>>,
}

impl Extend<SymbolHandle> for ClosedInputs<'_> {
    fn extend<I: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: I) {
        use symbols::SymbolKind;
        for symbol in symbols {
            // Optional call/selection payloads use zero for absence. Formation
            // checking, not dependency discovery, rejects unresolved operands.
            if !symbol.is_valid()
                || self
                    .bound_scopes
                    .iter()
                    .any(|scope| scope.contains(&symbol))
            {
                continue;
            }
            self.closed &= match self.program.symbols.get(symbol).kind {
                SymbolKind::Unknown
                | SymbolKind::Parameter
                | SymbolKind::Local
                | SymbolKind::TypeParameter
                | SymbolKind::MachineParameter
                | SymbolKind::ConformanceParameter
                | SymbolKind::PropositionParameter
                | SymbolKind::PropositionMachineParameter => false,
                SymbolKind::Const => !has_open_scope(self.program, symbol),
                _ => true,
            };
        }
    }
}

impl typed_trees::references::TypedReferences for ClosedInputs<'_> {
    fn callable_signature(&mut self, signature: &typed_trees::signature::StateSignature) {
        self.pending_binders.extend(
            self.program
                .tables
                .state_parameters
                .span_or_empty(signature.parameters)
                .iter()
                .map(|parameter| parameter.symbol),
        );
        self.pending_binders.extend(
            self.program
                .data_type_parameters
                .span_or_empty(signature.type_parameters)
                .iter()
                .map(|parameter| parameter.symbol),
        );
    }

    fn enter_lexical_scope(&mut self, _: SymbolHandle) {
        self.bound_scopes
            .push(std::mem::take(&mut self.pending_binders));
    }

    fn leave_lexical_scope(&mut self) {
        self.bound_scopes.pop();
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::expression::ExpressionNode;
    use typed_trees::references::TypedReferences;
    use typed_trees::types::TypeReferenceNode;

    #[test]
    fn constant_entry_inputs_include_the_result_carrier() {
        let mut symbols = SymbolTableBuilder::new();
        let parameter =
            symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Borrowed("T"));
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let root = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let closed = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let open = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: parameter,
                name: typed_trees::name::Identifier::generated("T"),
            });
        assert!(has_closed_inputs(&program, root, closed));
        assert!(!has_closed_inputs(&program, root, open));
    }

    #[test]
    fn constant_entry_signature_binders_do_not_hide_free_operands() {
        let mut symbols = SymbolTableBuilder::new();
        let parameter =
            symbols.insert_root(SymbolKind::Parameter, SymbolNameRef::Borrowed("value"));
        let program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let mut inputs = ClosedInputs {
            program: &program,
            closed: true,
            pending_binders: vec![parameter],
            bound_scopes: Vec::new(),
        };
        inputs.enter_lexical_scope(SymbolHandle::invalid());
        inputs.extend([SymbolHandle::invalid(), parameter]);
        assert!(
            inputs.closed,
            "a signature's own formal is bound; zero is absent"
        );
        inputs.leave_lexical_scope();
        inputs.extend([parameter]);
        assert!(
            !inputs.closed,
            "the same formal outside its scope is a free input"
        );
    }
}
