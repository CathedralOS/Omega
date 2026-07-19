//! Q6 ruling (Zach, 2026-07-13): machine CALL cycles are banned ("yes
//! fucking banned"), regardless of boundedness. The specializer already
//! refuses UNBOUNDED cycles ("calls into a recursive cycle"), but a bounded
//! `A -> B -> A` -- the dungeon's old find_item_at/find_item_after pair,
//! spelled through arm-target `self.SIBLING(..)` calls -- was absorbed by
//! clone specialization and compiled. Absorbable does not make it legal
//! Omega: calls are stack-based; repetition is a STATE transition. This walk
//! sees every call spelling (statement calls, value-position calls, and
//! `self.X(..)` transition/match arm targets), builds the machine-level call
//! graph, and rejects any cycle with the cycle path named.

use crate::symbols::TopLevelSymbols;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetNode,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub(crate) fn validate_machine_call_cycles(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let machines = program.machines();
    let mut index_of: HashMap<u32, usize> = HashMap::with_capacity(machines.len());
    for (index, machine) in machines.iter().enumerate() {
        index_of.insert(machine.symbol.arena_index(), index);
    }

    let mut edges: Vec<Vec<usize>> = Vec::with_capacity(machines.len());
    let mut edge_is_tail: HashMap<(usize, usize), bool> = HashMap::new();
    let mut edge_decreases: HashMap<(usize, usize), bool> = HashMap::new();
    for (from, machine) in machines.iter().enumerate() {
        let mut out: BTreeMap<usize, (bool, bool)> = BTreeMap::new();
        for state in program.machine_states(machine) {
            // MR4 admission v1: the guard context for the decrease proof.
            // Walking the state's arm statements IN ORDER, a transition
            // guarded `When(m == 0)` excludes zero from `m` for every
            // LATER arm in the same state (dispatch tests arms in order,
            // so a later arm only runs when the equality failed; the
            // measure subjects are unsigned by the Nat::Descending view,
            // so `m != 0` is `m >= 1`).
            let mut zero_excluded: Vec<String> = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                collect_statement_edges(
                    program,
                    machine,
                    symbols,
                    &index_of,
                    statement,
                    &zero_excluded,
                    &mut out,
                );
                if let StatementNode::Transition(transition) = statement {
                    if std::env::var_os("OMEGA_MR4_TRACE").is_some() {
                        match transition.guard {
                            TransitionGuardNode::When(guard) => {
                                if let ExpressionNode::Binary(binary) =
                                    program.expression_table.expression(guard)
                                {
                                    eprintln!(
                                        "MR4 guard {}::{}: When L={:?} R={:?}",
                                        machine.name,
                                        state.name,
                                        program.expression_table.expression(binary.left),
                                        program.expression_table.expression(binary.right)
                                    );
                                }
                            }
                            _ => eprintln!(
                                "MR4 guard {}::{}: {:?}",
                                machine.name, state.name, transition.guard
                            ),
                        }
                    }
                    if let TransitionGuardNode::When(guard) = transition.guard
                        && let Some(name) = equals_zero_subject(program, guard)
                    {
                        zero_excluded.push(name);
                    }
                }
            }
        }
        let mut targets = Vec::with_capacity(out.len());
        for (to, (is_tail, decreases)) in out {
            edge_is_tail.insert((from, to), is_tail);
            edge_decreases.insert((from, to), decreases);
            targets.push(to);
        }
        edges.push(targets);
    }

    // DFS with an explicit path; each distinct cycle (as a machine SET) is
    // reported once, naming the path in call order.
    let mut color = vec![0u8; machines.len()]; // 0 unvisited, 1 on-stack, 2 done
    let mut reported: HashSet<BTreeSet<usize>> = HashSet::new();
    for start in 0..machines.len() {
        if color[start] == 0 {
            dfs_report_cycles(
                program,
                &edges,
                &edge_is_tail,
                &edge_decreases,
                start,
                &mut color,
                &mut Vec::new(),
                &mut reported,
                diagnostics,
            );
        }
    }
}

