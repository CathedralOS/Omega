//! Arithmetic-domain checks (frozen decision 17). Two rules, both OPERAND-driven
//! (the domain lives on each value's type, not the assignment target):
//!
//! - **S2 mixed-domain rejection**: a binary arithmetic op whose operands carry
//!   DIFFERENT explicit domains is illegal (cross with an `as` cast). Literals are
//!   neutral and adopt the other operand's domain.
//! - **S3 exact-by-default enforcement**: an `Exact` (default, undomained) integer
//!   `+`/`-`/`*` must be PROVEN not to overflow its type, else it is a compile
//!   error directing the user to widen (`as`) or pick a domain. Wrapping/
//!   Saturating/Trapping ops have defined overflow behaviour and are exempt.
//!
//! Operand ranges come from declared type bounds (an `i32` is its full range),
//! narrowed for literals to their exact value; the interval engine then bounds
//! the result and checks it fits the result type. (Range-constraint and
//! loop-bound narrowing -- the ergonomics that keep this from being annotation-
//! hell -- are S4.)

use std::collections::BTreeMap;

use omega_core::arithmetic::ArithmeticDomain;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::SignatureContractKind;
use omega_typed_trees::state::State;
use omega_typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

use crate::places::declared_place_type_raw;

/// S4: build a value environment pre-seeded with the integer bounds a machine's
/// `requires` clause places on its parameters (`requires amount <= 100`). Used to
/// seed the ENTRY state's env so param arithmetic with a declared bound stays
/// exact instead of being forced into a domain. Only simple `param <OP> literal`
/// (and the flipped `literal <OP> param`) comparisons are read; anything else is
/// ignored (sound -- a missing bound just falls back to the type width).
pub(crate) fn requires_value_env(program: &TypedTrees, machine: &Machine) -> ValueEnv {
    let mut bounds: BTreeMap<String, (Option<i64>, Option<i64>)> = BTreeMap::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some((name, low, high)) = comparison_bound(program, *expression) {
                let entry = bounds.entry(name).or_insert((None, None));
                // Intersect across facts: tightest lower (max) and upper (min).
                if let Some(low) = low {
                    entry.0 = Some(entry.0.map_or(low, |existing| existing.max(low)));
                }
                if let Some(high) = high {
                    entry.1 = Some(entry.1.map_or(high, |existing| existing.min(high)));
                }
            }
        }
    }
    let mut env = ValueEnv::new();
    for (name, (low, high)) in bounds {
        env.set(name, Interval { low, high });
    }
    env
}

/// Read a `requires` comparison as `(param_name, lower, upper)` -- one of the
/// bounds is `None` (open). `None` when the fact is not a simple
/// `name <OP> literal` / `literal <OP> name` integer comparison.
fn comparison_bound(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(String, Option<i64>, Option<i64>)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if let (Some(name), Some(literal)) = (
        place_path(program, binary.left),
        literal_i64(program, binary.right),
    ) {
        return Some(bound_from(name, binary.operator, literal, true));
    }
    if let (Some(name), Some(literal)) = (
        place_path(program, binary.right),
        literal_i64(program, binary.left),
    ) {
        return Some(bound_from(name, binary.operator, literal, false));
    }
    None
}

/// Convert a single `name <OP> literal` (`name_on_left`) or `literal <OP> name`
/// comparison into a one-sided (or, for `==`, two-sided) bound.
fn bound_from(
    name: String,
    operator: BinaryOperator,
    literal: i64,
    name_on_left: bool,
) -> (String, Option<i64>, Option<i64>) {
    // Normalise to `name <OP> literal` by flipping the operator when the name is
    // on the right.
    let operator = if name_on_left {
        operator
    } else {
        match operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            other => other,
        }
    };
    let (low, high) = match operator {
        BinaryOperator::LessOrEqual => (None, Some(literal)),
        BinaryOperator::Less => (None, Some(literal.saturating_sub(1))),
        BinaryOperator::GreaterOrEqual => (Some(literal), None),
        BinaryOperator::Greater => (Some(literal.saturating_add(1)), None),
        BinaryOperator::Equal => (Some(literal), Some(literal)),
        _ => (None, None),
    };
    (name, low, high)
}

/// The comparison operator whose truth is the LOGICAL NEGATION of `operator`
/// (`>` ⟺ `<=`, ...). `None` for `==`/`!=` (their negation has no single-interval
/// bound). Used to narrow a transition's FALSE arm by the negated guard.
fn negate_comparison(operator: BinaryOperator) -> Option<BinaryOperator> {
    Some(match operator {
        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
        BinaryOperator::NotEqual => BinaryOperator::Equal,
        _ => return None,
    })
}

/// S4 dominating-guard narrowing: a transition arm fires only when its guard
/// holds, so the arm's argument arithmetic can assume that bound. Returns `base`
/// refined by the arm's guard. The desugared arm guard is `<comparison> ==
/// true|false`; the comparison's bound (negated for the `false` arm) is
/// INTERSECTED with the guarded place's type range so a one-sided guard (`n >
/// 0`) keeps the type's other end (else `n - 1` loses its `u32` upper bound).
/// Only simple `place <OP> literal` comparisons narrow; anything else leaves the
/// env unchanged (sound -- the arm's arithmetic then has to prove on its own).
pub(crate) fn guard_narrowed_env(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    guard: &omega_typed_trees::statement::TransitionGuardNode,
    base: &ValueEnv,
) -> ValueEnv {
    use omega_typed_trees::statement::TransitionGuardNode;
    let mut env = base.clone();
    let TransitionGuardNode::When(guard_expr) = guard else {
        return env;
    };
    let ExpressionNode::Binary(equality) = program.expression_table.expression(*guard_expr) else {
        return env;
    };
    if equality.operator != BinaryOperator::Equal {
        return env;
    }
    let ExpressionNode::Boolean(arm_true) = program.expression_table.expression(equality.right)
    else {
        return env;
    };
    narrow_env_by_condition(program, machine, state, &mut env, equality.left, *arm_true);
    env
}

/// S4 fall-through complement (MR2 exact-domain unlock): a guarded
/// transition with a valid target and NO fall-through arm EXITS when its
/// guard holds, so every LATER statement in the state runs under the
/// guard's NEGATION (`transition n == 0 { true -> 7 }` then
/// `-> countdown(n - 1)` may assume n >= 1). Returns `base` refined by the
/// negated guard; same simple-comparison leaves as the arm narrowing.
pub(crate) fn fall_through_narrowed_env(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    guard: &omega_typed_trees::statement::TransitionGuardNode,
    base: &ValueEnv,
) -> ValueEnv {
    use omega_typed_trees::statement::TransitionGuardNode;
    let mut env = base.clone();
    let TransitionGuardNode::When(guard_expr) = guard else {
        return env;
    };
    // The multi-arm desugar wraps `(cmp) == true|false`; a single-arm guard
    // stores the comparison bare. Unwrap when present, else negate the whole
    // expression.
    if let ExpressionNode::Binary(equality) = program.expression_table.expression(*guard_expr)
        && equality.operator == BinaryOperator::Equal
        && let ExpressionNode::Boolean(arm_true) =
            program.expression_table.expression(equality.right)
    {
        narrow_env_by_condition(program, machine, state, &mut env, equality.left, !*arm_true);
        return env;
    }
    narrow_env_by_condition(program, machine, state, &mut env, *guard_expr, false);
    env
}

/// Narrow `env` by a guard condition holding with the given polarity,
/// recursing through the boolean structure: a POSITIVE `a && b` narrows by
/// both conjuncts (each may bound a DIFFERENT place -- `dir >= 0 && dir <= 1`
/// or multi-variable conjunctions both narrow); a NEGATIVE `a || b` narrows by
/// both negated disjuncts (De Morgan). A negative `&&` / positive `||` cannot
/// attribute which side holds, so it leaves the env unchanged (sound). Leaves
/// are the existing simple `place <OP> literal` comparisons.
fn narrow_env_by_condition(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &mut ValueEnv,
    condition: ExpressionHandle,
    positive: bool,
) {
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(condition) else {
        return;
    };
    match comparison.operator {
        BinaryOperator::And if positive => {
            let (left, right) = (comparison.left, comparison.right);
            narrow_env_by_condition(program, machine, state, env, left, true);
            narrow_env_by_condition(program, machine, state, env, right, true);
            return;
        }
        BinaryOperator::Or if !positive => {
            let (left, right) = (comparison.left, comparison.right);
            narrow_env_by_condition(program, machine, state, env, left, false);
            narrow_env_by_condition(program, machine, state, env, right, false);
            return;
        }
        BinaryOperator::And | BinaryOperator::Or => return,
        _ => {}
    }
    let comparison = comparison.clone();
    // Identify the (place, literal) sides.
    let (place_expr, literal, name_on_left) =
        if let Some(literal) = literal_i64(program, comparison.right) {
            (comparison.left, literal, true)
        } else if let Some(literal) = literal_i64(program, comparison.left) {
            (comparison.right, literal, false)
        } else {
            return;
        };
    // A negative arm narrows by the NEGATED comparison. A negated EQUALITY
    // is a point exclusion: it tightens an interval only when an END sits
    // exactly on the excluded literal (`n == 0` refuted with `n: u64` gives
    // n >= 1), handled below after the type/declared intersection.
    let negated_equality = !positive && comparison.operator == BinaryOperator::Equal;
    let operator = if positive {
        comparison.operator
    } else if negated_equality {
        comparison.operator
    } else {
        let Some(negated) = negate_comparison(comparison.operator) else {
            return;
        };
        negated
    };
    let Some(name) = place_path(program, place_expr) else {
        return;
    };
    if negated_equality {
        // Start from the full line; the intersection below brings in the
        // type + declared ranges, then the point exclusion bumps an end.
        let mut interval = Interval {
            low: None,
            high: None,
        };
        if let Some(handle) = declared_place_type_raw(program, machine, state, place_expr) {
            if let Some(type_interval) = program
                .primitive_type_reference(handle)
                .and_then(primitive_range)
            {
                interval = interval.intersect(type_interval);
            }
            if let Some(declared_range) = range_constraint_interval(program, handle) {
                interval = interval.intersect(declared_range);
            }
        }
        if interval.low == Some(literal) {
            interval.low = literal.checked_add(1);
        }
        if interval.high == Some(literal) {
            interval.high = literal.checked_sub(1);
        }
        env.narrow(name, interval);
        return;
    }
    let (_, low, high) = bound_from(name.clone(), operator, literal, name_on_left);
    let mut interval = Interval { low, high };
    // Intersect with the place's type range AND its declared `[a..=b]` range
    // constraint to retain the bounds the guard leaves open. Skipping the
    // DECLARED range here was a live regression: a one-sided `i < 7` on
    // `i: i32 [0..=7]` seeded [i32::MIN, 6] into the env, which SHADOWS the
    // declared [0, 7] in the operand analysis (env wins over the constraint
    // there) -- `7 - i` then "may overflow" even though it provably cannot.
    if let Some(handle) = declared_place_type_raw(program, machine, state, place_expr) {
        if let Some(type_interval) = program
            .primitive_type_reference(handle)
            .and_then(primitive_range)
        {
            interval = interval.intersect(type_interval);
        }
        if let Some(declared_range) = range_constraint_interval(program, handle) {
            interval = interval.intersect(declared_range);
        }
    }
    // `narrow` intersects with anything already established, so a prior
    // conjunct on the SAME place composes: `dir >= 0 && dir <= 1` lands [0, 1].
    env.narrow(name, interval);
}

/// R4 witness mint (out-params as witnesses): a BOUNDARY callee's
/// `ensures <param> <OP> <literal>` bounds the `&mut` OUT-ARGUMENT's place
/// the moment the call returns -- the boundary model's citable fact
/// (design brief: a boundary machine MINTS facts; ensures are the trusted
/// tier the way requires are the checked tier). Called after the call
/// clears the env; each conjunct that names a signature parameter bound by
/// a literal seeds the matching argument place, intersected with the
/// place's type + declared ranges. Conjunctions split; anything else is
/// skipped (sound -- fewer facts).
pub(crate) fn seed_out_param_ensures(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    call: &omega_typed_trees::statement::TableCall,
    signature: &omega_typed_trees::signature::StateSignature,
    env: &mut ValueEnv,
) {
    use omega_typed_trees::domain::ProofFact;
    use omega_typed_trees::signature::SignatureContractKind;
    let arguments = program.statement_table.expression_handles(call.arguments);
    let parameters: Vec<&omega_typed_trees::signature::StateParameter> = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect();
    for contract in program.signature_contracts.span_or_empty(signature.contracts) {
        if !matches!(contract.kind, SignatureContractKind::Ensures) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            seed_ensures_conjunct(program, machine, state, &parameters, arguments, *expression, env);
        }
    }
}

