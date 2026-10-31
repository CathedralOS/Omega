//! Exact range custody and occurrence guards enter the shared arithmetic owner.
//!
//! Prefix preservation is independent of where a value-producing call occurs.
//! Complete caller-relative frames cover direct stores and nested operand
//! evaluation before the range owner reuses an entry premise. Only inert
//! evaluations enter its arithmetic context; a disjoint call preserves existing
//! facts but does not establish a fact about its returned value.
use super::super::{ExpressionHandle, ExpressionNode, write_preservation};
use crate::checks::termination::graph;
use crate::checks::termination::ranking::DecreaseMeasure;
use crate::checks::termination::ranking::RankingOrder;
use crate::checks::termination::ranking::patterns;
use crate::flow::expressions;
use typed_trees::state::State;
use typed_trees::statement::TransitionGuardNode;

mod retained_correspondence;
mod state_edges;

/// Failure summary over attempted coherent assignments. A formed recurrent
/// edge that fails descent takes diagnostic priority over an earlier-phase
/// decline from another attempt. This does not certify other arrivals or a
/// complete assignment; neither failure supplies an admitted coordinate.
pub(in crate::checks::termination) enum RankArrivalFailure {
    RangeOrFormation,
    /// The recurrent edge whose formed arrival did not strictly decrease,
    /// by its source and destination states, so the refusal can name it.
    StrictDecrease {
        source: symbols::SymbolHandle,
        destination: symbols::SymbolHandle,
    },
}

#[derive(Clone)]
pub(in crate::checks::termination) struct RankArrivalAdmission {
    pub applications: Vec<(
        symbols::SymbolHandle,
        validation::IntegerApplicationFormation,
    )>,
    pub premises: validation::RankingRangePremises,
    pub states: Vec<validation::RankStateBindings>,
    pub origins: Vec<validation::StateEntryCorrespondence>,
    pub edges: Vec<checked_trees::CheckedRankArrivalEdge>,
}

