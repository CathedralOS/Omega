/*
The call family of the reader: which callee a call names, the static
application and authored selection it must leave unused, and the call itself
with its arguments.
*/

use crate::body::Qualification;
use language_core::ReferenceAccess;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionExposure,
    AuthoredDeclarationSelectionOccurrenceId, ResolvedAuthoredCallBinding,
};
use semantic_vocabulary::MachineId;
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::StructuralAccess;
use typed_trees::expression::{
    BuildSelectionRequest, ExpressionHandle, ExpressionNode, StaticMachineArgument,
    TableCallExpression,
};
use typed_trees::references::StaticArgumentCall;
use typed_trees::statement::TableCall;
use typed_trees::static_applications::StaticMachineApplicationHandle;

use super::authority::Access;
use super::storage_path::{names_storage, root_slot};
use super::{Program, Slot, symbol_kind};
use crate::body::{CallTarget, Expression, ExpressionKind, Signature, ValueType};
use crate::{SourceOrigin, Unserved, refuse};
use semantic_vocabulary::StructuralFieldId;
use terminal_psi::ResolvedStructuralPathSegment as Segment;

struct StaticInvocation<'a> {
    target: SymbolHandle,
    application: StaticMachineApplicationHandle,
    authored_arguments: &'a [StaticMachineArgument],
    arguments: &'a [ExpressionHandle],
    selection: Option<AuthoredDeclarationSelectionOccurrenceId>,
    binding: Option<ResolvedAuthoredCallBinding>,
    unsupported: bool,
    runtime_receiver: bool,
    receiver: Option<SymbolHandle>,
}

/// The binder receipt authenticates the target; the retained occurrence
/// authenticates where its author was allowed to select it.
#[derive(Debug, Clone, Copy)]
pub(super) struct StaticMachineTarget {
    position: u32,
    target: SymbolHandle,
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
}

impl<'t> Program<'t> {
    /// Read a declaration without granting invocation authority. Actual
    /// calls check visibility before using this same retained boundary row.
    pub(super) fn boundary_declaration(
        &mut self,
        diagnostic: MachineId,
        target: SymbolHandle,
    ) -> Checked<semantic_vocabulary::BoundaryMachineId> {
        self.read_boundary_declaration(diagnostic, target)?
            .map(|(id, _)| id)
            .ok_or_else(|| {
                refuse(
                    diagnostic,
                    Construct::Machine,
                    Unserved::Call("target has no exact boundary declaration"),
                )
            })
    }

