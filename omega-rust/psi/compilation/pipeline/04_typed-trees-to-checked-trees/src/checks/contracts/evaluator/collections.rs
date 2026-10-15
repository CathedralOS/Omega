use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};

use super::ContractExpressionEvaluator;

impl<'program> ContractExpressionEvaluator<'program, '_> {
    pub(super) fn collection_length(&mut self, expression: ExpressionHandle) -> Option<usize> {
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
            _ => None,
        }
    }

    /// The declared fixed-array extent of `self.<field>` in the TARGET
    /// machine's contract, when the field's type is `[T; N]` with a literal
    /// `N`. Contract expressions carry no resolved member symbols, so the
    /// field is found by name inside the callee's attached data type — never
    /// by a global name scan, which could alias a same-named field of another
    /// type.
    fn fixed_array_self_field_length(&mut self, member: &TableMemberExpression) -> Option<usize> {
        // Only `self.<field>` receivers: the self type is the one type the
        // callee contract pins statically.
        let Some(path) = self
            .program
            .expression_table
            .name_path_node(member.receiver)
        else {
            return None;
        };
        let receiver_is_self = self
            .program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|name| name.is_self_receiver());
        if !receiver_is_self {
            return None;
        }

        // The receiver's `self` name resolves to the machine that owns the
        // expression, not necessarily the call target: a substituted argument
        // keeps the CALLER's receiver (`&mut self.control` under a boundary
        // read is the caller's own field), while an authored contract
        // `self.<field>` stays the callee's. Follow the resolved symbol; only
        // an unresolved receiver (a trait boundary `self` with no machine)
        // falls back to the target's data.
        let data = self
            .program
            .machine_by_symbol(path.symbol)
            .or_else(|| self.program.machine_by_symbol(path.head_symbol))
            .map(|machine| self.self_data_definition(machine))
            .unwrap_or_else(|| self.target_self_data_definition())?;
        for data_member in self.program.data_members(data) {
            let Some(field) = data_member.as_field() else {
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
    fn target_self_data_definition(
        &mut self,
    ) -> Option<&'program typed_trees::data::DataDefinition> {
        let machine = self
            .program
            .machine_by_symbol(self.target_symbol)
            .or_else(|| {
                validation::find_state(self.program, self.target_symbol).map(|(machine, _)| machine)
            })?;
        self.self_data_definition(machine)
    }

    /// The data definition a machine is attached to. `None` for free machines
    /// (no attached data).
    fn self_data_definition(
        &mut self,
        machine: &typed_trees::machine::Machine,
    ) -> Option<&'program typed_trees::data::DataDefinition> {
        let attached_data = machine.attached_data.as_ref()?;
        self.program.data_definition_by_name(attached_data)
    }
}
