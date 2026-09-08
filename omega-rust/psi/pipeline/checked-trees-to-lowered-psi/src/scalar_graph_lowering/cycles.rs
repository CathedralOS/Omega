//! Prepare invocation-independent storage for an authored scalar state loop.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
use checked_trees::{
    CheckedStructuralRankedArgumentPlan, CheckedStructuralRankedGuardPlan,
    CheckedStructuralRankedSccEdgePlan, CheckedStructuralRankedSccPlan,
};

#[cfg(test)]
mod tests;

pub(crate) struct ScalarLoopPlan {
    pub parameters: Vec<StructuralParameterDeclaration>,
    pub rank: Option<checked_trees::CheckedStructuralRankedSccPlan>,
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    graph: &CheckedScalarMachineGraph,
    parameters: &[StructuralParameterDeclaration],
    next_place: &mut u64,
) -> Result<Option<ScalarLoopPlan>, LoweringError> {
    let [state] = graph.states.as_slice() else {
        return Ok(None);
    };
    let returns_to_entry = |destination: &CheckedScalarBranchDestination| matches!(destination, CheckedScalarBranchDestination::Jump(successor) if successor.target == state.state);
    let reentered = match &state.terminator {
        CheckedScalarStateTerminator::Jump(successor) => successor.target == state.state,
        CheckedScalarStateTerminator::Conditional {
            when_true,
            when_false,
            ..
        } => returns_to_entry(when_true) || returns_to_entry(when_false),
        _ => false,
    };
    if !reentered {
        if graph.ranked_scc.is_some() {
            return unsupported("scalar loop ranking has no retained source backedge");
        }
        return Ok(None);
    }
    let (machine, source) = source_custody::authored_state(checked, state.state)?;
    if machine.symbol != graph.machine {
        return unsupported("scalar loop has a foreign source state");
    }
    if parameters.iter().any(|parameter| {
        parameter.access != StructuralAccess::Owned
            || !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            )
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
    }) {
        return unsupported("scalar loop requires whole plain-owned state parameters");
    }
    let rank = graph.ranked_scc.clone();
    if machine.termination_plan.implementation_witness.is_some() != rank.is_some() {
        return unsupported("scalar loop lost or substituted its authored ranking witness");
    }
    if let Some(rank) = &rank {
        validate_rank(checked, machine, source, state, rank)?;
    }
    let mut loop_parameters = parameters.to_vec();
    for (position, parameter) in loop_parameters.iter_mut().enumerate() {
        parameter.position = u32::try_from(position)
            .map_err(|_| LoweringError::Unsupported("scalar loop descriptor index exceeds u32"))?;
        parameter.place = place_id(allocate_dense(next_place)?);
    }
    Ok(Some(ScalarLoopPlan {
        parameters: loop_parameters,
        rank,
    }))
}

