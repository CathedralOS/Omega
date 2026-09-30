//! Float ranges proved for the operands each bounded obligation names.
//!
//! A declaration describes valid boundary values, not every intermediate
//! store inside an invariant window. At an observation, a surviving literal
//! store can establish the current value independently of its declared domain.
//! The backward query crosses only complete, disjoint mutation frames and
//! includes sibling operand effects at consumption. It neither imports a
//! later repair nor transports a valuation across a state boundary.

use crate::checker::integer_ranges::type_constraints;
use crate::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, FloatRange, ProofConstraint,
    ProofPlan,
};
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use typed_trees::types::PrimitiveType;

pub(crate) fn float_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => float_range_at(
            proof_plan,
            obligation.machine_symbol,
            obligation.state_symbol,
            obligation.statement_index,
            obligation.argument,
        )
        .or_else(|| {
            float_range_from_constraints(type_constraints(
                proof_plan,
                obligation.argument_constraints,
            ))
        }),
    }
}

pub(crate) fn float_range_for_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => float_range_at(
            proof_plan,
            obligation.machine_symbol,
            obligation.state_symbol,
            obligation.statement_index,
            obligation.value,
        )
        .or_else(|| {
            float_range_from_constraints(type_constraints(proof_plan, obligation.value_constraints))
        }),
    }
}

pub(crate) fn float_range_for_call_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => float_range_for_call_place(proof_plan, obligation).or_else(|| {
            float_range_from_constraints(type_constraints(
                proof_plan,
                obligation.argument_constraints,
            ))
        }),
    }
}

pub(crate) fn float_range_for_return_value(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => float_range_at(
            proof_plan,
            obligation.machine_symbol,
            obligation.state_symbol,
            obligation.statement_index,
            obligation.value,
        )
        .or_else(|| {
            float_range_from_constraints(type_constraints(proof_plan, obligation.value_constraints))
        }),
    }
}

fn float_range_for_call_place(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
) -> Option<FloatRange> {
    let program = proof_plan.program;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == obligation.machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == obligation.state_symbol)?;
    // Statement calls retain their exact argument handles. An expression call
    // without a retained statement position cannot borrow a later store's fact.
    let mut occurrences = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(statement_index, statement)| {
            let StatementNode::Call(call) = statement else {
                return None;
            };
            (call.target_symbol == obligation.target_symbol
                && program
                    .statement_table
                    .expression_handles(call.arguments)
                    .contains(&obligation.argument))
            .then_some(statement_index)
        });
    let statement_index = occurrences.next()?;
    if occurrences.next().is_some() {
        return None;
    }
    float_range_at(
        proof_plan,
        machine.symbol,
        state.symbol,
        statement_index,
        obligation.argument,
    )
}

/// A reaching literal describes current storage, not its declared invariant.
/// Only resolved scalar places participate. Every crossed effect is checked
/// with the ordinary alias-aware write frames; incomplete frames lose the fact.
fn float_range_at(
    proof_plan: &ProofPlan,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<FloatRange> {
    let program = proof_plan.program;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)?;
    let identity = resolved_place_identity(program, expression)?;
    let place_type =
        validation::declared_place_type_raw(program, machine, Some(state), expression)?;
    let primitive = program.primitive_type_reference(place_type)?;
    let format = match primitive {
        PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
        PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
        _ => return None,
    };
    let path = super::assignment_stability::written_place_path(proof_plan, expression)?.join(".");
    let frames = validation::CallFrameResolver::new(program)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let current = statements.get(statement_index)?;
    let root = *identity.first()?;
    let reference_local = statements.iter().find_map(|statement| {
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
        if local.symbol != root {
            return None;
        }
        let mut reference = local.type_reference;
        while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        )
        .then_some(local.symbol)
    });
    // A conservative write footprint is not an exact referent identity.
    // Divergent or coarse local origins cannot supply a reaching-store fact.
    let observed_origin = match reference_local {
        Some(local) => {
            Some(frames.local_reference_origin_before_statement(machine, current, local)?)
        }
        None => None,
    };
    // Include all evaluation effects, not just the selected argument: a sibling
    // argument or guard can overwrite the observed place before consumption.
    let preserves = |written: Option<Vec<String>>| {
        written.is_some_and(|written| {
            written
                .iter()
                .all(|written| !validation::frame_paths_overlap(&path, written))
        })
    };
    if !preserves(frames.statement_value_may_write_paths(machine, current)) {
        return None;
    }
    for statement in statements[..statement_index].iter().rev() {
        if let StatementNode::Assignment(assignment) = statement
            && resolved_place_identity(program, assignment.target).as_ref() == Some(&identity)
        {
            if let Some(local) = reference_local
                && frames.local_reference_origin_before_statement(machine, statement, local)
                    != observed_origin
            {
                return None;
            }
            let target_type = validation::declared_place_type_raw(
                program,
                machine,
                Some(state),
                assignment.target,
            )?;
            if program.primitive_type_reference(target_type) != Some(primitive) {
                return None;
            }
            let ExpressionNode::Float(literal) =
                program.expression_table.expression(assignment.value)
            else {
                return None;
            };
            return (literal.landing() == Some(format))
                .then(|| finite_float_literal(literal))
                .flatten()
                .map(FloatRange::closed);
        }
        if !preserves(frames.statement_value_may_write_paths(machine, statement)) {
            return None;
        }
        match statement {
            StatementNode::Assignment(_) => {
                if !preserves(
                    frames
                        .assignment_write_frame(machine, statement)
                        .into_complete_paths(),
                ) {
                    return None;
                }
            }
            StatementNode::Call(call) => {
                if !preserves(frames.may_write_paths(machine, call)) {
                    return None;
                }
            }
            StatementNode::LocalData(_) | StatementNode::Expression(_) => {}
            _ => return None,
        }
    }
    None
}

fn resolved_place_identity(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<Vec<symbols::SymbolHandle>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && path.head_symbol == path.symbol
                && program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1 =>
        {
            Some(vec![path.symbol])
        }
        ExpressionNode::Member(member) if member.member_symbol.is_valid() => {
            let mut identity = resolved_place_identity(program, member.receiver)?;
            identity.push(member.member_symbol);
            Some(identity)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn float_range_for_initializer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => None,
    }
}

pub(crate) fn float_range_from_constraints(constraints: &[ProofConstraint]) -> Option<FloatRange> {
    let mut range: Option<FloatRange> = None;

    for constraint in constraints {
        let ProofConstraint::FloatRange {
            minimum,
            maximum,
            maximum_inclusive,
        } = constraint
        else {
            continue;
        };

        let candidate = FloatRange {
            minimum: minimum.landed_f64(),
            maximum: maximum.landed_f64(),
            maximum_inclusive: *maximum_inclusive,
        };

        range = Some(match range {
            Some(existing) => existing.intersect(candidate),
            None => candidate,
        });
    }

    range
}

pub(crate) fn finite_float_literal(value: &typed_trees::expression::FloatLiteral) -> Option<f64> {
    // The riding landing decides the value: `0.3f32` is the widened f32, not
    // the f64 read of its spelling (and f32 overflow is genuinely infinite).
    let value = value.landed_f64();
    value.is_finite().then_some(value)
}

pub(crate) fn float_range_display(range: FloatRange) -> String {
    if range.maximum_inclusive {
        format!("expected {}..={}", range.minimum, range.maximum)
    } else {
        format!("expected {}..{}", range.minimum, range.maximum)
    }
}
