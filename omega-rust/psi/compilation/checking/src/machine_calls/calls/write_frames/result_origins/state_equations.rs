//! Finite normal-result equations over the existing captured return routes.
//!
//! A backedge reads a provisional relation, never an active body's partial
//! proof. Only actual normal returns seed the least fixed point. Unknown
//! origins remain leaves and cannot disappear as though that route diverged.

use super::super::cycle_substitutions::Substitutions;
use super::super::path_instantiation::aggregate_arguments::AggregateMove;
use super::super::transition_topology::strongly_connected_components;
use super::{
    AggregateOrigins, CollectedStatementPrefix, ExpressionHandle, FrameInference,
    FramePathPrecision, FramePlaceOrigin, Machine, ReferenceLeaf, State, TopLevelSymbols,
    TypeReferenceHandle, instantiate_edge_origins, state_equation,
};

pub(super) struct Edge<'program> {
    pub(super) target: &'program State,
    pub(super) statement_index: usize,
    pub(super) arguments: arena::HandleSpan<ExpressionHandle>,
    pub(super) prefix: CollectedStatementPrefix,
}

pub(super) struct Equation<'program> {
    pub(super) state: &'program State,
    /// None means no direct normal return, not an unknown returned origin.
    pub(super) direct: Option<AggregateOrigins>,
    pub(super) edges: Vec<Edge<'program>>,
}

