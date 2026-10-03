//! Find an authored scalar state graph's natural loops. Each feedback edge
//! must return to a header that dominates it; only a re-entered entry needs
//! invocation-independent loop-header storage, because every later row
//! already binds its values as block parameters.
use super::super::{allocate_dense, place_id};
use super::{
    CheckedScalarBranchDestination, CheckedScalarMachineGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor, CheckedTrees, IntegerValue, LoweringError, ScalarType,
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, source_custody, terminal_scalar_type, unsupported,
};
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedStructuralRankedArgumentPlan, CheckedStructuralRankedGuardPlan,
    CheckedStructuralRankedSccEdgePlan, CheckedStructuralRankedSccPlan,
};
use terminal_psi::StructuralTypeShape;

#[cfg(test)]
mod tests;

pub(crate) struct ScalarLoopPlan {
    pub entry: ScalarLoopEntry,
    pub rank: Option<checked_trees::CheckedStructuralRankedSccPlan>,
}

/// How the graph's entry row relates to its cycles. Every other row is an
/// ordinary block whose parameters already carry the values its incoming
/// edges bind, so a feedback edge into it needs no rebinding.
pub(crate) enum ScalarLoopEntry {
    /// A feedback edge re-enters the entry row. The full binding roster: the
    /// receiver keeps its machine place, while explicit state formals receive
    /// fresh loop-header places.
    Reentered {
        parameters: Vec<StructuralParameterDeclaration>,
    },
    /// Every feedback edge re-enters a later row; the entry runs once as the
    /// loop prologue and binds its roster on the machine signature only.
    Prologue,
}

impl ScalarLoopPlan {
    /// The loop-header roster that replaces the entry's machine namespace,
    /// present only when a feedback edge re-enters the entry row.
    pub(crate) fn entry_header_parameters(&self) -> Option<&[StructuralParameterDeclaration]> {
        match &self.entry {
            ScalarLoopEntry::Reentered { parameters } => Some(parameters),
            ScalarLoopEntry::Prologue => None,
        }
    }

    pub(crate) fn reenters_entry(&self) -> bool {
        self.entry_header_parameters().is_some()
    }
}

