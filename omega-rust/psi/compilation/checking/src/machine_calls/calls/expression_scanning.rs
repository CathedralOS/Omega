//! Value-position expression-call scanning and bound validation.
//!
//! The parent call validator owns shared argument/type rules. This child
//! owns recursive expression traversal, value-callee resolution, and the
//! diagnostics emitted at those scan sites.

use super::{
    free_machine_entry_state, machine_state_by_symbol, register_read_mnemonic,
    selected_float_operation_retains_argument_policy, user_asm_contract,
    validate_asm_operand_constraint, validate_call_arguments,
    validate_machine_call_type_parameter_bounds, validate_value_call_arguments,
};
use crate::declarations::symbols::TopLevelSymbols;
use crate::proof_contracts::arithmetic_domains::{self, ValueEnvironment};
use crate::value_custody::locals::WritableRoots;
use diagnostics::Diagnostic;
use symbols::BuiltinFunction;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceNode;

mod result_realization;
mod target_resolution;
mod traversal;

pub(crate) use result_realization::report_nested_call_in_local_initializer;
use result_realization::report_void_value_callee;
pub use result_realization::{
    nested_boundary_applications_compose, result_initializer_call_is_admitted,
    unit_result_initializer_call_is_admitted,
};
pub(crate) use target_resolution::declared_receiver_type_reference;
use target_resolution::{named_type_reference_name, report_unresolved_value_call};
pub(crate) use traversal::validate_value_position_calls;

/// A value-position call from emitted concrete code may not retain a generic
/// callee: its result slot has no concrete layout. Uninstantiated generic
/// templates are different—they are checked modularly but never emitted, and
/// their symbolic calls are resolved by fixed-point specialization once a
/// concrete outer call selects them.
fn fence_generic_value_callee(
    program: &dyn crate::CheckingInput,
    caller_machine: &Machine,
    callee_machine: &Machine,
    target: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // An uninstantiated generic template is checked modularly but is never
    // emitted. Its symbolic value calls become concrete when an outer call
    // specializes the template. Keep the fence for concrete callers whose
    // selected callee somehow remains generic: that is still an incomplete
    // lowering and must fail loudly.
    if program
        .typed_program()
        .callable_body_has_open_parameters(caller_machine)
        || program.machine_type_parameters(callee_machine).is_empty()
    {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "a value call to generic machine `{target}` cannot derive a complete \
         type/const/machine/conformance specialization tuple from its argument and result types; add a \
         concrete destination annotation or provide concrete argument type evidence",
    )));
}

/// The receiver's spelled member chain, root -> leaf (`["self", "p",
/// "second"]` for `self.p.second.stored()`). `None` for non-place receivers
/// (calls, literals). Mirrors the state-call plan's `append_receiver_path`
/// walk at the typed layer.
pub(super) fn receiver_member_chain(
    program: &dyn crate::CheckingInput,
    receiver: typed_trees::expression::ExpressionHandle,
) -> Option<Vec<String>> {
    if !receiver.is_valid() {
        return None;
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (!members.is_empty()).then(|| {
                members
                    .iter()
                    .map(|member| member.as_str().to_string())
                    .collect()
            })
        }
        ExpressionNode::Member(member) => {
            let mut chain = receiver_member_chain(program, member.receiver)?;
            chain.push(member.member.as_str().to_string());
            Some(chain)
        }
        ExpressionNode::Borrow(inner) => receiver_member_chain(program, inner.target),
        _ => None,
    }
}

/// A resolved machine state a VALUE-position call invokes.
#[derive(Clone, Copy)]
struct ValueCallee<'program> {
    machine: &'program Machine,
    state: &'program State,
    /// The spelling type-parameter bound diagnostics name.
    spelling: &'program str,
}

impl<'program> ValueCallee<'program> {
    /// A callee reported by the call's own spelling.
    fn named(
        machine: &'program Machine,
        state: &'program State,
        call: &'program TableCallExpression,
    ) -> Self {
        Self {
            machine,
            state,
            spelling: call.target.as_str(),
        }
    }

    /// A callee reported by its declared state name.
    fn declared(machine: &'program Machine, state: &'program State) -> Self {
        Self {
            machine,
            state,
            spelling: state.name.as_str(),
        }
    }
}