fn seed_ensures_conjunct(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    parameters: &[&omega_typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    conjunct: ExpressionHandle,
    env: &mut ValueEnv,
) {
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(conjunct) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        let (left, right) = (comparison.left, comparison.right);
        seed_ensures_conjunct(program, machine, state, parameters, arguments, left, env);
        seed_ensures_conjunct(program, machine, state, parameters, arguments, right, env);
        return;
    }
    // `param <OP> literal` (param on either side).
    let (param_expr, literal, name_on_left) =
        if let Some(literal) = literal_i64(program, comparison.right) {
            (comparison.left, literal, true)
        } else if let Some(literal) = literal_i64(program, comparison.left) {
            (comparison.right, literal, false)
        } else {
            return;
        };
    let ExpressionNode::Name(path) = program.expression_table.expression(param_expr) else {
        return;
    };
    let [param_name] = program.expression_table.name_path_members(path.members) else {
        return;
    };
    let Some(position) = parameters
        .iter()
        .position(|parameter| parameter.name.as_str() == param_name.as_str())
    else {
        return;
    };
    let Some(argument) = arguments.get(position).copied() else {
        return;
    };
    // The out-argument spells `&mut <place>`; unwrap to the place.
    let place_expr = match program.expression_table.expression(argument) {
        ExpressionNode::Mutable(inner) => *inner,
        _ => argument,
    };
    let Some(place) = place_path(program, place_expr) else {
        return;
    };
    let (_, low, high) = bound_from(place.clone(), comparison.operator, literal, name_on_left);
    let mut interval = Interval { low, high };
    if let Some(handle) = declared_place_type_raw(program, machine, state, place_expr) {
        if let Some(type_interval) = program
            .primitive_type_reference(handle)
            .and_then(primitive_range)
        {
            interval = interval.intersect(type_interval);
        }
        if let Some(declared_range) = range_constraint_interval(program, handle) {
            interval = interval.intersect(declared_range);
        }
    }
    env.narrow(place, interval);
}

/// Seed a NON-ENTRY state's starting env from its SOLE incoming guarded
/// transition: `transition dir >= 0 && dir <= 1 { true -> store() }` means
/// `store`'s body may assume the guard -- the target-state twin of the
/// same-state arm narrowing. Strictly conservative; returns an EMPTY env
/// unless ALL of:
/// - exactly ONE transition arm in the whole machine TARGETS the state
///   (a second entry, from anywhere, disqualifies);
/// - that arm is a guard-TRUE target from a DIFFERENT state (a continuation
///   fires on guard-FALSE with its own polarity; a self-loop back edge is
///   loop-invariant territory, owned by loop_invariants.rs);
/// - the state is never entered by a CALL (statement or value position) --
///   calls carry no guard.
/// Facts seeded here are body-scoped exactly like any env entry: a write to
/// the guarded place inside the body replaces its interval.
pub(crate) fn sole_incoming_guard_env(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
) -> ValueEnv {
    use omega_typed_trees::statement::{StatementNode, TransitionTargetNode};
    let state_name = state.name.as_str();
    let mut entries: Vec<omega_typed_trees::statement::TransitionGuardNode> = Vec::new();
    let mut disqualified = false;

    let target_names_state = |handle: omega_typed_trees::statement::TransitionTargetHandle| {
        if !handle.is_valid() {
            return false;
        }
        match program.statement_table.transition_target(handle) {
            TransitionTargetNode::Named { path, .. } => program
                .statement_table
                .name_path_members(path.members)
                .last()
                .is_some_and(|name| name.as_str() == state_name),
            _ => false,
        }
    };

    for source in program.machine_states(machine) {
        for statement in program.statement_table.statements(source.statement_nodes) {
            match statement {
                StatementNode::Transition(transition) => {
                    if target_names_state(transition.target) {
                        if source.symbol == state.symbol {
                            // A self-loop back edge is loop-invariant territory
                            // (loop_invariants.rs), not entry seeding.
                            disqualified = true;
                        } else {
                            entries.push(transition.guard.clone());
                        }
                    }
                    if target_names_state(transition.continuation) {
                        disqualified = true;
                    }
                }
                StatementNode::Call(call) => {
                    if call.target.as_str() == state_name {
                        disqualified = true;
                    }
                }
                _ => {}
            }
            if statement_expressions_call_state(program, statement, state_name) {
                disqualified = true;
            }
        }
    }

    if disqualified || entries.is_empty() {
        return ValueEnv::new();
    }
    // MULTI-predecessor JOIN: a fact holds at state entry only if EVERY guarded
    // edge implies it -- per place, keep the ones present under every entry's
    // guard, at the interval UNION (the widest value any edge admits).
    // Identical funnel guards join to themselves; differing guards keep their
    // common places at the union of their bounds. An edge whose guard
    // establishes nothing contributes an empty env, emptying the join (sound:
    // that edge admits anything).
    let mut joined =
        guard_narrowed_env(program, machine, Some(state), &entries[0], &ValueEnv::new());
    for guard in &entries[1..] {
        let entry_env = guard_narrowed_env(program, machine, Some(state), guard, &ValueEnv::new());
        joined = joined.join(&entry_env);
    }
    joined
}

/// Whether any CALL expression inside the statement's expression trees targets
/// `state_name` (a value-position state entry, which carries no guard).
fn statement_expressions_call_state(
    program: &TypedTrees,
    statement: &omega_typed_trees::statement::StatementNode,
    state_name: &str,
) -> bool {
    use omega_typed_trees::statement::StatementNode;
    let mut handles: Vec<ExpressionHandle> = Vec::new();
    match statement {
        StatementNode::LocalData(local) => handles.push(local.initial_value),
        StatementNode::Assignment(assignment) => {
            handles.push(assignment.target);
            handles.push(assignment.value);
        }
        StatementNode::Expression(expression) => handles.push(*expression),
        StatementNode::Call(call) => {
            handles.extend(
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        }
        StatementNode::Transition(transition) => {
            if let omega_typed_trees::statement::TransitionGuardNode::When(condition) =
                &transition.guard
            {
                handles.push(*condition);
            }
            for handle in [transition.target, transition.continuation] {
                if !handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(handle) {
                    omega_typed_trees::statement::TransitionTargetNode::Named {
                        arguments, ..
                    } => handles.extend(
                        program
                            .expression_table
                            .expression_handles(*arguments)
                            .iter()
                            .copied(),
                    ),
                    omega_typed_trees::statement::TransitionTargetNode::Value(value) => {
                        handles.push(*value)
                    }
                    _ => {}
                }
            }
        }
    }
    handles
        .into_iter()
        .any(|handle| expression_calls_state(program, handle, state_name))
}

fn expression_calls_state(
    program: &TypedTrees,
    expression: ExpressionHandle,
    state_name: &str,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            if call.target.as_str() == state_name {
                return true;
            }
            let receiver = call.receiver;
            let arguments = call.arguments;
            (receiver.is_valid() && expression_calls_state(program, receiver, state_name))
                || program
                    .expression_table
                    .expression_handles(arguments)
                    .iter()
                    .any(|argument| expression_calls_state(program, *argument, state_name))
        }
        ExpressionNode::Binary(binary) => {
            let (left, right) = (binary.left, binary.right);
            expression_calls_state(program, left, state_name)
                || expression_calls_state(program, right, state_name)
        }
        ExpressionNode::Unary(unary) => {
            expression_calls_state(program, unary.operand, state_name)
        }
        ExpressionNode::Member(member) => {
            expression_calls_state(program, member.receiver, state_name)
        }
        ExpressionNode::Indexed(indexed) => {
            let (collection, index) = (indexed.collection, indexed.index);
            expression_calls_state(program, collection, state_name)
                || expression_calls_state(program, index, state_name)
        }
        ExpressionNode::Cast(cast) => expression_calls_state(program, cast.value, state_name),
        ExpressionNode::Mutable(inner) => expression_calls_state(program, *inner, state_name),
        ExpressionNode::Range(range) => {
            let (start, end) = (range.start, range.end);
            expression_calls_state(program, start, state_name)
                || expression_calls_state(program, end, state_name)
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_calls_state(program, *value, state_name)),
        ExpressionNode::StructLiteral(literal) => {
            let fields = literal.fields;
            program
                .expression_table
                .struct_fields(fields)
                .iter()
                .any(|field| expression_calls_state(program, field.value, state_name))
        }
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_) => false,
    }
}

/// S4 flow-sensitive value environment: the proven interval of each place
/// (`self.field`, local) along the straight-line prefix of a state body. Lets the
/// overflow proof discharge `self.v = 10; self.v += 5` (v is known to be 10, so
/// 15 fits) instead of falling back to the full type range. Conservative: an
/// entry is only present when its value is definitely established on the linear
/// path; on anything we cannot model (a call that may mutate, a branch) the
/// relevant entries are dropped and the place falls back to its type bounds.
#[derive(Debug, Default, Clone)]
pub(crate) struct ValueEnv {
    intervals: BTreeMap<String, Interval>,
}

impl ValueEnv {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Drop all tracked values (after an opaque effect like a call that may
    /// mutate fields through `&mut`, or when leaving the linear prefix).
    pub(crate) fn clear(&mut self) {
        self.intervals.clear();
    }

    fn get(&self, path: &str) -> Option<Interval> {
        self.intervals.get(path).copied()
    }

    fn set(&mut self, path: String, interval: Interval) {
        self.intervals.insert(path, interval);
    }

    /// Intersect a place's tracked interval with `interval` (tightening it).
    /// Used by guard narrowing so an arm's guard refines the env without
    /// discarding a value already proven on the linear path.
    fn narrow(&mut self, path: String, interval: Interval) {
        let merged = match self.intervals.get(&path) {
            Some(existing) => existing.intersect(interval),
            None => interval,
        };
        self.intervals.insert(path, merged);
    }

    /// The JOIN of two envs at a control-flow merge: only places tracked in
    /// BOTH survive, each at the UNION of its intervals (the fact that holds
    /// regardless of which path was taken). Used to seed a multi-predecessor
    /// state from its incoming edge guards.
    pub(crate) fn join(&self, other: &ValueEnv) -> ValueEnv {
        let mut joined = ValueEnv::new();
        for (path, interval) in &self.intervals {
            if let Some(other_interval) = other.intervals.get(path) {
                joined
                    .intervals
                    .insert(path.clone(), interval.union(*other_interval));
            }
        }
        joined
    }
}

/// Walk a value expression, apply the domain + overflow rules to every nested
/// arithmetic binary, and return the expression's proven interval (so the caller
/// can record it for the assigned place). `owner` describes the site.
/// `target_primitive` is the declared integer type of the value's destination
/// (the local/field/return type). It is the FALLBACK integer type for an
/// otherwise-untyped operand tree -- so a bare-literal computation like
/// `let c: u8 = 200 + 100` is range-checked against `u8` (and rejected) instead
/// of slipping through with no primitive.
/// `target_domain` is the destination's declared arithmetic domain. When NEITHER
/// operand of a binary carries a domain, the operation computes INTO the
/// destination and adopts its domain -- the backend already selects the op by
/// the target's domain, so `let v: i32 in Wrapping = t + 100` (t a plain local)
/// is wrapping arithmetic, not an Exact obligation. `Exact` (the default) keeps
/// the S3 proof obligation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_arithmetic_domains(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    env: &ValueEnv,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Interval {
    validate_value_range(
        program,
        machine,
        state,
        expression,
        env,
        target_primitive,
        target_domain,
        owner,
        diagnostics,
    )
    .0
}

/// Like [`validate_arithmetic_domains`] but also returns the expression's source
/// integer primitive (the `None`-for-unknown result). The narrowing check needs
/// it: a value produced by a typed source is ALWAYS within that type's range (a
/// `u32 in Wrapping` sum is a u32 even when its mathematical interval spills past
/// `u32`), so the sound value range is `interval ∩ primitive_range(source)` --
/// intersecting keeps a flow-proven tighter interval while clamping a
/// domain-wrapped over-approximation back to the type.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_value_range(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    env: &ValueEnv,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Interval, Option<PrimitiveType>) {
    if !expression.is_valid() {
        return (Interval::UNBOUNDED, None);
    }
    let analysis = analyze(
        program,
        machine,
        state,
        expression,
        env,
        target_primitive,
        target_domain,
        owner,
        diagnostics,
    );
    (analysis.interval, analysis.primitive)
}

/// The straight-line place path an expression denotes (`self.v`, `count`), for
/// the value environment. `None` for non-place expressions.
pub(crate) fn place_path(program: &TypedTrees, expression: ExpressionHandle) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.is_empty() {
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
            let receiver = place_path(program, member.receiver)?;
            Some(format!("{receiver}.{}", member.member.as_str()))
        }
        _ => None,
    }
}

/// Record an assignment's proven interval into the environment (decision 17 S4).
/// A place whose path cannot be formed (a complex lvalue) just is not tracked.
/// The interval is INTERSECTED with the place's declared `[a..=b]` range before
/// recording: an env entry SHADOWS the declared-range fallback in the operand
/// analysis, so recording an UNBOUNDED interval (an unresolvable initializer)
/// onto a range-declared place would WIDEN its effective range -- the same
/// landmine as the guard-seeding one (`let __hoist: i32 [0..=9] = cells[k].v`
/// recorded unbounded and `__hoist + 5` "may overflow").
pub(crate) fn record_assignment(
    env: &mut ValueEnv,
    path: Option<String>,
    interval: Interval,
    declared_range: Option<Interval>,
) {
    if let Some(path) = path {
        let interval = match declared_range {
            Some(declared) => interval.intersect(declared),
            None => interval,
        };
        env.set(path, interval);
    }
}

