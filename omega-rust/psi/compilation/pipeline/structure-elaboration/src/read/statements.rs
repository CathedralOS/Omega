/*
The statement family of the reader: local bindings, locals declared without
an initializer, and assignment. An assignment to a name rebinds its slot (a
scalar value, or the fresh root of a whole record value); an assignment to a
member path is a store into that exact field of a mutable local's record; an
assignment to a scalar local declared without an initializer is a store into
its home. Formals are immutable here: a `mut` formal is refused where the
signature is read, and a store through a `&mut` formal belongs to the
references lane.
*/

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{TableAssignment, TableLocalData};

use super::{Program, Slot};
use crate::body::{ExpressionKind, Statement, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

impl Program<'_> {
    pub(super) fn assignment(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        assignment: &TableAssignment,
    ) -> Checked<Statement> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Statement(what));
        let typing = || {
            refuse(
                id,
                Construct::Machine,
                Unserved::Typing("assigned value type"),
            )
        };
        let source = SourceOrigin::of(self.typed.expression_table.source_span(assignment.target));
        match self.node(id, assignment.target)? {
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, slots, path)?;
                if !slots[slot].assignable {
                    return Err(unserved("assignment to an immutable binding"));
                }
                let value_type = slots[slot].value_type;
                let value = self.expression(id, slots, assignment.value, value_type.scalar())?;
                if value.result != Some(value_type) {
                    return Err(typing());
                }
                Ok(if slots[slot].homed {
                    Statement::Store {
                        slot,
                        path: Vec::new(),
                        value,
                        source,
                    }
                } else {
                    Statement::Bind { slot, value }
                })
            }
            ExpressionNode::Member(member) => {
                // The destination is read as the member path it is, then
                // taken apart into its slot and field positions: a store
                // names storage, so its base must be a local, never a value
                // an operation just produced.
                let place = self.member(id, slots, member, source)?;
                let Some(stored) = place.result else {
                    return Err(typing());
                };
                let mut path = Vec::new();
                let mut at = &place;
                let slot = loop {
                    match &at.kind {
                        ExpressionKind::Field { base, field } => {
                            path.push(*field);
                            at = base;
                        }
                        ExpressionKind::Slot(slot) => break *slot,
                        // A payload is stored into through its case's
                        // dispatch binding or a directly authored member
                        // (`s.at = x`); neither store is lowered yet.
                        ExpressionKind::Payload { .. } => {
                            return Err(unserved("store into a case payload"));
                        }
                        ExpressionKind::Integer(_)
                        | ExpressionKind::Boolean(_)
                        | ExpressionKind::Case { .. }
                        | ExpressionKind::IsCase { .. }
                        | ExpressionKind::Match { .. }
                        | ExpressionKind::Record { .. }
                        | ExpressionKind::Strict { .. }
                        | ExpressionKind::Not(_)
                        | ExpressionKind::BitwiseNot(_)
                        | ExpressionKind::Cast(_)
                        | ExpressionKind::ShortCircuit { .. }
                        | ExpressionKind::Call { .. } => {
                            return Err(unserved("store into a value that is not a local"));
                        }
                    }
                };
                path.reverse();
                if !slots[slot].assignable {
                    return Err(unserved("store into an immutable binding"));
                }
                let value = self.expression(id, slots, assignment.value, stored.scalar())?;
                if value.result != Some(stored) {
                    return Err(typing());
                }
                Ok(Statement::Store {
                    slot,
                    path,
                    value,
                    source,
                })
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
            | ExpressionNode::ZeroValue(_) => Err(unserved("assignment to a projected place")),
        }
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
            // A pattern's marker binds no value: the body driver reads it
            // with `record_pattern` and never as a local.
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
            // `let x: T;`: the declared type is all there is. Storage is
            // reserved and nothing is stored, so the slot starts with no
            // content (`Statement::Declare`).
            if !local.type_reference.is_valid() {
                return unserved("uninitialized local without a declared type");
            }
            let value_type = self.value_type(id, local.type_reference)?;
            let home = match value_type {
                ValueType::Rigid(_) => return unserved("local of a rigid type"),
                ValueType::Record(_) => None,
                ValueType::Scalar(_) => Some(
                    self.typed
                        .normalized_type_identity_text(local.type_reference),
                )
                .filter(|identity| !identity.is_empty())
                .map(Some)
                .ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Spelling("local type without an identity"),
                    )
                })?,
            };
            let slot = Slot {
                symbol: local.symbol,
                value_type,
                assignable: local.is_mutable,
                homed: value_type.scalar().is_some(),
            };
            return Ok((
                slot,
                Statement::Declare {
                    slot: slots.len(),
                    home,
                    source: SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
                },
            ));
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
            homed: false,
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
