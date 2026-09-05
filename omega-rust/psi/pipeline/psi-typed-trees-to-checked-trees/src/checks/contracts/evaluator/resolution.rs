use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::statement::StatementNode;

use super::ContractExpressionEvaluator;

impl ContractExpressionEvaluator<'_, '_> {
    pub(super) fn resolved_expression(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        // Re-entering an expression whose resolution is still in progress
        // means call-site/local following looped; stand down with unknown
        // rather than overflow the stack.
        Self::guarding_cycles(&self.active_resolutions, expression, || {
            self.resolved_expression_followed(expression)
        })
    }

    fn resolved_expression_followed(
        &self,
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
                let ExpressionNode::ArrayLiteral(values) =
                    self.program.expression_table.expression(collection)
                else {
                    return None;
                };
                self.program
                    .expression_table
                    .expression_handles(*values)
                    .get(index)
                    .copied()
            }
            ExpressionNode::Member(member) => {
                let receiver = self.resolved_expression(member.receiver)?;
                let ExpressionNode::StructLiteral(struct_literal) =
                    self.program.expression_table.expression(receiver)
                else {
                    return None;
                };
                self.program
                    .expression_table
                    .struct_fields(struct_literal.fields)
                    .iter()
                    .find(|field| field.name == member.member)
                    .map(|field| field.value)
            }
            _ => None,
        }
    }

    pub(super) fn argument_for_parameter(
        &self,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
        name: Option<&str>,
    ) -> Option<ExpressionHandle> {
        let arguments = crate::call_site_argument_expressions(self.program, self.call_site);
        let mut argument_index = 0usize;

        for parameter in self.target_parameters {
            let has_resolved_symbol = head_symbol.is_valid() || symbol.is_valid();
            let parameter_matches = if has_resolved_symbol {
                (head_symbol.is_valid() && head_symbol == parameter.symbol)
                    || (symbol.is_valid() && symbol == parameter.symbol)
            } else {
                name.is_some_and(|name| name == parameter.name.as_str())
            };
            if parameter.is_self {
                if parameter_matches {
                    return None;
                }
                continue;
            }

            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            if parameter_matches {
                return argument;
            }
        }

        None
    }

    fn local_initializer(
        &self,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
        members: psi_arena::HandleSpan<psi_typed_trees::name::Identifier>,
    ) -> Option<ExpressionHandle> {
        let name = self
            .program
            .expression_table
            .name_path_members(members)
            .last()?;
        self.program
            .statement_table
            .statements(self.caller_state.statement_nodes)
            .iter()
            .take(self.statement_index)
            .find_map(|statement| {
                let StatementNode::LocalData(local) = statement else {
                    return None;
                };
                let has_resolved_symbol = head_symbol.is_valid() || symbol.is_valid();
                let local_matches = if has_resolved_symbol {
                    (head_symbol.is_valid() && head_symbol == local.symbol)
                        || (symbol.is_valid() && symbol == local.symbol)
                } else {
                    local.name == *name
                };
                // This evaluator has no flow contexts or storage revisions.
                // Only immutable closed values can be reconstructed here;
                // mutable values and captured reads require live evidence.
                (local_matches && !local.is_mutable && self.closed_value(local.initial_value))
                    .then_some(local.initial_value)
            })
    }

    fn closed_value(&self, expression: ExpressionHandle) -> bool {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return false;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_) => true,
            ExpressionNode::ArrayLiteral(values) => self
                .program
                .expression_table
                .expression_handles(*values)
                .iter()
                .all(|value| self.closed_value(*value)),
            ExpressionNode::StructLiteral(value) => self
                .program
                .expression_table
                .struct_fields(value.fields)
                .iter()
                .all(|field| self.closed_value(field.value)),
            _ => false,
        }
    }
}