/// One VALUE-position call occurrence and the caller facts its resolved
/// callee is checked against.
struct ValueCallSite<'a> {
    program: &'a dyn crate::CheckingInput,
    current_machine: &'a Machine,
    current_state: &'a State,
    prior_statements: &'a [typed_trees::statement::StatementNode],
    value_environment: &'a ValueEnvironment,
    expression: ExpressionHandle,
    call: &'a TableCallExpression,
    arguments: &'a [ExpressionHandle],
    executes: bool,
}

impl ValueCallSite<'_> {
    /// Every resolved callee owes the same checks, whichever route found it:
    /// a value-producing result, a concrete specialization, its type-parameter
    /// bounds, and argument arity and classes.
    fn validate_callee(
        &self,
        symbols: &TopLevelSymbols<'_>,
        callee: ValueCallee<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        report_void_value_callee(
            self.program,
            callee.machine,
            self.current_machine,
            self.current_state,
            callee.state,
            self.expression,
            diagnostics,
        );
        fence_generic_value_callee(
            self.program,
            self.current_machine,
            callee.machine,
            self.call.target.as_str(),
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            self.program,
            symbols,
            callee.machine,
            callee.state,
            callee.spelling,
            self.arguments,
            self.current_machine,
            Some(self.current_state),
            diagnostics,
        );
        validate_value_call_arguments(
            self.program,
            self.current_machine,
            self.current_state,
            self.prior_statements,
            self.value_environment,
            self.arguments,
            callee.machine,
            callee.state,
            self.executes,
            diagnostics,
            &self.call.machine_arguments,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_expression_call_bounds(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: &State,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_environment: &ValueEnvironment,
    expression: ExpressionHandle,
    call: &TableCallExpression,
    statement_root: bool,
    executes: bool,
    boundary_operator_applications: &mut Vec<crate::ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if call.quotient_operation.is_some() {
        // A sealed `Quotient` request is not an ordinary value call: the
        // quotient formation and correspondence checks own its judgment.
        // The outer scanner still visits the call's argument expressions.
        return;
    }
    let unit_statement = statement_root
        && crate::machine_calls::calls::unit_statement_call_is_admitted(
            program,
            current_machine,
            current_state,
            expression,
        );
    crate::proof_contracts::contract_entailment::validate_const_range_call(
        program,
        current_machine,
        Some(current_state),
        call.target_symbol,
        &call.machine_arguments,
        program.expression_table.expression_handles(call.arguments),
        diagnostics,
    );
    let evidence_receiver = super::generic_requirement::expression_conformance_receiver(
        program,
        current_machine,
        call.receiver,
    );
    match super::generic_requirement::named_conformance_requirement(
        program,
        current_machine,
        evidence_receiver,
        call.target_symbol,
    ) {
        Ok(Some(requirement)) => {
            if !unit_statement
                && (!requirement.signature.return_type.is_valid()
                    || matches!(
                        program
                            .type_reference_table
                            .type_reference(requirement.signature.return_type),
                        TypeReferenceNode::Unit
                    ))
            {
                diagnostics.push(Diagnostic::error(format!("trait requirement `{}::{}` does not return a value but is used in a VALUE position", requirement.application.declaration.name, requirement.signature.name)));
            }
            super::generic_requirement::validate_named_conformance_arguments(
                program,
                current_machine,
                Some(current_state),
                value_environment,
                program.expression_table.expression_handles(call.arguments),
                &call.machine_arguments,
                &requirement,
                writable_roots,
                diagnostics,
            );
            return;
        }
        Err(error) => {
            diagnostics.push(Diagnostic::error(error));
            return;
        }
        Ok(None) => {}
    }
    let call_arguments = program.expression_table.expression_handles(call.arguments);
    match super::dynamic_requirement::dynamic_selector_call(
        program,
        current_machine,
        Some(current_state),
        super::dynamic_requirement::expression_trait_receiver(program, call.receiver),
        call.target_symbol,
        call_arguments,
    ) {
        Ok(Some(dynamic)) => {
            if !unit_statement
                && (!dynamic.signature.return_type.is_valid()
                    || matches!(
                        program
                            .type_reference_table
                            .type_reference(dynamic.signature.return_type),
                        TypeReferenceNode::Unit
                    ))
            {
                diagnostics.push(Diagnostic::error(format!("trait requirement `{}::{}` does not return a value but is used in a VALUE position", dynamic.application.declaration.name, dynamic.signature.name)));
            }
            super::dynamic_requirement::validate_dynamic_selector_arguments(
                program,
                current_machine,
                Some(current_state),
                value_environment,
                call_arguments,
                &call.machine_arguments,
                &dynamic,
                writable_roots,
                diagnostics,
            );
            return;
        }
        Err(error) => {
            diagnostics.push(Diagnostic::error(error));
            return;
        }
        Ok(None) => {}
    }
    if super::unit_returns::call_returns_unit(program, current_machine, expression)
        && !unit_statement
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}`: `{}(..)` does not return a value but is used in a VALUE position; Unit cannot supply a scalar operand or local",
            current_machine.name, current_state.name, call.target,
        )));
        return;
    }
    if super::unit_returns::admitted_build_declaration_call(program, expression) {
        // The exact admitted intrinsic returns Unit. The branch above still
        // rejects value uses, and the outer scanner visits every operand.
        return;
    }
    if crate::proof_contracts::proof_embeddings::is_exact_embed_call(program, call) {
        // Embedding is a proof term, not a value-machine invocation. Its
        // dedicated whole-program gate validates the exact unary carrier
        // shape and rejects every executable occurrence. The outer scanner
        // still visits its operand, including any nested ordinary call.
        return;
    }
    let builtin = program
        .symbols
        .builtin_function_for_symbol(call.target_symbol);
    if (builtin == Some(BuiltinFunction::AsmSnapshotFlags)
        || builtin.and_then(register_read_mnemonic).is_some())
        && !call.receiver.is_valid()
    {
        let arguments = program.expression_table.expression_handles(call.arguments);
        if !arguments.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "asm intrinsic `{}` takes 0 operands, found {}",
                call.target,
                arguments.len()
            )));
        }
        return;
    }

    if let Some(builtin @ (BuiltinFunction::AsmPortIn | BuiltinFunction::AsmReadMsr)) = builtin
        && !call.receiver.is_valid()
    {
        let intrinsic = builtin.name();
        let (instruction, operand_index) = if builtin == BuiltinFunction::AsmPortIn {
            ("in", 1)
        } else {
            ("rdmsr", 1)
        };
        let arguments = program.expression_table.expression_handles(call.arguments);
        if arguments.len() != 1 {
            diagnostics.push(Diagnostic::error(format!(
                "asm intrinsic `{intrinsic}` takes 1 operand, found {}",
                arguments.len()
            )));
            return;
        }
        let contract = user_asm_contract(instruction);
        validate_asm_operand_constraint(
            program,
            current_machine,
            Some(current_state),
            instruction,
            arguments[0],
            contract.operands[operand_index],
            diagnostics,
        );
        return;
    }

    if let Some(operator) = typed_trees::operator::resolve_named_expression_call(program, call) {
        if call.receiver.is_valid()
            && !crate::declarations::operators::namespaces::expression_call_has_operator_namespace(
                program, call,
            )
        {
            diagnostics.push(Diagnostic::error(
                "value-qualified machine calls are retired; use a declaration namespace and explicit arguments",
            ));
            return;
        }
        let arguments = program.expression_table.expression_handles(call.arguments);
        let parameters = program.operator_parameters(operator);
        let operand_types = arguments
            .iter()
            .map(|argument| {
                crate::value_custody::places::declared_place_type(
                    program,
                    current_machine,
                    Some(current_state),
                    *argument,
                )
                .or_else(|| {
                    crate::declarations::operators::landed_integer_literal_type_reference(
                        program, *argument,
                    )
                })
            })
            .collect::<Vec<_>>();
        match crate::declarations::operators::validate_named_operator_application(
            program,
            symbols,
            operator,
            &call.machine_arguments,
            &operand_types,
        ) {
            Ok(Some(bindings)) if operator.is_boundary => {
                let application =
                    crate::declarations::operators::validated_boundary_operator_application(
                        crate::ValidatedBoundaryOperatorApplicationUseSite::Expression(expression),
                        operator,
                        bindings,
                    );
                crate::declarations::operators::retain_validated_boundary_operator_application(
                    boundary_operator_applications,
                    application,
                    diagnostics,
                );
            }
            Ok(None) if operator.is_boundary && call.machine_arguments.is_empty() => {
                if let Some(application) =
                    crate::declarations::operators::validated_symbolic_boundary_operator_application(
                        program,
                        current_machine,
                        crate::ValidatedBoundaryOperatorApplicationUseSite::Expression(expression),
                        operator,
                        &operand_types,
                    )
                {
                    crate::declarations::operators::retain_validated_boundary_operator_application(
                        boundary_operator_applications,
                        application,
                        diagnostics,
                    );
                }
            }
            Ok(_) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
        validate_call_arguments(
            program,
            current_machine,
            Some(current_state),
            value_environment,
            arguments,
            call.target.as_str(),
            call.target.as_str(),
            parameters,
            writable_roots.statements,
            selected_float_operation_retains_argument_policy(program, operator.symbol),
            &[],
            |argument, required| {
                crate::value_custody::expression_types::argument_matches_carrier(
                    program,
                    current_machine.symbol,
                    argument,
                    required,
                    &[],
                )
            },
            diagnostics,
        );
        validate_named_float_to_integer_call(
            program,
            current_machine,
            current_state,
            value_environment,
            operator,
            arguments,
            diagnostics,
        );
        return;
    }

    // Receiverless calls use lexical declaration scope. A qualified value is
    // never permission to fall back to the current machine; its leaf spelling
    // is diagnostic payload, not target identity. The static-machine grammar
    // rejects authored self receivers before reaching this retained-tree query.
    let (call_is_self, external_receiver_name): (bool, Option<&str>) = if !call.receiver.is_valid()
    {
        (true, None)
    } else {
        match program.expression_table.expression(call.receiver) {
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                if members.is_empty() || matches!(members, [r] if r.is_self_receiver()) {
                    (true, None)
                } else {
                    (false, members.last().map(Identifier::as_str))
                }
            }
            ExpressionNode::Member(member) => (false, Some(member.member.as_str())),
            _ => (false, None),
        }
    };

    let arguments = program.expression_table.expression_handles(call.arguments);
    let site = ValueCallSite {
        program,
        current_machine,
        current_state,
        prior_statements: writable_roots.statements,
        value_environment,
        expression,
        call,
        arguments,
        executes,
    };

    // A named implementation qualifies its requirement call
    // (`Ascending::before(left, right)`); resolution already selected that
    // implementation's row, an ordinary machine called with its explicit
    // arguments.
    if let Some(path) = program.expression_table.name_path_node(call.receiver)
        && path.symbol.is_valid()
        && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Conformance
    {
        match machine_state_by_symbol(program, call.target_symbol) {
            Some((machine, state)) => {
                site.validate_callee(
                    symbols,
                    ValueCallee::named(machine, state, call),
                    diagnostics,
                );
            }
            None => diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}`: {}",
                current_machine.name,
                current_state.name,
                super::generic_requirement::unresolved_implementation_call(
                    program,
                    path.symbol,
                    call.target.as_str(),
                ),
            ))),
        }
        return;
    }

    if let Some(signature) = super::write_frames::boundary_trait_expression_call_signature(
        program,
        current_machine,
        expression,
    ) {
        // A result's scalar/structural use does not change the requirement's
        // argument contract. Apply the same custody, type and bound checks
        // as statement-position boundary calls before execution planning.
        super::validate_call_arguments(
            program,
            current_machine,
            Some(current_state),
            value_environment,
            arguments,
            signature.name.as_str(),
            signature.name.as_str(),
            program.state_signature_parameters(signature),
            writable_roots.statements,
            false,
            &[],
            |argument, required| {
                crate::value_custody::expression_types::argument_matches_carrier(
                    program,
                    current_machine.symbol,
                    argument,
                    required,
                    &[],
                )
            },
            diagnostics,
        );
        return;
    }

    // Self-call or receiverless call: select a machine requirement or entry,
    // never an internal state that happens to have the same spelling.
    if call_is_self {
        if let Some(signature) =
            program.machine_parameter_signature_in(current_machine, call.target_symbol)
        {
            if !signature.return_type.is_valid() && !unit_statement {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}`: machine parameter `{}` does not return a value but is used in a VALUE position",
                    current_machine.name,
                    current_state.name,
                    signature.name,
                )));
            }
            super::validate_machine_parameter_arguments(
                program,
                current_machine,
                Some(current_state),
                value_environment,
                arguments,
                signature,
                &call.machine_arguments,
                writable_roots,
                diagnostics,
            );
            return;
        }

        // The callee is a selected entry or a sibling sharing attached data (`machine
        // Main::pick<T [copy]>` called from `machine Main::main`; the
        // statement path uses `symbols.attached_machine_state` too), or a
        // free machine (`compute(item)` -- no `self.`, no receiver).
        let callee = machine_state_by_symbol(program, call.target_symbol)
            .map(|(machine, state)| ValueCallee::named(machine, state, call))
            .or_else(|| {
                let attached_data = current_machine.attached_data.as_ref()?;
                symbols
                    .attached_machine_state(program, attached_data.as_str(), call.target.as_str())
                    .map(|(machine, state)| ValueCallee::named(machine, state, call))
            })
            .or_else(|| {
                free_machine_entry_state(program, symbols, call.target.as_str())
                    .map(|(machine, state)| ValueCallee::named(machine, state, call))
            });
        if let Some(callee) = callee {
            site.validate_callee(symbols, callee, diagnostics);
            return;
        }
        report_unresolved_value_call(
            program,
            current_machine,
            current_state,
            symbols,
            None,
            None,
            call,
            diagnostics,
        );
        return;
    }

    let receiver_name = external_receiver_name.unwrap_or_default();
    let receiver_type_reference = crate::value_custody::places::declared_place_type_raw(
        program,
        current_machine,
        Some(current_state),
        call.receiver,
    );
    if crate::declarations::traits::report_dynamic_receiver_call_error(
        program,
        receiver_type_reference,
        call.target.as_str(),
        diagnostics,
    ) {
        return;
    }

    let namespace = traversal::is_declaration_namespace(program, call.receiver);
    // A placed accessor is selected by its compiler-owned plan, not by methods
    // on a data type. Preserve this memory operation without retaining general
    // data-receiver dispatch or inferring an implicit operand.
    let placed_accessor = receiver_type_reference
        .and_then(|reference| program.placed_view_field_plan_for_type_reference(reference))
        .is_some_and(|(_, field)| {
            field.accessor_targets.iter().any(|target| {
                target.state_symbol == call.target_symbol
                    && target.operation == call.target.as_str()
            })
        });
    if !namespace
        && !placed_accessor
        && machine_state_by_symbol(program, call.target_symbol).is_some()
    {
        if program
            .expression_table
            .name_path_node(call.receiver)
            .is_some_and(|path| !path.head_symbol.is_valid() || !path.symbol.is_valid())
        {
            diagnostics.push(Diagnostic::error(
                "machine call has no resolved declaration namespace; its head is unavailable or ambiguous",
            ));
            return;
        }
        diagnostics.push(Diagnostic::error(
            "value-qualified machine calls are retired; use a declaration namespace and explicit arguments",
        ));
        return;
    }
    // Static qualification retains the exact owner; no data type or display
    // name can supply a different declaration when the retained target is absent.
    if let Some((callee_machine, callee_state)) =
        machine_state_by_symbol(program, call.target_symbol)
    {
        let qualifier = program
            .expression_table
            .name_path_node(call.receiver)
            .map_or_else(symbols::SymbolHandle::invalid, |path| path.symbol);
        if namespace
            && callee_machine.attached_data_symbol.is_valid()
            && qualifier != callee_machine.attached_data_symbol
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected attached call `{}` does not belong to the receiver's exact data type",
                call.target,
            )));
            return;
        }
        site.validate_callee(
            symbols,
            ValueCallee::declared(callee_machine, callee_state),
            diagnostics,
        );
        return;
    }

    report_unresolved_value_call(
        program,
        current_machine,
        current_state,
        symbols,
        Some(receiver_name),
        receiver_type_reference.and_then(|reference| named_type_reference_name(program, reference)),
        call,
        diagnostics,
    );
}

fn validate_named_float_to_integer_call(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: &State,
    value_environment: &ValueEnvironment,
    operator: &typed_trees::operator::OperatorDefinition,
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((_, semantic)) = crate::float_integer_conversion_contract(program, operator) else {
        return;
    };
    if !matches!(
        semantic.kernel(),
        numerics::float_semantics_catalog::FloatSemanticKernel::ToIntegerExact
    ) || program
        .type_reference_table
        .arithmetic_domain(operator.return_type)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return;
    }
    let Some(target) = program.primitive_type_reference(operator.return_type) else {
        return;
    };
    let [value] = arguments else {
        return;
    };
    if arithmetic_domains::float_source_proves_integer_conversion(
        program,
        current_machine,
        Some(current_state),
        value_environment,
        *value,
        target,
    ) {
        return;
    }
    let path = program.operator_path_string(operator.name);
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` cannot prove unqualified `{}` operand `{}` is finite and truncates into `{}`; constrain it with a declared range or dominating non-NaN/range guard, or select result type `{} in Trapping` or `{} in Saturating`",
        current_machine.name,
        current_state.name,
        path,
        program.expression_table.display_name(*value),
        target.name(),
        target.name(),
        target.name(),
    )));
}
