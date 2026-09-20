//! Scanning statement and expression reads and validating each data read
//! against established places and range gates.

use crate::proof_contracts::default_domains::place_queries::{
    data_definition_for_expression, is_self_rooted, self_place_spelling,
};
use crate::proof_contracts::default_domains::{InvariantWindow, TrackedPlace};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

/// R2 rung 3 slice 2: refuse reads of an unestablished GATED place. V1
/// scans value-position expressions for member chains whose self-rooted
/// receiver names a tracked-or-fresh gated place; cross-state
/// establishment is not trackable yet and refuses with direction.
pub(crate) fn scan_statement_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reads: Vec<ExpressionHandle> = Vec::new();
    match statement {
        // An assembly fact is itself a checked consumption point. Its proof
        // checker handles establishment; this runtime-read scan must not
        // interpret it as an executed expression.
        StatementNode::RootBinding(binding) => {
            reads.extend(
                [binding.receiver, binding.implementation_operand]
                    .into_iter()
                    .filter(|expression| expression.is_valid()),
            );
        }
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => reads.push(assignment.value),
        StatementNode::Expression(expression) => reads.push(*expression),
        StatementNode::LocalData(local) => {
            if local.initial_value.is_valid() {
                reads.push(local.initial_value);
            }
        }
        StatementNode::Call(call) => {
            let receiver = program.statement_table.name_path_members(call.receiver);
            if receiver.len() > 1
                && receiver[0].as_str() == "self"
                && let Some(definition) = machine.attached_data.as_ref().and_then(|attached| {
                    program
                        .data_definitions()
                        .iter()
                        .find(|definition| definition.name == *attached)
                })
                && crate::value_custody::data::data_requires_establishment(program, definition)
            {
                validate_data_read(
                    program,
                    machine,
                    definition,
                    "self",
                    receiver[1].as_str(),
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
            reads.extend(
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        }
        StatementNode::Transition(transition) => {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = &transition.guard {
                reads.push(*guard);
            }
        }
    }
    for read in reads {
        scan_expression_reads(
            program,
            machine,
            state,
            read,
            tracked,
            entry_established,
            call_established,
            inherited_windows,
            diagnostics,
        );
    }
}

fn scan_expression_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.len() == 1
                && members[0].as_str() == "self"
                && let Some(definition) = machine.attached_data.as_ref().and_then(|attached| {
                    program
                        .data_definitions()
                        .iter()
                        .find(|definition| definition.name == *attached)
                })
                && let Some(place) = tracked.iter().find(|place| place.spelling == "self")
                && place.window_open
            {
                diagnostics.push(Diagnostic::error(format!(
                    "the next whole-value read of `self` occurs inside an OPEN invariant \
                     window: a prior write left data `{}`'s default domain FALSE -- \
                     restore the facts before copying or exposing the value (ch11)",
                    definition.name.as_str()
                )));
            }
            if members.len() > 1 {
                let receiver_spelling = members[..members.len() - 1]
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                if let Some(place) = tracked
                    .iter()
                    .find(|place| place.spelling == receiver_spelling)
                {
                    validate_data_read(
                        program,
                        machine,
                        place.definition,
                        &receiver_spelling,
                        members.last().expect("non-empty path").as_str(),
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        diagnostics,
                    );
                }
            }
            // A machine call such as `self.console.exit_process(..)` stores
            // `self.console` as the call receiver Name path. It still consumes
            // the attached `self` value, so it must not bypass establishment
            // merely because no standalone Member node was built.
            if members.len() > 1
                && members[0].as_str() == "self"
                && let Some(definition) = machine.attached_data.as_ref().and_then(|attached| {
                    program
                        .data_definitions()
                        .iter()
                        .find(|definition| definition.name == *attached)
                })
            {
                validate_data_read(
                    program,
                    machine,
                    definition,
                    "self",
                    members[1].as_str(),
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Member(member) => {
            if let Some(receiver_spelling) = self_place_spelling(program, member.receiver) {
                let tracked_receiver = tracked
                    .iter()
                    .find(|place| place.spelling == receiver_spelling);
                let definition = tracked_receiver.map(|place| place.definition).or_else(|| {
                    data_definition_for_expression(program, machine, Some(state), member.receiver)
                });
                if let Some(definition) = definition
                    && crate::value_custody::data::data_requires_establishment(program, definition)
                {
                    validate_data_read(
                        program,
                        machine,
                        definition,
                        &receiver_spelling,
                        member.member.as_str(),
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        diagnostics,
                    );
                }
            }
            if !is_bare_self_name(program, member.receiver) {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    member.receiver,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Indexed(indexed) => {
            scan_expression_reads(
                program,
                machine,
                state,
                indexed.collection,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
            scan_expression_reads(
                program,
                machine,
                state,
                indexed.index,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Binary(binary) => {
            scan_expression_reads(
                program,
                machine,
                state,
                binary.left,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
            scan_expression_reads(
                program,
                machine,
                state,
                binary.right,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Borrow(inner) => {
            if let ExpressionNode::Member(member) =
                program.expression_table.expression(inner.target)
                && let Some(receiver_spelling) = self_place_spelling(program, member.receiver)
                && let Some(place) = tracked
                    .iter()
                    .find(|place| place.spelling == receiver_spelling && place.window_open)
            {
                validate_data_read(
                    program,
                    machine,
                    place.definition,
                    &receiver_spelling,
                    member.member.as_str(),
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
            scan_expression_reads(
                program,
                machine,
                state,
                inner.target,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Call(call) => {
            scan_expression_reads(
                program,
                machine,
                state,
                call.receiver,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    *argument,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

fn is_bare_self_name(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    let members = program.expression_table.name_path_members(path.members);
    members.len() == 1 && members[0].as_str() == "self"
}

fn validate_data_read(
    program: &TypedTrees,
    machine: &Machine,
    definition: &DataDefinition,
    receiver_spelling: &str,
    member_name: &str,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let place = tracked
        .iter()
        .find(|place| place.spelling == receiver_spelling);
    let established = place.map(|place| place.established).unwrap_or_else(|| {
        // Parameters and locals arrive domain-valid from their caller or
        // initializer; machine-owned `self` storage starts as representation
        // only and must earn establishment on every incoming path.
        entry_established
            .iter()
            .any(|established| established == receiver_spelling)
            || call_established
                .iter()
                .any(|established| established == receiver_spelling)
            || !is_self_rooted(receiver_spelling)
    });
    let established = established
        || (receiver_spelling == "self"
            && attached_value_established(
                program,
                machine,
                tracked,
                entry_established,
                call_established,
            ));
    if !established {
        diagnostics.push(Diagnostic::error(format!(
            "reading `{receiver_spelling}.{member_name}` crosses an open default-domain \
             invariant window before data `{}` is established: the zeroed representation \
             is not yet a `{}` (ch12's access gate) -- construct it on every path first \
             (the cross-state must-analysis carries establishment)",
            definition.name.as_str(),
            definition.name.as_str()
        )));
    }
    if place.is_some_and(|place| place.window_open) {
        diagnostics.push(Diagnostic::error(format!(
            "reading `{receiver_spelling}.{member_name}` inside an OPEN invariant window: \
             a prior write left data `{}`'s default domain FALSE -- restore the facts \
             before this consumption point (ch11)",
            definition.name.as_str()
        )));
    }
    if place.is_none()
        && inherited_windows
            .iter()
            .any(|(spelling, _, _)| spelling == receiver_spelling)
    {
        diagnostics.push(Diagnostic::error(format!(
            "reading `{receiver_spelling}.{member_name}` inside an OPEN invariant window \
             carried from a predecessor state: data `{}`'s default domain is FALSE -- \
             restore the facts before this consumption point (ch11 window transport)",
            definition.name.as_str()
        )));
    }
}

/// A nested gated field gates its containing machine value, but establishing
/// that child must in turn establish the parent once every gated child is
/// ready. This is the establishment analogue of structural ZII composition:
/// the parent carries no independent ceremony when its own authored default
/// domain accepts zero.
pub(crate) fn attached_value_established(
    program: &TypedTrees,
    machine: &Machine,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
) -> bool {
    if direct_place_established("self", tracked, entry_established, call_established) {
        return true;
    }
    let Some(attached) = machine.attached_data.as_ref() else {
        return false;
    };
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name == *attached)
    else {
        return false;
    };
    // An authored default domain that rejects zero must be established by its
    // own proof net; child establishment cannot manufacture that evidence.
    if definition.zero_gated {
        return false;
    }
    let root = tracked.iter().find(|place| place.spelling == "self");
    for member in program.data_members(definition) {
        let typed_trees::data::DataMember::Field(field) = member else {
            continue;
        };
        if !crate::value_custody::data::type_requires_establishment(program, field.type_reference) {
            continue;
        }
        if let Some(interval) =
            crate::proof_contracts::arithmetic_domains::range_constraint_interval(
                program,
                field.type_reference,
            )
        {
            let Some(value) = root
                .and_then(|place| {
                    place
                        .fields
                        .iter()
                        .find(|(name, _)| name == field.name.as_str())
                })
                .and_then(|(_, value)| *value)
            else {
                return false;
            };
            if interval.low().is_some_and(|low| value < i128::from(low))
                || interval.high().is_some_and(|high| value > i128::from(high))
            {
                return false;
            }
        } else {
            let child = format!("self.{}", field.name.as_str());
            if !direct_place_established(&child, tracked, entry_established, call_established) {
                return false;
            }
        }
    }
    true
}

fn direct_place_established(
    spelling: &str,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
) -> bool {
    tracked
        .iter()
        .any(|place| place.spelling == spelling && place.established)
        || entry_established.iter().any(|place| place == spelling)
        || call_established.iter().any(|place| place == spelling)
}

/// Implicit range/containment gates hold only after every zero-excluding
/// common field has a known established value. Scalar ranges can be discharged
/// by the ordinary literal valuation. Nested data and arrays are established
/// by whole-value construction for now; a scalar field write cannot fabricate
/// evidence for them.
pub(crate) fn range_gates_hold(program: &TypedTrees, place: &TrackedPlace<'_>) -> bool {
    for member in program.data_members(place.definition) {
        let typed_trees::data::DataMember::Field(field) = member else {
            continue;
        };
        if !crate::value_custody::data::type_requires_establishment(program, field.type_reference) {
            continue;
        }
        let Some(interval) = crate::proof_contracts::arithmetic_domains::range_constraint_interval(
            program,
            field.type_reference,
        ) else {
            // Nested records and arrays require whole-value establishment in
            // this first slice.
            return false;
        };
        let Some(value) = place
            .fields
            .iter()
            .find(|(name, _)| name == field.name.as_str())
            .and_then(|(_, value)| *value)
        else {
            return false;
        };
        if interval.low().is_some_and(|low| value < i128::from(low))
            || interval.high().is_some_and(|high| value > i128::from(high))
        {
            return false;
        }
    }
    true
}
