//! R2 rung 3 slice 1 (ch12 "Dependent Data"): the default-domain WRITE
//! obligation -- every store to a `where`-mentioned field of a
//! domain-carrying place must leave the facts TRUE at the post-write
//! valuation. This is the strict pre-window semantics (ch11's
//! consumption-point windows are the sanctioned ADDITIVE relaxation);
//! obligations land BEFORE hypotheses on purpose -- over-refusal is safe,
//! over-assumption is not, so readers may not assume the facts until the
//! obligation net is total.
//!
//! V1 tracking model: per-state linear walk over `self`-rooted places
//! (machine-owned data is BORN ZEROED -- ch12's machine-owned rule -- so
//! untracked fields read 0). An integer-literal store tracks its value; a
//! runtime-valued store to a where-mentioned field refuses (the entailment
//! integration relaxes this later); a whole-place struct-literal store
//! reseeds the valuation from the literal (already proven at construction
//! by rung 2b); any CALL statement poisons every tracked valuation
//! (conservative aliasing fence).

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::DataDefinition;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::StatementNode;

pub(crate) fn validate_default_domain_writes(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let states = program.machine_states(machine);
        // Bodyless machines (boundary/requirement declarations) own no
        // states -- nothing to walk.
        if states.is_empty() {
            continue;
        }
        // R2 rung 3 slice 3: CROSS-STATE establishment. Establishment is
        // globally monotone in the strict model (every accepted write
        // anywhere re-proves the domain), so a MUST analysis over the
        // state graph is sound: established at entry of S = established at
        // exit of EVERY predecessor. Bottom-start iteration converges to
        // the LEAST fixpoint -- an UNDER-approximation (loop-carried
        // establishment stays conservative), which only over-refuses.
        let edges = state_edges(program, states);
        // R2 rung 3 slice 4 (SOUNDNESS): untracked fields read the born
        // zero ONLY in the boot state when nothing can re-enter it --
        // machine-owned fields persist, so in any other state an untracked
        // field may hold a prior value and must fold as UNKNOWN (poison ->
        // directed refusal; cross-state valuation transport is the
        // precision rung).
        let born_zero = |index: usize| index == 0 && !edges.iter().any(|(_, to)| *to == 0);
        // R2 rung 3 slice 5: the combined MUST fixpoint -- establishment
        // (as slice 3) and per-place field VALUATIONS (Kildall constant
        // propagation: non-boot entries start TOP/unvisited; meet keeps a
        // field only when every visited predecessor exits it with the SAME
        // literal; establishment survives calls, valuations do not).
        let mut throwaway = Vec::new();
        let mut entry_established: Vec<Vec<String>> = vec![Vec::new(); states.len()];
        let mut entry_valuations: Vec<Option<Vec<PlaceValuation>>> = vec![None; states.len()];
        entry_valuations[0] = Some(Vec::new());
        loop {
            let mut changed = false;
            let exits: Vec<(Vec<String>, Vec<PlaceValuation>)> = states
                .iter()
                .enumerate()
                .map(|(index, state)| {
                    walk_state(
                        program,
                        machine,
                        state,
                        &entry_established[index],
                        entry_valuations[index].as_deref().unwrap_or(&[]),
                        born_zero(index),
                        &mut throwaway,
                    )
                })
                .collect();
            for index in 1..states.len() {
                let predecessors: Vec<usize> = edges
                    .iter()
                    .filter(|(_, to)| *to == index)
                    .map(|(from, _)| *from)
                    .collect();
                if predecessors.is_empty() {
                    continue;
                }
                // Establishment meet (intersection over ALL predecessors).
                let mut established_meet: Option<Vec<String>> = None;
                for predecessor in &predecessors {
                    let exit = &exits[*predecessor].0;
                    established_meet = Some(match established_meet {
                        None => exit.clone(),
                        Some(current) => current
                            .into_iter()
                            .filter(|place| exit.contains(place))
                            .collect(),
                    });
                }
                let established_meet = established_meet.unwrap_or_default();
                if established_meet != entry_established[index] {
                    entry_established[index] = established_meet;
                    changed = true;
                }
                // Valuation meet (over VISITED predecessors only -- the
                // Kildall optimism; unvisited preds resolve as iteration
                // reaches them, only ever REMOVING knowledge).
                let visited: Vec<usize> = predecessors
                    .iter()
                    .copied()
                    .filter(|predecessor| entry_valuations[*predecessor].is_some())
                    .collect();
                if visited.is_empty() {
                    continue;
                }
                let mut valuation_meet: Option<Vec<PlaceValuation>> = None;
                for predecessor in visited {
                    let exit = &exits[predecessor].1;
                    valuation_meet = Some(match valuation_meet {
                        None => exit.clone(),
                        Some(current) => meet_valuations(&current, exit),
                    });
                }
                let valuation_meet = valuation_meet.unwrap_or_default();
                if entry_valuations[index].as_ref() != Some(&valuation_meet) {
                    entry_valuations[index] = Some(valuation_meet);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for (index, state) in states.iter().enumerate() {
            walk_state(
                program,
                machine,
                state,
                &entry_established[index],
                entry_valuations[index].as_deref().unwrap_or(&[]),
                born_zero(index),
                diagnostics,
            );
        }
    }
}

/// One place's transported field valuation (`None` value = known-unknown).
type PlaceValuation = (String, Vec<(String, Option<i128>)>);

/// MUST meet of two exit valuations: a place survives only when present in
/// both; a field survives only when both sides agree on the SAME literal.
fn meet_valuations(left: &[PlaceValuation], right: &[PlaceValuation]) -> Vec<PlaceValuation> {
    let mut result = Vec::new();
    for (spelling, left_fields) in left {
        let Some((_, right_fields)) = right.iter().find(|(name, _)| name == spelling) else {
            continue;
        };
        let mut fields = Vec::new();
        for (field, left_value) in left_fields {
            if let Some((_, right_value)) = right_fields.iter().find(|(name, _)| name == field)
                && left_value == right_value
                && left_value.is_some()
            {
                fields.push((field.clone(), *left_value));
            }
        }
        result.push((spelling.clone(), fields));
    }
    result
}

/// The machine's state-transition edges by state INDEX (Named targets
/// matched by simple state name; Value/Terminal/SelfTarget edges carry no
/// establishment transfer -- SelfTarget re-enters with the same entry set,
/// modeled as a self-edge).
fn state_edges(program: &TypedTrees, states: &[State]) -> Vec<(usize, usize)> {
    use omega_typed_trees::statement::{StatementNode, TransitionTargetNode};
    let mut edges = Vec::new();
    for (from, state) in states.iter().enumerate() {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for handle in [transition.target, transition.continuation] {
                if !handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(handle) {
                    // Resolve by SYMBOL first (the termination graph's proven
                    // rule), name as the fallback.
                    TransitionTargetNode::Named { path, .. } => {
                        let to = states
                            .iter()
                            .position(|candidate| candidate.symbol == path.symbol)
                            .or_else(|| {
                                program
                                    .expression_table
                                    .name_path_members(path.members)
                                    .last()
                                    .and_then(|target_name| {
                                        states.iter().position(|candidate| {
                                            candidate.name == *target_name
                                        })
                                    })
                            });
                        if let Some(to) = to {
                            edges.push((from, to));
                        }
                    }
                    TransitionTargetNode::SelfTarget => edges.push((from, from)),
                    _ => {}
                }
            }
        }
    }
    edges
}

/// One tracked place: its rendered spelling, its data definition, and the
/// per-field valuation (`None` value = written with a non-literal).
struct TrackedPlace<'program> {
    spelling: String,
    definition: &'program DataDefinition,
    fields: Vec<(String, Option<i128>)>,
    /// R2 rung 3 slice 2: the ACCESS GATE. A `zero_gated` place starts
    /// UNESTABLISHED (its zero violates the domain); a proven whole-place
    /// literal or an accepted constrained write establishes it (every
    /// accepted write leaves the facts true). Reads before establishment
    /// refuse. Zero-satisfying places are born established.
    established: bool,
    /// R2 rung 3 slice 6: whether THIS place's untracked fields read the
    /// born zero -- true only for self-rooted machine-owned places in the
    /// never-re-entered boot state. Parameter/local-rooted places arrive
    /// with UNKNOWN valuations (poison until a whole-place literal
    /// reseeds).
    born_zero: bool,
    /// R2 rung 3 slice 8 (ch11): an INVARIANT WINDOW -- a checkable write
    /// left the facts FALSE; every consumption point (a read of the place,
    /// a call, state exit) refuses until a later write folds them true.
    window_open: bool,
}