/// The declared `[a..=b]` range of a place's type, ONLY when that range is
/// store-enforced (EXACT arithmetic domain; atomics wrap by hardware): the
/// interval an env entry may soundly be intersected with. A `in Wrapping`
/// place can legitimately hold out-of-range values (its declared range is
/// deliberately permissive at stores), so clamping its precise env fact
/// against the range would fabricate an in-range claim -- return None there.
pub(crate) fn enforced_declared_range(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    if is_atomic_type(program, handle)
        || program.arithmetic_domain_for_type_reference(handle) != ArithmeticDomain::Exact
    {
        return None;
    }
    range_constraint_interval(program, handle)
}

/// An integer value range with optional (= unbounded) ends; all arithmetic is
/// checked, so an overflowing corner becomes `None` (unbounded) -- which fails
/// the containment test and so is reported as a possible overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Interval {
    pub(crate) low: Option<i64>,
    pub(crate) high: Option<i64>,
}

impl Interval {
    const UNBOUNDED: Interval = Interval {
        low: None,
        high: None,
    };

    pub(crate) fn low(&self) -> Option<i64> {
        self.low
    }

    pub(crate) fn high(&self) -> Option<i64> {
        self.high
    }

    fn constant(value: i64) -> Self {
        Self {
            low: Some(value),
            high: Some(value),
        }
    }

    fn add(self, other: Self) -> Self {
        Self {
            low: pair(self.low, other.low, i64::checked_add),
            high: pair(self.high, other.high, i64::checked_add),
        }
    }

    fn subtract(self, other: Self) -> Self {
        // [a,b] - [c,d] = [a-d, b-c]
        Self {
            low: pair(self.low, other.high, i64::checked_sub),
            high: pair(self.high, other.low, i64::checked_sub),
        }
    }

    fn multiply(self, other: Self) -> Self {
        let (Some(a), Some(b), Some(c), Some(d)) = (self.low, self.high, other.low, other.high)
        else {
            return Interval::UNBOUNDED;
        };
        let corners = [
            a.checked_mul(c),
            a.checked_mul(d),
            b.checked_mul(c),
            b.checked_mul(d),
        ];
        if corners.iter().any(Option::is_none) {
            return Interval::UNBOUNDED;
        }
        let values: Vec<i64> = corners.into_iter().flatten().collect();
        Self {
            low: values.iter().min().copied(),
            high: values.iter().max().copied(),
        }
    }

    /// `a % b`: the remainder's magnitude is strictly below the divisor's
    /// magnitude (truncated-division semantics: the remainder takes the
    /// dividend's sign). SOUND only when the divisor is provably nonzero with a
    /// finite magnitude bound; otherwise unbounded (as before). This is what lets
    /// `self.seed % 100` feed exact arithmetic (`% 100` is in `[-99, 99]`)
    /// instead of poisoning the enclosing op with an unbounded operand. The
    /// result interval can only SHRINK relative to the old unbounded value, so it
    /// is strictly permissive for any enclosing overflow check (never a new
    /// rejection).
    fn modulo(self, divisor: Self) -> Self {
        let Some(magnitude) = divisor.nonzero_magnitude_bound() else {
            return Interval::UNBOUNDED;
        };
        let bound = magnitude.saturating_sub(1);
        // Remainder sign follows the dividend: a provably non-negative dividend
        // yields a non-negative remainder, a non-positive one a non-positive
        // remainder, an unknown-sign dividend either sign.
        let low = if self.low.is_some_and(|low| low >= 0) {
            0
        } else {
            -bound
        };
        let high = if self.high.is_some_and(|high| high <= 0) {
            0
        } else {
            bound
        };
        Self {
            low: Some(low),
            high: Some(high),
        }
    }

    /// `a / b`: truncated division by a divisor of magnitude >= 1 never grows the
    /// dividend's magnitude and preserves its sign, so the quotient stays within
    /// the dividend's own bounds widened to include 0 (the quotient can reach 0,
    /// e.g. `small / large`). SOUND only when the divisor is provably nonzero;
    /// else unbounded. Like `modulo`, the result can only shrink, so it is
    /// strictly permissive.
    fn divide(self, divisor: Self) -> Self {
        if divisor.nonzero_magnitude_bound().is_none() {
            return Interval::UNBOUNDED;
        }
        let (Some(low), Some(high)) = (self.low, self.high) else {
            return Interval::UNBOUNDED;
        };
        // EXACT quotient bounds for a single-valued POSITIVE divisor
        // (`x / 10`): truncated division is monotone non-decreasing in the
        // dividend for k > 0, so `[lo/k, hi/k]` is tight -- `[0, 99] / 10 =
        // [0, 9]`, which is what lets `let tens: u32 [0..=9] = x / 10`
        // store-prove (the range-containment keystone). Any other divisor
        // shape keeps the magnitude-preserving over-approximation.
        if let (Some(k), Some(k_high)) = (divisor.low, divisor.high)
            && k == k_high
            && k > 0
        {
            return Self {
                low: Some(low / k),
                high: Some(high / k),
            };
        }
        Self {
            low: Some(low.min(0)),
            high: Some(high.max(0)),
        }
    }

    /// `min(a, b)`: the result is <= both operands and >= the smaller of the two
    /// possible values. Unbounded ends behave as the appropriate infinity (a
    /// `None` low is -inf, a `None` high is +inf), so `min(x, 100)` upper-bounds
    /// at 100 even when `x` is unbounded.
    fn min_with(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.min(b)),
                _ => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
        }
    }

    /// `max(a, b)`: the dual of `min_with` -- `max(0, x)` lower-bounds at 0 even
    /// when `x` is unbounded.
    fn max_with(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            },
        }
    }

    /// `Some(max|b|)` when this interval (a divisor) is finite and provably
    /// excludes 0 -- either entirely positive (`low >= 1`) or entirely negative
    /// (`high <= -1`). `None` otherwise (the divisor may be 0 or is unbounded, so
    /// `%`/`/` cannot be magnitude-bounded soundly).
    fn nonzero_magnitude_bound(self) -> Option<i64> {
        let (low, high) = (self.low?, self.high?);
        if low >= 1 || high <= -1 {
            Some(low.saturating_abs().max(high.saturating_abs()))
        } else {
            None
        }
    }

    /// Widest interval covering both (`[min(lows), max(highs)]`, an unbounded end
    /// on EITHER side making that side unbounded). Used to union a callee's
    /// multiple return paths when inferring its return range.
    fn union(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.min(b)),
                _ => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            },
        }
    }

    /// Tightest interval contained in both (`[max(lows), min(highs)]`, an
    /// unbounded end deferring to the other). Used to intersect a guard bound
    /// with a place's type range.
    fn intersect(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
        }
    }

    /// Does `self` (a type's range) fully contain `inner` (a computed value
    /// range)? An unbounded `inner` end against a bounded `self` end is NOT
    /// contained -- the value might exceed the type.
    fn contains(self, inner: Interval) -> bool {
        let low_ok = match (self.low, inner.low) {
            (Some(bound), Some(value)) => value >= bound,
            (Some(_), None) => false,
            (None, _) => true,
        };
        let high_ok = match (self.high, inner.high) {
            (Some(bound), Some(value)) => value <= bound,
            (Some(_), None) => false,
            (None, _) => true,
        };
        low_ok && high_ok
    }
}

fn pair(left: Option<i64>, right: Option<i64>, op: fn(i64, i64) -> Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(a), Some(b)) => op(a, b),
        _ => None,
    }
}

/// Decision 17 at a VALUE-BINDING boundary (`self.f = v`, `let x: T = v`): a
/// value whose proven range does not fit the destination integer type is a
/// SILENT NARROWING (truncation). Storing a wider value into a narrower slot is
/// the same proof obligation as an overflowing `+` -- prove it fits or opt into
/// an explicit `as` cast. Flagged ONLY when the source interval is fully bounded
/// AND provably escapes the target range: an unbounded end (a call result, a
/// param, a `u64` high) stays permissive, exactly as exact arithmetic leaves
/// unbounded unknowns unchecked. A `Cast` re-ranges to the target (its interval
/// is the target's own range), so `v as i32` always fits -- the escape hatch.
/// Signedness falls out for free: `i32 -> u32` is caught on the negative half,
/// `u32 -> i32` on the upper half. `target` is `None` (non-primitive or
/// non-integer destination) => nothing to prove.
pub(crate) fn check_narrowing_assignment(
    target: Option<PrimitiveType>,
    value: Interval,
    source: Option<PrimitiveType>,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(primitive) = target else {
        return;
    };
    let Some(range) = primitive_range(primitive) else {
        return;
    };
    // The value produced by a typed source is always WITHIN that type's range
    // (even a `Wrapping`/`Saturating` result), so intersect the mathematical
    // interval with the source type's range: a flow-proven `[7, 7]` survives
    // (tighter than the type), while a domain-wrapped over-approximation is
    // clamped back to the source type -- which, if it fits the target, is no
    // narrowing at all.
    let effective = match source.and_then(primitive_range) {
        Some(source_range) => value.intersect(source_range),
        None => value,
    };
    // Only a fully-bounded source can be PROVEN out of range; leave an unbounded
    // (unknown) end permissive rather than turn "unknown" into a spurious error.
    if effective.low().is_some() && effective.high().is_some() && !range.contains(effective) {
        diagnostics.push(Diagnostic::error(format!(
            "narrowing store in {owner} may not fit `{}`: the value is not provably \
             in range (decision 17 -- a narrowing store is a proof obligation, like exact \
             arithmetic). Truncate explicitly with an `as` cast, or constrain the source's \
             range.",
            primitive_name(primitive),
        )));
    }
}

/// Report the decision-17 narrowing-store obligation for a `value` flowing into a
/// `target` scalar slot: analyze the value's interval (honoring the flow facts in
/// `env`) and flag it if it may not fit the target's width. The value's OWN
/// arithmetic obligations are reported by the normal statement walk, so they go to
/// a THROWAWAY buffer here -- only the narrowing check contributes to `diagnostics`.
/// SINGLE SOURCE OF TRUTH for the "does this value fit its typed scalar slot?"
/// obligation, shared by every store position: call/transition arguments,
/// struct-literal field construction, and array-literal elements. Pass the
/// statement `value_env` for flow-sensitive positions, or `&ValueEnv::new()` where
/// no per-statement env is threaded (construction).
pub(crate) fn check_value_narrowing(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    target: PrimitiveType,
    env: &ValueEnv,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut throwaway = Vec::new();
    let (interval, source) = validate_value_range(
        program,
        machine,
        state,
        value,
        env,
        Some(target),
        ArithmeticDomain::Exact,
        owner,
        &mut throwaway,
    );
    if throwaway.is_empty() {
        check_narrowing_assignment(Some(target), interval, source, owner, diagnostics);
    }
}

/// Narrowing check for a data field DEFAULT (`b: i8 = 300`), which is always a
/// literal/const. Machine-free: an integer literal contributes its exact interval,
/// checked against the field's target type via the shared `check_narrowing_assignment`
/// core. A non-integer-literal default (a `const` name, a computed expression) is left
/// to the value's own analysis and skipped here -- so this only ever rejects the
/// unambiguous literal-out-of-range case.
pub(crate) fn check_literal_default_narrowing(
    program: &TypedTrees,
    value: ExpressionHandle,
    target: PrimitiveType,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut node = program.expression_table.expression(value);
    while let ExpressionNode::Mutable(inner) = node {
        node = program.expression_table.expression(*inner);
    }
    let ExpressionNode::Integer(literal) = node else {
        return;
    };
    check_narrowing_assignment(
        Some(target),
        literal_interval(literal),
        None,
        owner,
        diagnostics,
    );
}

/// The interval of an anonymous literal (D14). A literal that fits i64 is a
/// point; an oversize u64-magnitude literal is honestly over-approximated as
/// "above i64::MAX" (or "below i64::MIN" for its folded negation), so
/// narrowing checks against i64-bounded targets still REJECT by interval
/// math alone -- never by silently skipping.
fn literal_interval(literal: &omega_core::literals::IntegerLiteral) -> Interval {
    match literal.value_i64() {
        Some(value) => Interval::constant(value),
        None if !literal.text().starts_with('-') => Interval {
            low: Some(i64::MAX),
            high: None,
        },
        None => Interval {
            low: None,
            high: Some(i64::MIN),
        },
    }
}

/// The representable range of an integer primitive. `None` for non-integers
/// (`bool`/`f32`/`f64`/`String`) and for `u64`/`usize` whose maximum exceeds
/// `i64` (their high end is left unbounded -- an over-approximation that still
/// rejects genuine overflow).
fn primitive_range(primitive: PrimitiveType) -> Option<Interval> {
    let (low, high): (Option<i64>, Option<i64>) = match primitive {
        PrimitiveType::I8 => (Some(i8::MIN as i64), Some(i8::MAX as i64)),
        PrimitiveType::U8 => (Some(0), Some(u8::MAX as i64)),
        PrimitiveType::I16 => (Some(i16::MIN as i64), Some(i16::MAX as i64)),
        PrimitiveType::U16 => (Some(0), Some(u16::MAX as i64)),
        PrimitiveType::I32 => (Some(i32::MIN as i64), Some(i32::MAX as i64)),
        PrimitiveType::U32 => (Some(0), Some(u32::MAX as i64)),
        PrimitiveType::I64 => (Some(i64::MIN), Some(i64::MAX)),
        PrimitiveType::U64 | PrimitiveType::Addr => (Some(0), None),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::String => {
            return None;
        }
    };
    Some(Interval { low, high })
}

