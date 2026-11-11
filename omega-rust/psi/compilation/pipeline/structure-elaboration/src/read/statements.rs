/*
The statement family of the reader: local bindings and assignment to scalar
locals. A record local is bound once by its initializer; storing into a field
of it and replacing it whole are refused here until the slice that serves
them.
*/

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{TableAssignment, TableLocalData};

use super::{Program, Slot};
use crate::body::{Statement, ValueType};
use crate::{Unserved, refuse};

impl Program<'_> {
    pub(super) fn assignment(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        assignment: &TableAssignment,
    ) -> Checked<Statement> {
        let path = match self.node(id, assignment.target)? {
            ExpressionNode::Name(path) => path,
            ExpressionNode::Member(_) => {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Statement("field store"),
                ));
            }
            ExpressionNode::Indexed(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::Match(_)
            | ExpressionNode::SavedSubject(_)
            | ExpressionNode::ArrayLiteral(_)
            | ExpressionNode::Atomic(_)
            | ExpressionNode::Binary(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Call(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Borrow(_)
            | ExpressionNode::Range(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::String(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::ZeroValue(_) => {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Statement("assignment to a projected place"),
                ));
            }
        };
        let slot = self.slot_of(id, slots, path)?;
        let ValueType::Scalar(scalar_type) = slots[slot].value_type else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Statement("whole assignment of a record"),
            ));
        };
        if !slots[slot].assignable {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Statement("assignment to an immutable binding"),
            ));
        }
        let value = self.expression(id, slots, assignment.value, Some(scalar_type))?;
        if value.result != Some(ValueType::Scalar(scalar_type)) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("assigned value type"),
            ));
        }
        Ok(Statement::Bind { slot, value })
    }

    pub(super) fn local(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        local: &TableLocalData,
    ) -> Checked<(Slot, Statement)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Statement(what)));
        match local.role {
            language_core::LocalBindingRole::Value
            | language_core::LocalBindingRole::DispatchSubject => {}
            language_core::LocalBindingRole::AtomicObservedResult => {
                return unserved("atomic observed result");
            }
            language_core::LocalBindingRole::RecordPattern => {
                return unserved("record pattern binding");
            }
            language_core::LocalBindingRole::ArmPattern => return unserved("arm pattern binding"),
            language_core::LocalBindingRole::DispatchObservation => {
                return unserved("dispatch observation");
            }
        }
        if !local.symbol.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("local without a symbol"),
            ));
        }
        if local.is_operand_capture || local.reflected_query_origin.is_some() {
            return unserved("operand capture or reflected query");
        }
        if local.relevance != language_core::BindingRelevance::Relevant {
            return unserved("erased local");
        }
        if !local.initial_value.is_valid() {
            return unserved("uninitialized local");
        }
        // A declared type is the initializer's expected type; without one the
        // local takes the initializer's type.
        let declared = if local.type_reference.is_valid() {
            Some(self.value_type(id, local.type_reference)?)
        } else {
            None
        };
        let value = self.expression(
            id,
            slots,
            local.initial_value,
            declared.and_then(ValueType::scalar),
        )?;
        let Some(value_type) = value.result else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("local bound to a Unit call"),
            ));
        };
        if declared.is_some_and(|declared| declared != value_type) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("local initializer type"),
            ));
        }
        let slot = Slot {
            symbol: local.symbol,
            value_type,
            assignable: local.is_mutable,
        };
        Ok((
            slot,
            Statement::Bind {
                slot: slots.len(),
                value,
            },
        ))
    }
}
