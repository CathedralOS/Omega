use psi_diagnostics::Diagnostic;
use psi_language_semantics::{MachineSupplyMode, ReferenceAccess};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};

#[derive(Clone)]
struct WriteOnlyRoot {
    symbol: SymbolHandle,
    name: String,
    referee: TypeReferenceHandle,
}

pub(crate) fn validate_checked_write_only_slice(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let roots = program
                .state_parameters(state)
                .iter()
                .filter_map(|parameter| {
                    let TypeReferenceNode::Reference {
                        referee,
                        access: ReferenceAccess::WriteOnly,
                        ..
                    } = program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    else {
                        return None;
                    };
                    Some(WriteOnlyRoot {
                        symbol: parameter.symbol,
                        name: parameter.name.as_str().to_owned(),
                        referee: *referee,
                    })
                })
                .collect::<Vec<_>>();
            if roots.is_empty() {
                continue;
            }

            if !matches!(machine.supply_mode, MachineSupplyMode::CheckedBody) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` declares `&write`, but the current milestone proves non-observation only for checked Omega bodies; boundary, accepted, requirement, and external-provider declarations require an admitted write-only boundary claim",
                    machine.name,
                    state.name,
                )));
            }

            for root in &roots {
                if !is_supported_checked_referee(program, root.referee) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` parameter `{}` uses `&write` with `{}`; the current checked slice supports unrestricted primitive scalars and fixed byte arrays",
                        machine.name,
                        state.name,
                        root.name,
                        program.display_type_reference_with_constraints(root.referee),
                    )));
                }
            }

            for statement in program.statement_table.statements(state.statement_nodes) {
                validate_statement(
                    program,
                    machine.name.as_str(),
                    state.name.as_str(),
                    statement,
                    &roots,
                    diagnostics,
                );
            }
        }
    }
}

fn is_unrestricted_scalar(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    let TypeReferenceNode::Named { name, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    !name.as_str().starts_with("Atomic")
        && program.primitive_type_reference(type_reference).is_some()
}

fn fixed_byte_array_length(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    let TypeReferenceNode::FixedArray {
        element_type,
        length: FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    (is_unrestricted_scalar(program, *element_type)
        && program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8))
    .then_some(*length)
}

fn is_supported_checked_referee(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    is_unrestricted_scalar(program, type_reference)
        || fixed_byte_array_length(program, type_reference).is_some()
}

fn validate_statement(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    statement: &StatementNode,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::AssemblyFact(fact) => {
            validate_expression(program, machine, state, fact.expression, roots, diagnostics)
        }
        StatementNode::Assignment(assignment) => {
            match direct_write_only_root(program, assignment.target, roots) {
                Some(_) => {}
                None if fixed_byte_element_assignment_target(program, assignment.target, roots)
                    .is_some() => {}
                None if expression_mentions_write_only_root(program, assignment.target, roots) => {
                    diagnose_unsupported_write_only_assignment_target(
                        program,
                        machine,
                        state,
                        assignment.target,
                        roots,
                        diagnostics,
                    );
                }
                None => validate_expression(
                    program,
                    machine,
                    state,
                    assignment.target,
                    roots,
                    diagnostics,
                ),
            }
            validate_expression(
                program,
                machine,
                state,
                assignment.value,
                roots,
                diagnostics,
            );
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                validate_expression(program, machine, state, *argument, roots, diagnostics);
            }
        }
        StatementNode::Expression(expression) => {
            validate_expression(program, machine, state, *expression, roots, diagnostics)
        }
        StatementNode::LocalData(local) => validate_expression(
            program,
            machine,
            state,
            local.initial_value,
            roots,
            diagnostics,
        ),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                validate_expression(program, machine, state, guard, roots, diagnostics);
            }
            validate_transition_target(
                program,
                machine,
                state,
                transition.target,
                roots,
                diagnostics,
            );
            validate_transition_target(
                program,
                machine,
                state,
                transition.continuation,
                roots,
                diagnostics,
            );
        }
    }
}

fn fixed_byte_element_assignment_target<'a>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &'a [WriteOnlyRoot],
) -> Option<&'a WriteOnlyRoot> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let root = direct_write_only_root(program, indexed.collection, roots)?;
    let length = fixed_byte_array_length(program, root.referee)?;
    let ExpressionNode::Integer(index) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    let index = usize::try_from(index.value_i64()?).ok()?;
    (index < length).then_some(root)
}

