//! Runtime scalar observations require the original receiver, not an attachment.
use super::super::CheckedScalarExpression;
use super::{CheckFacts, TypedTrees};
use crate::execution::terminal_unit::{is_reference, structural_parameter_candidate};
use checked_trees::{CheckedBooleanExpression, CheckedScalarComputationKind};

pub(in crate::execution::terminal_unit) fn reads_receiver(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
) -> bool {
    let Some(position) = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.is_self && is_reference(program, parameter.type_reference))
    else {
        return false;
    };
    // Structural argument plans address the dense structural namespace, so a
    // `Place` subject rooted at the receiver carries the count of structural
    // candidates ahead of it, not the authored position.
    let Ok(receiver_source) = u32::try_from(
        program
            .state_parameters(state)
            .iter()
            .take(position)
            .filter(|parameter| structural_parameter_candidate(program, parameter))
            .count(),
    ) else {
        return false;
    };
    let Ok(position) = u32::try_from(position) else {
        return false;
    };
    if facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .any(|expression| {
            expression.state == state.symbol && scalar_reads(&expression.expression, position)
        })
    {
        return true;
    }
    let computations = &facts.values.scalar_computations;
    let mut pending = computations
        .roots
        .iter()
        .filter(|(_, root)| root.state == state.symbol)
        .map(|(_, root)| root.root)
        .collect::<Vec<_>>();
    // Builder appends fresh operand nodes before their parents. This runs on
    // that freshly produced graph, before externally retained checked plans;
    // it needs no cycle-discovery or per-node visited lookup.
    while let Some(handle) = pending.pop() {
        if !computations.nodes.is_valid(handle) {
            continue;
        }
        let node = computations.nodes.get(handle);
        match &node.kind {
            CheckedScalarComputationKind::StructuralField { subject, .. } => {
                if subject.source_parameter_index() == Some(receiver_source) {
                    return true;
                }
            }
            CheckedScalarComputationKind::CaseMembership { subject, .. } => match subject {
                checked_trees::CheckedScalarComputationStructuralArgument::Place(place) => {
                    if place.source_parameter_index() == Some(receiver_source) {
                        return true;
                    }
                }
                checked_trees::CheckedScalarComputationStructuralArgument::Array {
                    elements,
                    ..
                } => {
                    pending.extend_from_slice(computations.operands.span_or_empty(*elements));
                }
                checked_trees::CheckedScalarComputationStructuralArgument::Case(subject) => {
                    pending.extend(
                        computations
                            .case_fields
                            .span_or_empty(subject.fields)
                            .iter()
                            .map(|field| field.value),
                    );
                }
            },
            CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                pending.extend([*left, *right])
            }
            CheckedScalarComputationKind::Value(expression) => {
                if scalar_reads(expression, position) {
                    return true;
                }
            }
            CheckedScalarComputationKind::Apply { operands, .. } => {
                pending.extend_from_slice(computations.operands.span_or_empty(*operands));
            }
            CheckedScalarComputationKind::Call { arguments, .. } => {
                pending.extend_from_slice(computations.operands.span_or_empty(*arguments));
            }
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                pending.extend([*condition, *when_true, *when_false]);
            }
            CheckedScalarComputationKind::Qualification { operand, .. }
            | CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
                pending.push(*operand)
            }
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                pending.push(*subject);
                for arm in computations.dispatch_arms.span_or_empty(*arms) {
                    if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern
                    {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
        }
    }
    false
}

fn scalar_reads(expression: &CheckedScalarExpression, receiver: u32) -> bool {
    match expression {
        CheckedScalarExpression::StructuralParameterByteLength {
            parameter_position, ..
        }
        | CheckedScalarExpression::StructuralParameterField {
            parameter_position, ..
        } => *parameter_position == receiver,
        CheckedScalarExpression::StructuralParameterIndexedRead {
            parameter_position,
            index,
            ..
        } => *parameter_position == receiver || scalar_reads(index, receiver),
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            scalar_reads(left, receiver) || scalar_reads(right, receiver)
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. }
        | CheckedScalarExpression::IntegerWrappingCast { operand, .. }
        | CheckedScalarExpression::IntegerSaturatingCast { operand, .. }
        | CheckedScalarExpression::IntegerTrappingCast { operand, .. } => {
            scalar_reads(operand, receiver)
        }
        CheckedScalarExpression::Boolean(expression) => boolean_reads(expression, receiver),
        CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::ErasedParameter { .. } => false,
    }
}

