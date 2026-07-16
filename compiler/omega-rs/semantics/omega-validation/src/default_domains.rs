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
            // Conservative aliasing fence: a call may write any place.
            StatementNode::Call(_) => {
                tracked.clear();
                poisoned = true;
            }
            StatementNode::Expression(expression) => {
                if expression_contains_call(program, *expression) {
                    tracked.clear();
                    poisoned = true;
                }
            }
            StatementNode::LocalData(local) => {
                if local.initial_value.is_valid()
                    && expression_contains_call(program, local.initial_value)
                {
                    tracked.clear();
                    poisoned = true;
                }
            }
            _ => {}
        }
    }

    let mut exit_established: Vec<String> = entry_established.to_vec();
    exit_established.extend(
        tracked
            .iter()
            .filter(|place| place.established)
            .map(|place| place.spelling.clone()),
    );
    exit_established.sort();
    exit_established.dedup();

    // Exit valuations: in-state tracked places, plus (when no call poisoned
    // the state) the untouched entry places passing through.
    let mut exit_valuations: Vec<PlaceValuation> = tracked
        .iter()
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
        tracked.push(TrackedPlace {
            spelling,
            definition,
            fields,
            // Rung 2b proved this literal against the domain.
            established: true,
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
        tracked.push(TrackedPlace {
            spelling: receiver_spelling,
            definition,
            fields: seeded_fields,
            // Zero-satisfying data is born established; gated data must
            // earn it (the accepted write below does, since it re-proves
            // the whole domain).
            established: !definition.zero_gated,
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
        match fold_with_valuation(program, &valuation, born_zero, *expression) {
            Some(value) if value != 0 => {}
            Some(_) => {
                all_hold = false;
                diagnostics.push(Diagnostic::error(format!(
                    "write to `{}.{field_name}` violates data `{}`'s default domain: a \
                     `where` fact evaluates FALSE at the post-write valuation (strict \
                     store-time semantics; ch11 windows are the future relaxation)",
                    place.spelling,
                    place.definition.name.as_str()
                )));
            }
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
    // Every fact re-proven at the post-write valuation: the place now
    // satisfies its domain (relevant for a GATED place's access gate).
    if all_hold {
        place.established = true;
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
                && definition.zero_gated
            {
                let established = tracked
                    .iter()
                    .find(|place| place.spelling == receiver_spelling)
                    .map(|place| place.established)
                    .unwrap_or_else(|| {
                        // R2 rung 3 slice 3: established on every path in.
                        entry_established.contains(&receiver_spelling)
                    });
                if !established {
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

/// Render a `self`-rooted place (`self.map`, `self.a.b`); `None` for
/// anything else (parameters arrive with unknown-but-valid valuations, so
/// v1 does not track them).
fn self_place_spelling(program: &TypedTrees, expression: ExpressionHandle) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let first = members.first()?;
            if first.as_str() != "self" {
                return None;
            }
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
