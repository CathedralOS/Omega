//! Exact machine dependency extraction and conservative interface retention.

use std::collections::{HashMap, HashSet};

use checked_trees::CheckFacts;
use symbols::SymbolHandle;
use typed_trees::{expression::StaticMachineArgument, statement::StatementHandle};

/// Exact declaration indices used to resolve machine references and to key
/// machine-owner lookups without scanning the declaration table.
pub(super) struct MachineIndex {
    /// Membership of every declared machine symbol; order comes from the program.
    pub(super) machine_symbols: HashSet<SymbolHandle>,
    /// Owning machine for every machine state symbol.
    pub(super) state_machine: HashMap<SymbolHandle, SymbolHandle>,
    /// Owning machine for every statement handle inside a machine state.
    pub(super) statement_machine: HashMap<StatementHandle, SymbolHandle>,
}

impl MachineIndex {
    pub(super) fn build(program: &dyn ::validation::CheckingInput) -> Self {
        let mut machine_symbols = HashSet::new();
        let mut state_machine = HashMap::new();
        let mut statement_machine = HashMap::new();
        for machine in program.machines() {
            machine_symbols.insert(machine.symbol);
            for state in program.machine_states(machine) {
                state_machine.insert(state.symbol, machine.symbol);
                for node in state.statement_nodes.handles() {
                    statement_machine.insert(node, machine.symbol);
                }
            }
        }
        Self {
            machine_symbols,
            state_machine,
            statement_machine,
        }
    }

    /// Resolve any symbol that names a machine or a state owned by a machine
    /// to the machine's declaration symbol. Anything else (data, traits,
    /// parameters, intrinsics, evidence terms) is not a machine edge.
    pub(super) fn machine_of(&self, symbol: SymbolHandle) -> Option<SymbolHandle> {
        if !symbol.is_valid() {
            return None;
        }
        if self.machine_symbols.contains(&symbol) {
            return Some(symbol);
        }
        self.state_machine.get(&symbol).copied()
    }
}

/// Machines that are always retained independent of reachability: every
/// non-`CheckedBody` supply mode plus the provider/authority surface whose
/// machine references no root can bound.
pub(super) fn interface_retention_set(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    index: &MachineIndex,
) -> HashSet<SymbolHandle> {
    let mut retained = HashSet::new();

    // Requirement, boundary, admission-claim, and external-realization
    // machines are product interface surface consumed by Omega provider
    // selection and admission; they are never pruning candidates.
    for machine in program.machines() {
        if !machine.supply_mode.is_checked_body() {
            retained.insert(machine.symbol);
        }
    }

    // Closed conformances name the exact realization machine Omega may
    // select for any retained dispatch surface. Because provider selection
    // happens outside Psi, every realization named by a retained conformance
    // row is interface surface.
    for conformance in program.conformances() {
        for row in &conformance.rows {
            retain_symbol(&mut retained, index, row.realization_machine);
            retain_symbol(&mut retained, index, row.realization_state);
        }
    }

    // Selected nominal machine uses (registration/callback selections) are
    // Omega-bound callables; the selected machine is always product surface.
    for nominal_use in &facts.nominal_machine_uses.uses {
        retain_symbol(&mut retained, index, nominal_use.selected_machine);
        retain_symbol(&mut retained, index, nominal_use.selected_entry);
        retain_symbol(&mut retained, index, nominal_use.registration_operation);
        retain_symbol(&mut retained, index, nominal_use.satisfaction_requirement);
        retain_symbol(&mut retained, index, nominal_use.satisfaction_trait);
    }

    // Contract evidence calls may be owned by trait/operator machinery whose
    // contract is always retained, so every evidence target is retained.
    for call in &facts.proof.contract_expression_evidence_calls {
        retained.insert(call.target_machine_symbol);
        retain_symbol(&mut retained, index, call.target_state_symbol);
    }
    for application in &facts
        .proof
        .contract_expression_static_conformance_applications
    {
        retain_machine_argument(program, index, &mut retained, &application.application);
    }

    // Denotational call projections do not carry an owner key; their targets
    // are conservative interface surface.
    for projection in &facts.fact_call_projections {
        retained.insert(projection.target_machine);
        retain_symbol(&mut retained, index, projection.target_state);
        for argument in projection.machine_arguments.iter() {
            retain_static_machine_argument(program, &mut retained, index, argument);
        }
    }

    // Placed-view plans retain the exact `Policy::plan` machine whose
    // evaluated output produced the placement.
    for plan in &program.placed_view_plans {
        retain_symbol(&mut retained, index, plan.policy_plan_machine_symbol);
    }
    for (_, plan) in program.placement_lifecycle_plans.iter() {
        if let typed_trees::typed_trees::PlacementLifecyclePlan::Evaluated {
            policy_machine, ..
        } = plan
        {
            retain_symbol(&mut retained, index, *policy_machine);
        }
    }
    for input in &facts.placed_view_inputs {
        retain_symbol(&mut retained, index, input.policy_plan_machine);
    }

    // Open-index selections retain their exact provider and operator.
    for operation in program.expression_table.selected_index_operations() {
        retain_symbol(&mut retained, index, operation.provider);
        retain_symbol(&mut retained, index, operation.operator);
    }

    // Every machine referenced by a retained non-machine declaration stays
    // reachable: declarations other than machines are not pruned, so any
    // machine they name would otherwise become a stale reference.
    retain_declaration_surface_machines(program, index, &mut retained);

    retained.retain(|symbol| index.machine_symbols.contains(symbol));
    retained
}