impl RankArrivalAdmission {
    /// A body with only its initial entry has the declaration's identity
    /// coordinates. There is no local transport to infer or arrival invariant
    /// to assume. External calls still owe their own complete entry formation
    /// and the call-component owner separately judges every recurrent call.
    pub(in crate::checks::termination) fn entry_only(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Option<Self> {
        let [_] = program.machine_states(machine) else {
            return None;
        };
        if !graph::machine_adjacency(program, machine)
            .first()?
            .is_empty()
        {
            return None;
        }
        Self::root_identity(program, machine, Vec::new())
    }

    /// A source proof over the entry declaration frame retains its identity
    /// coordinates. Other states receive no rank bindings only after the shared
    /// executable topology proves they cannot reach a ranked cycle, including
    /// through nested calls or any target-family member. Unknown calls remain
    /// relevant. The caller proves every entry backedge; this constructor
    /// supplies no formation, descent, or callee progress premise.
    pub(in crate::checks::termination) fn root_identity(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        edges: Vec<checked_trees::CheckedRankArrivalEdge>,
    ) -> Option<Self> {
        let states = program.machine_states(machine);
        let entry = states.first()?;
        let relevance = validation::rank_relevance(program, &[machine.symbol])?;
        if states
            .iter()
            .skip(1)
            .any(|state| relevance.requires_rank(state.symbol))
        {
            return None;
        }
        let origins = validation::discover_rank_arrival_correspondence(program, machine)?;
        let coordinates = program
            .state_parameters(entry)
            .iter()
            .map(|parameter| {
                let coordinate = validation::RankSubjectCoordinate {
                    parameter: parameter.symbol,
                    path: Vec::new(),
                };
                validation::RankCoordinateBinding {
                    entry: coordinate.clone(),
                    destination: coordinate,
                }
            })
            .collect();
        let mut selected = vec![validation::RankStateBindings {
            state: entry.symbol,
            coordinates,
        }];
        selected.extend(
            states
                .iter()
                .skip(1)
                .map(|state| validation::RankStateBindings {
                    state: state.symbol,
                    coordinates: Vec::new(),
                }),
        );
        Some(Self {
            applications: Vec::new(),
            premises: validation::RankingRangePremises::RankInvariant,
            states: selected,
            origins,
            edges,
        })
    }
}

pub(super) fn admit(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    measure: DecreaseMeasure,
    order: &RankingOrder,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
) -> Result<RankArrivalAdmission, RankArrivalFailure> {
    let mut failure = RankArrivalFailure::RangeOrFormation;
    admit_complete(
        program,
        machine,
        measure,
        order,
        call_frames,
        accept,
        &mut failure,
    )
    .ok_or(failure)
}

fn admit_complete(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    measure: DecreaseMeasure,
    order: &RankingOrder,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
    failure: &mut RankArrivalFailure,
) -> Option<RankArrivalAdmission> {
    let range = program
        .ranking_expression_custody_for(machine.symbol)
        .and_then(|custody| custody.rank_range)
        .unwrap_or_default();
    if !matches!(
        order,
        RankingOrder::NatDescending
            | RankingOrder::CustomNatDescending { .. }
            | RankingOrder::CustomScalarView { .. }
            | RankingOrder::BoundedDistance
            | RankingOrder::IncreasingTo(_)
            | RankingOrder::SliceLength
            | RankingOrder::CustomStructView { .. }
    ) {
        // Arbitrary custom expressions and lexicographic ranges need their
        // exact produced-value projection, not an assumed operand polynomial.
        return None;
    }
    let measure = match (order, measure) {
        (RankingOrder::IncreasingTo(limit), DecreaseMeasure::Distance { lower, upper })
            if *limit == upper =>
        {
            validation::RankingRangeMeasure::IncreasingTo {
                subject: lower,
                limit: upper,
            }
        }
        (
            RankingOrder::NatDescending | RankingOrder::CustomNatDescending { .. },
            DecreaseMeasure::Single(subject),
        ) => validation::RankingRangeMeasure::Single(subject),
        (
            RankingOrder::CustomScalarView {
                parameter,
                body,
                carrier,
                ..
            },
            DecreaseMeasure::Single(subject),
        ) => validation::RankingRangeMeasure::Computed {
            subject,
            parameter: *parameter,
            body: *body,
            carrier: *carrier,
        },
        (RankingOrder::SliceLength, DecreaseMeasure::Single(subject)) => {
            validation::RankingRangeMeasure::SliceLength(subject)
        }
        // The relational field coordinate follows the declared view's exact
        // projection chain, nested or direct, and reads a borrowed subject's
        // referent; validation re-resolves that chain from the declaration.
        (RankingOrder::CustomStructView { measure, .. }, DecreaseMeasure::Single(subject)) => {
            validation::RankingRangeMeasure::Field {
                subject,
                measure: *measure,
            }
        }
        (RankingOrder::BoundedDistance, DecreaseMeasure::Distance { lower, upper }) => {
            validation::RankingRangeMeasure::Distance { lower, upper }
        }
        _ => return None,
    };
    let states = program.machine_states(machine);
    let Some(root) = states.first() else {
        return None;
    };
    // The entry obligation is independent of every edge's guards. In
    // particular, an acyclic body cannot pass vacuously through an empty SCC.
    let entry_proof = super::super::super::rank_inputs::prove_integer_entry_with_formation(
        program,
        machine,
        range,
        measure,
        call_frames,
    )
    .or_else(|| {
        validation::prove_ranking_range_entry_with_formation(program, machine, root, range, measure)
    })?;
    if !entry_proof.membership_and_pinning {
        return None;
    }
    // The judgment reads only these entry formals' arrival values; every
    // prefix below is checked against the paths that carry them.
    let Some(premise_inputs) =
        validation::ranking_range_premise_symbols(program, machine, range, measure)
    else {
        return None;
    };
    let mut owned_frames = None;
    let frames = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames);
    // A whole graph uses one inductive premise set. A failed edge cannot borrow
    // stronger assumptions from a different, incompletely proved attempt.
    // Prefer a complete entry-invariant closure when it is independently
    // established: later authored Requires may read only that retained regime.
    [
        validation::RankingRangePremises::EntryInvariant,
        validation::RankingRangePremises::RankInvariant,
    ]
    .into_iter()
    .find_map(|premises| {
        let states = prove_edges(
            program,
            machine,
            range,
            measure,
            frames,
            premises,
            &premise_inputs,
            entry_proof.application.clone(),
            accept,
            failure,
        )?;
        Some(RankArrivalAdmission {
            applications: states.applications,
            premises,
            states: states.states,
            origins: states.origins,
            edges: states.edges,
        })
    })
}

