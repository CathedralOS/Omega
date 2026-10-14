//! Fail-closed fences for value calls whose result cannot yet be realized.
//!
//! These checks are separate from target resolution and argument validation:
//! they reject call shapes that otherwise silently bind zero or read an
//! unmaterialized nested result at runtime.
//! Assignments are not fenced here: their common operand planner evaluates
//! nested calls in order, and a missing plan fails product construction. A
//! second destination/signature whitelist would reject supported compositions
//! without proving any additional semantic obligation.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::CollectionViewOperation;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetNode};

mod boundary_return;

/// Result operations own the outer call while their operands use the shared
/// scalar evaluator. Each caller family retains its existing source topology.
pub fn result_initializer_call_is_admitted(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    // Completion owns its call directly, while a local initializer may instead
    // belong to the scalar graph's whole-call computation. Do not reclassify
    // those existing local computations merely because final calls are supported.
    if let [state] = program.machine_states(machine)
        && program
            .primitive_type_reference(state.return_type)
            .is_some()
        && matches!(program.statement_table.statements(state.statement_nodes).last(),
            Some(StatementNode::Expression(expression)) if *expression == value)
        && program.expression_table.expression_is_valid(value)
    {
        return initializer_target_is_admitted(program, machine, value, state.return_type, true);
    }
    unit_result_initializer_call_is_admitted(program, machine, value)
        || boundary_return::is_admitted(program, machine, value)
}

/// Local calls retain ordered operands in either their computation or structural
/// result operation. Admission follows the resolved receiving type, not an
/// incidental combination of reference leaves, projections and nested helpers.
pub(crate) fn report_nested_call_in_local_initializer(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if local_initializer_has_operand_evaluation(program, machine, value)
        || result_initializer_call_is_admitted(program, machine, value)
    {
        // This exempts a destination, not its semantics. Ordinary call checks
        // still validate every argument; unsupported computation nodes or call
        // custody fail lowering before any Terminal artifact can be published.
        return;
    }
    report_nested_call_in_bound_value_call(program, machine, state_name, value, diagnostics);
}

