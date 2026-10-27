//! Runtime machine call cycles require terminal transfers at every internal
//! occurrence. A termination promise additionally requires a shared well-founded
//! ranking under the runtime_ranking component judgment. Admission covers every
//! exact call in the entire strongly connected component; a DFS cycle subset
//! cannot supply admission for an unsafe cross-edge or parallel call site.
//! Proof-only machines are a separate stratum: they emit no frames and may
//! form non-tail SCCs only when exact argument transport establishes a common
//! structural rank and every cycle strictly decreases it. Private helper ranks
//! may be inferred; lexical ancestry contributes no ranking evidence.

use super::graph_components::strongly_connected_components;
use crate::value_custody::places::exact_data_member_field;
use diagnostics::Diagnostic;
use std::collections::BTreeSet;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::TypeReferenceHandle;

mod proof_components;
pub(crate) use proof_components::structural_return_descends;
mod proof_intervals;
mod proof_ranking;
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
/// SCC. The ranked value has one common normalized type across every member.
/// The relation distinguishes per-edge strictness from every-cycle strictness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValidatedProofRankingRelation {
    StructuralSubterm,
    /// Every edge preserves or descends; rank-preserving edges form a DAG.
    StructuralSubtermCycles,
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
    /// Exact resolved member declarations from the ranked root. Empty means
    /// preservation and is allowed only by StructuralSubtermCycles.
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

/// Source progress evidence for a complete proof-call component. Computed
/// decreases are source judgments, not structural paths or portable proof
/// certificates. Only `structural` may be projected into that certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProofCallComponent {
    pub members: Vec<SymbolHandle>,
    pub structural: Option<ValidatedProofRecursiveComponent>,
    pub intervals: Vec<ValidatedProofRankInterval>,
}

/// An exact arrival at a ranked entry, checked against every path from the
/// preceding ranked entry without using the arriving call's conclusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProofRankInterval {
    pub origin: SymbolHandle,
    pub origin_rank: SymbolHandle,
    pub caller: SymbolHandle,
    pub callee: SymbolHandle,
    pub callee_rank: SymbolHandle,
    pub site: ValidatedProofRecursiveCallSite,
    pub argument: ExpressionHandle,
    pub relation: SymbolHandle,
}

