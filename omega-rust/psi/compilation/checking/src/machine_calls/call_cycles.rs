//! Runtime machine call cycles require a shared well-founded ranking under
//! the single runtime_ranking component judgment. Admission covers every
//! exact call in the entire strongly connected component; a DFS cycle subset
//! cannot supply admission for an unsafe cross-edge or parallel call site.
//! Proof-only machines are a separate stratum: they emit no frames and may
//! form non-tail SCCs only when every member is structurally measured and every
//! edge passes a strict case-payload subterm to the callee's ranked parameter.

use crate::declarations::symbols::TopLevelSymbols;
use crate::value_custody::places::exact_data_member_field;
use diagnostics::Diagnostic;
use std::collections::{BTreeSet, HashMap};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::TypeReferenceHandle;

mod rank_components;
mod rank_relevance;
pub use rank_components::{
    RuntimeRankComponents, prepare_runtime_rank_components, retain_runtime_rank_component,
    retained_internal_rank_nonincrease, retained_runtime_natural_components,
    retained_runtime_recursive_components,
};
pub(crate) mod runtime_ranking;
pub use rank_relevance::{RankRelevance, rank_relevance};
pub use runtime_ranking::{
    RuntimeRankBodyWitness, ValidatedRuntimeNaturalComponent, ValidatedRuntimeNaturalEdge,
    ValidatedRuntimeNaturalState, activation_target_state_index, internal_state_index,
};

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
    pub rank_type_identity: typed_trees::type_identity::TypeIdentityId,
    pub edges: Vec<ValidatedProofRecursiveEdge>,
}

#[derive(Debug, Clone)]
struct ProofEdgeDecreaseWitness {
    caller_rank_parameter: SymbolHandle,
    callee_rank_parameter: SymbolHandle,
    rank_type_identity: typed_trees::type_identity::TypeIdentityId,
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
    program: &dyn crate::CheckingInput,
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

/// Direct source dependencies of one exact callable or transparent term.
/// The returned owner canonicalizes a named state to its machine. Proof
/// independence follows authored contracts as well as bodies; operational
/// effect closure deliberately answers a different question. Mathematical
/// indirection cannot hide a dependency on the theorem being established.
pub(crate) fn declaration_call_dependencies(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<(SymbolHandle, Vec<SymbolHandle>)> {
    // Compiler-owned functions and core type formers have no authored body
    // that can depend on the caller. This is dependency classification only;
    // their invocation, effects and arithmetic still require formation.
    if program
        .symbols
        .builtin_function_for_symbol(symbol)
        .is_some()
        || symbols::FixedCoreDeclaration::ALL
            .into_iter()
            .any(|declaration| {
                program.symbols.fixed_core_declaration_symbol(declaration) == Some(symbol)
            })
    {
        return Some((symbol, Vec::new()));
    }
    let machine = program.machine_by_symbol(symbol).or_else(|| {
        crate::machine_calls::calls::machine_state_by_symbol(program, symbol)
            .map(|(machine, _)| machine)
    });
    if let Some(machine) = machine {
        return Some((
            machine.symbol,
            machine_call_dependency_symbols(program, machine),
        ));
    }
    let mut dependencies = Vec::new();
    if let Some(definition) = program
        .mathematical_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
    {
        for root in crate::mathematical_applications::definition_roots(program, definition) {
            collect_expression_dependency_symbols(program, root, &mut dependencies);
        }
    } else if let Some(operator) = typed_trees::operator::declaration_by_symbol(program, symbol) {
        collect_contract_dependency_symbols(
            program,
            program.operator_contracts(operator),
            &mut dependencies,
        );
    } else if let Some(signature) = program
        .trait_machine_signature(symbol)
        .map(|(_, signature)| signature)
        .or_else(|| {
            program
                .machine_parameter_signature(symbol)
                .map(|(_, signature)| signature)
        })
        .or_else(|| {
            program
                .retained_static_machine_contract(symbol)
                .map(|contract| contract.signature())
        })
    {
        collect_contract_dependency_symbols(
            program,
            program.state_signature_contracts(signature),
            &mut dependencies,
        );
    } else if let Some(proposition) = program
        .propositions()
        .iter()
        .find(|proposition| proposition.symbol == symbol)
    {
        if let typed_trees::proposition::PropositionBody::Transparent { proposition } =
            &proposition.body
        {
            match proposition {
                typed_trees::proposition::PropositionFormula::BooleanExpression(expression) => {
                    collect_expression_dependency_symbols(program, *expression, &mut dependencies);
                }
                typed_trees::proposition::PropositionFormula::Application(application) => {
                    collect_proposition_dependency_symbols(program, application, &mut dependencies);
                }
            }
        }
    } else {
        return None;
    }
    dependencies
        .sort_unstable_by_key(|dependency| (dependency.arena_index(), dependency.generation()));
    dependencies.dedup();
    Some((symbol, dependencies))
}

fn collect_contract_dependency_symbols(
    program: &dyn crate::CheckingInput,
    contracts: &[typed_trees::signature::SignatureContract],
    symbols: &mut Vec<SymbolHandle>,
) {
    for fact in contracts
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
    {
        match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                collect_expression_dependency_symbols(program, *expression, symbols);
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                collect_expression_dependency_symbols(program, membership.value, symbols);
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
                collect_proposition_dependency_symbols(program, application, symbols);
            }
        }
    }
}