fn validate_rank(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    source: &checked_trees::state::State,
    state: &checked_trees::CheckedScalarStateGraph,
    rank: &CheckedStructuralRankedSccPlan,
) -> Result<(), LoweringError> {
    source_custody::parameter_storage(checked, machine.symbol, state)?;
    let mut custodies = checked
        .ranking_expression_custody
        .iter()
        .filter(|custody| custody.machine == machine.symbol);
    let custody = custodies.next().ok_or(LoweringError::Unsupported(
        "scalar loop lost its ranking source",
    ))?;
    if custodies.next().is_some() {
        return unsupported("scalar loop has ambiguous ranking source custody");
    }
    let [subject] = custody.subjects.as_slice() else {
        return unsupported("scalar loop requires one exact natural rank subject");
    };
    let ExpressionNode::Name(name) = checked.expression_table.expression(*subject) else {
        return unsupported("scalar loop rank is not an authored parameter");
    };
    if !checked.expression_table.expression_is_valid(*subject)
        || !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || name.members.count() != 1
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return unsupported("scalar loop rank lost its exact resolved parameter");
    }
    let witness = machine
        .termination_plan
        .implementation_witness
        .as_ref()
        .ok_or(LoweringError::Unsupported(
            "scalar loop lost its authored ranking witness",
        ))?;
    let natural_view = language_semantics::RankingViewId::NAT_DESCENDING;
    if witness.ranking_view != natural_view
        || Some(witness.view_path.as_str()) != natural_view.canonical_path()
        || !witness.view_arguments.is_empty()
        || !custody.view_arguments.is_empty()
        || witness.subjects.len() != 1
        || witness.subjects[0] != checked_trees::ranking::witness_expression_text(checked, *subject)
    {
        return unsupported("scalar loop witness differs from its canonical natural source view");
    }
    match (&witness.rank_range, custody.rank_range) {
        (None, None) => {}
        (Some(recorded), Some(expression)) => {
            if !checked.expression_table.expression_is_valid(expression) {
                return unsupported("scalar loop lost its typed rank range");
            }
            let ExpressionNode::Range(range) = checked.expression_table.expression(expression)
            else {
                return unsupported("scalar loop rank range is not an authored range");
            };
            // Rejoin normalized witness metadata to the retained typed roots,
            // never resolve expressions by their labels. The existing normalizer
            // intentionally renders general arithmetic as "value". This is not
            // a range-membership proof: the existing checker owns that judgment,
            // and Natural independently verifies emitted descent.
            if !checked.expression_table.expression_is_valid(range.start)
                || !checked.expression_table.expression_is_valid(range.end)
                || recorded.floor
                    != checked_trees::ranking::witness_expression_text(checked, range.start)
                || recorded.ceiling
                    != checked_trees::ranking::witness_expression_text(checked, range.end)
                || recorded.ceiling_inclusive != range.end_inclusive
            {
                return unsupported("scalar loop witness differs from its typed rank range");
            }
        }
        _ => return unsupported("scalar loop witness lost or added an authored rank range"),
    }
    let parameters = checked.state_parameters(source);
    let source_position = parameters
        .iter()
        .position(|parameter| parameter.symbol == name.symbol)
        .ok_or(LoweringError::Unsupported(
            "scalar loop rank names a foreign parameter",
        ))?;
    let parameter = &parameters[source_position];
    let primitive = checked
        .primitive_type_reference(parameter.type_reference)
        .ok_or(LoweringError::Unsupported(
            "scalar loop rank is not primitive",
        ))?;
    let scalar_position = parameters[..source_position]
        .iter()
        .filter(|parameter| {
            checked
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .count();
    let scalar_position = u32::try_from(scalar_position)
        .map_err(|_| LoweringError::Unsupported("scalar loop rank index exceeds u32"))?;
    let argument_ordinal = u32::try_from(source_position)
        .map_err(|_| LoweringError::Unsupported("scalar loop rank position exceeds u32"))?;
    let ScalarType::Integer(carrier) = terminal_scalar_type(primitive)? else {
        return unsupported("scalar natural rank requires an unsigned integer carrier");
    };
    let IntegerValue::Unsigned(upper_bound) = carrier.maximum_value() else {
        return unsupported("scalar natural rank requires an unsigned integer carrier");
    };

    fn branch(destination: &CheckedScalarBranchDestination) -> Option<&CheckedScalarSuccessor> {
        match destination {
            CheckedScalarBranchDestination::Jump(successor) => Some(successor),
            _ => None,
        }
    }
    let successors = match &state.terminator {
        CheckedScalarStateTerminator::Jump(successor) => [Some(successor), None],
        CheckedScalarStateTerminator::Conditional {
            when_true,
            when_false,
            ..
        } => [branch(when_true), branch(when_false)],
        _ => [None, None],
    };
    let mut covered_cyclic_edges = Vec::new();
    // Source order is authoritative, not retained witness order or emitted
    // block order. Rejoin every authored backedge before deriving its row.
    for (statement_ordinal, statement) in checked
        .statement_table
        .statements(source.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        for (target, is_continuation) in
            [(transition.target, false), (transition.continuation, true)]
        {
            if !target.is_valid() {
                continue;
            }
            let TransitionTargetNode::Named { path, .. } =
                checked.statement_table.transition_target(target)
            else {
                if matches!(
                    checked.statement_table.transition_target(target),
                    TransitionTargetNode::SelfTarget
                ) {
                    return unsupported(
                        "scalar natural rank has an implicit unauthored argument roster",
                    );
                }
                continue;
            };
            if source_custody::successors::normalize_machine_state_target(
                checked,
                machine,
                path.symbol,
            )? != source.symbol
            {
                continue;
            }
            // The existing Nat countdown judgment records primary targets only.
            // Its guard/decrement proof remains with that owner; do not add a
            // second syntax recognizer for Boolean wrappers or failed guards.
            if transition.exit != TransitionExit::Ordinary || is_continuation {
                return unsupported("scalar natural rank has an unsupported source edge role");
            }
            let statement_ordinal = u32::try_from(statement_ordinal)
                .map_err(|_| LoweringError::Unsupported("scalar loop statement exceeds u32"))?;
            let mut matching = successors.iter().flatten().copied().filter(|successor| {
                successor.statement_ordinal == statement_ordinal
                    && successor.is_continuation == is_continuation
                    && successor.target == source.symbol
            });
            let successor = matching.next().ok_or(LoweringError::Unsupported(
                "scalar natural rank lost an authored successor",
            ))?;
            if matching.next().is_some() {
                return unsupported("scalar natural rank duplicated an authored successor");
            }
            source_custody::validate_successor(checked, source.symbol, successor)?;
            // Both descriptors name the exact subject in the existing Nat
            // judgment. The argument ordinal is authored, while the three
            // parameter indices are dense scalar indices. Actual expression
            // custody and Natural verification separately preserve and prove
            // the selected guard/decrement, including private evaluation steps.
            covered_cyclic_edges.push(CheckedStructuralRankedSccEdgePlan {
                source_state: source.symbol,
                target_state: source.symbol,
                statement_ordinal,
                guard: CheckedStructuralRankedGuardPlan::UnsignedParameterPositive {
                    scalar_parameter_index: scalar_position,
                    primitive_type: primitive,
                },
                successor_argument:
                    CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne {
                        argument_ordinal,
                        source_scalar_parameter_index: scalar_position,
                        target_scalar_parameter_index: scalar_position,
                        primitive_type: primitive,
                    },
            });
        }
    }
    if covered_cyclic_edges.is_empty()
        || covered_cyclic_edges.len() != successors.iter().flatten().count()
        || *rank
            != (CheckedStructuralRankedSccPlan {
                header_state: source.symbol,
                rank_scalar_parameter_index: scalar_position,
                rank_primitive_type: primitive,
                rank_lower_bound: 0,
                rank_upper_bound: upper_bound,
                covered_cyclic_edges,
            })
    {
        return unsupported("scalar loop ranking differs from its complete authored rank roster");
    }
    Ok(())
}