/// Walk one state (write obligations + the access gate), seeded with the
/// places ESTABLISHED AT ENTRY (the cross-state fixpoint). Returns the
/// EXIT-established spellings (entry-established places stay established:
/// monotone).
fn walk_state(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    entry_established: &[String],
    entry_valuations: &[PlaceValuation],
    born_zero: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Vec<String>, Vec<PlaceValuation>) {
    let mut tracked: Vec<TrackedPlace> = Vec::new();
    // A call statement poisons transported valuations (the callee may write
    // any place); establishment survives (globally monotone).
    let mut poisoned = false;

    for statement in program.statement_table.statements(state.statement_nodes) {
        // R2 rung 3 slice 2: reads of an unestablished GATED place refuse
        // BEFORE this statement's own write effect is applied.
        scan_statement_reads(
            program,
            machine,
            state,
            statement,
            &tracked,
            entry_established,
            diagnostics,
        );
        match statement {
            StatementNode::Assignment(assignment) => {
                handle_assignment(
                    program,
                    machine,
                    state,
                    assignment.target,
                    assignment.value,
                    &mut tracked,
                    entry_valuations,
                    poisoned,
                    born_zero,
                    diagnostics,
                );
            }
            // Conservative aliasing fence: a call may write any place --
            // and OBSERVES state, so it is a consumption point (ch11): any
            // open window must have closed.
            StatementNode::Call(_) => {
                refuse_open_windows(&tracked, "a call", diagnostics);
                tracked.clear();
                poisoned = true;
            }
            StatementNode::Expression(expression) => {
                if expression_contains_call(program, *expression) {
                    refuse_open_windows(&tracked, "a call", diagnostics);
                    tracked.clear();
                    poisoned = true;
                }
            }
            StatementNode::LocalData(local) => {
                if local.initial_value.is_valid()
                    && expression_contains_call(program, local.initial_value)
                {
                    refuse_open_windows(&tracked, "a call", diagnostics);
                    tracked.clear();
                    poisoned = true;
                }
            }
            _ => {}
        }
    }

    // Ch11 (slice 8): STATE EXIT is a consumption point -- an open window
    // may not escape the state (the place would be observable violated).
    refuse_open_windows(&tracked, "state exit", diagnostics);

    let mut exit_established: Vec<String> = entry_established.to_vec();
    exit_established.extend(
        tracked
            .iter()
            // Slice 6: parameters are per-invocation -- only machine-owned
            // places transport across states.
            .filter(|place| place.established && is_self_rooted(&place.spelling))
            .map(|place| place.spelling.clone()),
    );
    exit_established.sort();
    exit_established.dedup();

    // Exit valuations: in-state tracked places, plus (when no call poisoned
    // the state) the untouched entry places passing through.
    let mut exit_valuations: Vec<PlaceValuation> = tracked
        .iter()
        .filter(|place| is_self_rooted(&place.spelling))
        .map(|place| (place.spelling.clone(), place.fields.clone()))
        .collect();
    if !poisoned {
        for (spelling, fields) in entry_valuations {
            if !exit_valuations.iter().any(|(name, _)| name == spelling) {
                exit_valuations.push((spelling.clone(), fields.clone()));
            }
        }
    }
    (exit_established, exit_valuations)
}