/// The formals of `state` whose path must stay unwritten through a prefix:
/// the root's own premise formals, or -- under a discovered telescope -- each
/// selected destination whose entry coordinate is a premise input, together
/// with this state's own arrival-premise observations. A scratch input outside
/// those premises may be stored to before the transition: no hypothesis reads
/// its arrival value, and its edge actual is read live. `None` for
/// `entry_parameters` is the root's identity telescope.
pub(super) fn protected_input_paths(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &State,
    entry_parameters: Option<&validation::RankStateBindings>,
    premise_inputs: &[validation::RankPremiseInput],
) -> Option<Vec<validation::RankProtectedPath>> {
    let mut protected = Vec::new();
    match entry_parameters {
        None => {
            for input in premise_inputs {
                program.state_parameter_by_symbol(state, input.parameter)?;
                validation::protect_rank_premise(
                    &mut protected,
                    validation::rank_protected_path(
                        program,
                        input.parameter,
                        &input.path,
                        input.observation,
                    )?,
                );
            }
        }
        Some(selected) => {
            for binding in &selected.coordinates {
                let inputs = premise_inputs
                    .iter()
                    .filter(|input| input.parameter == binding.entry.parameter)
                    .collect::<Vec<_>>();
                if inputs.is_empty() {
                    continue;
                }
                program.state_parameter_by_symbol(state, binding.destination.parameter)?;
                // An extent reading follows the binding only while the
                // destination still denotes the entry's whole formal; a
                // projected coordinate reads a value of its own.
                let readings =
                    if binding.entry.path.is_empty() && binding.destination.path.is_empty() {
                        inputs
                            .into_iter()
                            .map(|input| (input.path.as_slice(), input.observation))
                            .collect::<Vec<_>>()
                    } else {
                        vec![(
                            binding.destination.path.as_slice(),
                            validation::RankPremiseObservation::Value,
                        )]
                    };
                for (path, observation) in readings {
                    validation::protect_rank_premise(
                        &mut protected,
                        validation::rank_protected_path(
                            program,
                            binding.destination.parameter,
                            path,
                            observation,
                        )?,
                    );
                }
            }
        }
    }
    for premise in validation::ranking_range_state_premise_coordinates(program, machine, state)? {
        validation::protect_rank_premise(
            &mut protected,
            validation::rank_protected_path(
                program,
                premise.coordinate.parameter,
                &premise.coordinate.path,
                premise.observation,
            )?,
        );
    }
    Some(protected)
}

fn prove_edges<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
    range: ExpressionHandle,
    measure: validation::RankingRangeMeasure,
    frames: Option<&validation::CallFrameResolver<'program>>,
    premises: validation::RankingRangePremises,
    premise_inputs: &[validation::RankPremiseInput],
    entry_application: Option<validation::IntegerApplicationFormation>,
    accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
    failure: &mut RankArrivalFailure,
) -> Option<state_edges::SelectedCorrespondence> {
    let states = program.machine_states(machine);
    let Some(root) = states.first() else {
        return None;
    };
    let edges = patterns::edges_to_state(program, root, root.symbol);
    if edges.is_empty() && states.len() == 1 && graph::machine_has_cycle(program, machine) {
        return None;
    }
    let protected = protected_input_paths(program, machine, root, None, premise_inputs)?;
    let mut root_edges = Vec::new();
    let mut root_applications = entry_application
        .into_iter()
        .map(|application| (root.symbol, application))
        .collect::<Vec<_>>();
    for edge in edges {
        let Some(evaluated_prefix) = preserved_entry_prefix(
            program,
            machine,
            root,
            frames,
            edge.statement_ordinal,
            &protected,
        ) else {
            return None;
        };
        let guards = edge
            .guards
            .iter()
            .map(|guard| (guard.expression, guard.holds))
            .collect::<Vec<_>>();
        let Some(proof) = validation::prove_ranking_range_edge(
            program,
            machine,
            root,
            range,
            measure,
            premises,
            &guards,
            &evaluated_prefix,
            edge.arguments,
        ) else {
            return None;
        };
        if !proof.membership_and_pinning {
            return None;
        }
        if !proof.strictly_decreases {
            *failure = RankArrivalFailure::StrictDecrease {
                source: root.symbol,
                destination: root.symbol,
            };
            return None;
        }
        if let Some(application) = proof.arrival_application {
            root_applications.push((root.symbol, application));
        }
        root_edges.push(checked_trees::CheckedRankArrivalEdge {
            source: root.symbol,
            destination: root.symbol,
            statement_ordinal: u32::try_from(edge.statement_ordinal).ok()?,
            target: edge.target,
            non_increasing: proof.non_increasing,
            strictly_decreases: proof.strictly_decreases,
        });
    }
    let mut selected = state_edges::select(
        program,
        machine,
        range,
        measure,
        frames,
        premises,
        premise_inputs,
        &root_edges,
        &root_applications,
        accept,
        failure,
    )?;
    selected.edges.extend(root_edges);
    selected.applications.extend(root_applications);
    Some(selected)
}

