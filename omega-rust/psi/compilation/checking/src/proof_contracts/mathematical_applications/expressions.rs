//! Mathematical declarations own their body and dependent-type applications;
//! their generic arguments are not executable machine selections.

use std::collections::HashSet;

use typed_trees::expression::ExpressionHandle;
use typed_trees::mathematical::{MathematicalBody, MathematicalType};

use crate::expression_traversal::collect_expression_nodes;

pub fn expression_nodes(program: &dyn crate::CheckingInput) -> Vec<ExpressionHandle> {
    crate::frozen_program::frozen_memo(
        program,
        |memos| &memos.mathematical_expression_nodes,
        || {
            let mut seen = HashSet::new();
            scoped_expression_nodes(program)
                .into_iter()
                .filter_map(|(_, expression)| seen.insert(expression).then_some(expression))
                .collect()
        },
    )
    .as_ref()
    .clone()
}

/// Preserve the declaration telescope even when an expression arena payload is
/// shared. Executable membership still excludes the same nodes as the unscoped
/// view; a mathematical use does not exempt a runtime use from formation.
pub fn scoped_expression_nodes(
    program: &dyn crate::CheckingInput,
) -> Vec<(symbols::SymbolHandle, ExpressionHandle)> {
    let mut expressions = Vec::new();
    for definition in program.mathematical_definitions() {
        let mut owned = Vec::new();
        let mut expressions_seen = HashSet::new();
        for root in definition_roots(program, definition) {
            collect_expression_nodes(program, root, &mut owned, &mut expressions_seen);
        }
        expressions.extend(
            owned
                .into_iter()
                .map(|expression| (definition.symbol, expression)),
        );
    }
    if expressions.is_empty() {
        return expressions;
    }
    // A shared handle can also be demanded by executable storage or a state.
    // Mathematical ownership never exempts that second use from MP2b.
    let mut executable = Vec::new();
    let mut executable_seen = HashSet::new();
    for machine in program.machines() {
        for data in program.machine_owned_data(machine) {
            collect_expression_nodes(
                program,
                data.initial_value,
                &mut executable,
                &mut executable_seen,
            );
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                for root in crate::machine_calls::calls::statement_value_expression_roots(
                    program, statement,
                ) {
                    collect_expression_nodes(program, root, &mut executable, &mut executable_seen);
                }
            }
        }
    }
    expressions.retain(|(_, expression)| !executable_seen.contains(expression));
    expressions
}

/// Ordered expression roots owned by one declaration: transparent body and
/// dependent type applications, including bodyless assumptions. Consumers
/// retain the declaration's exact telescope while walking each root.
pub fn definition_roots(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::mathematical::MathematicalDefinition,
) -> Vec<ExpressionHandle> {
    let mut roots = Vec::new();
    if let MathematicalBody::Definition(body) = definition.body {
        roots.push(body);
    }
    let mut types = vec![definition.result];
    types.extend(
        program
            .mathematical_parameters(definition.parameters)
            .iter()
            .map(|parameter| parameter.ty),
    );
    let mut visited = Vec::new();
    while let Some(handle) = types.pop() {
        if !handle.is_valid() || visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        match program.mathematical_type(handle) {
            MathematicalType::Ordinary(_) => {}
            MathematicalType::Arrow {
                domain, codomain, ..
            } => {
                types.push(*domain);
                types.push(*codomain);
            }
            MathematicalType::Application { callee, arguments } => {
                types.push(*callee);
                roots.extend_from_slice(program.expression_table.expression_handles(*arguments));
            }
        }
    }
    roots
}

