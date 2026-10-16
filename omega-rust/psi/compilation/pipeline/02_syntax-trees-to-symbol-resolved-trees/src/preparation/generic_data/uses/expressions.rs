//! Executable expression collection outside generic templates.

use super::super::{
    ExpressionHandle, ExpressionNode, HandleSpan, HashSet, Identifier, Item, StatementNode,
    SyntaxTrees,
};
/// Marks expressions reached by a collection walk; `mark` reports whether
/// the expression was new, so a shared subtree is walked once.
pub(in crate::preparation::generic_data) trait ExpressionMarks {
    fn mark(&mut self, expression: ExpressionHandle) -> bool;
}

impl ExpressionMarks for HashSet<ExpressionHandle> {
    fn mark(&mut self, expression: ExpressionHandle) -> bool {
        self.insert(expression)
    }
}

/// Construction-order inventory; no lexical selection is performed here.
#[derive(Default)]
struct ExpressionInventory {
    seen: ConcreteExpressions,
    handles: Vec<ExpressionHandle>,
}

impl ExpressionMarks for ExpressionInventory {
    fn mark(&mut self, expression: ExpressionHandle) -> bool {
        if !self.seen.mark(expression) {
            return false;
        }
        self.handles.push(expression);
        true
    }
}

pub(crate) fn statement_type_inputs(
    syntax: &SyntaxTrees,
    statement: syntax_trees::statement::StatementHandle,
) -> Vec<syntax_trees::types::TypeReferenceHandle> {
    let mut inputs = Vec::new();
    match syntax.statements.statement(statement) {
        StatementNode::LocalData(local) => inputs.push(local.type_reference),
        StatementNode::Call(call) => {
            visit_static_argument_type_inputs(&call.machine_arguments, &mut |reference| {
                inputs.push(reference)
            });
        }
        _ => {}
    }
    let mut expressions = ExpressionInventory::default();
    collect_statement_expression_handles(syntax, statement, &mut expressions);
    for expression in expressions.handles {
        visit_expression_type_inputs(syntax, expression, &mut |reference| inputs.push(reference));
    }
    inputs.retain(|reference| reference.is_valid());
    inputs
}

pub(in crate::preparation::generic_data) fn visit_expression_type_inputs(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    visit: &mut impl FnMut(syntax_trees::types::TypeReferenceHandle),
) {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Cast(cast) => {
            visit(cast.target_type);
            for &argument in syntax
                .type_references
                .type_reference_handles(cast.semantic_domain_arguments)
            {
                visit(argument);
            }
        }
        ExpressionNode::TypeExpression(reference) | ExpressionNode::ZeroValue(reference) => {
            visit(*reference)
        }
        ExpressionNode::Call(call) => {
            visit_static_argument_type_inputs(&call.machine_arguments, visit)
        }
        _ => {}
    }
}

pub(in crate::preparation::generic_data) fn visit_static_argument_type_inputs(
    arguments: &[syntax_trees::expression::StaticMachineArgument],
    visit: &mut impl FnMut(syntax_trees::types::TypeReferenceHandle),
) {
    for argument in arguments {
        if argument.type_reference.is_valid() {
            visit(argument.type_reference);
        }
        if let Some(application) = &argument.application {
            visit_static_argument_type_inputs(&application.arguments, visit);
        }
    }
}

#[derive(Default)]
pub(in crate::preparation::generic_data) struct ConcreteExpressions(
    std::collections::HashSet<ExpressionHandle>,
);

impl ConcreteExpressions {
    pub(in crate::preparation::generic_data) fn contains(
        &self,
        expression: ExpressionHandle,
    ) -> bool {
        self.0.contains(&expression)
    }
}

impl ExpressionMarks for ConcreteExpressions {
    fn mark(&mut self, expression: ExpressionHandle) -> bool {
        self.0.insert(expression)
    }
}

pub(in crate::preparation::generic_data) fn concrete_machine_expression_handles(
    syntax: &SyntaxTrees,
) -> ConcreteExpressions {
    selected_machine_expression_handles(syntax, syntax.root_item_handles())
}

pub(in crate::preparation::generic_data) fn selected_machine_expression_handles(
    syntax: &SyntaxTrees,
    declarations: &[syntax_trees::item::ItemHandle],
) -> ConcreteExpressions {
    let mut handles = ConcreteExpressions::default();
    for state in selected_machine_state_handles(syntax, declarations) {
        let state = syntax.items.state(state);
        for statement in syntax.items.statements(state.statements) {
            collect_statement_expression_handles(syntax, *statement, &mut handles);
        }
    }
    handles
}

pub(in crate::preparation::generic_data) fn selected_machine_state_handles(
    syntax: &SyntaxTrees,
    declarations: &[syntax_trees::item::ItemHandle],
) -> Vec<syntax_trees::item::StateHandle> {
    // Generic data names once, instead of a root-item scan per attached
    // machine.
    let generic_data = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Data(data) if !data.type_parameters.is_empty() => Some(data.name.as_str()),
            _ => None,
        })
        .collect::<HashSet<&str>>();
    declarations
        .iter()
        .map(|declaration| syntax.root_item(*declaration))
        .filter_map(|item| {
            let machine = item.as_machine()?;
            if !machine.type_parameters.is_empty()
                || machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|attached| generic_data.contains(attached.as_str()))
            {
                return None;
            }
            Some(syntax.items.state_handles(machine.states))
        })
        .flatten()
        .copied()
        .collect()
}

