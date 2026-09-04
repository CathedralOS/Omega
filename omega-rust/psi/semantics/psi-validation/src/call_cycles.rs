//! The language-guide runtime-cycle ruling (chapter 3) bans ordinary machine
//! CALL cycles unless the MR4 constant-stack tail-cycle admission proves
//! every required edge. The specializer already refuses UNBOUNDED cycles
//! ("calls into a recursive cycle"), but a bounded
//! `A -> B -> A` -- the dungeon's old find_item_at/find_item_after pair,
//! spelled through arm-target `self.SIBLING(..)` calls -- was absorbed by
//! clone specialization and compiled. Absorbable does not make it legal
//! Omega: calls are stack-based; repetition is a STATE transition. This walk
//! sees every call spelling (statement calls, value-position calls, and
//! `self.X(..)` transition/match arm targets), builds the machine-level call
//! graph, and rejects any unqualified runtime cycle with the cycle path named.
//! Proof-only machines are a separate stratum: they emit no frames and may
//! form non-tail SCCs only when every member is structurally measured and every
//! edge passes a strict case-payload subterm to the callee's ranked parameter.

use crate::symbols::TopLevelSymbols;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_typed_trees::types::TypeReferenceHandle;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// The one closed ranking relation currently admitted for a proof-only call
/// SCC. The ranked value must have one common normalized type across every
/// member and each internal call must pass a nonempty member path rooted at
/// the caller's ranked parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValidatedProofRankingRelation {
    StructuralSubterm,
}

/// Exact source-independent coordinate of one proof-SCC call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatedProofRecursiveCallSite {
    Statement {
        state: SymbolHandle,
        statement_index: usize,
    },
    Expression {
        state: SymbolHandle,
        statement_index: usize,
        /// Preorder expression-node ordinal within the owning statement.
        expression_ordinal: usize,
    },
    Transition {
        state: SymbolHandle,
        statement_index: usize,
        lane: ValidatedProofRecursiveTransitionLane,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValidatedProofRecursiveTransitionLane {
    Target,
    Continuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProofRecursiveMember {
    pub machine: SymbolHandle,
    pub rank_parameter: SymbolHandle,
}

/// One exact internal call-site decrease witness. A machine pair may occur
/// more than once; no pair-level Boolean is an adequate certificate input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProofRecursiveEdge {
    pub caller: SymbolHandle,
    pub callee: SymbolHandle,
    pub site: ValidatedProofRecursiveCallSite,
    pub caller_rank_parameter: SymbolHandle,
    pub callee_rank_parameter: SymbolHandle,
    /// Exact resolved member declarations from the ranked root to the strict
    /// descendant, never source member spellings.
    pub strict_member_path: Vec<SymbolHandle>,
}

/// One validator-owned strongly connected proof component. The common rank
/// type and relation occur once; every exact internal call site has its own
/// decrease witness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProofRecursiveComponent {
    pub members: Vec<ValidatedProofRecursiveMember>,
    pub ranking_relation: ValidatedProofRankingRelation,
    pub rank_type_identity: String,
    pub edges: Vec<ValidatedProofRecursiveEdge>,
}

#[derive(Debug, Clone)]
struct ProofEdgeDecreaseWitness {
    caller_rank_parameter: SymbolHandle,
    callee_rank_parameter: SymbolHandle,
    rank_type_identity: String,
    strict_member_path: Vec<SymbolHandle>,
}

#[derive(Debug, Clone)]
struct ExactProofCallEdge {
    caller: usize,
    callee: usize,
    site: ValidatedProofRecursiveCallSite,
    decrease: Option<ProofEdgeDecreaseWitness>,
}

/// Return every exact machine/state symbol called from `machine`, independent
/// of whether the call executes at runtime.  Unlike the cycle graph, proof
/// provenance must include calls in assembly facts and all value/terminal
/// expression positions: an admitted theorem does not become checked merely
/// because its citation is nested inside another expression.
pub fn machine_call_dependency_symbols(
    program: &TypedTrees,
    machine: &Machine,
) -> Vec<SymbolHandle> {
    let mut symbols = Vec::new();
    collect_contract_dependency_symbols(program, program.machine_contracts(machine), &mut symbols);
    for state in program.machine_states(machine) {
        collect_contract_dependency_symbols(program, program.state_contracts(state), &mut symbols);
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_dependency_symbols(program, statement, &mut symbols);
        }
    }
    symbols.sort_unstable_by_key(|symbol| symbol.arena_index());
    symbols.dedup();
    symbols
}

