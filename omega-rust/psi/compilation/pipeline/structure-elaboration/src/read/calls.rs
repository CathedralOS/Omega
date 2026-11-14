/*
The call family of the reader: which callee a call names, the static
application and authored selection it must leave unused, and the call itself
with its arguments.
*/

use crate::body::Qualification;
use language_core::ReferenceAccess;
use semantic_vocabulary::MachineId;
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::references::StaticArgumentCall;
use typed_trees::statement::TableCall;

use super::expressions::names_storage;
use super::{Program, Slot, symbol_kind};
use crate::body::{Expression, ExpressionKind, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

impl<'t> Program<'t> {
    /// Read a bodyless boundary through the same formal and contract owner as
    /// a machine. Its distinct target identity prevents a source declaration
    /// from standing in for an executable provider body.
    fn boundary(
        &mut self,
        caller: MachineId,
        target: SymbolHandle,
    ) -> Checked<semantic_vocabulary::BoundaryMachineId> {
        if let Some((boundary, _)) = self
            .boundary_symbols
            .iter()
            .find(|(_, symbol)| **symbol == target)
        {
            return Ok(boundary);
        }
        let unserved = |detail| refuse(caller, Construct::Machine, Unserved::Call(detail));
        let (owner, requirement) = self
            .typed
            .trait_machine_signature(target)
            .filter(|(owner, requirement)| owner.is_boundary && requirement.symbol == target)
            .ok_or_else(|| unserved("target has no exact boundary requirement declaration"))?;
        if !owner.type_parameters.is_empty()
            || !owner.lifetime_parameters.is_empty()
            || !owner.conformance_bounds.is_empty()
        {
            return Err(unserved(
                "boundary requirement needs its complete applied declaration",
            ));
        }
        if owner.refines.is_some() || !owner.refinement_clauses.is_empty() {
            return Err(unserved(
                "boundary contract needs its complete inherited refinement correspondence",
            ));
        }
        // The typed trees own which requirement is the canonical process
        // exit (process_exit.md: "Psi recognizes the exact admitted
        // canonical requirement, including package and contract identity; a
        // matching path, method spelling, or ordinary user declaration does
        // not acquire its meaning"). The signature carries that fact to the
        // one call path every boundary call takes.
        let completion =
            typed_trees::boundary_completions::boundary_requirement_completion(self.typed, target);
        self.visible_to(caller, target)?;
        let (mut signature, _) = self.requirement_signature(caller, requirement, &[])?;
        signature.termination = match &requirement.termination_guarantee {
            language_semantics::TerminationGuarantee::NoGuarantee => crate::Termination::Silent,
            language_semantics::TerminationGuarantee::Terminates { premises }
                if premises.is_empty() =>
            {
                crate::Termination::Promised
            }
            language_semantics::TerminationGuarantee::Terminates { .. } => {
                return Err(unserved(
                    "boundary termination needs its actual progress premises",
                ));
            }
        };
        // effects.md: "Every boundary-trait machine requirement must declare
        // that ceiling, using `reaches none` when it permits no services".
        if signature.reaches == crate::Ceiling::Inferred {
            return Err(unserved(
                "boundary requirement must declare its reach ceiling",
            ));
        }
        // effects.md, "Open callable contracts": "A boundary requirement and
        // a runtime callable type must write the clause", with `crashes none`
        // for no crash route. It has no body to infer from and no static
        // selection to defer to.
        if matches!(signature.crashes, crate::CrashCeiling::Inferred) {
            return Err(unserved(
                "boundary requirement must declare its crash ceiling",
            ));
        }
        let service = self.declared_service(caller, owner)?;
        let boundary = self.boundary_symbols.push(target);
        let signature_id = self.boundary_signatures.push(signature);
        let service_id = self.boundary_services.push(service);
        let completion_id = self.boundary_completions.push(completion);
        if boundary != signature_id || boundary != service_id || boundary != completion_id {
            return Err(unserved(
                "boundary signature lost its exact declaration coordinate",
            ));
        }
        Ok(boundary)
    }

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
            && self.closes_rigid_parameters_only(generated)
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
        let owner = if self.typed.trait_machine_signature(target).is_some() {
            target
        } else {
            match self.typed.symbols.get(target).kind {
                SymbolKind::State => self.typed.symbols.get(target).parent,
                SymbolKind::Machine => target,
                other => {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call(symbol_kind(other)),
                    ));
                }
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
    /// state, is an implementation, value or type parameter. Only such a
    /// declaration's generated machines are read. Each is a closed machine:
    /// selecting an implementation changes which machine a call names,
    /// binding a value parameter puts a literal where the body named the
    /// binder, and a type argument puts a concrete type where it named the
    /// parameter. What such a machine still holds that the subset does not
    /// serve (a type equation in a clause, for one) is refused where it is
    /// read.
    fn closes_rigid_parameters_only(&self, declared: SymbolHandle) -> bool {
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
                    matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Implementation
                            | typed_trees::data::TypeParameterKind::Const { .. }
                            | typed_trees::data::TypeParameterKind::Type
                    )
                })
                && machine.lifetime_parameters.is_empty()
        })
    }

    /// The machine a symbol enters: a machine itself, or the machine whose
    /// one state the symbol is. `None` for anything else, including a state
    /// of a machine with several.
    fn entered_machine(&self, symbol: SymbolHandle) -> Option<SymbolHandle> {
        match self.typed.symbols.get(symbol).kind {
            SymbolKind::Machine => Some(symbol),
            SymbolKind::State => {
                let parent = self.typed.symbols.get(symbol).parent;
                let machine = self.typed.machine_by_symbol(parent)?;
                match self.typed.machine_states.span(machine.states)? {
                    [state] if state.symbol == symbol => Some(parent),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// A declaration qualifier only completes the callee's name; it is not
    /// an implicit receiver or a source of arguments. Both a trait requirement
    /// and a machine attached under a data declaration keep that same rule:
    /// `Counter::clear(&mut counter)` names the machine `Counter::clear` and
    /// passes it one argument. machines.md, "Static declarations and
    /// explicit state": "Every argument, including context and
    /// capabilities, is ordinary and explicit." The two are compared as
    /// symbols, and the qualifier is the declaration itself, never a
    /// value of the type, so no receiver is passed. Any other qualifier or
    /// receiver stays refused.
    pub(super) fn names_carrier(&self, target: SymbolHandle, qualifier: SymbolHandle) -> bool {
        if !qualifier.is_valid() || !target.is_valid() {
            return false;
        }
        if let Some((owner, requirement)) = self.typed.trait_machine_signature(target) {
            return owner.is_boundary && requirement.symbol == target && owner.symbol == qualifier;
        }
        self.entered_machine(target)
            .and_then(|machine| self.typed.machine_by_symbol(machine))
            .is_some_and(|machine| machine.attached_data_symbol == qualifier)
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
                // The selection names the callee's machine or its one
                // entry state, and so does the call; a call of the machine
                // being read records the two differently.
                AuthoredDeclarationSelectionTarget::Resolved(resolved)
                    if names_target(resolved.selected_symbol())
                        || self
                            .entered_machine(resolved.selected_symbol())
                            .is_some_and(|selected| {
                                Some(selected) == self.entered_machine(target)
                            }) => {}
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

    /// A call the typed trees bound to no declaration is refused before
    /// anything asks what its target is: an unbound symbol has no kind,
    /// parent or generated origin to read. Typing leaves such a call unbound
    /// when the callee is chosen from a receiver's type or the name resolves
    /// to nothing.
    fn bound_target(&self, id: MachineId, target: SymbolHandle) -> Checked<()> {
        if target.is_valid() && self.typed.symbols.symbols().nodes().is_valid(target) {
            return Ok(());
        }
        Err(refuse(
            id,
            Construct::Machine,
            Unserved::Call("without a bound target"),
        ))
    }

    pub(super) fn call_statement(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCall,
    ) -> Checked<Expression> {
        self.bound_target(id, call.target_symbol)?;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if let Some(read) = self.rigid_call(
            id,
            slots,
            StaticArgumentCall::Statement(call),
            SourceOrigin::of(call.source_span),
        )? {
            return Ok(read);
        }
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
        // A receiver that is the callee's data type by name, and nothing
        // beneath it, is part of the callee's name (`names_carrier`); any
        // other is a value passed outside the argument list.
        let receiver = call.receiver_root_symbol.is_valid()
            || call.receiver_symbol.is_valid()
            || !call.receiver.is_empty();
        let qualified = call.receiver_root_symbol == call.receiver_symbol
            && call.receiver.len() == 1
            && self.names_carrier(call.target_symbol, call.receiver_symbol);
        if receiver && !qualified {
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
            None,
        )
    }

    pub(super) fn call_expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCallExpression,
        handle: ExpressionHandle,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        // Taken before anything nested is read, so no argument sees it.
        let expected = self.expected.take();
        self.bound_target(id, call.target_symbol)?;
        if let Some(description) = self.declaration_query(id, call, source)? {
            return Ok(description);
        }
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
        // A Build selection is neither: it is read below, where its
        // designated operands are attached, and nowhere else.
        if self.rigid && id.get() == 1 && !call.build_selection.is_valid() {
            if let Some(read) = self.rigid_call(
                id,
                slots,
                StaticArgumentCall::Expression(handle, call),
                source,
            )? {
                return Ok(read);
            }
            // A call of another generic machine at this body's own type
            // parameters, `same<T>(left, right)`: held to that machine's
            // contract at them.
            if let Some(arguments) = self.type_arguments(id, call)? {
                return self.generic_call(id, slots, call, arguments, source);
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
        // A receiver that is the callee's data type by name is part of the
        // callee's name (`names_carrier`); any other is a value passed
        // outside the argument list.
        let qualified = call.receiver.is_valid()
            && matches!(
                self.node(id, call.receiver)?,
                ExpressionNode::Name(path) if self.names_carrier(call.target_symbol, path.symbol)
            );
        if (call.receiver.is_valid() && !qualified)
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
        let mut read = self.call(id, slots, call.target_symbol, arguments, source, expected)?;
        if call.build_selection.is_valid() {
            self.designate(id, call.build_selection, &mut read)?;
        }
        Ok(read)
    }

    /// Attaches a Build selection's designated operands to the call typing
    /// spelled for it. scoped_execution.md: the operations "themselves run
    /// as admitted Build operations. No product receiver, function body,
    /// initializer, or provider is executed." So the call stays the
    /// ordinary boundary call it is, and the reference to the declarations
    /// it names rides beside it to the operation lowering emits.
    ///
    /// Only an operation of the toolchain's own selection protocol receives
    /// one: the Build host hands the reference to no other callee. The two
    /// selections whose host row needs more than the reference are refused
    /// by name rather than published without it.
    fn designate(
        &self,
        id: MachineId,
        selection: arena::Handle<typed_trees::expression::BuildSelectionRequest>,
        read: &mut Expression,
    ) -> Checked<()> {
        use language_semantics::declaration_selection::{BuildOperation, BuildSelectionRequest};
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        let Some(request) = self
            .typed
            .expression_table
            .build_selection_request(selection)
        else {
            return unserved("Build selection without its designated operands");
        };
        let ExpressionKind::Call {
            callee: semantic_vocabulary::TargetMachineCallable::Boundary(boundary),
            designated,
            ..
        } = &mut read.kind
        else {
            return unserved("Build selection on a call that is no boundary operation");
        };
        let protocol = self
            .boundary_symbols
            .get(*boundary)
            .and_then(|requirement| self.typed.trait_machine_signature(*requirement))
            .is_some_and(|(owner, _)| {
                language_semantics::build_requests::is_build_protocol(
                    &self.typed.symbols,
                    owner.symbol,
                    language_semantics::build_requests::SELECTION_PROTOCOL,
                )
            });
        if !protocol {
            return unserved("Build selection outside the toolchain's selection protocol");
        }
        match request {
            // Its host row names the requirement's application and the
            // boundaries that application covers.
            BuildSelectionRequest::ProviderReadiness(_) => {
                return unserved("Build provider readiness needs its requirement application");
            }
            // Its host row carries the locator value's shape.
            BuildSelectionRequest::Declaration(declaration)
                if declaration.operation == BuildOperation::NativeBinding =>
            {
                return unserved("native Build binding needs its locator shape");
            }
            BuildSelectionRequest::Entry(_)
            | BuildSelectionRequest::Declaration(_)
            | BuildSelectionRequest::Exclusion(_) => {}
        }
        *designated = Some(crate::body::DesignatedOperands {
            request: selection,
            selecting: id,
        });
        Ok(())
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
        if call.build_selection.is_valid()
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
        if !dispatch.requirement.is_valid() {
            return unserved("requirement dispatch with no requirement");
        }
        // The member's value parameters, bound by this call's static
        // arguments (`Implementation::value<7>(x)`).
        let declared =
            self.declared_requirement(id, dispatch.declaring_trait, dispatch.requirement)?;
        let binders = self.requirement_binders(id, declared)?;
        let value_arguments = self.static_value_arguments(
            id,
            &binders,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        // Stage 04 records the requirement's signature at those arguments
        // beside the dispatch. The route states that contract itself, from
        // the requirement and the literals (`requirement_signature`), so the
        // record is not read; without value arguments nothing explains one.
        if dispatch.contract_signature.is_valid() && value_arguments.is_empty() {
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
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        if !dispatch.realization_machine.is_valid() {
            return unserved("requirement dispatch with no realization");
        }
        // The call targets the member that realizes the requirement, or,
        // when the call binds value parameters, the machine stage 04
        // generated from that member for them.
        let target = if dispatch.realization_state == call.target_symbol {
            dispatch.realization_machine
        } else if !value_arguments.is_empty()
            && self.typed.symbols.get(call.target_symbol).generated_from
                == dispatch.realization_state
        {
            call.target_symbol
        } else {
            return unserved("requirement dispatch that targets another machine");
        };
        let read = self.call(id, slots, target, arguments, source, None)?;
        if let ExpressionKind::Call {
            callee: semantic_vocabulary::TargetMachineCallable::Machine(callee),
            ..
        } = &read.kind
        {
            self.selections.push(crate::Selected {
                caller: id,
                parameter: dispatch.evidence_parameter,
                declaring_trait: dispatch.declaring_trait,
                requirement: dispatch.requirement,
                implementation: *callee,
                value_arguments,
            });
        }
        Ok(read)
    }

    /// The literals a call's static arguments bind `binders` to, a
    /// requirement's value parameters in declaration order. Each argument
    /// must be one integer literal and nothing else, as the call site
    /// records it (`StaticMachineArgument::const_literal`), and there must be
    /// exactly one per parameter; the application may apply nothing more.
    /// With no value parameter the call may carry no static argument.
    fn static_value_arguments(
        &self,
        id: MachineId,
        binders: &[(SymbolHandle, semantic_vocabulary::ScalarType)],
        application_handle: typed_trees::static_applications::StaticMachineApplicationHandle,
        authored_arguments: &[typed_trees::expression::StaticMachineArgument],
    ) -> Checked<Vec<semantic_vocabulary::ScalarTerm>> {
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::static_applications::StaticMachineBindingValue;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        let table = &self.typed.expression_table;
        let (application, arguments, bindings) = if application_handle.is_valid() {
            (
                Some(table.static_machine_application(application_handle)),
                table.static_machine_application_arguments(application_handle),
                table.static_machine_application_bindings(application_handle),
            )
        } else {
            (None, &[][..], &[][..])
        };
        if let Some(application) = application
            && (!application.selected_machine_arguments.is_empty()
                || !application.input_selections.is_empty()
                || !application.lifetime_bindings.is_empty()
                || application.conformance_selection.is_some()
                || application.callable_contract_source.is_valid())
        {
            return unserved("requirement call with a static application feature");
        }
        // The call's own list, when it keeps one, is the application's.
        if !authored_arguments.is_empty() && authored_arguments != arguments {
            return unserved("requirement call whose static arguments differ from its application");
        }
        if arguments.len() != binders.len() || bindings.len() != binders.len() {
            return unserved("requirement call with static arguments");
        }
        if bindings
            .iter()
            .any(|binding| !matches!(binding.value, StaticMachineBindingValue::Const(_)))
        {
            return unserved("requirement call that binds something other than a constant");
        }
        let mut literals = Vec::with_capacity(binders.len());
        for (argument, (_, scalar_type)) in arguments.iter().zip(binders) {
            let Some(literal) = &argument.const_literal else {
                return unserved("value argument that is not an integer literal");
            };
            let only_a_literal = StaticMachineArgument {
                const_literal: Some(literal.clone()),
                ..StaticMachineArgument::default()
            };
            if *argument != only_a_literal {
                return unserved("value argument that is not only an integer literal");
            }
            let (value, scalar_type) = self.literal(id, literal, Some(*scalar_type))?;
            literals.push(semantic_vocabulary::ScalarTerm::Integer { scalar_type, value });
        }
        Ok(literals)
    }

    /// The types a call in a generic check form's entry applies its callee's
    /// type parameters at, in the callee's declaration order, when the call
    /// is a type application: every binding of its static application gives
    /// one type parameter of the callee a type. `None` for any other call.
    /// Each type must be one of the entry's own rigid type parameters: a
    /// generic machine applied at a concrete type inside a generic body is
    /// not generated by stage 04 and has no closed machine to call.
    fn type_arguments(
        &mut self,
        id: MachineId,
        call: &TableCallExpression,
    ) -> Checked<Option<Vec<crate::body::ValueType>>> {
        use typed_trees::static_applications::StaticMachineBindingValue;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if !call.authored_machine_application.is_valid() {
            return Ok(None);
        }
        let typed = self.typed;
        let table = &typed.expression_table;
        let bindings = table.static_machine_application_bindings(call.authored_machine_application);
        if bindings.is_empty()
            || bindings
                .iter()
                .any(|binding| !matches!(binding.value, StaticMachineBindingValue::Type(_)))
        {
            return Ok(None);
        }
        let application = table.static_machine_application(call.authored_machine_application);
        if !application.selected_machine_arguments.is_empty()
            || !application.input_selections.is_empty()
            || !application.lifetime_bindings.is_empty()
            || application.conformance_selection.is_some()
            || application.callable_contract_source.is_valid()
        {
            return unserved("type application with another static feature");
        }
        let owner = match typed.symbols.get(call.target_symbol).kind {
            SymbolKind::State => typed.symbols.get(call.target_symbol).parent,
            SymbolKind::Machine => call.target_symbol,
            other => return unserved(symbol_kind(other)),
        };
        let Some(callee) = typed.machine_by_symbol(owner) else {
            return unserved("type application of something that is not a machine");
        };
        let parameters = typed.machine_type_parameters(callee);
        if parameters.len() != bindings.len() {
            return unserved("type application that does not bind every parameter");
        }
        let mut arguments = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            let mut bound = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol);
            let (Some(binding), None) = (bound.next(), bound.next()) else {
                return unserved("type parameter without exactly one binding");
            };
            let StaticMachineBindingValue::Type(type_reference) = &binding.value else {
                return unserved("type parameter bound to something other than a type");
            };
            let argument = self.value_type(id, *type_reference)?;
            if !matches!(argument, crate::body::ValueType::Rigid(_)) {
                return unserved("generic call at a concrete type inside a generic body");
            }
            arguments.push(argument);
        }
        Ok(Some(arguments))
    }

    /// The entry of a generic check form calling another generic machine at
    /// its own type parameters: a direct call of that machine's contract at
    /// them (`application`).
    fn generic_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCallExpression,
        type_arguments: Vec<crate::body::ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if call.installed_interrupt_operation.is_some()
            || call.static_requirement_dispatch.is_some()
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
            return unserved("type application with another call feature");
        }
        self.direct_selection(
            id,
            call.target_symbol,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        // A call names its callee's entry state; the machine is that
        // state's parent, and must have exactly that one state.
        let machine = match self.typed.symbols.get(call.target_symbol).kind {
            SymbolKind::Machine => call.target_symbol,
            SymbolKind::State => {
                let parent = self.typed.symbols.get(call.target_symbol).parent;
                let (_, state) = self.declaration(id, parent)?;
                if state.symbol != call.target_symbol {
                    return unserved("state other than the entry");
                }
                parent
            }
            other => return unserved(symbol_kind(other)),
        };
        let callee = self.application(machine, type_arguments)?;
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        self.call_of(id, slots, callee.into(), arguments, source, None)
    }

    /// The entry of a generic check form calling through a rigid
    /// implementation parameter: a direct call of the requirement.
    fn rigid_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: StaticArgumentCall<'_>,
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        if !self.rigid || id.get() != 1 {
            return Ok(None);
        }
        let (target, application, authored_arguments, arguments, selection, binding, unsupported) =
            match call {
                StaticArgumentCall::Expression(_, call) => (
                    call.target_symbol,
                    call.authored_machine_application,
                    &call.machine_arguments[..],
                    self.typed
                        .expression_table
                        .expression_handles(call.arguments),
                    call.authored_call_selection,
                    call.authored_call_binding,
                    call.build_selection.is_valid()
                        || call.quotient_operation.is_some()
                        || call.private_layout_operation.is_some()
                        || call.installed_interrupt_operation.is_some()
                        || call.layout_query.is_some()
                        || call.native_code_layout.is_some()
                        || call.static_requirement_dispatch.is_some()
                        || call.requirement_application.is_valid()
                        || call.exact_coercion_target.is_valid()
                        || call.static_machine_parameter.is_valid()
                        || !call.evidence_arguments.is_empty()
                        || call.operational_acknowledgement.acknowledges_suspend
                        || call.operational_acknowledgement.acknowledges_block,
                ),
                StaticArgumentCall::Statement(call) => (
                    call.target_symbol,
                    call.authored_machine_application,
                    &call.machine_arguments[..],
                    self.typed
                        .statement_table
                        .expression_handles(call.arguments),
                    call.authored_call_selection,
                    None,
                    call.installed_interrupt_operation.is_some()
                        || call.static_requirement_dispatch.is_some()
                        || call.reflected_field_origin.is_some()
                        || call.static_machine_parameter.is_valid()
                        || !call.evidence_arguments.is_empty()
                        || call.operational_acknowledgement.acknowledges_suspend
                        || call.operational_acknowledgement.acknowledges_block,
                ),
            };
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let requirement = match self.typed.evidence_requirement_declaration(target) {
            Ok(Some(requirement)) => requirement,
            Ok(None) => return Ok(None),
            Err(_) => {
                return Err(unserved(
                    "implementation parameter child with no requirement",
                ));
            }
        };
        if unsupported {
            return Err(unserved(
                "implementation parameter call with another call feature",
            ));
        }
        // Both source forms keep their real authored selection. A call with
        // no binding record cannot acquire one from its target's spelling.
        if selection.is_none() && binding.is_none() {
            return Err(unserved(
                "implementation parameter call not bound to its requirement",
            ));
        }
        self.direct_selection(id, target, selection, binding)?;
        let binders = self.requirement_binders(id, requirement)?;
        let value_arguments =
            self.static_value_arguments(id, &binders, application, authored_arguments)?;
        let callee = self.requirement(requirement, &value_arguments)?;
        self.call_of(id, slots, callee.into(), arguments, source, None)
            .map(Some)
    }

    pub(super) fn call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
        expected: Option<crate::body::ValueType>,
    ) -> Checked<Expression> {
        if !target.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("callee without a symbol"),
            ));
        }
        if self.typed.trait_machine_signature(target).is_some() {
            let boundary = self.boundary(id, target)?;
            return self.call_of(
                id,
                slots,
                semantic_vocabulary::TargetMachineCallable::Boundary(boundary),
                arguments,
                source,
                expected,
            );
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
        self.visible_to(id, machine)?;
        // The callee's formals are the arguments' expected types, so its
        // signature is read first; reading order is not evaluation order.
        let callee = self.machine(machine)?;
        self.call_of(id, slots, callee.into(), arguments, source, expected)
    }

    /// Refuses a call from machine `id` to a machine of another package that
    /// is not public. Which package a declaration belongs to is the package
    /// of the source file its symbol was declared in, as for subjects
    /// (`subjects`); a declaration with no source file cannot be placed and
    /// is refused. The typed trees can hold such a call: the deleted checker
    /// rejected it after them, where a product selected the callee, so a
    /// reader that took every resolved call as permitted would accept a
    /// program that reaches into a dependency's private machine.
    pub(super) fn visible_to(&self, id: MachineId, callee: SymbolHandle) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        let is_public = self
            .typed
            .machine_by_symbol(callee)
            .map(|declaration| declaration.is_public)
            .or_else(|| {
                self.typed
                    .trait_machine_signature(callee)
                    .map(|(owner, _)| owner.is_public)
            })
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("target has no callable declaration"),
                )
            })?;
        if is_public {
            return Ok(());
        }
        let symbols = &self.typed.symbols;
        // A machine stage 04 generated belongs where its declaration does.
        let package = |symbol: SymbolHandle| {
            let symbol = crate::template_of(self.typed, symbol).unwrap_or(symbol);
            symbol
                .is_valid()
                .then(|| symbols.symbol_provenance_source_span(symbol))
                .flatten()
                .and_then(|span| symbols.source_file(span))
                .map(|file| (file.origin, file.package_identity))
        };
        let caller = self.symbols.get(id).copied().and_then(package);
        match (caller, package(callee)) {
            (Some(caller), Some(callee)) if caller == callee => Ok(()),
            (Some(_), Some(_)) => unserved("private machine of another package"),
            _ => unserved("call between declarations without a source file"),
        }
    }

    /// The call of `callee`, a machine or a requirement already read.
    ///
    /// A callee generic over lifetimes (`machine wrap<'a>(..)`) has a
    /// signature in its own regions; the call instantiates them with the
    /// caller's (`regions`). The region each of the callee's parameters
    /// stands for is taken from `expected`, the type the caller expects of
    /// the result, then from each argument as it is read, and every formal
    /// is read at the regions bound so far, so a literal argument is a value
    /// of the region its formal is at. Once every argument is read, each
    /// must be its formal at the bound regions, and the result is the
    /// callee's at them. A result whose region no input or expectation
    /// supplies is refused: lifetimes.md, "A signature does not authorize
    /// returning unrelated storage merely because its type has the same
    /// lifetime spelling".
    fn call_of(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        callee: semantic_vocabulary::TargetMachineCallable,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
        expected: Option<crate::body::ValueType>,
    ) -> Checked<Expression> {
        let free: Vec<_> = match callee {
            semantic_vocabulary::TargetMachineCallable::Machine(machine) => {
                let symbol = self
                    .symbols
                    .get(machine)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                self.machine_lifetimes(symbol)
            }
            semantic_vocabulary::TargetMachineCallable::Boundary(boundary) => {
                let symbol = self
                    .boundary_symbols
                    .get(boundary)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                self.machine_lifetimes(symbol)
            }
        };
        let mut bound = super::regions::Substitution::new();
        let signature = crate::body::Signatures {
            machines: &self.signatures,
            boundaries: &self.boundary_signatures,
            completions: &self.boundary_completions,
        }
        .get(callee, id)?;
        let formals: Vec<_> = signature
            .parameters
            .iter()
            .enumerate()
            .map(|(position, formal)| {
                (
                    *formal,
                    signature.is_erased(position),
                    signature.borrowed(position).cloned(),
                )
            })
            .collect();
        let declared_result = signature.result;
        if formals.len() != arguments.len() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("argument count"),
            ));
        }
        if !free.is_empty()
            && let (Some(result), Some(expected)) = (declared_result, expected)
        {
            self.unify(id, &free, result, expected, &mut bound)?;
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
        let mut lenders = Vec::new();
        for (argument, (declared, erased, borrowed)) in arguments.iter().zip(&formals) {
            let formal = self.substitute(*declared, &bound);
            let formal = &self.argument_context(formal, &free, &bound);
            // The argument for an erased formal is a term, not a
            // computation; the argument for a borrowed formal is a loan of
            // a place.
            read.push(if *erased {
                self.erased_term(id, slots, *argument, formal.scalar())?
            } else if let Some(borrowed) = borrowed {
                let (lent, lender) = self.lent(id, slots, *argument, borrowed, *formal)?;
                // Its position among the runtime actuals, as the invocation
                // row counts them: erased arguments are not actuals.
                let actual = formals[..read.len()]
                    .iter()
                    .filter(|(_, erased, _)| !erased)
                    .count();
                lenders.push((actual, lender));
                lent
            } else {
                self.expression(id, slots, *argument, Some(*formal))?
            });
            if !free.is_empty()
                && let Some(actual) = read.last().and_then(|argument| argument.result)
            {
                self.unify(id, &free, *declared, actual, &mut bound)?;
            }
        }
        lent_storage_is_untouched(id, &read, &self.captures, |place| {
            place.result.is_some_and(|held| self.is_borrowed_view(held))
        })?;
        if let SourceOrigin::Authored(span) = source
            && let Some(names) = self.names.get_mut(id)
        {
            for (position, lender) in lenders {
                names.loans.push((span, position, lender));
            }
        }
        if formals.len() != read.len() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("argument types"),
            ));
        }
        for ((declared, _, _), actual) in formals.iter().zip(&read) {
            let formal = self.substitute(*declared, &bound);
            if !self.agrees(actual.result, formal) {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("argument types"),
                ));
            }
        }
        // A result lifetime no formal names relates the result to no input:
        // lifetimes.md, "Generic returned views require the complete
        // instantiated frontier and its result-to-input relation".
        let related: Vec<_> = free
            .iter()
            .copied()
            .filter(|parameter| {
                formals
                    .iter()
                    .any(|(formal, _, _)| self.names_any(*formal, &[*parameter]))
            })
            .collect();
        let unrelated: Vec<_> = free
            .iter()
            .copied()
            .filter(|parameter| !related.contains(parameter))
            .collect();
        if declared_result.is_some_and(|result| self.names_any(result, &unrelated)) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("result lifetime that no argument is a view of"),
            ));
        }
        let result = declared_result.map(|result| self.substitute(result, &bound));
        // Binding is an invocation relation, even when a recursive call's
        // actual region has the same declaration identity as a free formal.
        // Test missing bindings on the declared result, before substitution.
        let unbound: Vec<_> = free
            .iter()
            .copied()
            .filter(|parameter| !bound.iter().any(|(known, _)| known == parameter))
            .collect();
        if declared_result.is_some_and(|result| self.names_any(result, &unbound)) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("result lifetime that no argument supplies"),
            ));
        }
        // A value a call returns is the result of an operation, and no
        // operation of the lowered form has a result of a rigid type.
        if matches!(result, Some(crate::body::ValueType::Rigid(_))) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Call("call that returns a value of a type parameter"),
            ));
        }
        Ok(Expression {
            kind: ExpressionKind::Call {
                callee,
                arguments: read,
                designated: None,
            },
            result,
            source,
        })
    }

    /// The argument for a borrowed formal: `&place` or `&mut place`, or a
    /// borrowed formal of this machine named whole, which lends its
    /// referent on (a reborrow). The place is a slot or a field path
    /// beneath one. ownership.md, "Owned local mutability": "modifying an
    /// established ordinary local's owned storage requires a `mut` binding.
    /// This covers ... formation of `&mut` or `&write` loans to those
    /// places. A callee cannot grant writable access that the caller's place
    /// lacks."
    fn lent(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        argument: ExpressionHandle,
        formal: &crate::body::Borrowed,
        expected: ValueType,
    ) -> Checked<(Expression, String)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        let source = SourceOrigin::of(self.typed.expression_table.source_span(argument));
        let node = self.node(id, argument)?;
        if matches!(node, ExpressionNode::String(_)) && formal.access != ReferenceAccess::Shared {
            return unserved("byte literal cannot lend mutable storage");
        }
        // A shared view already carries its immutable backing. Read every
        // expression of that type through the same descriptor operation,
        // including constructors and returned views; the call's region check
        // still authenticates the actual type and its result relation.
        if formal.access == ReferenceAccess::Shared
            && self.is_borrowed_view(expected)
            && !matches!(node, ExpressionNode::Borrow(_))
        {
            let place = self.expression(id, slots, argument, Some(expected))?;
            return Ok((
                Expression {
                    result: place.result,
                    kind: ExpressionKind::Borrow {
                        access: ReferenceAccess::Shared,
                        place: Box::new(place),
                        home: None,
                    },
                    source,
                },
                self.typed.expression_table.display_name(argument),
            ));
        }
        // Ordinary references still name existing storage or explicitly
        // borrow it; a descriptor expression cannot grant writable custody.
        let (target, access) = match self.node(id, argument)? {
            ExpressionNode::Borrow(borrow) => (borrow.target, borrow.access),
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, slots, path)?;
                match slots[slot].borrowed {
                    Some(_) => (argument, formal.access),
                    None => return unserved("argument for a borrowed formal that is not a borrow"),
                }
            }
            _ => return unserved("argument for a borrowed formal that is not a borrow"),
        };
        if access != formal.access {
            return unserved("borrow whose access differs from its formal's");
        }
        // What a diagnostic calls the lender: the binding at the root of
        // the place, as the deleted checker named it.
        let mut root = target;
        while let ExpressionNode::Member(member) = self.node(id, root)? {
            root = member.receiver;
        }
        let lender_text = self.typed.expression_table.display_name(root);
        let place = self.expression(id, slots, target, None)?;
        if !names_storage(&place) {
            return unserved("borrow of a value that is not a place");
        }
        // The binding whose storage the place is in: through fields,
        // elements, case payloads and the subject a match saved.
        let captures: Vec<_> = self
            .captures
            .iter()
            .map(|(capture, subject)| (*capture, subject))
            .collect();
        let Some(slot) = root_slot(id, &place, &captures)? else {
            return unserved("borrow of a value that is not a place");
        };
        // A binding's mutability is a source rule the lowered form does not
        // keep, so it is decided here; a reborrow's access against its
        // parent's is the checker's judgment (`lowered-discharge`, loans).
        let lender = &slots[slot];
        if access == ReferenceAccess::Mutable && lender.borrowed.is_none() && !lender.assignable {
            return unserved("exclusive loan of an immutable binding");
        }
        // An exclusive loan of a field hands its holder a write inside
        // every value the path goes through.
        if access != ReferenceAccess::Shared {
            self.written_through(id, &place)?;
        }
        // A scalar local that is an SSA value here gets a home for the loan;
        // its type's identity is the formal's referent identity, the same
        // scalar type.
        // A reference formal's referent is never policy-qualified
        // (`formal`), and the place a loan lends has the referent's type.
        if place.result.and_then(ValueType::policy).is_some() {
            return unserved("loan of a policy-qualified integer");
        }
        let home = match (&place.kind, lender.value_type) {
            (ExpressionKind::Slot(_), ValueType::Scalar(_, Qualification::NONE))
                if !lender.homed =>
            {
                Some(formal.home.clone().ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Spelling("scalar referent without a type identity"),
                    )
                })?)
            }
            _ => None,
        };
        Ok((
            Expression {
                result: place.result,
                kind: ExpressionKind::Borrow {
                    access,
                    place: Box::new(place),
                    home,
                },
                source,
            },
            lender_text,
        ))
    }
}