/// The bit width of an integer primitive (the F8 shift-count bound), or `None`
/// for non-integer types. `Addr` is excluded deliberately: shifting an address
/// is already rejected by the address-arithmetic model above.
fn integer_bit_width(primitive: PrimitiveType) -> Option<i64> {
    match primitive {
        PrimitiveType::I8 | PrimitiveType::U8 => Some(8),
        PrimitiveType::I16 | PrimitiveType::U16 => Some(16),
        PrimitiveType::I32 | PrimitiveType::U32 => Some(32),
        PrimitiveType::I64 | PrimitiveType::U64 => Some(64),
        PrimitiveType::Addr
        | PrimitiveType::Bool
        | PrimitiveType::F32
        | PrimitiveType::F64
        | PrimitiveType::String => None,
    }
}

/// The float value of a literal operand (through a `Mutable` wrapper), read at
/// its landed format, or `None` when the operand is not a plain float literal.
/// The F4 Exact cast obligation's fold-visible proof source.
fn float_literal_value(program: &TypedTrees, value: ExpressionHandle) -> Option<f64> {
    let mut node = program.expression_table.expression(value);
    while let ExpressionNode::Mutable(inner) = node {
        node = program.expression_table.expression(*inner);
    }
    match node {
        ExpressionNode::Float(literal) => Some(literal.landed_f64()),
        _ => None,
    }
}

/// The integer value of a literal operand (through a `Mutable` wrapper), or `None`
/// when the operand is not a plain integer literal.
fn integer_literal_value(program: &TypedTrees, value: ExpressionHandle) -> Option<i64> {
    let mut node = program.expression_table.expression(value);
    while let ExpressionNode::Mutable(inner) = node {
        node = program.expression_table.expression(*inner);
    }
    match node {
        ExpressionNode::Integer(literal) => literal.value_i64(),
        _ => None,
    }
}

