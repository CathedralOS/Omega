use psi_diagnostics::Diagnostic;
use psi_language_semantics::{MachineSupplyMode, ReferenceAccess};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
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
                        "machine `{}` state `{}` parameter `{}` uses `&write` with `{}`; the current checked slice supports unrestricted primitive scalars, fixed byte arrays, and non-generic invariant-free checked records",
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
        || write_only_record(program, type_reference).is_some()
}

/// The first aggregate rung is deliberately nominal and closed. It admits an
/// ordinary checked record only when its shape is known without substitution
/// and no authored default-domain fact can couple a field write to retained
/// content. Field eligibility is checked separately at the exact assignment
/// target, so a record may contain wider siblings without making them writable.
fn write_only_record<'program>(
    program: &'program TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&'program DataDefinition> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *symbol)?;
    (definition.supply_mode == psi_language_semantics::DataSupplyMode::CheckedShape
        && definition.lifetime_parameters.is_empty()
        && program.data_type_parameters(definition).is_empty()
        && definition.quotient.is_none()
        && definition.where_facts.is_empty()
        && !definition.zero_gated
        && DataDefinition::shape_kind_from_members(program.data_members(definition))
            == DataShapeKind::Record)
        .then_some(definition)
}

fn whole_root_replacement_is_supported(program: &TypedTrees, root: &WriteOnlyRoot) -> bool {
    is_unrestricted_scalar(program, root.referee)
        || fixed_byte_array_length(program, root.referee).is_some()
        || write_only_record(program, root.referee).is_some_and(|definition| {
            definition.properties.multiplicity == psi_language_semantics::Multiplicity::Unrestricted
        })
}

/// Recognize exactly `root.common_field`, where the root is an admitted plain
/// record and the displaced field is a relevant, unconstrained primitive.
/// This is a store-place judgment only: expression traversal still rejects
/// reading the same member, and a member receiver that is itself a projection
/// remains outside this rung.
fn write_only_record_field_assignment(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> bool {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return false;
    };
    if member.case_variant.is_some() {
        return false;
    }
    let Some(root) = direct_write_only_root(program, member.receiver, roots) else {
        return false;
    };
    let Some(definition) = write_only_record(program, root.referee) else {
        return false;
    };
    let Some(field) = program
        .data_members(definition)
        .iter()
        .find_map(|candidate| {
            let DataMember::Field(field) = candidate else {
                return None;
            };
            ((member.member_symbol.is_valid() && field.symbol == member.member_symbol)
                || (!member.member_symbol.is_valid()
                    && field.name.as_str() == member.member.as_str()))
            .then_some(field)
        })
    else {
        return false;
    };
    !field.relevance.is_erased() && is_unrestricted_scalar(program, field.type_reference)
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
            if let Some(root) = direct_write_only_root(program, assignment.target, roots) {
                if !whole_root_replacement_is_supported(program, root) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine}` state `{state}` replaces whole write-only record `{}`; whole-root replacement requires a freely discardable root, so replace one eligible direct primitive field or declare and prove an unrestricted record instead",
                        root.name,
                    )));
                }
                // An admitted whole-value replacement observes no prior
                // content. Record roots additionally satisfy the explicit
                // discardability check above.
            } else if write_only_record_field_assignment(program, assignment.target, roots) {
                // One content-independent common-field store. The exact field
                // place is retained by the ordinary checked mutation facts.
            } else if let Some(index) =
                write_only_byte_element_assignment_index(program, assignment.target, roots)
            {
                // The normal range checker separately proves a dynamic index
                // is in bounds. This gate owns only non-observation: the index
                // expression must not recover information from the write-only
                // referent.
                validate_expression(program, machine, state, index, roots, diagnostics);
            } else if expression_mentions_write_only_root(program, assignment.target, roots) {
                diagnose_unsupported_write_only_assignment_target(
                    program,
                    machine,
                    state,
                    assignment.target,
                    roots,
                    diagnostics,
                );
            } else {
                validate_expression(
                    program,
                    machine,
                    state,
                    assignment.target,
                    roots,
                    diagnostics,
                );
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

fn write_only_byte_element_assignment_index(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> Option<ExpressionHandle> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let root = direct_write_only_root(program, indexed.collection, roots)?;
    let length = fixed_byte_array_length(program, root.referee)?;
    match program.expression_table.expression(indexed.index) {
        ExpressionNode::Range(_) => None,
        ExpressionNode::Integer(index) => {
            let index = usize::try_from(index.value_i64()?).ok()?;
            (index < length).then_some(indexed.index)
        }
        _ => Some(indexed.index),
    }
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
            _ => "the index expression is not an admissible byte-element place",
        };
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine}` state `{state}` writes through unsupported projection of write-only byte array `{}`; {detail}; whole-array replacement and proven-in-bounds element replacement are accepted",
            root.name,
        )));
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "machine `{machine}` state `{state}` writes through an unsupported write-only projection; accepted partial stores are a direct common field of a non-generic invariant-free record when the displaced field is an unconstrained unrestricted primitive, or a proven-in-bounds element of a fixed byte array; nested, sum-payload, qualified, invariant-dependent, range, take, swap, and read-modify-write operations remain rejected"
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
                        "machine `{machine}` state `{state}` forms `&write` from a projection or computed expression; the current checked slice supports explicit attenuation of a whole parameter only"
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
                    "machine `{machine}` state `{state}` reads field `{}` from write-only parameter `{}`; a direct eligible record field may be replaced as an assignment target, but write-only projection never grants observation",
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