pub(super) fn solve<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    entry: &'program State,
    expected_result: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
) -> Option<AggregateOrigins> {
    let mut states = vec![entry];
    let mut equations = Vec::new();
    let mut index = 0;
    while index < states.len() {
        let equation = state_equation(
            program,
            machine,
            states[index],
            expected_result,
            symbols,
            inference,
            include_shared,
        )?;
        for edge in &equation.edges {
            if !states
                .iter()
                .any(|state| state.symbol == edge.target.symbol)
            {
                states.push(edge.target);
            }
        }
        equations.push(equation);
        index += 1;
    }
    let successors = equations
        .iter()
        .map(|equation| {
            equation
                .edges
                .iter()
                .map(|edge| {
                    states
                        .iter()
                        .position(|state| state.symbol == edge.target.symbol)
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    finite_substitutions(
        program,
        machine,
        &equations,
        &successors,
        symbols,
        inference,
        include_shared,
    )?;

    // A whole moved subtree masks the destination's symbolic reference/case
    // rows. Solve that finite monotone relation first; its actuals and captured
    // prefixes do not depend on any provisional returned reference rows.
    let move_seeds = equations
        .iter()
        .map(|equation| {
            equation.direct.as_ref().map(|direct| AggregateOrigins {
                moves: direct.moves.clone(),
                ..Default::default()
            })
        })
        .collect();
    /// The shared transfer preamble each propagate phase runs: instantiate the
    /// source equation's edge into a fresh output before the phase's own masks.
    #[allow(clippy::too_many_arguments)]
    fn instantiate_into(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        equations: &[Equation<'_>],
        source: usize,
        edge: &Edge<'_>,
        target: &mut AggregateOrigins,
        symbols: &TopLevelSymbols<'_>,
        inference: &mut FrameInference,
        include_shared: bool,
    ) -> Option<AggregateOrigins> {
        let mut output = AggregateOrigins::default();
        instantiate(
            program,
            machine,
            &equations[source],
            edge,
            target,
            &mut output,
            symbols,
            inference,
            include_shared,
        )?;
        Some(output)
    }

    let moves = propagate(
        &equations,
        &successors,
        move_seeds,
        |source, edge, target| {
            let mut output = instantiate_into(
                program,
                machine,
                &equations,
                source,
                edge,
                target,
                symbols,
                inference,
                include_shared,
            )?;
            output.references.clear();
            output.cases.clear();
            Some(output)
        },
    )?;
    // Start references and cases afresh under the stable move masks. Otherwise
    // a leaf discovered before its mask could circulate through a positive
    // cycle after the original seed is suppressed, yielding a larger fixed point.
    let seeds = equations
        .iter()
        .zip(&moves)
        .map(|(equation, moved)| {
            moved.as_ref().map(|moved| {
                let mut seed = equation.direct.clone().unwrap_or_default();
                seed.moves = moved.moves.clone();
                seed
            })
        })
        .collect();
    let complete = propagate(&equations, &successors, seeds, |source, edge, target| {
        let output = instantiate_into(
            program,
            machine,
            &equations,
            source,
            edge,
            target,
            symbols,
            inference,
            include_shared,
        )?;
        // The second relation cannot invent a moved subtree absent from the
        // completed first relation. Keep the phase boundary independently exact.
        let held = moves[source].as_ref()?;
        if output.moves.iter().any(|moved| {
            !held.moves.iter().any(|other| {
                moved.local_segments == other.local_segments
                    && moved.source == other.source
                    && moved.type_reference == other.type_reference
            })
        }) {
            return None;
        }
        Some(output)
    })?;
    // An unseeded component has no proven normal result. Its declared reference
    // type cannot manufacture an identity return route.
    complete.into_iter().next()?
}

/// Worklist scheduling for each monotone relation, without re-reading source
/// statements. Unknown leaves are ordinary relation members, never missing work.
fn propagate(
    equations: &[Equation<'_>],
    successors: &[Vec<usize>],
    mut known: Vec<Option<AggregateOrigins>>,
    mut transfer: impl FnMut(usize, &Edge<'_>, &mut AggregateOrigins) -> Option<AggregateOrigins>,
) -> Option<Vec<Option<AggregateOrigins>>> {
    let mut pending = known
        .iter()
        .enumerate()
        .filter_map(|(index, result)| result.as_ref().map(|_| index))
        .collect::<std::collections::VecDeque<_>>();
    while let Some(target_index) = pending.pop_front() {
        for (source_index, equation) in equations.iter().enumerate() {
            for (edge, target) in equation.edges.iter().zip(&successors[source_index]) {
                if *target != target_index {
                    continue;
                }
                let mut target = known[target_index].as_ref()?.clone();
                let incoming = transfer(source_index, edge, &mut target)?;
                let mut replacement = known[source_index].clone().unwrap_or_default();
                append_new(&mut replacement, incoming);
                if !same_relation(known[source_index].as_ref(), Some(&replacement)) {
                    known[source_index] = Some(replacement);
                    if !pending.contains(&source_index) {
                        pending.push_back(source_index);
                    }
                }
            }
        }
    }
    Some(known)
}

fn same_relation(left: Option<&AggregateOrigins>, right: Option<&AggregateOrigins>) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return left.is_none() && right.is_none();
    };
    left.references.len() == right.references.len()
        && left.cases.len() == right.cases.len()
        && left.moves.len() == right.moves.len()
        && left.references.iter().all(|leaf| {
            right.references.iter().any(|other| {
                leaf.kind == other.kind
                    && leaf.local_suffix == other.local_suffix
                    && leaf.local_segments == other.local_segments
                    && leaf.may_select == other.may_select
                    && leaf.origin == other.origin
                    && leaf.lifetime_dependencies == other.lifetime_dependencies
            })
        })
        && left.cases.iter().all(|case| right.cases.contains(case))
        && left.moves.iter().all(|moved| {
            right.moves.iter().any(|other| {
                moved.local_segments == other.local_segments
                    && moved.source == other.source
                    && moved.type_reference == other.type_reference
            })
        })
}

fn append_new(held: &mut AggregateOrigins, incoming: AggregateOrigins) {
    for leaf in incoming.references {
        if !held.references.iter().any(|prior| {
            prior.kind == leaf.kind
                && prior.local_suffix == leaf.local_suffix
                && prior.local_segments == leaf.local_segments
                && prior.may_select == leaf.may_select
                && prior.origin == leaf.origin
                && prior.lifetime_dependencies == leaf.lifetime_dependencies
        }) {
            held.references.push(leaf);
        }
    }
    for case in incoming.cases {
        if !held.cases.contains(&case) {
            held.cases.push(case);
        }
    }
    for moved in incoming.moves {
        if !held.moves.iter().any(|prior| {
            prior.local_segments == moved.local_segments
                && prior.source == moved.source
                && prior.type_reference == moved.type_reference
        }) {
            held.moves.push(moved);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn instantiate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    equation: &Equation<'_>,
    edge: &Edge<'_>,
    target: &mut AggregateOrigins,
    output: &mut AggregateOrigins,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
) -> Option<()> {
    let boundary = program
        .statement_table
        .statement_at(equation.state, edge.statement_index)?;
    instantiate_edge_origins(
        program,
        machine,
        equation.state,
        edge.target,
        boundary,
        edge.statement_index,
        program.statement_table.expression_handles(edge.arguments),
        target,
        &edge.prefix,
        output,
        symbols,
        inference,
        include_shared,
    )
}

/// Only substitutions inside a cyclic component can grow without bound.
/// Prove their full reference/owned-origin inventory before iteration. This
/// uses the same edge substitution as returned leaves, including captured
/// stored references and moved aggregates; writes provide no origin authority.
#[allow(clippy::too_many_arguments)]
fn finite_substitutions(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    equations: &[Equation<'_>],
    successors: &[Vec<usize>],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
) -> Option<()> {
    let components = strongly_connected_components(successors);
    let mut substitutions = Substitutions::default();
    for (index, equation) in equations.iter().enumerate() {
        for (edge, target) in equation.edges.iter().zip(&successors[index]) {
            if components[index] != components[*target] {
                continue;
            }
            for parameter in program.state_parameters(edge.target) {
                if program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
                {
                    continue;
                }
                let source = super::super::FrameSourcePlace {
                    root: { parameter.symbol },
                    builtin_coordinates: true,
                    ..Default::default()
                };
                let mut basis =
                    if super::super::type_reference_is_reference(program, parameter.type_reference)
                    {
                        AggregateOrigins {
                            references: vec![ReferenceLeaf {
                                kind: super::super::stored_origins::StoredOriginKind::Reference(
                                    crate::reference_shell(program, parameter.type_reference)?
                                        .access,
                                ),
                                local_suffix: String::new(),
                                local_segments: Vec::new(),
                                may_select: false,
                                origin: Some(FramePlaceOrigin {
                                    path: { parameter.name.as_str().to_owned() },
                                    precision: FramePathPrecision::Exact,
                                    source,
                                }),
                                lifetime_dependencies: Vec::new(),
                            }],
                            ..Default::default()
                        }
                    } else {
                        let declared = super::super::stored_origins::declared_origins_for_query(
                            program,
                            parameter.symbol,
                            parameter.name.as_str(),
                            parameter.type_reference,
                            include_shared,
                        )?;
                        AggregateOrigins {
                            references: declared
                                .references
                                .into_iter()
                                .map(|leaf| ReferenceLeaf {
                                    kind: leaf.kind,
                                    local_suffix: leaf.local_path,
                                    local_segments: leaf.local_segments,
                                    may_select: leaf.may_select,
                                    origin: leaf.origin,
                                    lifetime_dependencies: leaf.lifetime_dependencies,
                                })
                                .collect(),
                            cases: declared.cases,
                            moves: vec![AggregateMove {
                                local_segments: Vec::new(),
                                source,
                                type_reference: parameter.type_reference,
                            }],
                        }
                    };
                let mut actual = AggregateOrigins::default();
                instantiate(
                    program,
                    machine,
                    equation,
                    edge,
                    &mut basis,
                    &mut actual,
                    symbols,
                    inference,
                    include_shared,
                )?;
                let from = substitutions.parameter(edge.target.symbol, parameter.symbol);
                let mut record = |source: &super::super::FrameSourcePlace, coarse: bool| {
                    if !source.builtin_coordinates {
                        return None;
                    }
                    if source.is_static() {
                        return Some(());
                    }
                    let source_parameter = program
                        .state_parameters(equation.state)
                        .iter()
                        .find(|candidate| candidate.symbol == source.root)?;
                    let to =
                        substitutions.parameter(equation.state.symbol, source_parameter.symbol);
                    substitutions.insert(
                        from,
                        to,
                        coarse
                            || !source.segments.is_empty()
                            || !source.recasts.is_empty()
                            || source.representation.is_some(),
                    );
                    Some(())
                };
                for leaf in actual.references {
                    let origin = leaf.origin?;
                    record(
                        &origin.source,
                        origin.precision != FramePathPrecision::Exact,
                    )?;
                }
                for moved in actual.moves {
                    record(&moved.source, false)?;
                }
            }
        }
    }
    substitutions.is_finite().then_some(())
}