/// Extract exact machine edges from typed references and checked facts.
pub(super) fn collect_machine_edges(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    index: &MachineIndex,
) -> HashMap<SymbolHandle, Vec<SymbolHandle>> {
    let mut edges: HashMap<SymbolHandle, Vec<SymbolHandle>> = HashMap::new();

    // Authoritative checked call facts: every call the semantic traversal
    // captured, keyed by the owning machine.
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            push_edge(
                &mut edges,
                state.machine_symbol,
                index.machine_of(call.target_symbol),
            );
        }
    }

    for machine in program.machines() {
        retain_machine_references(
            program,
            index,
            edges.entry(machine.symbol).or_default(),
            machine,
        );
    }

    // Machine-specialization arguments bind a retained instance to the exact
    // machines it may invoke through its static telescope.
    for specialization in &program.machine_specializations {
        for argument in &specialization.machine_arguments {
            push_edge(
                &mut edges,
                specialization.instance,
                index.machine_of(*argument),
            );
        }
        for application in &specialization.conformance_applications {
            for argument in application.machine_arguments.iter() {
                push_edge(
                    &mut edges,
                    specialization.instance,
                    index.machine_of(*argument),
                );
            }
            for row in &application.rows {
                push_edge(
                    &mut edges,
                    specialization.instance,
                    index.machine_of(row.realization_machine),
                );
            }
        }
    }

    // Contract evidence calls owned by a machine are call edges into the
    // exact evidence target.
    for call in &facts.proof.contract_expression_evidence_calls {
        if let Some(machine_symbol) = contract_caller(&facts.proof, call.owner) {
            push_edge(
                &mut edges,
                machine_symbol,
                index.machine_of(call.target_machine_symbol),
            );
        }
    }
    for application in &facts
        .proof
        .contract_expression_static_conformance_applications
    {
        if let Some(machine_symbol) = contract_caller(&facts.proof, application.owner) {
            for argument in application.application.machine_arguments.iter() {
                push_edge(&mut edges, machine_symbol, index.machine_of(*argument));
            }
            for row in &application.application.rows {
                push_edge(
                    &mut edges,
                    machine_symbol,
                    index.machine_of(row.realization_machine),
                );
            }
        }
    }

    // Proof-position recursive components retain their exact internal call
    // edges even where the ordinary call traversal does not observe them.
    for component in &facts.termination.proof_recursive_components {
        for edge in &component.edges {
            push_edge(&mut edges, edge.caller, index.machine_of(edge.callee));
        }
    }

    // Suspension-crossing rows retain the exact call target they may suspend
    // across.
    for crossing in &facts.carry.suspension_crossings {
        push_edge(
            &mut edges,
            crossing.machine,
            index.machine_of(crossing.target),
        );
    }

    // Checked semantic dependencies include `AutomaticCleanupMachine` rows
    // (consumer -> cleanup machine it implicitly invokes) and any
    // machine-valued nominal/layout dependency.
    for row in facts.flow.semantic_dependencies.iter() {
        push_edge(
            &mut edges,
            row.consumer_machine,
            index.machine_of(row.dependency),
        );
    }

    // Traversal writes directly into each owner's final dependency storage.
    // Normalize once: repeated expression, proof, and checked-fact edges have
    // the same reachability meaning, and traversal order is not product order.
    for targets in edges.values_mut() {
        targets.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        targets.dedup();
    }
    edges
}

