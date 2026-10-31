use crate::flow::expressions;
use crate::flow::state_statements;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::{BooleanExpressionOwner, ContractExpressionEvaluator};

impl ContractExpressionEvaluator<'_, '_> {
    /// Substitute a contract formal once, then retain caller ownership through
    /// local resolution. Recursive calls share symbols but not these namespaces.
    pub(super) fn resolved_boolean_expression(
        &mut self,
        expression: ExpressionHandle,
        owner: BooleanExpressionOwner,
    ) -> Option<(ExpressionHandle, BooleanExpressionOwner)> {
        let Some(path) = self.program.expression_table.name_path_node(expression) else {
            return None;
        };
        if !self
            .program
            .expression_table
            .name_path_is_single_member(path)
        {
            return None;
        }
        let resolved = match owner {
            BooleanExpressionOwner::Contract => {
                self.argument_for_parameter(path.head_symbol, path.symbol, None)?
            }
            BooleanExpressionOwner::Caller => {
                self.local_initializer(path.head_symbol, path.symbol, path.members)?
            }
        };
        Some((resolved, BooleanExpressionOwner::Caller))
    }

    pub(super) fn resolved_expression(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        // Re-entering an expression whose resolution is still in progress
        // means call-site/local following looped; stand down with unknown
        // rather than overflow the stack.
        self.guarding_cycles(super::ExpressionWork::Resolution, expression, |evaluator| {
            evaluator.resolved_expression_followed(expression)
        })
    }

    fn resolved_expression_followed(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => self
                .resolved_expression(inner.target)
                .or(Some(inner.target)),
            ExpressionNode::Name(path) => {
                let name = self
                    .program
                    .expression_table
                    .name_path_members(path.members)
                    .last()
                    .map(|name| name.as_str());
                self.argument_for_parameter(path.head_symbol, path.symbol, name)
                    .or_else(|| self.local_initializer(path.head_symbol, path.symbol, path.members))
                    .and_then(|resolved| {
                        if resolved == expression {
                            return Some(resolved);
                        }
                        self.resolved_expression(resolved).or(Some(resolved))
                    })
            }
            ExpressionNode::Indexed(indexed) => {
                let collection = self.resolved_expression(indexed.collection)?;
                let index = usize::try_from(self.integer_value(indexed.index)?).ok()?;
                let values = self
                    .program
                    .expression_table
                    .array_literal_node(collection)?;
                expressions(self.program)
                    .expression_handles(*values)
                    .get(index)
                    .copied()
            }
            ExpressionNode::Member(member) => {
                let receiver = self.resolved_expression(member.receiver)?;
                let struct_literal = self
                    .program
                    .expression_table
                    .struct_literal_node(receiver)?;
                expressions(self.program)
                    .struct_fields(struct_literal.fields)
                    .iter()
                    .find(|field| field.name == member.member)
                    .map(|field| field.value)
            }
            _ => None,
        }
    }

    pub(super) fn argument_for_parameter(
        &mut self,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
        name: Option<&str>,
    ) -> Option<ExpressionHandle> {
        let arguments = crate::call_formals::arguments_aligned_to_formals(
            self.program,
            self.call_site.target_symbol(self.program),
            self.target_parameters.len(),
            validation::call_site_argument_expressions(self.program, self.call_site),
        );
        let mut argument_index = 0usize;

        for parameter in self.target_parameters {
            let has_resolved_symbol = head_symbol.is_valid() || symbol.is_valid();
            let parameter_matches = if has_resolved_symbol {
                (head_symbol.is_valid() && head_symbol == parameter.symbol)
                    || (symbol.is_valid() && symbol == parameter.symbol)
            } else {
                name.is_some_and(|name| name == parameter.name.as_str())
            };

            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            if parameter_matches {
                return argument;
            }
        }

        None
    }

    fn local_initializer(
        &mut self,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
        members: arena::HandleSpan<typed_trees::name::Identifier>,
    ) -> Option<ExpressionHandle> {
        let name = self
            .program
            .expression_table
            .name_path_members(members)
            .last()?;
        state_statements(self.program, self.caller_state.statement_nodes)
            .iter()
            .take(self.statement_index)
            .find_map(|statement| {
                let local = statement.as_local_data()?;
                let has_resolved_symbol = head_symbol.is_valid() || symbol.is_valid();
                let local_matches = if has_resolved_symbol {
                    (head_symbol.is_valid() && head_symbol == local.symbol)
                        || (symbol.is_valid() && symbol == local.symbol)
                } else {
                    local.name == *name
                };
                // This evaluator has no flow contexts or storage revisions.
                // Only immutable scalar values can be reconstructed here.
                // Owned aggregate fields may change even without `let mut`;
                // their reads require live evidence, not initializer replay.
                (local_matches && !local.is_mutable && self.closed_value(local.initial_value))
                    .then_some(local.initial_value)
            })
    }

    fn closed_value(&mut self, expression: ExpressionHandle) -> bool {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return false;
        }
        matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Boolean(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::String(_)
        )
    }
}
