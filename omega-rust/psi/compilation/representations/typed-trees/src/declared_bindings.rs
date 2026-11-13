/*
We answer what type the declaration of the binding a name denotes carries
inside one state (`declared_binding_type`).
*/

use crate::TypedTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::state::State;
use crate::statement::StatementNode;
use crate::types::TypeReferenceHandle;
use symbols::SymbolHandle;

impl TypedTrees {
    /// The type on the declaration of the binding `name` denotes: a
    /// parameter of `state` or a local of it. `None` for anything that is
    /// not one plain name of such a binding: a field path, any other
    /// expression, a name of another declaration, and a local whose
    /// declaration carries no type.
    ///
    /// This reads declarations only. It infers nothing, so a caller that
    /// gets `None` has no type and must not substitute one.
    pub fn declared_binding_type(
        &self,
        state: &State,
        name: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        if !self.expression_table.expression_is_valid(name) {
            return None;
        }
        let ExpressionNode::Name(path) = self.expression_table.expression(name) else {
            return None;
        };
        let symbol = path.resolved_symbol()?;
        if self.expression_table.name_path_members(path.members).len() != 1 {
            return None;
        }
        self.declared_binding_type_by_symbol(state, symbol)
    }

    /// Read the unique parameter or local declaration with this exact binder
    /// in the current state. This supplies metadata without authorizing access.
    pub fn declared_binding_type_by_symbol(
        &self,
        state: &State,
        symbol: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        if !self.symbols.symbols().nodes().is_valid(symbol) {
            return None;
        }
        let mut declarations = self
            .state_parameters(state)
            .iter()
            .filter(|parameter| parameter.symbol == symbol)
            .map(|parameter| parameter.type_reference)
            .chain(
                self.statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .filter_map(StatementNode::as_local_data)
                    .filter(|binding| binding.symbol == symbol)
                    .map(|binding| binding.type_reference),
            );
        let declared = declarations.next()?;
        (declarations.next().is_none()
            && self.type_reference_table.contains_type_reference(declared))
        .then_some(declared)
    }
}