/// Append one resolved machine edge; unresolved symbols are non-machine
/// identities and contribute nothing.
fn push_edge(
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    to: Option<SymbolHandle>,
) {
    if let Some(to) = to {
        edges.entry(from).or_default().push(to);
    }
}

struct MachineReferences<'index, 'output, Output> {
    index: &'index MachineIndex,
    output: &'output mut Output,
}
impl<Output: Extend<SymbolHandle>> Extend<SymbolHandle> for MachineReferences<'_, '_, Output> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        self.output.extend(
            symbols
                .into_iter()
                .filter_map(|symbol| self.index.machine_of(symbol)),
        );
    }
}
impl<Output: Extend<SymbolHandle>> typed_trees::references::TypedReferences
    for MachineReferences<'_, '_, Output>
{
}

fn retain_declaration_surface_machines(
    program: &dyn ::validation::CheckingInput,
    index: &MachineIndex,
    output: &mut impl Extend<SymbolHandle>,
) {
    typed_trees::references::visit_declarations(program, &mut MachineReferences { index, output });
}
fn retain_machine_references(
    program: &dyn ::validation::CheckingInput,
    index: &MachineIndex,
    output: &mut impl Extend<SymbolHandle>,
    machine: &typed_trees::machine::Machine,
) {
    typed_trees::references::visit_machine(
        program,
        &mut MachineReferences { index, output },
        machine,
    );
}
fn retain_static_machine_argument(
    program: &dyn ::validation::CheckingInput,
    output: &mut impl Extend<SymbolHandle>,
    index: &MachineIndex,
    argument: &StaticMachineArgument,
) {
    typed_trees::references::visit_static_argument(
        program,
        &mut MachineReferences { index, output },
        argument,
    );
}
fn retain_machine_argument(
    program: &dyn ::validation::CheckingInput,
    index: &MachineIndex,
    output: &mut impl Extend<SymbolHandle>,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
) {
    typed_trees::references::visit_closed_conformance(
        program,
        &mut MachineReferences { index, output },
        application,
    );
}

fn retain_symbol(
    output: &mut impl Extend<SymbolHandle>,
    index: &MachineIndex,
    symbol: SymbolHandle,
) {
    if let Some(machine) = index.machine_of(symbol) {
        output.extend([machine]);
    }
}

fn contract_caller(
    proof: &checked_trees::ProofFacts,
    owner: checked_trees::ContractProofFactOwner,
) -> Option<SymbolHandle> {
    match owner {
        checked_trees::ContractProofFactOwner::Machine { machine_symbol }
        | checked_trees::ContractProofFactOwner::MachineState { machine_symbol, .. } => {
            Some(machine_symbol)
        }
        checked_trees::ContractProofFactOwner::CallSignature { application, .. } => {
            (crate::proof::contracts::call_signatures::owner(
                &proof.call_contract_signatures,
                application,
            ) == Some(owner))
            .then(|| {
                proof
                    .call_contract_signatures
                    .get(application)
                    .caller_machine_symbol
            })
        }
        _ => None,
    }
}