/// Row successors of the authored graph: the targets of each unconditional
/// jump and each conditional arm that jumps, as dense row indices. A target
/// outside the rows is left to successor lowering, which rejects it.
fn row_successors(graph: &CheckedScalarMachineGraph) -> Vec<Vec<usize>> {
    let row = |successor: &CheckedScalarSuccessor| {
        graph
            .states
            .iter()
            .position(|state| state.state == successor.target)
    };
    let jump = |destination: &CheckedScalarBranchDestination| match destination {
        CheckedScalarBranchDestination::Jump(successor) => row(successor),
        _ => None,
    };
    graph
        .states
        .iter()
        .map(|state| match &state.terminator {
            CheckedScalarStateTerminator::Jump(successor) => row(successor).into_iter().collect(),
            CheckedScalarStateTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => jump(when_true)
                .into_iter()
                .chain(jump(when_false))
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

/// Feedback edges `(source, header)` of a depth-first walk from the entry row:
/// each targets a row still on the walk's active path.
fn feedback_edges(successors: &[Vec<usize>]) -> Vec<(usize, usize)> {
    let mut feedback = Vec::new();
    let mut visited = vec![false; successors.len()];
    let mut active = vec![false; successors.len()];
    // Each frame is a row and the index of its next unvisited successor.
    let mut path = vec![(0usize, 0usize)];
    visited[0] = true;
    active[0] = true;
    while let Some((row, next)) = path.last_mut() {
        let row = *row;
        let Some(&target) = successors[row].get(*next) else {
            active[row] = false;
            path.pop();
            continue;
        };
        *next += 1;
        if active[target] {
            feedback.push((row, target));
        } else if !visited[target] {
            visited[target] = true;
            active[target] = true;
            path.push((target, 0));
        }
    }
    feedback
}

/// Whether every path from the entry row to `row` passes through `header`.
fn dominates(successors: &[Vec<usize>], header: usize, row: usize) -> bool {
    if header == 0 || header == row {
        return true;
    }
    let mut reached = vec![false; successors.len()];
    reached[0] = true;
    let mut pending = vec![0usize];
    while let Some(current) = pending.pop() {
        if current == row {
            return false;
        }
        for &target in &successors[current] {
            if target != header && !reached[target] {
                reached[target] = true;
                pending.push(target);
            }
        }
    }
    true
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    graph: &CheckedScalarMachineGraph,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Option<ScalarLoopPlan>, LoweringError> {
    // The graph's dispatch entry is its first row. A cycle may re-enter it or
    // any later row: a fused graph reenters the entry when any member's
    // terminator jumps back, while a multi-state machine whose entry only
    // seeds its loop re-enters the state that tests the loop condition.
    let Some(entry) = graph.states.first() else {
        return Ok(None);
    };
    let successors = row_successors(graph);
    let feedback = feedback_edges(&successors);
    if feedback.is_empty() {
        if graph.ranked_scc.is_some() {
            return unsupported("scalar loop ranking has no retained source backedge");
        }
        return Ok(None);
    }
    // Every feedback edge must return to a header that dominates its source,
    // so each cycle is a natural loop entered only through that header. The
    // header's block parameters then carry every loop value, and emission
    // needs no second entry into the loop body.
    if feedback
        .iter()
        .any(|&(source, header)| !dominates(&successors, header, source))
    {
        return unsupported("scalar graph cycle has no dominating loop header");
    }
    let (machine, _) = source_custody::authored_state(checked, entry.state)?;
    if machine.symbol != graph.machine {
        return unsupported("scalar loop has a foreign source state");
    }
    let mut headers = feedback
        .iter()
        .map(|&(_, header)| header)
        .collect::<Vec<_>>();
    headers.sort_unstable();
    headers.dedup();
    let rank = graph.ranked_scc.clone();
    if let Some(rank) = &rank {
        // One ranked component re-enters one header; its authored source is
        // the rank coordinate every covered edge descends from.
        let [header] = headers.as_slice() else {
            return unsupported("scalar loop ranking requires one loop header");
        };
        let (header_machine, header_source) =
            source_custody::authored_state(checked, graph.states[*header].state)?;
        if header_machine
            .termination_plan
            .implementation_witness
            .is_none()
        {
            return unsupported("scalar loop lost or substituted its authored ranking witness");
        }
        validate_rank(checked, header_machine, header_source, graph, rank)?;
    }
    for header in headers {
        let (header_machine, _) =
            source_custody::authored_state(checked, graph.states[header].state)?;
        let Some(witness) = &header_machine.termination_plan.implementation_witness else {
            continue;
        };
        // A fused member's own witness needs its retained component judgment;
        // an unranked dispatch prologue cannot erase that obligation.
        if header_machine.symbol != graph.machine && rank.is_none() {
            return unsupported("scalar loop header's ranking witness has no graph judgment");
        }
        // The Nat countdown is only one ranking order: a cyclic machine proven
        // under another order (`Slice::Length`, struct views) keeps its
        // judgment in the shared natural ranks and lowers here without a
        // ranked-SCC certificate rather than substituting one.
        if rank.is_none()
            && witness.ranking_view == language_semantics::RankingViewId::NAT_DESCENDING
        {
            return unsupported("scalar loop lost or substituted its authored ranking witness");
        }
    }
    if feedback.iter().all(|&(_, header)| header != 0) {
        // The entry roster binds once on the machine signature, before any
        // cycle; later rows' scalar and borrowed structural formals rebind
        // through their own block parameters on every feedback edge.
        return Ok(Some(ScalarLoopPlan {
            entry: ScalarLoopEntry::Prologue,
            rank,
        }));
    }
    if parameters.iter().any(|parameter| {
        // Signature replay preserves the receiver's exact shared or mutable
        // access. It remains the same invocation-scoped loan throughout the
        // loop and never binds on a backedge; only explicit formals need
        // loop-carrier admission. Requiring a shared receiver here would
        // reject an already admitted exclusive loan without improving custody.
        if parameter.is_self {
            return !matches!(
                parameter.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            );
        }
        // A borrowed runtime-length view rebinds as a shared loan on the
        // backedge: it carries no owned disposal, so the plain-owned loop
        // rule does not apply, but every other custody shape still rejects.
        let plain_owned = parameter.access == StructuralAccess::Owned
            && matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            );
        let shared_view = parameter.access == StructuralAccess::SharedBorrow
            && parameter.multiplicity == StructuralMultiplicity::Unrestricted
            && structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ElementView { .. }
                            | StructuralTypeShape::ByteSequence(_)
                    )
                });
        // Ordinary record loans move only the usable binding across the
        // edge, never the caller's storage or an owned disposal obligation.
        // Signature and successor custody independently rejoin the exact
        // referent and access; Terminal replay checks the resulting aliases.
        let borrowed_record = matches!(
            parameter.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
            && structural_types.iter().any(|declaration| {
                declaration.id == parameter.structural_type
                    && matches!(declaration.shape, StructuralTypeShape::Record { .. })
            });
        !(plain_owned || shared_view || borrowed_record)
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
    }) {
        return unsupported("scalar loop requires plain owned values or whole supported loans");
    }
    let mut loop_parameters = parameters.to_vec();
    for (position, parameter) in loop_parameters
        .iter_mut()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
    {
        parameter.position = u32::try_from(position)
            .map_err(|_| LoweringError::Unsupported("scalar loop descriptor index exceeds u32"))?;
        parameter.place = place_id(allocate_dense(next_place)?);
    }
    Ok(Some(ScalarLoopPlan {
        entry: ScalarLoopEntry::Reentered {
            parameters: loop_parameters,
        },
        rank,
    }))
}

/// One SCC member's rank coordinate, resolved against its own authored source
/// and witness. A fused cycle ranks every member's own formal; the plan-level
/// index names the header's scalar position.
struct MemberRank<'a> {
    machine: &'a checked_trees::machine::Machine,
    source: &'a checked_trees::state::State,
    scalar_position: u32,
    argument_ordinal: u32,
    primitive: PrimitiveType,
    upper_bound: u128,
}