fn collect_contract_dependency_symbols(
    program: &TypedTrees,
    contracts: &[psi_typed_trees::signature::SignatureContract],
    symbols: &mut Vec<SymbolHandle>,
) {
    for fact in contracts
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
    {
        match fact {
            psi_typed_trees::domain::ProofFact::Expression(expression) => {
                collect_expression_dependency_symbols(program, *expression, symbols);
            }
            psi_typed_trees::domain::ProofFact::Membership(membership) => {
                collect_expression_dependency_symbols(program, membership.value, symbols);
            }
            psi_typed_trees::domain::ProofFact::Proposition(application) => {
                for argument in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    collect_expression_dependency_symbols(program, *argument, symbols);
                }
            }
        }
    }
}

fn push_dependency_symbol(symbols: &mut Vec<SymbolHandle>, symbol: SymbolHandle) {
    if symbol.is_valid() {
        symbols.push(symbol);
    }
}

fn collect_statement_dependency_symbols(
    program: &TypedTrees,
    statement: &StatementNode,
    symbols: &mut Vec<SymbolHandle>,
) {
    match statement {
        StatementNode::AssemblyFact(fact) => {
            collect_expression_dependency_symbols(program, fact.expression, symbols)
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_dependency_symbols(program, assignment.target, symbols);
            collect_expression_dependency_symbols(program, assignment.value, symbols);
        }
        StatementNode::Call(call) => {
            push_dependency_symbol(symbols, call.target_symbol);
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_dependency_symbols(program, *argument, symbols);
            }
        }
        StatementNode::Expression(handle) => {
            collect_expression_dependency_symbols(program, *handle, symbols)
        }
        StatementNode::LocalData(local) => {
            collect_expression_dependency_symbols(program, local.initial_value, symbols)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_dependency_symbols(program, guard, symbols);
            }
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        push_dependency_symbol(symbols, path.symbol);
                        for argument in program.statement_table.expression_handles(*arguments) {
                            collect_expression_dependency_symbols(program, *argument, symbols);
                        }
                    }
                    TransitionTargetNode::Value(handle) => {
                        collect_expression_dependency_symbols(program, *handle, symbols)
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

fn collect_expression_dependency_symbols(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbols: &mut Vec<SymbolHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            collect_expression_dependency_symbols(program, atomic.value, symbols)
        }
        ExpressionNode::Call(call) => {
            push_dependency_symbol(symbols, call.target_symbol);
            collect_expression_dependency_symbols(program, call.receiver, symbols);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_dependency_symbols(program, *argument, symbols);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_dependency_symbols(program, binary.left, symbols);
            collect_expression_dependency_symbols(program, binary.right, symbols);
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_dependency_symbols(program, cast.value, symbols)
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_dependency_symbols(program, indexed.collection, symbols);
            collect_expression_dependency_symbols(program, indexed.index, symbols);
        }
        ExpressionNode::Member(member) => {
            collect_expression_dependency_symbols(program, member.receiver, symbols)
        }
        ExpressionNode::Borrow(inner) => {
            collect_expression_dependency_symbols(program, inner.target, symbols)
        }
        ExpressionNode::Range(range) => {
            collect_expression_dependency_symbols(program, range.start, symbols);
            collect_expression_dependency_symbols(program, range.end, symbols);
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_dependency_symbols(program, unary.operand, symbols)
        }
        ExpressionNode::ArrayLiteral(items) => {
            for item in program.expression_table.expression_handles(*items) {
                collect_expression_dependency_symbols(program, *item, symbols);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_expression_dependency_symbols(program, field.value, symbols);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

pub(crate) fn validate_machine_call_cycles(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedProofRecursiveComponent> {
    let proof_only = psi_typed_trees::proof_only::classify(program);
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
                &proof_only,
                start,
                &mut color,
                &mut Vec::new(),
                &mut reported,
                diagnostics,
            );
        }
    }

    build_validated_proof_recursive_components(program, symbols, &proof_only, &edges, &index_of)
}

fn build_validated_proof_recursive_components(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    proof_only: &psi_typed_trees::proof_only::ProofOnlyClassification,
    adjacency: &[Vec<usize>],
    index_of: &HashMap<u32, usize>,
) -> Vec<ValidatedProofRecursiveComponent> {
    let machines = program.machines();
    let exact_edges = collect_exact_proof_call_edges(program, symbols, proof_only, index_of);
    let exact_self_recursive_members = exact_edges
        .iter()
        .filter(|edge| edge.caller == edge.callee)
        .map(|edge| edge.caller)
        .collect::<BTreeSet<_>>();
    let mut components = strongly_connected_components(adjacency)
        .into_iter()
        .filter(|members| {
            (members.len() > 1
                || members
                    .first()
                    .is_some_and(|member| exact_self_recursive_members.contains(member)))
                && members
                    .iter()
                    .all(|member| proof_only.is_proof_machine(program, &machines[*member]))
        })
        .collect::<Vec<_>>();
    components.sort_by_key(|members| members.first().copied().unwrap_or(usize::MAX));

    components
        .into_iter()
        .filter_map(|members| {
            let member_set = members.iter().copied().collect::<BTreeSet<_>>();
            let mut component_edges = exact_edges
                .iter()
                .filter(|edge| {
                    member_set.contains(&edge.caller) && member_set.contains(&edge.callee)
                })
                .cloned()
                .collect::<Vec<_>>();
            component_edges.sort_by_key(|edge| {
                (
                    machines[edge.caller].symbol.arena_index(),
                    proof_call_site_sort_key(edge.site),
                    machines[edge.callee].symbol.arena_index(),
                )
            });
            if component_edges.is_empty()
                || component_edges.iter().any(|edge| edge.decrease.is_none())
            {
                return None;
            }
            let rank_type_identity = component_edges
                .first()
                .and_then(|edge| edge.decrease.as_ref())?
                .rank_type_identity
                .clone();
            if component_edges.iter().any(|edge| {
                edge.decrease
                    .as_ref()
                    .is_none_or(|witness| witness.rank_type_identity != rank_type_identity)
            }) {
                return None;
            }

            let members = members
                .into_iter()
                .map(|member| {
                    let machine = &machines[member];
                    let (rank_parameter, _, member_rank_type) =
                        proof_rank_parameter(program, machine)?;
                    (member_rank_type == rank_type_identity).then_some(
                        ValidatedProofRecursiveMember {
                            machine: machine.symbol,
                            rank_parameter,
                        },
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            let edges = component_edges
                .into_iter()
                .map(|edge| {
                    let decrease = edge.decrease?;
                    Some(ValidatedProofRecursiveEdge {
                        caller: machines[edge.caller].symbol,
                        callee: machines[edge.callee].symbol,
                        site: edge.site,
                        caller_rank_parameter: decrease.caller_rank_parameter,
                        callee_rank_parameter: decrease.callee_rank_parameter,
                        strict_member_path: decrease.strict_member_path,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(ValidatedProofRecursiveComponent {
                members,
                ranking_relation: ValidatedProofRankingRelation::StructuralSubterm,
                rank_type_identity,
                edges,
            })
        })
        .collect()
}

fn proof_call_site_sort_key(
    site: ValidatedProofRecursiveCallSite,
) -> (u8, u32, u32, usize, u32, u32) {
    match site {
        ValidatedProofRecursiveCallSite::Statement {
            state,
            statement_index,
        } => (
            1,
            state.arena_index(),
            state.generation(),
            statement_index,
            0,
            0,
        ),
        ValidatedProofRecursiveCallSite::Expression {
            state,
            statement_index,
            expression_ordinal,
        } => (
            2,
            state.arena_index(),
            state.generation(),
            statement_index,
            u32::try_from(expression_ordinal).unwrap_or(u32::MAX),
            0,
        ),
        ValidatedProofRecursiveCallSite::Transition {
            state,
            statement_index,
            lane,
        } => (
            3,
            state.arena_index(),
            state.generation(),
            statement_index,
            match lane {
                ValidatedProofRecursiveTransitionLane::Target => 1,
                ValidatedProofRecursiveTransitionLane::Continuation => 2,
            },
            0,
        ),
    }
}

fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    fn finish_order(
        node: usize,
        adjacency: &[Vec<usize>],
        visited: &mut [bool],
        order: &mut Vec<usize>,
    ) {
        if std::mem::replace(&mut visited[node], true) {
            return;
        }
        for next in &adjacency[node] {
            finish_order(*next, adjacency, visited, order);
        }
        order.push(node);
    }

    fn collect_component(
        node: usize,
        reverse: &[Vec<usize>],
        visited: &mut [bool],
        component: &mut Vec<usize>,
    ) {
        if std::mem::replace(&mut visited[node], true) {
            return;
        }
        component.push(node);
        for next in &reverse[node] {
            collect_component(*next, reverse, visited, component);
        }
    }

    let mut order = Vec::with_capacity(adjacency.len());
    let mut visited = vec![false; adjacency.len()];
    for node in 0..adjacency.len() {
        finish_order(node, adjacency, &mut visited, &mut order);
    }
    let mut reverse = vec![Vec::new(); adjacency.len()];
    for (source, targets) in adjacency.iter().enumerate() {
        for target in targets {
            reverse[*target].push(source);
        }
    }
    for targets in &mut reverse {
        targets.sort_unstable();
        targets.dedup();
    }
    visited.fill(false);
    let mut components = Vec::new();
    while let Some(node) = order.pop() {
        if visited[node] {
            continue;
        }
        let mut component = Vec::new();
        collect_component(node, &reverse, &mut visited, &mut component);
        component.sort_unstable();
        components.push(component);
    }
    components
}

fn collect_exact_proof_call_edges(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    proof_only: &psi_typed_trees::proof_only::ProofOnlyClassification,
    index_of: &HashMap<u32, usize>,
) -> Vec<ExactProofCallEdge> {
    let mut edges = Vec::new();
    for (caller, machine) in program.machines().iter().enumerate() {
        if !proof_only.is_proof_machine(program, machine) {
            continue;
        }
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                collect_exact_proof_statement_edges(
                    program,
                    machine,
                    state.symbol,
                    statement_index,
                    statement,
                    symbols,
                    index_of,
                    caller,
                    &mut edges,
                );
            }
        }
    }
    edges
}

#[allow(clippy::too_many_arguments)]
fn collect_exact_proof_statement_edges(
    program: &TypedTrees,
    machine: &Machine,
    state: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    symbols: &TopLevelSymbols<'_>,
    index_of: &HashMap<u32, usize>,
    caller: usize,
    edges: &mut Vec<ExactProofCallEdge>,
) {
    let mut expression_ordinal = 0usize;
    macro_rules! collect_expression {
        ($expression:expr) => {
            collect_exact_proof_expression_edges(
                program,
                machine,
                state,
                statement_index,
                $expression,
                symbols,
                index_of,
                caller,
                &mut expression_ordinal,
                edges,
            )
        };
    }
    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Call(call) => {
            let receiver_members = program.statement_table.name_path_members(call.receiver);
            if (receiver_members.is_empty()
                || matches!(receiver_members, [receiver] if receiver.as_str() == "self"))
                && let Some(callee) =
                    resolve_machine_index(program, machine, symbols, index_of, &call.target)
            {
                edges.push(ExactProofCallEdge {
                    caller,
                    callee,
                    site: ValidatedProofRecursiveCallSite::Statement {
                        state,
                        statement_index,
                    },
                    decrease: proof_edge_decrease_witness(
                        program,
                        machine,
                        symbols,
                        &call.target,
                        program.statement_table.expression_handles(call.arguments),
                    ),
                });
            }
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression!(*argument);
            }
        }
        StatementNode::Assignment(assignment) => collect_expression!(assignment.value),
        StatementNode::Expression(expression) => collect_expression!(*expression),
        StatementNode::LocalData(local) => collect_expression!(local.initial_value),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression!(guard);
            }
            for (target_handle, lane) in [
                (
                    transition.target,
                    ValidatedProofRecursiveTransitionLane::Target,
                ),
                (
                    transition.continuation,
                    ValidatedProofRecursiveTransitionLane::Continuation,
                ),
            ] {
                if !target_handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        let members = program.statement_table.name_path_members(path.members);
                        if let [receiver, target] = members
                            && receiver.as_str() == "self"
                            && let Some(callee) =
                                resolve_machine_index(program, machine, symbols, index_of, target)
                        {
                            edges.push(ExactProofCallEdge {
                                caller,
                                callee,
                                site: ValidatedProofRecursiveCallSite::Transition {
                                    state,
                                    statement_index,
                                    lane,
                                },
                                decrease: proof_edge_decrease_witness(
                                    program,
                                    machine,
                                    symbols,
                                    target,
                                    program.statement_table.expression_handles(*arguments),
                                ),
                            });
                        }
                        for argument in program.statement_table.expression_handles(*arguments) {
                            collect_expression!(*argument);
                        }
                    }
                    TransitionTargetNode::Value(expression) => collect_expression!(*expression),
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_exact_proof_expression_edges(
    program: &TypedTrees,
    machine: &Machine,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    index_of: &HashMap<u32, usize>,
    caller: usize,
    next_expression_ordinal: &mut usize,
    edges: &mut Vec<ExactProofCallEdge>,
) {
    if !expression.is_valid() {
        return;
    }
    let expression_ordinal = *next_expression_ordinal;
    *next_expression_ordinal = expression_ordinal.saturating_add(1);
    macro_rules! recurse {
        ($child:expr) => {
            collect_exact_proof_expression_edges(
                program,
                machine,
                state,
                statement_index,
                $child,
                symbols,
                index_of,
                caller,
                next_expression_ordinal,
                edges,
            )
        };
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => recurse!(atomic.value),
        ExpressionNode::Call(call) => {
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
                if let Some(callee) =
                    resolve_machine_index(program, machine, symbols, index_of, &call.target)
                {
                    edges.push(ExactProofCallEdge {
                        caller,
                        callee,
                        site: ValidatedProofRecursiveCallSite::Expression {
                            state,
                            statement_index,
                            expression_ordinal,
                        },
                        decrease: proof_edge_decrease_witness(
                            program,
                            machine,
                            symbols,
                            &call.target,
                            program.expression_table.expression_handles(call.arguments),
                        ),
                    });
                }
            } else {
                recurse!(call.receiver);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse!(*argument);
            }
        }
        ExpressionNode::Binary(binary) => {
            recurse!(binary.left);
            recurse!(binary.right);
        }
        ExpressionNode::Cast(cast) => recurse!(cast.value),
        ExpressionNode::Indexed(indexed) => {
            recurse!(indexed.collection);
            recurse!(indexed.index);
        }
        ExpressionNode::Member(member) => recurse!(member.receiver),
        ExpressionNode::Borrow(inner) => recurse!(inner.target),
        ExpressionNode::Range(range) => {
            recurse!(range.start);
            recurse!(range.end);
        }
        ExpressionNode::Unary(unary) => recurse!(unary.operand),
        ExpressionNode::ArrayLiteral(items) => {
            for item in program.expression_table.expression_handles(*items) {
                recurse!(*item);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                recurse!(field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn dfs_report_cycles(
    program: &TypedTrees,
    edges: &[Vec<usize>],
    edge_is_tail: &HashMap<(usize, usize), bool>,
    edge_decreases: &HashMap<(usize, usize), bool>,
    proof_only: &psi_typed_trees::proof_only::ProofOnlyClassification,
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
                    })
                    .map(|&member| format!("`{}`", machines[member].name))
                    .collect();
                let proof_cycle = cycle
                    .iter()
                    .all(|&member| proof_only.is_proof_machine(program, &machines[member]));
                // Proof-only machines emit no runtime frames, so their call
                // SCCs do not need tail-call lowering. They do still need a
                // structural well-foundedness proof: every member is measured
                // and every edge passes a strict subterm of the caller's
                // ranking subject to the callee's ranking position.
                if proof_cycle {
                    if unmeasured.is_empty() && undecreasing.is_empty() {
                        continue;
                    }
                    let reason = if !unmeasured.is_empty() {
                        format!("unmeasured proof machine(s): {}", unmeasured.join(", "),)
                    } else {
                        format!(
                            "the ranking subject does not structurally decrease on edge(s): {}",
                            undecreasing.join(", "),
                        )
                    };
                    diagnostics.push(Diagnostic::error(format!(
                        "proof-only machine call cycle: `{}` -- {reason}; every member of a \
                         proof-only call cycle must declare `terminates by <param>;`, and every \
                         edge must pass a case-payload subterm of the caller's ranking subject",
                        names.join("` -> `"),
                    )));
                    continue;
                }
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
                        parts.push(format!("non-tail call edge(s): {}", non_tail.join(", "),));
                    }
                    if !unmeasured.is_empty() {
                        parts.push(format!("unmeasured machine(s): {}", unmeasured.join(", "),));
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
                program,
                edges,
                edge_is_tail,
                edge_decreases,
                proof_only,
                next,
                color,
                path,
                reported,
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
                let decreases = proof_edge_decrease_proven(
                    program,
                    machine,
                    symbols,
                    &call.target,
                    program.statement_table.expression_handles(call.arguments),
                );
                add_edge_for_name(
                    program,
                    machine,
                    symbols,
                    index_of,
                    &call.target,
                    false,
                    decreases,
                    out,
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
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
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
                            ) || proof_edge_decrease_proven(
                                program,
                                machine,
                                symbols,
                                target,
                                program.statement_table.expression_handles(*arguments),
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
                            program,
                            machine,
                            symbols,
                            index_of,
                            *expression,
                            out,
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
        ExpressionNode::Atomic(atomic) => recurse(atomic.value, out),
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
                let decreases = proof_edge_decrease_proven(
                    program,
                    machine,
                    symbols,
                    &call.target,
                    program.expression_table.expression_handles(call.arguments),
                );
                add_edge_for_name(
                    program,
                    machine,
                    symbols,
                    index_of,
                    &call.target,
                    false,
                    decreases,
                    out,
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
        ExpressionNode::Borrow(inner) => recurse(inner.target, out),
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
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                recurse(field.value, out);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// Resolve a called NAME to a cross-machine edge. Internal targets (the
/// current machine's own states, incl. its entry -- the stable
/// `machine-self-call-cycle-ban` decision owns that fence
/// spelling) and unresolvable names add nothing.
#[allow(clippy::too_many_arguments)]
fn add_edge_for_name(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    index_of: &HashMap<u32, usize>,
    name: &psi_typed_trees::name::Identifier,
    is_tail: bool,
    decreases: bool,
    out: &mut BTreeMap<usize, (bool, bool)>,
) {
    if let Some(index) = resolve_cross_machine_index(program, machine, symbols, index_of, name) {
        // Every call site between the pair must be a tail transition arm
        // target (and decrease-proven) for the EDGE to classify (MR4).
        let entry = out.entry(index).or_insert((is_tail, decreases));
        entry.0 = entry.0 && is_tail;
        entry.1 = entry.1 && decreases;
    }
}

fn resolve_cross_machine_index(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    index_of: &HashMap<u32, usize>,
    name: &psi_typed_trees::name::Identifier,
) -> Option<usize> {
    let callee = resolve_machine_index(program, machine, symbols, index_of, name)?;
    (callee != *index_of.get(&machine.symbol.arena_index())?).then_some(callee)
}

/// Resolve an exact proof call to its owning machine, including the caller's
/// own entry. Runtime cycle classification deliberately excludes self calls;
/// proof-recursion certificates must retain them as exact induction edges.
fn resolve_machine_index(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    index_of: &HashMap<u32, usize>,
    name: &psi_typed_trees::name::Identifier,
) -> Option<usize> {
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
    let callee = callee?;
    index_of.get(&callee.symbol.arena_index()).copied()
}

/// The v1 guard shape: `name == 0` (either operand order) over an integer
/// literal zero. Returns the excluded name.
fn equals_zero_subject(program: &TypedTrees, guard: ExpressionHandle) -> Option<String> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    if binary.operator != psi_typed_trees::expression::BinaryOperator::Equal {
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

/// Proof-stratum cross-machine descent: both caller and callee declare one
/// ranking subject, and the argument delivered to the callee's ranked
/// parameter is a strict member subterm of the caller's subject (`n.prev`, or
/// a deeper member chain). Proof-only SCC admission consumes this evidence;
/// runtime SCCs continue to use the separate tail-edge MR4 proof below.
fn proof_edge_decrease_proven(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    target: &psi_typed_trees::name::Identifier,
    arguments: &[ExpressionHandle],
) -> bool {
    proof_edge_decrease_witness(program, machine, symbols, target, arguments).is_some()
}

fn proof_rank_parameter(
    program: &TypedTrees,
    machine: &Machine,
) -> Option<(SymbolHandle, TypeReferenceHandle, String)> {
    let caller_witness = machine.termination_plan.implementation_witness.as_ref()?;
    let [caller_subject] = caller_witness.subjects.as_slice() else {
        return None;
    };
    let parameter = program.machine_states(machine).first().and_then(|entry| {
        program
            .state_parameters(entry)
            .iter()
            .find(|parameter| parameter.name.as_str() == caller_subject.as_str())
    })?;
    Some((
        parameter.symbol,
        parameter.type_reference,
        program
            .package_qualified_type_identity(parameter.type_reference)
            .into_string(),
    ))
}

fn proof_edge_decrease_witness(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    target: &psi_typed_trees::name::Identifier,
    arguments: &[ExpressionHandle],
) -> Option<ProofEdgeDecreaseWitness> {
    let (caller_measure_symbol, caller_rank_type, caller_rank_type_identity) =
        proof_rank_parameter(program, machine)?;
    let caller_subject = machine
        .termination_plan
        .implementation_witness
        .as_ref()?
        .subjects
        .first()?;
    let (callee_machine, callee_entry) = machine
        .attached_data
        .as_ref()
        .and_then(|attached_data| {
            symbols.attached_machine_state(program, attached_data.as_str(), target.as_str())
        })
        .or_else(|| crate::calls::free_machine_entry_state(program, symbols, target.as_str()))?;
    let callee_witness = callee_machine
        .termination_plan
        .implementation_witness
        .as_ref()?;
    let [callee_subject] = callee_witness.subjects.as_slice() else {
        return None;
    };
    let (measure_position, callee_parameter) = program
        .state_parameters(callee_entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .find(|(_, parameter)| parameter.name.as_str() == callee_subject.as_str())?;
    let callee_rank_type_identity = program
        .package_qualified_type_identity(callee_parameter.type_reference)
        .into_string();
    if callee_rank_type_identity != caller_rank_type_identity {
        return None;
    }
    let strict_member_path = expression_strict_member_path(
        program,
        *arguments.get(measure_position)?,
        caller_measure_symbol,
        caller_subject.as_str(),
        caller_rank_type,
    );
    let strict_member_path = strict_member_path?;
    Some(ProofEdgeDecreaseWitness {
        caller_rank_parameter: caller_measure_symbol,
        callee_rank_parameter: callee_parameter.symbol,
        rank_type_identity: caller_rank_type_identity,
        strict_member_path,
    })
}

fn expression_strict_member_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
    root_symbol: SymbolHandle,
    root_name: &str,
    root_type: TypeReferenceHandle,
) -> Option<Vec<SymbolHandle>> {
    let (path, _) = expression_member_path(program, expression, root_symbol, root_name, root_type)?;
    (!path.is_empty()).then_some(path)
}

fn expression_member_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
    root_symbol: SymbolHandle,
    root_name: &str,
    root_type: TypeReferenceHandle,
) -> Option<(Vec<SymbolHandle>, TypeReferenceHandle)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            expression_member_path(program, atomic.value, root_symbol, root_name, root_type)
        }
        ExpressionNode::Cast(cast) => {
            expression_member_path(program, cast.value, root_symbol, root_name, root_type)
        }
        ExpressionNode::Member(member) => {
            let (mut path, receiver_type) = expression_member_path(
                program,
                member.receiver,
                root_symbol,
                root_name,
                root_type,
            )?;
            let field = exact_member_field(
                program,
                receiver_type,
                member.member_symbol,
                member.member.as_str(),
                member.case_variant.as_ref().map(|variant| variant.as_str()),
            )?;
            path.push(field.symbol);
            Some((path, field.type_reference))
        }
        ExpressionNode::Borrow(inner) => {
            expression_member_path(program, inner.target, root_symbol, root_name, root_type)
        }
        ExpressionNode::Name(path) => (path.symbol == root_symbol
            || (!path.symbol.is_valid()
                && matches!(
                    program.expression_table.name_path_members(path.members),
                    [only] if only.as_str() == root_name
                )))
        .then(|| (Vec::new(), root_type)),
        _ => None,
    }
}

/// Resolve one member hop to the exact declaration selected by the typed
/// receiver. Case-payload member expressions currently retain the selected
/// variant name but not the payload field symbol, so recover that symbol from
/// the unique declaration under the receiver's nominal type. The returned
/// witness never retains either spelling.
fn exact_member_field<'program>(
    program: &'program TypedTrees,
    receiver_type: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &str,
    case_variant: Option<&str>,
) -> Option<&'program psi_typed_trees::data::DataField> {
    let data = crate::places::data_definition_for_type(program, receiver_type)?;

    if let Some(case_variant) = case_variant {
        let mut matches = program.data_members(data).iter().filter_map(|member| {
            let psi_typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.name.as_str() == case_variant).then_some(variant)
        });
        let variant = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let mut fields = program.data_payload_fields(variant).iter().filter(|field| {
            field.name.as_str() == member_name
                && (!member_symbol.is_valid() || field.symbol == member_symbol)
                && field.symbol.is_valid()
                && field.type_reference.is_valid()
        });
        let field = fields.next()?;
        return fields.next().is_none().then_some(field);
    }

    let mut fields = program.data_members(data).iter().filter_map(|member| {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.name.as_str() == member_name
            && (!member_symbol.is_valid() || field.symbol == member_symbol)
            && field.symbol.is_valid()
            && field.type_reference.is_valid())
        .then_some(field)
    });
    let field = fields.next()?;
    fields.next().is_none().then_some(field)
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
    target: &psi_typed_trees::name::Identifier,
    arguments: psi_arena::HandleSpan<ExpressionHandle>,
    zero_excluded: &[String],
) -> bool {
    let debug = std::env::var_os("OMEGA_MR4_TRACE").is_some();
    // The caller's own single measure subject.
    let Some(caller_witness) = machine.termination_plan.implementation_witness.as_ref() else {
        if debug {
            eprintln!(
                "MR4 {}->{}: no caller witness",
                machine.name,
                target.as_str()
            );
        }
        return false;
    };
    if debug {
        eprintln!(
            "MR4 {}->{}: caller subjects {:?} zero_excluded {:?}",
            machine.name,
            target.as_str(),
            caller_witness.subjects,
            zero_excluded
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
    if binary.operator != psi_typed_trees::expression::BinaryOperator::Subtract {
        return false;
    }
    let ExpressionNode::Name(left_path) = program.expression_table.expression(binary.left) else {
        return false;
    };
    let [left_name] = program
        .expression_table
        .name_path_members(left_path.members)
    else {
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

#[cfg(test)]
mod dependency_tests {
    use super::machine_call_dependency_symbols;
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::domain::ProofFact;
    use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::signature::{SignatureContract, SignatureContractKind};
    use psi_typed_trees::state::State;
    use psi_typed_trees::statement::{StatementNode, TableTransition, TransitionTargetNode};

    fn call(program: &mut TypedTrees, target: u32) -> ExpressionHandle {
        let arguments = program
            .expression_table
            .insert_expression_handles(std::iter::empty());
        program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol: SymbolHandle::from_arena_index(target),
                target: Identifier::generated("proof"),
                static_requirement_dispatch: None,
                machine_arguments: Box::default(),
                quotient_operation: None,
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: Default::default(),
            }))
    }

    #[test]
    fn proof_dependencies_include_contract_and_terminal_value_calls() {
        let mut program = TypedTrees::default();
        let contract_call = call(&mut program, 41);
        let terminal_call = call(&mut program, 42);
        let facts = program
            .proof_facts
            .insert_many([ProofFact::Expression(contract_call)]);
        let mut machine = Machine {
            symbol: SymbolHandle::from_arena_index(10),
            name: Identifier::generated("checked_row"),
            ..Machine::default()
        };
        program.push_machine_contract(
            &mut machine,
            SignatureContract {
                kind: SignatureContractKind::Ensures,
                facts,
                ..SignatureContract::default()
            },
        );
        let target = program
            .statement_table
            .insert_transition_target(TransitionTargetNode::Value(terminal_call));
        let mut state = State::default();
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Transition(TableTransition {
                target,
                ..TableTransition::default()
            }),
        );
        program.push_machine_state(&mut machine, state);

        assert_eq!(
            machine_call_dependency_symbols(&program, &machine),
            [
                SymbolHandle::from_arena_index(41),
                SymbolHandle::from_arena_index(42),
            ]
        );
    }
}
