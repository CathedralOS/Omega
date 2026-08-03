use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};

use super::ContractExpressionEvaluator;

impl ContractExpressionEvaluator<'_, '_> {
    pub(super) fn collection_length(&self, expression: ExpressionHandle) -> Option<usize> {
        let resolved = self.resolved_expression(expression).unwrap_or(expression);
        match self.program.expression_table.expression(resolved) {
            ExpressionNode::ArrayLiteral(values) => Some(values.count() as usize),
            ExpressionNode::Mutable(inner) => self.collection_length(*inner),
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
            let psi_typed_trees::data::DataMember::Field(field) = data_member else {
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
    fn target_self_data_definition(&self) -> Option<&psi_typed_trees::data::DataDefinition> {
        let machine = self.program.machines().iter().find(|machine| {
            self.program
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
