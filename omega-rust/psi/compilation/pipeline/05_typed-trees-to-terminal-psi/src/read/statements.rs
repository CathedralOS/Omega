/*
The statement family of the reader: local bindings, locals declared without
an initializer, and assignment. An assignment to a name rebinds its slot (a
scalar value, or the fresh root of a whole record value); an assignment to a
member path is a store into that exact field of a mutable local's record; an
assignment to a scalar local declared without an initializer is a store into
its home. A `mut` formal that holds its own value is assigned and stored
into as a `mut` local is; any other formal that holds its own value is
immutable. A formal passed by `&mut` is stored into in place: a scalar
referent like a home, a field of a record referent like a field of a mutable
local.
*/

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{TableAssignment, TableLocalData};

use super::authority::Access;
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
        let assigned = |program: &Self, value: &crate::Expression, place: ValueType| {
            program.mistyped(
                id,
                value.source,
                super::rejections::Destination::Assigned,
                Some(place),
                value.result,
                "assigned value type",
            )
        };
        let source = SourceOrigin::of(self.typed.expression_table.source_span(assignment.target));
        match self.node(id, assignment.target)? {
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, slots, path)?;
                if !slots[slot].assignable {
                    return Err(unserved("assignment to an immutable binding"));
                }
                // A whole record stored through a reference replaces the
                // caller's storage in place, a store no slice lowers yet; a
                // record local's assignment rebinds its slot instead.
                if slots[slot].borrowed.is_some() && !slots[slot].homed {
                    return Err(unserved("whole assignment through a reference"));
                }
                let value_type = slots[slot].value_type;
                // The value's type first: a value that is not the place's
                // type is wrong whoever may write the place.
                let value = self.expression(id, slots, assignment.value, Some(value_type))?;
                if !self.agrees(value.result, value_type) {
                    return Err(assigned(self, &value, value_type));
                }
                self.authorized(id, value_type, Access::Replace)?;
                Ok(if slots[slot].homed {
                    Statement::Store {
                        destination: crate::Expression {
                            kind: ExpressionKind::Slot(slot),
                            result: Some(value_type),
                            source,
                        },
                        value,
                        source,
                    }
                } else {
                    Statement::Bind { slot, value }
                })
            }
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                // A destination names existing local storage. Retain its
                // expression intact after checking the root's authority;
                // lowering captures its selectors before the replacement.
                let place = self.expression(id, slots, assignment.target, None)?;
                let Some(stored) = place.result else {
                    return Err(typing());
                };
                let root = super::storage_path::walk(
                    &place,
                    |_| Err(unserved("store into a value that is not a local")),
                    |projection, _| {
                        // A payload is stored into through its case's
                        // dispatch binding or a directly authored member
                        // (`s.at = x`); neither store is lowered yet.
                        if matches!(projection.kind, ExpressionKind::Payload { .. }) {
                            return Err(unserved("store into a case payload"));
                        }
                        Ok(())
                    },
                )?;
                let ExpressionKind::Slot(slot) = root.kind else {
                    return Err(unserved("store into a value that is not a local"));
                };
                if !slots[slot].assignable {
                    return Err(unserved("store into an immutable binding"));
                }
                self.written_through(id, &place)?;
                let value = self.expression(id, slots, assignment.value, Some(stored))?;
                if !self.agrees(value.result, stored) {
                    return Err(assigned(self, &value, stored));
                }
                self.authorized(id, stored, Access::Replace)?;
                Ok(Statement::Store {
                    destination: place,
                    value,
                    source,
                })
            }
            ExpressionNode::TypeExpression(_)
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
        if local.reflected_query_origin.is_some() {
            return unserved("reflected query");
        }
        // A selector must keep the value captured before later operands run.
        // Resolution emits an immutable relevant binding; typing must retain
        // its complete carrier. The ordinary binder below evaluates it once
        // and keeps that carrier, including any supported backing. It never
        // turns a reference into its referent's value.
        if local.is_operand_capture {
            if local.role != language_core::LocalBindingRole::Value
                || local.is_mutable
                || local.relevance != language_core::BindingRelevance::Relevant
            {
                return unserved("operand capture without its immutable value binding");
            }
            if !self
                .typed
                .expression_table
                .expression_is_valid(local.initial_value)
                || !self
                    .typed
                    .type_reference_table
                    .contains_type_reference(local.type_reference)
            {
                return unserved("operand capture without its retained initializer and type");
            }
        }
        // Dispatch subjects and saved operands may retain an incoming
        // reference. Capture its storage binding, never the referent value.
        if (local.is_operand_capture
            || local.role == language_core::LocalBindingRole::DispatchSubject)
            && local.type_reference.is_valid()
            && let typed_trees::types::TypeReferenceNode::Reference {
                referee, access, ..
            } = self
                .typed
                .type_reference_table
                .type_reference(local.type_reference)
            && self.view_type(id, local.type_reference)?.is_none()
        {
            if local.is_mutable || local.relevance != language_core::BindingRelevance::Relevant {
                return unserved("reference capture without an immutable relevant binding");
            }
            let value_type = self.value_type(id, *referee)?;
            let value = self.expression(id, slots, local.initial_value, Some(value_type))?;
            let ExpressionKind::Slot(original) = value.kind else {
                return unserved("reference capture without an existing reference binding");
            };
            if slots[original].borrowed != Some(*access) || slots[original].value_type != value_type
            {
                return unserved("reference capture without matching storage and access");
            }
            return Ok((
                Slot {
                    symbol: local.symbol,
                    value_type,
                    assignable: slots[original].assignable,
                    homed: slots[original].homed,
                    erased: false,
                    borrowed: Some(*access),
                },
                Statement::CaptureReference {
                    slot: slots.len(),
                    original,
                },
            ));
        }
        if local.relevance != language_core::BindingRelevance::Relevant {
            return self.erased_local(id, slots, local);
        }
        if !local.initial_value.is_valid() {
            // `let x: T;`: the declared type is all there is. Storage is
            // reserved and nothing is stored, so the slot starts with no
            // content (`Statement::Declare`).
            if !local.type_reference.is_valid() {
                return unserved("uninitialized local without a declared type");
            }
            let value_type = self.local_type(id, local.type_reference)?;
            self.zero_established(id, value_type)?;
            let home = match value_type {
                ValueType::Rigid(_) => return unserved("local of a rigid type"),
                // Its home would be declared with the identity of a
                // primitive type the lowered form has no row for.
                ValueType::Scalar(_, qualification) if !qualification.is_none() => {
                    return unserved("uninitialized local of a qualified type");
                }
                ValueType::Record(_, _, _) => None,
                ValueType::Scalar(..) => Some(
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
                erased: false,
                borrowed: None,
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
            Some(self.local_type(id, local.type_reference)?)
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
        // `let r = reference;` with no declared type: whether the local is
        // a copy of the referent or the reference itself, the typed trees
        // do not say here, and a reference local belongs to a later slice.
        if declared.is_none()
            && let ExpressionKind::Slot(named) = value.kind
            && slots[named].borrowed.is_some()
        {
            return unserved("local bound to a reference without a declared type");
        }
        // domains.md, "Literals and expected types": "An expected type
        // selects the carrier in which an anonymous literal lands. It never
        // inserts a domain cast", and "An annotated `let` is no exception".
        // So `let x: u32 in Wrapping = 5;` binds a plain `u32` to a
        // Wrapping local, which only a cast does.
        if let Some(declared) = declared
            && !self.agrees(Some(value_type), declared)
        {
            return Err(self.mistyped(
                id,
                value.source,
                super::rejections::Destination::Local,
                Some(declared),
                Some(value_type),
                "local initializer type",
            ));
        }
        let slot = Slot {
            symbol: local.symbol,
            value_type,
            assignable: local.is_mutable,
            homed: false,
            erased: false,
            borrowed: None,
        };
        Ok((
            slot,
            Statement::Bind {
                slot: slots.len(),
                value,
            },
        ))
    }

    /// `let x [erased]: T = term;`. The initializer is the term the binding
    /// names (contracts.md, "Explicit erased bindings": "Construction
    /// supplies the erased term"); nothing is evaluated.
    fn erased_local(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        local: &TableLocalData,
    ) -> Checked<(Slot, Statement)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Statement(what)));
        if !local.initial_value.is_valid() {
            return unserved("erased local without an initializer");
        }
        let declared = if local.type_reference.is_valid() {
            Some(self.local_type(id, local.type_reference)?)
        } else {
            None
        };
        if declared.is_some_and(|declared| declared.scalar().is_none()) {
            return unserved("erased local of a data type");
        }
        let term = self.erased_term(
            id,
            slots,
            local.initial_value,
            declared.and_then(ValueType::scalar),
        )?;
        let Some(value_type) = term.result else {
            return unserved("erased local of a data type");
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
            assignable: false,
            homed: false,
            erased: true,
            borrowed: None,
        };
        Ok((
            slot,
            Statement::Witness {
                slot: slots.len(),
                term,
            },
        ))
    }
}