fn validate_rank<'a>(
    checked: &'a CheckedTrees,
    machine: &'a checked_trees::machine::Machine,
    source: &'a checked_trees::state::State,
    graph: &'a CheckedScalarMachineGraph,
    rank: &CheckedStructuralRankedSccPlan,
) -> Result<(), LoweringError> {
    if rank.covered_cyclic_edges.iter().any(|edge| {
        matches!(
            edge.successor_argument,
            CheckedStructuralRankedArgumentPlan::UnsignedParameterComparison { .. }
        )
    }) {
        return validate_runtime_rank(checked, graph, rank);
    }
    // The member set preserves authored edge order — header first, then each
    // covered edge's endpoints — so the rejoined roster compares to the
    // checker's retained plan element for element.
    let mut member_sources: Vec<(
        &'a checked_trees::machine::Machine,
        &'a checked_trees::state::State,
    )> = vec![(machine, source)];
    for edge in &rank.covered_cyclic_edges {
        for state_symbol in [edge.source_state, edge.target_state] {
            let (member_machine, member_source) =
                source_custody::authored_state(checked, state_symbol)?;
            if member_sources
                .iter()
                .all(|(_, state)| state.symbol != member_source.symbol)
            {
                member_sources.push((member_machine, member_source));
            }
        }
    }
    let mut members = Vec::new();
    for (member_machine, member_source) in member_sources {
        let member_row = graph
            .states
            .iter()
            .find(|row| row.state == member_source.symbol)
            .ok_or(LoweringError::Unsupported(
                "scalar loop lost a fused member row",
            ))?;
        let coordinate = resolve_member(checked, member_machine, member_source, member_row)?;
        members.push(coordinate);
    }
    let member_lookup =
        |state: symbols::SymbolHandle| members.iter().find(|member| member.source.symbol == state);
    let header = member_lookup(source.symbol).ok_or(LoweringError::Unsupported(
        "scalar loop lost its header rank coordinate",
    ))?;

    fn branch(destination: &CheckedScalarBranchDestination) -> Option<&CheckedScalarSuccessor> {
        match destination {
            CheckedScalarBranchDestination::Jump(successor) => Some(successor),
            _ => None,
        }
    }
    let mut covered_cyclic_edges = Vec::new();
    let mut member_successor_count = 0usize;
    // Source order is authoritative, not retained witness order or emitted
    // block order. Rejoin every authored backedge before deriving its row;
    // a transition landing outside the member set leaves the cycle and is not
    // a covered cyclic edge.
    for member in &members {
        let member_row = graph
            .states
            .iter()
            .find(|row| row.state == member.source.symbol)
            .expect("member row checked above");
        let successors = match &member_row.terminator {
            CheckedScalarStateTerminator::Jump(successor) => [Some(successor), None],
            CheckedScalarStateTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => [branch(when_true), branch(when_false)],
            _ => [None, None],
        };
        member_successor_count += successors
            .iter()
            .flatten()
            .filter(|successor| member_lookup(successor.target).is_some())
            .count();
        for (statement_ordinal, statement) in checked
            .statement_table
            .statements(member.source.statement_nodes)
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
                    continue;
                };
                let normalized = source_custody::successors::normalize_machine_state_target(
                    checked,
                    member.machine,
                    path.symbol,
                )?;
                if member_lookup(normalized).is_none() {
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
                        && successor.target == normalized
                });
                let successor = matching.next().ok_or(LoweringError::Unsupported(
                    "scalar natural rank lost an authored successor",
                ))?;
                if matching.next().is_some() {
                    return unsupported("scalar natural rank duplicated an authored successor");
                }
                source_custody::validate_successor(checked, member.source.symbol, successor)?;
                let target_member = member_lookup(normalized).ok_or(LoweringError::Unsupported(
                    "scalar natural rank lost a fused member coordinate",
                ))?;
                // Both descriptors name the exact subject in the existing Nat
                // judgment. The argument ordinal is authored, while the three
                // parameter indices are dense scalar indices. Actual expression
                // custody and Natural verification separately preserve and prove
                // the selected guard/decrement, including private evaluation steps.
                covered_cyclic_edges.push(CheckedStructuralRankedSccEdgePlan {
                    source_state: member.source.symbol,
                    target_state: normalized,
                    statement_ordinal,
                    guard: CheckedStructuralRankedGuardPlan::UnsignedParameterPositive {
                        scalar_parameter_index: member.scalar_position,
                        primitive_type: member.primitive,
                    },
                    successor_argument:
                        CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne {
                            argument_ordinal: target_member.argument_ordinal,
                            source_scalar_parameter_index: member.scalar_position,
                            target_scalar_parameter_index: target_member.scalar_position,
                            primitive_type: member.primitive,
                        },
                });
            }
        }
    }
    // The checker retains covered edges in one canonical order independent of
    // which member the fused worklist visited first; rejoin in the same order.
    covered_cyclic_edges.sort_by_key(|edge| {
        (
            edge.source_state.arena_index(),
            edge.source_state.generation(),
            edge.target_state.arena_index(),
            edge.target_state.generation(),
            edge.statement_ordinal,
        )
    });
    if covered_cyclic_edges.is_empty()
        || covered_cyclic_edges.len() != member_successor_count
        || *rank
            != (CheckedStructuralRankedSccPlan {
                header_state: source.symbol,
                rank_scalar_parameter_index: header.scalar_position,
                rank_primitive_type: header.primitive,
                rank_lower_bound: 0,
                rank_upper_bound: header.upper_bound,
                covered_cyclic_edges,
            })
    {
        return unsupported("scalar loop ranking differs from its complete authored rank roster");
    }
    Ok(())
}

