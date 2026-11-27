//! Construct a typed caller containing the actual source call. Its selected
//! declaration and materialized arguments go through ordinary call checking.

use super::super::typed_invocation::TypedInvocationDeclaration;
use super::arguments;
use crate::compile_time_invocations::BuildTimeValue;
use crate::source_state::TypedTrees;
use crate::source_state::callable_signature_shapes::CallableDeclaration;
use crate::source_state::data::TypeParameterKind;
use crate::source_state::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
};
use crate::source_state::machine::Machine;
use crate::source_state::name::Identifier;
use crate::source_state::references::{StaticArgumentCall, StaticArgumentRole};
use crate::source_state::reflection::construction::declaration_query_argument_role;
use crate::source_state::types::TypeReferenceHandle;
use symbols::SymbolHandle;

/// One generated caller in a probe program, and the call it returns.
#[derive(Debug, Clone)]
pub(super) struct TypedCall {
    pub(super) caller: SymbolHandle,
    pub(super) call: ExpressionHandle,
    /// Actual backing and issued opaque inputs of the checked probe. Constructed
    /// source values retain their ordinary literal premises.
    pub(super) input_values: Vec<BuildTimeValue>,
}

/// Adds to `probe` a new caller that returns `request` with `arguments`.
/// `program` is the source `probe` was copied from: every authored row is
/// read there, so callers added to the same probe earlier cannot change what
/// this one reads. A failure can leave part of a caller in `probe`.
pub(super) fn prepare_into(
    program: &TypedTrees,
    probe: &mut TypedTrees,
    requester: SymbolHandle,
    declaration: CallableDeclaration<'_>,
    request: &TableCallExpression,
    arguments: &[BuildTimeValue],
    request_expression: ExpressionHandle,
    selected: Option<(&StaticMachineArgument, SymbolHandle)>,
) -> Result<TypedCall, String> {
    if request.authored_machine_application.is_valid()
        && !program
            .expression_table
            .static_machine_application_is_valid(request.authored_machine_application)
    {
        return Err("source invocation has no retained static application".into());
    }
    let source_span = if request_expression.is_valid() {
        if !program
            .expression_table
            .expression_is_valid(request_expression)
            || program.expression_table.call_node(request_expression) != Some(request)
        {
            return Err(
                "source invocation call differs from its retained source occurrence".into(),
            );
        }
        program.expression_table.try_source_span(request_expression)
    } else {
        None
    };
    // Allocate this call's real source owner before closing the selected
    // application. Its result below comes from that public signature before
    // the caller enters typed preparation or judgment.
    let original_result = declaration.shape(program)?.return_type;
    let caller = TypedInvocationDeclaration::new(probe, requester, "@const-invocation");
    let mut call = request.clone();
    if request_expression.is_valid() && matches!(declaration, CallableDeclaration::Signature(_)) {
        // This is another actual call of the same public requirement. Its
        // application belongs to this caller, while the original source row
        // and every supplied operand keep their existing owner and identity.
        let (_, captures, application) = probe.expression_table.copy_call_operands_from(
            &program.expression_table,
            &[],
            request.authored_machine_application,
        );
        if !captures.is_empty() || !application.is_valid() {
            return Err("copied public call has no complete source application".into());
        }
        probe
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = caller.symbol;
        call.authored_machine_application = application;
    }
    let concrete = probe
        .expression_table
        .insert(ExpressionNode::Call(call.clone()));
    let selected_signature = if let Some((selection, selection_owner)) = selected {
        if request_expression.is_valid() || request.receiver.is_valid() {
            return Err("a selected invocation cannot replace an authored receiver".into());
        }
        if !program.symbols.symbols().nodes().is_valid(selection_owner) {
            return Err("selected invocation lost its supplying declaration".into());
        }
        let mut selection = probe
            .expression_table
            .copy_static_argument_from_self(selection);
        let name = Identifier::from(probe.symbols.name(selection.symbol));
        if selection.path.is_empty() {
            // The resolved declaration supplies diagnostic text only; its
            // symbol and complete source operand remain the selected identity.
            selection.path = vec![name.clone()].into_boxed_slice();
        }
        let application = if selection.type_reference.is_valid() {
            selection.type_reference
        } else {
            probe.static_operand_type_reference_in_scope(&selection, selection_owner)?
        };
        call.requirement_application = application;
        let mut path = arguments::name_path(probe, selection.symbol, name);
        path.static_application = probe.expression_table.insert_static_member(selection);
        call.receiver = probe.expression_table.insert(ExpressionNode::Name(path));
        *probe.expression_table.expression_mut(concrete) = ExpressionNode::Call(call);
        probe.retain_call_static_argument_context(concrete, caller.symbol)?;
        call = probe
            .expression_table
            .call_node(concrete)
            .cloned()
            .ok_or("selected invocation lost its retained source call")?;
        probe.prepare_applied_callable_signatures_for_application(caller.symbol, application)?;
        Some(
            probe
                .named_conformance_call_member(&call)?
                .ok_or("selected invocation has no exact public requirement row")?
                .2
                .clone(),
        )
    } else {
        None
    };
    let declaration = selected_signature
        .as_ref()
        .map_or(declaration, CallableDeclaration::Signature);
    let shape = declaration.shape(probe)?;
    let parameters = shape.parameters.to_vec();
    let declared_result = shape.return_type;
    let type_parameters = declaration.static_parameters(probe)?.to_vec();
    // These are fresh caller binders, not the callee's region identities.
    // Ordinary call checking relates them through the actual formal inputs.
    let lifetime_parameters = declaration.lifetime_parameters(probe)?.to_vec();
    // Region substitution resolves declaration ordinals through the canonical
    // machine table, including inside nominal applications. Publish this
    // caller's telescope before constructing any such types.
    let mut body = caller.define(probe, original_result, lifetime_parameters);
    let declaration_argument = declaration_query_argument_role(
        program,
        StaticArgumentCall::Expression(ExpressionHandle::invalid(), request),
    ) == StaticArgumentRole::DeclarationDescription;
    if declaration_argument {
        if !arguments.is_empty()
            || program
                .expression_table
                .expression_handles(request.arguments)
                .len()
                != parameters.len()
        {
            return Err(
                "a declaration query retains its authored operand, not a runtime argument".into(),
            );
        }
    } else if parameters.len() != arguments.len() {
        return Err("constant invocation arguments differ from its checked entry arity".into());
    }
    let static_call = StaticArgumentCall::Expression(request_expression, request);
    let mut supplied = static_call.arguments(program).to_vec();
    if !request.machine_arguments.is_empty()
        && request.machine_arguments.as_ref() != supplied.as_slice()
    {
        return Err(
            "source invocation static arguments differ from their retained application".into(),
        );
    }
    if type_parameters
        .iter()
        .any(|parameter| !matches!(parameter.kind, TypeParameterKind::Type))
    {
        return Err("source invocation non-type static arguments are not materialized".into());
    }
    if type_parameters.len() != supplied.len() {
        return Err("source invocation lost its complete static argument list".into());
    }
    let owner = static_call.lexical_owner(program);
    if !supplied.is_empty() && !program.symbols.symbols().nodes().is_valid(owner) {
        return Err("source invocation static arguments lost their supplying declaration".into());
    }
    let mut substitutions = Vec::with_capacity(type_parameters.len());
    let mut materialized = false;
    for (parameter, argument) in type_parameters.iter().zip(&mut supplied) {
        if argument.type_reference.is_valid() {
            // A stale supplied reference is malformed, not a missing operand
            // that can be rebuilt from its remaining symbol or spelling.
            if !program
                .type_reference_table
                .contains_type_reference(argument.type_reference)
            {
                return Err("source invocation type argument has no retained reference".into());
            }
        } else {
            // A bare type retains its declaration and full static application.
            // Form its reference through that same owner before substitution;
            // applied type expressions already have their retained reference.
            argument.type_reference = probe
                .static_operand_type_reference_in_scope(argument, owner)
                .map_err(|reason| format!("source invocation type argument: {reason}"))?;
            materialized = true;
        }
        substitutions.push((parameter.symbol, argument.type_reference));
    }
    if !substitutions.is_empty() {
        let references = substitutions
            .iter()
            .map(|(_, reference)| *reference)
            .collect::<Vec<_>>();
        probe
            .require_owned_application_operands(&references, owner)
            .map_err(|reason| format!("source invocation type argument ownership: {reason}"))?;
    }
    if materialized {
        let application = call.authored_machine_application;
        if application.is_valid() {
            let retained = probe
                .expression_table
                .static_machine_application_arguments_mut(application);
            if retained.len() != supplied.len() {
                return Err("source invocation static application lost its argument span".into());
            }
            retained.clone_from_slice(&supplied);
        }
        if request_expression.is_valid()
            && !request.machine_arguments.is_empty()
            && call.authored_machine_application == request.authored_machine_application
        {
            let ExpressionNode::Call(original) =
                probe.expression_table.expression_mut(request_expression)
            else {
                return Err(
                    "source invocation lost its original call during operand preparation".into(),
                );
            };
            original.machine_arguments = supplied.clone().into_boxed_slice();
        }
    }
    let (input_types, result_type) = probe.instantiate_invocation_signature(
        &parameters
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect::<Vec<_>>(),
        declared_result,
        &substitutions,
        declaration,
        &mut body.machine,
    )?;
    body.state.return_type = result_type;
    let actuals =
        arguments::materialize(probe, &mut body.state, &parameters, arguments, &input_types)?;
    if materialized && !call.machine_arguments.is_empty() {
        call.machine_arguments = supplied.into_boxed_slice();
    }
    if !declaration_argument {
        call.arguments = probe
            .expression_table
            .insert_expression_handles(actuals.expressions);
    }
    *probe.expression_table.expression_mut(concrete) = ExpressionNode::Call(call);
    let call_span = source_span
        .or_else(|| probe.symbols.symbol_provenance_source_span(requester))
        .ok_or("source invocation lost its actual requesting source")?;
    probe.expression_table.set_source_span(concrete, call_span);
    let owner = body.return_expression(probe, concrete)?;
    probe.retain_call_static_argument_context(concrete, owner)?;
    Ok(TypedCall {
        caller: owner,
        call: concrete,
        input_values: actuals.input_values,
    })
}

pub(super) fn direct_call(
    program: &TypedTrees,
    machine: &Machine,
) -> Result<TableCallExpression, String> {
    let [entry] = program
        .machine_states(machine)
        .as_array()
        .ok_or("constant invocation has no exact entry state")?;
    Ok(declaration_call(entry.symbol, entry.name.clone()))
}

pub(super) fn declaration_call(target: SymbolHandle, name: Identifier) -> TableCallExpression {
    TableCallExpression {
        // The retained declaration is the callee; there is no runtime value.
        runtime_call: None,
        authored_call_selection: None,
        authored_call_binding: None,
        authored_machine_application: Default::default(),
        requirement_application: Default::default(),
        exact_coercion_target: TypeReferenceHandle::invalid(),
        receiver: ExpressionHandle::invalid(),
        target_symbol: target,
        static_machine_parameter: SymbolHandle::invalid(),
        target: name,
        static_requirement_dispatch: None,
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        installed_interrupt_operation: None,
        layout_query: None,
        native_code_layout: None,
        build_selection: Default::default(),
        arguments: Default::default(),
        evidence_arguments: Box::default(),
        operational_acknowledgement: language_core::CallOperationalAcknowledgement::default(),
    }
}