    /// An authored call has no unspent static application. Stage 04 may
    /// retain the exact application which already selected a closed callee;
    /// otherwise no arguments, bindings or other static features may remain.
    pub(super) fn applies_nothing(
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
        if !target.is_valid() {
            return unserved("callee without a declaration symbol");
        }
        // Stage 04 spends a requirement's own static arguments by selecting
        // its closed signature. The application remains as exact source
        // provenance, including equivalent tuples interned to that signature.
        if self.typed.closed_requirement_application(target).is_some() {
            self.typed
                .require_closed_requirement_call_application(target, application)
                .map_err(|reason| refuse(id, Construct::Machine, Unserved::Call(reason)))?;
            return Ok(());
        }
        if self.boundary_application_is_empty(id, target, application)? {
            return Ok(());
        }
        if !application.is_valid() {
            return Ok(());
        }
        // Equal applications may share one generated body. Each caller still
        // owes visibility for its own selections, including a cached instance.
        for argument in self
            .typed
            .expression_table
            .static_machine_application_arguments(application)
        {
            self.static_argument_selections(id, argument)?;
        }
        let application = self
            .typed
            .expression_table
            .static_machine_application(application);
        // Stage 04 closes a static application by generating the machine it
        // selects: the call then targets that generated machine, whose
        // retained application records the declaration registered. Such a
        // callee has no parameter left to apply (`machine_shape` refuses one
        // that has), so the application's arguments are already spent.
        let generated = self
            .typed
            .executable_callable_template(target)
            .map_err(|what| refuse(id, Construct::Machine, Unserved::Call(what)))?;
        if let Some(generated) = generated
            && self.instantiates_callable(target, application.registration_operation)
            && self.closes_rigid_parameters_only(application.registration_operation)
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
        // entry state), or the exact mathematical/builtin declaration the
        // proof reader selected. Other identities remain static or captured.
        let registration = application.registration_operation;
        let owner = if self.typed.trait_machine_signature(target).is_some() {
            target
        } else {
            match self.typed.symbols.get(target).kind {
                SymbolKind::State => self.typed.symbols.get(target).parent,
                SymbolKind::Machine
                | SymbolKind::MathematicalDefinition
                | SymbolKind::BuiltinFunction => target,
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
                .and_then(|states| states.as_array::<1>())
                .map(|[state]| state.symbol);
            if state != Some(registration) {
                return unserved("static application registers another declaration");
            }
        }
        Ok(())
    }

    /// Read the exact executable selected by an ordinary retained application.
    /// Unspent static arguments stay subject to the ordinary call floor.
    pub(super) fn ordinary_application_target(
        &self,
        id: MachineId,
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
    ) -> Checked<SymbolHandle> {
        let table = &self.typed.expression_table;
        if !table.static_machine_application_is_valid(application) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Call("selected ordinary application is absent"),
            ));
        }
        let registered = table
            .static_machine_application(application)
            .registration_operation;
        let target = self.entered_machine(registered).ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Call("selected ordinary application has no machine body"),
            )
        })?;
        self.applies_nothing(id, registered, application, &[])?;
        Ok(target)
    }

    /// A finite alternative still checks a generic declaration's body. Its
    /// type bindings happen to be concrete; calls use the same generic
    /// contracts as a form whose binders remain rigid.
    fn checking_generic_entry(&self, id: MachineId) -> bool {
        ((self.rigid || self.alternative.is_some())
            && (id.get() == 1
                || self
                    .symbols
                    .get(id)
                    .and_then(|symbol| self.typed.machine_by_symbol(*symbol))
                    .is_some_and(|machine| {
                        self.typed
                            .machine_type_parameters(machine)
                            .iter()
                            .any(|parameter| self.generic_binders.contains(&parameter.symbol))
                    })))
            || self.applied.binary_search_by_key(&id, |row| row.0).is_ok()
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
                            | typed_trees::data::TypeParameterKind::Machine {
                                contract: typed_trees::data::MachineParameterContract::Structural(
                                    _
                                ),
                            }
                    )
                })
                && machine.lifetime_parameters.is_empty()
        })
    }

    /// The machine a symbol enters: a machine itself, or the machine whose
    /// one state the symbol is. `None` for anything else, including a state
    /// of a machine with several.
    fn entered_machine(&self, symbol: SymbolHandle) -> Option<SymbolHandle> {
        if !self.typed.symbols.symbols().nodes().is_valid(symbol) {
            return None;
        }
        match self.typed.symbols.get(symbol).kind {
            SymbolKind::Machine => Some(symbol),
            SymbolKind::State => {
                let parent = self.typed.symbols.get(symbol).parent;
                let machine = self.typed.machine_by_symbol(parent)?;
                match self.typed.machine_states.span(machine.states)?.as_array() {
                    Some([state]) if state.symbol == symbol => Some(parent),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Copied lexical declarations may be applied again at their independent
    /// binders. Authenticate the complete recorded path, including each entry
    /// correspondence, instead of accepting only the immediately preceding copy.
    fn instantiates_callable(&self, selected: SymbolHandle, original: SymbolHandle) -> bool {
        let (Some(instance), Some(template)) = (
            self.entered_machine(selected),
            self.entered_machine(original),
        ) else {
            return false;
        };
        match self.typed.symbols.get(original).kind {
            SymbolKind::Machine => self
                .typed
                .machine_instantiation_path(instance, template)
                .is_some(),
            SymbolKind::State => {
                let entry = if self.typed.symbols.get(selected).kind == SymbolKind::State {
                    Some(selected)
                } else {
                    self.typed
                        .machine_by_symbol(instance)
                        .and_then(|machine| self.typed.machine_states(machine).first())
                        .map(|state| state.symbol)
                };
                entry.is_some_and(|entry| {
                    self.typed
                        .instantiated_machine_state_matches(instance, template, original, entry)
                })
            }
            _ => false,
        }
    }

    /// A boundary trait qualifier identifies its exact requirement; it adds
    /// no receiver or argument (machines.md, "Static declarations and
    /// explicit state"). Ordinary module qualification is already resolved
    /// in the target symbol and does not use this receiver-shaped route.
    pub(super) fn names_requirement(&self, target: SymbolHandle, qualifier: SymbolHandle) -> bool {
        self.names_boundary_carrier(target, qualifier)
    }

    /// The checked named row retains its public requirement and exact body
    /// demand. An ordinary selected machine keeps its own contract; the call
    /// separately names the public contract it must establish.
    fn named_call_target(
        &self,
        id: MachineId,
        namespace: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Checked<
        Option<(
            SymbolHandle,
            &'t typed_trees::trait_definition::Conformance,
            &'t typed_trees::trait_definition::ConformanceRow,
            &'t typed_trees::signature::StateSignature,
        )>,
    > {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let Some((owner, row, required)) = self
            .typed
            .named_conformance_member(namespace, requirement)
            .map_err(unserved)?
        else {
            return Ok(None);
        };
        self.visible_to(id, requirement)?;
        self.visible_declaration_to(id, owner.symbol, owner.is_public)?;
        Ok(Some((row.realization_machine, owner, row, required)))
    }

    fn named_expression_target(
        &self,
        id: MachineId,
        call: &TableCallExpression,
    ) -> Checked<
        Option<(
            SymbolHandle,
            &'t typed_trees::trait_definition::Conformance,
            &'t typed_trees::trait_definition::ConformanceRow,
            &'t typed_trees::signature::StateSignature,
        )>,
    > {
        let Some((owner, row, required)) = self
            .typed
            .named_conformance_call_member(call)
            .map_err(|what| refuse(id, Construct::Machine, Unserved::Call(what)))?
        else {
            return Ok(None);
        };
        self.visible_to(id, call.target_symbol)?;
        self.visible_declaration_to(id, owner.symbol, owner.is_public)?;
        let template = row.realization_machine;
        if owner.type_parameters.is_empty() {
            return Ok(Some((template, owner, row, required)));
        }
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let source = self
            .typed
            .expression_table
            .static_machine_application(call.authored_machine_application);
        let source_machine = self
            .typed
            .machine_by_own_or_state_symbol(source.lexical_owner)
            .map(|machine| machine.symbol);
        let caller = self
            .symbols
            .get(id)
            .and_then(|symbol| self.typed.machine_by_own_or_state_symbol(*symbol))
            .map(|machine| machine.symbol);
        if source_machine.is_none() || source_machine != caller {
            return Err(unserved(
                "named application changed its actual calling owner",
            ));
        }
        let capture = self
            .shared
            .captures
            .get(self.typed, call)
            .map_err(unserved)?
            .ok_or_else(|| unserved("named map body has no prepared capture"))?;
        let machine = self
            .typed
            .machine_by_symbol(capture.instance)
            .ok_or_else(|| unserved("named capture lost its actual machine"))?;
        let Some([entry]) = self.typed.machine_states(machine).as_array() else {
            return Err(unserved("named capture without one actual entry"));
        };
        if !self.typed.instantiated_machine_state_matches(
            capture.instance,
            template,
            row.realization_state,
            entry.symbol,
        ) {
            return Err(unserved(
                "named capture changed its exact entry correspondence",
            ));
        }
        let expected = self
            .typed
            .applied_requirement_scope(source.lexical_owner, call.requirement_application)
            .ok_or_else(|| unserved("named call lost its complete application"))?;
        let actual = self
            .typed
            .machine_conformance_requirement_scope(
                capture.instance,
                row.declaring_trait,
                row.requirement,
            )
            .map_err(unserved)?
            .ok_or_else(|| unserved("named capture lost its owning application"))?;
        if !actual.same_application(self.typed, &expected) {
            return Err(unserved(
                "named capture supplies another requirement application",
            ));
        }
        Ok(Some((capture.instance, owner, row, required)))
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
        if !self.typed.symbols.symbols().nodes().is_valid(target) {
            return unserved("callee without a declaration symbol");
        }
        // Admit the selected declaration before interpreting the call's
        // qualifier. A retired data-owned target is invalid source, not an
        // unsupported receiver call. Requirements keep their separate owner.
        if self.typed.symbols.symbols().nodes().is_valid(target)
            && let Some(machine) = self.entered_machine(target)
        {
            self.declaration(id, machine)?;
        }
        // A closed callable targets its retained application while the author
        // still names the declaration. Requirements retain that correspondence
        // in their applied signature; body-bearing machines retain it in their
        // executable application. Neither is established by generated names.
        if selection.is_none()
            && binding.is_none()
            && self.typed.symbols.symbols().nodes().is_valid(target)
            && self.typed.symbols.get(target).kind == SymbolKind::MachineParameter
        {
            return unserved("static call has no authored selection");
        }
        let closed_requirement = self
            .typed
            .closed_requirement_application(target)
            .map(|application| application.template);
        let generated = match closed_requirement {
            Some(template) => Some(template),
            None => self
                .typed
                .executable_callable_template(target)
                .map_err(|what| refuse(id, Construct::Machine, Unserved::Call(what)))?,
        };
        let names_target = |symbol: SymbolHandle| {
            symbol == target
                || closed_requirement == Some(symbol)
                || self.instantiates_callable(target, symbol)
        };
        let selects_target = |symbol| {
            names_target(symbol)
                || self.entered_machine(symbol).is_some_and(|selected| {
                    self.entered_machine(target) == Some(selected)
                        // The retained application maps the execution entry
                        // to its exact template entry. The ledger can name
                        // that entry by its machine, as in a transition.
                        || generated.and_then(|template| self.entered_machine(template))
                            == Some(selected)
                })
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
                    if selects_target(resolved.selected_symbol()) => {}
                AuthoredDeclarationSelectionTarget::Resolved(_) => {
                    return unserved("selection resolved to another declaration");
                }
                AuthoredDeclarationSelectionTarget::Intrinsic(_) => {
                    return unserved("intrinsic selection");
                }
                AuthoredDeclarationSelectionTarget::LateBound(_) => {
                    let mut refusal = refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("late-bound selection"),
                    );
                    refusal.detail.push_str(&format!(
                        " for `{}`",
                        self.typed.symbols.display_path(target, "::")
                    ));
                    return Err(refusal);
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
            // Copying a lexical declaration rebinds the execution target,
            // while the authored binding keeps its original declaration.
            // The existing application/state correspondence must join them;
            // generated provenance alone is not that association.
            Some(binding)
                if self.instantiates_callable(target, binding.selected)
                    && self.instantiates_callable(binding.selected, binding.original) =>
            {
                Ok(())
            }
            Some(_) => unserved("rebound call"),
        }
    }

    /// A call through a callable parameter is selected by the enclosing
    /// specialization. The authored call still names the parameter itself.
    pub(super) fn static_machine_target(
        &self,
        id: MachineId,
        target: SymbolHandle,
        binder: SymbolHandle,
    ) -> Checked<(SymbolHandle, Option<StaticMachineTarget>)> {
        if !binder.is_valid() {
            return Ok((target, None));
        }
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let owner = self
            .symbols
            .get(id)
            .ok_or_else(|| unserved("static call has no owner"))?;
        let (position, argument) = self
            .typed
            .static_machine_binding(*owner, binder, target)
            .map_err(unserved)?
            .ok_or_else(|| unserved("static call target is not selected by its binder"))?;
        let occurrence = self.static_argument_selection(id, argument)?;
        Ok((
            binder,
            Some(StaticMachineTarget {
                position,
                target,
                occurrence,
            }),
        ))
    }

    fn static_argument_selections(
        &self,
        id: MachineId,
        argument: &StaticMachineArgument,
    ) -> Checked<()> {
        if self
            .typed
            .symbols
            .symbols()
            .nodes()
            .is_valid(argument.symbol)
            && matches!(
                self.typed.symbols.get(argument.symbol).kind,
                SymbolKind::Machine | SymbolKind::State
            )
        {
            self.static_argument_selection(id, argument)?;
        }
        if let Some(application) = &argument.application {
            for argument in &application.arguments {
                self.static_argument_selections(id, argument)?;
            }
            if application.member.is_valid() {
                let member = self
                    .typed
                    .expression_table
                    .static_member(application.member)
                    .ok_or_else(|| {
                        refuse(
                            id,
                            Construct::Machine,
                            Unserved::Call("static argument lost its selected member"),
                        )
                    })?;
                self.static_argument_selections(id, member)?;
            }
        }
        Ok(())
    }

    fn static_argument_selection(
        &self,
        id: MachineId,
        argument: &StaticMachineArgument,
    ) -> Checked<AuthoredDeclarationSelectionOccurrenceId> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let selection = argument
            .authored_selection
            .and_then(|occurrence| self.typed.authored_declaration_selections().get(occurrence))
            .ok_or_else(|| unserved("static call argument lost its authored selection"))?;
        if selection.kind() != language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticArgument
            || selection.target().as_resolved().is_none_or(|selected| {
                let selected = selected.selected_symbol();
                !self.typed.symbols.symbols().nodes().is_valid(selected)
                    || (selected != argument.symbol
                        && !self.instantiates_callable(argument.symbol, selected)
                        && self.entered_machine(selected) != self.entered_machine(argument.symbol))
            })
        {
            return Err(unserved("static call argument changed its authored selection"));
        }
        let selection_source = selection.source_span();
        let file = self
            .typed
            .symbols
            .source_file(selection_source)
            .ok_or_else(|| unserved("static call selection has no source file"))?;
        if selection_source.span.start >= selection_source.span.end
            || file
                .source
                .get(selection_source.span.start..selection_source.span.end)
                .is_none()
        {
            return Err(unserved(
                "static call selection has no authored source coordinate",
            ));
        }
        let target = if self
            .typed
            .trait_machine_signature(argument.symbol)
            .is_some()
        {
            argument.symbol
        } else {
            self.call_target_symbol(id, argument.symbol)?
        };
        self.visible_to_from(id, target, Some(*selection))?;
        Ok(selection.occurrence_id())
    }

    pub(super) fn retain_static_reach(
        &self,
        id: MachineId,
        mut call: Expression,
        binding: Option<StaticMachineTarget>,
    ) -> Checked<Expression> {
        if let Some(binding) = binding {
            let ExpressionKind::Call {
                callee: CallTarget::Machine(_),
                static_reach_binding,
                ..
            } = &mut call.kind
            else {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("static callback did not retain its exact machine invocation"),
                ));
            };
            *static_reach_binding = Some(binding.position);
        }
        Ok(call)
    }

    /// A call the typed trees bound to no declaration is refused before
    /// anything asks what its target is: an unbound symbol has no kind,
    /// parent or generated origin to read. Typing leaves such a call unbound
    /// when the callee is chosen from a receiver's type or the name resolves
    /// to nothing.
    pub(super) fn bound_target(&self, id: MachineId, target: SymbolHandle) -> Checked<()> {
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
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let mut read = self.read_call_statement(id, slots, call, source)?;
        crate::operational::retain(&mut read, call.operational_acknowledgement, true);
        Ok(read)
    }

    fn read_call_statement(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCall,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if let Some(runtime) = call.runtime_call {
            if call.target_symbol.is_valid()
                || call.authored_machine_application.is_valid()
                || !call.machine_arguments.is_empty()
                || !call.evidence_arguments.is_empty()
                || call.installed_interrupt_operation.is_some()
                || call.static_requirement_dispatch.is_some()
                || call.static_machine_parameter.is_valid()
            {
                return unserved("runtime call with static dispatch or evidence");
            }
            let arguments = self
                .typed
                .statement_table
                .expression_handles(call.arguments);
            return self.runtime_call(id, slots, runtime, arguments, source, None);
        }
        self.bound_target(id, call.target_symbol)?;
        if let Some(read) =
            self.rigid_call(id, slots, StaticArgumentCall::Statement(call), source)?
        {
            return Ok(read);
        }
        if let Some(dispatch) = &call.static_requirement_dispatch {
            return self.requirement_call(
                id,
                slots,
                StaticArgumentCall::Statement(call),
                dispatch,
                source,
            );
        }
        if call.installed_interrupt_operation.is_some() || call.reflected_field_origin.is_some() {
            return unserved("interrupt, requirement dispatch or reflected call");
        }
        if self.checking_generic_entry(id) {
            let invocation = self.static_invocation(StaticArgumentCall::Statement(call));
            if let Some(read) =
                self.generic_value_call(id, slots, &invocation, SourceOrigin::of(call.source_span))?
            {
                return Ok(read);
            }
            if let Some(arguments) = self.type_arguments(id, &invocation)? {
                return self.generic_call(
                    id,
                    slots,
                    invocation,
                    arguments,
                    SourceOrigin::of(call.source_span),
                );
            }
        }
        self.applies_nothing(
            id,
            call.target_symbol,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        let (authored, static_reach_binding) =
            self.static_machine_target(id, call.target_symbol, call.static_machine_parameter)?;
        self.direct_selection(id, authored, call.authored_call_selection, None)?;
        // A boundary qualifier names a requirement without supplying a
        // receiver. An actual value outside the argument list is refused.
        let receiver = call.receiver_root_symbol.is_valid()
            || call.receiver_symbol.is_valid()
            || !call.receiver.is_empty();
        let named = self.named_call_target(id, call.receiver_symbol, call.target_symbol)?;
        let qualified = named.is_some()
            || (call.receiver_root_symbol == call.receiver_symbol
                && call.receiver.len() == 1
                && self.names_requirement(call.target_symbol, call.receiver_symbol));
        if receiver && !qualified {
            return unserved("receiver call");
        }
        if !call.evidence_arguments.is_empty() {
            return unserved("evidence arguments");
        }
        let arguments = self
            .typed
            .statement_table
            .expression_handles(call.arguments);
        let read = if let Some(selected) = named {
            self.named_call(
                id,
                slots,
                selected,
                StaticArgumentCall::Statement(call),
                (arguments, source, None),
            )
        } else {
            self.call_selected(
                id,
                slots,
                call.target_symbol,
                arguments,
                source,
                None,
                static_reach_binding,
            )
        }?;
        self.retain_static_reach(id, read, static_reach_binding)
    }

    pub(super) fn call_expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCallExpression,
        handle: ExpressionHandle,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let mut read = self.read_call_expression(id, slots, call, handle, source)?;
        crate::operational::retain(&mut read, call.operational_acknowledgement, false);
        Ok(read)
    }

    fn read_call_expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCallExpression,
        handle: ExpressionHandle,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        // Taken before anything nested is read, so no argument sees it.
        let expected = self.expected.take();
        // A layout query names no declaration; typing kept its operand.
        if let Some(query) = &call.layout_query {
            return self.layout_query(id, call, query, source);
        }
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if let Some(runtime) = call.runtime_call {
            if call.target_symbol.is_valid()
                || call.authored_machine_application.is_valid()
                || !call.machine_arguments.is_empty()
                || !call.evidence_arguments.is_empty()
                || call.installed_interrupt_operation.is_some()
                || call.static_requirement_dispatch.is_some()
                || call.static_machine_parameter.is_valid()
            {
                return unserved("runtime call with static dispatch or evidence");
            }
            let arguments = self
                .typed
                .expression_table
                .expression_handles(call.arguments);
            return self.runtime_call(id, slots, runtime, arguments, source, expected);
        }
        self.bound_target(id, call.target_symbol)?;
        // The integer carrier query is answered only through its fields in a
        // static condition (`carriers`); its own body is no answer to run.
        if self.is_carrier_query(call) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Call("integer carrier query read as a runtime value"),
            ));
        }
        if let Some(description) = self.declaration_query(id, call, expected, source)? {
            return Ok(description);
        }
        if let Some(reference) = self.borrow_factory_call(id, slots, call, source)? {
            return Ok(reference);
        }
        if call.installed_interrupt_operation.is_some() {
            return unserved("installed interrupt operation");
        }
        if let Some(dispatch) = &call.static_requirement_dispatch {
            return self.requirement_call(
                id,
                slots,
                StaticArgumentCall::Expression(handle, call),
                dispatch,
                source,
            );
        }
        // In a generic check form the entry's call through an implementation
        // parameter targets that parameter's requirement child. Which
        // requirement the child stands for is the typed trees' own relation
        // (`evidence_requirement_declaration`), never the member's name.
        // A Build selection is neither: it is read below, where its
        // designated operands are attached, and nowhere else.
        if self.checking_generic_entry(id) && !call.build_selection.is_valid() {
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
            let invocation = self.static_invocation(StaticArgumentCall::Expression(handle, call));
            if let Some(read) = self.generic_value_call(id, slots, &invocation, source)? {
                return Ok(read);
            }
            if let Some(arguments) = self.type_arguments(id, &invocation)? {
                return self.generic_call(id, slots, invocation, arguments, source);
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
        let (authored, static_reach_binding) =
            self.static_machine_target(id, call.target_symbol, call.static_machine_parameter)?;
        self.direct_selection(
            id,
            authored,
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
        // Qualification names a declaration without supplying an extra
        // runtime argument. Typing owns that distinction for named operators
        // and machines; trait requirements retain their exact owner relation.
        let named = self.named_expression_target(id, call)?;
        let qualified = named.is_some()
            || self.typed.call_has_no_runtime_receiver_for_target(call)
            || (call.receiver.is_valid()
                && matches!(
                    self.node(id, call.receiver)?,
                    ExpressionNode::Name(path) if self.names_requirement(call.target_symbol, path.symbol)
                ));
        if (call.receiver.is_valid() && !qualified)
            || (call.requirement_application.is_valid() && named.is_none())
            || call.exact_coercion_target.is_valid()
        {
            return unserved("receiver, requirement or coercion call");
        }
        if !call.evidence_arguments.is_empty() {
            return unserved("evidence arguments");
        }
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        let mut read = if let Some(selected) = named {
            self.named_call(
                id,
                slots,
                selected,
                StaticArgumentCall::Expression(handle, call),
                (arguments, source, expected),
            )?
        } else {
            self.call_selected(
                id,
                slots,
                call.target_symbol,
                arguments,
                source,
                expected,
                static_reach_binding,
            )?
        };
        if call.build_selection.is_valid() {
            self.designate(id, call.build_selection, &mut read)?;
        }
        self.retain_static_reach(id, read, static_reach_binding)
    }

    /// A sealed ordinary borrow returns the source's existing permission.
    /// Its closed result belongs to the actual application, never to the
    /// type a caller hopes to receive. Other interpretations still need
    /// their evaluated policy, content, profile and resident correspondence.
    /// `size_of<T>()` and `align_of<T>()` (layouts/plans.md, "These forms
    /// take data types"). The operand is read as an ordinary value type, so
    /// `CLayout<R>` and `R` name the record rows placement and ABI use, and
    /// the observation measures exactly that row. Only a record row has a
    /// storage geometry this reader can name; a scalar, a rigid type
    /// parameter and the value form `align_of(view)` stay refused here
    /// rather than receive a fabricated measurement.
    pub(super) fn layout_query(
        &mut self,
        id: MachineId,
        call: &TableCallExpression,
        query: &typed_trees::expression::DataLayoutQueryRequest,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use language_core::data_layout_queries::DataLayoutQueryKind;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if query.nonempty_view.is_valid()
            || !query.operand.type_reference.is_valid()
            || !call.arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.receiver.is_valid()
        {
            return unserved("layout query without exactly one type operand");
        }
        let ValueType::Record(subject, _, _) = self.value_type(id, query.operand.type_reference)?
        else {
            return unserved("layout query of a carrier with no record row");
        };
        let measurement = match query.kind {
            DataLayoutQueryKind::Size => terminal_psi::LayoutMeasurement::Size,
            DataLayoutQueryKind::Alignment => terminal_psi::LayoutMeasurement::Alignment,
        };
        let scalar_type = semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .map_err(|_| refuse(id, Construct::Machine, Unserved::Call("u64 carrier")))?,
        );
        let application = self
            .scalar_applications
            .layout(subject, measurement, scalar_type)
            .map_err(|reason| refuse(id, Construct::Machine, Unserved::Call(reason)))?;
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::ScalarApplication { application },
            result: Some(ValueType::plain(scalar_type)),
            source,
        })
    }

    fn borrow_factory_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &TableCallExpression,
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        use typed_trees::expression::AccessBorrowBacking;
        use typed_trees::static_applications::StaticMachineBindingValue;
        use typed_trees::types::TypeReferenceNode;

        let typed = self.typed;
        let Some(factory) = typed.access_borrow_factory(call.target_symbol).copied() else {
            return Ok(None);
        };
        let unavailable = |reason| refuse(id, Construct::Machine, Unserved::Call(reason));
        self.visible_to(id, factory.factory)?;
        self.direct_selection(
            id,
            call.target_symbol,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        let namespace = typed
            .expression_table
            .name_path_node(call.receiver)
            .map_or_else(SymbolHandle::invalid, |path| path.symbol);
        if (call.receiver.is_valid() && !self.names_requirement(call.target_symbol, namespace))
            || call.build_selection.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.static_machine_parameter.is_valid()
            || !call.evidence_arguments.is_empty()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.installed_interrupt_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return Err(unavailable(
                "sealed borrow has another call operation or value receiver",
            ));
        }
        if !matches!(factory.backing, AccessBorrowBacking::Ordinary) {
            return Err(unavailable(
                "resident view needs evaluated layout, content, profile and policy custody",
            ));
        }
        let signature = typed
            .bodyless_callable_signature(call.authored_machine_application, call.target_symbol)
            .map_err(unavailable)?
            .ok_or_else(|| {
                unavailable("sealed borrow has no prepared closed declaration contract")
            })?;
        let application = typed
            .expression_table
            .static_machine_application(call.authored_machine_application);
        let actual_owner = typed
            .machine_by_symbol(application.lexical_owner)
            .or_else(|| typed.machine_holding_state(application.lexical_owner))
            .map(|machine| machine.symbol);
        if actual_owner != self.symbols.get(id).copied()
            || application.binding_declaration != factory.factory
            || (!call.machine_arguments.is_empty()
                && call.machine_arguments.as_ref()
                    != typed
                        .expression_table
                        .static_machine_application_arguments(call.authored_machine_application))
        {
            return Err(unavailable(
                "sealed borrow application belongs to another source call owner",
            ));
        }
        if !signature.type_parameters.is_empty()
            || !signature.conformance_bounds.is_empty()
            || !signature.contracts.is_empty()
            || !signature.invokes.is_empty()
            || signature.suspends
            || signature.blocks
            || signature.service_reach_is_installation_bound
            || signature.service_reach_row != language_semantics::ServiceReachRowTable::EMPTY_ROW
            || !signature.declares_no_crash_route
            || signature.termination_guarantee
                != language_semantics::TerminationGuarantee::NoGuarantee
            || !typed
                .service_reach_rows
                .services(signature.service_reach_row)
                .is_empty()
        {
            return Err(unavailable(
                "sealed borrow needs its complete static or operational contract",
            ));
        }
        let bindings = typed
            .expression_table
            .static_machine_application_bindings(call.authored_machine_application);
        let bound_type = |parameter| {
            let mut selected = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter);
            match (selected.next(), selected.next()) {
                (Some(binding), None) => match binding.value {
                    StaticMachineBindingValue::Type(reference) => Some(reference),
                    _ => None,
                },
                _ => None,
            }
        };
        let source_type = bound_type(factory.source_parameter)
            .ok_or_else(|| unavailable("sealed borrow lost its exact source type argument"))?;
        let view_type = bound_type(factory.view_parameter)
            .ok_or_else(|| unavailable("sealed borrow lost its complete view type argument"))?;
        let Some([parameter]) = typed.state_signature_parameters(signature).as_array() else {
            return Err(unavailable(
                "sealed borrow closed declaration needs one source parameter",
            ));
        };
        let TypeReferenceNode::Reference {
            referee: declared_source,
            access: source_access,
            ..
        } = typed
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return Err(unavailable(
                "sealed borrow source parameter is not its declared reference",
            ));
        };
        let TypeReferenceNode::Reference {
            referee: declared_view,
            access: result_access,
            ..
        } = typed
            .type_reference_table
            .type_reference(signature.return_type)
        else {
            return Err(unavailable(
                "sealed borrow result is not its declared reference",
            ));
        };
        if *declared_source != source_type
            || *declared_view != view_type
            || *source_access != factory.access
            || *result_access != factory.access
            || parameter.is_const
            || parameter.relevance.is_erased()
            || parameter.is_mutable != (factory.access == ReferenceAccess::Mutable)
        {
            return Err(unavailable(
                "sealed borrow closed signature differs from its source and view binders",
            ));
        }
        let source_value = self.local_type(id, source_type)?;
        let view_value = self.local_type(id, view_type)?;
        if !self.same_type(Some(source_value), view_value) {
            return Err(unavailable(
                "a different qualified view needs evaluated policy, layout and backing correspondence",
            ));
        }
        let result = self.local_type(id, signature.return_type)?;
        let [argument] = typed.expression_table.expression_handles(call.arguments) else {
            return Err(unavailable(
                "sealed borrow needs exactly one actual source argument",
            ));
        };
        Ok(Some(
            self.borrow_value(id, slots, *argument, result, source)?,
        ))
    }

    /// Attaches a Build selection's designated operands to the call typing
    /// spelled for it. scoped_execution.md: the operations "themselves run
    /// as admitted Build operations. No product receiver, function body,
    /// initializer, or provider is executed." So the call stays the
    /// ordinary boundary call it is, and the reference to the declarations
    /// it names rides beside it to the operation lowering emits.
    ///
    /// Only an operation of the toolchain's own selection protocol receives
    /// one: the Build host hands the reference to no other callee. Companion
    /// production then authenticates each request's complete application and
    /// physical argument shape against the accepted boundary declaration.
    fn designate(
        &mut self,
        id: MachineId,
        selection: arena::Handle<typed_trees::expression::BuildSelectionCall>,
        read: &mut Expression,
    ) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        let Some(request) = self
            .typed
            .expression_table
            .build_selection_request(selection)
        else {
            return unserved("Build selection without its designated operands");
        };
        let ExpressionKind::Call {
            callee: CallTarget::Boundary { boundary },
            ..
        } = &read.kind
        else {
            return unserved("Build selection on a call that is no boundary operation");
        };
        let protocol = self
            .boundaries
            .row(*boundary)
            .and_then(|row| self.typed.trait_machine_signature(row.source))
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
        // Provider readiness names a boundary trait application. Its host
        // row states that application and the boundaries it covers, so the
        // application is retained as the same service row a call of one of
        // its requirements uses (provider_selection.md, "Build provider
        // readiness"). The route declares boundaries only of traits with no
        // parameters (`boundary`), so an applied requirement stays refused.
        let readiness = match request {
            BuildSelectionRequest::ProviderReadiness(readiness) => Some(readiness.requirement),
            BuildSelectionRequest::Entry(_)
            | BuildSelectionRequest::Declaration(_)
            | BuildSelectionRequest::Exclusion(_) => None,
        };
        let readiness = match readiness {
            Some(requirement) => {
                let typed = self.typed;
                let lexical_owner = *self.symbols.get(id).ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("Build provider readiness without its selecting machine"),
                    )
                })?;
                let scope = typed
                    .build_provider_requirement_scope(requirement, lexical_owner)
                    .map_err(|reason| refuse(id, Construct::Machine, Unserved::Call(reason)))?;
                if !typed.trait_type_parameters(scope.declaration).is_empty()
                    || !scope.declaration.lifetime_parameters.is_empty()
                {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call(
                            "Build provider readiness for a requirement applied to arguments",
                        ),
                    ));
                }
                let service = self.service(id, scope)?;
                self.awaits(service);
                Some(service)
            }
            None => None,
        };
        let ExpressionKind::Call { designated, .. } = &mut read.kind else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Call("Build selection on a call that is no boundary operation"),
            ));
        };
        *designated = Some(crate::body::DesignatedOperands {
            request: selection,
            selecting: id,
            readiness,
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
        call: StaticArgumentCall<'_>,
        dispatch: &typed_trees::StaticRequirementDispatch,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        let (unsupported, target, application_handle, raw_arguments, arguments, binding) =
            match call {
                StaticArgumentCall::Expression(_, call) => (
                    call.runtime_call.is_some()
                        || call.build_selection.is_valid()
                        || call.quotient_operation.is_some()
                        || call.private_layout_operation.is_some()
                        || call.layout_query.is_some()
                        || call.native_code_layout.is_some()
                        || call.installed_interrupt_operation.is_some()
                        || call.receiver.is_valid()
                        || call.requirement_application.is_valid()
                        || call.exact_coercion_target.is_valid()
                        || call.static_machine_parameter.is_valid()
                        || !call.evidence_arguments.is_empty(),
                    call.target_symbol,
                    call.authored_machine_application,
                    call.machine_arguments.as_ref(),
                    self.typed
                        .expression_table
                        .expression_handles(call.arguments),
                    call.authored_call_binding,
                ),
                StaticArgumentCall::Statement(call) => (
                    call.installed_interrupt_operation.is_some()
                        || call.reflected_field_origin.is_some()
                        || call.receiver_root_symbol.is_valid()
                        || call.receiver_symbol.is_valid()
                        || !call.receiver.is_empty()
                        || call.static_machine_parameter.is_valid()
                        || !call.evidence_arguments.is_empty(),
                    call.target_symbol,
                    call.authored_machine_application,
                    call.machine_arguments.as_ref(),
                    self.typed
                        .statement_table
                        .expression_handles(call.arguments),
                    None,
                ),
            };
        if unsupported {
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
            dispatch.realization_machine,
            application_handle,
            raw_arguments,
            false,
        )?;
        if !dispatch.evidence_parameter.is_valid() {
            return unserved("static requirement dispatch");
        }
        // An evidence parameter keeps its exact authored child. A named map
        // is authenticated by the same retained application and contract scope.
        if dispatch.evidence_parameter.is_valid() {
            let original = call.registration_operation(self.typed);
            if !self.typed.symbols.symbols().nodes().is_valid(original) {
                return unserved("requirement dispatch lost its original source operation");
            }
            let declared = self
                .typed
                .evidence_requirement_declaration(original)
                .map_err(|reason| {
                    let mut refusal = refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("implementation parameter requirement"),
                    );
                    refusal.detail.push_str(": ");
                    refusal.detail.push_str(&reason);
                    refusal
                })?;
            if self.typed.symbols.get(original).parent != dispatch.evidence_parameter
                || declared.is_none_or(|required| required.symbol != dispatch.requirement)
                || binding.is_some_and(|binding| {
                    binding.selected != binding.original || binding.original != original
                })
            {
                return unserved(
                    "requirement dispatch not bound to its exact parameter requirement",
                );
            }
        }
        if !dispatch.realization_machine.is_valid() {
            return unserved("requirement dispatch with no realization");
        }
        // The call targets the member that realizes the requirement, or,
        // when the call binds value parameters, the machine stage 04
        // generated from that member for them.
        let target = if dispatch.realization_state == target {
            dispatch.realization_machine
        } else if !value_arguments.is_empty()
            && self
                .typed
                .executable_callable_template(target)
                .map_err(|what| refuse(id, Construct::Machine, Unserved::Call(what)))?
                == Some(dispatch.realization_state)
        {
            target
        } else {
            return unserved("requirement dispatch that targets another machine");
        };
        let caller = call.lexical_owner(self.typed);
        if !self.typed.symbols.symbols().nodes().is_valid(caller) {
            return unserved("selected call lost its actual static argument owner");
        }
        let (operand, operand_owner) = self
            .typed
            .selected_implementation_operand(caller, dispatch.evidence_parameter)
            .map_err(|reason| refuse(id, Construct::Machine, Unserved::Call(reason)))?;
        let operand = operand.clone();
        let required_source = self
            .typed
            .source_call_requirement_signature(caller, call)
            .map_err(|reason| {
                let mut refusal = refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("applied public requirement"),
                );
                refusal.detail.push_str(": ");
                refusal.detail.push_str(&reason);
                refusal
            })?;
        let machine = self.call_target_symbol(id, target)?;
        let callee = self.machine(machine)?;
        self.selected_call(
            id,
            slots,
            (
                callee,
                dispatch.clone(),
                operand,
                operand_owner,
                value_arguments,
                None,
            ),
            required_source,
            (arguments, source, None),
        )
    }

    /// All selected calls use the same public-header, argument and emission
    /// path. The selection describes the choice; normal refinement checking
    /// still owes the body's contract independently.
    fn selected_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        selection: (
            MachineId,
            typed_trees::StaticRequirementDispatch,
            typed_trees::expression::StaticMachineArgument,
            SymbolHandle,
            Vec<semantic_vocabulary::ScalarTerm>,
            Option<terminal_check_tables::program::SelectedStaticRecordOperand>,
        ),
        required_source: &'t typed_trees::signature::StateSignature,
        (arguments, source, expected): (&[ExpressionHandle], SourceOrigin, Option<ValueType>),
    ) -> Checked<Expression> {
        let (callee, dispatch, operand, operand_owner, value_arguments, static_record) = selection;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        let (required, _) = self.requirement_signature(
            id,
            required_source,
            if required_source.type_parameters.is_empty() {
                &[]
            } else {
                &value_arguments
            },
        )?;
        let machine = *self.symbols.get(callee).ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Call("selected body lost its exact source declaration"),
            )
        })?;
        let supplied = self.signatures.get(callee).cloned().ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Call("selected body has no signature"),
            )
        })?;
        if !self.same_signature(id, &required, &supplied, machine)? {
            return unserved("selected body has another runtime signature");
        }
        if !self.typed.symbols.symbols().nodes().is_valid(operand_owner) {
            return unserved("selected call lost its actual operand supplier");
        }
        let Some(operand_identity) = self
            .typed
            .declared_static_operand_identity(
                typed_trees::type_identity::DeclaredStaticOperandIdentityRequest {
                    operand: typed_trees::type_identity::IndexIdentityArgument::StaticArgument(
                        &operand,
                    ),
                    lexical_owner: operand_owner,
                    binders: &[],
                    lifetime_binders: &[],
                    exact_toolchain_sources: self
                        .shared
                        .exact_toolchain_sources(self.typed)
                        .map_err(|reason| super::services::unavailable(id, reason))?,
                },
            )
            .and_then(|identity| self.typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
        else {
            return unserved("selected call lost its complete implementation operand identity");
        };
        let envelope = self.signature_envelope(id, required_source)?;
        let required_contract = self.declaration_signatures.push(required);
        if self.declaration_envelopes.push(envelope) != required_contract {
            return unserved("selected contract lost its complete envelope coordinate");
        }
        if self.declaration_sources.push(required_source) != required_contract
            || self.declaration_selections.push(Some((
                id,
                callee,
                dispatch.clone(),
                operand,
                operand_owner,
            ))) != required_contract
        {
            return unserved("selected contract lost its declaration coordinate");
        }
        let read = self.call_of(
            id,
            slots,
            (callee.into(), Some(required_contract)),
            arguments,
            source,
            expected,
        )?;
        self.selections.push(crate::Selected {
            caller: id,
            required_contract: Some(required_contract),
            source_operand: Some(terminal_check_tables::program::SelectedSourceOperand {
                identity: operand_identity,
                supplier: operand_owner,
                static_record,
            }),
            parameter: dispatch.evidence_parameter,
            declaring_trait: dispatch.declaring_trait,
            requirement: dispatch.requirement,
            implementation: callee,
            value_arguments,
        });
        Ok(read)
    }

    /// Close the authenticated named row through the same selected-call path.
    /// Its public contract does not make the body directly callable.
    fn named_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        (body, owner, row, required): (
            SymbolHandle,
            &'t typed_trees::trait_definition::Conformance,
            &'t typed_trees::trait_definition::ConformanceRow,
            &'t typed_trees::signature::StateSignature,
        ),
        call: StaticArgumentCall<'_>,
        (arguments, source, expected): (&[ExpressionHandle], SourceOrigin, Option<ValueType>),
    ) -> Checked<Expression> {
        let machine = self.typed.machine_by_symbol(body).ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Call("named call lost its actual body"),
            )
        })?;
        let Some([entry]) = self.typed.machine_states(machine).as_array() else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Call("named call lost its actual entry"),
            ));
        };
        let dispatch = typed_trees::StaticRequirementDispatch {
            evidence_parameter: SymbolHandle::invalid(),
            declaring_trait: row.declaring_trait,
            requirement: row.requirement,
            realization_machine: body,
            realization_state: entry.symbol,
        };
        let lexical_owner = *self.symbols.get(id).ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Call("named member lost its actual source caller"),
            )
        })?;
        let (operand, supplier) = self
            .typed
            .source_call_selected_operand(lexical_owner, call)
            .map_err(|reason| {
                let mut error = refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("named call lost its retained source operand"),
                );
                error.detail.push_str(": ");
                error.detail.push_str(&reason);
                error
            })?;
        if operand.symbol != owner.symbol {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Call("named call selected another source implementation"),
            ));
        }
        let mut operand = operand.into_owned();
        let mut supplier = supplier;
        let static_record = if let Some((actual, actual_supplier, binding)) =
            self.selected_record_operand(id, call, &operand)?
        {
            operand = actual;
            supplier = actual_supplier;
            Some(binding)
        } else {
            None
        };
        let callee = self.machine(body)?;
        self.selected_call(
            id,
            slots,
            (
                callee,
                dispatch,
                operand,
                supplier,
                Vec::new(),
                static_record,
            ),
            required,
            (arguments, source, expected),
        )
    }

    /// Read exact retained value bindings through the ordinary scalar owner.
    /// Declaration order determines the tuple; each binder must occur once.
    pub(super) fn bound_scalar_values(
        &self,
        id: MachineId,
        binders: &[(SymbolHandle, semantic_vocabulary::ScalarType)],
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
    ) -> Checked<Vec<semantic_vocabulary::ScalarTerm>> {
        self.bound_scalar_values_in_scope(id, binders, application, false)
    }

    fn bound_scalar_values_in_scope(
        &self,
        id: MachineId,
        binders: &[(SymbolHandle, semantic_vocabulary::ScalarType)],
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
        rigid: bool,
    ) -> Checked<Vec<semantic_vocabulary::ScalarTerm>> {
        use typed_trees::static_applications::StaticMachineBindingValue;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let bindings = self
            .typed
            .expression_table
            .static_machine_application_bindings(application);
        if bindings.len() != binders.len() {
            return Err(unserved(
                "value application has an incomplete binding tuple",
            ));
        }
        let mut ordered = bindings.iter().collect::<Vec<_>>();
        ordered.sort_unstable_by_key(|binding| {
            (
                binding.parameter.arena_index(),
                binding.parameter.generation(),
            )
        });
        if ordered
            .windows(2)
            .any(|pair| pair[0].parameter == pair[1].parameter)
        {
            return Err(unserved("value application repeats a binder"));
        }
        let mut values = Vec::with_capacity(binders.len());
        for &(parameter, scalar_type) in binders {
            let Ok(position) = ordered.binary_search_by_key(
                &(parameter.arena_index(), parameter.generation()),
                |binding| {
                    (
                        binding.parameter.arena_index(),
                        binding.parameter.generation(),
                    )
                },
            ) else {
                return Err(unserved("value parameter has no unique retained binding"));
            };
            let binding = ordered[position];
            let StaticMachineBindingValue::Const(reference) = binding.value else {
                return Err(unserved("value parameter is not bound to a static value"));
            };
            values.push(self.bound_scalar_value(id, scalar_type, reference, rigid)?);
        }
        Ok(values)
    }

    /// One canonical scalar Const leaf, shared by scalar-only and mixed schema applications.
    pub(super) fn bound_scalar_value(
        &self,
        id: MachineId,
        scalar_type: semantic_vocabulary::ScalarType,
        reference: typed_trees::types::TypeReferenceHandle,
        rigid: bool,
    ) -> Checked<semantic_vocabulary::ScalarTerm> {
        use semantic_vocabulary::{ScalarTerm, ScalarType};
        use typed_trees::types::PrimitiveType;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err(unserved("static value lost its retained type reference"));
        }
        if rigid
            && let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
                self.typed.type_reference_table.type_reference(reference)
            && let Some(input) = self.const_scope.iter().find(|input| {
                input.symbol == *symbol && input.owner == id && input.scalar_type == scalar_type
            })
        {
            return Ok(ScalarTerm::value(input.value, scalar_type));
        }
        let primitive = match scalar_type {
            ScalarType::Integer(carrier) if !carrier.is_address() => {
                let fixed = carrier
                    .bits()
                    .and_then(|bits| {
                        numerics::integer_carrier::FixedInteger::new(
                            carrier.sign(),
                            u64::from(bits),
                        )
                    })
                    .ok_or_else(|| unserved("static value has an unresolved integer carrier"))?;
                PrimitiveType::Integer(fixed)
            }
            ScalarType::Boolean => PrimitiveType::Bool,
            _ => {
                return Err(unserved(
                    "value parameter has an unsupported scalar carrier",
                ));
            }
        };
        let operand = self
            .typed
            .static_operand_from_type_reference(reference)
            .map_err(unserved)?;
        let value = self
            .typed
            .prepare_callable_scalar_argument(primitive, &operand)
            .map_err(unserved)?;
        Ok(match value {
            ExpressionNode::Integer(literal) => {
                let (value, scalar_type) = self.literal(id, &literal, Some(scalar_type))?;
                ScalarTerm::Integer { scalar_type, value }
            }
            ExpressionNode::Boolean(value) => ScalarTerm::Boolean(value),
            _ => return Err(unserved("static value did not retain its scalar meaning")),
        })
    }

    /// The canonical values a call binds the requirement's parameters to.
    /// The retained declaration supplies its exact binder association;
    /// the authored list remains checked against that same application.
    /// With no value parameter the call may carry no static argument.
    fn static_value_arguments(
        &self,
        id: MachineId,
        binders: &[(SymbolHandle, semantic_vocabulary::ScalarType)],
        declaration: SymbolHandle,
        application_handle: typed_trees::static_applications::StaticMachineApplicationHandle,
        authored_arguments: &[typed_trees::expression::StaticMachineArgument],
        rigid: bool,
    ) -> Checked<Vec<semantic_vocabulary::ScalarTerm>> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        let table = &self.typed.expression_table;
        if application_handle.is_valid()
            && !table.static_machine_application_is_valid(application_handle)
        {
            return unserved("requirement call with a stale method application");
        }
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
        if binders.is_empty() {
            return Ok(Vec::new());
        }
        if application.is_none_or(|application| application.binding_declaration != declaration) {
            return unserved("requirement call binds another declaration");
        }
        let parameters = if let Some(machine) = self.typed.machine_by_symbol(declaration) {
            self.typed.machine_callable_parameters(machine)
        } else if let Some((_, requirement)) = self.typed.trait_machine_signature(declaration) {
            self.typed
                .state_signature_type_parameters(requirement)
                .to_vec()
        } else {
            return unserved("requirement call lost its binding declaration");
        };
        let binders = self.value_binder_correspondence(id, binders, (&parameters).into())?;
        self.bound_scalar_values_in_scope(id, &binders, application_handle, rigid)
    }

    /// The types a call in a generic check form's entry applies its callee's
    /// type parameters at, in the callee's declaration order, when the call
    /// is a type application: every binding of its static application gives
    /// one type parameter of the callee a type. `None` for any other call.
    /// Arguments may be opaque binders or concrete members of a checked
    /// finite family. Both call the callee's contract here; its own generic
    /// forms still check every admitted body independently of this caller.
    fn type_arguments(
        &mut self,
        id: MachineId,
        call: &StaticInvocation<'_>,
    ) -> Checked<Option<Vec<crate::body::ValueType>>> {
        use typed_trees::static_applications::StaticMachineBindingValue;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if !call.application.is_valid() {
            return Ok(None);
        }
        let typed = self.typed;
        let table = &typed.expression_table;
        let bindings = table.static_machine_application_bindings(call.application);
        if bindings.is_empty()
            || bindings
                .iter()
                .any(|binding| !matches!(binding.value, StaticMachineBindingValue::Type(_)))
        {
            return Ok(None);
        }
        let application = table.static_machine_application(call.application);
        if !application.selected_machine_arguments.is_empty()
            || !application.input_selections.is_empty()
            || !application.lifetime_bindings.is_empty()
            || application.conformance_selection.is_some()
            || application.callable_contract_source.is_valid()
        {
            return unserved("type application with another static feature");
        }
        let owner = match typed.symbols.get(call.target).kind {
            SymbolKind::State => typed.symbols.get(call.target).parent,
            SymbolKind::Machine => call.target,
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
            arguments.push(argument);
        }
        Ok(Some(arguments))
    }

    /// A const application in a generic body forwards actual rigid inputs as
    /// ordinary check-form operands. The callee keeps its own formal IDs;
    /// the checker independently authenticates dependent array correspondence.
    fn generic_value_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &StaticInvocation<'_>,
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        let target = self.call_target_symbol(id, call.target)?;
        let Some(machine) = self.typed.machine_by_symbol(target) else {
            return Ok(None);
        };
        let parameters = self.typed.machine_type_parameters(machine);
        if parameters.is_empty()
            || parameters.iter().any(|parameter| {
                !matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Const { .. }
                )
            })
        {
            return Ok(None);
        }
        if call.unsupported || call.runtime_receiver || !machine.conformance_bounds.is_empty() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Call("const application with another call feature"),
            ));
        }
        self.direct_selection(id, call.target, call.selection, call.binding)?;
        let callee = self.const_application(target)?;
        let values =
            self.const_call_values(id, target, call.application, call.authored_arguments)?;
        let operands = self.const_operands(id, slots, &values, source)?;
        self.call_of_values(
            id,
            slots,
            (callee.into(), None),
            call.arguments,
            source,
            None,
            &operands,
        )
        .map(Some)
    }

    fn const_operands(
        &self,
        id: MachineId,
        slots: &[Slot],
        values: &[semantic_vocabulary::ScalarTerm],
        source: SourceOrigin,
    ) -> Checked<Vec<Expression>> {
        use semantic_vocabulary::ScalarTerm;
        let mut operands = Vec::with_capacity(values.len());
        for value in values {
            let scalar_type = value.scalar_type();
            let kind = match value {
                ScalarTerm::Value { id: value, .. } => {
                    let input = self
                        .const_scope
                        .iter()
                        .find(|input| {
                            input.value == *value
                                && input.owner == id
                                && input.scalar_type == scalar_type
                        })
                        .ok_or_else(|| {
                            refuse(
                                id,
                                Construct::Machine,
                                Unserved::Call("const actual has no caller input"),
                            )
                        })?;
                    let slot = slots
                        .iter()
                        .position(|slot| slot.symbol == input.symbol)
                        .ok_or_else(|| {
                            refuse(
                                id,
                                Construct::Machine,
                                Unserved::Call("const actual has no caller slot"),
                            )
                        })?;
                    ExpressionKind::Slot(slot)
                }
                ScalarTerm::Integer { value, .. } => ExpressionKind::Integer(*value),
                ScalarTerm::Boolean(value) => ExpressionKind::Boolean(*value),
                _ => {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("const actual is neither a literal nor an owned input"),
                    ));
                }
            };
            operands.push(Expression {
                call_context: None,
                kind,
                result: Some(ValueType::plain(scalar_type)),
                source,
            });
        }
        Ok(operands)
    }

    /// A generic check form calls the callee's contract at the entry's exact
    /// opaque or concrete type bindings (`application`).
    fn generic_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: StaticInvocation<'_>,
        type_arguments: Vec<crate::body::ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if call.unsupported || call.runtime_receiver {
            return unserved("type application with another call feature");
        }
        self.direct_selection(id, call.target, call.selection, call.binding)?;
        // A call names its callee's entry state; the machine is that
        // state's parent, and must have exactly that one state.
        let machine = match self.typed.symbols.get(call.target).kind {
            SymbolKind::Machine => call.target,
            SymbolKind::State => {
                let parent = self.typed.symbols.get(call.target).parent;
                let (_, state) = self.declaration(id, parent)?;
                if state.symbol != call.target {
                    return unserved("state other than the entry");
                }
                parent
            }
            other => return unserved(symbol_kind(other)),
        };
        let callee = self.application(id, machine, type_arguments)?;
        self.call_of(
            id,
            slots,
            (callee.into(), None),
            call.arguments,
            source,
            None,
        )
    }

    /// The entry of a generic check form calling through a rigid
    /// implementation or structural machine parameter: a direct call
    /// read under the binder's published requirement.
    fn rigid_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: StaticArgumentCall<'_>,
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        if !self.checking_generic_entry(id) {
            return Ok(None);
        }
        self.read_rigid_call(id, slots, self.static_invocation(call), source)
    }

    /// Both source call forms expose one retained static application and the
    /// same authored selection. Their operands still belong to their own arena.
    fn static_invocation<'call>(&self, call: StaticArgumentCall<'call>) -> StaticInvocation<'call>
    where
        't: 'call,
    {
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
                    call.runtime_call.is_some()
                        || call.build_selection.is_valid()
                        || call.quotient_operation.is_some()
                        || call.private_layout_operation.is_some()
                        || call.installed_interrupt_operation.is_some()
                        || call.layout_query.is_some()
                        || call.native_code_layout.is_some()
                        || call.static_requirement_dispatch.is_some()
                        || call.requirement_application.is_valid()
                        || call.exact_coercion_target.is_valid()
                        || call.static_machine_parameter.is_valid()
                        || !call.evidence_arguments.is_empty(),
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
                    call.runtime_call.is_some()
                        || call.installed_interrupt_operation.is_some()
                        || call.static_requirement_dispatch.is_some()
                        || call.reflected_field_origin.is_some()
                        || call.static_machine_parameter.is_valid()
                        || !call.evidence_arguments.is_empty(),
                ),
            };
        let runtime_receiver = match call {
            StaticArgumentCall::Expression(_, call) => call.receiver.is_valid(),
            StaticArgumentCall::Statement(call) => {
                call.receiver_root_symbol.is_valid()
                    || call.receiver_symbol.is_valid()
                    || !call.receiver.is_empty()
            }
        };
        let receiver = match call {
            StaticArgumentCall::Expression(_, call) => self
                .typed
                .expression_table
                .name_path_node(call.receiver)
                .map(|path| path.symbol),
            StaticArgumentCall::Statement(call) => call
                .receiver_symbol
                .is_valid()
                .then_some(call.receiver_symbol),
        };
        StaticInvocation {
            target,
            receiver,
            application,
            authored_arguments,
            arguments,
            selection,
            binding,
            unsupported,
            runtime_receiver,
        }
    }

    /// A named transition publishes the same binder contract as a call
    /// expression. Its syntax has no separate static argument application.
    pub(super) fn rigid_named_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        target: SymbolHandle,
        receiver: Option<SymbolHandle>,
        arguments: &[ExpressionHandle],
        selection: Option<AuthoredDeclarationSelectionOccurrenceId>,
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        if !self.checking_generic_entry(id) {
            return Ok(None);
        }
        self.read_rigid_call(
            id,
            slots,
            StaticInvocation {
                target,
                application: StaticMachineApplicationHandle::invalid(),
                authored_arguments: &[],
                arguments,
                selection,
                binding: None,
                unsupported: false,
                runtime_receiver: false,
                receiver,
            },
            source,
        )
    }

    fn read_rigid_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        invocation: StaticInvocation<'_>,
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        let StaticInvocation {
            target,
            application,
            authored_arguments,
            arguments,
            selection,
            binding,
            unsupported,
            runtime_receiver: _,
            receiver,
        } = invocation;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let record = if let Some(receiver) = receiver {
            self.record_requirement_signature(id, receiver, target)?
                .map(|(position, signature)| (receiver, position, signature))
        } else {
            None
        };
        let requirement = if let Some((_, _, signature)) = &record {
            signature
        } else {
            match self.typed.evidence_requirement_declaration(target) {
                Ok(Some(requirement)) => requirement,
                Ok(None) => {
                    let Some(typed_trees::data::MachineParameterContractView::Structural(
                        requirement,
                    )) = self.typed.retained_static_machine_contract(target)
                    else {
                        return Ok(None);
                    };
                    let owner = self
                        .symbols
                        .get(id)
                        .and_then(|owner| self.typed.machine_by_symbol(*owner));
                    if owner
                        .and_then(|owner| self.typed.machine_parameter_signature_in(owner, target))
                        != Some(requirement)
                    {
                        return Err(unserved(
                            "static machine parameter outside its declaration scope",
                        ));
                    }
                    requirement
                }
                Err(_) => {
                    return Err(unserved(
                        "implementation parameter child with no requirement",
                    ));
                }
            }
        };
        if unsupported {
            return Err(unserved("rigid parameter call with another call feature"));
        }
        // Both source forms keep their real authored selection. A call with
        // no binding record cannot acquire one from its target's spelling.
        if selection.is_none() && binding.is_none() {
            return Err(unserved(
                "rigid parameter call not bound to its requirement",
            ));
        }
        self.direct_selection(id, target, selection, binding)?;
        let binders = self.requirement_binders(id, requirement)?;
        let value_arguments = self.static_value_arguments(
            id,
            &binders,
            requirement.symbol,
            application,
            authored_arguments,
            false,
        )?;
        let callee = if let Some((receiver, position, _)) = record.as_ref() {
            self.record_requirement(*receiver, *position, requirement, &value_arguments)?
        } else {
            self.requirement(requirement, &value_arguments)?
        };
        self.call_of(id, slots, (callee.into(), None), arguments, source, None)
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
        self.call_from(id, slots, target, arguments, source, expected, None)
    }

    pub(super) fn call_selected(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
        expected: Option<crate::body::ValueType>,
        selection: Option<StaticMachineTarget>,
    ) -> Checked<Expression> {
        match selection {
            None => self.call(id, slots, target, arguments, source, expected),
            Some(selection) => self.call_from(
                id,
                slots,
                target,
                arguments,
                source,
                expected,
                Some(selection),
            ),
        }
    }

    fn call_from(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
        expected: Option<crate::body::ValueType>,
        selection: Option<StaticMachineTarget>,
    ) -> Checked<Expression> {
        if selection.is_some_and(|selection| selection.target != target) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Call("static call changed its selected target"),
            ));
        }
        if !target.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("callee without a symbol"),
            ));
        }
        // Resolve executable entries to their machine owner. Trait requirements
        // also use state symbols, but keep their own signature identity.
        let target = if self.typed.machine_by_own_or_state_symbol(target).is_some() {
            self.call_target_symbol(id, target)?
        } else {
            target
        };
        match selection {
            Some(selection) => {
                let occurrence = self
                    .typed
                    .authored_declaration_selections()
                    .get(selection.occurrence)
                    .ok_or_else(|| {
                        refuse(
                            id,
                            Construct::Machine,
                            Unserved::Call("static call argument lost its authored selection"),
                        )
                    })?;
                self.visible_to_from(id, target, Some(*occurrence))?;
            }
            None => self.visible_to(id, target)?,
        }
        if let Some(fence) = self.atomic_fence(id, slots, target, arguments, source)? {
            return Ok(fence);
        }
        if let Some((boundary, signature)) = self.read_boundary_declaration(id, target)? {
            return self.call_with_signature(
                id,
                slots,
                (CallTarget::Boundary { boundary }, None),
                signature,
                arguments,
                source,
                expected,
            );
        }
        // The callee's formals are the arguments' expected types, so its
        // signature is read first; reading order is not evaluation order.
        let inherited_const = self.checking_generic_entry(id)
            && self.typed.machine_by_symbol(target).is_some_and(|machine| {
                let parameters = self.typed.machine_type_parameters(machine);
                !parameters.is_empty()
                    && parameters.iter().all(|parameter| {
                        matches!(
                            parameter.kind,
                            typed_trees::data::TypeParameterKind::Const { .. }
                        ) && self.typed.symbols.get(parameter.symbol).parent != target
                            && self.generic_binders.contains(&parameter.symbol)
                    })
            });
        let callee = if inherited_const {
            self.const_application(target)?
        } else {
            match self.inherited_type_arguments(id, target) {
                Some(arguments) => self.application(id, target, arguments)?,
                None => self.machine(target)?,
            }
        };
        if inherited_const {
            let values =
                self.const_call_values(id, target, StaticMachineApplicationHandle::invalid(), &[])?;
            let operands = self.const_operands(id, slots, &values, source)?;
            self.call_of_values(
                id,
                slots,
                (callee.into(), None),
                arguments,
                source,
                expected,
                &operands,
            )
        } else {
            self.call_of(
                id,
                slots,
                (callee.into(), None),
                arguments,
                source,
                expected,
            )
        }
    }

    /// A direct entry into a lexical child carries the caller's own
    /// telescope even when the call has no explicit static application.
    /// Read the child's signature at those same rigid types. The existing
    /// application owner also requests the child's independent body check.
    fn inherited_type_arguments(
        &self,
        id: MachineId,
        target: SymbolHandle,
    ) -> Option<Vec<crate::body::ValueType>> {
        if !self.checking_generic_entry(id) {
            return None;
        }
        let caller = *self.symbols.get(id)?;
        let mut enclosing = self.typed.symbols.get(target).parent;
        while enclosing.is_valid() && enclosing != caller {
            enclosing = self.typed.symbols.get(enclosing).parent;
        }
        if enclosing != caller {
            return None;
        }
        let machine = self.typed.machine_by_symbol(target)?;
        let parameters = self.typed.machine_type_parameters(machine);
        if parameters.is_empty() || !machine.conformance_bounds.is_empty() {
            return None;
        }
        parameters
            .iter()
            .map(|parameter| {
                self.scope
                    .iter()
                    .find_map(|(binder, value)| (*binder == parameter.symbol).then_some(*value))
            })
            .collect()
    }

    /// Resolve the same exact entry identity for direct and selected calls.
    /// Selection decides which contract is exposed; it does not change which
    /// source machine owns this entry state.
    fn call_target_symbol(&self, id: MachineId, target: SymbolHandle) -> Checked<SymbolHandle> {
        Ok(match self.typed.symbols.get(target).kind {
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
        })
    }

    /// Refuses a call from machine `id` to a machine of another package that
    /// is not public. Which package a declaration belongs to is the package
    /// of the source file its symbol was declared in, as for subjects
    /// (`subjects`); a declaration with no source file cannot be placed and
    /// is refused. Resolution supplies identity, not permission: modules.md,
    /// "Visibility" makes independently nameable machines private unless
    /// public, regardless of their qualifier or argument types.
    pub(super) fn visible_to(&self, id: MachineId, callee: SymbolHandle) -> Checked<()> {
        self.visible_to_from(id, callee, None)
    }

    fn visible_to_from(
        &self,
        id: MachineId,
        callee: SymbolHandle,
        selection: Option<AuthoredDeclarationSelection>,
    ) -> Checked<()> {
        // A machine stage 04 generated is called as its authored template,
        // whose visibility it has.
        let declared = crate::template_of(self.typed, callee).unwrap_or(callee);
        let (declaration, is_public) = self
            .typed
            .machine_by_own_or_state_symbol(declared)
            .map(|declaration| (declaration.symbol, declaration.is_public))
            .or_else(|| {
                self.typed
                    .trait_machine_signature(callee)
                    .map(|(owner, _)| (callee, owner.is_public))
            })
            .or_else(|| {
                self.typed
                    .conformance_by_symbol(callee)
                    .map(|declaration| (declaration.symbol, declaration.is_public))
            })
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("target has no callable declaration"),
                )
            })?;
        self.visible_declaration_from(id, declaration, is_public, selection)
    }

    /// Visibility follows the selected public declaration. A named map may
    /// demand its private body, while a direct call still checks the body's
    /// own declaration above.
    fn visible_declaration_to(
        &self,
        id: MachineId,
        callee: SymbolHandle,
        is_public: bool,
    ) -> Checked<()> {
        self.visible_declaration_from(id, callee, is_public, None)
    }

    fn visible_declaration_from(
        &self,
        id: MachineId,
        callee: SymbolHandle,
        is_public: bool,
        selection: Option<AuthoredDeclarationSelection>,
    ) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if is_public {
            return Ok(());
        }
        if selection.is_some_and(|selection| {
            selection.exposure() == AuthoredDeclarationSelectionExposure::PublicInterface
        }) {
            return unserved("private machine in a public interface");
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
        let caller = match selection {
            Some(selection) => symbols
                .source_file(selection.source_span())
                .map(|file| (file.origin, file.package_identity)),
            None => self.symbols.get(id).copied().and_then(package),
        };
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
    /// callee's at them. A result lifetime no input carries is chosen by
    /// the caller: by the expected type, or else by the result's first use.
    fn call_of(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        (callee, required_contract): (CallTarget, Option<semantic_vocabulary::ContractId>),
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
        expected: Option<crate::body::ValueType>,
    ) -> Checked<Expression> {
        self.call_of_values(
            id,
            slots,
            (callee, required_contract),
            arguments,
            source,
            expected,
            &[],
        )
    }

    fn call_of_values(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        (callee, required_contract): (CallTarget, Option<semantic_vocabulary::ContractId>),
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
        expected: Option<ValueType>,
        const_values: &[Expression],
    ) -> Checked<Expression> {
        let signature = self
            .source_call_signature(id, &callee, required_contract)?
            .clone();
        self.call_with_signature_values(
            id,
            slots,
            (callee, required_contract),
            signature,
            arguments,
            source,
            expected,
            const_values,
        )
    }

    pub(super) fn source_call_signature(
        &self,
        id: MachineId,
        callee: &CallTarget,
        required_contract: Option<semantic_vocabulary::ContractId>,
    ) -> Checked<&Signature> {
        if let Some(required) = required_contract {
            let Some(Some((caller, body, ..))) = self.declaration_selections.get(required) else {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("selected call has no exact public declaration association"),
                ));
            };
            if *caller != id || !matches!(callee, CallTarget::Machine(machine) if machine == body) {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("selected call changes its public contract body or caller"),
                ));
            }
            return terminal_check_tables::row(
                &self.declaration_signatures,
                required,
                id,
                Construct::Machine,
            );
        }
        match callee {
            CallTarget::Machine(machine) => {
                terminal_check_tables::row(&self.signatures, *machine, id, Construct::Machine)
            }
            CallTarget::Boundary { boundary } => {
                self.boundaries.signature(*boundary).ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("boundary has no exact retained signature"),
                    )
                })
            }
            CallTarget::Selected { contract, .. } => self
                .callables
                .contracts
                .get(*contract)
                .map(|row| &row.signature)
                .ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("selected call has no exact retained contract signature"),
                    )
                }),
        }
    }

    fn runtime_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        runtime: typed_trees::callable_values::RuntimeCallBinding,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
        expected: Option<ValueType>,
    ) -> Checked<Expression> {
        let scalar_type = self.callable_type(id, runtime.contract)?;
        let semantic_vocabulary::ScalarType::Callable(contract) = scalar_type else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("runtime call has no callable contract"),
            ));
        };
        let callable = self.expression(
            id,
            slots,
            runtime.callee,
            Some(ValueType::plain(scalar_type)),
        )?;
        if callable.result != Some(ValueType::plain(scalar_type)) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("runtime callee contract differs from the call"),
            ));
        }
        let signature = self
            .callables
            .contracts
            .get(contract)
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("runtime callable contract is missing"),
                )
            })?
            .signature
            .clone();
        self.call_with_signature(
            id,
            slots,
            (
                CallTarget::Selected {
                    callable: Box::new(callable),
                    contract,
                },
                None,
            ),
            signature,
            arguments,
            source,
            expected,
        )
    }

    pub(super) fn call_with_signature(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        (callee, required_contract): (CallTarget, Option<semantic_vocabulary::ContractId>),
        signature: Signature,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
        expected: Option<ValueType>,
    ) -> Checked<Expression> {
        self.call_with_signature_values(
            id,
            slots,
            (callee, required_contract),
            signature,
            arguments,
            source,
            expected,
            &[],
        )
    }

    fn call_with_signature_values(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        (callee, required_contract): (CallTarget, Option<semantic_vocabulary::ContractId>),
        signature: Signature,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
        expected: Option<ValueType>,
        const_values: &[Expression],
    ) -> Checked<Expression> {
        let free = if let Some(required) = required_contract {
            let signature = self
                .declaration_sources
                .get(required)
                .copied()
                .ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Declaration("selected call lost its public requirement"),
                    )
                })?;
            self.machine_lifetimes(signature.symbol)
        } else {
            match &callee {
                CallTarget::Machine(machine) => {
                    let symbol = self
                        .symbols
                        .get(*machine)
                        .copied()
                        .unwrap_or_else(SymbolHandle::invalid);
                    self.machine_lifetimes(symbol)
                }
                CallTarget::Boundary { boundary } => {
                    let declaration = self
                        .boundaries
                        .row(*boundary)
                        .ok_or_else(|| {
                            refuse(
                                id,
                                Construct::Machine,
                                Unserved::Call("boundary lifetime owner is absent"),
                            )
                        })?
                        .source;
                    self.machine_lifetimes(declaration)
                }
                CallTarget::Selected { contract, .. } => self
                    .callables
                    .contracts
                    .get(*contract)
                    .ok_or_else(|| {
                        refuse(
                            id,
                            Construct::Machine,
                            Unserved::Call("callable lifetime contract is absent"),
                        )
                    })?
                    .lifetimes
                    .clone(),
            }
        };
        let mut bound = super::regions::Substitution::new();
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
        if formals.len() != arguments.len() + const_values.len() {
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
        if let Some(span) = source.span()
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
                let (mut lent, lender) = self.lent(id, slots, *argument, borrowed, *formal)?;
                lent.result = self.borrowed_argument_type(id, slots, &lent, *formal, borrowed)?;
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
                && let Some(borrowed) = borrowed
                && let Some(Expression {
                    kind: ExpressionKind::Borrow { place, .. },
                    ..
                }) = read.last()
            {
                let actual = self.loan_regions(id, slots, place)?;
                self.unify_regions(id, &free, borrowed.regions, actual, &mut bound)?;
            }
            if !free.is_empty()
                && let Some(actual) = read.last().and_then(|argument| argument.result)
            {
                self.unify(id, &free, *declared, actual, &mut bound)?;
            }
        }
        read.extend_from_slice(const_values);
        lent_storage_is_untouched(
            id,
            &read,
            &self.captures,
            |place| place.result.is_some_and(|held| self.is_borrowed_view(held)),
            |slot| {
                slots
                    .get(slot)
                    .is_some_and(|held| held.borrowed == Some(ReferenceAccess::Shared))
            },
            |record, field| match record {
                ValueType::Record(row, _, _) => match &self.data_type(id, row).ok()?.shape {
                    crate::StorageShape::Record(fields) => fields.get(field).map(|field| field.id),
                    _ => None,
                },
                _ => None,
            },
        )?;
        if let Some(span) = source.span()
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
        for ((declared, _, borrowed), actual) in formals.iter().zip(&read) {
            let formal = self.substitute(*declared, &bound);
            let agrees = if borrowed.is_some() {
                self.same_type(actual.result, formal)
            } else {
                self.agrees(actual.result, Some(formal))
            };
            let agrees = agrees
                || self.const_array_argument_agrees(
                    id,
                    &callee,
                    actual.result,
                    formal,
                    const_values,
                    slots,
                );
            if !agrees {
                if borrowed.as_ref().is_some_and(|borrowed| {
                    borrowed.referent == crate::body::BorrowedReferent::Slice
                }) && let ExpressionKind::Borrow { access, place, .. } = &actual.kind
                    && self.byte_array_element(place.result).is_some()
                    && self.is_borrowed_view(formal)
                {
                    let borrow = if *access == ReferenceAccess::Mutable {
                        "&mut array[..]"
                    } else {
                        "&array[..]"
                    };
                    return Err(crate::reject(
                        id,
                        place.source,
                        format!(
                            "a reference to a fixed array is not a byte view; use `{borrow}` to borrow an explicit view of the array"
                        ),
                    ));
                }
                // The typed trees give a borrow no span of its own; the
                // place it lends is the authored argument.
                let at = match (&actual.kind, actual.source) {
                    (ExpressionKind::Borrow { place, .. }, SourceOrigin::Unpositioned(_)) => {
                        place.source
                    }
                    _ => actual.source,
                };
                return Err(self.mistyped(
                    id,
                    at,
                    super::rejections::Destination::Parameter,
                    Some(formal),
                    actual.result,
                    "argument types",
                ));
            }
        }
        // A lifetime the result names and no formal carries is the caller's
        // to choose: lifetimes.md, "Caller-chosen result lifetimes": "A
        // lifetime that a machine binds and no input carries is chosen
        // independently by each caller: the call instantiates it from the
        // result's use". Where the caller expected a type, that type chose
        // it above. Otherwise it is a region the first place the result
        // meets decides, as the region of a local written without a
        // lifetime is. Choosing it promises nothing about the callee: its
        // body is judged against that lifetime as a parameter it knows
        // nothing about, so only a view valid at every choice is returned.
        for parameter in &free {
            let chosen_by_caller = !formals.iter().any(|(formal, _, borrowed)| {
                self.names_any(*formal, &[*parameter])
                    || borrowed.as_ref().is_some_and(|borrowed| {
                        self.regions_name_any(borrowed.regions, &[*parameter])
                    })
            }) && declared_result
                .is_some_and(|result| self.names_any(result, &[*parameter]))
                && !bound.iter().any(|(known, _)| known == parameter);
            if chosen_by_caller {
                let chosen = self.fresh_region();
                bound.push((*parameter, chosen));
            }
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
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Call {
                callee,
                required_contract,
                static_reach_binding: None,
                arguments: read,
                designated: None,
            },
            result,
            source,
        })
    }

    fn const_array_argument_agrees(
        &self,
        caller: MachineId,
        callee: &CallTarget,
        actual: Option<ValueType>,
        expected: ValueType,
        values: &[Expression],
        slots: &[Slot],
    ) -> bool {
        let CallTarget::Machine(callee) = callee else {
            return false;
        };
        let callee = *callee;
        let (Some(ValueType::Record(actual, aq, ar)), ValueType::Record(expected, eq, er)) =
            (actual, expected)
        else {
            return false;
        };
        if aq != eq || ar != er || values.is_empty() {
            return false;
        }
        fn matches(
            program: &Program<'_>,
            caller: MachineId,
            callee: MachineId,
            actual: semantic_vocabulary::StructuralTypeId,
            expected: semantic_vocabulary::StructuralTypeId,
            values: &[Expression],
            slots: &[Slot],
            depth: usize,
        ) -> bool {
            if actual == expected {
                return true;
            }
            if depth > 128 {
                return false;
            }
            let (Some(actual), Some(expected)) =
                (program.records.get(actual), program.records.get(expected))
            else {
                return false;
            };
            let (
                crate::StorageShape::FixedArray {
                    element: ae,
                    qualification: aq,
                    length: al,
                },
                crate::StorageShape::FixedArray {
                    element: ee,
                    qualification: eq,
                    length: el,
                },
            ) = (&actual.shape, &expected.shape)
            else {
                return false;
            };
            if aq != eq
                || actual.multiplicity != expected.multiplicity
                || !actual.requires.is_empty()
                || !expected.requires.is_empty()
                || actual.cleanup.is_some()
                || expected.cleanup.is_some()
            {
                return false;
            }
            let length = if al == el {
                true
            } else if let terminal_psi::ArrayExtent::Parameter { owner, value } = el {
                if *owner != callee {
                    return false;
                }
                let Some(inputs) = program.const_scopes.get(callee) else {
                    return false;
                };
                let Some(position) = inputs.iter().position(|input| input.value == *value) else {
                    return false;
                };
                // const_scope is symbol-sorted; recover declaration order.
                let Some(symbol) = program
                    .symbols
                    .get(callee)
                    .and_then(|symbol| program.typed.machine_by_symbol(*symbol))
                else {
                    return false;
                };
                let parameters = program.typed.machine_type_parameters(symbol);
                let Some(position) = parameters
                    .iter()
                    .filter(|parameter| program.record_parameter(parameter.symbol).is_none())
                    .position(|parameter| parameter.symbol == inputs[position].symbol)
                else {
                    return false;
                };
                match (al, values.get(position).map(|value| &value.kind)) {
                    (
                        terminal_psi::ArrayExtent::Parameter { owner, value },
                        Some(ExpressionKind::Slot(slot)),
                    ) if *owner == caller => slots.get(*slot).is_some_and(|slot| {
                        program.const_scope.iter().any(|input| {
                            input.owner == caller
                                && input.value == *value
                                && input.symbol == slot.symbol
                        })
                    }),
                    (
                        terminal_psi::ArrayExtent::Literal(length),
                        Some(ExpressionKind::Integer(semantic_vocabulary::IntegerValue::Unsigned(
                            value,
                        ))),
                    ) => u128::from(*length) == *value,
                    _ => false,
                }
            } else {
                false
            };
            length && matches(program, caller, callee, *ae, *ee, values, slots, depth + 1)
        }
        matches(self, caller, callee, actual, expected, values, slots, 0)
    }

    /// Whether a value of this type lends observation only: a shared
    /// reference, or a shared view of bytes or elements.
    pub(super) fn observes_only(
        &self,
        id: MachineId,
        value_type: Option<ValueType>,
    ) -> Checked<bool> {
        use crate::StorageShape;
        use terminal_psi::{ByteSequenceCarrier, StructuralAccess};
        let Some(ValueType::Record(row, _, _)) = value_type else {
            return Ok(false);
        };
        Ok(matches!(
            self.data_type(id, row)?.shape,
            StorageShape::Reference {
                access: StructuralAccess::SharedBorrow,
                ..
            } | StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            }) | StorageShape::ElementView {
                access: Some(StructuralAccess::SharedBorrow),
                ..
            }
        ))
    }

    /// The argument for a borrowed formal: `&place` or `&mut place`, or a
    /// borrowed formal of this machine named whole, which lends its
    /// referent on (a reborrow). The place is a slot or a field path
    /// beneath one. ownership.md, "Owned local mutability": "modifying an
    /// established ordinary local's owned storage requires a `mut` binding.
    /// This covers ... formation of `&mut` or `&write` loans to those
    /// places. A callee cannot grant writable access that the caller's place
    /// lacks."
    pub(super) fn lent(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        argument: ExpressionHandle,
        formal: &crate::body::Borrowed,
        expected: ValueType,
    ) -> Checked<(Expression, String)> {
        self.lent_from(id, slots, argument, None, formal, expected)
    }

    /// `lent`, where an explicit borrow `argument` lends `beneath` in place
    /// of its own target: `&(text as +D)` lends `text`, and the caller
    /// qualifies the reference it forms (`borrow_value`).
    pub(super) fn lent_from(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        argument: ExpressionHandle,
        beneath: Option<ExpressionHandle>,
        formal: &crate::body::Borrowed,
        expected: ValueType,
    ) -> Checked<(Expression, String)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        let source = SourceOrigin::of(self.typed.expression_table.source_span(argument));
        if formal.referent == crate::body::BorrowedReferent::Slice
            && let Some(view) = self.borrowed_array_range(
                id,
                slots,
                argument,
                expected,
                source,
                Some(formal.access),
            )?
        {
            return Ok((
                Expression {
                    call_context: None,
                    result: view.result,
                    kind: ExpressionKind::Borrow {
                        access: formal.access,
                        place: Box::new(view),
                        home: None,
                    },
                    source,
                },
                self.typed.expression_table.display_name(argument),
            ));
        }
        let node = self.node(id, argument)?;
        if matches!(node, ExpressionNode::String(_)) && formal.access != ReferenceAccess::Shared {
            return unserved("byte literal cannot lend mutable storage");
        }
        // Evaluate the supplied value once. A reference descriptor carries
        // the permission to cross into its backing, whether it came from a
        // binding, a field or another helper call. An ordinary owned value
        // still needs an explicit borrow of its place.
        let (operand, access, explicit) = match node {
            ExpressionNode::Borrow(borrow) => {
                (beneath.unwrap_or(borrow.target), borrow.access, true)
            }
            _ => (argument, formal.access, false),
        };
        // A recast presents the same captured place under another type. The
        // parser keeps it inside the loan; it never produces an owned value.
        let (mut place, presentation) = match self.node(id, operand)? {
            // `view as &[T]` given to a slice formal is the re-view of an
            // existing view; it passes on like any other view value.
            ExpressionNode::Cast(cast)
                if cast.form.is_recast()
                    && formal.referent == crate::body::BorrowedReferent::Slice
                    && !explicit =>
            {
                (self.expression(id, slots, operand, Some(expected))?, None)
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                if access != ReferenceAccess::Shared || cast.form.recast_access() != Some(access) {
                    return unserved(
                        "exclusive storage view without establishment and handback evidence",
                    );
                }
                if cast.semantic_domain_symbol.is_valid()
                    || !cast.semantic_domain.is_empty()
                    || cast.domain != numerics::arithmetic::ArithmeticDomain::Exact
                {
                    return unserved("storage view that also changes a domain");
                }
                let presentation = self.value_type(id, cast.target_type)?;
                if presentation.qualification() != Qualification::NONE
                    || !self.same_type(Some(presentation), expected)
                {
                    return unserved("storage view target differs from its borrowed destination");
                }
                if cast.result_type.is_valid()
                    && self.value_type(id, cast.result_type)? != presentation
                {
                    return unserved("storage view has another retained result type");
                }
                (
                    self.expression(id, slots, cast.value, None)?,
                    Some(presentation),
                )
            }
            _ => (
                self.place(
                    id,
                    slots,
                    operand,
                    (!explicit
                        && formal.access == ReferenceAccess::Shared
                        && self.is_borrowed_view(expected))
                    .then_some(expected),
                    if access == ReferenceAccess::Shared {
                        Access::Borrow
                    } else {
                        Access::Write
                    },
                )?,
                None,
            ),
        };
        // A write-only loan cannot supply observation to its callee, even
        // when the caller's own source place holds stronger access.
        if explicit
            && access == ReferenceAccess::WriteOnly
            && formal.access != ReferenceAccess::WriteOnly
        {
            return Err(crate::reject(
                id,
                source,
                "the caller lends write-only access to this argument and its parameter requires \
                 readable access: a callee cannot read through a write-only loan",
            ));
        }
        // ownership.md, "Borrows and aliases": `&T` grants "Shared
        // observation under its loan contract", and "A callee cannot grant
        // writable access that the caller's place lacks." So an argument
        // that lends observation only is wrong for a formal that writes, in
        // every program: an explicit `&place`, or a value that is itself a
        // shared reference or a shared view.
        if formal.access != ReferenceAccess::Shared
            && if explicit {
                access == ReferenceAccess::Shared
            } else {
                self.observes_only(id, place.result)?
            }
        {
            return Err(crate::reject(
                id,
                source,
                "the caller lends only immutable access to this argument and its parameter \
                 writes through it: a callee cannot grant writable access that the caller's \
                 place lacks",
            ));
        }
        // loans.md, "Reborrow lineage and access": a Mutable parent may
        // form a Shared child, "Freeze mutation until the complete shared
        // cohort ends; restore once." So `&mut place` given to a `&T`
        // parameter is the exclusive loan the program wrote, lent on for
        // observation: the place must admit that exclusive loan (every
        // check below is made at the written access), and the callee
        // receives a shared child of that formation. Both loans remain in
        // the lowered form, so the checker judges the actual formation and
        // its complete descendant lifetime against existing cohorts.
        let attenuated = explicit
            && access == ReferenceAccess::Mutable
            && formal.access == ReferenceAccess::Shared;
        if !access.forms(formal.access) {
            return unserved("borrow whose access differs from its formal's");
        }
        // An existing view already carries its backing; an owned array still
        // needs an explicit loan. Test the actual value, not the expected view.
        if formal.referent == crate::body::BorrowedReferent::Slice
            && !explicit
            && place
                .result
                .is_some_and(|actual| self.is_borrowed_view(actual))
        {
            return Ok((
                Expression {
                    call_context: None,
                    result: place.result,
                    kind: ExpressionKind::Borrow {
                        access: formal.access,
                        place: Box::new(place),
                        home: None,
                    },
                    source,
                },
                self.typed.expression_table.display_name(argument),
            ));
        }
        if let Some(value_type) = place.result
            && let Some((referent, _)) = self.reference_referent(id, value_type)?
        {
            place = Expression {
                call_context: None,
                source: place.source,
                kind: ExpressionKind::Referent(Box::new(place)),
                result: Some(referent),
            };
        }
        if presentation.is_some() {
            self.representation_read_authorized(id, &place)?;
        }
        let crossed = self.reference_place_access(id, slots, &place, access)?;
        let slot = root_slot(id, &place, &self.captures)?;
        if !crossed {
            let Some(slot) = slot else {
                return unserved("borrow of a value that is not a place");
            };
            let lender = &slots[slot];
            if !explicit && lender.borrowed.is_none() {
                return unserved("argument for a borrowed formal that is not a borrow");
            }
            if !names_storage(&place) {
                return unserved("borrow of a value that is not a place");
            }
            if access == ReferenceAccess::Mutable && lender.borrowed.is_none() && !lender.assignable
            {
                return unserved("exclusive loan of an immutable binding");
            }
        }
        let lender_text = self.typed.expression_table.display_name(argument);
        // An exclusive loan of a field hands its holder a write inside
        // every value the path goes through.
        if access != ReferenceAccess::Shared {
            self.written_through(id, &place)?;
        }
        // A scalar local that is an SSA value here gets a home for the loan;
        // its type identity comes from the actual lender. A representation
        // view must not give the backing the destination's different type.
        // A reference formal's referent is never policy-qualified
        // (`formal`), and the place a loan lends has the referent's type.
        if place.result.and_then(ValueType::policy).is_some() {
            return unserved("loan of a policy-qualified integer");
        }
        let mut home = match (&place.kind, slot.and_then(|slot| slots.get(slot))) {
            (ExpressionKind::Slot(_), Some(lender))
                if matches!(lender.value_type, ValueType::Scalar(_, Qualification::NONE))
                    && !lender.homed =>
            {
                let scalar = lender.value_type.scalar().ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Typing("scalar lender without its carrier"),
                    )
                })?;
                let atom = super::types::scalar_atom(scalar, id)?;
                let symbol = self
                    .typed
                    .symbols
                    .builtin_atom_symbol(atom)
                    .ok_or_else(|| {
                        refuse(
                            id,
                            Construct::Machine,
                            Unserved::Type("scalar lender has no registered builtin"),
                        )
                    })?;
                let reference = self
                    .typed
                    .type_reference_table
                    .find_arithmetic_result_type_reference(
                        symbol,
                        numerics::arithmetic::ArithmeticDomain::Exact,
                    )
                    .ok_or_else(|| {
                        refuse(
                            id,
                            Construct::Machine,
                            Unserved::Type("scalar lender has no registered type occurrence"),
                        )
                    })?;
                let storage = self.storage_type(id, reference, lender.value_type)?;
                Some(self.data_type(id, storage)?.identity.clone())
            }
            _ => None,
        };
        if attenuated {
            let reference = self
                .typed
                .expression_table
                .constructed_value_type_reference(argument);
            if !self
                .typed
                .type_reference_table
                .contains_type_reference(reference)
            {
                return unserved("exclusive formation without its retained reference type");
            }
            let formed = self.local_type(id, reference)?;
            let Some((referent, StructuralAccess::MutableBorrow)) =
                self.reference_referent(id, formed)?
            else {
                return unserved("exclusive formation without its actual reference type");
            };
            if !self.same_type(place.result, referent) {
                return unserved("exclusive formation changes its captured referent type");
            }
            let result = place.result;
            let value = Expression {
                call_context: None,
                result,
                kind: ExpressionKind::Borrow {
                    access,
                    place: Box::new(place),
                    home: home.take(),
                },
                source,
            };
            let parent = self.finish_reference_value(id, slots, value, formed, source)?;
            place = Expression {
                call_context: None,
                result,
                kind: ExpressionKind::Referent(Box::new(parent)),
                source,
            };
        }
        Ok((
            Expression {
                call_context: None,
                result: presentation.or(place.result),
                kind: ExpressionKind::Borrow {
                    access: formal.access,
                    place: Box::new(place),
                    home,
                },
                source,
            },
            lender_text,
        ))
    }

    /// A slice lend presents the backing with the formal's access. The
    /// captured place and its producer retain the actual descriptor type and
    /// loan. Other geometry still requires exact correspondence.
    pub(super) fn borrowed_argument_type(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        lent: &Expression,
        expected: ValueType,
        formal: &crate::body::Borrowed,
    ) -> Checked<Option<ValueType>> {
        use crate::StorageShape;
        use terminal_psi::{ByteSequenceCarrier, StructuralAccess};

        let actual = lent.result;
        let (
            Some(ValueType::Record(actual_row, qualification, regions)),
            ValueType::Record(expected_row, _, _),
        ) = (actual, expected)
        else {
            return Ok(actual);
        };
        let actual_access = match self.data_type(machine, actual_row)?.shape {
            StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView { access }) => access,
            _ => return Ok(actual),
        };
        let expected_access = match self.data_type(machine, expected_row)?.shape {
            StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView { access }) => access,
            _ => return Ok(actual),
        };
        let lends_mutable_backing = formal.referent == crate::body::BorrowedReferent::Slice
            && formal.access == ReferenceAccess::Mutable
            && expected_access.is_none()
            && actual_access == Some(StructuralAccess::MutableBorrow);
        let lends_for_observation = formal.access == ReferenceAccess::Shared
            && matches!(actual_access, None | Some(StructuralAccess::MutableBorrow))
            && expected_access == Some(StructuralAccess::SharedBorrow);
        // A shared loan of the direct formal may observe its descriptor.
        // Only that formal supplies the inner mutable access it describes.
        let observes_mutable_formal = formal.access == ReferenceAccess::Shared
            && actual_access.is_none()
            && expected_access == Some(StructuralAccess::MutableBorrow)
            && matches!(
                &lent.kind,
                ExpressionKind::Borrow { place, .. }
                    if matches!(place.kind, ExpressionKind::Slot(slot)
                        if slots.get(slot).and_then(|slot| slot.borrowed) == Some(ReferenceAccess::Mutable))
            );
        if !lends_mutable_backing && !lends_for_observation && !observes_mutable_formal {
            return Ok(actual);
        }
        let presented = ValueType::Record(expected_row, qualification, regions);
        Ok(if self.same_type(Some(presented), expected) {
            Some(presented)
        } else {
            actual
        })
    }
}