pub(in crate::preparation::generic_data) fn collect_statement_expression_handles(
    syntax: &SyntaxTrees,
    statement: syntax_trees::statement::StatementHandle,
    handles: &mut impl ExpressionMarks,
) {
    use syntax_trees::statement::{TransitionGuardNode, TransitionTargetNode};
    match syntax.tables.statements.statement(statement) {
        StatementNode::Assembly(handle) => {
            let graph = syntax.tables.statements.assembly(*handle);
            for block in syntax.tables.statements.assembly_blocks(graph.blocks) {
                for expression in syntax
                    .tables
                    .statements
                    .assembly_expressions(block.requires)
                    .iter()
                    .chain(syntax.tables.statements.assembly_expressions(block.ensures))
                {
                    collect_expression_handles(syntax, *expression, handles);
                }
                for instruction in syntax
                    .tables
                    .statements
                    .assembly_instructions(block.instructions)
                {
                    collect_statement_expression_handles(syntax, *instruction, handles);
                }
            }
        }
        StatementNode::RootBinding(binding) => {
            collect_expression_handles(syntax, binding.receiver, handles);
            if binding.implementation_operand.is_valid() {
                collect_expression_handles(syntax, binding.implementation_operand, handles);
            }
        }
        StatementNode::AssemblyFact(fact) => {
            collect_expression_handles(syntax, fact.expression, handles)
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_handles(syntax, assignment.target, handles);
            collect_expression_handles(syntax, assignment.value, handles);
        }
        StatementNode::Call(call) => {
            for argument in syntax.tables.statements.expression_handles(call.arguments) {
                collect_expression_handles(syntax, *argument, handles);
            }
        }
        StatementNode::ProofOutputBindingStatement(binding) => {
            collect_expression_handles(syntax, binding.call, handles)
        }
        StatementNode::Expression(expression) => {
            collect_expression_handles(syntax, *expression, handles)
        }
        StatementNode::LocalData(local) => {
            collect_expression_handles(syntax, local.initial_value, handles)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_handles(syntax, guard, handles);
            }
            for target in transition.targets() {
                match syntax.tables.statements.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in syntax.tables.statements.expression_handles(*arguments) {
                            collect_expression_handles(syntax, *argument, handles);
                        }
                    }
                    TransitionTargetNode::Value(value) => {
                        collect_expression_handles(syntax, *value, handles)
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

pub(in crate::preparation::generic_data) fn collect_expression_handles(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    handles: &mut impl ExpressionMarks,
) {
    if !expression.is_valid() || !handles.mark(expression) {
        return;
    }
    match syntax.expressions.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            collect_expression_handles(syntax, dispatch.subject, handles);
            for arm in syntax.expressions.match_arms(dispatch.arms) {
                if let syntax_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    collect_expression_handles(syntax, pattern, handles);
                }
                collect_expression_handles(syntax, arm.value, handles);
            }
        }
        ExpressionNode::ArrayLiteral(expressions) => {
            for expression in syntax.expressions.expression_handles(*expressions) {
                collect_expression_handles(syntax, *expression, handles);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression_handles(syntax, atomic.value, handles);
            collect_expression_handles(syntax, atomic.result, handles);
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_handles(syntax, binary.left, handles);
            collect_expression_handles(syntax, binary.right, handles);
        }
        ExpressionNode::Cast(cast) => collect_expression_handles(syntax, cast.value, handles),
        ExpressionNode::Call(call) => {
            collect_expression_handles(syntax, call.receiver, handles);
            for argument in syntax.expressions.expression_handles(call.arguments) {
                collect_expression_handles(syntax, *argument, handles);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_handles(syntax, indexed.collection, handles);
            collect_expression_handles(syntax, indexed.index, handles);
        }
        ExpressionNode::Membership(membership) => {
            collect_expression_handles(syntax, membership.value, handles)
        }
        ExpressionNode::Member(member) => {
            collect_expression_handles(syntax, member.receiver, handles)
        }
        ExpressionNode::Borrow(inner) => collect_expression_handles(syntax, inner.target, handles),
        ExpressionNode::Range(range) => {
            collect_expression_handles(syntax, range.start, handles);
            collect_expression_handles(syntax, range.end, handles);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in syntax.expressions.struct_fields(literal.fields) {
                collect_expression_handles(syntax, field.value, handles);
            }
        }
        ExpressionNode::Unary(unary) => collect_expression_handles(syntax, unary.operand, handles),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_)
        | ExpressionNode::TypeExpression(_) => {}
    }
}

/// Advisory constructor rewriting borrows the actual value frontier. A captured
/// Name remains authored syntax for the ordinary resolver; no binding is chosen
/// here. Callers visit concrete machines, whose template binders are already gone.
pub(in crate::preparation::generic_data) struct ConstructorFrontier<'a> {
    pub parameters: HandleSpan<syntax_trees::item::StateParameterHandle>,
    pub prior_statements: &'a [syntax_trees::statement::StatementHandle],
}

impl ConstructorFrontier<'_> {
    pub fn captures(&self, syntax: &SyntaxTrees, path: HandleSpan<Identifier>) -> bool {
        let Some(head) = syntax.expressions.identifier_path_members(path).first() else {
            return false;
        };
        self.captures_name(syntax, head)
    }

    pub fn captures_name(&self, syntax: &SyntaxTrees, name: &Identifier) -> bool {
        self.prior_statements.iter().rev().any(|statement| matches!(
            syntax.statements.statement(*statement), StatementNode::LocalData(local) if local.name == *name
        )) || syntax.items.state_parameters(self.parameters).iter().any(|parameter|
            syntax.items.state_parameter(*parameter).name == *name)
    }
}