fn diagnose_unsupported_write_only_assignment_target(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
        && let Some(root) = direct_write_only_root(program, indexed.collection, roots)
        && fixed_byte_array_length(program, root.referee).is_some()
    {
        let detail = match program.expression_table.expression(indexed.index) {
            ExpressionNode::Range(_) => "range projection is not implemented",
            ExpressionNode::Integer(index) => match index.value_i64() {
                Some(value) if value < 0 => "the index must be non-negative",
                Some(_) => "the literal index is outside the fixed byte array",
                None => "the literal index is outside the supported index range",
            },
            _ => "the index must be a literal for this milestone",
        };
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine}` state `{state}` writes through unsupported projection of write-only byte array `{}`; {detail}; only whole-array replacement and statically in-bounds literal element replacement are accepted",
            root.name,
        )));
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "machine `{machine}` state `{state}` projects or observes a write-only parameter in an assignment target; the current `&write` slice permits whole-root replacement and statically in-bounds literal element replacement for fixed byte arrays"
    )));
}

fn validate_transition_target(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    target: psi_typed_trees::statement::TransitionTargetHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                validate_expression(program, machine, state, *argument, roots, diagnostics);
            }
        }
        TransitionTargetNode::Value(value) => {
            validate_expression(program, machine, state, *value, roots, diagnostics)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn validate_expression(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if let Some(root) = roots.iter().find(|root| path.head_symbol == root.symbol) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine}` state `{state}` reads write-only parameter `{}`; `&write` permits replacement or exact `&write` forwarding, never observation",
                    root.name,
                )));
            }
        }
        ExpressionNode::Borrow(borrow) => match borrow.access {
            ReferenceAccess::WriteOnly => {
                if !is_direct_name(program, borrow.target) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine}` state `{state}` forms `&write` from a projection or computed expression; the current checked slice supports explicit attenuation of a whole scalar root only"
                    )));
                }
            }
            ReferenceAccess::Mutable => {
                if let Some(root) = mentioned_write_only_root(program, borrow.target, roots) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine}` state `{state}` widens write-only parameter `{}` to `&mut`; forward it explicitly as `&write {}` instead",
                        root.name, root.name,
                    )));
                } else {
                    validate_expression(program, machine, state, borrow.target, roots, diagnostics);
                }
            }
            ReferenceAccess::Shared => {
                validate_expression(program, machine, state, borrow.target, roots, diagnostics)
            }
        },
        ExpressionNode::Member(member) => {
            if let Some(root) = mentioned_write_only_root(program, member.receiver, roots) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine}` state `{state}` projects field `{}` from write-only parameter `{}`; write-only projection is not implemented in the whole-scalar slice",
                    member.member, root.name,
                )));
            } else {
                validate_expression(program, machine, state, member.receiver, roots, diagnostics);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(root) = mentioned_write_only_root(program, indexed.collection, roots) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine}` state `{state}` reads through index projection of write-only parameter `{}`; `&write` permits fixed byte-element replacement but never observation",
                    root.name,
                )));
            } else {
                validate_expression(
                    program,
                    machine,
                    state,
                    indexed.collection,
                    roots,
                    diagnostics,
                );
            }
            validate_expression(program, machine, state, indexed.index, roots, diagnostics);
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                validate_expression(program, machine, state, *value, roots, diagnostics);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            validate_expression(program, machine, state, atomic.value, roots, diagnostics);
            validate_expression(program, machine, state, atomic.result, roots, diagnostics);
        }
        ExpressionNode::Binary(binary) => {
            validate_expression(program, machine, state, binary.left, roots, diagnostics);
            validate_expression(program, machine, state, binary.right, roots, diagnostics);
        }
        ExpressionNode::Cast(cast) => {
            validate_expression(program, machine, state, cast.value, roots, diagnostics)
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                validate_expression(program, machine, state, call.receiver, roots, diagnostics);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                validate_expression(program, machine, state, *argument, roots, diagnostics);
            }
        }
        ExpressionNode::Range(range) => {
            validate_expression(program, machine, state, range.start, roots, diagnostics);
            validate_expression(program, machine, state, range.end, roots, diagnostics);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                validate_expression(program, machine, state, field.value, roots, diagnostics);
            }
        }
        ExpressionNode::Unary(unary) => {
            validate_expression(program, machine, state, unary.operand, roots, diagnostics)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn is_direct_name(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(path)
            if program.expression_table.name_path_members(path.members).len() == 1
    )
}

fn direct_write_only_root<'a>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &'a [WriteOnlyRoot],
) -> Option<&'a WriteOnlyRoot> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1)
        .then(|| roots.iter().find(|root| path.head_symbol == root.symbol))
        .flatten()
}

fn mentioned_write_only_root<'a>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &'a [WriteOnlyRoot],
) -> Option<&'a WriteOnlyRoot> {
    roots
        .iter()
        .find(|root| expression_mentions_symbol(program, expression, root.symbol))
}

fn expression_mentions_write_only_root(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> bool {
    mentioned_write_only_root(program, expression, roots).is_some()
}

fn expression_mentions_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => path.head_symbol == symbol,
        ExpressionNode::Borrow(value) => expression_mentions_symbol(program, value.target, symbol),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_mentions_symbol(program, *value, symbol)),
        ExpressionNode::Atomic(atomic) => {
            expression_mentions_symbol(program, atomic.value, symbol)
                || expression_mentions_symbol(program, atomic.result, symbol)
        }
        ExpressionNode::Binary(binary) => {
            expression_mentions_symbol(program, binary.left, symbol)
                || expression_mentions_symbol(program, binary.right, symbol)
        }
        ExpressionNode::Cast(cast) => expression_mentions_symbol(program, cast.value, symbol),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && expression_mentions_symbol(program, call.receiver, symbol))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_mentions_symbol(program, *argument, symbol))
        }
        ExpressionNode::Indexed(indexed) => {
            expression_mentions_symbol(program, indexed.collection, symbol)
                || expression_mentions_symbol(program, indexed.index, symbol)
        }
        ExpressionNode::Member(member) => {
            expression_mentions_symbol(program, member.receiver, symbol)
        }
        ExpressionNode::Range(range) => {
            expression_mentions_symbol(program, range.start, symbol)
                || expression_mentions_symbol(program, range.end, symbol)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_mentions_symbol(program, field.value, symbol)),
        ExpressionNode::Unary(unary) => expression_mentions_symbol(program, unary.operand, symbol),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}