/// Ch11 (slice 8): refuse every open invariant window at a consumption
/// point, naming the place and the point.
fn refuse_open_windows(
    tracked: &[TrackedPlace<'_>],
    consumption_point: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for place in tracked.iter().filter(|place| place.window_open) {
        diagnostics.push(Diagnostic::error(format!(
            "data `{}`'s default domain is still FALSE at {consumption_point}: the \
             invariant window opened on `{}` must close first -- restore the \
             `where` facts before this consumption point (ch11)",
            place.definition.name.as_str(),
            place.spelling
        )));
    }
}

fn handle_assignment<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    value: ExpressionHandle,
    tracked: &mut Vec<TrackedPlace<'program>>,
    entry_valuations: &[PlaceValuation],
    poisoned: bool,
    born_zero: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A whole-place store of a struct literal reseeds the valuation (the
    // literal itself was proven at construction, rung 2b).
    if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(value)
        && let Some(spelling) = self_place_spelling(program, target)
        && let Some(definition) = domain_definition_by_name(program, literal.type_name.as_str())
    {
        let fields = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .map(|field| {
                (
                    field.name.as_str().to_string(),
                    integer_literal_value(program, field.value),
                )
            })
            .collect();
        tracked.retain(|place| place.spelling != spelling);
        let place_born_zero = born_zero && is_self_rooted(&spelling);
        tracked.push(TrackedPlace {
            spelling,
            definition,
            fields,
            // Rung 2b proved this literal against the domain.
            established: true,
            born_zero: place_born_zero,
            window_open: false,
        });
        return;
    }

    // A FIELD store: `<self-place>.field = value` where the receiver's type
    // carries where facts.
    let ExpressionNode::Member(member) = program.expression_table.expression(target) else {
        return;
    };
    let Some(receiver_spelling) = self_place_spelling(program, member.receiver) else {
        return;
    };
    let Some(receiver_type) = crate::places::declared_place_type(program, machine, Some(state), member.receiver)
    else {
        return;
    };
    let Some(definition) = data_definition_for_type(program, receiver_type) else {
        return;
    };
    if definition.where_facts.is_empty() {
        return;
    }
    let field_name = member.member.as_str().to_string();
    let written = integer_literal_value(program, value);

    let place = if let Some(position) = tracked
        .iter()
        .position(|place| place.spelling == receiver_spelling)
    {
        &mut tracked[position]
    } else {
        // R2 rung 3 slice 5: seed the fresh place from the transported
        // entry valuation (unless a call poisoned this state's view).
        let seeded_fields = if poisoned {
            Vec::new()
        } else {
            entry_valuations
                .iter()
                .find(|(name, _)| *name == receiver_spelling)
                .map(|(_, fields)| fields.clone())
                .unwrap_or_default()
        };
        let self_rooted = is_self_rooted(&receiver_spelling);
        tracked.push(TrackedPlace {
            spelling: receiver_spelling,
            definition,
            fields: seeded_fields,
            // Zero-satisfying data is born established; gated data must
            // earn it (the accepted write below does, since it re-proves
            // the whole domain). A parameter place arrives ALREADY VALID
            // (the caller's net enforced its domain), so it counts as
            // established for the access gate; its VALUATION stays unknown.
            established: !definition.zero_gated || !self_rooted,
            born_zero: born_zero && self_rooted,
            window_open: false,
        });
        let last = tracked.len() - 1;
        &mut tracked[last]
    };
    place.fields.retain(|(name, _)| *name != field_name);
    place.fields.push((field_name.clone(), written));

    // Obligation: the facts mentioning this field must hold at the
    // post-write valuation.
    if !field_is_where_mentioned(program, place.definition, &field_name) {
        return;
    }
    let valuation: Vec<(&str, Option<i128>)> = place
        .fields
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect();
    let mut all_hold = true;
    for fact in program
        .proof_facts
        .span_or_empty(place.definition.where_facts)
    {
        let omega_typed_trees::domain::ProofFact::Expression(expression) = fact else {
            continue;
        };
        match fold_with_valuation(program, &valuation, place.born_zero, *expression) {
            Some(value) if value != 0 => {}
            // Ch11 (slice 8): a checkable violation OPENS a window instead
            // of refusing -- the consumption points demand closure.
            Some(_) => all_hold = false,
            None => {
                all_hold = false;
                diagnostics.push(Diagnostic::error(format!(
                    "write to `{}.{field_name}` cannot PROVE data `{}`'s default domain: \
                     a `where`-mentioned field's value is not a literal known here (a \
                     runtime value, or a co-field last written in another state) -- \
                     restructure with literal stores in one state for now (the \
                     entailment integration and cross-state valuation transport relax \
                     this)",
                    place.spelling,
                    place.definition.name.as_str()
                )));
            }
        }
    }
    // Every fact re-proven at the post-write valuation: the place
    // satisfies its domain again (any open window CLOSES; a gated place
    // establishes). A checkable violation leaves the window OPEN for the
    // consumption points to police (ch11).
    if all_hold {
        place.established = true;
        place.window_open = false;
    } else {
        place.window_open = true;
    }
}

/// R2 rung 3 slice 2: refuse reads of an unestablished GATED place. V1
/// scans value-position expressions for member chains whose self-rooted
/// receiver names a tracked-or-fresh gated place; cross-state
/// establishment is not trackable yet and refuses with direction.
fn scan_statement_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reads: Vec<ExpressionHandle> = Vec::new();
    match statement {
        StatementNode::Assignment(assignment) => reads.push(assignment.value),
        StatementNode::Expression(expression) => reads.push(*expression),
        StatementNode::LocalData(local) => {
            if local.initial_value.is_valid() {
                reads.push(local.initial_value);
            }
        }
        StatementNode::Call(call) => reads.extend(
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        ),
        StatementNode::Transition(transition) => {
            if let omega_typed_trees::statement::TransitionGuardNode::When(guard) =
                &transition.guard
            {
                reads.push(*guard);
            }
        }
        _ => {}
    }
    for read in reads {
        scan_expression_reads(
            program,
            machine,
            state,
            read,
            tracked,
            entry_established,
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
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            if let Some(receiver_spelling) = self_place_spelling(program, member.receiver)
                && let Some(receiver_type) =
                    crate::places::declared_place_type(program, machine, Some(state), member.receiver)
                && let Some(definition) = data_definition_for_type(program, receiver_type)
                && !definition.where_facts.is_empty()
            {
                let place = tracked
                    .iter()
                    .find(|place| place.spelling == receiver_spelling);
                let established = place.map(|place| place.established).unwrap_or_else(|| {
                    // R2 rung 3 slice 3: established on every path in.
                    // Slice 10: a parameter/local place arrived domain-VALID
                    // (the caller's total net enforced it) -- arrival
                    // establishes, mirroring the write path's rule.
                    entry_established.contains(&receiver_spelling)
                        || !is_self_rooted(&receiver_spelling)
                });
                // The zero-gate applies only to GATED types (zero-satisfying
                // places are born established).
                if definition.zero_gated && !established {
                    diagnostics.push(Diagnostic::error(format!(
                        "reading `{receiver_spelling}.{}` before data `{}`'s default \
                         domain is established: the zeroed value is not a `{}` \
                         (ch12's access gate) -- construct it on every path first \
                         (the cross-state must-analysis carries establishment)",
                        member.member.as_str(),
                        definition.name.as_str(),
                        definition.name.as_str()
                    )));
                }
                // Ch11 (slice 8): a READ is a consumption point -- an open
                // invariant window must close before it.
                if place.is_some_and(|place| place.window_open) {
                    diagnostics.push(Diagnostic::error(format!(
                        "reading `{receiver_spelling}.{}` inside an OPEN invariant \
                         window: a prior write left data `{}`'s default domain FALSE \
                         -- restore the facts before this consumption point (ch11)",
                        member.member.as_str(),
                        definition.name.as_str()
                    )));
                }
            }
            scan_expression_reads(
                program,
                machine,
                state,
                member.receiver,
                tracked,
                entry_established,
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
                diagnostics,
            );
            scan_expression_reads(
                program,
                machine,
                state,
                binary.right,
                tracked,
                entry_established,
                diagnostics,
            );
        }
        ExpressionNode::Mutable(inner) => {
            scan_expression_reads(
                program,
                machine,
                state,
                *inner,
                tracked,
                entry_established,
                diagnostics,
            );
        }
        ExpressionNode::Call(call) => {
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    *argument,
                    tracked,
                    entry_established,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

/// Render a Name-rooted place (`self.map`, `target`, `local.a`); `None`
/// for computed receivers. Slice 6: parameter/local roots are tracked too
/// -- their writes carry the same obligation; only their VALUATION model
/// differs (no born zero).
fn self_place_spelling(program: &TypedTrees, expression: ExpressionHandle) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            members.first()?;
            Some(
                members
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            )
        }
        ExpressionNode::Member(member) => {
            let receiver = self_place_spelling(program, member.receiver)?;
            Some(format!("{receiver}.{}", member.member.as_str()))
        }
        ExpressionNode::Mutable(inner) => self_place_spelling(program, *inner),
        _ => None,
    }
}

