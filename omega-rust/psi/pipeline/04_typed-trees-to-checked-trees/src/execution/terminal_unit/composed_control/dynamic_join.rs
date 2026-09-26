use super::super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralScalarParameterPlan, StatementNode,
    SymbolHandle, TransitionExit, TransitionGuardNode, TransitionTargetNode, TypedTrees,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;
use crate::execution::terminal_unit::composed_control::topology;

use crate::execution::terminal_unit::types::{ShapeCollector, is_unit, machine_binders};

use crate::execution::terminal_unit::composed_control::guards;

pub(in crate::execution::terminal_unit) struct DynamicJoinControlTopology {
    pub entry_state: SymbolHandle,
    pub attachment_type_identity: String,
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub guard: CheckedScalarExpression,
    pub successors: [CheckedStructuralControlSuccessorPlan; 2],
}

/// Reuse the ordinary composed-control topology proof for the dynamic join
/// lane. The entry accepts an implicit borrowed `self`, scalar entry
/// parameters in contiguous authored positions starting at 1, and two
/// custody-free leaves; the leaf operations are owned by the dynamic call
/// plans instead of the general effect planner. Which (parameter type,
/// guard shape) pairs compose is decided by `exact_guard`, not here — a
/// Boolean parameter admits the bare/equality/negated forms, an integer
/// parameter the comparison forms, and a retained `self` field the
/// field-read forms.
pub(in crate::execution::terminal_unit) fn admit_dynamic_join_control_topology(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
) -> Option<DynamicJoinControlTopology> {
    let [entry, first_branch_state, second_branch_state] = program.machine_states(machine) else {
        return None;
    };
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, entry_structural, scalar_parameters) =
        structural_scalar_signature(program, shapes, machine, entry, &binders, false)?;
    if !scalar_parameters
        .iter()
        .enumerate()
        .all(|(position, parameter)| parameter.source_position as usize == position + 1)
    {
        return None;
    }
    let statements = program.statement_table.statements(entry.statement_nodes);
    let split = statements.len().checked_sub(2)?;
    let [
        StatementNode::Transition(when_true),
        StatementNode::Transition(when_false),
    ] = &statements[split..]
    else {
        return None;
    };
    // Immutable scalar lets may precede the split: the joined caller
    // re-evaluates any the guard references and drops the rest, so each
    // must be a pure expression the guard grammar could itself admit.
    if !statements[..split]
        .iter()
        .enumerate()
        .all(|(ordinal, statement)| {
            let StatementNode::LocalData(local) = statement else {
                return false;
            };
            let Ok(ordinal) = u32::try_from(ordinal) else {
                return false;
            };
            !local.is_mutable
                && facts
                    .values
                    .scalar_expressions
                    .expression_at(
                        entry.symbol,
                        ordinal,
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: ordinal,
                        },
                    )
                    .is_some_and(guards::evaluatable_scalar)
        })
    {
        return None;
    }
    if when_true.exit != TransitionExit::Ordinary
        || !matches!(when_true.guard, TransitionGuardNode::When(_))
        || when_false.exit != TransitionExit::Ordinary
        || when_false.guard != TransitionGuardNode::Always
        || when_true.continuation.is_valid()
        || when_false.continuation.is_valid()
    {
        return None;
    }
    // Each arm resolves its own branch state by name: the `true` arm may
    // target either non-entry state, so the polarity of the transition is
    // read from the authored targets rather than declaration order.
    let true_target = branch_target_symbol(program, when_true)?;
    let false_target = branch_target_symbol(program, when_false)?;
    let (when_true_state, when_false_state) = if true_target == first_branch_state.symbol
        && false_target == second_branch_state.symbol
    {
        (first_branch_state, second_branch_state)
    } else if true_target == second_branch_state.symbol && false_target == first_branch_state.symbol
    {
        (second_branch_state, first_branch_state)
    } else {
        return None;
    };
    let (true_attachment, true_structural, true_scalar) =
        structural_scalar_signature(program, shapes, machine, when_true_state, &binders, false)?;
    let (false_attachment, false_structural, false_scalar) =
        structural_scalar_signature(program, shapes, machine, when_false_state, &binders, false)?;
    if [entry, when_true_state, when_false_state]
        .iter()
        .any(|state| {
            !is_unit(program, state.return_type) || !program.state_contracts(state).is_empty()
        })
        || attachment_type_identity != true_attachment
        || attachment_type_identity != false_attachment
        || !entry_structural.is_empty()
        || !true_structural.is_empty()
        || !false_structural.is_empty()
        || !true_scalar.is_empty()
        || !false_scalar.is_empty()
        || !topology::only_implicit_reference_self_is_omitted(
            program,
            entry,
            &entry_structural,
            &scalar_parameters,
        )
        || !topology::only_implicit_reference_self_is_omitted(
            program,
            when_true_state,
            &true_structural,
            &true_scalar,
        )
        || !topology::only_implicit_reference_self_is_omitted(
            program,
            when_false_state,
            &false_structural,
            &false_scalar,
        )
    {
        return None;
    }
    let guard_ordinal = u32::try_from(split).ok()?;
    let guard = guards::exact_guard(
        facts.values.scalar_expressions.expression_at(
            entry.symbol,
            guard_ordinal,
            CheckedScalarExpressionRole::Guard,
        )?,
        &scalar_parameters,
        &facts.values.scalar_expressions,
        entry.symbol,
    )?;
    let successors = [
        topology::successor(
            program,
            facts,
            machine,
            entry,
            &[],
            &[],
            &[],
            &[],
            guard_ordinal,
            when_true,
            when_true_state.symbol,
            &[],
        )?,
        topology::successor(
            program,
            facts,
            machine,
            entry,
            &[],
            &[],
            &[],
            &[],
            guard_ordinal + 1,
            when_false,
            when_false_state.symbol,
            &[],
        )?,
    ];
    Some(DynamicJoinControlTopology {
        entry_state: entry.symbol,
        attachment_type_identity,
        scalar_parameters,
        guard,
        successors,
    })
}

/// The state symbol a branch transition names, or `None` when the arm does
/// not target a named state.
fn branch_target_symbol(
    program: &TypedTrees,
    transition: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::TableTransition,
) -> Option<SymbolHandle> {
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    Some(path.symbol)
}