/// Binding placement is separate from mathematical admission. Every erased
/// field, local, formal actual and transition actual supplies a proof term;
/// a handle that also occurs in executable storage retains its runtime demand.
pub fn erased_initializer_nodes(
    program: &dyn crate::CheckingInput,
) -> std::sync::Arc<Vec<ExpressionHandle>> {
    crate::frozen_program::frozen_memo(
        program,
        |memos| &memos.erased_initializers,
        || {
            let mut placements = InitializerPlacements::default();
            for machine in program.machines() {
                for data in program.machine_owned_data(machine) {
                    placements.visit(program, data.initial_value, false);
                }
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        use typed_trees::statement::{
                            StatementNode, TransitionGuardNode, TransitionTargetNode,
                        };
                        match statement {
                            StatementNode::LocalData(local) => placements.visit(
                                program,
                                local.initial_value,
                                local.relevance.is_erased(),
                            ),
                            StatementNode::Call(call) => {
                                for (position, argument) in program
                                    .statement_table
                                    .expression_handles(call.arguments)
                                    .iter()
                                    .enumerate()
                                {
                                    placements.visit(
                                        program,
                                        *argument,
                                        erased_parameter(program, call.target_symbol, position),
                                    );
                                }
                            }
                            StatementNode::Transition(transition) => {
                                if let TransitionGuardNode::When(guard) = transition.guard {
                                    placements.visit(program, guard, false);
                                }
                                for target in transition.targets() {
                                    match program.statement_table.transition_target(target) {
                                        TransitionTargetNode::Named {
                                            path, arguments, ..
                                        } => {
                                            for (position, argument) in program
                                                .statement_table
                                                .expression_handles(*arguments)
                                                .iter()
                                                .enumerate()
                                            {
                                                placements.visit(
                                                    program,
                                                    *argument,
                                                    erased_parameter(
                                                        program,
                                                        path.symbol,
                                                        position,
                                                    ),
                                                );
                                            }
                                        }
                                        TransitionTargetNode::Value(value) => {
                                            placements.visit(program, *value, false)
                                        }
                                        TransitionTargetNode::Terminal => {}
                                    }
                                }
                            }
                            StatementNode::AssemblyFact(_) => {}
                            _ => {
                                for root in
                                    crate::machine_calls::calls::statement_value_expression_roots(
                                        program, statement,
                                    )
                                {
                                    placements.visit(program, root, false);
                                }
                            }
                        }
                    }
                }
            }
            placements
                .erased
                .retain(|expression| !placements.runtime_seen.contains(expression));
            placements.erased
        },
    )
}

fn erased_parameter(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
    position: usize,
) -> bool {
    program
        .state_by_symbol(target)
        .or_else(|| {
            program
                .machine_by_symbol(target)
                .and_then(|machine| program.machine_states(machine).first())
        })
        .and_then(|state| program.state_parameters(state).get(position))
        .is_some_and(|parameter| parameter.relevance.is_erased())
}

#[derive(Default)]
struct InitializerPlacements {
    erased: Vec<ExpressionHandle>,
    erased_seen: HashSet<ExpressionHandle>,
    runtime_seen: HashSet<ExpressionHandle>,
}

impl InitializerPlacements {
    fn visit(
        &mut self,
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        erased: bool,
    ) {
        if !program.expression_table.expression_is_valid(expression) {
            return;
        }
        if erased {
            if !self.erased_seen.insert(expression) {
                return;
            }
            self.erased.push(expression);
        } else if !self.runtime_seen.insert(expression) {
            return;
        }
        use typed_trees::expression::ExpressionNode;
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                self.visit(program, call.receiver, erased);
                for (position, argument) in program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .enumerate()
                {
                    self.visit(
                        program,
                        *argument,
                        erased || erased_parameter(program, call.target_symbol, position),
                    );
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    let field_erased = field.field_symbol.is_valid()
                        && program
                            .any_field_owner(field.field_symbol)
                            .is_some_and(|(_, declared)| declared.relevance.is_erased());
                    self.visit(program, field.value, erased || field_erased);
                }
            }
            node => crate::expression_traversal::children(program, node, |child| {
                self.visit(program, child, erased)
            }),
        }
    }
}