/// Slice 6: the born-zero valuation model applies only to machine-owned
/// (self-rooted) storage.
fn is_self_rooted(spelling: &str) -> bool {
    spelling == "self" || spelling.starts_with("self.")
}

fn domain_definition_by_name<'program>(
    program: &'program TypedTrees,
    name: &str,
) -> Option<&'program DataDefinition> {
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .filter(|definition| !definition.where_facts.is_empty())
}

fn data_definition_for_type<'program>(
    program: &'program TypedTrees,
    handle: omega_typed_trees::types::TypeReferenceHandle,
) -> Option<&'program DataDefinition> {
    use omega_typed_trees::types::TypeReferenceNode;
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { name, .. } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == *name),
        TypeReferenceNode::Reference { referee, .. } => data_definition_for_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            data_definition_for_type(program, *base_type)
        }
        _ => None,
    }
}

fn field_is_where_mentioned(
    program: &TypedTrees,
    definition: &DataDefinition,
    field: &str,
) -> bool {
    program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .any(|fact| match fact {
            omega_typed_trees::domain::ProofFact::Expression(expression) => {
                expression_mentions_name(program, *expression, field)
            }
            _ => false,
        })
}

fn expression_mentions_name(program: &TypedTrees, expression: ExpressionHandle, name: &str) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == name),
        ExpressionNode::Binary(binary) => {
            expression_mentions_name(program, binary.left, name)
                || expression_mentions_name(program, binary.right, name)
        }
        _ => false,
    }
}