/// Arguments run left to right, but this lowering forms loans together at
/// the call. An argument read or newly evaluated selector that touches
/// storage this call lends is refused, including reads before the loan,
/// until loan timing is represented.
///
/// Storage is judged by field path, not by root (AGENTS.md rule 3), with
/// the checker's own relation (`place_paths::may_overlap`, which
/// `places::overlap` uses): two different fields of one record are disjoint, and every
/// other pair of paths below one root may share storage. So `fs.a` does not
/// touch a loan of `fs.b`, but does touch a loan of `fs` or of `fs.a.c`.
/// An index selects a runtime element, so a path stops at its first index.
///
/// A shared loan only forbids changing the lent storage (ownership.md,
/// "Borrows and aliases": "Shared loans may duplicate or reborrow, while
/// exclusive loans must not overlap incompatibly"). A read beside it is
/// harmless. A mutable loan conflicts with any use of overlapping storage,
/// and so does a whole non-scalar slot that is not a shared reference,
/// because using it moves or reborrows its value.
///
/// A place that holds a byte view lends none of its binding's storage: the
/// callee is given the view, whose bytes are elsewhere. lifetimes.md: "The
/// result carries loans into the original backing, not a borrow of the
/// caller's private carrier storage." `holds_view` says which places do;
/// `shared_reference` says which slots hold shared references, and
/// `field_identity` names the field a record type has at a position.
fn lent_storage_is_untouched<'a>(
    id: MachineId,
    arguments: &'a [Expression],
    captures: &'a [(
        arena::Handle<typed_trees::expression::CompletedValueCapture>,
        Expression,
    )],
    holds_view: impl Fn(&Expression) -> bool,
    shared_reference: impl Fn(usize) -> bool,
    field_identity: impl Fn(ValueType, usize) -> Option<StructuralFieldId>,
) -> Checked<()> {
    let mut captures: Vec<_> = captures
        .iter()
        .map(|(capture, subject)| (*capture, subject))
        .collect();
    let mut lent = Vec::new();
    for argument in arguments {
        if let ExpressionKind::Borrow { place, access, .. } = &argument.kind
            && !holds_view(place)
            && let Some((slot, path)) = place_path(id, place, &captures, &field_identity)?
        {
            lent.push(Lent {
                slot,
                path,
                mutable: *access != ReferenceAccess::Shared,
            });
        }
    }
    if lent.is_empty() {
        return Ok(());
    }
    let touches = Touches {
        id,
        lent: &lent,
        shared_reference: &shared_reference,
        field_identity: &field_identity,
    };
    for argument in arguments {
        if let ExpressionKind::Borrow { place, .. } = &argument.kind {
            if touches.selectors(place, &mut captures)? {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Call("loan selector that names storage another argument lends"),
                ));
            }
        } else if touches.expression(argument, false, &mut captures)? {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Call("argument that names storage another argument lends"),
            ));
        }
    }
    Ok(())
}

