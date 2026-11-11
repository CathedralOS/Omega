/*
The statement family of the reader: local bindings and assignment to them.
*/

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{TableAssignment, TableLocalData};

use super::{Program, Slot};
use crate::body::Statement;
use crate::{Unserved, refuse};

impl Program<'_> {
    pub(super) fn assignment(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        assignment: &TableAssignment,
    ) -> Checked<Statement> {
        let ExpressionNode::Name(path) = self.node(id, assignment.target)? else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Statement("assignment to a projected place"),
            ));
        };
        let slot = self.slot_of(id, slots, path)?;
        if !slots[slot].assignable {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Statement("assignment to an immutable binding"),
            ));
        }
        let value = self.expression(id, slots, assignment.value, Some(slots[slot].scalar_type))?;
        if value.result != Some(slots[slot].scalar_type) {
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
            Some(self.scalar_type(id, local.type_reference)?)
        } else {
            None
        };
        let value = self.expression(id, slots, local.initial_value, declared)?;
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
            scalar_type: value_type,
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