/// Reject a comparison (`==`/`!=`/`<`/`<=`/`>`/`>=`) between an integer-typed value
/// and an integer LITERAL outside that type's range: `self.b == 300` for a `u8` `b`
/// silently TRUNCATED the literal to the operand width (`300 & 0xFF == 44`) and
/// compared `b == 44` -- a confirmed miscompile (native took the `== 44` branch).
/// A literal compared against a value must be a representable value of that value's
/// type. Fires only when one operand resolves to an integer primitive and the other
/// is an integer literal outside its range; two-place, float/bool/text, and in-range
/// pairings are skipped. Sibling of the decision-17 narrowing obligation for stores.
pub(crate) fn report_out_of_range_comparison_literal(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    ) {
        return false;
    }
    for (typed_operand, literal_operand) in [(left, right), (right, left)] {
        let Some(primitive) =
            crate::places::declared_place_type(program, machine, state, typed_operand)
                .and_then(|type_reference| program.primitive_type_reference(type_reference))
        else {
            continue;
        };
        let Some(range) = primitive_range(primitive) else {
            continue;
        };
        let Some(literal) = integer_literal_value(program, literal_operand) else {
            continue;
        };
        let in_range = range.low().is_none_or(|low| literal >= low)
            && range.high().is_none_or(|high| literal <= high);
        if !in_range {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` compares a `{}` value against `{literal}`, which is out of \
                 range for `{}` -- the comparison would silently truncate the literal to the \
                 operand width; compare an in-range value or widen the value with an `as` cast",
                machine.name.as_str(),
                state.map(|state| state.name.as_str()).unwrap_or(""),
                primitive.name(),
                primitive.name(),
            )));
            return true;
        }
    }
    false
}

/// Reject a COMPARISON (`==`/`!=`/`<`/`<=`/`>`/`>=`) or BITWISE (`&`/`|`/`^`)
/// operation between two integer places of DIFFERENT primitive types
/// (`self.i8 == self.i32`, `self.u32 | self.u8`): the backend performs it at the
/// NARROWER operand's width, silently truncating the wider one -- `i8(44) == i32(300)`
/// reads TRUE (`300 & 0xFF == 44`) and `u32(256) | u8(1)` reads `1` not `257` (both
/// confirmed native). Two integer operands must be the SAME type; convert one with an
/// `as` cast. Fires only when BOTH operands resolve to integer primitives
/// (`primitive_range` is `Some`) that differ. NOT arithmetic (`+ - * / %`, whose
/// mismatch is caught by the decision-17 overflow obligation) nor SHIFT (whose right
/// operand is a bit COUNT, not a width-matched value). A literal operand (no declared
/// place type -- handled by the out-of-range check), a float/bool/text operand, and
/// same-type operands are all skipped. Sibling of
/// `report_out_of_range_comparison_literal`.
pub(crate) fn report_mismatched_width_operands(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
    ) {
        return false;
    }
    let operand_integer = |operand| {
        crate::places::declared_place_type(program, machine, state, operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
            .filter(|primitive| primitive_range(*primitive).is_some())
    };
    let (Some(left_primitive), Some(right_primitive)) =
        (operand_integer(left), operand_integer(right))
    else {
        return false;
    };
    if left_primitive == right_primitive {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies `{operator:?}` to a `{}` value and a `{}` value -- the \
         operands have different integer types and the operation would silently truncate the wider \
         one to the narrower width; convert one with an `as` cast so both are the same type",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
        left_primitive.name(),
        right_primitive.name(),
    )));
    true
}

/// The operators whose result is genuine integer arithmetic and can therefore
/// exceed the `{0, 1}` range even when the operands are bools (bool feeds in as
/// its 0/1 value). Excludes bitwise `& | ^` (which preserve `{0, 1}` for `{0, 1}`
/// operands) and comparison/logical ops (which yield a bool). Used both for the
/// overflow analysis here and, via `expression_types`, to classify an arithmetic
/// result as numeric for the cross-class store check.
/// The source spelling of an arithmetic operator, for diagnostics.
fn arithmetic_operator_spelling(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Modulo => "%",
        BinaryOperator::ShiftLeft => "<<",
        BinaryOperator::ShiftRight => ">>",
        _ => "?",
    }
}

pub(crate) fn is_arithmetic(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
    )
}

/// The result of analysing an expression for the domain + overflow rules.
struct Analysis {
    /// The arithmetic domain (`None` = neutral: a literal or `bool` result).
    domain: Option<ArithmeticDomain>,
    /// The value range, for the overflow proof obligation.
    interval: Interval,
    /// The integer primitive type, for the overflow range bound (`None` when it
    /// cannot be determined, e.g. a bare literal).
    primitive: Option<PrimitiveType>,
}

const NEUTRAL: Analysis = Analysis {
    domain: None,
    interval: Interval::UNBOUNDED,
    primitive: None,
};

#[allow(clippy::too_many_arguments)]
fn analyze(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    env: &ValueEnv,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Analysis {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let operator = binary.operator;
            let left = analyze(
                program,
                machine,
                state,
                binary.left,
                env,
                target_primitive,
                target_domain,
                owner,
                diagnostics,
            );
            let right = analyze(
                program,
                machine,
                state,
                binary.right,
                env,
                target_primitive,
                target_domain,
                owner,
                diagnostics,
            );
            if !is_arithmetic(operator) {
                // Comparison / logical `and`/`or`: a `bool` whose integer value is
                // 0 or 1. Its interval is [0, 1] (NOT unbounded) so it does not
                // poison an enclosing arithmetic op -- e.g. the match desugar
                // `d + (s == p) * (v - d)` stays bounded. No arithmetic domain.
                return Analysis {
                    domain: None,
                    interval: Interval {
                        low: Some(0),
                        high: Some(1),
                    },
                    primitive: None,
                };
            }

            // A divisor that is PROVABLY zero (a literal `0`, or a value the prover
            // has pinned to exactly 0) always traps -- the interpreter traps on
            // div/mod-by-zero in every domain, and native `idiv` faults -- so it is
            // dead-wrong code, rejected here like an out-of-range literal. This is
            // the constant case only: a divisor that MIGHT be zero (an interval that
            // merely straddles 0) stays a runtime concern, not a compile error.
            if matches!(operator, BinaryOperator::Divide | BinaryOperator::Modulo)
                && right.interval.low == Some(0)
                && right.interval.high == Some(0)
            {
                let operation = if operator == BinaryOperator::Divide {
                    "division"
                } else {
                    "remainder"
                };
                diagnostics.push(Diagnostic::error(format!(
                    "{operation} by zero in {owner}: the divisor is provably zero, which always \
                     traps at runtime. Remove the operation or use a nonzero divisor."
                )));
            }

            // The index/count/address model (design brief, SETTLED): `addr`
            // composes with COUNTS -- `addr + count` / `count + addr` /
            // `addr - count` offset an address -- and differences with
            // ITSELF (`addr - addr` is the count between two addresses).
            // Every other arithmetic pairing is meaningless: `addr + addr`
            // has no referent (and no representation on CHERI-class targets
            // where addr may be a 128-bit capability), and multiplying,
            // dividing, or shifting an address conflates the axes the model
            // exists to separate. Reject loudly.
            let left_is_addr = left.primitive == Some(PrimitiveType::Addr);
            let right_is_addr = right.primitive == Some(PrimitiveType::Addr);
            if left_is_addr || right_is_addr {
                let legal = match operator {
                    // addr - addr -> count; addr - count -> addr. (count -
                    // addr has left_is_addr == false and stays illegal.)
                    BinaryOperator::Subtract => left_is_addr,
                    // Exactly one side is the address; the other offsets it.
                    BinaryOperator::Add => left_is_addr != right_is_addr,
                    _ => false,
                };
                if !legal {
                    diagnostics.push(Diagnostic::error(format!(
                        "meaningless address arithmetic in {owner}: `addr` composes with \
                         counts (`addr + u64`, `addr - u64`) or differences with itself \
                         (`addr - addr` is the count between two addresses); `{}` over \
                         these operands conflates the address and count axes.",
                        arithmetic_operator_spelling(operator),
                    )));
                }
            }

            // A SHIFT's count operand carries no domain weight: "shift
            // overflow is defined by the domain on which the operator is
            // happening... lhs domain governs, rhs doesn't matter" (owner
            // ruling, 2026-07-13). `wrapped << self.k` takes the LHS domain
            // and the count is just a number -- exempt from the mixed-domain
            // check and from the domain merge below.
            let shift_count_rhs = matches!(
                operator,
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            );

            // S2: a binary mixing two different explicit domains is illegal.
            if !shift_count_rhs
                && let (Some(left_domain), Some(right_domain)) = (left.domain, right.domain)
                && left_domain != right_domain
            {
                diagnostics.push(Diagnostic::error(format!(
                    "mixed arithmetic domains in {owner}: one operand is `{}` and the other is \
                     `{}`. Decision 17 forbids implicit domain mixing -- cross domains with an \
                     explicit `as` cast, or declare both operands in the same domain.",
                    left_domain.name(),
                    right_domain.name(),
                )));
            }

            let domain = if shift_count_rhs {
                left.domain
            } else {
                match (left.domain, right.domain) {
                    (Some(left_domain), Some(right_domain)) => Some(if left_domain
                        == ArithmeticDomain::Exact
                    {
                        right_domain
                    } else {
                        left_domain
                    }),
                    (Some(domain), None) | (None, Some(domain)) => Some(domain),
                    (None, None) => None,
                }
            };
            let interval = match operator {
                BinaryOperator::Add => refine_dependent_product(
                    program,
                    machine,
                    state,
                    binary.left,
                    binary.right,
                    left.interval.add(right.interval),
                ),
                BinaryOperator::Subtract => refine_dependent_subtract(
                    program,
                    machine,
                    state,
                    binary.left,
                    binary.right,
                    left.interval.subtract(right.interval),
                ),
                BinaryOperator::Multiply => refine_dependent_product_factor(
                    program,
                    machine,
                    state,
                    binary.left,
                    binary.right,
                    left.interval.multiply(right.interval),
                ),
                // S4: modulo is bounded by the divisor's magnitude and division
                // never grows the dividend's magnitude -- bounding both lets a
                // `(a % K)` / `(a / K)` result feed exact arithmetic instead of
                // poisoning the enclosing op with an unbounded operand. Neither is
                // overflow-flagged below; the tighter interval is purely a better
                // (still sound) over-approximation for any ENCLOSING op. Shifts
                // stay unbounded.
                BinaryOperator::Modulo => left.interval.modulo(right.interval),
                BinaryOperator::Divide => left.interval.divide(right.interval),
                _ => Interval::UNBOUNDED,
            };
            // Operand primitives win. The destination type is a fallback ONLY when
            // the result is a BOUNDED constant (a bare-literal computation like
            // `let c: u8 = 200 + 100`), so it is range-checked against `c`. An
            // UNbounded result (an unknown operand -- a call result, a param) keeps
            // no primitive and stays unchecked, as before -- the target fallback
            // must not turn "unknown" into a spurious overflow.
            let primitive = left.primitive.or(right.primitive).or_else(|| {
                if interval.low.is_some() && interval.high.is_some() {
                    target_primitive
                } else {
                    None
                }
            });

            // S3: an EXACT (undomained) `+`/`-`/`*` must be provably in range.
            let effective_domain = domain.unwrap_or(ArithmeticDomain::Exact);
            // Abort-as-effect follow-up (owner 2026-07-18): a TRAPPING op
            // whose result interval is provably DISJOINT from its type's
            // range ALWAYS traps at runtime -- legal (the trap is the
            // requested effect, and a trap is never dead), but almost
            // certainly not what the author meant, so it warns.
            if effective_domain == ArithmeticDomain::Trapping
                && matches!(
                    operator,
                    BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                )
                && let Some(primitive) = primitive
                && let Some(range) = primitive_range(primitive)
            {
                let always_above = matches!(
                    (range.high, interval.low),
                    (Some(bound), Some(low)) if low > bound
                );
                let always_below = matches!(
                    (range.low, interval.high),
                    (Some(bound), Some(high)) if high < bound
                );
                if always_above || always_below {
                    diagnostics.push(Diagnostic::warning(format!(
                        "trapping arithmetic in {owner} ALWAYS overflows `{}` -- this \
                         computation traps unconditionally at runtime (the trap is an \
                         effect and will fire even if the result is never used)",
                        primitive_name(primitive),
                    )));
                }
            }
            if effective_domain == ArithmeticDomain::Exact
                && matches!(
                    operator,
                    BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                )
                && let Some(primitive) = primitive
                && let Some(range) = primitive_range(primitive)
                && !range.contains(interval)
            {
                // When an operand is a value-machine CALL, "constrain the operands'
                // range" is unactionable at the call site -- the fix is to annotate
                // the CALLEE's return type. Name it so the user knows where to look.
                let call_hint = overflow_operand_value_call_target(program, machine, binary.left)
                    .or_else(|| {
                        overflow_operand_value_call_target(program, machine, binary.right)
                    })
                    .map(|target| {
                        format!(
                            " Here the operand `{target}(..)` is a value-machine call whose \
                             return range is unproven -- annotate its return type with a range or \
                             domain (e.g. `-> {} in Wrapping`).",
                            primitive_name(primitive)
                        )
                    })
                    .unwrap_or_default();
                // A user who already declared the TARGET's domain (`let v: i32 in
                // Wrapping = t + 100`) needs to hear the operand-driven rule, not
                // "opt into a domain" -- they think they already did.
                let target_hint = if target_domain != ArithmeticDomain::Exact {
                    format!(
                        " The target's `in {domain}` does not re-domain the value expression \
                         (decision 17 is operand-driven): declare the domain on an operand or \
                         intermediate (`let t: {prim} in {domain} = ...`), or re-tag the operand \
                         inline (`x as {prim} in {domain}`).",
                        domain = target_domain.name(),
                        prim = primitive_name(primitive),
                    )
                } else {
                    String::new()
                };
                diagnostics.push(Diagnostic::error(format!(
                    "exact arithmetic in {owner} may overflow `{}`: the operands are not provably \
                     in range (decision 17 -- exact arithmetic is a proof obligation). Widen with \
                     an `as` cast to a larger type, constrain the operands' range (bound a \
                     parameter with a `requires` clause, or narrow a value with a dominating \
                     guard), or opt into a defined-overflow domain \
                     (`{} in Wrapping`/`Saturating`/`Trapping`).{}{}",
                    primitive_name(primitive),
                    primitive_name(primitive),
                    call_hint,
                    target_hint,
                )));
            }

            // F8 -- the shift-COUNT ruling (ch5, settled 2026-07-18): the count is
            // proof-or-policy. Under Exact the count must be PROVABLY in
            // [0, width); Saturating governs value overflow, not operand
            // validity, so its count obligation is Exact's. Wrapping MASKS the
            // count (`k & (width - 1)`) and Trapping TRAPS on an out-of-range
            // count -- both defined, no obligation. The width is the SHIFTED
            // operand's (decision 17 stays operand-driven: an anonymous lhs
            // falls back to the destination primitive, but the lhs DOMAIN
            // governs and a target `in Wrapping` never re-domains the count).
            // The ISA's silent count-masking under Exact is an invented number
            // and never adopted.
            if shift_count_rhs
                && matches!(
                    effective_domain,
                    ArithmeticDomain::Exact | ArithmeticDomain::Saturating
                )
                && let Some(shift_primitive) = left.primitive.or(target_primitive)
                && let Some(width) = integer_bit_width(shift_primitive)
            {
                let provably_in_range = matches!(right.interval.low, Some(low) if low >= 0)
                    && matches!(right.interval.high, Some(high) if high < width);
                if !provably_in_range {
                    // A count that can NEVER be legal (a spelled `1 << 40` on
                    // u32) reads differently from an unproven one.
                    let always_out = matches!(right.interval.low, Some(low) if low >= width)
                        || matches!(right.interval.high, Some(high) if high < 0);
                    let verdict = if always_out {
                        "is provably out of range and can never execute"
                    } else {
                        "is not provably below the operand width"
                    };
                    let saturating_hint = if effective_domain == ArithmeticDomain::Saturating {
                        " (`Saturating` governs value overflow, not count validity -- its count \
                         obligation is Exact's)"
                    } else {
                        ""
                    };
                    diagnostics.push(Diagnostic::error(format!(
                        "shift count in {owner} {verdict} for `{prim}`{saturating_hint}: exact \
                         shifts prove `count < {width}` (ch5 shift-count ruling -- \
                         proof-or-policy). Constrain the count's range (a ranged type, a \
                         `requires` clause, or a dominating guard), or pick a defined-count \
                         policy on the SHIFTED operand (`{prim} in Wrapping` masks the count \
                         to `count & {mask}`; `in Trapping` traps at runtime).",
                        prim = primitive_name(shift_primitive),
                        mask = width - 1,
                    )));
                }
            }

            Analysis {
                domain,
                interval,
                primitive,
            }
        }
        ExpressionNode::Cast(cast) => {
            // A cast re-types its operand, so the outer target does not flow in.
            let source = analyze(
                program,
                machine,
                state,
                cast.value,
                env,
                None,
                ArithmeticDomain::Exact,
                owner,
                diagnostics,
            );
            let primitive = program
                .expression_table
                .name_path_members(cast.target_type)
                .last()
                .and_then(|name| PrimitiveType::from_name(name.as_str()));
            // F4 (the float->int cast ruling): there is NO MODULAR READING
            // of a float, so `f as iN in Wrapping` is a compile error (ch5;
            // the ruling's precedent generalized to the float domain list).
            // Saturating (NaN -> 0, clamp to the target range) and Trapping
            // (trap on NaN/out-of-range) are the defined policies; a bare
            // Exact cast keeps the transitional truncation until float
            // constant tracking can carry the value obligation.
            if cast.domain == ArithmeticDomain::Wrapping
                && matches!(
                    source.primitive,
                    Some(PrimitiveType::F32 | PrimitiveType::F64)
                )
                && primitive.is_some_and(|target| integer_bit_width(target).is_some())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "float-to-int cast `in Wrapping` in {owner}: there is no modular reading \
                     of a float (ch5 cast ruling). Use `in Saturating` (NaN -> 0, clamp to \
                     the target range) or `in Trapping` (trap on NaN/out-of-range) instead.",
                )));
            }
            // F4 Exact obligation (the cast ruling's proof side): a BARE
            // float->int cast requires the value provably in the target's
            // range. What validation can prove today: a float LITERAL source
            // (through Mutable) whose truncation fits -- the two-phase law's
            // fold-visible face; runtime sources need a policy. (Mirrors the
            // F8a shift-count obligation's shape: proof where visible,
            // policy otherwise, never a silent target-defined number -- the
            // out-of-range bare cast was a pinned THREE-WAY native
            // divergence, x86 integer-indefinite vs aarch64/interp
            // saturation.)
            if cast.domain == ArithmeticDomain::Exact
                && matches!(
                    source.primitive,
                    Some(PrimitiveType::F32 | PrimitiveType::F64)
                )
                && let Some(target) = primitive
                && integer_bit_width(target).is_some()
            {
                let provable = float_literal_value(program, cast.value).is_some_and(|value| {
                    !value.is_nan()
                        && primitive_range(target).is_some_and(|range| {
                            let truncated = value.trunc();
                            // The u64-classed row has no i64 upper bound; its
                            // range check is the exact [0, 2^64) window.
                            match (range.low, range.high) {
                                (Some(low), Some(high)) => {
                                    truncated >= low as f64 && truncated <= high as f64
                                }
                                (Some(low), None) => {
                                    truncated >= low as f64
                                        && truncated < 18446744073709551616.0
                                }
                                _ => false,
                            }
                        })
                });
                if !provable {
                    diagnostics.push(Diagnostic::error(format!(
                        "float-to-int cast in {owner} is not provably in `{}`'s range \
                         (ch5 cast ruling -- proof-or-policy; only a float-LITERAL source \
                         proves today). Use `in Saturating` (NaN -> 0, clamp to the target \
                         range) or `in Trapping` (trap on NaN/out-of-range).",
                        primitive_name(target),
                    )));
                }
            }
            // The cast bounds the value to the target type's range (and re-tags its
            // domain), so it is a widening/narrowing escape from an overflow.
            let interval = primitive
                .and_then(primitive_range)
                .unwrap_or(Interval::UNBOUNDED);
            Analysis {
                domain: Some(cast.domain),
                interval,
                primitive,
            }
        }
        ExpressionNode::Call(call) => {
            // S4: the `min`/`max` builtins bound their result by their operands'
            // intervals (`max(0, x)` is >= 0, `min(x, 100)` is <= 100), so a
            // clamped value can feed exact arithmetic instead of poisoning the
            // enclosing op. Reserved-builtin name + a free (receiverless) call +
            // exactly two arguments. Each operand is analyzed into a THROWAWAY
            // diagnostic buffer and its interval is trusted ONLY if it proves
            // clean -- the bound then rests on sound operand ranges, while an
            // unproven operand's diagnostics are dropped (call arguments are not
            // otherwise overflow-checked today, so this stays strictly permissive:
            // it can only tighten a previously-unbounded call result, never add a
            // rejection).
            if !call.receiver.is_valid()
                && matches!(call.target.as_str(), "min" | "max")
                && let [left_arg, right_arg] =
                    program.expression_table.expression_handles(call.arguments)
            {
                let mut throwaway = Vec::new();
                let left = analyze(
                    program, machine, state, *left_arg, env, target_primitive, target_domain,
                    owner, &mut throwaway,
                );
                let right = analyze(
                    program, machine, state, *right_arg, env, target_primitive, target_domain,
                    owner, &mut throwaway,
                );
                if throwaway.is_empty() {
                    let interval = if call.target.as_str() == "max" {
                        left.interval.max_with(right.interval)
                    } else {
                        left.interval.min_with(right.interval)
                    };
                    let domain = match (left.domain, right.domain) {
                        (Some(left_domain), Some(right_domain)) if left_domain == right_domain => {
                            Some(left_domain)
                        }
                        (Some(domain), None) | (None, Some(domain)) => Some(domain),
                        _ => None,
                    };
                    return Analysis {
                        domain,
                        interval,
                        primitive: left.primitive.or(right.primitive),
                    };
                }
            }

            // S4 return-range inference: a machine whose return type declares a
            // literal range constraint (`-> i32 [0..=10]`) is ENFORCED to
            // return within that range (validate_return_value_range, below), so a
            // caller doing exact arithmetic on the result can rely on the narrowed
            // interval instead of being forced into a domain. Narrow ONLY when the
            // return type carries a range constraint -- a plain `-> u32` stays
            // NEUTRAL (opaque/unbounded, as before); attaching a bare type's full
            // range + primitive to an otherwise-unbounded call result would turn a
            // previously-unchecked expression into a spurious overflow.
            if let Some((primitive, interval)) =
                call_return_type(program, machine, call).and_then(|return_type| {
                    range_constraint_interval(program, return_type)
                        .map(|interval| (program.primitive_type_reference(return_type), interval))
                })
            {
                return Analysis {
                    domain: None,
                    interval,
                    primitive,
                };
            }
            // ch15 stage 2 (modular return-range inference): no DECLARED range, so
            // infer the callee's return interval from its body (sound, permissive).
            match resolve_unique_self_call_state(program, machine, call).and_then(|(callee, state)| {
                let primitive = program.primitive_type_reference(state.return_type);
                infer_return_interval(program, callee, state, primitive)
                    .map(|interval| (primitive, interval))
            }) {
                Some((primitive, interval)) => Analysis {
                    domain: None,
                    interval,
                    primitive,
                },
                None => NEUTRAL,
            }
        }
        ExpressionNode::Integer(value) => Analysis {
            domain: None,
            interval: literal_interval(value),
            primitive: None,
        },
        ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => NEUTRAL,
        // A place (`x`, `self.field`): its declared type gives the domain and the
        // integer primitive. The value range is the FLOW-tracked interval (S4) if
        // we have proven one on this linear path, else the primitive's full range.
        // An INDEXED read (`self.cells[rp]`) resolves through its collection's
        // ELEMENT type, so a range-refined element (`[i32 [0..=7]; N]`) feeds
        // the overflow proof exactly like a range-refined field (writes into
        // elements are range-enforced by the proof side, and ZII requires 0 in
        // the element range -- the interval is a true invariant).
        _ => match declared_place_type_raw(program, machine, state, expression).or_else(|| {
            // Only a RANGE-refined element feeds the analysis. An unranged
            // element stays NEUTRAL (unchecked) as before -- resolving it
            // would stamp `primitive + Exact` onto reads whose enclosing
            // arithmetic was historically domain-neutral (`[i32; 3] in
            // Wrapping` carries the domain on the ARRAY, so its bare-`i32`
            // element read as Exact broke Wrapping-array canaries with
            // spurious overflow + domain-mixing rejects).
            crate::places::declared_indexed_element_type_raw(program, machine, state, expression)
                .filter(|handle| range_constraint_interval(program, *handle).is_some())
        }) {
            Some(handle) => {
                let primitive = program.primitive_type_reference(handle);
                let type_range = primitive
                    .and_then(primitive_range)
                    .unwrap_or(Interval::UNBOUNDED);
                // Narrowest sound interval: a value PROVEN on this path (flow env)
                // wins; else a declared `[min..max]` range constraint (S4); else
                // the full type width. A proven interval is INTERSECTED with the
                // type range, because a typed value is ALWAYS within its type even
                // when the proof only bounds ONE end -- a one-sided `requires x <
                // 100` gives the env `[None, 99]`, and `[None, 99] ∩ i32 = [i32::MIN,
                // 99]` keeps the type's low end so `x + 1` proves Exact (a Wrapping-
                // spilled interval is likewise clamped back to the type). The same
                // intersect-with-source-type keystone the narrowing store uses.
                let interval = place_path(program, expression)
                    .and_then(|path| env.get(&path))
                    .or_else(|| range_constraint_interval(program, handle))
                    .map(|proven| proven.intersect(type_range))
                    .unwrap_or(type_range);
                // R2 rung 3 slice 7 (READER HYPOTHESES): a domain-carrying
                // place's standing where facts refine the read -- sound
                // because the write net is TOTAL and gated reads are
                // access-gated, so the facts hold at every legal
                // observation.
                let interval = match crate::default_domains::where_fact_interval(
                    program, machine, state, expression,
                ) {
                    Some(facts) => interval.intersect(facts),
                    None => interval,
                };
                // Atomic integer types (AtomicU32, ...) have hardware wrap-around
                // semantics, so their arithmetic is Wrapping, not Exact -- a
                // `fetch_add` never raises an overflow proof obligation.
                let domain = if is_atomic_type(program, handle) {
                    ArithmeticDomain::Wrapping
                } else {
                    program.arithmetic_domain_for_type_reference(handle)
                };
                Analysis {
                    domain: Some(domain),
                    interval,
                    primitive,
                }
            }
            None => NEUTRAL,
        },
    }
}