type SavedSubjects<'a> = Vec<(
    arena::Handle<typed_trees::expression::CompletedValueCapture>,
    &'a Expression,
)>;

/// A lent place: its root slot and the storage path below it. Crossing a
/// reference adds no segment: whether a path spells `Referent` depends on how
/// it was formed, and leaving it out only merges paths, never separates them.
/// A path stops at its first index or case payload, so it covers the whole
/// collection or sum: case payloads overlay one another.
struct Lent {
    slot: usize,
    path: Vec<Segment>,
    mutable: bool,
}

/// The root slot and static path a place names, without evaluating any
/// selector. `None` when the place is not rooted in a slot.
fn place_path<'a>(
    id: MachineId,
    place: &'a Expression,
    captures: &[(
        arena::Handle<typed_trees::expression::CompletedValueCapture>,
        &'a Expression,
    )],
    field_identity: &dyn Fn(ValueType, usize) -> Option<StructuralFieldId>,
) -> Checked<Option<(usize, Vec<Segment>)>> {
    let mut resolved = super::storage_path::Captures::new(id, captures);
    let mut steps = Vec::new();
    let root = super::storage_path::walk(
        place,
        |capture| resolved.resolve(capture),
        |projection, _| {
            match &projection.kind {
                ExpressionKind::Field { base, field } => steps.push(
                    base.result
                        .and_then(|record| field_identity(record, *field))
                        .map(Segment::Field),
                ),
                ExpressionKind::Payload { .. } | ExpressionKind::Index { .. } => steps.push(None),
                _ => {}
            }
            Ok(())
        },
    )?;
    let ExpressionKind::Slot(slot) = root.kind else {
        return Ok(None);
    };
    steps.reverse();
    let path = steps.into_iter().map_while(|step| step).collect();
    Ok(Some((slot, path)))
}