fn collect_proposition_dependency_symbols(
    program: &dyn crate::CheckingInput,
    application: &typed_trees::proposition::PropositionApplication,
    symbols: &mut Vec<SymbolHandle>,
) {
    push_dependency_symbol(symbols, application.proposition);
    for argument in program
        .expression_table
        .expression_handles(application.arguments)
    {
        collect_expression_dependency_symbols(program, *argument, symbols);
    }
    for argument in &application.binder_arguments {
        if argument.kind == typed_trees::proposition::PropositionBinderArgumentKind::Machine {
            push_dependency_symbol(symbols, argument.symbol);
        }
    }
}

fn push_dependency_symbol(symbols: &mut Vec<SymbolHandle>, symbol: SymbolHandle) {
    if symbol.is_valid() {
        symbols.push(symbol);
    }
}

fn collect_statement_dependency_symbols(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
    symbols: &mut Vec<SymbolHandle>,
) {
    match statement {
        StatementNode::Assembly(_) => {}
        StatementNode::RootBinding(binding) => {
            for expression in [binding.receiver, binding.implementation_operand] {
                if expression.is_valid() {
                    collect_expression_dependency_symbols(program, expression, symbols);
                }
            }
        }
        StatementNode::AssemblyFact(fact) => {
            for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                collect_expression_dependency_symbols(program, expression, symbols);
            }
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_dependency_symbols(program, assignment.target, symbols);
            collect_expression_dependency_symbols(program, assignment.value, symbols);
        }
        StatementNode::Call(call) => {
            push_dependency_symbol(symbols, call.target_symbol);
            symbols.extend(
                super::machine_parameters::machine_selection_dependency_symbols(
                    program,
                    &call.machine_arguments,
                ),
            );
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
            for target_handle in transition.targets() {
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
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

fn collect_expression_dependency_symbols(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    symbols: &mut Vec<SymbolHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            collect_expression_dependency_symbols(program, dispatch.subject, symbols);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    collect_expression_dependency_symbols(program, pattern, symbols);
                }
                collect_expression_dependency_symbols(program, arm.value, symbols);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression_dependency_symbols(program, atomic.value, symbols)
        }
        ExpressionNode::Call(call) => {
            push_dependency_symbol(symbols, call.target_symbol);
            symbols.extend(
                super::machine_parameters::machine_selection_dependency_symbols(
                    program,
                    &call.machine_arguments,
                ),
            );
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
        typed_trees::literal_expression_pattern!() => {}
    }
}

/// Temporary graph construction shared by validation and admission queries.
struct MachineCallGraph {
    edges: Vec<Vec<usize>>,
    index_of: HashMap<u32, usize>,
    proof_dependencies: Vec<Vec<usize>>,
    selector_self_recursion: Vec<usize>,
}

fn build_machine_call_graph(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
) -> MachineCallGraph {
    let machines = program.machines();
    let mut index_of: HashMap<u32, usize> = HashMap::with_capacity(machines.len());
    for (index, machine) in machines.iter().enumerate() {
        index_of.insert(machine.symbol.arena_index(), index);
    }

    let proof_dependencies = collect_proof_call_dependencies(program, proof_only);
    let mut edges = proof_dependencies.clone();
    // Resolved dependencies keep unsupported proof call spellings visible;
    // exact structural witnesses remain the only proof admission authority.
    for edge in collect_exact_proof_call_edges(program, symbols, proof_only, &index_of) {
        edges[edge.caller].push(edge.callee);
    }
    for targets in &mut edges {
        targets.sort_unstable();
        targets.dedup();
    }
    let selector_self_recursion =
        runtime_ranking::extend_runtime_adjacency(program, proof_only, &mut edges);
    MachineCallGraph {
        edges,
        index_of,
        proof_dependencies,
        selector_self_recursion,
    }
}

pub(crate) fn validate_machine_call_cycles(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedProofRecursiveComponent> {
    let proof_only = crate::proof_only_classification(program);
    let machines = program.machines();
    let MachineCallGraph {
        edges,
        index_of,
        proof_dependencies,
        selector_self_recursion,
    } = build_machine_call_graph(program, symbols, &proof_only);
    let proof_components = build_validated_proof_recursive_components(
        program,
        symbols,
        &proof_only,
        &edges,
        &index_of,
        &proof_dependencies,
    );

    for members in strongly_connected_components(&edges) {
        // Runtime singletons retain their actual call edge and use the same
        // completed-effects component judgment as mutual recursion. Proof
        // self-recursion keeps its independent structural-descent judgment.
        if members.len() == 1 {
            let member = members[0];
            // The prepared graph retains selector-edge provenance separately
            // from direct self calls. No source spelling or singleton shape
            // can supply the missing implementation-selection ranking.
            if selector_self_recursion.contains(&member) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine call cycle: `{}` can re-enter itself through a selector call; \
                     termination across runtime implementation selection is not yet ranked",
                    machines[member].name
                )));
            }
            continue;
        }
        let names = members
            .iter()
            .map(|member| machines[*member].name.as_str())
            .collect::<Vec<_>>()
            .join("`, `");
        if members
            .iter()
            .all(|member| proof_only.is_proof_machine(program, &machines[*member]))
        {
            if !proof_components.iter().any(|component| {
                component.members.len() == members.len()
                    && members.iter().all(|member| {
                        component
                            .members
                            .iter()
                            .any(|validated| validated.machine == machines[*member].symbol)
                    })
            }) {
                let reason = if members.iter().any(|member| {
                    machines[*member]
                        .termination_plan
                        .implementation_witness
                        .is_none()
                }) {
                    "unmeasured proof machine in component"
                } else {
                    "the ranking subject does not structurally decrease on every exact edge"
                };
                diagnostics.push(Diagnostic::error(format!(
                    "proof-only machine call cycle: `{names}` -- {reason}; every member must declare \
                     `terminates by <param>;`, and every edge must pass a case-payload subterm \
                     of the caller's ranking subject"
                )));
            }
        }
        // Runtime and mixed components keep their exact topology here. Their
        // complete edge judgment waits for completed effects and each body's
        // retained arrival choice. Proof recursion above remains early.
    }
    proof_components
}