fn integer_literal_value(program: &TypedTrees, expression: ExpressionHandle) -> Option<i128> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
        ExpressionNode::Mutable(inner) => integer_literal_value(program, *inner),
        _ => None,
    }
}

/// Fold a where fact over the tracked valuation: tracked fields read their
/// value (a non-literal write poisons), untracked fields read the ZII zero
/// (machine-owned data is born zeroed).
fn fold_with_valuation(
    program: &TypedTrees,
    valuation: &[(&str, Option<i128>)],
    born_zero: bool,
    expression: ExpressionHandle,
) -> Option<i128> {
    use omega_typed_trees::expression::BinaryOperator;
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let last = program
                .expression_table
                .name_path_members(path.members)
                .last()?
                .as_str();
            match valuation.iter().find(|(name, _)| *name == last) {
                Some((_, value)) => *value,
                // SOUNDNESS (slice 4): the born zero is real only in the
                // never-re-entered boot state; elsewhere an untracked field
                // may hold any prior value -- poison the fold.
                None if born_zero => Some(0),
                None => None,
            }
        }
        ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
        ExpressionNode::Binary(binary) => {
            let left = fold_with_valuation(program, valuation, born_zero, binary.left)?;
            let right = fold_with_valuation(program, valuation, born_zero, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => left.checked_add(right),
                BinaryOperator::Subtract => left.checked_sub(right),
                BinaryOperator::Multiply => left.checked_mul(right),
                BinaryOperator::LessOrEqual => Some(i128::from(left <= right)),
                BinaryOperator::Less => Some(i128::from(left < right)),
                BinaryOperator::GreaterOrEqual => Some(i128::from(left >= right)),
                BinaryOperator::Greater => Some(i128::from(left > right)),
                BinaryOperator::Equal => Some(i128::from(left == right)),
                BinaryOperator::NotEqual => Some(i128::from(left != right)),
                BinaryOperator::And => Some(i128::from(left != 0 && right != 0)),
                BinaryOperator::Or => Some(i128::from(left != 0 || right != 0)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn expression_contains_call(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            expression_contains_call(program, binary.left)
                || expression_contains_call(program, binary.right)
        }
        ExpressionNode::Member(member) => expression_contains_call(program, member.receiver),
        ExpressionNode::Mutable(inner) => expression_contains_call(program, *inner),
        _ => false,
    }
}

/// R2 rung 3 slice 7 -- READER HYPOTHESES: the standing where facts refine
/// a field READ's interval. Sound because the write net is TOTAL (every
/// write path re-proves the facts) and gated reads are access-gated, so
/// the facts hold at every legal observation. Bounds come from literals or
/// the co-field's DECLARED range (declared ranges always hold), never from
/// flow values.
pub(crate) fn where_fact_interval(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<crate::arithmetic_domains::Interval> {
    use omega_typed_trees::expression::BinaryOperator;

    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    let receiver_type =
        crate::places::declared_place_type(program, machine, state, member.receiver)?;
    let definition = data_definition_for_type(program, receiver_type)?;
    if definition.where_facts.is_empty() {
        return None;
    }
    let field = member.member.as_str();

    let mut interval = crate::arithmetic_domains::Interval {
        low: None,
        high: None,
    };
    let mut refined = false;
    for fact in program.proof_facts.span_or_empty(definition.where_facts) {
        let omega_typed_trees::domain::ProofFact::Expression(fact_expression) = fact else {
            continue;
        };
        let ExpressionNode::Binary(binary) =
            program.expression_table.expression(*fact_expression)
        else {
            continue;
        };
        // R2 rung 3 slice 10 -- PRODUCT hypotheses (`count * stride <= len`,
        // ch12's canonical shape): when OUR field is one FACTOR of a
        // product bounded above, the field's upper bound is
        // bound.high / co-factor.low (floor) -- SOUND iff the co-factor's
        // lower bound is >= 1 (from its declared range or a sibling
        // literal fact) and the field's primitive is UNSIGNED (>= 0).
        if matches!(
            binary.operator,
            omega_typed_trees::expression::BinaryOperator::LessOrEqual
                | omega_typed_trees::expression::BinaryOperator::Less
        ) && let ExpressionNode::Binary(product) =
            program.expression_table.expression(binary.left)
            && matches!(
                product.operator,
                omega_typed_trees::expression::BinaryOperator::Multiply
            )
        {
            let factor = if side_names_field(program, product.left, field) {
                Some(product.right)
            } else if side_names_field(program, product.right, field) {
                Some(product.left)
            } else {
                None
            };
            if let Some(factor) = factor
                && field_is_unsigned(program, definition, field)
                && let Some(factor_low) = factor_lower_bound(program, definition, factor)
                && factor_low >= 1
                && let Some(bound) = bound_source_interval(program, definition, binary.right)
                && let Some(mut bound_high) = bound.high
            {
                if matches!(
                    binary.operator,
                    omega_typed_trees::expression::BinaryOperator::Less
                ) {
                    bound_high = bound_high.saturating_sub(1);
                }
                let high = bound_high.div_euclid(factor_low);
                interval.high = Some(interval.high.map_or(high, |current| current.min(high)));
                refined = true;
                continue;
            }
        }
        // Identify which side names OUR field; the other side supplies the
        // bound (a literal, or a co-field's declared interval end).
        let (field_side_left, other) = if side_names_field(program, binary.left, field) {
            (true, binary.right)
        } else if side_names_field(program, binary.right, field) {
            (false, binary.left)
        } else {
            continue;
        };
        let Some(other_interval) = bound_source_interval(program, definition, other) else {
            continue;
        };
        // Normalize to `field OP other`.
        let operator = if field_side_left {
            binary.operator
        } else {
            match binary.operator {
                BinaryOperator::Less => BinaryOperator::Greater,
                BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                BinaryOperator::Greater => BinaryOperator::Less,
                BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                other_operator => other_operator,
            }
        };
        match operator {
            BinaryOperator::LessOrEqual => {
                if let Some(high) = other_interval.high {
                    interval.high = Some(interval.high.map_or(high, |current| current.min(high)));
                    refined = true;
                }
            }
            BinaryOperator::Less => {
                if let Some(high) = other_interval.high.and_then(|high| high.checked_sub(1)) {
                    interval.high = Some(interval.high.map_or(high, |current| current.min(high)));
                    refined = true;
                }
            }
            BinaryOperator::GreaterOrEqual => {
                if let Some(low) = other_interval.low {
                    interval.low = Some(interval.low.map_or(low, |current| current.max(low)));
                    refined = true;
                }
            }
            BinaryOperator::Greater => {
                if let Some(low) = other_interval.low.and_then(|low| low.checked_add(1)) {
                    interval.low = Some(interval.low.map_or(low, |current| current.max(low)));
                    refined = true;
                }
            }
            _ => {}
        }
    }
    refined.then_some(interval)
}

fn side_names_field(program: &TypedTrees, expression: ExpressionHandle, field: &str) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == field),
        _ => false,
    }
}