/// Result destinations handled by the ordinary checked statement sequence.
/// Typing and ordinary call validation still own result, argument and contract
/// compatibility; this predicate does not supply those semantic judgments.
/// Scalar and owned structural results use the authored statement sequence.
/// Structural ownership and cleanup are checked by that sequence's producer.
pub fn unit_result_initializer_call_is_admitted(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    let mut states = program.machine_states(machine).iter().filter(|state| {
        program.statement_table.statements(state.statement_nodes).iter().any(|statement| {
            matches!(statement, StatementNode::LocalData(local) if local.initial_value == value)
                || matches!(statement, StatementNode::Expression(expression) if *expression == value)
        })
    });
    let Some(state) = states.next() else {
        return false;
    };
    if states.next().is_some() || !program.expression_table.expression_is_valid(value) {
        return false;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    // A final array call has the state's result destination, not a synthetic
    // local. Its operands still use the same checked computation evaluator.
    if matches!(statements.last(), Some(StatementNode::Expression(expression)) if *expression == value)
        && crate::is_closed_primitive_array_type(program, state.return_type)
        && program.machine_states(machine).len() == 1
    {
        return initializer_target_is_admitted(program, machine, value, state.return_type, true);
    }
    let mut initializers =
        statements
            .iter()
            .enumerate()
            .filter_map(|(statement_index, statement)| match statement {
                StatementNode::LocalData(local) if local.initial_value == value => {
                    Some((statement_index, local))
                }
                _ => None,
            });
    let Some((statement_index, local)) = initializers.next() else {
        return false;
    };
    let has_ordinary_structural_initializer =
        ordinary_structural_initializer(program, local.initial_value, local.type_reference);
    let has_boundary_scalar_initializer = program
        .primitive_type_reference(local.type_reference)
        .is_some()
        && initializer_target_is_admitted(
            program,
            machine,
            local.initial_value,
            local.type_reference,
            false,
        );
    if initializers.next().is_some()
        // A structural local belongs to the statement sequence regardless of
        // what the surrounding machine eventually returns. Scalar locals in
        // scalar-return bodies retain their whole-call computation owner.
        || (!has_ordinary_structural_initializer
            && !(unit_type(program, state.return_type)
                || ordinary_structural_result_type(program, state.return_type)))
        // Ordinary scalar helpers in state graphs keep their whole-call
        // computation owner. Bodyless boundaries instead retain their result
        // operation, independently of the surrounding state's successors.
        || (program.machine_states(machine).len() != 1
            && !has_ordinary_structural_initializer
            && !has_boundary_scalar_initializer)
        // A mutable owned structural result keeps the same receiving storage
        // regardless of the callee's supply. Primitive initialization and
        // linear custody retain their separate receiving contracts.
        || (local.is_mutable
            && (program.type_multiplicity(local.type_reference)
                == language_semantics::Multiplicity::Linear
                || program.primitive_type_reference(local.type_reference).is_some()))
        || (statement_index != 0
            && program
                .primitive_type_reference(local.type_reference)
                .is_none()
            && !has_ordinary_structural_initializer
            && !boundary_structural_initializer(program, machine, local))
    {
        return false;
    }
    initializer_target_is_admitted(
        program,
        machine,
        local.initial_value,
        local.type_reference,
        true,
    )
}

fn boundary_structural_initializer(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    local: &typed_trees::statement::TableLocalData,
) -> bool {
    // The existing exact target resolver also covers boundary-trait requirements
    // and this caller's nominal machine parameters. This is not a new receiver,
    // qualification, reference, or linear-result realization route.
    program.type_multiplicity(local.type_reference) != language_semantics::Multiplicity::Linear
        && crate::has_contents(
            program,
            local.type_reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::PlainOwned,
        )
        && initializer_target_is_admitted(
            program,
            machine,
            local.initial_value,
            local.type_reference,
            false,
        )
}

fn ordinary_structural_initializer(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
    result_type: typed_trees::types::TypeReferenceHandle,
) -> bool {
    if !ordinary_structural_result_type(program, result_type) {
        return false;
    }
    let Some(call) = program.expression_table.call_node(value) else {
        return false;
    };
    program
        .machine_holding_state(call.target_symbol)
        .is_some_and(|owner| {
            owner.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && program.machine_states(owner).first().is_some_and(|target| {
                    target.symbol == call.target_symbol
                        && !unit_type(program, target.return_type)
                        && program
                            .primitive_type_reference(target.return_type)
                            .is_none()
                        && ordinary_structural_result_type(program, target.return_type)
                })
        })
}

/// The same statement sequence owns these results in Unit and structural-return
/// bodies. Classifying a linear destination does not establish its claims:
/// ordinary call/return custody must still reconstruct the exact input lineage.
/// Reference destinations reject here; boundary results retain their existing
/// target admission and separate custody validation.
fn ordinary_structural_result_type(
    program: &dyn crate::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_multiplicity(reference) {
        language_semantics::Multiplicity::Linear => {
            crate::structural_result_qualifications(program, reference).is_ok()
        }
        language_semantics::Multiplicity::Affine => crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::PlainOwned,
        ),
        language_semantics::Multiplicity::Unrestricted => {
            crate::is_closed_primitive_array_type(program, reference)
        }
    }
}

fn initializer_target_is_admitted(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    value: ExpressionHandle,
    result_type: typed_trees::types::TypeReferenceHandle,
    allow_ordinary: bool,
) -> bool {
    let Some(call) = program.expression_table.call_node(value) else {
        return false;
    };
    if !call.target_symbol.is_valid()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
    {
        return false;
    }
    // The target entry is unique: state spans are disjoint, so at most one
    // machine can hold `call.target_symbol` as its first state.
    let target_pair = program
        .machine_holding_state(call.target_symbol)
        .and_then(|owner| {
            program
                .machine_states(owner)
                .first()
                .filter(|entry| entry.symbol == call.target_symbol)
                .map(|entry| (owner, entry))
        });
    if let Some((owner, target)) = target_pair {
        return program.call_has_no_runtime_receiver(call, owner, target)
            && !unit_type(program, target.return_type)
            && (allow_ordinary
                || program.primitive_type_reference(target.return_type)
                    == program.primitive_type_reference(result_type))
            && (owner.supply_mode.is_boundary_declaration()
                || (allow_ordinary
                    && ((program.primitive_type_reference(result_type).is_some()
                        && program
                            .primitive_type_reference(target.return_type)
                            .is_some())
                        || (program.primitive_type_reference(result_type).is_none()
                            && ordinary_structural_initializer(program, value, result_type)))));
    }

    let selected_parameter = program.machine_parameter_signature(call.target_symbol);
    let requirement = match selected_parameter {
        Some((owner, signature)) if owner.symbol == machine.symbol => signature.symbol,
        Some(_) => return false,
        None => call.target_symbol,
    };
    let mut requirements = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == requirement)
                .map(move |signature| (definition, signature))
        });
    let Some((definition, signature)) = requirements.next() else {
        return false;
    };
    if requirements.next().is_some()
        || unit_type(program, signature.return_type)
        || (!allow_ordinary
            && program.primitive_type_reference(signature.return_type)
                != program.primitive_type_reference(result_type))
    {
        return false;
    }
    if selected_parameter.is_some() {
        return !call.receiver.is_valid();
    }
    program.expression_table.expression_is_valid(call.receiver)
        && matches!(program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path) if path.symbol == definition.symbol)
}

