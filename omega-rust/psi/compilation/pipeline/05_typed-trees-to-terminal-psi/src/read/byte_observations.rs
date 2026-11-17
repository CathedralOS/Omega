//! Read byte properties through checked Core declarations. Runtime calls form
//! shared loans. Contract calls retain the exact storage being observed and
//! the declaration's complete contract.

use semantic_vocabulary::{ByteSequencePredicate, MachineId, ScalarType};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::{ExpressionHandle, TableCallExpression};

use super::{Program, Slot};
use crate::body::{Expression, ExpressionKind, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

impl Program<'_> {
    fn byte_observation_declaration(
        &self,
        target: SymbolHandle,
    ) -> Option<(SymbolHandle, ByteSequencePredicate)> {
        typed_trees::byte_predicates::byte_observation_declaration(self.typed, target)
    }

    pub(super) fn byte_observation(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        let Some((target, predicate)) = self.byte_observation_declaration(target) else {
            return Ok(None);
        };
        let signature = self.byte_observation_signature(id, target)?;
        let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Call(detail));
        let [argument] = arguments else {
            return Err(unavailable(
                "byte observation needs exactly one shared byte view",
            ));
        };
        let formal = &signature.borrowed[0];
        let declared = signature.parameters[0];
        let free = self.machine_lifetimes(target);
        let mut bound = super::regions::Substitution::new();
        let contextual = self.argument_context(declared, &free, &bound);
        let (mut base, _) = self.lent(id, slots, *argument, formal, contextual)?;
        base.result = self.borrowed_argument_type(id, slots, &base, contextual, formal)?;
        if let ExpressionKind::Borrow { place, .. } = &base.kind {
            let actual = self.loan_regions(id, slots, place)?;
            self.unify_regions(id, &free, formal.regions, actual, &mut bound)?;
        }
        if let Some(actual) = base.result {
            self.unify(id, &free, declared, actual, &mut bound)?;
        }
        let expected = self.substitute(declared, &bound);
        if !self.same_type(base.result, expected) {
            return Err(unavailable(
                "byte observation argument differs from its shared byte-view formal",
            ));
        }
        Ok(Some(Expression {
            call_context: None,
            kind: ExpressionKind::ByteSequencePredicate {
                base: Box::new(base),
                predicate,
            },
            result: Some(ValueType::plain(ScalarType::Boolean)),
            source,
        }))
    }

    /// This is only declaration meaning. The clause reader must still resolve
    /// the supplied argument as readable structural storage in its own scope.
    pub(super) fn byte_observation_clause_call(
        &mut self,
        id: MachineId,
        call: &TableCallExpression,
    ) -> Checked<Option<(ByteSequencePredicate, ExpressionHandle)>> {
        let Some((target, predicate)) = self.byte_observation_declaration(call.target_symbol)
        else {
            return Ok(None);
        };
        let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Contract(detail));
        self.visible_to(id, target)?;
        self.direct_selection(
            id,
            call.target_symbol,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        self.applies_nothing(
            id,
            call.target_symbol,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        if !self.typed.call_has_no_runtime_receiver_for_target(call)
            || !call.evidence_arguments.is_empty()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.build_selection.is_valid()
            || call.static_machine_parameter.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.installed_interrupt_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return Err(unavailable(
                "byte observation has another call operation or runtime receiver",
            ));
        }
        self.byte_observation_signature(id, target)?;
        let [argument] = self
            .typed
            .expression_table
            .expression_handles(call.arguments)
        else {
            return Err(unavailable(
                "byte observation needs exactly one shared byte view",
            ));
        };
        Ok(Some((predicate, *argument)))
    }
}