/// The bound-supplying side's SOUND interval: a literal is itself; a
/// co-field name reads its DECLARED range (or full type width) from the
/// data definition's own members.
fn bound_source_interval(
    program: &TypedTrees,
    definition: &DataDefinition,
    expression: ExpressionHandle,
) -> Option<crate::arithmetic_domains::Interval> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => {
            let literal = value.text().parse::<i64>().ok()?;
            Some(crate::arithmetic_domains::Interval {
                low: Some(literal),
                high: Some(literal),
            })
        }
        ExpressionNode::Name(path) => {
            let name = program
                .expression_table
                .name_path_members(path.members)
                .last()?;
            let handle = program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    omega_typed_trees::data::DataMember::Field(data_field)
                        if data_field.name == *name =>
                    {
                        Some(data_field.type_reference)
                    }
                    _ => None,
                })?;
            crate::arithmetic_domains::range_constraint_interval(program, handle)
        }
        _ => None,
    }
}

/// Slice 10: is the definition's named field an UNSIGNED primitive (its
/// values are >= 0 -- the product-division soundness guard)?
fn field_is_unsigned(program: &TypedTrees, definition: &DataDefinition, field: &str) -> bool {
    use omega_typed_trees::types::PrimitiveType;
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(data_field)
                if data_field.name.as_str() == field =>
            {
                program.primitive_type_reference(data_field.type_reference)
            }
            _ => None,
        })
        .is_some_and(|primitive| {
            matches!(
                primitive,
                PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
            )
        })
}