#[derive(Debug, Clone)]
struct ExactProofCallEdge {
    caller: usize,
    callee: usize,
    site: ValidatedProofRecursiveCallSite,
    arguments: Vec<ExpressionHandle>,
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
        let mut dependencies = machine_call_dependency_symbols(program, machine);
        dependencies.extend(
            super::machine_parameters::specialization_selection_dependency_symbols(
                program,
                machine.symbol,
            )?,
        );
        if machine.target_family_interface {
            // A portable guarantee depends on every retained member, even
            // though the interface itself has no body. This prevents a
            // member from proving its caller through the empty family head.
            dependencies.extend(
                super::calls::target_family_members(program, machine)?
                    .iter()
                    .map(|member| member.machine),
            );
        }
        return Some((machine.symbol, dependencies));
    }
    let mut dependencies = Vec::new();
    // A selector call targets its instantiated slot, and the requirement that
    // slot instantiates owns the call's contract -- `CalleeContract`,
    // `contract_target_from_state_symbol` and `selected_contract_by_symbol`
    // all resolve a slot that way. The requirement's declared contracts are
    // therefore the call's proof sources: a selector's runtime candidate
    // carries no guarantee beyond the requirement it satisfies, and the
    // implementation roster is reached through the call's machine arguments
    // when it is a retained selection. Keep the slot as graph owner so
    // distinct slots do not collapse into the one requirement declaration.
    if let Some(requirement) = program
        .dynamic_slot_by_symbol(symbol)
        .and_then(|slot| program.dynamic_slot_requirement(slot))
    {
        collect_contract_dependency_symbols(
            program,
            program.state_signature_contracts(requirement),
            &mut dependencies,
        );
    } else if let Some(definition) = program
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
    // An evidence-child state is a logical requirement, not an executable
    // machine row. Rejoin its unique authored bound and complete application
    // through the same owner used by call admission. Its full public contracts
    // remain dependencies; neither missing states nor provider bodies supply
    // independence. Keep `symbol` as graph owner so distinct evidence binders
    // do not collapse into the trait's nominal declaration.
    } else if let Some(signature) = crate::selected_requirement_signature(program, symbol)
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
                super::machine_parameters::call_selection_dependency_symbols(
                    program,
                    typed_trees::references::StaticArgumentCall::Statement(call),
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
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                    collect_expression_dependency_symbols(program, pattern, symbols);
                }
                collect_expression_dependency_symbols(program, arm.value, symbols);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            for child in
                crate::value_custody::atomic_operations::atomic_execution_children(program, atomic)
            {
                collect_expression_dependency_symbols(program, child, symbols);
            }
        }
        ExpressionNode::Call(call) => {
            push_dependency_symbol(symbols, call.target_symbol);
            symbols.extend(
                super::machine_parameters::call_selection_dependency_symbols(
                    program,
                    typed_trees::references::StaticArgumentCall::Expression(expression, call),
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
        ExpressionNode::SavedSubject(_) | typed_trees::literal_expression_pattern!() => {}
    }
}

/// Temporary graph construction shared by validation and admission queries.
struct MachineCallGraph {
    edges: Vec<Vec<usize>>,
    proof_dependencies: Vec<Vec<usize>>,
    selector_self_recursion: Vec<usize>,
}

fn build_machine_call_graph(
    program: &dyn crate::CheckingInput,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
) -> MachineCallGraph {
    let proof_dependencies = collect_proof_call_dependencies(program, proof_only);
    let mut edges = proof_dependencies.clone();
    // Resolved dependencies keep unsupported proof call spellings visible;
    // exact structural witnesses remain the only proof admission authority.
    for edge in collect_exact_proof_call_edges(program, proof_only) {
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
        proof_dependencies,
        selector_self_recursion,
    }
}

pub(crate) fn validate_machine_call_cycles(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedProofCallComponent> {
    let proof_only = crate::proof_only_classification(program);
    let machines = program.machines();
    let MachineCallGraph {
        edges,
        proof_dependencies,
        selector_self_recursion,
    } = build_machine_call_graph(program, &proof_only);
    let mut ranking_failures = Vec::new();
    let proof_components = build_validated_proof_recursive_components(
        program,
        &proof_only,
        &edges,
        &proof_dependencies,
        &mut ranking_failures,
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
                            .any(|validated| *validated == machines[*member].symbol)
                    })
            }) {
                let reason = ranking_failures
                    .iter()
                    .find(|(failed, _)| failed == &members)
                    .map(|(_, reason)| *reason)
                    .unwrap_or("the exact call inventory is incomplete");
                diagnostics.push(Diagnostic::error(format!(
                    "proof-only machine call cycle: `{names}` -- no complete structural ranking \
                     establishes strict descent on every cycle; exact arguments must preserve \
                     or descend from the ranking subject, and preserving edges must be acyclic; \
                     computed ranking: {reason}"
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
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
    adjacency: &[Vec<usize>],
    proof_dependencies: &[Vec<usize>],
    failures: &mut Vec<(Vec<usize>, &'static str)>,
) -> Vec<ValidatedProofCallComponent> {
    let machines = program.machines();
    proof_components::complete(program, proof_only, adjacency, proof_dependencies)
        .into_iter()
        .filter_map(|component| {
            let structural =
                proof_ranking::validate_component(program, &component.members, &component.edges);
            let intervals = if structural.is_some() {
                Vec::new()
            } else {
                match proof_intervals::validate_component(
                    program,
                    &component.members,
                    &component.edges,
                ) {
                    Ok(intervals) => intervals,
                    Err(reason) => {
                        failures.push((component.members, reason));
                        return None;
                    }
                }
            };
            Some(ValidatedProofCallComponent {
                members: component
                    .members
                    .iter()
                    .map(|member| machines[*member].symbol)
                    .collect(),
                structural,
                intervals,
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
            if let Some(callee) = runtime_ranking::target_machine(program, target)
                && callee != caller
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
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
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
                caller,
                &mut expression_ordinal,
                edges,
            )
        };
    }
    match statement {
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
        StatementNode::Call(call) => {
            if let Some(callee) = runtime_ranking::target_machine(program, call.target_symbol) {
                edges.push(ExactProofCallEdge {
                    caller,
                    callee,
                    site: ValidatedProofRecursiveCallSite::Statement {
                        state,
                        statement_index,
                    },
                    arguments: program
                        .statement_table
                        .expression_handles(call.arguments)
                        .to_vec(),
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
                        if let Some(callee) = runtime_ranking::target_machine(program, path.symbol)
                        {
                            edges.push(ExactProofCallEdge {
                                caller,
                                callee,
                                site: ValidatedProofRecursiveCallSite::Transition {
                                    state,
                                    statement_index,
                                    lane,
                                },
                                arguments: program
                                    .statement_table
                                    .expression_handles(*arguments)
                                    .to_vec(),
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
                caller,
                next_expression_ordinal,
                edges,
            )
        };
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            for child in
                crate::value_custody::atomic_operations::atomic_execution_children(program, atomic)
            {
                recurse!(child);
            }
        }
        ExpressionNode::Call(call) => {
            if let Some(callee) = runtime_ranking::target_machine(program, call.target_symbol) {
                edges.push(ExactProofCallEdge {
                    caller,
                    callee,
                    site: ValidatedProofRecursiveCallSite::Expression {
                        state,
                        statement_index,
                        expression_ordinal,
                    },
                    arguments: program
                        .expression_table
                        .expression_handles(call.arguments)
                        .to_vec(),
                });
            }
            recurse!(call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse!(*argument);
            }
        }
        other => crate::expression_traversal::children(program, other, |child| {
            recurse!(child);
        }),
    }
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

fn expression_member_path(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    root_symbol: SymbolHandle,
    root_type: TypeReferenceHandle,
) -> Option<(Vec<SymbolHandle>, TypeReferenceHandle)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let (mut path, receiver_type) =
                expression_member_path(program, member.receiver, root_symbol, root_type)?;
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
        ExpressionNode::Name(path) => {
            (root_symbol.is_valid() && path.symbol == root_symbol).then(|| (Vec::new(), root_type))
        }
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