fn preserved_entry_prefix<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    frames: Option<&validation::CallFrameResolver<'program>>,
    statement: usize,
    protected: &[validation::RankProtectedPath],
) -> Option<Vec<ExpressionHandle>> {
    let mut evaluated = Vec::new();
    for statement in crate::flow::state_statements(program, state.statement_nodes)
        .iter()
        .take(statement + 1)
    {
        if validation::is_arm_pattern_marker(statement) {
            continue;
        }
        // One frame bar covers assignments, locals, statement calls, guards,
        // and discarded expressions. The complete operand frame includes
        // nested calls, including calls evaluating an indexed store target.
        let frames = frames?;
        if !protected.iter().all(|carrier| {
            write_preservation::prefix_preserves_premise(
                frames,
                machine,
                std::slice::from_ref(statement),
                carrier,
            )
        }) {
            return None;
        }
        let known_evaluation = |expression| {
            pure_guard_or_calls(program, machine, state, expression, 0, &|call| {
                call.target_symbol.is_valid()
            })
        };
        if let Some(assignment) = statement.as_assignment() {
            // Entry hypotheses name the protected premise carriers only. A
            // direct store whose alias-closed frame is disjoint from each of
            // them preserves every hypothesis, whatever it stores into an
            // unprotected mutable input; the frame and the operand evaluation
            // must both be known before reusing them.
            let preserved = validation::place_has_builtin_coordinates(
                program,
                machine,
                Some(state),
                assignment.target,
            ) && known_evaluation(assignment.target)
                && known_evaluation(assignment.value);
            if !preserved {
                return None;
            }
            // Retain selected-operator custody without assuming the stored
            // expression establishes a new entry hypothesis.
            if pure_guard(program, machine, state, assignment.value, 0) {
                evaluated.push(assignment.value);
            }
            continue;
        }
        if let Some(local) = statement.as_local_data() {
            // An unrelated local does not revise the entry
            // telescope. Keep numeric substitution handle-first: local
            // expressions are not promoted into parameter hypotheses.
            // A local's binding is fresh storage no premise carrier can name;
            // its initializer has already met the shared operand-frame bar.
            let fresh_local = local.symbol.is_valid()
                && !program
                    .state_parameter_by_symbol(state, local.symbol)
                    .is_some();
            // An omitted initializer has no operand evaluation. Ordinary local
            // establishment still checks its zero value; this judgment only
            // preserves earlier premises across the fresh storage declaration.
            let preserved = fresh_local
                && (!local.initial_value.is_valid() || known_evaluation(local.initial_value));
            if !preserved {
                return None;
            }
            if pure_guard(program, machine, state, local.initial_value, 0) {
                evaluated.push(local.initial_value);
            }
            continue;
        }
        if let Some(call) = statement.as_call() {
            // The direct call and every nested argument have already met
            // the same frame bar. Its discarded result grants no hypothesis.
            let preserved = program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .all(|argument| known_evaluation(*argument));
            if !preserved {
                return None;
            }
            continue;
        }
        if let Some(expression) = statement.as_expression() {
            // Discarding a value changes no premise; evaluation still owes
            // the same known-callee and complete-frame evidence.
            if !known_evaluation(*expression) {
                return None;
            }
            continue;
        }
        let transition = statement.as_transition()?;
        match transition.guard {
            TransitionGuardNode::Always => {}
            TransitionGuardNode::When(guard) => {
                if !known_evaluation(guard) {
                    return None;
                }
                if pure_guard(program, machine, state, guard, 0) {
                    evaluated.push(guard);
                }
            }
        }
    }
    Some(evaluated)
}