fn build_validated_proof_recursive_components(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
    adjacency: &[Vec<usize>],
    index_of: &HashMap<u32, usize>,
    proof_dependencies: &[Vec<usize>],
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
            // Compare occurrences per machine pair, not just the component's
            // total. A missing selected callee or parallel receiver call must
            // not borrow another pair's structural certificate. Same-machine
            // dependency targets also include state jumps, whose existing
            // recursion judgment remains separate from this completeness fence.
            let mut certified_calls = component_edges
                .iter()
                .filter(|edge| edge.caller != edge.callee)
                .map(|edge| (edge.caller, edge.callee))
                .collect::<Vec<_>>();
            let mut resolved_calls = members
                .iter()
                .flat_map(|caller| {
                    proof_dependencies[*caller]
                        .iter()
                        .filter(|callee| member_set.contains(callee))
                        .map(|callee| (*caller, *callee))
                })
                .collect::<Vec<_>>();
            certified_calls.sort_unstable();
            resolved_calls.sort_unstable();
            if certified_calls != resolved_calls {
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

fn collect_proof_call_dependencies(
    program: &dyn crate::CheckingInput,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
) -> Vec<Vec<usize>> {
    let machines = program.machines();
    let mut dependencies = vec![Vec::new(); machines.len()];
    for (caller, machine) in machines.iter().enumerate() {
        if !proof_only.is_proof_machine(program, machine) {
            continue;
        }
        let mut targets = Vec::new();
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if !matches!(statement, StatementNode::AssemblyFact(_)) {
                    collect_statement_dependency_symbols(program, statement, &mut targets);
                }
            }
        }
        for target in targets {
            if let Some(callee) = machines.iter().position(|candidate| {
                candidate.symbol == target
                    || typed_trees::state::state_by_symbol(
                        program.machine_states(candidate),
                        target,
                    )
                    .is_some()
            }) && callee != caller
            {
                // Keep duplicates: each occurrence needs its own witness.
                dependencies[caller].push(callee);
            }
        }
    }
    dependencies
}