/// Resolve a captured subject through this reader's already-authenticated
/// match scope. A missing, duplicated or cyclic capture cannot grant access.
fn captured_subject<'a>(
    id: MachineId,
    capture: arena::Handle<typed_trees::expression::CompletedValueCapture>,
    captures: &[(
        arena::Handle<typed_trees::expression::CompletedValueCapture>,
        &'a Expression,
    )],
) -> Checked<&'a Expression> {
    let mut found = captures.iter().filter(|(known, _)| *known == capture);
    let Some((_, subject)) = found.next() else {
        return Err(refuse(
            id,
            Construct::Machine,
            Unserved::Call("saved subject outside its match capture"),
        ));
    };
    if found.next().is_some() {
        return Err(refuse(
            id,
            Construct::Machine,
            Unserved::Call("ambiguous saved subject capture"),
        ));
    }
    Ok(subject)
}

/// The slot whose storage a place expression names. Captured index operands
/// have already run; following their base retains storage provenance only.
fn root_slot<'a>(
    id: MachineId,
    mut place: &'a Expression,
    captures: &[(
        arena::Handle<typed_trees::expression::CompletedValueCapture>,
        &'a Expression,
    )],
) -> Checked<Option<usize>> {
    let mut followed = 0;
    loop {
        match &place.kind {
            ExpressionKind::Slot(slot) => return Ok(Some(*slot)),
            ExpressionKind::Field { base, .. }
            | ExpressionKind::Payload { base, .. }
            | ExpressionKind::Index { base, .. } => place = base,
            ExpressionKind::SavedSubject(capture) => {
                followed += 1;
                if followed > captures.len() {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("cyclic saved subject capture"),
                    ));
                }
                place = captured_subject(id, *capture, captures)?;
            }
            _ => return Ok(None),
        }
    }
}