fn unit_type(
    program: &dyn crate::CheckingInput,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type
            }
            typed_trees::types::TypeReferenceNode::Unit => return true,
            _ => return false,
        }
    }
}

/// Every resolved local call evaluates operands before binding its result.
/// Scalar computations and structural result operations share ordered operand
/// preparation. Reference custody remains attached to those operations; a
/// source-shape recognizer cannot establish it. This only retires the legacy
/// nested-result fence: type, effect and custody checks still run, and missing
/// operand plans fail product construction rather than supplying a value.
/// A receiving range refinement changes its proof obligation, not the operand
/// evaluation owner; requiring identical qualified types would reject a call
/// whose ordinary ensures establish the narrower destination.
fn local_initializer_has_operand_evaluation(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(value) {
        return false;
    }
    let Some(call) = program.expression_table.call_node(value) else {
        return false;
    };
    if !call.carries_only_positional_arguments() {
        return false;
    }
    // An explicit implementation map retains its selection syntax, but that
    // does not change the local's ordered operand-evaluation owner. Actual
    // receiver, selection and custody admission remain ordinary call checks.
    program.machine_states(machine).iter().any(|state| {
        if crate::expression_result_type_reference(program, machine, state, value).is_none() {
            return false;
        }
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(statement, StatementNode::LocalData(local)
                if local.initial_value == value)
            })
    })
}