fn collect_exact_proof_call_edges(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
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
                .indexed_statements(state.statement_nodes)
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
    program: &dyn crate::CheckingInput,
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
        StatementNode::RootBinding(binding) => {
            for expression in [binding.receiver, binding.implementation_operand] {
                if expression.is_valid() {
                    collect_expression!(expression);
                }
            }
        }
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
        StatementNode::Call(call) => {
            let receiver_members = program.statement_table.name_path_members(call.receiver);
            if (receiver_members.is_empty()
                || matches!(receiver_members, [receiver] if receiver.is_self_receiver()))
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
                            && receiver.is_self_receiver()
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
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_exact_proof_expression_edges(
    program: &dyn crate::CheckingInput,
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
                            [only] if only.is_self_receiver()
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
        other => crate::expression_traversal::children(program, other, |child| {
            recurse!(child);
        }),
    }
}

/// Resolve an exact proof call to its owning machine, including the caller's
/// own entry. Runtime cycle classification deliberately excludes self calls;
/// proof-recursion certificates must retain them as exact induction edges.
fn resolve_machine_index(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    index_of: &HashMap<u32, usize>,
    name: &typed_trees::name::Identifier,
) -> Option<usize> {
    let callee = machine
        .attached_data
        .as_ref()
        .and_then(|attached_data| {
            symbols.attached_machine_state(program, attached_data.as_str(), name.as_str())
        })
        .map(|(callee_machine, _)| callee_machine)
        .or_else(|| {
            crate::machine_calls::calls::free_machine_entry_state(program, symbols, name.as_str())
                .map(|(callee_machine, _)| callee_machine)
        });
    let callee = callee?;
    index_of.get(&callee.symbol.arena_index()).copied()
}

fn proof_rank_parameter(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Option<(
    SymbolHandle,
    TypeReferenceHandle,
    typed_trees::type_identity::TypeIdentityId,
)> {
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
        program.package_qualified_type_identity(parameter.type_reference),
    ))
}

fn proof_edge_decrease_witness(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    target: &typed_trees::name::Identifier,
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
        .or_else(|| {
            crate::machine_calls::calls::free_machine_entry_state(program, symbols, target.as_str())
        })?;
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
        .enumerate()
        .find(|(_, parameter)| parameter.name.as_str() == callee_subject.as_str())?;
    let callee_rank_type_identity =
        program.package_qualified_type_identity(callee_parameter.type_reference);
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    root_symbol: SymbolHandle,
    root_name: &str,
    root_type: TypeReferenceHandle,
) -> Option<Vec<SymbolHandle>> {
    let (path, _) = expression_member_path(program, expression, root_symbol, root_name, root_type)?;
    (!path.is_empty()).then_some(path)
}

fn expression_member_path(
    program: &dyn crate::CheckingInput,
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
    program: &'program dyn crate::CheckingInput,
    receiver_type: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &str,
    case_variant: Option<&str>,
) -> Option<&'program typed_trees::data::DataField> {
    let data = crate::value_custody::places::data_definition_for_type(program, receiver_type)?;
    exact_data_member_field(program, data, member_symbol, member_name, case_variant)
}
