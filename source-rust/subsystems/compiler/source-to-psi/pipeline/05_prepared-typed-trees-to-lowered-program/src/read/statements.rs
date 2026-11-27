use crate::read::refuse_at;
/*
The statement family of the reader: local bindings, locals declared without
an initializer, and assignment. An assignment to a name rebinds its slot (a
scalar value, or the fresh root of a whole record value); an assignment to a
member path is a store into that exact field of a mutable local's record; an
assignment to a scalar local declared without an initializer is a store into
its home. A `mutable` formal that holds its own value is assigned and stored
into as a `mutable` local is; any other formal that holds its own value is
immutable. A formal passed by `@mutable` is stored into in place: a scalar
referent like a home, a field of a record referent like a field of a mutable
local.
*/

use crate::source_state::expression::{ExpressionHandle, ExpressionNode, UnaryOperator};
use crate::source_state::statement::{
    StatementHandle, StatementNode, TableAssignment, TableLocalData,
};
use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::StructuralAccess;

use super::authority::Access;
use super::references::reference_access_violation;
use super::{Program, Slot};
use crate::body::{ExpressionKind, Qualification, Statement, ValueType};
use crate::{SourceOrigin, Unserved, reject};

impl Program<'_> {
    pub(super) fn evaluated_expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        expression: ExpressionHandle,
        discards_result: bool,
    ) -> Checked<Statement> {
        let mut expression = self.expression(id, slots, expression, None)?;
        crate::lower::operational::boundary(&mut expression);
        Ok(Statement::Evaluate {
            expression,
            discards_result,
        })
    }

    pub(super) fn call_statement_at(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: StatementHandle,
    ) -> Checked<crate::body::Expression> {
        let StatementNode::Call(call) = self.typed.statement_table.statement(handle) else {
            return Err(refuse_at(
                id,
                Unserved::Statement("call occurrence does not name a call statement"),
            ));
        };
        if call.runtime_call.is_none() {
            self.bound_target(id, call.target_symbol)?;
        }
        let source = self.authored_node(
            id,
            crate::source_state::SourceNode::Statement(handle),
            call.source_span,
        )?;
        let mut called = self.call_statement(id, slots, call, source)?;
        self.cite_theorem(&mut called);
        Ok(called)
    }

    /// A machine that runs cites a theorem over indexed data by calling it.
    /// The theorem does no work, so the call is a fact-only invocation and
    /// emits no call: contracts.md, "Citation and induction": "Establish its
    /// `requires` under exact operand substitution before importing its
    /// `ensures`; erase the call if fact-only." The call becomes a theorem
    /// application that names the theorem's machine. A mathematical body
    /// is read as a proof instead, where a citation is the theorem itself.
    fn cite_theorem(&self, called: &mut crate::body::Expression) {
        use crate::body::{CallTarget, ExpressionKind};
        if self.mathematical {
            return;
        }
        let ExpressionKind::Call {
            callee: CallTarget::Machine(callee),
            required_contract: None,
            static_reach_binding: None,
            static_reach_arguments,
            proof_arguments,
            arguments,
            designated: None,
        } = &mut called.kind
        else {
            return;
        };
        if !static_reach_arguments.is_empty() || !proof_arguments.is_empty() {
            return;
        }
        let theorem = self
            .symbols
            .get(*callee)
            .and_then(|symbol| self.typed.machine_by_own_or_state_symbol(*symbol))
            .is_some_and(|machine| self.is_family_theorem(machine));
        if theorem {
            called.kind = ExpressionKind::TheoremApplication {
                source: terminal_psi::TheoremSource::Named(*callee),
                arguments: std::mem::take(arguments),
            };
        }
    }

    /// A slice formal carries a descriptor for borrowed backing. A whole
    /// assignment needs a stored value to replace. Indexed stores and
    /// assignment to an owned descriptor local have their own destinations.
    fn require_whole_replacement(
        &self,
        id: MachineId,
        slots: &[Slot],
        index: usize,
    ) -> Checked<()> {
        if slots[index].borrowed.is_some()
            && !slots[index].homed
            && !self
                .signatures
                .get(id)
                .and_then(|signature| signature.borrowed(index))
                .is_some_and(|borrowed| borrowed.referent == crate::body::BorrowedReferent::Value)
        {
            return Err(refuse_at(
                id,
                Unserved::Statement("whole assignment through a reference"),
            ));
        }
        Ok(())
    }

    pub(super) fn assignment(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        assignment: &TableAssignment,
    ) -> Checked<Statement> {
        let unserved = |what| refuse_at(id, Unserved::Statement(what));
        let typing = || refuse_at(id, Unserved::Typing("assigned value type"));
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
        if let ExpressionNode::Atomic(atomic) = self.node(id, assignment.value)? {
            return self.atomic_store(id, slots, assignment.target, atomic, source);
        }
        let write_refusal = |slot: &Slot, unsupported| {
            // A borrowed formal's slot names its referent directly, so the
            // path has no explicit descriptor crossing to check its access.
            match (slot.erased, slot.borrowed) {
                (false, Some(language_core::ReferenceAccess::Shared)) => {
                    reference_access_violation(
                        id,
                        source,
                        StructuralAccess::MutableBorrow,
                        StructuralAccess::SharedBorrow,
                    )
                }
                (false, None) => reject(
                    id,
                    source,
                    "writing to an immutable owned binding requires `mutable`",
                ),
                _ => unserved(unsupported),
            }
        };
        match self.node(id, assignment.target)? {
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, slots, path)?;
                if let Some((referent, _)) = self.reference_referent(id, slots[slot].value_type)? {
                    let descriptor = crate::Expression {
                        call_context: None,
                        kind: ExpressionKind::Slot(slot),
                        result: Some(slots[slot].value_type),
                        source,
                    };
                    let destination = crate::Expression {
                        call_context: None,
                        kind: ExpressionKind::Referent(Box::new(descriptor)),
                        result: Some(referent),
                        source,
                    };
                    self.reference_place_access(
                        id,
                        slots,
                        &destination,
                        language_core::ReferenceAccess::Mutable,
                    )?;
                    self.authorized(id, referent, Access::Replace)?;
                    let value = self.expression(id, slots, assignment.value, Some(referent))?;
                    if !value.diverges() && !self.agrees(value.result, Some(referent)) {
                        return Err(assigned(self, &value, referent));
                    }
                    return Ok(Statement::Store {
                        slot_home: None,
                        destination,
                        value,
                        source,
                    });
                }
                if !slots[slot].assignable {
                    return Err(write_refusal(
                        &slots[slot],
                        "assignment to an immutable binding",
                    ));
                }
                self.require_whole_replacement(id, slots, slot)?;
                let value_type = slots[slot].value_type;
                // The value's type first: a value that is not the place's
                // type is wrong whoever may write the place.
                let value = self.expression(id, slots, assignment.value, Some(value_type))?;
                if !value.diverges() && !self.agrees(value.result, Some(value_type)) {
                    return Err(assigned(self, &value, value_type));
                }
                self.authorized(id, value_type, Access::Replace)?;
                // A record local, a homed scalar and a borrowed formal all
                // name storage that loans may still observe. Replace there
                // through the same captured-place store as a field
                // (structural_access.md: "Root, field, and indexed
                // destinations do not change construction semantics"), so
                // the checker sees the write.
                let stored = slots[slot].homed
                    || slots[slot].borrowed.is_some()
                    || matches!(value_type, ValueType::Record(..));
                // An outer scalar written by an arm needs one place visible to
                // every incoming edge, including the edge which skips the write.
                let slot_home = if !stored && self.block_floor.is_some_and(|floor| slot < floor) {
                    let owner =
                        *terminal_check_tables::row(&self.symbols, id, id, Construct::Machine)?;
                    let (_, state) = self.declaration(id, owner)?;
                    let declared = self
                        .typed
                        .declared_binding_type_by_symbol(state, slots[slot].symbol);
                    let storage = match (declared, value_type) {
                        (Some(reference), _) => self.storage_type(id, reference, value_type)?,
                        (None, ValueType::Scalar(scalar, Qualification::NONE)) => {
                            self.plain_scalar_storage(id, scalar)?
                        }
                        _ => return Err(typing()),
                    };
                    Some(self.data_type(id, storage)?.identity.clone())
                } else {
                    None
                };
                Ok(if stored || slot_home.is_some() {
                    Statement::Store {
                        slot_home,
                        destination: crate::Expression {
                            call_context: None,
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
            ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Unary(crate::source_state::expression::TableUnaryExpression {
                operator: crate::source_state::expression::UnaryOperator::Dereference,
                ..
            }) => {
                // A destination names existing local storage. Retain its
                // expression intact after checking the root's authority;
                // lowering captures its selectors before the replacement.
                //
                // A field that holds a reference is the descriptor's place:
                // assigning to it replaces the reference, where naming it as
                // a value would observe its referent (`expressions::observed`).
                // lifetimes.md, "Carried-loan transport": "Assignment
                // evaluates the replacement while old loans remain active,
                // ends the overwritten field's carried loans, then installs
                // the replacement's exact loans."
                let place = self.place(id, slots, assignment.target, None, Access::Replace)?;
                if let ExpressionKind::Slot(slot) = place.kind {
                    self.require_whole_replacement(id, slots, slot)?;
                }
                let Some(stored) = place.result else {
                    return Err(typing());
                };
                // A byte assignment writes through the view, not over its
                // descriptor. Keep the read expression out of the place walk.
                let storage = if let ExpressionKind::ByteElement { base, .. } = &place.kind {
                    let Some(value_type @ ValueType::Record(record, _, _)) = base.result else {
                        return Err(typing());
                    };
                    use crate::StorageShape::ByteSequence;
                    use terminal_psi::{ByteSequenceCarrier::BorrowedView, StructuralAccess};

                    let writable = match self.data_type(id, record)?.shape {
                        ByteSequence(BorrowedView {
                            access: Some(StructuralAccess::MutableBorrow),
                        }) => true,
                        // An incoming direct view draws permission from its formal.
                        ByteSequence(BorrowedView { access: None }) => matches!(
                            base.kind,
                            ExpressionKind::Slot(slot)
                                if slots[slot].borrowed == Some(language_core::ReferenceAccess::Mutable)
                        ),
                        _ => false,
                    };
                    if !writable {
                        return Err(unserved("store through a byte view without mutable access"));
                    }
                    self.authorized(id, value_type, Access::Write)?;
                    base.as_ref()
                } else {
                    &place
                };
                let byte_store = matches!(place.kind, ExpressionKind::ByteElement { .. });
                let root = super::storage_path::walk(
                    storage,
                    |_| Err(unserved("store into a value that is not a local")),
                    // The captured path retains a payload's exact case and
                    // field. Storage checking proves that case at the store;
                    // ancestry itself grants no membership or write authority.
                    |_, _| Ok(()),
                )?;
                let ExpressionKind::Slot(slot) = root.kind else {
                    return Err(unserved("store into a value that is not a local"));
                };
                let crossed = self.reference_place_access(
                    id,
                    slots,
                    storage,
                    language_core::ReferenceAccess::Mutable,
                )?;
                if !crossed && !byte_store && !slots[slot].assignable {
                    return Err(write_refusal(
                        &slots[slot],
                        "store into an immutable binding",
                    ));
                }
                if byte_store
                    && slots[slot].borrowed == Some(language_core::ReferenceAccess::Shared)
                {
                    return Err(unserved("store through shared descriptor storage"));
                }
                self.written_through(id, storage)?;
                let value = self.expression(id, slots, assignment.value, Some(stored))?;
                if !value.diverges() && !self.agrees(value.result, Some(stored)) {
                    return Err(assigned(self, &value, stored));
                }
                self.authorized(id, stored, Access::Replace)?;
                Ok(Statement::Store {
                    slot_home: None,
                    destination: place,
                    value,
                    source,
                })
            }
            ExpressionNode::MachineValue(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::Match(_)
            | ExpressionNode::Block(_)
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
        let unserved = |what| Err(refuse_at(id, Unserved::Statement(what)));
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
            return Err(refuse_at(id, Unserved::Spelling("local without a symbol")));
        }
        if local.reflected_query_origin.is_some() {
            return unserved("reflected query");
        }
        // A selector must keep the value captured before later operands run.
        // Resolution emits an immutable relevant binding. The ordinary
        // binder below evaluates it once and keeps its complete carrier,
        // including any supported backing.
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
            && let crate::source_state::types::TypeReferenceNode::Reference {
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
            // The binding's type is read as a local's annotation is
            // (`local_type`): an elided lifetime there is a region variable
            // the captured reference decides, and the capture keeps that
            // reference's exact type.
            let declared = self.local_type(id, *referee)?;
            // The capture names the reference binding itself. Reading it as
            // a value of the referent's type would be the dereference only
            // `$` may write, so the binding is found by name.
            let ExpressionNode::Name(path) = self.node(id, local.initial_value)? else {
                return unserved("reference capture without an existing reference binding");
            };
            let original = self.slot_of(id, slots, path)?;
            if slots[original].erased {
                return unserved("reference capture of an erased binding");
            }
            let value_type = slots[original].value_type;
            if slots[original].borrowed != Some(*access)
                || !self.same_type(Some(value_type), declared)
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
                    borrowed_regions: slots[original].borrowed_regions,
                },
                Statement::CaptureReference {
                    slot: self.next_slot(slots),
                    original,
                },
            ));
        }
        if local.relevance != language_core::BindingRelevance::Relevant {
            return self.erased_local(id, slots, local);
        }
        if !local.initial_value.is_valid() {
            // `let x: T;` constructs the declared type's zero value. Lowering
            // records that actual initializer, and the checker proves its
            // default-domain obligations before observation. This is not an
            // assumed zero for genuinely uninitialized storage.
            if !local.type_reference.is_valid() {
                return unserved("uninitialized local without a declared type");
            }
            let value_type = self.local_type(id, local.type_reference)?;
            let source = self
                .typed
                .symbols
                .symbol_provenance_source_span(local.symbol)
                .map_or(
                    SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
                    SourceOrigin::of,
                );
            self.zero_established(id, value_type, None, source)?;
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
                    refuse_at(id, Unserved::Spelling("local type without an identity"))
                })?,
            };
            let slot = Slot {
                symbol: local.symbol,
                value_type,
                assignable: local.is_mutable,
                homed: value_type.scalar().is_some(),
                erased: false,
                borrowed: None,
                borrowed_regions: crate::body::Regions::NONE,
            };
            return Ok((
                slot,
                Statement::Declare {
                    slot: self.next_slot(slots),
                    home,
                    source,
                },
            ));
        }
        // Only an authored annotation constrains the supplied value. An
        // inferred binding derives its complete type from the initializer,
        // including regions substituted through an actual parent value.
        let declared = if local.type_reference.is_valid() && !local.type_is_inferred {
            Some(self.local_type(id, local.type_reference)?)
        } else {
            None
        };
        let mut value = if local.type_is_inferred {
            self.inferred_initializer(id, slots, local.initial_value, local.type_reference)?
        } else {
            self.expression(id, slots, local.initial_value, declared)?
        };
        if !local.is_operand_capture {
            crate::lower::operational::boundary(&mut value);
        }
        let Some(value_type) = value
            .result
            .or_else(|| value.diverges().then_some(declared).flatten())
        else {
            return Err(refuse_at(
                id,
                Unserved::Typing("local bound to a Unit call"),
            ));
        };
        // A borrowed formal already addresses its referent, so reading `$v`
        // lowers to the same slot. An inferred binding may copy that value.
        // Without an explicit read, the binding must keep the reference.
        if declared.is_none()
            && let ExpressionKind::Slot(named) = value.kind
            && slots[named].borrowed.is_some()
            && !matches!(self.node(id, local.initial_value)?,
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::Dereference)
        {
            return unserved("local bound to a reference without a declared type");
        }
        // domains.md, "Literals and expected types": "An expected type
        // selects the carrier in which an anonymous literal lands. It never
        // inserts a domain cast", and "An annotated `let` is no exception".
        // So `let x: u32 in Wrapping = 5;` binds a plain `u32` to a
        // Wrapping local, which only a cast does.
        if let Some(declared) = declared
            && !self.agrees(Some(value_type), Some(declared))
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
        // A bottom initializer produces no value, but the declared local
        // keeps its annotation for source checking after the call. Keep
        // existing inferred regions when the ordinary carriers already agree.
        let value_type = declared
            .filter(|declared| declared.erased() != value_type.erased())
            .unwrap_or(value_type);
        // Matching a non-copy field through a borrowed root observes the place.
        // Moving it into a value local would leave a hole in the lender.
        if local.role == language_core::LocalBindingRole::DispatchSubject
            && let ValueType::Record(record, _, _) = value_type
            && !self.data_type(id, record)?.is_copyable()
            && let Some(root) = super::storage_path::root_slot(id, &value, &self.captures)?
            && let Some(access) = slots[root].borrowed
        {
            return Ok((
                Slot {
                    symbol: local.symbol,
                    value_type,
                    assignable: false,
                    homed: false,
                    erased: false,
                    borrowed: Some(access),
                    borrowed_regions: slots[root].borrowed_regions,
                },
                Statement::Observe {
                    slot: self.next_slot(slots),
                    value,
                },
            ));
        }
        let slot = Slot {
            symbol: local.symbol,
            value_type,
            assignable: local.is_mutable,
            homed: false,
            erased: false,
            borrowed: None,
            borrowed_regions: crate::body::Regions::NONE,
        };
        Ok((
            slot,
            Statement::Bind {
                slot: self.next_slot(slots),
                value,
            },
        ))
    }

    /// The slot a new binding takes. Inside a body statement that holds
    /// block values, slots after the body's are numbered in reading order,
    /// so a block local and the statement's own binding never share one.
    pub(super) fn next_slot(&self, slots: &[Slot]) -> usize {
        if self.block_slots.is_empty() {
            slots.len()
        } else {
            self.block_slot_base + self.block_slots.len()
        }
    }

    /// Extend `scope`, the slots of a block being read, with the slots block
    /// values inside it have bound since: they are not visible, by symbol,
    /// but they occupy their numbers.
    fn adopt_block_slots(&self, scope: &mut Vec<Slot>) {
        let start = scope.len().saturating_sub(self.block_slot_base);
        scope.extend(
            self.block_slots
                .iter()
                .skip(start)
                .map(|(slot, _)| slot.clone()),
        );
    }

    /// A block value, `0 -> { let port: u32 = 1; port }`: its statements are
    /// read as a body's are, in a scope that adds each block local after its
    /// own initializer, and then its final value with the expected type of
    /// the arm (`wiki/spec/language/patterns.md`: "a comma separates each arm
    /// from the next, including after a block-valued arm"). The locals end
    /// with the block: no slot list outside it names them, and the arm's
    /// edge out of its dispatch ends their storage.
    pub(super) fn block_value(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        block: &crate::source_state::expression::TableBlockExpression,
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<crate::Expression> {
        let unserved = |what| refuse_at(id, Unserved::Statement(what));
        if slots.len() < self.block_slot_base
            || slots.len() - self.block_slot_base > self.block_slots.len()
        {
            return Err(unserved("a block value outside a body statement"));
        }
        let mut scope = slots.to_vec();
        self.adopt_block_slots(&mut scope);
        let floor = self.block_floor.replace(scope.len());
        let read = self.block_statements(id, &mut scope, block, expected, source);
        self.block_floor = floor;
        read
    }

    fn block_statements(
        &mut self,
        id: MachineId,
        scope: &mut Vec<Slot>,
        block: &crate::source_state::expression::TableBlockExpression,
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<crate::Expression> {
        use crate::body::ArmGuard;
        use crate::publication::selection_questions::TransitionRead;

        let unserved = |what| refuse_at(id, Unserved::Statement(what));
        let typed = self.typed;
        let machine_symbol =
            *terminal_check_tables::row(&self.symbols, id, id, Construct::Machine)?;
        let machine_result =
            terminal_check_tables::row(&self.signatures, id, id, Construct::Machine)?.result;
        let block_statements = typed.statement_table.statements(block.statements);
        let mut statements = Vec::with_capacity(block_statements.len());
        let mut completed = false;
        let mut statically_completed = false;
        let static_truths_len = self.static_truths.len();
        let mut position = 0;
        while let Some(statement) = block_statements.get(position) {
            let handle = block.statements.handle_at(
                u32::try_from(position)
                    .map_err(|_| unserved("statement position exceeds its source arena"))?,
            );
            if statically_completed && let StatementNode::Transition(transition) = statement {
                let read = match transition.guard {
                    crate::source_state::statement::TransitionGuardNode::When(guard)
                        if matches!(
                            self.read_static_truth(
                                id,
                                machine_symbol,
                                guard,
                                super::types::Reading::Value,
                            )?,
                            Some(super::types::TypeTruth::Known(_))
                        ) =>
                    {
                        TransitionRead::Guard
                    }
                    _ => TransitionRead::None,
                };
                self.completed_operators
                    .record_transition(machine_symbol, handle, read);
                position += 1;
                continue;
            }
            if completed {
                self.static_truths.truncate(static_truths_len);
                return Err(refuse_at(
                    id,
                    Unserved::Completion("statement after an unconditional arm"),
                ));
            }
            let read = match statement {
                StatementNode::Assignment(assignment) => self.assignment(id, scope, assignment)?,
                StatementNode::Call(call) => Statement::Evaluate {
                    expression: self.call_statement_at(id, scope, handle)?,
                    discards_result: call.discards_result,
                },
                StatementNode::Evaluate {
                    expression,
                    discards_result,
                } => self.evaluated_expression(id, scope, *expression, *discards_result)?,
                StatementNode::Expression(expression) => {
                    self.evaluated_expression(id, scope, *expression, false)?
                }
                StatementNode::LocalData(local)
                    if local.role == language_core::LocalBindingRole::RecordPattern =>
                {
                    match self.record_pattern(id, scope, local)? {
                        Some(statement) => statement,
                        None => {
                            position += 1;
                            continue;
                        }
                    }
                }
                StatementNode::LocalData(local)
                    if local.role == language_core::LocalBindingRole::AtomicObservedResult =>
                {
                    self.static_truths.truncate(static_truths_len);
                    return Err(unserved("an atomic observed result inside a block value"));
                }
                StatementNode::LocalData(local) => {
                    let (slot, statement) = self.local(id, scope, local).map_err(|refusal| {
                        self.read_failure_at(refusal, local.symbol, "the local binding")
                    })?;
                    if !local.is_mutable
                        && let Some(truth) = self.static_truth(local.initial_value)
                    {
                        self.static_truths.push((local.symbol, truth));
                    }
                    self.adopt_block_slots(scope);
                    scope.push(slot.clone());
                    self.block_slots
                        .push((slot, local.name.as_str().to_owned()));
                    statement
                }
                StatementNode::Transition(transition) if self.case_arm(transition) => {
                    let (dispatch, taken) = self.dispatch(
                        id,
                        scope,
                        block.statements.handles().skip(position),
                        machine_result,
                    )?;
                    for arm in block.statements.handles().skip(position).take(taken) {
                        self.completed_operators.record_transition(
                            machine_symbol,
                            arm,
                            TransitionRead::GuardAndTarget,
                        );
                    }
                    completed = true;
                    position += taken.saturating_sub(1);
                    dispatch
                }
                StatementNode::Transition(transition) => {
                    let decided = match transition.guard {
                        crate::source_state::statement::TransitionGuardNode::When(guard) => {
                            self.static_truth(guard)
                        }
                        crate::source_state::statement::TransitionGuardNode::Always => None,
                    };
                    self.completed_operators.record_transition(
                        machine_symbol,
                        handle,
                        if decided == Some(false) {
                            TransitionRead::Guard
                        } else {
                            TransitionRead::GuardAndTarget
                        },
                    );
                    if decided == Some(false) {
                        self.transition_predicate(id, scope, transition)?;
                        position += 1;
                        continue;
                    }
                    let (guard, target) = self.transition(id, scope, handle, machine_result)?;
                    let guard = match (decided, guard) {
                        (Some(true), _) => {
                            statically_completed = true;
                            ArmGuard::Always
                        }
                        (_, None) => ArmGuard::Always,
                        (_, Some(condition)) if transition.ends_dispatch => {
                            ArmGuard::Final(condition)
                        }
                        (_, Some(condition)) => ArmGuard::When(condition),
                    };
                    completed = guard.closes_dispatch();
                    Statement::Arm {
                        guard,
                        target,
                        source: SourceOrigin::of(transition.source_span),
                    }
                }
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {
                    self.static_truths.truncate(static_truths_len);
                    return Err(unserved("an assembly statement inside a block value"));
                }
            };
            self.adopt_block_slots(scope);
            statements.push(read);
            position += 1;
        }
        let value = if self.typed.expression_table.expression_is_valid(block.tail) {
            if completed {
                self.static_truths.truncate(static_truths_len);
                return Err(refuse_at(
                    id,
                    Unserved::Completion("statement after an unconditional arm"),
                ));
            }
            Some(Box::new(self.expression(id, scope, block.tail, expected)?))
        } else {
            None
        };
        self.static_truths.truncate(static_truths_len);
        Ok(crate::Expression {
            call_context: None,
            result: value.as_ref().and_then(|value| value.result),
            kind: ExpressionKind::Block { statements, value },
            source,
        })
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
        let unserved = |what| Err(refuse_at(id, Unserved::Statement(what)));
        if !local.initial_value.is_valid() {
            return unserved("erased local without an initializer");
        }
        // An erased local of data indexed by values is evidence. Its
        // initializer is the mathematical signature's to read; here the
        // local is a witness nothing reads (`families::evidence_witness`).
        if local.type_reference.is_valid() && self.names_indexed_data(local.type_reference) {
            if let Some(&owner) = self.symbols.get(id) {
                self.complete_index_operators(id, owner, local.type_reference)?;
            }
            let value_type = super::families::evidence_witness();
            let slot = Slot {
                symbol: local.symbol,
                value_type,
                assignable: false,
                homed: false,
                erased: true,
                borrowed: None,
                borrowed_regions: crate::body::Regions::NONE,
            };
            let term = crate::body::Expression {
                call_context: None,
                kind: crate::body::ExpressionKind::Boolean(true),
                result: Some(value_type),
                source: SourceOrigin::of(
                    self.typed.expression_table.source_span(local.initial_value),
                ),
            };
            return Ok((
                slot,
                Statement::Witness {
                    slot: self.next_slot(slots),
                    term,
                },
            ));
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
            return Err(refuse_at(id, Unserved::Typing("local initializer type")));
        }
        let slot = Slot {
            symbol: local.symbol,
            value_type,
            assignable: false,
            homed: false,
            erased: true,
            borrowed: None,
            borrowed_regions: crate::body::Regions::NONE,
        };
        Ok((
            slot,
            Statement::Witness {
                slot: self.next_slot(slots),
                term,
            },
        ))
    }
}