/// Destinations without ordinary computation or result-operand evaluation must
/// retain this fence: the legacy value sink cannot materialize an inner callee's
/// result in the outer argument frame. Supported local computations are admitted
/// above and sequence nested calls explicitly; statement-call operands have their
/// own ordered materialization path. This check only handles the remaining local
/// and assignment destinations, not every bound nested call.
pub(crate) fn report_nested_call_in_bound_value_call(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() {
        return;
    }
    let Some(call) = program.expression_table.call_node(value) else {
        return;
    };
    // Collection views retain their own operand materialization. Ordinary
    // machine calls use the receiving operation admitted above.
    if call_is_collection_view(call) {
        return;
    }
    for argument in program.expression_table.expression_handles(call.arguments) {
        if let Some(inner) = first_non_builtin_call(program, *argument) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{state_name}`: a value-call argument cannot itself \
                 be a machine call yet (`{inner}(..)` nested in `{}(..)` would read a \
                 garbage result) -- bind the inner call to a local first, then pass \
                 the local.",
                machine.name,
                call.target.as_str(),
            )));
            return;
        }
    }
}

/// True when `value` is a bodyless scalar boundary application whose every
/// nested non-builtin call is also one. Such a tree composes as one statement
/// result operation over ordered nested application operands; a checked-body
/// or structural call anywhere inside still needs the fence.
pub fn nested_boundary_applications_compose(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
) -> bool {
    fn scalar_parameters(
        program: &dyn crate::CheckingInput,
        parameters: &[typed_trees::signature::StateParameter],
    ) -> bool {
        parameters.iter().all(|parameter| {
            !parameter.is_const
                && !parameter.is_mutable
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
        })
    }
    fn scalar_boundary_application(
        program: &dyn crate::CheckingInput,
        call: &typed_trees::expression::TableCallExpression,
    ) -> bool {
        // A nongeneric legacy named operator (`F64::from_i32`): its qualifier
        // is not an operand.
        if let Some(operator) = program
            .operators()
            .iter()
            .find(|operator| operator.symbol == call.target_symbol && operator.is_boundary)
        {
            return program.operator_type_parameters(operator).is_empty()
                && program
                    .primitive_type_reference(operator.return_type)
                    .is_some()
                && scalar_parameters(program, program.operator_parameters(operator));
        }
        let Some(owner) = program.machine_holding_state(call.target_symbol) else {
            return false;
        };
        let Some(entry) = program.machine_states(owner).first() else {
            return false;
        };
        owner.supply_mode.is_boundary_declaration()
            && !owner.body_is_present
            && entry.symbol == call.target_symbol
            && !call.receiver.is_valid()
            && program
                .primitive_type_reference(entry.return_type)
                .is_some()
            && scalar_parameters(program, program.state_parameters(entry))
    }
    fn composes(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
        if !expression.is_valid() {
            return true;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                (call_is_collection_view(call) || scalar_boundary_application(program, call))
                    && program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .all(|argument| composes(program, *argument))
            }
            ExpressionNode::Atomic(atomic) => composes(program, atomic.value),
            ExpressionNode::Binary(binary) => {
                composes(program, binary.left) && composes(program, binary.right)
            }
            ExpressionNode::Unary(unary) => composes(program, unary.operand),
            ExpressionNode::Cast(cast) => composes(program, cast.value),
            ExpressionNode::Borrow(inner) => composes(program, inner.target),
            ExpressionNode::Indexed(indexed) => {
                composes(program, indexed.collection) && composes(program, indexed.index)
            }
            ExpressionNode::Member(member) => composes(program, member.receiver),
            _ => true,
        }
    }
    matches!(program.expression_table.expression(value), ExpressionNode::Call(call)
        if scalar_boundary_application(program, call))
        && composes(program, value)
}

/// Collection views have no callable symbol; their vocabulary applies only
/// when no authored declaration was selected. Other calls retain ordinary
/// call-result realization.
fn call_is_collection_view(call: &TableCallExpression) -> bool {
    !call.target_symbol.is_valid()
        && CollectionViewOperation::from_authored_spelling(call.target.as_str()).is_some()
}

/// The first NON-BUILTIN machine call nested anywhere inside `expression`
/// (its target name, for the diagnostic), or None. Collection view operations
/// compose in arguments and retain their own value route.
fn first_non_builtin_call(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<Identifier> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => first_non_builtin_call(program, atomic.value),
        ExpressionNode::Call(call) => {
            if !call_is_collection_view(call) {
                return Some(call.target.clone());
            }
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .find_map(|argument| first_non_builtin_call(program, *argument))
        }
        ExpressionNode::Binary(binary) => first_non_builtin_call(program, binary.left)
            .or_else(|| first_non_builtin_call(program, binary.right)),
        ExpressionNode::Unary(unary) => first_non_builtin_call(program, unary.operand),
        ExpressionNode::Cast(cast) => first_non_builtin_call(program, cast.value),
        ExpressionNode::Borrow(inner) => first_non_builtin_call(program, inner.target),
        ExpressionNode::Indexed(indexed) => first_non_builtin_call(program, indexed.collection)
            .or_else(|| first_non_builtin_call(program, indexed.index)),
        ExpressionNode::Member(member) => first_non_builtin_call(program, member.receiver),
        _ => None,
    }
}

/// A VOID callee in VALUE position used to compile and silently bind 0 (ZII)
/// -- and native/interp DIVERGED on the bound value. "Void" means: no declared
/// return type on the resolved state (the parser now lands `-> T` written
/// after the machine clauses too; it used to be silently dropped) AND no state
/// of the callee machine produces a value through a transition VALUE arm --
/// undeclared-return value machines (`transition r > 0 { true -> self.f(r-1)
/// false -> 0 }`, the termination-canary surface) stay callable.
pub(super) fn report_void_value_callee(
    program: &dyn crate::CheckingInput,
    callee_machine: &Machine,
    current_machine: &Machine,
    current_state: &State,
    callee_state: &State,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if callee_state.return_type.is_valid()
        || crate::machine_calls::calls::unit_statement_call_is_admitted(
            program,
            current_machine,
            current_state,
            expression,
        )
    {
        return;
    }
    let produces_value = program.machine_states(callee_machine).iter().any(|state| {
        state.return_type.is_valid()
            || program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    let Some(transition) = statement.as_transition() else {
                        return false;
                    };
                    transition.targets().any(|handle| {
                        matches!(
                            program.statement_table.transition_target(handle),
                            TransitionTargetNode::Value(_)
                        )
                    })
                })
    });
    if produces_value {
        return;
    }
    let Some(call) = program.expression_table.call_node(expression) else {
        return;
    };
    let callee_display = call.target.as_str();
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}`: `{callee_display}(..)` does not return a value but is \
         used in a VALUE position -- it would silently bind 0 (ZII) at runtime. Declare \
         a return type on the callee (`-> T`) or call it as a statement.",
        current_machine.name,
        current_state.name.as_str(),
    )));
}