/// Rejoin component evidence through the same runtime ranking judgment, then
/// independently rejoin each executable successor to its authored call site.
fn validate_runtime_rank(
    checked: &CheckedTrees,
    graph: &CheckedScalarMachineGraph,
    rank: &CheckedStructuralRankedSccPlan,
) -> Result<(), LoweringError> {
    let components = validation::validated_runtime_natural_components(checked);
    let mut matching = components.iter().filter(|component| {
        component
            .states
            .iter()
            .any(|state| state.state == rank.header_state)
    });
    let component = matching.next().ok_or(LoweringError::Unsupported(
        "scalar loop lost its exact runtime ranking component",
    ))?;
    if matching.next().is_some() {
        return unsupported("scalar loop has ambiguous runtime ranking components");
    }
    let coordinate = |state| -> Result<_, LoweringError> {
        let subject = component
            .states
            .iter()
            .find(|subject| subject.state == state)
            .ok_or(LoweringError::Unsupported(
                "scalar loop lost its runtime rank subject",
            ))?;
        let row = graph.states.iter().find(|row| row.state == state).ok_or(
            LoweringError::Unsupported("scalar loop lost a runtime component row"),
        )?;
        let (owner, source) = source_custody::authored_state(checked, state)?;
        source_custody::parameter_storage(checked, owner.symbol, row)?;
        if checked
            .machine_states(owner)
            .first()
            .is_some_and(|entry| entry.symbol == state)
        {
            // The runtime judgment reads typed subject/range roots. Retain
            // the normalized witness metadata's independent join to those
            // roots too; subordinate states use the filtered telescope.
            resolve_member(checked, owner, source, row)?;
        }
        if checked
            .state_parameters(source)
            .get(subject.parameter_position as usize)
            .is_none_or(|parameter| parameter.symbol != subject.parameter)
        {
            return unsupported("scalar runtime rank substituted its authored parameter");
        }
        let position = row
            .scalar_parameters
            .iter()
            .position(|parameter| {
                parameter.source_position == subject.parameter_position
                    && parameter.primitive_type == subject.primitive_type
            })
            .ok_or(LoweringError::Unsupported(
                "scalar loop lost its runtime scalar rank lane",
            ))?;
        Ok((
            subject,
            u32::try_from(position)
                .map_err(|_| LoweringError::Unsupported("scalar runtime rank index exceeds u32"))?,
        ))
    };
    let header = graph
        .states
        .iter()
        .find(|row| {
            component
                .states
                .iter()
                .any(|subject| subject.state == row.state)
        })
        .ok_or(LoweringError::Unsupported(
            "scalar runtime rank lost its header",
        ))?;
    let (header_subject, header_position) = coordinate(header.state)?;
    let ScalarType::Integer(carrier) = terminal_scalar_type(header_subject.primitive_type)? else {
        return unsupported("scalar runtime rank requires an integer carrier");
    };
    let IntegerValue::Unsigned(upper_bound) = carrier.maximum_value() else {
        return unsupported("scalar runtime rank requires an unsigned carrier");
    };
    for subject in &component.states {
        coordinate(subject.state)?;
    }
    let mut edges = Vec::new();
    for edge in &component.edges {
        let (source, source_position) = coordinate(edge.source)?;
        let (target, target_position) = coordinate(edge.target)?;
        let source_row = graph
            .states
            .iter()
            .find(|row| row.state == edge.source)
            .ok_or(LoweringError::Unsupported(
                "scalar runtime rank lost its source row",
            ))?;
        let successors = match &source_row.terminator {
            CheckedScalarStateTerminator::Jump(successor) => [Some(successor), None],
            CheckedScalarStateTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                fn jump(
                    destination: &CheckedScalarBranchDestination,
                ) -> Option<&CheckedScalarSuccessor> {
                    match destination {
                        CheckedScalarBranchDestination::Jump(successor) => Some(successor),
                        _ => None,
                    }
                }
                [jump(when_true), jump(when_false)]
            }
            _ => [None, None],
        };
        let mut matching = successors.into_iter().flatten().filter(|successor| {
            successor.statement_ordinal == edge.statement_ordinal
                && successor.target == edge.target
                && !successor.is_continuation
        });
        let successor = matching.next().ok_or(LoweringError::Unsupported(
            "scalar runtime rank lost its authored successor",
        ))?;
        if matching.next().is_some()
            || source.primitive_type != target.primitive_type
            || source.primitive_type != header_subject.primitive_type
        {
            return unsupported("scalar runtime rank has substituted edge coordinates");
        }
        source_custody::validate_successor(checked, source.state, successor)?;
        edges.push(CheckedStructuralRankedSccEdgePlan {
            source_state: source.state,
            target_state: target.state,
            statement_ordinal: edge.statement_ordinal,
            guard: CheckedStructuralRankedGuardPlan::SourceRankingJudgment,
            successor_argument: CheckedStructuralRankedArgumentPlan::UnsignedParameterComparison {
                argument_ordinal: target.parameter_position,
                source_scalar_parameter_index: source_position,
                target_scalar_parameter_index: target_position,
                primitive_type: source.primitive_type,
                strict: edge.strict,
            },
        });
    }
    edges.sort_by_key(|edge| {
        (
            edge.source_state.arena_index(),
            edge.source_state.generation(),
            edge.target_state.arena_index(),
            edge.target_state.generation(),
            edge.statement_ordinal,
        )
    });
    if *rank
        != (CheckedStructuralRankedSccPlan {
            header_state: header.state,
            rank_scalar_parameter_index: header_position,
            rank_primitive_type: header_subject.primitive_type,
            rank_lower_bound: 0,
            rank_upper_bound: upper_bound,
            covered_cyclic_edges: edges,
        })
    {
        return unsupported("scalar loop ranking differs from its runtime component judgment");
    }
    Ok(())
}

