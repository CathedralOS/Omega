//! Structural queries and snapshots of the current typed program.

pub mod callees;
pub mod snapshot;

impl crate::TypedTrees {
    /// Display captured operands through their exact generated declarations.
    /// Authored locals keep their names: their initializer need not describe
    /// the current value. Generated immutable captures retain the expression
    /// evaluated at their formation, even through nested operand captures.
    /// This is presentation only, never a proof alias or declaration selection.
    /// Source spans locate tokens and cannot substitute for expression syntax.
    pub fn diagnostic_expression_name(
        &self,
        expression: crate::expression::ExpressionHandle,
    ) -> String {
        self.expression_table
            .display_name_resolving(expression, &|mut current| {
                // A well-formed capture chain is acyclic. Bound diagnostic walking
                // even for an incomplete program instead of requiring its validation.
                for _ in 0..self.expression_table.expression_count() {
                    let Some(path) = self.expression_table.name_path_node(current) else {
                        break;
                    };
                    if !path.symbol.is_valid() {
                        break;
                    }
                    let authored = self.symbols.authored_origin(path.symbol);
                    if self.symbols.get(authored).kind == symbols::SymbolKind::Local
                        && self.symbols.symbol_source_span(authored).is_some()
                    {
                        break;
                    }
                    let Some(state) = self.state_by_symbol(self.symbols.get(path.symbol).parent)
                    else {
                        break;
                    };
                    let Some((_, local)) = self.state_local_declaration(state, path.symbol) else {
                        break;
                    };
                    if local.is_mutable || !local.initial_value.is_valid() {
                        break;
                    }
                    current = local.initial_value;
                }
                current
            })
    }
}