/// An operand is inert when its evaluation cannot perform a write: every
/// reachable call or authored operator is rejected, while builtin place
/// expressions (members, borrows, builtin indexing or subslicing) and builtin
/// value forms are write-free by construction. Meaning and membership of an
/// admitted expression are still proved separately by the range owner.
fn pure_guard(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    pure_guard_or_calls(program, machine, state, expression, depth, &|_| false)
}

/// The same inertness walk with value-position calls admitted per
/// `admit_call`: an accepted call's receiver and arguments recur under the
/// same rule, so nested call arguments and composed initializers are
/// covered, while every subterm that is not a call must still be pure the
/// way it would be with no call present.
fn pure_guard_or_calls(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
    admit_call: &dyn Fn(&typed_trees::expression::TableCallExpression) -> bool,
) -> bool {
    let Some(node) = expressions(program).expression_at_depth(expression, depth) else {
        return false;
    };
    let mut inert = |expression| {
        pure_guard_or_calls(program, machine, state, expression, depth + 1, admit_call)
    };
    match node {
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => true,
        ExpressionNode::Atomic(atomic) => inert(atomic.value),
        ExpressionNode::Unary(unary) => inert(unary.operand),
        ExpressionNode::Cast(cast) => inert(cast.value),
        ExpressionNode::Member(member) => {
            // Projection identity and selected arithmetic meaning are checked
            // by the range owner. A member with a pure receiver performs no call.
            inert(member.receiver)
        }
        ExpressionNode::Binary(binary) => {
            // Only builtin operations and exact sealed IEEE comparisons
            // establish inert evaluation here. Arbitrary selected operators
            // can hide writes; admitting a float guard grants no integer fact.
            (validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) || validation::sealed_float_comparison(
                program,
                machine,
                Some(state),
                expression,
                binary,
            )) && inert(binary.left)
                && inert(binary.right)
        }
        ExpressionNode::Borrow(borrow) => {
            // Forming a reference writes nothing. A later store through the
            // alias is a separate statement whose write frame is closed over
            // the referent's origins by the frame owner.
            inert(borrow.target)
        }
        ExpressionNode::Indexed(indexed) => {
            // A builtin element read or subslice is a pure projection of its
            // collection. An authored `[]` selection can run arbitrary code,
            // and the index operands must be inert as well.
            let index_inert = match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => [range.start, range.end]
                    .into_iter()
                    .all(|endpoint| !endpoint.is_valid() || inert(endpoint)),
                _ => inert(indexed.index),
            };
            index_inert
                && inert(indexed.collection)
                && (validation::place_has_builtin_coordinates(
                    program,
                    machine,
                    Some(state),
                    expression,
                ) || validation::has_builtin_subslice_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                ))
        }
        ExpressionNode::StructLiteral(literal) => expressions(program)
            .struct_fields(literal.fields)
            .iter()
            .all(|field| inert(field.value)),
        ExpressionNode::ArrayLiteral(items) => expressions(program)
            .expression_handles(*items)
            .iter()
            .all(|item| inert(*item)),
        ExpressionNode::Match(dispatch) => {
            expressions(program).match_deep_all(dispatch, &mut inert)
        }
        ExpressionNode::Call(call) => {
            // A call admitted by the caller's bar keeps every subterm pure
            // the way transition actuals are; the caller decides which
            // callees may appear and covers their writes in the aggregate
            // write frame separately.
            admit_call(call)
                && (!call.receiver.is_valid() || inert(call.receiver))
                && expressions(program)
                    .expression_handles(call.arguments)
                    .iter()
                    .all(|argument| inert(*argument))
        }
        _ => false,
    }
}