struct Touches<'l> {
    id: MachineId,
    lent: &'l [Lent],
    shared_reference: &'l dyn Fn(usize) -> bool,
    field_identity: &'l dyn Fn(ValueType, usize) -> Option<StructuralFieldId>,
}

impl Touches<'_> {
    /// Whether using the storage at `slot` and `path` conflicts with a loan,
    /// by the one storage-path relation the checker also uses.
    /// `changes` marks a use that may change, move or mutably reborrow it.
    fn conflicts(&self, slot: usize, path: &[Segment], changes: bool) -> bool {
        self.lent.iter().any(|lent| {
            lent.slot == slot
                && (lent.mutable || changes)
                && terminal_semantics::storage::place_paths::may_overlap(&lent.path, path)
        })
    }

    /// A saved subject's selectors are captured, so they do not evaluate
    /// again. New index operands still run while earlier argument loans
    /// are live.
    fn selectors<'a>(
        &self,
        place: &'a Expression,
        captures: &mut SavedSubjects<'a>,
    ) -> Checked<bool> {
        match &place.kind {
            ExpressionKind::Index { base, index } => {
                Ok(self.selectors(base, captures)? || self.expression(index, false, captures)?)
            }
            ExpressionKind::Field { base, .. }
            | ExpressionKind::Payload { base, .. }
            | ExpressionKind::Referent(base) => self.selectors(base, captures),
            ExpressionKind::SavedSubject(_) => {
                root_slot(self.id, place, captures)?;
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    /// A place touches the storage its path names; its new selectors and
    /// any root that is not a slot are evaluated expressions. A saved
    /// subject still uses its captured backing, but not its old selector.
    fn place<'a>(
        &self,
        place: &'a Expression,
        changes: bool,
        captures: &mut SavedSubjects<'a>,
    ) -> Checked<bool> {
        let mut root = place;
        loop {
            match &root.kind {
                ExpressionKind::Index { base, index } => {
                    if self.expression(index, false, captures)? {
                        return Ok(true);
                    }
                    root = base;
                }
                ExpressionKind::Field { base, .. }
                | ExpressionKind::Payload { base, .. }
                | ExpressionKind::Referent(base) => root = base,
                _ => break,
            }
        }
        if !matches!(
            root.kind,
            ExpressionKind::Slot(_) | ExpressionKind::SavedSubject(_)
        ) {
            return self.expression(root, changes, captures);
        }
        let Some((slot, path)) = place_path(self.id, place, captures, self.field_identity)? else {
            return Ok(false);
        };
        // Using a whole slot moves or reborrows its value unless it is a
        // scalar copy or a shared reference.
        let whole = matches!(place.kind, ExpressionKind::Slot(_))
            && place.result.and_then(ValueType::scalar).is_none()
            && !(self.shared_reference)(slot);
        Ok(self.conflicts(slot, &path, changes || whole))
    }

    fn expression<'a>(
        &self,
        expression: &'a Expression,
        changes: bool,
        captures: &mut SavedSubjects<'a>,
    ) -> Checked<bool> {
        match &expression.kind {
            ExpressionKind::Atomic(operation) => {
                for operand in operation.inputs() {
                    if self.expression(operand, false, captures)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            ExpressionKind::Slot(_)
            | ExpressionKind::SavedSubject(_)
            | ExpressionKind::Field { .. }
            | ExpressionKind::Payload { .. }
            | ExpressionKind::Index { .. }
            | ExpressionKind::Referent(_) => self.place(expression, changes, captures),
            ExpressionKind::Integer(_)
            | ExpressionKind::Float(_)
            | ExpressionKind::ScalarApplication { .. }
            | ExpressionKind::ByteLiteral(_)
            | ExpressionKind::MachineValue { .. }
            | ExpressionKind::Zero
            | ExpressionKind::Boolean(_) => Ok(false),
            ExpressionKind::Not(operand)
            | ExpressionKind::BitwiseNot(operand)
            | ExpressionKind::Cast(operand)
            | ExpressionKind::Requalify(operand)
            | ExpressionKind::Establish { operand, .. }
            | ExpressionKind::CollectionView { base: operand, .. }
            | ExpressionKind::CollectionMeasure { base: operand, .. }
            | ExpressionKind::IsCase {
                subject: operand, ..
            } => self.expression(operand, changes, captures),
            ExpressionKind::Borrow { place, access, .. } => self.expression(
                place,
                changes || *access != ReferenceAccess::Shared,
                captures,
            ),
            ExpressionKind::Array { elements } => {
                for element in elements {
                    if self.expression(element, changes, captures)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            ExpressionKind::CollectionSubslice { base, start, end } => {
                if self.expression(base, changes, captures)? {
                    return Ok(true);
                }
                for operand in start.iter().chain(end.iter()) {
                    if self.expression(operand, false, captures)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            ExpressionKind::ByteElement { base, index } => Ok(self
                .expression(base, false, captures)?
                || self.expression(index, false, captures)?),
            ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
                for (_, operand) in fields {
                    if self.expression(operand, changes, captures)? {
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
                if self.expression(subject, changes, captures)? {
                    return Ok(true);
                }
                if let Some(capture) = capture {
                    captures.push((*capture, subject));
                }
                let result = (|| {
                    for (selector, arm) in arms {
                        if let crate::body::Selector::Value(pattern) = selector
                            && self.expression(pattern, false, captures)?
                        {
                            return Ok(true);
                        }
                        if self.expression(arm, changes, captures)? {
                            return Ok(true);
                        }
                    }
                    match default {
                        Some(arm) => self.expression(arm, changes, captures),
                        None => Ok(false),
                    }
                })();
                if capture.is_some() {
                    captures.pop();
                }
                result
            }
            ExpressionKind::Strict { left, right, .. }
            | ExpressionKind::ShortCircuit { left, right, .. } => Ok(self
                .expression(left, false, captures)?
                || self.expression(right, false, captures)?),
            ExpressionKind::Call {
                callee, arguments, ..
            } => {
                if let Some(operand) = callee.operand()
                    && self.expression(operand, false, captures)?
                {
                    return Ok(true);
                }
                for argument in arguments {
                    if self.expression(argument, false, captures)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Unpositioned;
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};

    #[test]
    fn copied_call_bindings_keep_their_exact_authored_state() {
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
            ResolvedAuthoredCallBinding,
        };
        use symbols::{SymbolNameRef, SymbolTableBuilder};
        use typed_trees::{MachineSpecialization, machine::Machine, state::State};
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = builder.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("source")),
                (SymbolKind::Machine, SymbolNameRef::Static("foreign")),
            ],
        );
        let source = declarations.handle_at(0);
        let foreign = declarations.handle_at(1);
        let source_state = builder
            .insert_children(source, [(SymbolKind::State, SymbolNameRef::Static("run"))])
            .handle_at(0);
        let foreign_state = builder
            .insert_children(foreign, [(SymbolKind::State, SymbolNameRef::Static("run"))])
            .handle_at(0);
        let mut typed = typed_trees::TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let instance =
            typed
                .symbols
                .insert_generated_root_from(source, SymbolKind::Machine, "instance");
        let state = typed
            .symbols
            .insert_generated_children_from(instance, [(source_state, SymbolKind::State, "run")])
            .handle_at(0);
        for (symbol, state) in [
            (source, source_state),
            (foreign, foreign_state),
            (instance, state),
        ] {
            let states = typed.machine_states.insert_many([State {
                symbol: state,
                ..Default::default()
            }]);
            typed.push_machine(Machine {
                symbol,
                states,
                ..Default::default()
            });
        }
        let row = MachineSpecialization {
            template: source,
            instance,
            states: typed.captured_machine_states.insert_many([
                typed_trees::CapturedMachineState {
                    template: source_state,
                    instance: state,
                },
            ]),
            ..Default::default()
        };
        typed.machine_specializations.push(row.clone());
        let binding = ResolvedAuthoredCallBinding {
            selected: state,
            original: source_state,
        };
        let id = MachineId::new(1).unwrap();
        let mut selection = |symbol| {
            typed
                .record_resolved_authored_declaration_selection_once(
                    source::SourceSpan::default(),
                    AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    AuthoredDeclarationSelectionKind::Call,
                    symbol,
                )
                .unwrap()
        };
        let ledger_names_machine = selection(source);
        let ledger_names_unrelated_entry = selection(foreign_state);
        assert!(
            Program::new(&typed, false)
                .direct_selection(id, state, Some(ledger_names_machine), None)
                .is_ok()
        );
        assert!(
            Program::new(&typed, false)
                .direct_selection(id, state, Some(ledger_names_unrelated_entry), None)
                .is_err()
        );
        assert!(
            Program::new(&typed, false)
                .direct_selection(id, state, None, Some(binding))
                .is_ok()
        );
        assert!(
            Program::new(&typed, false)
                .direct_selection(
                    id,
                    instance,
                    None,
                    Some(ResolvedAuthoredCallBinding {
                        selected: instance,
                        original: source
                    })
                )
                .is_ok()
        );
        assert!(
            Program::new(&typed, false)
                .direct_selection(
                    id,
                    state,
                    None,
                    Some(ResolvedAuthoredCallBinding {
                        original: foreign_state,
                        ..binding
                    })
                )
                .is_err()
        );
        assert!(
            Program::new(&typed, false)
                .direct_selection(
                    id,
                    state,
                    None,
                    Some(ResolvedAuthoredCallBinding {
                        original: SymbolHandle::invalid(),
                        ..binding
                    })
                )
                .is_err()
        );
        typed
            .machine_specializations
            .edit(|rows| rows[0].template = foreign);
        assert!(
            Program::new(&typed, false)
                .direct_selection(id, state, None, Some(binding))
                .is_err()
        );
        typed
            .machine_specializations
            .edit(|rows| rows[0] = row.clone());
        typed
            .machine_specializations
            .edit(|rows| rows[0].states = Default::default());
        assert!(
            Program::new(&typed, false)
                .direct_selection(id, state, Some(ledger_names_machine), None)
                .is_err()
        );
        assert!(
            Program::new(&typed, false)
                .direct_selection(id, state, None, Some(binding))
                .is_err(),
            "a machine receipt alone does not establish its entry correspondence"
        );
        typed
            .machine_specializations
            .edit(|rows| rows[0] = row.clone());
        typed.machine_specializations.push(row);
        assert!(
            Program::new(&typed, false)
                .direct_selection(id, state, None, Some(binding))
                .is_err()
        );
        typed.machine_specializations.clear();
        assert!(
            Program::new(&typed, false)
                .direct_selection(id, state, None, Some(binding))
                .is_err(),
            "generated_from is insufficient without the actual application"
        );
    }

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
            call_context: None,
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
            call_context: None,
            kind: ExpressionKind::SavedSubject(capture),
            result: Some(record),
            source,
        };
        let borrow = |place: Expression| {
            let result = place.result;
            Expression {
                call_context: None,
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
            call_context: None,
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
                call_context: None,
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
        let overlapping = lent_storage_is_untouched(
            id,
            &[borrow(slot(0)), payload(capture)],
            &captures,
            |_| false,
            |_| false,
            |_, _| None,
        )
        .unwrap_err();
        assert!(
            overlapping
                .detail
                .contains("argument that names storage another argument lends")
        );
        assert!(
            lent_storage_is_untouched(
                id,
                &[borrow(slot(1)), payload(capture)],
                &captures,
                |_| false,
                |_| false,
                |_, _| None
            )
            .is_ok()
        );
        assert!(
            lent_storage_is_untouched(
                id,
                &[borrow(slot(2)), payload(capture)],
                &captures,
                |_| false,
                |_| false,
                |_, _| None
            )
            .is_ok()
        );
        let refused = lent_storage_is_untouched(
            id,
            &[borrow(slot(0)), payload(foreign)],
            &captures,
            |_| false,
            |_| false,
            |_, _| None,
        )
        .unwrap_err();
        assert!(refused.detail.contains("outside its match capture"));
        assert!(
            lent_storage_is_untouched(
                id,
                &[borrow(saved(capture)), payload(capture)],
                &captures,
                |_| false,
                |_| false,
                |_, _| None
            )
            .is_err()
        );
        assert!(
            lent_storage_is_untouched(
                id,
                &[borrow(slot(2)), payload(capture)],
                &[(capture, saved(capture))],
                |_| false,
                |_| false,
                |_, _| None
            )
            .is_err()
        );
    }

    /// AGENTS.md rule 3: a loan of one field leaves a sibling field usable,
    /// and a shared loan leaves its own storage readable.
    #[test]
    fn lent_storage_is_judged_by_field_path_and_access() {
        let id = MachineId::new(1).unwrap();
        let source = SourceOrigin::Unpositioned(Unpositioned::SpanAbsent);
        let record = ValueType::record(StructuralTypeId::new(1).unwrap());
        let scalar = ValueType::plain(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        ));
        let root = || Expression {
            call_context: None,
            kind: ExpressionKind::Slot(0),
            result: Some(record),
            source,
        };
        let field = |field, result| Expression {
            call_context: None,
            kind: ExpressionKind::Field {
                base: Box::new(Expression {
                    call_context: None,
                    kind: ExpressionKind::Referent(Box::new(root())),
                    result: Some(record),
                    source,
                }),
                field,
            },
            result: Some(result),
            source,
        };
        let borrow = |access, place: Expression| Expression {
            call_context: None,
            result: place.result,
            kind: ExpressionKind::Borrow {
                access,
                place: Box::new(place),
                home: None,
            },
            source,
        };
        let check = |arguments: &[Expression], shared: bool| {
            lent_storage_is_untouched(
                id,
                arguments,
                &[],
                |_| false,
                |_| shared,
                |_, field| StructuralFieldId::new(u64::try_from(field).ok()? + 1),
            )
        };
        let mutable = || borrow(ReferenceAccess::Mutable, field(1, record));
        let shared = || borrow(ReferenceAccess::Shared, field(1, record));
        // `f(fs.fd, &mut fs.buffer)`: distinct fields.
        assert!(check(&[field(0, scalar), mutable()], false).is_ok());
        // `f(fs.buffer.len, &mut fs.buffer)`: inside the mutable loan.
        let inner = Expression {
            call_context: None,
            kind: ExpressionKind::Field {
                base: Box::new(field(1, record)),
                field: 0,
            },
            result: Some(scalar),
            source,
        };
        let refused = check(&[inner.clone(), mutable()], false).unwrap_err();
        assert!(
            refused
                .detail
                .contains("argument that names storage another argument lends")
        );
        // `f(fs, &mut fs.buffer)`: the whole root contains the loan.
        assert!(check(&[root(), mutable()], true).is_err());
        // `f(fs.buffer.len, &fs.buffer)`: reading beside a shared loan.
        assert!(check(&[inner, shared()], false).is_ok());
        // `f(fs, &fs.buffer)`: a shared reference passed whole only reads;
        // an owned record passed whole moves the lent storage.
        assert!(check(&[root(), shared()], true).is_ok());
        assert!(check(&[root(), shared()], false).is_err());
    }
}
