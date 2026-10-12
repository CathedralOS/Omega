//! Qualified calls select exact declarations; service values select exact
//! requirements. Data values never select attached machines or supply arguments.

use super::{
    CallScope, boundary_trait_call_signature, call_gates, declared_receiver_type_reference,
    machine_state_by_symbol, validate_call_arguments,
    validate_resolved_target_type_parameter_bounds, validate_result_use,
};
use diagnostics::Diagnostic;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateSignature;
use typed_trees::statement::TableCall;

/// Rejoin the selected declaration rather than searching a value's type or
/// the last spelling in its path. Generated placed accessors retain their
/// independent plan authority; they are memory operations, not data methods.
pub(super) fn validate_receiver_call(
    scope: &CallScope<'_>,
    receiver_members: &[typed_trees::name::Identifier],
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A namespace supplies no runtime receiver. Rejoin its resolved target
    // without a place/name ladder that can see a different local or declaration
    // sharing the namespace's leaf spelling. Operator declarations
    // keep their separate signature validator; absence of an ordinary selected
    // target is a rejection, never permission to search by name again.
    if call_gates::receiver_is_declaration_namespace(scope, receiver_members) {
        if scope.program.symbols.get(scope.call.receiver_symbol).kind
            == symbols::SymbolKind::Conformance
            && !scope.call.target_symbol.is_valid()
        {
            diagnostics.push(Diagnostic::error(
                super::generic_requirement::unresolved_implementation_call(
                    scope.program,
                    scope.call.receiver_symbol,
                    scope.call.target.as_str(),
                ),
            ));
            return;
        }
        if !validate_resolved_receiver_call(scope, arguments, true, diagnostics)
            && !call_gates::receiver_is_operator_namespace(scope, receiver_members, arguments.len())
        {
            diagnostics.push(Diagnostic::error(format!(
                "qualified call `{}` has no resolved callable declaration",
                scope.call.target,
            )));
        }
        return;
    }
    let CallScope {
        program,
        call,
        current_machine,
        current_state,
        ..
    } = *scope;
    let receiver = receiver_members
        .last()
        .map(|member| member.as_str())
        .unwrap_or_default();
    let receiver_type_reference = current_state.and_then(|state| {
        declared_receiver_type_reference(program, current_machine, state, receiver)
    });

    if crate::declarations::traits::report_dynamic_receiver_call_error(
        program,
        receiver_type_reference,
        call.target.as_str(),
        diagnostics,
    ) {
        return;
    }

    if !validate_resolved_receiver_call(scope, arguments, false, diagnostics) {
        diagnostics.push(Diagnostic::error(
            "value-qualified machine calls are retired; use a declaration namespace and explicit arguments",
        ));
    }
}

/// Result-use and argument validation every resolved receiver signature
/// shares; the boundary rung adds native-callback destination selection on
/// top of it.
#[allow(clippy::too_many_arguments)]
fn validate_signature_result_and_arguments(
    program: &dyn crate::CheckingInput,
    call: &TableCall,
    signature: &StateSignature,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    value_environment: &crate::proof_contracts::arithmetic_domains::ValueEnvironment,
    arguments: &[ExpressionHandle],
    writable_roots: &crate::value_custody::locals::WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_result_use(
        program,
        call,
        &signature.name,
        signature.return_type,
        diagnostics,
    );
    validate_call_arguments(
        program,
        current_machine,
        current_state,
        value_environment,
        arguments,
        &signature.name,
        &signature.name,
        program.state_signature_parameters(signature),
        writable_roots.statements,
        false,
        &[],
        |argument, required| {
            crate::value_custody::expression_types::argument_matches_carrier(
                program,
                argument,
                required,
                &[],
            )
        },
        diagnostics,
    );
}