/// Resolve one member's rank subject through its own authored ranking witness
/// and source parameters.
fn resolve_member<'a>(
    checked: &'a CheckedTrees,
    machine: &'a checked_trees::machine::Machine,
    source: &'a checked_trees::state::State,
    state: &checked_trees::CheckedScalarStateGraph,
) -> Result<MemberRank<'a>, LoweringError> {
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
    let states = checked.machine_states(machine);
    let entry = states.first().ok_or(LoweringError::Unsupported(
        "scalar loop rank has no authored entry",
    ))?;
    let entry_parameter = checked
        .state_parameters(entry)
        .iter()
        .find(|parameter| parameter.symbol == name.symbol && !parameter.is_self)
        .ok_or(LoweringError::Unsupported(
            "scalar loop rank names a foreign parameter",
        ))?;
    let parameters = checked.state_parameters(source);
    let source_position = if source.symbol == entry.symbol {
        parameters
            .iter()
            .position(|parameter| parameter.symbol == name.symbol)
    } else {
        // A later header can rename and reorder the entry's rank subject.
        // Rebuild the same entry-role telescope as the range judgment; neither
        // a matching spelling nor the retained plan's index proves this join.
        let required_roles = match custody.rank_range {
            Some(range) => validation::ranking_range_required_symbols(
                checked,
                machine,
                range,
                validation::RankingRangeMeasure::Single(*subject),
                validation::RankingRangePremises::RankInvariant,
            )
            .ok_or(LoweringError::Unsupported(
                "scalar loop rank lost its range endpoint roles",
            ))?,
            None => vec![name.symbol],
        };
        let mappings = validation::discover_state_entry_mappings(
            checked,
            machine,
            name.symbol,
            &required_roles,
        )
        .ok_or(LoweringError::Unsupported(
            "scalar loop rank lost its entry-role telescope",
        ))?;
        let state_position = states
            .iter()
            .position(|state| state.symbol == source.symbol)
            .ok_or(LoweringError::Unsupported(
                "scalar loop rank names a foreign state",
            ))?;
        let roles = mappings
            .get(state_position)
            .ok_or(LoweringError::Unsupported(
                "scalar loop rank lost its state-role telescope",
            ))?;
        let mut matching = parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| !parameter.is_self)
            .zip(roles)
            .filter_map(|((position, _), role)| (*role == name.symbol).then_some(position));
        match (matching.next(), matching.next()) {
            (Some(position), None) => Some(position),
            _ => None,
        }
    }
    .ok_or(LoweringError::Unsupported(
        "scalar loop rank lost its exact carried parameter",
    ))?;
    let parameter = &parameters[source_position];
    if parameter.relevance.is_erased() {
        return unsupported("scalar loop rank names an erased parameter");
    }
    let primitive = checked
        .primitive_type_reference(parameter.type_reference)
        .ok_or(LoweringError::Unsupported(
            "scalar loop rank is not primitive",
        ))?;
    if checked.primitive_type_reference(entry_parameter.type_reference) != Some(primitive) {
        return unsupported("scalar loop rank changed its entry subject carrier");
    }
    let scalar_position = parameters[..source_position]
        .iter()
        .filter(|parameter| {
            // An `[erased]` binding occupies no position in this namespace.
            !parameter.relevance.is_erased()
                && checked
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
    Ok(MemberRank {
        machine,
        source,
        scalar_position,
        argument_ordinal,
        primitive,
        upper_bound,
    })
}