/// S4: the value range a type declares via a `Range` constraint (`x: i32 [0..N]`),
/// so a bounded value's exact arithmetic can be proven in-range instead of using
/// the full type width. Inclusive bounds (a sound over-approximation either way).
/// `None` when the type has no literal range constraint. Looks through reference
/// shells.
/// Q9 ruling (Zach, 2026-07-13: "this is just a compile error"): a declared
/// RANGE constraint combined with a non-Exact arithmetic domain is
/// ill-formed. The range is only enforced under Exact stores, so
/// `u8 [0..=4] in Wrapping` accepted `self.i = 100` silently -- the
/// declaration lied to every reader. Rejecting at the declaration keeps the
/// two features composable-by-omission: keep the range (Exact proof
/// obligations enforce it) or keep the domain (defined overflow, full type
/// range), never both.
pub(crate) fn check_range_under_non_exact_domain(
    program: &TypedTrees,
    handle: omega_typed_trees::types::TypeReferenceHandle,
    owner: crate::type_references::TypeReferenceOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use omega_core::arithmetic::ArithmeticDomain;
    // Walk Reference wrappers AND nested Constrained spellings: `u8 [0..=4]
    // in Wrapping` may parse as Constrained(Constrained(u8, [Range]),
    // [Domain]), where the shallow per-node accessors see only one
    // constraint set each.
    fn has_range_constraint(
        program: &TypedTrees,
        handle: omega_typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => {
                has_range_constraint(program, *referee)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| {
                        matches!(constraint, TypeConstraintNode::Range { .. })
                    })
                    || has_range_constraint(program, *base_type)
            }
            _ => false,
        }
    }
    if !has_range_constraint(program, handle) {
        return;
    }
    let domain = program.type_reference_table.arithmetic_domain(handle);
    if domain == ArithmeticDomain::Exact {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "{owner} declares a range constraint together with the `{domain:?}` domain: ranges are only enforced under Exact arithmetic, so the combination is ill-formed (a store outside the range would be silently accepted). Keep the range and drop the domain, or keep the domain and drop the range"
    )));
}

pub(crate) fn range_constraint_interval(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => range_constraint_interval(program, *referee),
        TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .find_map(|constraint| match constraint {
                TypeConstraintNode::Range { minimum, maximum } => Some(Interval {
                    low: Some(literal_i64(program, *minimum)?),
                    high: Some(
                        literal_i64(program, *maximum).or_else(|| {
                            dependent_maximum_substituted(program, *maximum)
                        })?,
                    ),
                }),
                _ => None,
            }),
        _ => None,
    }
}

/// R1a dependent maximum in interval position: `[0..=self.count]` reads as
/// the named field's own enforced literal HIGH plus the offset -- sound
/// because the field's range is store-enforced at every write, so the
/// dependent bound can never exceed it. The field is resolved by NAME
/// across all data definitions and must be UNIQUE (or agree everywhere);
/// an ambiguous name with disagreeing ranges bails rather than guesses
/// (this helper has no machine context; the declaration gate has already
/// verified the binding machine's own field is ranged).
fn dependent_maximum_substituted(
    program: &TypedTrees,
    maximum: omega_typed_trees::expression::ExpressionHandle,
) -> Option<i64> {
    let symbolic = omega_typed_trees::dependent_ranges::symbolic_max_bound(
        &program.expression_table,
        maximum,
    )?;
    let mut resolved: Option<i64> = None;
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            if field.name.as_str() != symbolic.field.as_str() || !field.type_reference.is_valid()
            {
                continue;
            }
            let high = range_constraint_interval(program, field.type_reference)
                .and_then(|interval| interval.high)?;
            match resolved {
                None => resolved = Some(high),
                Some(existing) if existing == high => {}
                Some(_) => return None,
            }
        }
    }
    resolved?.checked_add(symbolic.offset)
}

/// S4 return-range inference: the DECLARED return type of a value-position call,
/// when it resolves soundly and UNIQUELY from `program` alone. Conservative --
/// only `self`/receiver-free calls to a sibling machine (one attached to the same
/// data as the caller, with a state named after the call target) are resolved;
/// an external receiver, an ambiguous match, or no match returns `None` (no
/// narrowing). Soundness rests on uniqueness: bail rather than guess.
/// If `operand` is a value-machine call resolving to a self/sibling machine with an
/// INTEGER return type, return the call's target name (for the decision-17 overflow
/// hint). `None` for non-calls, builtins/external-receiver calls (unresolved), and
/// non-integer returns -- so the hint only fires where "annotate the callee's return"
/// is the actionable fix.
fn overflow_operand_value_call_target(
    program: &TypedTrees,
    current_machine: &Machine,
    operand: ExpressionHandle,
) -> Option<String> {
    let ExpressionNode::Call(call) = program.expression_table.expression(operand) else {
        return None;
    };
    let call = call.clone();
    let return_type = call_return_type(program, current_machine, &call)?;
    program
        .primitive_type_reference(return_type)
        .filter(|primitive| primitive.accepts_integer_literal())
        .map(|_| call.target.as_str().to_string())
}

pub(crate) fn call_return_type(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCallExpression,
) -> Option<TypeReferenceHandle> {
    let receiver_is_self = !call.receiver.is_valid()
        || matches!(
            program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path)
                if matches!(
                    program.expression_table.name_path_members(path.members),
                    [only] if only.as_str() == "self"
                )
        );
    if !receiver_is_self {
        return None;
    }
    let target = call.target.as_str();
    let attached_data = current_machine.attached_data.as_ref()?;
    let mut returns = program
        .machines()
        .iter()
        .filter(|candidate| {
            candidate
                .attached_data
                .as_ref()
                .is_some_and(|data| data.as_str() == attached_data.as_str())
        })
        .filter_map(|candidate| {
            program
                .machine_states(candidate)
                .iter()
                .find(|state| state.name.as_str() == target)
        })
        .filter(|state| state.return_type.is_valid())
        .map(|state| state.return_type);
    let first = returns.next()?;
    // Unique match only -- bail on ambiguity (sound: fall back to unbounded).
    if returns.next().is_some() {
        return None;
    }
    Some(first)
}

thread_local! {
    /// One-level recursion guard for return-range INFERENCE (ch15 stage 2). While
    /// inferring a callee's return interval we analyze its body; if that body
    /// calls another machine, we must NOT recurse into inference again (would
    /// loop on recursive/mutually-recursive callees). The nested call simply
    /// stays NEUTRAL.
    static INFERRING_RETURN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The unique self/sibling machine+state a receiver-free `self.target(..)` call
/// resolves to (mirrors `call_return_type`'s resolution but keeps the state so
/// its body can be analyzed). `None` on a non-self receiver or an ambiguous
/// match (sound: bail rather than guess).
fn resolve_unique_self_call_state<'program>(
    program: &'program TypedTrees,
    current_machine: &Machine,
    call: &TableCallExpression,
) -> Option<(&'program Machine, &'program State)> {
    let receiver_is_self = !call.receiver.is_valid()
        || matches!(
            program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path)
                if matches!(
                    program.expression_table.name_path_members(path.members),
                    [only] if only.as_str() == "self"
                )
        );
    if !receiver_is_self {
        return None;
    }
    let target = call.target.as_str();
    let attached_data = current_machine.attached_data.as_ref()?;
    let mut matches = program.machines().iter().filter_map(|candidate| {
        let same_data = candidate
            .attached_data
            .as_ref()
            .is_some_and(|data| data.as_str() == attached_data.as_str());
        if !same_data {
            return None;
        }
        let state = program
            .machine_states(candidate)
            .iter()
            .find(|state| state.name.as_str() == target)?;
        Some((candidate, state))
    });
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

/// ch15 stage 2 -- MODULAR RETURN-RANGE INFERENCE: when a callee declares no
/// return range, infer one from its body so the caller's arithmetic on the
/// result can stay Exact without the callee writing `-> i32 [a..=b]`. Sound and
/// STRICTLY PERMISSIVE: the body is analyzed with an EMPTY env (params at full
/// type width -> the widest possible result, so any caller's actual return is
/// within it). All return paths of the callee state are UNIONed -- a terminal
/// expression and/or transition VALUE targets (`{ cond -> v1 _ -> v2 }`). SOUND:
/// every path must be captured, so the state must be a LEAF value state -- if any
/// transition target is Named/SelfTarget (a return could come from a state we are
/// not analyzing, or a loop), we bail. The interval is trusted only if every
/// path's analysis is clean and the union is fully bounded. Recursion-guarded to
/// one level.
fn infer_return_interval(
    program: &TypedTrees,
    callee_machine: &Machine,
    callee_state: &State,
    target_primitive: Option<PrimitiveType>,
) -> Option<Interval> {
    use omega_typed_trees::statement::{StatementNode, TransitionTargetNode};
    if INFERRING_RETURN.with(std::cell::Cell::get) {
        return None;
    }
    let statements = program.statement_table.statements(callee_state.statement_nodes);

    // Collect every return expression, bailing if any exit could escape to an
    // uncaptured state. A terminal expression is the last statement; transition
    // arms return via VALUE targets.
    let mut return_expressions = Vec::new();
    if let Some(StatementNode::Expression(expression)) = statements.last() {
        return_expressions.push(*expression);
    }
    for statement in statements {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        for target in [transition.target, transition.continuation] {
            if !target.is_valid() {
                continue;
            }
            match program.statement_table.transition_target(target) {
                TransitionTargetNode::Value(expression) => return_expressions.push(*expression),
                TransitionTargetNode::Terminal => {}
                // Named (another state / recursion) or SelfTarget (loop): the
                // return may come from somewhere we are not analyzing -> bail.
                TransitionTargetNode::Named { .. } | TransitionTargetNode::SelfTarget => {
                    return None;
                }
            }
        }
    }
    if return_expressions.is_empty() {
        return None;
    }

    let env = ValueEnv::new();
    INFERRING_RETURN.with(|flag| flag.set(true));
    let mut union: Option<Interval> = None;
    let mut clean = true;
    for expression in return_expressions {
        let mut throwaway = Vec::new();
        let analysis = analyze(
            program,
            callee_machine,
            Some(callee_state),
            expression,
            &env,
            target_primitive,
            ArithmeticDomain::Exact,
            "inferred return",
            &mut throwaway,
        );
        if !throwaway.is_empty() {
            clean = false;
            break;
        }
        union = Some(match union {
            Some(current) => current.union(analysis.interval),
            None => analysis.interval,
        });
    }
    INFERRING_RETURN.with(|flag| flag.set(false));
    if !clean {
        return None;
    }
    let union = union?;
    (union.low().is_some() && union.high().is_some()).then_some(union)
}