/// The exact target owns the call's signature. Namespace and runtime-place
/// calls share explicit argument checks. The only value-qualified machine
/// target is an accessor selected by its exact compiler-owned placement plan.
fn validate_resolved_receiver_call(
    scope: &CallScope<'_>,
    arguments: &[ExpressionHandle],
    namespace: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let CallScope {
        program,
        call,
        current_machine,
        current_state,
        symbols,
        writable_roots,
        value_environment,
        ..
    } = *scope;
    // The retained target owns the signature. Ordinary machines require a
    // namespace; a service value rejoins its exact requirement through the
    // write-frame owner's receiver/symbol join, including machine-generic
    // callback registrars. Missing identity is not permission to reselect.
    if let Some((callee_machine, state)) = machine_state_by_symbol(program, call.target_symbol) {
        if !namespace
            && !current_state.is_some_and(|state| {
                crate::value_custody::placed_views::statement_call_target(
                    program,
                    current_machine,
                    state,
                    call,
                ) == Some(call.target_symbol)
            })
        {
            diagnostics.push(Diagnostic::error(
                "value-qualified machine calls are retired; use a declaration namespace and explicit arguments",
            ));
            return true;
        }
        // A named implementation's row may be any machine, attached to any
        // data; the implementation, not the data, qualifies the call.
        let receiver_is_implementation =
            program.symbols.get(call.receiver_symbol).kind == symbols::SymbolKind::Conformance;
        if namespace
            && !receiver_is_implementation
            && callee_machine.attached_data_symbol.is_valid()
            && call.receiver_symbol != callee_machine.attached_data_symbol
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected attached call `{}` does not belong to the receiver's exact data type",
                call.target,
            )));
            return true;
        }
        validate_result_use(
            program,
            call,
            state.name.as_str(),
            state.return_type,
            diagnostics,
        );
        super::type_arguments::validate_resolved_call_arguments(
            program,
            current_machine,
            current_state,
            value_environment,
            arguments,
            state.name.as_str(),
            callee_machine,
            state,
            &call.machine_arguments,
            writable_roots,
            diagnostics,
        );
        validate_resolved_target_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            state,
            state.name.as_str(),
            arguments,
            current_machine,
            current_state,
            diagnostics,
        );
        return true;
    }
    if let Some(selected) = boundary_trait_call_signature(program, current_machine, call) {
        // Runtime formals only: native callback destinations live outside
        // `parameters` and are joined to their selections below.
        validate_signature_result_and_arguments(
            program,
            call,
            selected.contract,
            current_machine,
            current_state,
            value_environment,
            arguments,
            writable_roots,
            diagnostics,
        );
        validate_native_callback_selections(
            program,
            call.target.as_str(),
            &call.machine_arguments,
            call.authored_machine_application,
            selected,
            diagnostics,
        );
        return true;
    }
    false
}

/// A `native callback name from Binder` destination has no source runtime
/// argument (private_callbacks.md, "Authored selection"): the registrar call
/// omits it, and its callee is the call's static machine argument for that
/// binder. Each destination must therefore name one `machine` binder of the
/// original declaration. A prepared call retains its selection by that exact
/// binder handle in the original application; an unapplied call supplies its
/// authored machine ordinal. Consumed contract axes and cleared source argument
/// lists cannot replace either receipt. MP2b separately admits the selection
/// against the binder's `where machine ... satisfies` bound; this check only refuses a
/// destination with no selection to bind, which later stages would otherwise
/// have to reject without a source diagnostic.
pub(super) fn validate_native_callback_selections(
    program: &dyn crate::CheckingInput,
    target: &str,
    arguments: &[typed_trees::expression::StaticMachineArgument],
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    selected: typed_trees::typed_trees::calls::PublicTraitRequirementSignature<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let signature = selected.declaration;
    if signature.native_callback_parameters.is_empty() {
        return;
    }
    let machine_binders = program
        .state_signature_type_parameters(signature)
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
        .collect::<Vec<_>>();
    let applied = if application.is_valid() {
        if !program
            .expression_table
            .static_machine_application_is_valid(application)
        {
            diagnostics.push(Diagnostic::error(format!(
                "call `{target}` lost its original native callback application",
            )));
            return;
        }
        let Ok(applied) =
            typed_trees::static_applications::StaticApplicationView::new(program, application)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "call `{target}` lost its complete native callback binding correspondence",
            )));
            return;
        };
        if applied.application().binding_declaration != signature.symbol {
            diagnostics.push(Diagnostic::error(format!(
                "call `{target}` native callback bindings do not belong to its original declaration",
            )));
            return;
        }
        Some(applied)
    } else {
        None
    };
    let selections =
        crate::machine_calls::machine_parameters::machine_selection_arguments(program, arguments)
            .collect::<Vec<_>>();
    for callback in &signature.native_callback_parameters {
        let mut binders = machine_binders
            .iter()
            .enumerate()
            .filter(|(_, binder)| binder.name == callback.binder);
        let (Some((ordinal, binder)), None) = (binders.next(), binders.next()) else {
            diagnostics.push(Diagnostic::error(format!(
                "native callback `{}` of `{}` names `{}`, which is not exactly one `machine` binder of the signature",
                callback.name, signature.name, callback.binder,
            )));
            continue;
        };
        let has_selection = if let Some(applied) = &applied {
            applied.bindings().iter().any(|binding| {
                if binding.parameter != binder.symbol {
                    return false;
                }
                let typed_trees::static_applications::StaticMachineBindingValue::Machine(argument) =
                    &binding.value
                else {
                    return false;
                };
                crate::machine_calls::machine_parameters::machine_selection_arguments(
                    program,
                    std::slice::from_ref(argument),
                )
                .next()
                .is_some()
            })
        } else {
            selections.get(ordinal).is_some()
        };
        if !has_selection {
            diagnostics.push(Diagnostic::error(format!(
                "call `{}` supplies no static machine argument for native callback `{}` from `{}`",
                target, callback.name, callback.binder,
            )));
        }
    }
}
