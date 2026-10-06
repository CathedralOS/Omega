//! Find an authored scalar state graph's natural loops. Each feedback edge
//! must return to a header that dominates it; only a re-entered entry needs
//! invocation-independent loop-header storage, because every later row
//! already binds its values as block parameters.
use super::super::{allocate_dense, place_id};
use super::{
    CheckedScalarBranchDestination, CheckedScalarMachineGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor, CheckedTrees, IntegerValue, LoweringError, ScalarType,
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, terminal_scalar_type, unsupported,
};
use checked_trees::expression::ExpressionNode;
use checked_trees::{CheckedStructuralRankedArgumentPlan, CheckedStructuralRankedSccPlan};
use terminal_psi::StructuralTypeShape;

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
    let (machine, _) = crate::declarations::state_declaration(checked, entry.state)?;
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
            crate::declarations::state_declaration(checked, graph.states[*header].state)?;
        validate_rank(checked, header_machine, header_source, graph, rank)?;
    }
    for header in headers {
        let (header_machine, _) =
            crate::declarations::state_declaration(checked, graph.states[header].state)?;
        let Some(_) = &header_machine.termination_plan.implementation_witness else {
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
        // Ordinary record and fixed-array loans move only the usable binding
        // across the edge, never the caller's storage or an owned disposal
        // obligation. Signature and successor custody independently rejoin
        // the exact referent and access; Terminal replay checks the resulting
        // aliases.
        let borrowed_aggregate = matches!(
            parameter.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) && parameter.multiplicity
            == StructuralMultiplicity::Unrestricted
            && structural_types.iter().any(|declaration| {
                declaration.id == parameter.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }
                    )
            });
        !(plain_owned || shared_view || borrowed_aggregate)
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
    }) {
        return unsupported("scalar loop requires plain owned values or whole supported loans");
    }
    let mut loop_parameters = parameters.to_vec();
    for (position, parameter) in loop_parameters.iter_mut().enumerate() {
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

/// One SCC member, resolved against its own authored source and witness.
struct MemberRank<'a> {
    source: &'a checked_trees::state::State,
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
    // The member set preserves authored edge order: header first, then each
    // covered edge's endpoints.
    let mut member_sources: Vec<(
        &'a checked_trees::machine::Machine,
        &'a checked_trees::state::State,
    )> = vec![(machine, source)];
    for edge in &rank.covered_cyclic_edges {
        for state_symbol in [edge.source_state, edge.target_state] {
            let (member_machine, member_source) =
                crate::declarations::state_declaration(checked, state_symbol)?;
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
        if !graph
            .states
            .iter()
            .any(|row| row.state == member_source.symbol)
        {
            return unsupported("scalar loop lost a fused member row");
        }
        let coordinate = resolve_member(checked, member_machine, member_source)?;
        members.push(coordinate);
    }
    let member_lookup =
        |state: symbols::SymbolHandle| members.iter().find(|member| member.source.symbol == state);
    if member_lookup(source.symbol).is_none() {
        return unsupported("scalar loop lost its header rank coordinate");
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
        let (owner, source) = crate::declarations::state_declaration(checked, state)?;
        if checked
            .machine_states(owner)
            .first()
            .is_some_and(|entry| entry.symbol == state)
        {
            // The runtime judgment reads typed subject/range roots. Retain
            // the normalized witness metadata's independent join to those
            // roots too; subordinate states use the filtered telescope.
            resolve_member(checked, owner, source)?;
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
    let (header_subject, _) = coordinate(header.state)?;
    let ScalarType::Integer(carrier) = terminal_scalar_type(header_subject.primitive_type)? else {
        return unsupported("scalar runtime rank requires an integer carrier");
    };
    let IntegerValue::Unsigned(_) = carrier.maximum_value() else {
        return unsupported("scalar runtime rank requires an unsigned carrier");
    };
    for subject in &component.states {
        coordinate(subject.state)?;
    }
    for edge in &component.edges {
        let (source, _) = coordinate(edge.source)?;
        let (target, _) = coordinate(edge.target)?;
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
        let _ = matching.next().ok_or(LoweringError::Unsupported(
            "scalar runtime rank lost its authored successor",
        ))?;
        if matching.next().is_some()
            || source.primitive_type != target.primitive_type
            || source.primitive_type != header_subject.primitive_type
        {
            return unsupported("scalar runtime rank has substituted edge coordinates");
        }
    }
    Ok(())
}

/// Resolve one member's rank subject through its own authored ranking witness
/// and source parameters.
fn resolve_member<'a>(
    checked: &'a CheckedTrees,
    machine: &'a checked_trees::machine::Machine,
    source: &'a checked_trees::state::State,
) -> Result<MemberRank<'a>, LoweringError> {
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
    match (&witness.rank_range, custody.rank_range) {
        (None, None) => {}
        (Some(_), Some(expression)) => {
            if !checked.expression_table.expression_is_valid(expression) {
                return unsupported("scalar loop lost its typed rank range");
            }
            let ExpressionNode::Range(_) = checked.expression_table.expression(expression) else {
                return unsupported("scalar loop rank range is not an authored range");
            };
        }
        _ => return unsupported("scalar loop witness lost or added an authored rank range"),
    }
    let states = checked.machine_states(machine);
    let entry = states.first().ok_or(LoweringError::Unsupported(
        "scalar loop rank has no authored entry",
    ))?;
    if !checked
        .state_parameters(entry)
        .iter()
        .any(|parameter| parameter.symbol == name.symbol)
    {
        return unsupported("scalar loop rank names a foreign parameter");
    }
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
    let ScalarType::Integer(carrier) = terminal_scalar_type(primitive)? else {
        return unsupported("scalar natural rank requires an unsigned integer carrier");
    };
    let IntegerValue::Unsigned(_) = carrier.maximum_value() else {
        return unsupported("scalar natural rank requires an unsigned integer carrier");
    };
    Ok(MemberRank { source })
}