fn dfs_report_cycles(
    program: &TypedTrees,
    edges: &[Vec<usize>],
    edge_is_tail: &HashMap<(usize, usize), bool>,
    edge_decreases: &HashMap<(usize, usize), bool>,
    node: usize,
    color: &mut [u8],
    path: &mut Vec<usize>,
    reported: &mut HashSet<BTreeSet<usize>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    color[node] = 1;
    path.push(node);
    for &next in &edges[node] {
        if color[next] == 1 {
            // Back-edge: the cycle is the path suffix from `next`.
            let position = path
                .iter()
                .position(|&member| member == next)
                .unwrap_or(path.len() - 1);
            let cycle = &path[position..];
            let key: BTreeSet<usize> = cycle.iter().copied().collect();
            if reported.insert(key) {
                let machines = program.machines();
                let mut names: Vec<&str> = cycle
                    .iter()
                    .map(|&member| machines[member].name.as_str())
                    .collect();
                names.push(machines[next].name.as_str());
                // MR4 qualification report: name what a future joint-measure
                // admission would require of THIS cycle. Every cycle still
                // refuses (admission is gated on cross-machine tail-call
                // lowering -- today an unbounded measured cycle would grow
                // the stack), but the diagnostic tells the author whether
                // the shape is right.
                let mut non_tail: Vec<String> = Vec::new();
                let mut undecreasing: Vec<String> = Vec::new();
                for window in 0..cycle.len() {
                    let from = cycle[window];
                    let to = if window + 1 < cycle.len() {
                        cycle[window + 1]
                    } else {
                        next
                    };
                    if !edge_is_tail.get(&(from, to)).copied().unwrap_or(false) {
                        non_tail.push(format!(
                            "`{}` -> `{}`",
                            machines[from].name, machines[to].name,
                        ));
                    }
                    if !edge_decreases.get(&(from, to)).copied().unwrap_or(false) {
                        undecreasing.push(format!(
                            "`{}` -> `{}`",
                            machines[from].name, machines[to].name,
                        ));
                    }
                }
                let unmeasured: Vec<String> = cycle
                    .iter()
                    .filter(|&&member| {
                        let machine = &machines[member];
                        machine.termination_plan.implementation_witness.is_none()
                            && program
                                .expression_table
                                .expression_handles(machine.decreases)
                                .is_empty()
                    })
                    .map(|&member| format!("`{}`", machines[member].name))
                    .collect();
                // MR4 ADMISSION (2026-07-20): a cycle whose every edge is a
                // tail transition arm target, every member measured, and
                // every edge PROVEN to strictly decrease the callee's
                // measure (v1: the `m == 0`-guarded base arm + `m - 1`
                // argument shape) is ADMITTED -- the dispatch loop lowers
                // every transition as a SetDispatchState jump over ONE
                // overlaid frame region, so the cycle runs on constant
                // stack (probe: 40M alternations, constant memory).
                if non_tail.is_empty() && unmeasured.is_empty() && undecreasing.is_empty() {
                    continue;
                }
                let qualification = if non_tail.is_empty() && unmeasured.is_empty() {
                    format!(
                        " MR4 shape check: every edge is a tail transition and every \
                         member is measured, but the strict measure DECREASE is \
                         unproven on edge(s): {} -- v1 admission proves exactly the \
                         `m == 0`-guarded base arm + `m - 1` tail-argument shape.",
                        undecreasing.join(", "),
                    )
                } else {
                    let mut parts = Vec::new();
                    if !non_tail.is_empty() {
                        parts.push(format!(
                            "non-tail call edge(s): {}",
                            non_tail.join(", "),
                        ));
                    }
                    if !unmeasured.is_empty() {
                        parts.push(format!(
                            "unmeasured machine(s): {}",
                            unmeasured.join(", "),
                        ));
                    }
                    format!(
                        " MR4 shape check: NOT met -- {}. A joint-measure \
                         admission would need every cycle edge spelled as a tail \
                         transition arm target and every member measured \
                         (`terminates by ...`).",
                        parts.join("; "),
                    )
                };
                diagnostics.push(Diagnostic::error(format!(
                    "machine call cycle: `{}` -- machine call cycles are banned \
                     (stack size must be predictable), even when specialization \
                     could unroll this one. Fold the cycle into ONE machine whose \
                     sub-states loop by transition: states are jumps, not calls.{}",
                    names.join("` -> `"),
                    qualification,
                )));
            }
        } else if color[next] == 0 {
            dfs_report_cycles(
                program, edges, edge_is_tail, edge_decreases, next, color, path, reported,
                diagnostics,
            );
        }
    }
    path.pop();
    color[node] = 2;
}

