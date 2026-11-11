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

impl<'t> Program<'t> {
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
        // Stage 04 closes a static application by generating the machine it
        // selects: the call then targets that generated machine, whose
        // symbol records the declaration the application registered. Such a
        // callee has no parameter left to apply (`machine_shape` refuses one
        // that has), so the application's arguments are already spent.
        let generated = self.typed.symbols.get(target).generated_from;
        if generated.is_valid()
            && generated == application.registration_operation
            && self.selects_implementations_only(generated)
        {
            return Ok(());
        }
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

    /// Whether every generic parameter of `declared`, a machine or its entry
    /// state, is an implementation parameter. Only such a declaration's
    /// generated machines are read: selecting an implementation changes
    /// which machine a call names and nothing else, while a type or value
    /// argument changes the body's types and operations, which is the
    /// generics lane's to serve.
    fn selects_implementations_only(&self, declared: SymbolHandle) -> bool {
        let owner = match self.typed.symbols.get(declared).kind {
            SymbolKind::State => self.typed.symbols.get(declared).parent,
            _ => declared,
        };
        self.typed.machine_by_symbol(owner).is_some_and(|machine| {
            // An implementation parameter is a conformance bound; where
            // the typed trees also list it as a parameter, its kind says so.
            let parameters = self.typed.machine_type_parameters(machine);
            (!parameters.is_empty() || !machine.conformance_bounds.is_empty())
                && parameters.iter().all(|parameter| {
                    parameter.kind == typed_trees::data::TypeParameterKind::Implementation
                })
                && machine.lifetime_parameters.is_empty()
        })
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
        // The author named a declaration; where stage 04 generated the
        // machine a static application of it selects, the call targets that
        // generated machine and the selection still names the declaration.
        let generated = self.typed.symbols.get(target).generated_from;
        let names_target = |symbol: SymbolHandle| {
            symbol == target || (generated.is_valid() && symbol == generated)
        };
        if let Some(occurrence) = selection {
            let Some(selected) = self.typed.authored_declaration_selections().get(occurrence)
            else {
                return unserved("authored selection missing from its ledger");
            };
            match selected.target() {
                AuthoredDeclarationSelectionTarget::Resolved(resolved)
                    if names_target(resolved.selected_symbol()) => {}
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
            Some(binding)
                if names_target(binding.selected) && binding.selected == binding.original =>
            {
                Ok(())
            }
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
        if let Some(dispatch) = &call.static_requirement_dispatch {
            return self.requirement_call(id, slots, call, dispatch, source);
        }
        // In a generic check form the entry's call through an implementation
        // parameter targets that parameter's requirement child. Which
        // requirement the child stands for is the typed trees' own relation
        // (`evidence_requirement_declaration`), never the member's name.
        if self.rigid && id.get() == 1 {
            match self
                .typed
                .evidence_requirement_declaration(call.target_symbol)
            {
                Ok(Some(requirement)) => {
                    return self.rigid_call(id, slots, call, requirement, source);
                }
                Ok(None) => {}
                Err(_) => return unserved("implementation parameter child with no requirement"),
            }
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

    /// A call through an implementation parameter, `Selected::apply(x)`.
    /// The typed trees record the parameter, the requirement and, once a
    /// static application closed the parameter, the member that realizes
    /// it, each as a symbol.
    ///
    /// In a machine stage 04 generated for an application the call targets
    /// that member and is the direct call of it (generics.md: "Each
    /// invocation of a static callable argument becomes a direct selected
    /// call after specialization"). The selection is recorded so a checker
    /// can join this machine to its generic body.
    fn requirement_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCallExpression,
        dispatch: &typed_trees::StaticRequirementDispatch,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if !call.machine_arguments.is_empty()
            || call.build_selection.is_valid()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
            || call.receiver.is_valid()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.static_machine_parameter.is_valid()
            || !call.evidence_arguments.is_empty()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return unserved("requirement dispatch with another call feature");
        }
        if dispatch.contract_signature.is_valid() || !dispatch.requirement.is_valid() {
            return unserved("requirement dispatch through a separate contract signature");
        }
        // Only a call through a parameter: a member named through the
        // implementation itself, `Double::apply(x)`, records no parameter
        // and stays refused.
        if !dispatch.evidence_parameter.is_valid() {
            return unserved("static requirement dispatch");
        }
        // The author named the parameter's requirement child, and nothing
        // rebound it.
        match call.authored_call_binding {
            Some(binding)
                if binding.selected == binding.original
                    && self.typed.symbols.get(binding.selected).parent
                        == dispatch.evidence_parameter => {}
            _ => return unserved("requirement dispatch not bound to its parameter"),
        }
        // The member takes no static argument of its own.
        if call.authored_machine_application.is_valid() {
            let application = self
                .typed
                .expression_table
                .static_machine_application(call.authored_machine_application);
            if !application.arguments.is_empty()
                || !application.selected_machine_arguments.is_empty()
                || !application.input_selections.is_empty()
                || !application.bindings.is_empty()
                || !application.lifetime_bindings.is_empty()
                || application.conformance_selection.is_some()
                || application.callable_contract_source.is_valid()
            {
                return unserved("requirement dispatch with static arguments");
            }
        }
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        if !dispatch.realization_machine.is_valid() {
            return unserved("requirement dispatch with no realization");
        }
        if dispatch.realization_state != call.target_symbol {
            return unserved("requirement dispatch that targets another machine");
        }
        let target = dispatch.realization_machine;
        let read = self.call(id, slots, target, arguments, source)?;
        if let ExpressionKind::Call { callee, .. } = &read.kind {
            self.selections.push(crate::Selected {
                caller: id,
                parameter: dispatch.evidence_parameter,
                declaring_trait: dispatch.declaring_trait,
                requirement: dispatch.requirement,
                implementation: *callee,
            });
        }
        Ok(read)
    }

    /// The entry of a generic check form calling through a rigid
    /// implementation parameter: a direct call of the requirement.
    fn rigid_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCallExpression,
        requirement: &'t typed_trees::signature::StateSignature,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if !call.machine_arguments.is_empty()
            || call.build_selection.is_valid()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.static_machine_parameter.is_valid()
            || !call.evidence_arguments.is_empty()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return unserved("implementation parameter call with another call feature");
        }
        if call.authored_machine_application.is_valid() {
            let application = self
                .typed
                .expression_table
                .static_machine_application(call.authored_machine_application);
            if !application.arguments.is_empty()
                || !application.selected_machine_arguments.is_empty()
                || !application.input_selections.is_empty()
                || !application.bindings.is_empty()
                || !application.lifetime_bindings.is_empty()
                || application.conformance_selection.is_some()
                || application.callable_contract_source.is_valid()
            {
                return unserved("implementation parameter call with static arguments");
            }
        }
        // The author named this child of the parameter, and nothing rebound
        // it.
        match call.authored_call_binding {
            Some(binding)
                if binding.selected == call.target_symbol
                    && binding.original == call.target_symbol => {}
            _ => return unserved("implementation parameter call not bound to its requirement"),
        }
        let callee = self.requirement(requirement)?;
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        self.call_of(id, slots, callee, arguments, source)
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
        self.call_of(id, slots, callee, arguments, source)
    }

    /// The call of `callee`, a machine or a requirement already read.
    fn call_of(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        callee: MachineId,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
    ) -> Checked<Expression> {
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
        if let SourceOrigin::Authored(span) = source
            && let Some(names) = self.names.get_mut(id)
        {
            let shown = arguments
                .iter()
                .map(|argument| self.typed.expression_table.display_name(*argument))
                .collect();
            names.calls.push((span, shown));
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
