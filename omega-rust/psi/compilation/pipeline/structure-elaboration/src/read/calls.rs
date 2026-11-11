/*
The call family of the reader: which callee a call names, the static
application and authored selection it must leave unused, and the call itself
with its arguments.
*/

use semantic_vocabulary::MachineId;
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use typed_trees::statement::TableCall;

use super::{Program, Slot, symbol_kind};
use crate::body::{Expression, ExpressionKind};
use crate::{SourceOrigin, Unserved, refuse};

impl Program<'_> {
    /// An authored static machine application that applies nothing: no
    /// static or selected arguments, bindings, lifetimes, inputs, conformance
    /// or contract source. Stage 03 records one for every authored call; one
    /// that applies anything is a generic or static call and is refused.
    fn applies_nothing(
        &self,
        id: MachineId,
        target: SymbolHandle,
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
        machine_arguments: &[typed_trees::expression::StaticMachineArgument],
    ) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if !machine_arguments.is_empty() {
            return unserved("static machine arguments");
        }
        if !application.is_valid() {
            return Ok(());
        }
        let application = self
            .typed
            .expression_table
            .static_machine_application(application);
        if !application.arguments.is_empty() || !application.selected_machine_arguments.is_empty() {
            return unserved("static application arguments");
        }
        if !application.input_selections.is_empty() {
            return unserved("static application input selections");
        }
        if !application.bindings.is_empty() {
            return unserved("static application bindings");
        }
        if !application.lifetime_bindings.is_empty() {
            return unserved("static application lifetime bindings");
        }
        if application.conformance_selection.is_some() {
            return unserved("static application conformance");
        }
        if application.callable_contract_source.is_valid() {
            return unserved("static application contract source");
        }
        // The registered operation of an ordinary call is the callee: its
        // machine symbol or its one state's symbol (stage 04 records the
        // entry state). Any other declaration is a static or captured call.
        let registration = application.registration_operation;
        let owner = match self.typed.symbols.get(target).kind {
            SymbolKind::State => self.typed.symbols.get(target).parent,
            SymbolKind::Machine => target,
            other => {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call(symbol_kind(other)),
                ));
            }
        };
        if registration.is_valid() && registration != target && registration != owner {
            let state = self
                .typed
                .machine_by_symbol(owner)
                .and_then(|machine| self.typed.machine_states.span(machine.states))
                .and_then(|states| match states {
                    [state] => Some(state.symbol),
                    _ => None,
                });
            if state != Some(registration) {
                return unserved("static application registers another declaration");
            }
        }
        Ok(())
    }

    /// A call is direct only when its authored selection, if recorded,
    /// resolved to exactly the called machine, and its call binding, if
    /// recorded, neither rebinds it nor reaches it through another
    /// declaration. All comparisons are of symbols.
    pub(super) fn direct_selection(
        &self,
        id: MachineId,
        target: SymbolHandle,
        selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
        binding: Option<language_semantics::declaration_selection::ResolvedAuthoredCallBinding>,
    ) -> Checked<()> {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if let Some(occurrence) = selection {
            let Some(selected) = self.typed.authored_declaration_selections().get(occurrence)
            else {
                return unserved("authored selection missing from its ledger");
            };
            match selected.target() {
                AuthoredDeclarationSelectionTarget::Resolved(resolved)
                    if resolved.selected_symbol() == target => {}
                AuthoredDeclarationSelectionTarget::Resolved(_) => {
                    return unserved("selection resolved to another declaration");
                }
                AuthoredDeclarationSelectionTarget::Intrinsic(_) => {
                    return unserved("intrinsic selection");
                }
                AuthoredDeclarationSelectionTarget::LateBound(_) => {
                    return unserved("late-bound selection");
                }
            }
        }
        match binding {
            None => Ok(()),
            Some(binding) if binding.selected == target && binding.original == target => Ok(()),
            Some(_) => unserved("rebound call"),
        }
    }

    pub(super) fn call_statement(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCall,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if call.installed_interrupt_operation.is_some()
            || call.static_requirement_dispatch.is_some()
            || call.reflected_field_origin.is_some()
        {
            return unserved("interrupt, requirement dispatch or reflected call");
        }
        self.applies_nothing(
            id,
            call.target_symbol,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        self.direct_selection(id, call.target_symbol, call.authored_call_selection, None)?;
        if call.receiver_root_symbol.is_valid()
            || call.receiver_symbol.is_valid()
            || !call.receiver.is_empty()
        {
            return unserved("receiver call");
        }
        if call.static_machine_parameter.is_valid() || !call.evidence_arguments.is_empty() {
            return unserved("machine parameter or evidence");
        }
        if call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return unserved("suspend or block acknowledgement");
        }
        let arguments = self
            .typed
            .statement_table
            .expression_handles(call.arguments);
        self.call(
            id,
            slots,
            call.target_symbol,
            arguments,
            SourceOrigin::of(call.source_span),
        )
    }

    pub(super) fn call_expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCallExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if call.installed_interrupt_operation.is_some() {
            return unserved("installed interrupt operation");
        }
        self.applies_nothing(
            id,
            call.target_symbol,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        if call.static_requirement_dispatch.is_some() {
            return unserved("static requirement dispatch");
        }
        if call.build_selection.is_valid() {
            return unserved("Build selection");
        }
        self.direct_selection(
            id,
            call.target_symbol,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        if call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
        {
            return unserved("compiler operation call");
        }
        if call.receiver.is_valid()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
        {
            return unserved("receiver, requirement or coercion call");
        }
        if call.static_machine_parameter.is_valid() || !call.evidence_arguments.is_empty() {
            return unserved("machine parameter or evidence");
        }
        if call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return unserved("suspend or block acknowledgement");
        }
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        self.call(id, slots, call.target_symbol, arguments, source)
    }

    pub(super) fn call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
    ) -> Checked<Expression> {
        if !target.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("callee without a symbol"),
            ));
        }
        // A call names its callee's entry state; the machine is that state's
        // symbol parent, and must have exactly that one state.
        let machine = match self.typed.symbols.get(target).kind {
            SymbolKind::Machine => target,
            SymbolKind::State => {
                let parent = self.typed.symbols.get(target).parent;
                let (_, state) = self.declaration(id, parent)?;
                if state.symbol != target {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("state other than the entry"),
                    ));
                }
                parent
            }
            other => {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call(symbol_kind(other)),
                ));
            }
        };
        // The callee's formals are the arguments' expected types, so its
        // signature is read first; reading order is not evaluation order.
        let callee = self.machine(machine)?;
        self.calls.push((id, callee));
        let formals = terminal_check_tables::row(&self.signatures, callee, id, Construct::Machine)?
            .parameters
            .clone();
        if formals.len() != arguments.len() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("argument count"),
            ));
        }
        let mut read = Vec::with_capacity(arguments.len());
        for (argument, formal) in arguments.iter().zip(&formals) {
            read.push(self.expression(id, slots, *argument, formal.scalar())?);
        }
        let signature =
            terminal_check_tables::row(&self.signatures, callee, id, Construct::Machine)?;
        if signature.parameters.len() != read.len()
            || signature
                .parameters
                .iter()
                .zip(&read)
                .any(|(formal, actual)| actual.result != Some(*formal))
        {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("argument types"),
            ));
        }
        Ok(Expression {
            kind: ExpressionKind::Call {
                callee,
                arguments: read,
            },
            result: signature.result,
            source,
        })
    }
}
