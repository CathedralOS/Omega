//! Recursive statement and expression traversal for value-position calls.
//!
//! This owner preserves source-order diagnostics while finding value calls,
//! and malformed bare names. Indexed reads keep their ordinary type, custody,
//! and per-occurrence bounds checks; checked structural projections retain every
//! field and selector for lowering rather than admitting source arrangements. Per-call target,
//! argument, and bound validation remains in the parent module.
//! Atomic carriers use the same authored-operand decoder as arithmetic checking
//! and execution planning. Their synthetic resident-update formula is not source
//! arithmetic; visiting it would invent boolean/numeric operations and repeat
//! checks on the observed-prior placeholder instead of the authored operands.

use super::validate_expression_call_bounds;
use crate::declarations::symbols::TopLevelSymbols;
use crate::proof_contracts::arithmetic_domains::ValueEnvironment;
use crate::value_custody::locals::WritableRoots;
use crate::value_custody::struct_literals::data_declares_field;
use diagnostics::Diagnostic;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

/// A resolved declaration namespace selects a callee; it is not an evaluated
/// receiver. This applies only to the namespace slot of a checked call, never
/// to a module spelling supplied as a runtime argument or standalone value.
pub(super) fn is_declaration_namespace(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    let Some(path) = program.expression_table.name_path_node(expression) else {
        return false;
    };
    let is_namespace = |symbol: symbols::SymbolHandle| {
        symbol.is_valid()
            && matches!(
                program.symbols.get(symbol).kind,
                symbols::SymbolKind::Module
                    | symbols::SymbolKind::Data
                    | symbols::SymbolKind::Trait
                    | symbols::SymbolKind::Domain
                    | symbols::SymbolKind::BuiltinType
                    | symbols::SymbolKind::Machine
                    | symbols::SymbolKind::Conformance
                    | symbols::SymbolKind::ConformanceParameter
            )
    };
    // Loader-certified package aliases have no declaration symbol. The
    // resolved endpoints must still be exact namespaces, and every retained
    // intermediate declaration must agree. A lexical value at either end
    // cannot become a namespace merely because its suffix spells a type.
    [path.head_symbol, path.symbol]
        .into_iter()
        .all(is_namespace)
        && program
            .expression_table
            .name_path_member_symbols(path.member_symbols)
            .iter()
            .copied()
            .filter(|symbol| symbol.is_valid())
            .all(is_namespace)
}