/// Arguments run left to right, but this lowering forms loans together at
/// the call. Conservatively refuse any argument read or newly evaluated
/// selector of a root this call lends, including reads before a loan. This
/// preserves the existing restriction until loan timing is represented.
///
/// A place that holds a byte view lends none of its binding's storage: the
/// callee is given the view, whose bytes are elsewhere. lifetimes.md: "The
/// result carries loans into the original backing, not a borrow of the
/// caller's private carrier storage." `holds_view` says which places do.
fn lent_storage_is_untouched<'a>(
    id: MachineId,
    arguments: &'a [Expression],
    captures: &'a [(
        arena::Handle<typed_trees::expression::CompletedValueCapture>,
        Expression,
    )],
    holds_view: impl Fn(&Expression) -> bool,
) -> Checked<()> {
    let mut captures: Vec<_> = captures
        .iter()
        .map(|(capture, subject)| (*capture, subject))
        .collect();
    let mut lent = Vec::new();
    for argument in arguments {
        if let ExpressionKind::Borrow { place, .. } = &argument.kind
            && !holds_view(place)
            && let Some(slot) = root_slot(id, place, &captures)?
        {
            lent.push(slot);
        }
    }
    if lent.is_empty() {
        return Ok(());
    }
    for argument in arguments {
        if let ExpressionKind::Borrow { place, .. } = &argument.kind {
            if selectors_mention(id, place, &lent, &mut captures)? {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("loan selector that names storage another argument lends"),
                ));
            }
        } else if mentions(id, argument, &lent, &mut captures)? {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Call("argument that names storage another argument lends"),
            ));
        }
    }
    Ok(())
}

