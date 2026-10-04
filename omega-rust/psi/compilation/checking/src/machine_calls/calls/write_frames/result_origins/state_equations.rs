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
    TypeReferenceHandle, TypedTrees, instantiate_edge_origins, state_equation,
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
    program: &'program TypedTrees,
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

    let mut known = equations
        .iter()
        .map(|equation| equation.direct.clone())
        .collect::<Vec<_>>();
    let mut edge_results = equations
        .iter()
        .map(|equation| vec![None::<AggregateOrigins>; equation.edges.len()])
        .collect::<Vec<_>>();
    let mut pending = known
        .iter()
        .enumerate()
        .filter_map(|(index, result)| result.as_ref().map(|_| index))
        .collect::<std::collections::VecDeque<_>>();
    // Moved-subtree substitutions use only fixed actuals and prefix evidence,
    // so their finite set grows monotonically. Once those governing moves
    // stabilize, reference/case propagation is monotone too. Replacements below
    // handle the finite earlier retirements without a depth/iteration cutoff.
    while let Some(target_index) = pending.pop_front() {
        for (source_index, equation) in equations.iter().enumerate() {
            for (edge_index, (edge, target)) in equation
                .edges
                .iter()
                .zip(&successors[source_index])
                .enumerate()
            {
                if *target != target_index {
                    continue;
                }
                // Moves mask their destination's symbolic leaf/case inventory.
                // Always substitute the complete target relation, never a delta
                // whose missing move row could resurrect an absent payload.
                let mut target = known[target_index].as_ref()?.clone();
                let mut incoming = AggregateOrigins::default();
                instantiate(
                    program,
                    machine,
                    equation,
                    edge,
                    &mut target,
                    &mut incoming,
                    symbols,
                    inference,
                    include_shared,
                )?;
                // A newly discovered whole move can replace an earlier symbolic
                // leaf. Keep one current contribution per authored edge so that
                // its superseded output does not survive in an append-only union.
                edge_results[source_index][edge_index] = Some(incoming);
                let mut replacement = equation.direct.clone();
                for contribution in edge_results[source_index].iter().flatten() {
                    append_new(
                        replacement.get_or_insert_with(AggregateOrigins::default),
                        contribution.clone(),
                    );
                }
                if !same_relation(known[source_index].as_ref(), replacement.as_ref()) {
                    known[source_index] = replacement;
                    if !pending.contains(&source_index) {
                        pending.push_back(source_index);
                    }
                }
            }
        }
    }
    // An unseeded component has no proven normal result. In particular, its
    // declared reference type cannot manufacture an identity return route.
    known.into_iter().next()?
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
                leaf.local_suffix == other.local_suffix
                    && leaf.local_segments == other.local_segments
                    && leaf.origin == other.origin
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
            prior.local_suffix == leaf.local_suffix
                && prior.local_segments == leaf.local_segments
                && prior.origin == leaf.origin
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
    program: &TypedTrees,
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
        .statements(equation.state.statement_nodes)
        .get(edge.statement_index)?;
    instantiate_edge_origins(
        program,
        machine,
        equation.state,
        machine,
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
    program: &TypedTrees,
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
                    root: if parameter.is_self {
                        machine.symbol
                    } else {
                        parameter.symbol
                    },
                    builtin_coordinates: true,
                    ..Default::default()
                };
                let mut basis =
                    if super::super::type_reference_is_reference(program, parameter.type_reference)
                    {
                        AggregateOrigins {
                            references: vec![ReferenceLeaf {
                                local_suffix: String::new(),
                                local_segments: Vec::new(),
                                origin: Some(FramePlaceOrigin {
                                    path: if parameter.is_self {
                                        "self".to_owned()
                                    } else {
                                        parameter.name.as_str().to_owned()
                                    },
                                    precision: FramePathPrecision::Exact,
                                    source,
                                }),
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
                                    local_suffix: leaf.local_path,
                                    local_segments: leaf.local_segments,
                                    origin: leaf.origin,
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
                    let source_parameter =
                        program
                            .state_parameters(equation.state)
                            .iter()
                            .find(|candidate| {
                                candidate.symbol == source.root
                                    || (candidate.is_self && source.root == machine.symbol)
                            })?;
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