/// S4 return-range ENFORCEMENT (companion to the call-site narrowing): when a
/// return type declares a literal `[a..=b]`, the returned value's proven
/// interval must fit inside it, else callers that trust the declared range are
/// unsound. No-op when the return type carries no range constraint (so plain
/// returns are unaffected -- range-constrained return types are a new
/// capability). `interval` is the return expression's already-analyzed interval.
pub(crate) fn enforce_declared_return_range(
    program: &TypedTrees,
    return_type: TypeReferenceHandle,
    interval: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(range) = range_constraint_interval(program, return_type)
        && !range.contains(interval)
    {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} returns a value not provably within its declared range: callers rely on the \
             declared `[a..=b]` for exact arithmetic on the result, so the returned value must \
             be proven to honor it (decision 17). Constrain the returned value, or widen/remove the \
             return range constraint."
        )));
    }
}

/// S4: analyze a transition VALUE-return expression and enforce its declared
/// return range. Gated on the return type carrying a range constraint -- only
/// then is the (otherwise un-validated) transition-value return analyzed, so
/// existing plain returns are byte-for-byte unaffected.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_return_value_range(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    return_expression: ExpressionHandle,
    env: &ValueEnv,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let return_primitive = program.primitive_type_reference(state.return_type);
    let return_domain = program.arithmetic_domain_for_type_reference(state.return_type);
    if range_constraint_interval(program, state.return_type).is_some() {
        // Range-constrained return: analyze (emitting any overflow obligation, as
        // before), enforce the declared `[a..=b]`, and -- on a clean value -- the
        // narrowing store obligation too. A value proven within `[a..=b]` already
        // fits the type, so the narrowing check adds no rejection here; it is
        // present only for uniformity with the unconstrained branch.
        let before = diagnostics.len();
        let (interval, source) = validate_value_range(
            program,
            machine,
            Some(state),
            return_expression,
            env,
            return_primitive,
            return_domain,
            owner,
            diagnostics,
        );
        if diagnostics.len() == before {
            check_narrowing_assignment(return_primitive, interval, source, owner, diagnostics);
        }
        enforce_declared_return_range(program, state.return_type, interval, owner, diagnostics);
        return;
    }
    // Unconstrained return: analyze into the REAL diagnostics, emitting any exact-
    // arithmetic overflow obligation just like every other value-binding boundary
    // (`_ -> (x + y)` with full-range Exact operands would otherwise wrap silently).
    // On a clean value, add the narrowing store obligation too -- a value that fits
    // its source type but not the return type (`-> i8 { _ -> (300) }`) is a silent
    // truncation.
    let before = diagnostics.len();
    let (interval, source) = validate_value_range(
        program,
        machine,
        Some(state),
        return_expression,
        env,
        return_primitive,
        return_domain,
        owner,
        diagnostics,
    );
    if diagnostics.len() == before {
        check_narrowing_assignment(return_primitive, interval, source, owner, diagnostics);
    }
}

/// A type-constraint range bound as an i64: a literal integer (`[0..100]`) or
/// a CONSTANT integer expression (`[0 - 1..=40]` folds to `-1` -- expression
/// bounds used to silently behave UNBOUNDED). A non-constant bound is not
/// narrowed (the caller falls back to the full type range -- sound; the
/// declaration check in type_references.rs rejects it loudly).
pub(crate) fn literal_i64(program: &TypedTrees, expression: ExpressionHandle) -> Option<i64> {
    program.expression_table.constant_integer_value(expression)
}

/// Whether a place's declared type is an atomic integer (`AtomicU32`, ...),
/// whose arithmetic wraps by hardware semantics, through reference/constraint
/// shells.
fn is_atomic_type(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { name, .. } => name.as_str().starts_with("Atomic"),
        TypeReferenceNode::Reference { referee, .. } => is_atomic_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => is_atomic_type(program, *base_type),
        _ => false,
    }
}

fn primitive_name(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::I8 => "i8",
        PrimitiveType::U8 => "u8",
        PrimitiveType::I16 => "i16",
        PrimitiveType::U16 => "u16",
        PrimitiveType::I32 => "i32",
        PrimitiveType::U32 => "u32",
        PrimitiveType::I64 => "i64",
        PrimitiveType::U64 => "u64",
        PrimitiveType::Addr => "addr",
        PrimitiveType::Bool => "bool",
        PrimitiveType::F32 => "f32",
        PrimitiveType::F64 => "f64",
        PrimitiveType::String => "String",
    }
}

#[cfg(test)]
mod tests {
    use super::Interval;

    fn iv(low: i64, high: i64) -> Interval {
        Interval {
            low: Some(low),
            high: Some(high),
        }
    }

    #[test]
    fn modulo_by_positive_constant_bounds_by_divisor() {
        // unknown-sign dividend % 100 -> [-99, 99]
        assert_eq!(Interval::UNBOUNDED.modulo(iv(100, 100)), iv(-99, 99));
        // non-negative dividend -> non-negative remainder
        assert_eq!(iv(0, i64::MAX).modulo(iv(100, 100)), iv(0, 99));
        // non-positive dividend -> non-positive remainder
        assert_eq!(iv(i64::MIN, 0).modulo(iv(100, 100)), iv(-99, 0));
        // divisor given as a positive RANGE uses its max magnitude
        assert_eq!(Interval::UNBOUNDED.modulo(iv(2, 8)), iv(-7, 7));
    }

    #[test]
    fn modulo_unsound_divisors_stay_unbounded() {
        // divisor may be 0 -> cannot bound
        assert_eq!(iv(0, 10).modulo(iv(0, 5)), Interval::UNBOUNDED);
        // divisor spans 0
        assert_eq!(iv(0, 10).modulo(iv(-3, 3)), Interval::UNBOUNDED);
        // unbounded divisor
        assert_eq!(iv(0, 10).modulo(Interval::UNBOUNDED), Interval::UNBOUNDED);
        // negative divisor range bounds by magnitude
        assert_eq!(Interval::UNBOUNDED.modulo(iv(-8, -2)), iv(-7, 7));
    }

    #[test]
    fn divide_by_nonzero_never_grows_magnitude() {
        // A single-valued POSITIVE divisor gives EXACT truncated-quotient
        // bounds (monotone in the dividend for k > 0) -- the tight interval
        // that lets `let tens: u32 [0..=9] = x / 10` store-prove.
        assert_eq!(iv(-99, 99).divide(iv(2, 2)), iv(-49, 49));
        assert_eq!(iv(10, 50).divide(iv(3, 3)), iv(3, 16));
        assert_eq!(iv(-50, -10).divide(iv(3, 3)), iv(-16, -3));
        assert_eq!(iv(0, 99).divide(iv(10, 10)), iv(0, 9));
        // A RANGED nonzero divisor keeps the magnitude-preserving
        // over-approximation, widened to include 0.
        assert_eq!(iv(10, 50).divide(iv(2, 3)), iv(0, 50));
        assert_eq!(iv(-50, -10).divide(iv(2, 3)), iv(-50, 0));
        // unbounded dividend cannot be bounded by division
        assert_eq!(Interval::UNBOUNDED.divide(iv(2, 2)), Interval::UNBOUNDED);
        // maybe-zero divisor: cannot assume magnitude >= 1
        assert_eq!(iv(10, 50).divide(iv(0, 5)), Interval::UNBOUNDED);
    }

    #[test]
    fn min_max_clamp_against_unbounded() {
        assert_eq!(
            Interval::UNBOUNDED.max_with(iv(0, 0)),
            Interval { low: Some(0), high: None }
        );
        assert_eq!(
            Interval::UNBOUNDED.min_with(iv(100, 100)),
            Interval { low: None, high: Some(100) }
        );
        assert_eq!(iv(0, 50).max_with(iv(10, 10)), iv(10, 50));
        assert_eq!(iv(0, 50).min_with(iv(10, 10)), iv(0, 10));
        // chained clamp: max(seed,0) then min(_,60) -> [0,60]
        assert_eq!(
            Interval::UNBOUNDED.max_with(iv(0, 0)).min_with(iv(60, 60)),
            iv(0, 60)
        );
    }

    #[test]
    fn nonzero_magnitude_bound_requires_excluding_zero() {
        assert_eq!(iv(1, 100).nonzero_magnitude_bound(), Some(100));
        assert_eq!(iv(-100, -1).nonzero_magnitude_bound(), Some(100));
        assert_eq!(iv(0, 100).nonzero_magnitude_bound(), None); // includes 0
        assert_eq!(iv(-5, 5).nonzero_magnitude_bound(), None); // spans 0
        assert_eq!(Interval::UNBOUNDED.nonzero_magnitude_bound(), None);
    }
}

/// R1 relational refinement (the ONE closed subtraction rule): `self.F - i`
/// where `i`'s DECLARED range carries the dependent maximum `self.F + k`
/// (recognizer class) satisfies `F - i >= -k` -- at k=0 the
/// capacity-minus-used idiom `self.count - i` is provably non-negative, and
/// the exclusive sugar's k=-1 gives `>= 1`. The left side may itself be
/// `self.F + m` (recognizer), shifting the floor to `m - k`. SOUND only
/// while the field holds still between the state's entry (where the
/// caller proved the atom) and this expression: any write to the field or
/// any opaque call in the state defeats the refinement (the naive interval
/// stands). Interval-only engines cannot express the relation; this rule
/// consumes it at the one operator shape R1 unblocks.
fn refine_dependent_subtract(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
    naive: Interval,
) -> Interval {
    let Some(state) = state else {
        return naive;
    };
    // Channel (b): a machine-level `requires` fact spelling `right <= left`
    // (or `left >= right`) proves `left - right >= 0` (strict compares give
    // `>= 1`). `requires` denotes MACHINE entry, so the fact must survive to
    // this state: both sides' named fields must be unwritten across the
    // WHOLE machine (conservative; call-free likewise) -- the same
    // entry-fact bridging rule every dependent discharge uses.
    if let Some(floor) = requires_orders_operands(program, machine, left, right) {
        let refined_low = match naive.low {
            Some(low) => Some(low.max(floor)),
            None => Some(floor),
        };
        return Interval {
            low: refined_low,
            high: naive.high,
        };
    }
    // Right: a place whose RAW declared type carries the dependent maximum.
    let Some(right_raw) =
        crate::places::declared_place_type_raw(program, machine, Some(state), right)
    else {
        return naive;
    };
    let Some((right_field, right_offset)) = dependent_maximum_of_type_reference(program, right_raw)
    else {
        return naive;
    };
    // Left: `self.F` or `self.F + m` for the SAME field.
    let Some(left_bound) = omega_typed_trees::dependent_ranges::symbolic_max_bound(
        &program.expression_table,
        left,
    ) else {
        return naive;
    };
    if left_bound.field.as_str() != right_field.as_str() {
        return naive;
    }
    if !validation_state_preserves_field(program, state, &right_field) {
        return naive;
    }
    let Some(floor) = left_bound.offset.checked_sub(right_offset) else {
        return naive;
    };
    let refined_low = match naive.low {
        Some(low) => Some(low.max(floor)),
        None => Some(floor),
    };
    Interval {
        low: refined_low,
        high: naive.high,
    }
}

/// The dependent maximum (field, offset) of a RAW type reference's Range
/// constraint, under Exact shells only (mirrors the checker's substitution
/// gates).
fn dependent_maximum_of_type_reference(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<(omega_typed_trees::name::Identifier, i64)> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            dependent_maximum_of_type_reference(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::Range { maximum, .. } => {
                        let symbolic = omega_typed_trees::dependent_ranges::symbolic_max_bound(
                            &program.expression_table,
                            *maximum,
                        )?;
                        Some((symbolic.field, symbolic.offset))
                    }
                    _ => None,
                })
                .or_else(|| dependent_maximum_of_type_reference(program, *base_type))
        }
        _ => None,
    }
}

/// Conservative whole-state field-preservation scan (twin of omega-proof's
/// route-c fence): any assignment mentioning the field or any call defeats
/// the entry-fact bridge.
fn validation_state_preserves_field(
    program: &TypedTrees,
    state: &State,
    field: &omega_typed_trees::name::Identifier,
) -> bool {
    use omega_typed_trees::statement::StatementNode;
    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::Assignment(assignment) => {
                if validation_expression_mentions_field(program, assignment.target, field)
                    || validation_expression_contains_call(program, assignment.value)
                {
                    return false;
                }
            }
            StatementNode::Expression(expression) => {
                if validation_expression_contains_call(program, *expression) {
                    return false;
                }
            }
            StatementNode::LocalData(local) => {
                if local.initial_value.is_valid()
                    && validation_expression_contains_call(program, local.initial_value)
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn validation_expression_mentions_field(
    program: &TypedTrees,
    expression: ExpressionHandle,
    field: &omega_typed_trees::name::Identifier,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            member.member.as_str() == field.as_str()
                || validation_expression_mentions_field(program, member.receiver, field)
        }
        ExpressionNode::Mutable(inner) => {
            validation_expression_mentions_field(program, *inner, field)
        }
        ExpressionNode::Indexed(indexed) => {
            validation_expression_mentions_field(program, indexed.collection, field)
        }
        _ => false,
    }
}

