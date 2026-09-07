use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};

use super::ContractExpressionEvaluator;

#[cfg(test)]
mod tests;

impl ContractExpressionEvaluator<'_, '_> {
    pub(super) fn collection_length(&self, expression: ExpressionHandle) -> Option<usize> {
        let resolved = self.resolved_expression(expression).unwrap_or(expression);
        match self.program.expression_table.expression(resolved) {
            ExpressionNode::ArrayLiteral(values) => Some(values.count() as usize),
            ExpressionNode::Borrow(inner) => self.collection_length(inner.target),
            // A member that does not resolve to a literal may still be a
            // fixed-array FIELD of the callee's self type (`self.items` with
            // `items: [T; N]`): its extent is pinned by the declared type,
            // independent of which receiver the contract is instantiated
            // against, so `.len` folds to the static length (the contract
            // evaluator's counterpart of the ranges lane's
            // `fixed_array_field_lengths` vocabulary).
            ExpressionNode::Member(member) => self.fixed_array_self_field_length(member),
            ExpressionNode::Name(_) => self.immutable_fixed_array_view_length(resolved),
            _ => None,
        }
    }

    /// A fixed-array view captures a type extent, not an array-content snapshot.
    /// Follow only an immutable descriptor whose binding is still unexposed at
    /// this occurrence; general initializer evaluation remains forbidden.
    fn immutable_fixed_array_view_length(&self, expression: ExpressionHandle) -> Option<usize> {
        use typed_trees::statement::{StatementNode, TransitionGuardNode};

        let ExpressionNode::Name(path) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if !path.symbol.is_valid()
            || path.head_symbol != path.symbol
            || self
                .program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return None;
        }
        let statements = self
            .program
            .statement_table
            .statements(self.caller_state.statement_nodes);
        // This first length-only route has no effects between the prefix and
        // its actuals: neither a guard nor an earlier argument can rebind a view.
        let StatementNode::Transition(transition) = statements.get(self.statement_index)? else {
            return None;
        };
        if !matches!(transition.guard, TransitionGuardNode::Always)
            || !matches!(self.call_site, crate::CallSite::TransitionNamed { .. })
            || crate::call_site_argument_expressions(self.program, self.call_site)
                .iter()
                .any(|argument| {
                    !matches!(
                        self.program.expression_table.expression(*argument),
                        ExpressionNode::Name(_)
                            | ExpressionNode::Integer(_)
                            | ExpressionNode::Boolean(_)
                    )
                })
        {
            return None;
        }
        let mut locals = statements[..self.statement_index]
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == path.symbol => Some(local),
                _ => None,
            });
        let local = locals.next()?;
        if local.is_mutable
            || locals.next().is_some()
            || !validation::CallFrameResolver::new(self.program)?
                .expression_reference_bindings_are_stable(self.caller_machine, expression)
        {
            return None;
        }
        let ExpressionNode::Call(call) = self
            .program
            .expression_table
            .expression(local.initial_value)
        else {
            return None;
        };
        if !matches!(call.target.as_str(), "as_slice" | "as_mut_slice")
            || call.target_symbol.is_valid()
            || !call.arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || !call.machine_arguments.is_empty()
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
        {
            return None;
        }
        let receiver_type = validation::declared_place_type_raw(
            self.program,
            self.caller_machine,
            Some(self.caller_state),
            call.receiver,
        )?;
        crate::checks::ranges::fixed_array_type_length(self.program, receiver_type)
    }

    /// The declared fixed-array extent of `self.<field>` in the TARGET
    /// machine's contract, when the field's type is `[T; N]` with a literal
    /// `N`. Contract expressions carry no resolved member symbols, so the
    /// field is found by name inside the callee's attached data type — never
    /// by a global name scan, which could alias a same-named field of another
    /// type.
    fn fixed_array_self_field_length(&self, member: &TableMemberExpression) -> Option<usize> {
        // Only `self.<field>` receivers: the self type is the one type the
        // callee contract pins statically.
        let ExpressionNode::Name(path) = self.program.expression_table.expression(member.receiver)
        else {
            return None;
        };
        let receiver_is_self = self
            .program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|name| name.as_str() == "self");
        if !receiver_is_self {
            return None;
        }

        let data = self.target_self_data_definition()?;
        for data_member in self.program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = data_member else {
                continue;
            };
            if field.name == member.member {
                return crate::checks::ranges::fixed_array_type_length(
                    self.program,
                    field.type_reference,
                );
            }
        }
        None
    }

    /// The data definition the TARGET machine is attached to (`Vec4` for
    /// `machine Vec4::get`), found through the machine that owns the target
    /// state. `None` for free machines (no attached data).
    fn target_self_data_definition(&self) -> Option<&typed_trees::data::DataDefinition> {
        let machine = self.program.machines().iter().find(|machine| {
            machine.symbol == self.target_symbol
                || self
                    .program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == self.target_symbol)
        })?;
        let attached_data = machine.attached_data.as_ref()?;
        self.program
            .data_definitions()
            .iter()
            .find(|data| data.name == *attached_data)
    }
}