fn boolean_reads(expression: &CheckedBooleanExpression, receiver: u32) -> bool {
    match expression {
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position, ..
        } => *parameter_position == receiver,
        CheckedBooleanExpression::Not(operand) => boolean_reads(operand, receiver),
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            boolean_reads(left, receiver) || boolean_reads(right, receiver)
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. }
        | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
            scalar_reads(left, receiver) || scalar_reads(right, receiver)
        }
        CheckedBooleanExpression::IeeeFloatComparison { left, right, .. }
        | CheckedBooleanExpression::ByteSequenceEqual { left, right }
        | CheckedBooleanExpression::PayloadlessSumEqual { left, right, .. } => {
            left.parameter_position == receiver || right.parameter_position == receiver
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, .. } => {
            subject.parameter_position == receiver
        }
        CheckedBooleanExpression::StorageRead { .. }
        | CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::ErasedParameter { .. }
        | CheckedBooleanExpression::Local { .. } => false,
    }
}

/// Whether the body uses the borrowed receiver as storage rather than only
/// as the ambient attachment its receiver calls share: it reads a receiver
/// scalar or a receiver-rooted projection, stores through a receiver-rooted
/// place, or lends a receiver field to a call. Each of those needs the
/// invocation's actual receiver loan, so the Unit signature retains `self`
/// as a structural parameter.
pub(in crate::execution::terminal_unit) fn uses_receiver_storage(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> bool {
    if reads_receiver(program, facts, state) {
        return true;
    }
    let receiver_roots = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_self)
        .map(|parameter| facts::PlaceRoot::Symbol(parameter.symbol))
        .chain(std::iter::once(facts::PlaceRoot::Symbol(machine.symbol)))
        .collect::<Vec<_>>();
    let statements = program.statement_table.statements(state.statement_nodes);
    // A store spelled through a reference local (`view.field = ..` after
    // `let view: &mut Self = &mut self`) writes the storage that local
    // aliases, so the target is judged by its resolved storage root.
    let stores_receiver = statements.iter().enumerate().any(|(index, statement)| {
        let typed_trees::statement::StatementNode::Assignment(assignment) = statement else {
            return false;
        };
        crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            index,
            assignment.target,
        )
        .is_some_and(|place| {
            let facts::PlaceRoot::Symbol(root_symbol) = place.root else {
                return receiver_roots.contains(&place.root);
            };
            let (root, _) =
                crate::execution::terminal_unit::borrowed_windows::resolve_storage_place(
                    program,
                    machine,
                    state,
                    statements,
                    index,
                    root_symbol,
                    &place.segments,
                );
            receiver_roots.contains(&root)
        })
    });
    if stores_receiver {
        return true;
    }
    // A receiver-rooted projection read in expression position — a
    // `self.field` leaf the body returns or hands to a structural value —
    // observes the same receiver storage a scalar member read does; the
    // scalar-read side above only watches the scalar computation namespaces.
    let reads_receiver_projection = statements.iter().enumerate().any(|(index, statement)| {
        let typed_trees::statement::StatementNode::Expression(expression) = statement else {
            return false;
        };
        crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            index,
            *expression,
        )
        .is_some_and(|place| receiver_roots.contains(&place.root) && !place.segments.is_empty())
    });
    if reads_receiver_projection {
        return true;
    }
    // Argument accesses key a receiver-rooted place by its field symbol, so
    // a lent receiver field is one whose root declares under the attached
    // data.
    facts
        .borrow
        .states
        .iter()
        .filter(|(_, borrow)| {
            borrow.machine_symbol == machine.symbol && borrow.state_symbol == state.symbol
        })
        .flat_map(|(_, borrow)| facts.borrow.calls.span_or_empty(borrow.calls))
        .flat_map(|call| facts.borrow.argument_accesses.span_or_empty(call.accesses))
        .any(|access| {
            access.root_symbol.is_valid()
                && program.symbols.get(access.root_symbol).parent == machine.attached_data_symbol
        })
}