/// FROZEN DECISION 13 residue (value-position complement of `validate_call_node`).
///
/// Walk every expression in every statement of `state` and enforce
/// machine-call type-parameter bounds for VALUE-position calls
/// (`let r = self.pick(&self.h)`).  These never reach `validate_call_node`
/// because they appear as `ExpressionNode::Call` inside expression trees,
/// not as top-level `StatementNode::Call` nodes.
///
/// Scope: covers all expression positions that feed into statements
/// (LocalData initializers, assignment values/targets, guard expressions,
/// transition arguments, terminal expressions) and recurses into nested
/// call arguments.  Enforces the type-parameter BOUND check plus, for a
/// RESOLVED callee, argument arity (`report_argument_count_mismatch`) and the
/// shared argument admission (`validate_value_call_arguments`).
/// The remaining frontier is the UNRESOLVED callee: a value call whose target
/// no branch resolves falls through silently (the nonexistent-value-call gap),
/// which needs the complete value-call target resolver.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_value_position_calls(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_environment: &ValueEnvironment,
    transition_values: &crate::declarations::transitions::TransitionValueEnvironments,
    boundary_operator_applications: &mut Vec<crate::ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let executes = true;
    match statement {
        StatementNode::RootBinding(binding) => {
            for expression in [binding.receiver, binding.implementation_operand] {
                if expression.is_valid() {
                    scan_expression_calls(
                        program,
                        machine,
                        state,
                        symbols,
                        writable_roots,
                        value_environment,
                        expression,
                        executes,
                        boundary_operator_applications,
                        diagnostics,
                    );
                }
            }
        }
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                assignment.value,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
            scan_assignment_target_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                assignment.target,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
        }
        StatementNode::Call(call) => {
            // Statement-position call arguments may themselves be value calls.
            for argument in program.statement_table.expression_handles(call.arguments) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    symbols,
                    writable_roots,
                    value_environment,
                    *argument,
                    executes,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
        }
        StatementNode::Expression(expression) => {
            scan_expression_calls_at_position(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                *expression,
                true,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
        }
        StatementNode::LocalData(local_data) => {
            crate::value_custody::locals::StateValueScope {
                program,
                machine,
                state,
                symbols,
                prior_statements: writable_roots.statements,
                context: "local type bound",
            }
            .type_reference(local_data.type_reference, diagnostics);
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                local_data.initial_value,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    symbols,
                    writable_roots,
                    value_environment,
                    guard,
                    executes,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
            for target_handle in transition.targets() {
                let target = program.statement_table.transition_target(target_handle);
                // Reuse the same guard and evaluation-order premises as root
                // transition checking. Re-narrowing the aggregate statement
                // environment could restore facts invalidated by guard calls.
                match target {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for (argument_index, argument) in program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .enumerate()
                        {
                            scan_expression_calls(
                                program,
                                machine,
                                state,
                                symbols,
                                writable_roots,
                                transition_values
                                    .for_target(target_handle)
                                    .get(argument_index)
                                    .unwrap_or(value_environment),
                                *argument,
                                executes,
                                boundary_operator_applications,
                                diagnostics,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        scan_expression_calls(
                            program,
                            machine,
                            state,
                            symbols,
                            writable_roots,
                            transition_values
                                .for_target(target_handle)
                                .first()
                                .unwrap_or(value_environment),
                            *expression,
                            executes,
                            boundary_operator_applications,
                            diagnostics,
                        );
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

/// Recursively scan `expression` for `ExpressionNode::Call` nodes and
/// validate machine-call type-parameter bounds for each one found.
#[allow(clippy::too_many_arguments)]
fn scan_expression_calls(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_environment: &ValueEnvironment,
    expression: ExpressionHandle,
    executes: bool,
    boundary_operator_applications: &mut Vec<crate::ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    scan_expression_calls_at_position(
        program,
        machine,
        state,
        symbols,
        writable_roots,
        value_environment,
        expression,
        false,
        executes,
        boundary_operator_applications,
        diagnostics,
    );
}

/// An assignment target is scanned for the calls inside it -- `self.rows[self
/// .index()] = v` performs one -- but the target itself is not a read.
/// `value_custody::places` already reports an unknown direct or nested field
/// on a target, with the verb that belongs to a write, so reporting it here
/// too produced two diagnostics for one typo and called the write a read.
/// Operands below the target are ordinary reads and keep the read reports.
#[allow(clippy::too_many_arguments)]
fn scan_assignment_target_calls(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_environment: &ValueEnvironment,
    expression: ExpressionHandle,
    executes: bool,
    boundary_operator_applications: &mut Vec<crate::ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut target_diagnostics = Vec::new();
    scan_expression_calls(
        program,
        machine,
        state,
        symbols,
        writable_roots,
        value_environment,
        expression,
        executes,
        boundary_operator_applications,
        &mut target_diagnostics,
    );
    let read_reports = unknown_field_read_reports(program, machine, state, expression);
    diagnostics.extend(
        target_diagnostics
            .into_iter()
            .filter(|diagnostic| !read_reports.contains(&diagnostic.message)),
    );
}

/// The unknown-field READ messages this walker would raise for `expression`
/// itself, so an assignment target can drop exactly those and keep every other
/// finding the walk produced.
fn unknown_field_read_reports(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Vec<String> {
    let mut reports = Vec::new();
    if let Some(field_name) =
        crate::value_custody::places::direct_self_field_member(program, expression)
        && let Some(data) = crate::value_custody::places::machine_attached_data(program, machine)
        && !data_declares_field(program, data, field_name)
    {
        reports.push(format!(
            "machine `{}` state `{}` reads `self.{field_name}`, but data `{}` has no field \
             `{field_name}` (check the spelling of the field name)",
            machine.name.as_str(),
            state.name.as_str(),
            data.name.as_str()
        ));
    }
    if let Some((container, member)) = crate::value_custody::places::first_unknown_nested_field(
        program,
        machine,
        Some(state),
        expression,
    ) {
        reports.push(format!(
            "machine `{}` state `{}` reads a nested member `{member}`, but data `{container}` \
             has no field `{member}` (check the spelling of the field name)",
            machine.name.as_str(),
            state.name.as_str(),
        ));
    }
    reports
}

/// Only the direct expression statement may discard Unit. Recursive operands
/// remain value uses even if a malformed tree reuses that statement's handle.
#[allow(clippy::too_many_arguments)]
fn scan_expression_calls_at_position(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_environment: &ValueEnvironment,
    expression: ExpressionHandle,
    statement_root: bool,
    executes: bool,
    boundary_operator_applications: &mut Vec<crate::ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    // Unknown-field READ: a direct `self.<field>` read of a nonexistent field (a typo)
    // gets a clear error instead of silently passing type-check. Mirrors the
    // assignment-target write check (places.rs): scoped to a direct `self.<field>`
    // against the machine's top-level data fields, versioned data excluded. Nested
    // `self.a.b` (checked at `a` when the recursion reaches the receiver) and non-self
    // members are left alone. The recursion continues afterward.
    if let Some(field_name) =
        crate::value_custody::places::direct_self_field_member(program, expression)
        && let Some(data) = crate::value_custody::places::machine_attached_data(program, machine)
        && !data_declares_field(program, data, field_name)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` reads `self.{field_name}`, but data `{}` has no field \
             `{field_name}` (check the spelling of the field name)",
            machine.name.as_str(),
            state.name.as_str(),
            data.name.as_str()
        )));
    }
    // Unknown data-typed local/parameter member READ, plus nested `self` reads:
    // `task.continuation`, `self.o.inner.nonexistent` (final missing), and
    // `self.o.bogus.value` (intermediate missing) must not become silent ZII
    // reads. The walker reports only a provably-missing member on a
    // provably-plain container -- versioned containers (fields in wire version
    // blocks), contained-machine/owned-data roots, and non-data hops all skip.
    // The recursion below may revisit the same missing hop, so an identical
    // message is deduplicated rather than reported per level.
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Name(_)
    ) && let Some((container, member)) = crate::value_custody::places::first_unknown_nested_field(
        program,
        machine,
        Some(state),
        expression,
    ) {
        let message = format!(
            "machine `{}` state `{}` reads a nested member `{member}`, but data `{container}` \
             has no field `{member}` (check the spelling of the field name)",
            machine.name.as_str(),
            state.name.as_str(),
        );
        if !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(&message))
        {
            diagnostics.push(Diagnostic::error(message));
        }
    }
    // Member / index access on a PRIMITIVE receiver. A number or bool has no fields,
    // no `.len`, and is not indexable, so `x.field` / `x.len` / `x[0]` on an `i32`
    // local silently reads a ZII 0 -- reject any such access. `String` is the one
    // exception, and only for MEMBER access: text carries a `.len` view, so `s.len`
    // is legal. An UNRESOLVED receiver or a struct / array / slice receiver is
    // left alone (those accesses are separate).
    let primitive_access = match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => Some((
            member.receiver,
            format!("member `{}`", member.member.as_str()),
        )),
        ExpressionNode::Indexed(indexed) => Some((indexed.collection, "an index".to_owned())),
        _ => None,
    };
    if let Some((receiver, access)) = primitive_access
        && let Some(receiver_type) = crate::value_custody::places::declared_place_type(
            program,
            machine,
            Some(state),
            receiver,
        )
        && let Some(primitive) = program.primitive_type_reference(receiver_type)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` accesses {access} of a `{}` value, but a primitive \
             scalar has no members or elements",
            machine.name.as_str(),
            state.name.as_str(),
            primitive.name(),
        )));
    }
    // Unknown BARE NAME: a SINGLE-segment name (`undeclared_var`, not `self.x` or
    // `Type::Case`) that resolves to nothing is otherwise silently accepted and reads
    // as 0/garbage. Reject it when it is none of the legal bare-name forms. The scope
    // is the current state; retained foreign binding identities cannot be
    // replaced by a coincidentally matching spelling. Qualified paths check
    // their root too, so member access cannot hide an implicit capture.
    if let Some(path) = program.expression_table.name_path_node(expression)
        && let Some(first) = program
            .expression_table
            .name_path_members(path.members)
            .first()
    {
        let name = first.as_str();
        let root = if path.head_symbol.is_valid() {
            path.head_symbol
        } else {
            path.symbol
        };
        let bare_identity_matches = program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
            || !path.head_symbol.is_valid()
            || path.head_symbol == path.symbol;
        if language_semantics::const_value::boolean_literal_spelling(name).is_none()
            && (!bare_identity_matches
                || !crate::value_custody::locals::state_value_root_is_known(
                    program,
                    machine,
                    state,
                    writable_roots.statements,
                    symbols,
                    root,
                    name,
                ))
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` uses `{name}`, which is not a declared local, parameter, field, or type in this state (check the spelling)",
                machine.name.as_str(), state.name.as_str(),
            )));
        }
        // Static paths cannot continue through a runtime binding. Dot fields
        // remain Member expressions; synthesized runtime Names have one member.
        // Reuse the exact root reconstructed above, including a retained leaf
        // when the head is absent, so malformed custody cannot hide the binding.
        if program
            .expression_table
            .name_path_members(path.members)
            .len()
            > 1
            && crate::value_custody::locals::state_binding_type(
                program,
                state,
                writable_roots.statements,
                root,
            )
            .is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` uses a static path through runtime binding `{name}`; \
                 use `.` to select a field or callable through a value",
                machine.name.as_str(),
                state.name.as_str(),
            )));
        }
    }
    // Unknown TWO-segment path (`Type::Case`, `Trait::NAME`): the sibling of the
    // bare-name check above. A LEGITIMATE qualified case (`Signal::Green`) or a
    // substituted const resolves to a valid symbol before this stage; an
    // unresolved `Scope::tail` -- a bogus case (`Signal::Blue`) or a typo -- keeps BOTH the
    // head and leaf symbols invalid and would silently read 0 (ZII) in value
    // position (native AND interpreter agree, so the differential cannot catch
    // it). Reject exactly that shape: both symbols unresolved.
    if let Some(path) = program.expression_table.name_path_node(expression)
        && let [scope, tail] = program.expression_table.name_path_members(path.members)
        && !path.head_symbol.is_valid()
        && !path.symbol.is_valid()
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` uses `{}::{}`, which resolves to no case or \
             constant -- it would silently read 0 (ZII). Check the spelling and \
             ensure the declaration is imported",
            machine.name.as_str(),
            state.name.as_str(),
            scope.as_str(),
            tail.as_str(),
        )));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            crate::value_custody::expression_types::validate_match_dispatch(
                program,
                machine,
                state,
                expression,
                dispatch,
                diagnostics,
            );
            let environments =
                crate::proof_contracts::arithmetic_domains::dispatch_arm_environments(
                    program,
                    machine,
                    Some(state),
                    expression,
                    dispatch,
                    value_environment,
                );
            let mut scan = |value, executes, environment: &ValueEnvironment| {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    symbols,
                    writable_roots,
                    environment,
                    value,
                    executes,
                    boundary_operator_applications,
                    diagnostics,
                )
            };
            scan(dispatch.subject, executes, value_environment);
            let mut remaining_executes = executes;
            let mut boolean_values = [false; 2];
            for (arm, environment) in program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .zip(&environments)
            {
                let selected = match arm.pattern {
                    typed_trees::expression::MatchPattern::Wildcard => Some(true),
                    typed_trees::expression::MatchPattern::Value(pattern) => {
                        scan(pattern, remaining_executes, value_environment);
                        if let Some(value) = program.expression_table.boolean_node(pattern) {
                            boolean_values[usize::from(*value)] = true;
                        }
                        match (
                            program.expression_table.expression(dispatch.subject),
                            program.expression_table.expression(pattern),
                        ) {
                            (
                                ExpressionNode::Boolean(subject),
                                ExpressionNode::Boolean(pattern),
                            ) => Some(subject == pattern),
                            _ => {
                                crate::value_custody::literals::evaluate_anonymous_numeric_equality(
                                    program,
                                    dispatch.subject,
                                    pattern,
                                    |expression| {
                                        crate::has_anonymous_operator_meaning(program, expression)
                                    },
                                )
                            }
                        }
                    }
                };
                scan(
                    arm.value,
                    remaining_executes && selected != Some(false),
                    environment,
                );
                if selected == Some(true) || boolean_values == [true, true] {
                    remaining_executes = false;
                }
            }
        }
        ExpressionNode::Atomic(_) => {
            let Some(operands) =
                crate::value_custody::atomic_operations::atomic_value_operands(program, expression)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` has an invalid atomic operation carrier: cannot identify its evaluated operands",
                    machine.name.as_str(),
                    state.name.as_str(),
                )));
                return;
            };
            for operand in operands {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    symbols,
                    writable_roots,
                    value_environment,
                    operand,
                    executes,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Call(call) => {
            let call = call.clone();
            if executes
                && call.exact_coercion_target.is_valid()
                && let [operand] = program.expression_table.expression_handles(call.arguments)
            {
                crate::value_custody::expression_types::validate_exact_float_coercion(
                    program,
                    machine,
                    state,
                    *operand,
                    call.exact_coercion_target,
                    value_environment,
                    diagnostics,
                );
            }
            if crate::proof_contracts::proof_embeddings::is_exact_embed_call(program, &call) {
                crate::proof_contracts::arithmetic_domains::validate_total_specification_arithmetic(
                    program,
                    machine,
                    Some(state),
                    expression,
                    value_environment,
                    "`embed` source in a proof body",
                    diagnostics,
                );
            }
            validate_expression_call_bounds(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                expression,
                &call,
                statement_root,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
            // Recurse into the receiver and arguments (nested calls).
            if call.receiver.is_valid()
                && !is_declaration_namespace(program, call.receiver)
                && !crate::declarations::operators::namespaces::expression_call_has_operator_namespace(program, &call)
                && !crate::machine_calls::calls::generic_requirement::expression_conformance_receiver(
                    program,
                    machine,
                    call.receiver,
                )
                .is_valid()
            {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    symbols,
                    writable_roots,
                    value_environment,
                    call.receiver,
                    executes,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    symbols,
                    writable_roots,
                    value_environment,
                    *argument,
                    executes,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            // Every binary-operand TYPE check (operator applied to operands it is
            // not defined for) lives behind one dispatcher.
            crate::value_custody::expression_types::validate_binary_operand_types(
                program,
                machine,
                Some(state),
                expression,
                binary.operator,
                binary.left,
                binary.right,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                binary.left,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
            let right_environment =
                crate::proof_contracts::arithmetic_domains::short_circuit_operand_environment(
                    program,
                    machine,
                    Some(state),
                    expression,
                    value_environment,
                );
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                right_environment.as_ref().unwrap_or(value_environment),
                binary.right,
                executes
                    && !matches!(
                        (
                            binary.operator,
                            program.expression_table.expression(binary.left)
                        ),
                        (BinaryOperator::And, ExpressionNode::Boolean(false))
                            | (BinaryOperator::Or, ExpressionNode::Boolean(true))
                    ),
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => {
            crate::value_custody::locals::StateValueScope {
                program,
                machine,
                state,
                symbols,
                prior_statements: writable_roots.statements,
                context: "cast type bound",
            }
            .type_reference(cast.target_type, diagnostics);
            // All cast TARGET/SOURCE type validation lives behind one dispatcher.
            crate::value_custody::expression_types::validate_cast_types(
                program,
                machine,
                Some(state),
                cast,
                value_environment,
                diagnostics,
            );
            if executes {
                crate::proof_contracts::arithmetic_domains::validate_range_cast_at_use(
                    program,
                    machine,
                    state,
                    expression,
                    value_environment,
                    diagnostics,
                );
            }
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                cast.value,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Indexed(indexed) => {
            crate::value_custody::expression_types::report_non_integer_index(
                program,
                machine,
                state,
                indexed,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                indexed.collection,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                indexed.index,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Member(member) => {
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                member.receiver,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Borrow(inner) => {
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                inner.target,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => {
            match unary.operator {
                typed_trees::expression::UnaryOperator::BitwiseNot => {
                    crate::value_custody::expression_types::report_non_integer_bitwise_not(
                        program,
                        machine,
                        Some(state),
                        unary.operand,
                        diagnostics,
                    );
                }
                typed_trees::expression::UnaryOperator::LogicalNot => {
                    crate::value_custody::expression_types::report_non_bool_logical_not(
                        program,
                        machine,
                        Some(state),
                        unary.operand,
                        diagnostics,
                    );
                }
            }
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                unary.operand,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let elements = *elements;
            for element in program.expression_table.expression_handles(elements) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    symbols,
                    writable_roots,
                    value_environment,
                    *element,
                    executes,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            let fields = literal.fields;
            for field in program.expression_table.struct_fields(fields) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    symbols,
                    writable_roots,
                    value_environment,
                    field.value,
                    executes,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Range(range) => {
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                range.start,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                symbols,
                writable_roots,
                value_environment,
                range.end,
                executes,
                boundary_operator_applications,
                diagnostics,
            );
        }
        typed_trees::executable_leaf_expression_pattern!() => {}
        ExpressionNode::ZeroValue(type_reference) => {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` uses `zero_value<{}>()` in executable code; \
                 `zero_value<T>()` is a proof-only representation observation and may \
                 appear only in contracts",
                machine.name.as_str(),
                state.name.as_str(),
                program.display_type_reference(*type_reference),
            )));
        }
    }
}