fn validation_expression_contains_call(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            validation_expression_contains_call(program, binary.left)
                || validation_expression_contains_call(program, binary.right)
        }
        ExpressionNode::Unary(unary) => validation_expression_contains_call(program, unary.operand),
        ExpressionNode::Mutable(inner) => validation_expression_contains_call(program, *inner),
        ExpressionNode::Cast(cast) => validation_expression_contains_call(program, cast.value),
        ExpressionNode::Member(member) => {
            validation_expression_contains_call(program, member.receiver)
        }
        ExpressionNode::Indexed(indexed) => {
            validation_expression_contains_call(program, indexed.collection)
                || validation_expression_contains_call(program, indexed.index)
        }
        _ => false,
    }
}

/// Channel (b) of the relational subtraction rule: scan the machine's
/// `requires` conjunctions for `right <= left` / `left >= right` (by display
/// spelling), returning the implied floor of `left - right` (0 inclusive,
/// 1 strict). `None` unless the fact exists AND both operands' named fields
/// are machine-wide preserved (requires speaks of machine ENTRY).
fn requires_orders_operands(
    program: &TypedTrees,
    machine: &Machine,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<i64> {
    let left_label = program.expression_table.display_name(left);
    let right_label = program.expression_table.display_name(right);
    let mut floor: Option<i64> = None;
    for contract in program.machine_contracts(machine) {
        if contract.kind != omega_typed_trees::signature::SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let omega_typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some(found) =
                conjunct_orders(program, *expression, &left_label, &right_label)
            {
                floor = Some(floor.map_or(found, |existing: i64| existing.max(found)));
            }
        }
    }
    let floor = floor?;
    // Machine-wide preservation of every field either operand mentions.
    for operand in [left, right] {
        if !machine_preserves_expression_fields(program, machine, operand) {
            return None;
        }
    }
    Some(floor)
}

/// `right <= left` (floor 0) / `right < left` (floor 1), matched by display
/// spelling at any depth of an `&&` conjunction; flipped `>=`/`>` normalize.
fn conjunct_orders(
    program: &TypedTrees,
    guard: ExpressionHandle,
    left_label: &str,
    right_label: &str,
) -> Option<i64> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => conjunct_orders(program, binary.left, left_label, right_label)
            .or_else(|| conjunct_orders(program, binary.right, left_label, right_label)),
        BinaryOperator::LessOrEqual | BinaryOperator::Less => {
            let lo = program.expression_table.display_name(binary.left);
            let hi = program.expression_table.display_name(binary.right);
            (lo == right_label && hi == left_label).then(|| {
                if binary.operator == BinaryOperator::Less { 1 } else { 0 }
            })
        }
        BinaryOperator::GreaterOrEqual | BinaryOperator::Greater => {
            let hi = program.expression_table.display_name(binary.left);
            let lo = program.expression_table.display_name(binary.right);
            (lo == right_label && hi == left_label).then(|| {
                if binary.operator == BinaryOperator::Greater { 1 } else { 0 }
            })
        }
        _ => None,
    }
}

/// Every `self.<field>` the expression mentions is preserved (never written,
/// no calls) across EVERY state of the machine -- the conservative bridge
/// from machine-entry facts to any state's expressions.
fn machine_preserves_expression_fields(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> bool {
    let mut fields: Vec<omega_typed_trees::name::Identifier> = Vec::new();
    collect_self_fields(program, expression, &mut fields);
    fields.iter().all(|field| {
        program
            .machine_states(machine)
            .iter()
            .all(|state| validation_state_preserves_field(program, state, field))
    })
}

fn collect_self_fields(
    program: &TypedTrees,
    expression: ExpressionHandle,
    fields: &mut Vec<omega_typed_trees::name::Identifier>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            fields.push(member.member.clone());
            collect_self_fields(program, member.receiver, fields);
        }
        ExpressionNode::Binary(binary) => {
            collect_self_fields(program, binary.left, fields);
            collect_self_fields(program, binary.right, fields);
        }
        ExpressionNode::Mutable(inner) => collect_self_fields(program, *inner, fields),
        ExpressionNode::Cast(cast) => collect_self_fields(program, cast.value, fields),
        _ => {}
    }
}

/// R3's ONE closed bounded-product rule: `a * self.Fb + c` where
/// `a <= self.Fa - 1` (a STRICT dependent atom), `c <= self.Fb - 1` (strict,
/// on the SAME field the product multiplies by), and a machine `requires`
/// couples `self.Fa * self.Fb <= K` -- then
/// `a*Fb + c <= (Fa-1)*Fb + (Fb-1) = Fa*Fb - 1 <= K - 1`, so the interval is
/// `[0, K-1]` (unsigned operands floor at 0; a signed floor keeps the naive
/// low). Needed exactly where operand ranges are NOT independently tight
/// (runtime dims bounded only by their product). Both fields must be
/// machine-wide preserved (the coupling speaks of machine entry; the atoms
/// of state entry).
fn refine_dependent_product(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
    naive: Interval,
) -> Interval {
    let Some(state) = state else {
        return naive;
    };
    // Left: `a * self.Fb` (either operand order).
    let ExpressionNode::Binary(product) = program.expression_table.expression(left) else {
        return naive;
    };
    if product.operator != BinaryOperator::Multiply {
        return naive;
    }
    let (a_expr, fb_expr) = {
        let left_is_field = omega_typed_trees::dependent_ranges::symbolic_max_bound(
            &program.expression_table,
            product.left,
        )
        .is_some_and(|bound| bound.offset == 0);
        if left_is_field {
            (product.right, product.left)
        } else {
            (product.left, product.right)
        }
    };
    let Some(fb) = omega_typed_trees::dependent_ranges::symbolic_max_bound(
        &program.expression_table,
        fb_expr,
    )
    .filter(|bound| bound.offset == 0)
    .map(|bound| bound.field) else {
        return naive;
    };
    // a's STRICT dependent atom names Fa; c's STRICT atom names Fb (the
    // multiplier field).
    let Some(fa) = strict_dependent_atom_field(program, machine, state, a_expr) else {
        return naive;
    };
    let Some(c_field) = strict_dependent_atom_field(program, machine, state, right) else {
        return naive;
    };
    if c_field.as_str() != fb.as_str() {
        return naive;
    }
    // The coupling: `requires self.Fa * self.Fb <= K` (either multiply order).
    let Some(k) = requires_product_coupling(program, machine, &fa, &fb) else {
        return naive;
    };
    // Preservation of both fields, machine-wide.
    for field in [&fa, &fb] {
        let preserved = program
            .machine_states(machine)
            .iter()
            .all(|state| validation_state_preserves_field(program, state, field));
        if !preserved {
            return naive;
        }
    }
    let Some(high) = k.checked_sub(1) else {
        return naive;
    };
    Interval {
        low: Some(naive.low.map_or(0, |low| low.max(0))),
        high: Some(naive.high.map_or(high, |naive_high| naive_high.min(high))),
    }
}

/// The field a STRICTLY-bounded dependent place names: the expression is a
/// place whose declared range maximum is `self.<field> - 1` (the exclusive
/// sugar's normalization -- `a < field` at entry).
fn strict_dependent_atom_field(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<omega_typed_trees::name::Identifier> {
    let raw = crate::places::declared_place_type_raw(program, machine, Some(state), expression)?;
    let (field, offset) = dependent_maximum_of_type_reference(program, raw)?;
    (offset == -1).then_some(field)
}

/// A machine `requires` conjunct `self.Fa * self.Fb <= K` (either multiply
/// order; strict `<` tightens to `K - 1`).
fn requires_product_coupling(
    program: &TypedTrees,
    machine: &Machine,
    fa: &omega_typed_trees::name::Identifier,
    fb: &omega_typed_trees::name::Identifier,
) -> Option<i64> {
    let fa_label = format!("self.{}", fa.as_str());
    let fb_label = format!("self.{}", fb.as_str());
    for contract in program.machine_contracts(machine) {
        if contract.kind != omega_typed_trees::signature::SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let omega_typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some(k) =
                product_coupling_conjunct(program, *expression, &fa_label, &fb_label)
            {
                return Some(k);
            }
        }
    }
    None
}

fn product_coupling_conjunct(
    program: &TypedTrees,
    guard: ExpressionHandle,
    fa_label: &str,
    fb_label: &str,
) -> Option<i64> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            product_coupling_conjunct(program, binary.left, fa_label, fb_label).or_else(|| {
                product_coupling_conjunct(program, binary.right, fa_label, fb_label)
            })
        }
        BinaryOperator::LessOrEqual | BinaryOperator::Less => {
            let ExpressionNode::Binary(product) =
                program.expression_table.expression(binary.left)
            else {
                return None;
            };
            if product.operator != BinaryOperator::Multiply {
                return None;
            }
            let lhs = program.expression_table.display_name(product.left);
            let rhs = program.expression_table.display_name(product.right);
            let matches = (lhs == fa_label && rhs == fb_label)
                || (lhs == fb_label && rhs == fa_label);
            if !matches {
                return None;
            }
            let k = literal_i64(program, binary.right)?;
            if binary.operator == BinaryOperator::Less {
                k.checked_sub(1)
            } else {
                Some(k)
            }
        }
        _ => None,
    }
}

/// The MULTIPLY half of R3's rule: `a * self.Fb` (either order) with
/// `a <= self.Fa - 1` strict and the coupling `Fa * Fb <= K` is bounded by
/// `(Fa-1)*Fb = Fa*Fb - Fb <= K` (Fb unsigned) -- interval `[0, K]`. The
/// enclosing Add then tightens to `K - 1`.
fn refine_dependent_product_factor(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
    naive: Interval,
) -> Interval {
    let Some(state) = state else {
        return naive;
    };
    let (a_expr, fb_expr) = {
        let left_is_field = omega_typed_trees::dependent_ranges::symbolic_max_bound(
            &program.expression_table,
            left,
        )
        .is_some_and(|bound| bound.offset == 0);
        if left_is_field { (right, left) } else { (left, right) }
    };
    let Some(fb) = omega_typed_trees::dependent_ranges::symbolic_max_bound(
        &program.expression_table,
        fb_expr,
    )
    .filter(|bound| bound.offset == 0)
    .map(|bound| bound.field) else {
        return naive;
    };
    let Some(fa) = strict_dependent_atom_field(program, machine, state, a_expr) else {
        return naive;
    };
    let Some(k) = requires_product_coupling(program, machine, &fa, &fb) else {
        return naive;
    };
    for field in [&fa, &fb] {
        let preserved = program
            .machine_states(machine)
            .iter()
            .all(|state| validation_state_preserves_field(program, state, field));
        if !preserved {
            return naive;
        }
    }
    Interval {
        low: Some(naive.low.map_or(0, |low| low.max(0))),
        high: Some(naive.high.map_or(k, |naive_high| naive_high.min(k))),
    }
}

/// Containment of a cleanly-analyzed store interval in the target's declared
/// Exact `[a..=b]` range -- refusing when a BOUND exists and provably
/// escapes (`[0, 12]` into `[0..=11]`: the R3-era native-OOB shape). An
/// UNBOUNDED side does NOT refuse here: the interval engine's env seeding
/// is conservative in post-entry states, so treating unknown as
/// out-of-range would flip provably-sound corpus shapes; the unbounded
/// residue of the store-proof gap stays pinned in TASKS.md until the env
/// seeding tightens. Non-Exact shells and unranged targets are untouched.
pub(crate) fn check_range_containment(
    program: &TypedTrees,
    target_type: TypeReferenceHandle,
    interval: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(declared) = enforced_declared_range_interval(program, target_type) else {
        return;
    };
    let escapes_low = match (interval.low, declared.low) {
        (Some(low), Some(declared_low)) => low < declared_low,
        _ => false,
    };
    let escapes_high = match (interval.high, declared.high) {
        (Some(high), Some(declared_high)) => high > declared_high,
        _ => false,
    };
    if escapes_low || escapes_high {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} stores a value not provably within its declared range: the range is a \
             store-enforced invariant every read trusts (indexes, exact arithmetic), so the \
             stored value must be proven to honor it. Narrow the value with a dominating \
             guard, a dependent bound, or a requires coupling -- or widen/remove the range",
        )));
    }
}

/// The declared literal `[a..=b]` of a type reference, ONLY under all-Exact
/// Constrained shells (non-Exact ranges are deliberately permissive).
fn enforced_declared_range_interval(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            enforced_declared_range_interval(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::Range { minimum, maximum } => Some(Interval {
                        low: Some(literal_i64(program, *minimum)?),
                        high: Some(literal_i64(program, *maximum)?),
                    }),
                    _ => None,
                })
                .or_else(|| enforced_declared_range_interval(program, *base_type))
        }
        _ => None,
    }
}