/// Slice 10: a co-factor's LOWER bound -- its declared range, or a sibling
/// literal fact (`stride >= 40` / `40 <= stride`), single level.
fn factor_lower_bound(
    program: &TypedTrees,
    definition: &DataDefinition,
    factor: ExpressionHandle,
) -> Option<i64> {
    use omega_typed_trees::expression::BinaryOperator;
    let ExpressionNode::Name(path) = program.expression_table.expression(factor) else {
        return None;
    };
    let factor_name = program
        .expression_table
        .name_path_members(path.members)
        .last()?
        .as_str();
    if let Some(interval) = bound_source_interval(program, definition, factor)
        && let Some(low) = interval.low
    {
        return Some(low);
    }
    for fact in program.proof_facts.span_or_empty(definition.where_facts) {
        let omega_typed_trees::domain::ProofFact::Expression(expression) = fact else {
            continue;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            continue;
        };
        let bound = match binary.operator {
            BinaryOperator::GreaterOrEqual
                if side_names_field(program, binary.left, factor_name) =>
            {
                integer_literal_value(program, binary.right).map(|value| value as i64)
            }
            BinaryOperator::Greater if side_names_field(program, binary.left, factor_name) => {
                integer_literal_value(program, binary.right)
                    .map(|value| (value as i64).saturating_add(1))
            }
            BinaryOperator::LessOrEqual
                if side_names_field(program, binary.right, factor_name) =>
            {
                integer_literal_value(program, binary.left).map(|value| value as i64)
            }
            BinaryOperator::Less if side_names_field(program, binary.right, factor_name) => {
                integer_literal_value(program, binary.left)
                    .map(|value| (value as i64).saturating_add(1))
            }
            _ => None,
        };
        if let Some(bound) = bound {
            return Some(bound);
        }
    }
    None
}