/// Collect this statement's cross-machine call edges into `out`: statement
/// calls, value-position calls in every expression slot, and `self.X(..)`
/// transition/match arm targets.
fn collect_statement_edges(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    index_of: &HashMap<u32, usize>,
    statement: &StatementNode,
    zero_excluded: &[String],
    out: &mut BTreeMap<usize, (bool, bool)>,
) {
    match statement {
        // Proof obligations do not execute their expression calls and cannot
        // introduce a runtime recursion edge.
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Call(call) => {
            let receiver_members = program.statement_table.name_path_members(call.receiver);
            if receiver_members.is_empty()
                || matches!(receiver_members, [receiver] if receiver.as_str() == "self")
            {
                add_edge_for_name(
                    program, machine, symbols, index_of, &call.target, false, false, out,
                );
            }
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_edges(program, machine, symbols, index_of, *argument, out);
            }
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_edges(program, machine, symbols, index_of, assignment.value, out);
        }
        StatementNode::Expression(expression) => {
            collect_expression_edges(program, machine, symbols, index_of, *expression, out);
        }
        StatementNode::LocalData(local_data) => {
            collect_expression_edges(
                program,
                machine,
                symbols,
                index_of,
                local_data.initial_value,
                out,
            );
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_edges(program, machine, symbols, index_of, guard, out);
            }
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named { path, arguments } => {
                        let members = program.statement_table.name_path_members(path.members);
                        // `-> self.X(..)`: the Nested arm-target spelling.
                        if let [receiver, target] = members
                            && receiver.as_str() == "self"
                        {
                            // The arm target is the arm's LAST action: a
                            // TAIL call edge (MR4 qualification input). The
                            // v1 decrease proof reads the site directly.
                            let decreases = tail_edge_decrease_proven(
                                program,
                                machine,
                                symbols,
                                target,
                                *arguments,
                                zero_excluded,
                            );
                            add_edge_for_name(
                                program, machine, symbols, index_of, target, true, decreases, out,
                            );
                        }
                        for argument in program.statement_table.expression_handles(*arguments) {
                            collect_expression_edges(
                                program, machine, symbols, index_of, *argument, out,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        collect_expression_edges(
                            program, machine, symbols, index_of, *expression, out,
                        );
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

fn collect_expression_edges(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    index_of: &HashMap<u32, usize>,
    expression: ExpressionHandle,
    out: &mut BTreeMap<usize, (bool, bool)>,
) {
    if !expression.is_valid() {
        return;
    }
    let recurse = |handle: ExpressionHandle, out: &mut BTreeMap<usize, (bool, bool)>| {
        collect_expression_edges(program, machine, symbols, index_of, handle, out);
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            // A bare or `self`-received value call (`self.pong(n - 1)`).
            let receiver_is_selfish = !call.receiver.is_valid()
                || matches!(
                    program.expression_table.expression(call.receiver),
                    ExpressionNode::Name(path)
                        if matches!(
                            program.expression_table.name_path_members(path.members),
                            [only] if only.as_str() == "self"
                        )
                );
            if receiver_is_selfish {
                add_edge_for_name(
                    program, machine, symbols, index_of, &call.target, false, false, out,
                );
            } else {
                recurse(call.receiver, out);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument, out);
            }
        }
        ExpressionNode::Binary(binary) => {
            recurse(binary.left, out);
            recurse(binary.right, out);
        }
        ExpressionNode::Cast(cast) => recurse(cast.value, out),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection, out);
            recurse(indexed.index, out);
        }
        ExpressionNode::Member(member) => recurse(member.receiver, out),
        ExpressionNode::Mutable(inner) => recurse(*inner, out),
        ExpressionNode::Range(range) => {
            recurse(range.start, out);
            recurse(range.end, out);
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand, out),
        ExpressionNode::ArrayLiteral(items) => {
            for item in program.expression_table.expression_handles(*items) {
                recurse(*item, out);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program.expression_table.struct_fields(struct_literal.fields) {
                recurse(field.value, out);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

/// Resolve a called NAME to a cross-machine edge. Internal targets (the
/// current machine's own states, incl. its entry -- the Q7 fence owns that
/// spelling) and unresolvable names add nothing.
#[allow(clippy::too_many_arguments)]
fn add_edge_for_name(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    index_of: &HashMap<u32, usize>,
    name: &omega_typed_trees::name::Identifier,
    is_tail: bool,
    decreases: bool,
    out: &mut BTreeMap<usize, (bool, bool)>,
) {
    let callee = machine
        .attached_data
        .as_ref()
        .and_then(|attached_data| {
            symbols.attached_machine_state(program, attached_data.as_str(), name.as_str())
        })
        .map(|(callee_machine, _)| callee_machine)
        .or_else(|| {
            crate::calls::free_machine_entry_state(program, symbols, name.as_str())
                .map(|(callee_machine, _)| callee_machine)
        });
    if let Some(callee_machine) = callee
        && callee_machine.symbol != machine.symbol
        && let Some(&index) = index_of.get(&callee_machine.symbol.arena_index())
    {
        // Every call site between the pair must be a tail transition arm
        // target (and decrease-proven) for the EDGE to classify (MR4).
        let entry = out.entry(index).or_insert((is_tail, decreases));
        entry.0 = entry.0 && is_tail;
        entry.1 = entry.1 && decreases;
    }
}

/// The v1 guard shape: `name == 0` (either operand order) over an integer
/// literal zero. Returns the excluded name.
fn equals_zero_subject(program: &TypedTrees, guard: ExpressionHandle) -> Option<String> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    if binary.operator != omega_typed_trees::expression::BinaryOperator::Equal {
        return None;
    }
    // The bool-subject arm lowers as `(subject) == true`: unwrap the outer
    // equality when one side is the boolean literal.
    let bool_true = |handle: ExpressionHandle| {
        matches!(
            program.expression_table.expression(handle),
            ExpressionNode::Boolean(true)
        )
    };
    if bool_true(binary.right) {
        return equals_zero_subject(program, binary.left);
    }
    if bool_true(binary.left) {
        return equals_zero_subject(program, binary.right);
    }
    let single_name = |handle: ExpressionHandle| -> Option<String> {
        let ExpressionNode::Name(path) = program.expression_table.expression(handle) else {
            return None;
        };
        match program.expression_table.name_path_members(path.members) {
            [only] => Some(only.as_str().to_owned()),
            _ => None,
        }
    };
    let is_zero = |handle: ExpressionHandle| -> bool {
        matches!(
            program.expression_table.expression(handle),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(0)
        )
    };
    if let Some(name) = single_name(binary.left)
        && is_zero(binary.right)
    {
        return Some(name);
    }
    if let Some(name) = single_name(binary.right)
        && is_zero(binary.left)
    {
        return Some(name);
    }
    None
}

/// MR4 admission v1: prove the tail edge strictly decreases the CALLEE's
/// measure. The recognized shape is deliberately tight (over-refusal safe):
/// caller and callee each rank a SINGLE unsigned subject (their own measure
/// parameter); the argument delivered at the callee's measure position is
/// exactly `m - 1` where `m` is the CALLER's measure subject; and a
/// preceding arm in the same state excluded `m == 0`, so `m >= 1` and the
/// monus is a true decrease. Anything else returns false and the cycle
/// keeps its refusal (with the shape verdict naming this edge).
fn tail_edge_decrease_proven(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    target: &omega_typed_trees::name::Identifier,
    arguments: omega_core::arena::HandleSpan<ExpressionHandle>,
    zero_excluded: &[String],
) -> bool {
    let debug = std::env::var_os("OMEGA_MR4_TRACE").is_some();
    // The caller's own single measure subject.
    let Some(caller_witness) = machine.termination_plan.implementation_witness.as_ref() else {
        if debug { eprintln!("MR4 {}->{}: no caller witness", machine.name, target.as_str()); }
        return false;
    };
    if debug {
        eprintln!(
            "MR4 {}->{}: caller subjects {:?} zero_excluded {:?}",
            machine.name, target.as_str(), caller_witness.subjects, zero_excluded
        );
    }
    let [caller_subject] = caller_witness.subjects.as_slice() else {
        return false;
    };
    if !zero_excluded.iter().any(|name| name == caller_subject) {
        return false;
    }
    // The callee + its single measure subject and that subject's parameter
    // position in the callee's entry state (the receiver is not part of the
    // arm-target argument list).
    let Some((callee_machine, callee_entry)) = machine
        .attached_data
        .as_ref()
        .and_then(|attached_data| {
            symbols.attached_machine_state(program, attached_data.as_str(), target.as_str())
        })
        .or_else(|| crate::calls::free_machine_entry_state(program, symbols, target.as_str()))
    else {
        return false;
    };
    let Some(callee_witness) = callee_machine
        .termination_plan
        .implementation_witness
        .as_ref()
    else {
        return false;
    };
    let [callee_subject] = callee_witness.subjects.as_slice() else {
        return false;
    };
    let parameters = program.state_parameters(callee_entry);
    let Some(measure_position) = parameters
        .iter()
        .filter(|parameter| parameter.name.as_str() != "self")
        .position(|parameter| parameter.name.as_str() == callee_subject.as_str())
    else {
        return false;
    };
    let argument_handles = program.statement_table.expression_handles(arguments);
    let Some(argument) = argument_handles.get(measure_position) else {
        return false;
    };
    // The argument must be exactly `caller_subject - 1`.
    let ExpressionNode::Binary(binary) = program.expression_table.expression(*argument) else {
        return false;
    };
    if binary.operator != omega_typed_trees::expression::BinaryOperator::Subtract {
        return false;
    }
    let ExpressionNode::Name(left_path) = program.expression_table.expression(binary.left)
    else {
        return false;
    };
    let [left_name] = program.expression_table.name_path_members(left_path.members) else {
        return false;
    };
    if left_name.as_str() != caller_subject.as_str() {
        return false;
    }
    matches!(
        program.expression_table.expression(binary.right),
        ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
    )
}