/// A saved subject's selectors are captured, so they do not evaluate again.
/// New index operands still run while earlier argument loans are live.
fn selectors_mention<'a>(
    id: MachineId,
    place: &'a Expression,
    slots: &[usize],
    captures: &mut Vec<(
        arena::Handle<typed_trees::expression::CompletedValueCapture>,
        &'a Expression,
    )>,
) -> Checked<bool> {
    match &place.kind {
        ExpressionKind::Index { base, index } => {
            Ok(selectors_mention(id, base, slots, captures)?
                || mentions(id, index, slots, captures)?)
        }
        ExpressionKind::Field { base, .. } | ExpressionKind::Payload { base, .. } => {
            selectors_mention(id, base, slots, captures)
        }
        ExpressionKind::SavedSubject(_) => {
            root_slot(id, place, captures)?;
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// A saved subject still reads its captured backing, but not its old selector.
fn mentions<'a>(
    id: MachineId,
    expression: &'a Expression,
    slots: &[usize],
    captures: &mut Vec<(
        arena::Handle<typed_trees::expression::CompletedValueCapture>,
        &'a Expression,
    )>,
) -> Checked<bool> {
    match &expression.kind {
        ExpressionKind::Slot(slot) => Ok(slots.contains(slot)),
        ExpressionKind::SavedSubject(_) => {
            Ok(root_slot(id, expression, captures)?.is_some_and(|root| slots.contains(&root)))
        }
        ExpressionKind::Integer(_)
        | ExpressionKind::ByteLiteral(_)
        | ExpressionKind::Boolean(_) => Ok(false),
        ExpressionKind::Not(operand)
        | ExpressionKind::BitwiseNot(operand)
        | ExpressionKind::Cast(operand)
        | ExpressionKind::Requalify(operand)
        | ExpressionKind::Establish { operand, .. }
        | ExpressionKind::Field { base: operand, .. }
        | ExpressionKind::Payload { base: operand, .. }
        | ExpressionKind::IsCase {
            subject: operand, ..
        }
        | ExpressionKind::Borrow { place: operand, .. } => mentions(id, operand, slots, captures),
        ExpressionKind::Array { elements } => {
            for element in elements {
                if mentions(id, element, slots, captures)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        ExpressionKind::Index { base, index } => {
            Ok(mentions(id, base, slots, captures)? || mentions(id, index, slots, captures)?)
        }
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, operand) in fields {
                if mentions(id, operand, slots, captures)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        ExpressionKind::Match {
            capture,
            subject,
            arms,
            default,
        } => {
            if mentions(id, subject, slots, captures)? {
                return Ok(true);
            }
            if let Some(capture) = capture {
                captures.push((*capture, subject));
            }
            let result = (|| {
                for (_, arm) in arms {
                    if mentions(id, arm, slots, captures)? {
                        return Ok(true);
                    }
                }
                match default {
                    Some(arm) => mentions(id, arm, slots, captures),
                    None => Ok(false),
                }
            })();
            if capture.is_some() {
                captures.pop();
            }
            result
        }
        ExpressionKind::Strict { left, right, .. }
        | ExpressionKind::ShortCircuit { left, right, .. } => {
            Ok(mentions(id, left, slots, captures)? || mentions(id, right, slots, captures)?)
        }
        ExpressionKind::Call { arguments, .. } => {
            for argument in arguments {
                if mentions(id, argument, slots, captures)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Unpositioned;
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};

    #[test]
    fn saved_subject_reads_keep_loan_provenance_without_replaying_selectors() {
        let id = MachineId::new(1).unwrap();
        let mut typed = typed_trees::TypedTrees::default();
        let marker = typed.expression_table.insert(ExpressionNode::Boolean(true));
        let capture = typed.expression_table.insert_match_subject(marker);
        let foreign = typed.expression_table.insert_match_subject(marker);
        let source = SourceOrigin::Unpositioned(Unpositioned::SpanAbsent);
        let record = ValueType::record(StructuralTypeId::new(1).unwrap());
        let scalar = ValueType::plain(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        ));
        let slot = |slot| Expression {
            kind: ExpressionKind::Slot(slot),
            result: Some(if slot == 1 {
                scalar
            } else if slot == 0 {
                ValueType::record(StructuralTypeId::new(2).unwrap())
            } else {
                record
            }),
            source,
        };
        let saved = |capture| Expression {
            kind: ExpressionKind::SavedSubject(capture),
            result: Some(record),
            source,
        };
        let borrow = |place: Expression| {
            let result = place.result;
            Expression {
                kind: ExpressionKind::Borrow {
                    access: ReferenceAccess::Mutable,
                    place: Box::new(place),
                    home: None,
                },
                result,
                source,
            }
        };
        let payload = |capture| Expression {
            kind: ExpressionKind::Payload {
                base: Box::new(saved(capture)),
                case: 0,
                field: 0,
            },
            result: Some(scalar),
            source,
        };
        let captures = vec![(
            capture,
            Expression {
                kind: ExpressionKind::Index {
                    base: Box::new(slot(0)),
                    index: Box::new(slot(1)),
                },
                result: Some(record),
                source,
            },
        )];
        // The payload aliases the original array, but its selector was read
        // before the call. Lending that selector's slot must not replay it.
        let overlapping =
            lent_storage_is_untouched(id, &[borrow(slot(0)), payload(capture)], &captures, |_| {
                false
            })
            .unwrap_err();
        assert!(
            overlapping
                .detail
                .contains("argument that names storage another argument lends")
        );
        assert!(
            lent_storage_is_untouched(id, &[borrow(slot(1)), payload(capture)], &captures, |_| {
                false
            })
            .is_ok()
        );
        assert!(
            lent_storage_is_untouched(id, &[borrow(slot(2)), payload(capture)], &captures, |_| {
                false
            })
            .is_ok()
        );
        let refused =
            lent_storage_is_untouched(id, &[borrow(slot(0)), payload(foreign)], &captures, |_| {
                false
            })
            .unwrap_err();
        assert!(refused.detail.contains("outside its match capture"));
        assert!(
            lent_storage_is_untouched(
                id,
                &[borrow(saved(capture)), payload(capture)],
                &captures,
                |_| false
            )
            .is_err()
        );
        assert!(
            lent_storage_is_untouched(
                id,
                &[borrow(slot(2)), payload(capture)],
                &[(capture, saved(capture))],
                |_| false
            )
            .is_err()
        );
    }
}
