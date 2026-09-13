//! Checked-trees optimization phase: exact opt-in whole-product root
//! selection and unreachable machine-declaration pruning.
//!
//! # Phase contract
//!
//! This module is the Psi-side execution seam for the checked-tree
//! optimization phase (`OptimizationExecutionPhase::CheckedTrees`). It runs
//! strictly AFTER authored checking: the input is a complete, diagnostic-
//! cleared [`CheckedTrees`], so pruning can never hide an invalid authored
//! declaration -- checking, diagnostics, and all fact construction have
//! already committed before this phase sees the program.
//!
//! The opt-in plan is [`CheckedTreeProductPruning`]: an exact, explicit set of
//! product root machines supplied by the coordinator. Nothing is enabled by
//! default, by an optimization level, or by inference; an absent plan is an
//! identity execution with no second pipeline route. Each execution returns a
//! [`CheckedTreeProductSelection`] evidence record carrying the exact roots,
//! the retained/pruned machine rosters in declaration order, and a
//! domain-separated SHA-256 identity, so every target/root-selected Psi
//! product is independently reproducible and joins to its source selection.
//!
//! # Retention bound
//!
//! Only `MachineSupplyMode::CheckedBody` machines are pruning candidates.
//! Requirement slots, top-level requirements, boundary declarations,
//! admission claims, and external realizations are product interface surface:
//! Omega-side provider selection, callback admission, and trust reporting
//! still consume them, so they are always retained.
//!
//! Retention is the transitive closure of the product roots over every
//! machine edge the checked representation enumerates:
//!
//! - checked call facts (`facts.flow.control`) and the typed statement/expression
//!   call nodes they join to, including `machine_arguments`,
//!   `static_requirement_dispatch`, quotient-theorem, and private-layout
//!   applications;
//! - authored `invokes` service targets and `reaches` rows;
//! - `satisfies` `via` expressions, conformance-bound selections, owned-data
//!   initializers, signature/contract proof facts, and const-expression type
//!   positions reachable from each machine;
//! - contract evidence calls, static conformance applications, proof
//!   recursive-component edges, suspension-crossing targets, carry-topology
//!   contained machine targets, specialization machine arguments, and
//!   `AutomaticCleanupMachine`/machine-valued semantic dependencies;
//! - conservative whole-surface retention for provider-selected identities a
//!   root cannot bound: closed-conformance realization machines, boundary
//!   calling plans, nominal machine-use selections, fact-call-projection
//!   targets, boundary adapter realizations, placed-view policy machines,
//!   open-index providers, fused service erasures, and every machine
//!   referenced by a retained non-machine declaration (data, trait,
//!   conformance, proposition, measure, const, operator, domain, wire).
//!
//! # What pruning changes
//!
//! Pruning rebuilds the machine declaration arena and every machine-keyed
//! `Vec` table in the typed sidecars and `CheckFacts`/`FlowFacts` terminal
//! plan families, plus the machine-keyed roster arenas
//! (`flow.control.states`, `service_reaches.machines`,
//! `carry.machine_topologies`) that no `Handle`/`HandleSpan` references into.
//! Arena-resident rows keyed by non-machine identities (expressions, state
//! fact handles, conformances, evidence terms, span-referenced fact rows)
//! remain as retained dead evidence: they are valid checked rows that are
//! unreachable through the pruned declaration surface, and compacting them
//! would remap handles owned by representations outside this crate.
//!
//! After rewriting, the phase independently re-extracts the edge relation on
//! the pruned product and fails closed if any retained machine still
//! references a pruned one.

use std::collections::{HashMap, HashSet};

use checked_trees::{CheckFacts, CheckedTrees};
use diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    domain::ProofFact,
    expression::{
        ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
        TableMatchArm, TableNamePath,
    },
    machine::{Machine, TraitConformance},
    proposition::{PropositionApplication, PropositionBinderArgumentKind, PropositionFormula},
    signature::{AuthoredInvocationTarget, SignatureContract},
    statement::{StatementHandle, StatementNode, TransitionGuardNode, TransitionTargetNode},
    trait_definition::ConformanceImplementation,
    types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
};

const PRODUCT_SELECTION_IDENTITY_DOMAIN: &[u8] = b"omega.psi.checked-tree-product-selection.v1\0";

/// Exact product roots for one checked-tree product selection. Roots are
/// machine symbols chosen by the coordinator (entry/product surface); the
/// phase validates each against the checked program and never invents or
/// infers an additional root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTreeProductRoots {
    machines: Vec<SymbolHandle>,
}

impl CheckedTreeProductRoots {
    /// Bind an exact root set. Duplicates are rejected rather than
    /// canonicalized so the selection identity never differs from the
    /// coordinator's input.
    pub fn new(
        machines: impl IntoIterator<Item = SymbolHandle>,
    ) -> Result<Self, CheckedTreeProductRootsError> {
        let mut machines: Vec<SymbolHandle> = machines.into_iter().collect();
        machines.sort_unstable_by_key(|symbol| symbol.arena_index());
        if let Some(duplicate) = machines
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0])
        {
            return Err(CheckedTreeProductRootsError::DuplicateRoot(duplicate));
        }
        Ok(Self { machines })
    }

    pub fn machines(&self) -> &[SymbolHandle] {
        &self.machines
    }
}

/// A product root selection that cannot be a canonical input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedTreeProductRootsError {
    DuplicateRoot(SymbolHandle),
}

/// The exact opt-in plan for the checked-tree product-pruning phase.
///
/// This is the phase-local projection a coordinator supplies when the unified
/// build selection names this optimization for the `CheckedTrees` phase. The
/// plan owns no defaults: every retained root is explicit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTreeProductPruning {
    roots: CheckedTreeProductRoots,
}

impl CheckedTreeProductPruning {
    pub fn new(roots: CheckedTreeProductRoots) -> Self {
        Self { roots }
    }

    pub const fn roots(&self) -> &CheckedTreeProductRoots {
        &self.roots
    }
}

/// Domain-separated commitment to one executed product selection. The digest
/// binds the phase identity, the exact roots, and the retained/pruned machine
/// rosters in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedTreeProductSelectionIdentity([u8; 32]);

impl CheckedTreeProductSelectionIdentity {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Identity-bearing evidence for one executed checked-tree product
/// selection. The rosters partition the source program's machine table: the
/// same plan against the same checked program always reproduces the same
/// record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTreeProductSelection {
    roots: CheckedTreeProductRoots,
    retained_machines: Vec<SymbolHandle>,
    pruned_machines: Vec<SymbolHandle>,
    identity: CheckedTreeProductSelectionIdentity,
}

impl CheckedTreeProductSelection {
    pub const fn roots(&self) -> &CheckedTreeProductRoots {
        &self.roots
    }

    /// Retained machine symbols in source declaration order.
    pub fn retained_machines(&self) -> &[SymbolHandle] {
        &self.retained_machines
    }

    /// Pruned machine symbols in source declaration order.
    pub fn pruned_machines(&self) -> &[SymbolHandle] {
        &self.pruned_machines
    }

    pub const fn identity(&self) -> CheckedTreeProductSelectionIdentity {
        self.identity
    }
}

/// The pruned checked product plus its identity-bearing selection evidence.
#[derive(Debug)]
pub struct CheckedTreeProductPruningOutcome {
    pub checked: CheckedTrees,
    pub selection: CheckedTreeProductSelection,
}

/// Exact declaration indices used to resolve machine references and to key
/// machine-owner lookups without scanning the declaration table.
struct MachineIndex {
    /// Every declared machine symbol, in declaration order.
    machine_symbols: HashSet<SymbolHandle>,
    /// Owning machine for every machine state symbol.
    state_machine: HashMap<SymbolHandle, SymbolHandle>,
    /// Owning machine for every statement handle inside a machine state.
    statement_machine: HashMap<StatementHandle, SymbolHandle>,
}

impl MachineIndex {
    fn build(program: &TypedTrees) -> Self {
        let mut machine_symbols = HashSet::new();
        let mut state_machine = HashMap::new();
        let mut statement_machine = HashMap::new();
        for machine in program.machines() {
            machine_symbols.insert(machine.symbol);
            for state in program.machine_states(machine) {
                state_machine.insert(state.symbol, machine.symbol);
                for offset in 0..state.statement_nodes.count() {
                    statement_machine.insert(
                        StatementHandle::from_parts(
                            state.statement_nodes.start().arena_index() + offset,
                            state.statement_nodes.start().generation(),
                        ),
                        machine.symbol,
                    );
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
    fn machine_of(&self, symbol: SymbolHandle) -> Option<SymbolHandle> {
        if !symbol.is_valid() {
            return None;
        }
        if self.machine_symbols.contains(&symbol) {
            return Some(symbol);
        }
        self.state_machine.get(&symbol).copied()
    }
}

/// Run the checked-tree product-pruning phase.
///
/// The input must be a fully checked program: this phase runs only after all
/// authored source has been parsed, resolved, typed, and checked, and it
/// returns diagnostics rather than ever relaxing that contract. A root that
/// does not name a declared machine is rejected before any mutation.
pub fn prune_checked_tree_product(
    mut checked: CheckedTrees,
    plan: &CheckedTreeProductPruning,
) -> Result<CheckedTreeProductPruningOutcome, Vec<Diagnostic>> {
    let index = MachineIndex::build(&checked.typed);

    let mut diagnostics = Vec::new();
    for root in plan.roots().machines() {
        if !index.machine_symbols.contains(root) {
            diagnostics.push(Diagnostic::error(format!(
                "checked-tree product pruning root does not name a declared machine (symbol #{})",
                root.arena_index()
            )));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let edges = collect_machine_edges(&checked.typed, &checked.facts, &index);
    let mut retained = interface_retention_set(&checked.typed, &checked.facts, &index);
    retained.extend(plan.roots().machines().iter().copied());

    // Transitive closure over the checked edge relation. The relation is
    // finite (bounded by the machine table) so iteration always converges.
    let mut frontier: Vec<SymbolHandle> = retained.iter().copied().collect();
    while let Some(machine) = frontier.pop() {
        let Some(targets) = edges.get(&machine) else {
            continue;
        };
        for target in targets {
            if retained.insert(*target) {
                frontier.push(*target);
            }
        }
    }

    let pruned: HashSet<SymbolHandle> = index
        .machine_symbols
        .iter()
        .copied()
        .filter(|symbol| !retained.contains(symbol))
        .collect();

    apply_pruning(&mut checked, &retained, &pruned, &index);

    if let Err(failures) = validate_pruned_product(&checked, &retained, &pruned, plan.roots()) {
        return Err(failures
            .into_iter()
            .map(|failure| {
                Diagnostic::error(format!(
                    "checked-tree product pruning internal validation failed: {failure}"
                ))
            })
            .collect());
    }

    let retained_machines: Vec<SymbolHandle> = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .collect();
    let pruned_machines: Vec<SymbolHandle> = {
        let retained_lookup: HashSet<SymbolHandle> = retained_machines.iter().copied().collect();
        // Declaration order is the canonical roster order; the source order
        // is recovered from the pre-pruning declaration sequence embedded in
        // the edge/retention maps (arena order equals declaration order).
        let mut pruned: Vec<SymbolHandle> = pruned.into_iter().collect();
        pruned.sort_unstable_by_key(|symbol| symbol.arena_index());
        debug_assert!(
            pruned
                .iter()
                .all(|symbol| !retained_lookup.contains(symbol))
        );
        pruned
    };

    let identity = selection_identity(
        plan.roots().machines(),
        &retained_machines,
        &pruned_machines,
    );
    let selection = CheckedTreeProductSelection {
        roots: plan.roots().clone(),
        retained_machines,
        pruned_machines,
        identity,
    };
    Ok(CheckedTreeProductPruningOutcome { checked, selection })
}

/// Machines that are always retained independent of reachability: every
/// non-`CheckedBody` supply mode plus the provider/authority surface whose
/// machine references no root can bound.
fn interface_retention_set(
    program: &TypedTrees,
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
        retain_symbol(&mut retained, index, conformance.carrier_symbol);
        if let ConformanceImplementation::Closed { rows } = &conformance.implementation {
            for row in rows {
                retain_symbol(&mut retained, index, row.realization_machine);
                retain_symbol(&mut retained, index, row.realization_state);
            }
        }
    }

    // Boundary calling plans bind an exact requirement machine to a validated
    // policy; their requirement surface is product interface.
    for plan in &program.boundary_calling_plans {
        retain_symbol(&mut retained, index, plan.requirement_machine);
        retain_symbol(&mut retained, index, plan.boundary_trait);
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
        retain_machine_argument(index, &mut retained, &application.application);
    }

    // Denotational call projections do not carry an owner key; their targets
    // are conservative interface surface.
    for projection in &facts.fact_call_projections {
        retained.insert(projection.target_machine);
        retain_symbol(&mut retained, index, projection.target_state);
        for argument in projection.machine_arguments.iter() {
            retain_static_machine_argument(&mut retained, index, argument);
        }
    }

    // Boundary adapter dispatch selects an exact realization state for each
    // routed boundary requirement.
    for dispatch in &facts.boundary_adapter_dispatch {
        retain_symbol(&mut retained, index, dispatch.realization_state);
        retain_symbol(&mut retained, index, dispatch.requirement);
        retain_symbol(&mut retained, index, dispatch.receiver);
    }

    // Placed-view plans retain the exact `Policy::plan` machine whose
    // evaluated output produced the placement.
    for plan in &program.placed_view_plans {
        retain_symbol(&mut retained, index, plan.policy_plan_machine_symbol);
    }
    for input in &facts.placed_view_inputs {
        retain_symbol(&mut retained, index, input.policy_plan_machine);
    }

    // Compiler-owned service-erasure authorizations and open-index operation
    // selections name exact requirements/providers that must survive pruning.
    for erasure in &program.fused_service_erasures {
        retain_symbol(&mut retained, index, erasure.requirement);
    }
    for normalization in &program.open_index_normalizations {
        for operation in &normalization.operations {
            retain_symbol(&mut retained, index, operation.provider);
            retain_symbol(&mut retained, index, operation.operator);
        }
    }

    // Every machine referenced by a retained non-machine declaration stays
    // reachable: declarations other than machines are not pruned, so any
    // machine they name would otherwise become a stale reference.
    retain_declaration_surface_machines(program, index, &mut retained);

    retained.retain(|symbol| index.machine_symbols.contains(symbol));
    retained
}

/// Retain a symbol when it resolves to a machine declaration.
fn retain_symbol(retained: &mut HashSet<SymbolHandle>, index: &MachineIndex, symbol: SymbolHandle) {
    if let Some(machine) = index.machine_of(symbol) {
        retained.insert(machine);
    }
}

/// Retain every machine referenced by a `StaticMachineArgument`: its selected
/// entry-state symbol plus any nested application arguments.
fn retain_static_machine_argument(
    retained: &mut HashSet<SymbolHandle>,
    index: &MachineIndex,
    argument: &StaticMachineArgument,
) {
    retain_symbol(retained, index, argument.symbol);
    if let Some(application) = &argument.application {
        for nested in application.arguments.iter() {
            retain_static_machine_argument(retained, index, nested);
        }
    }
}

/// Retain every machine named inside one closed conformance application:
/// machine arguments, static argument applications, and row realizations.
fn retain_machine_argument(
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
) {
    for machine in application.machine_arguments.iter() {
        retain_symbol(retained, index, *machine);
    }
    for argument in application.arguments.iter() {
        retain_static_machine_argument(retained, index, argument);
    }
    for row in &application.rows {
        retain_symbol(retained, index, row.realization_machine);
        retain_symbol(retained, index, row.realization_state);
    }
}

/// Machines referenced by retained non-machine declarations: data fields and
/// `where` facts, trait requirement signatures, conformance carriers,
/// proposition binders/bodies, measure bodies, const initializers, operator
/// signatures, domain facts, and wire field types.
fn retain_declaration_surface_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
) {
    for data in program.data_definitions() {
        retain_symbol(retained, index, data.symbol);
        if let Some(instance) = data.generic_instance {
            retain_type_machines(program, index, retained, instance);
        }
        if let Some(quotient) = &data.quotient {
            retain_type_machines(program, index, retained, quotient.carrier);
            retain_symbol(retained, index, quotient.relation_symbol);
        }
        for fact in program.tables.proof_facts.span_or_empty(data.where_facts) {
            retain_proof_fact_machines(program, index, retained, fact);
        }
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field) => {
                    retain_type_machines(program, index, retained, field.type_reference);
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    for field in program.data_payload_fields(variant) {
                        retain_type_machines(program, index, retained, field.type_reference);
                    }
                    for fact in program
                        .tables
                        .proof_facts
                        .span_or_empty(variant.where_facts)
                    {
                        retain_proof_fact_machines(program, index, retained, fact);
                    }
                }
            }
        }
    }

    for definition in program.traits() {
        retain_symbol(retained, index, definition.symbol);
        for bound in &definition.conformance_bounds {
            if let Some(selected) = &bound.selected_conformance {
                retain_static_machine_argument(retained, index, selected);
            }
        }
        for signature in program.trait_machine_signatures(definition) {
            retain_signature_machines(program, index, retained, signature);
        }
    }

    for conformance in program.conformances() {
        retain_symbol(retained, index, conformance.symbol);
        retain_symbol(retained, index, conformance.trait_symbol);
        for argument in program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
        {
            retain_type_machines(program, index, retained, *argument);
        }
    }

    for proposition in program.propositions() {
        retain_symbol(retained, index, proposition.symbol);
        for binder in program.proposition_binders(proposition) {
            retain_symbol(retained, index, binder.symbol);
            if let typed_trees::proposition::PropositionBinderKind::Const { type_reference } =
                binder.kind
            {
                retain_type_machines(program, index, retained, type_reference);
            }
        }
        for parameter in program.proposition_parameters(proposition) {
            retain_type_machines(program, index, retained, parameter.type_reference);
        }
        match &proposition.body {
            typed_trees::proposition::PropositionBody::Witness { evidence } => {
                retain_type_machines(program, index, retained, *evidence);
            }
            typed_trees::proposition::PropositionBody::Transparent { proposition } => {
                retain_proposition_formula_machines(program, index, retained, proposition);
            }
            typed_trees::proposition::PropositionBody::Primitive => {}
        }
    }

    for measure in program.measures() {
        retain_symbol(retained, index, measure.symbol);
        if let Some(parameter) = &measure.parameter {
            retain_type_machines(program, index, retained, parameter.type_reference);
        }
        retain_type_machines(program, index, retained, measure.return_type);
        for expression in program.expression_table.expression_handles(measure.body) {
            retain_expression_machines(program, index, retained, *expression);
        }
    }

    for declaration in program.const_declarations() {
        retain_symbol(retained, index, declaration.symbol);
        retain_type_machines(program, index, retained, declaration.declared_type);
        retain_expression_machines(program, index, retained, declaration.authored_initializer);
        retain_expression_machines(
            program,
            index,
            retained,
            declaration.materialized_initializer,
        );
    }

    for operator in program.operators() {
        retain_symbol(retained, index, operator.symbol);
        for parameter in program
            .tables
            .state_parameters
            .span_or_empty(operator.parameters)
        {
            retain_type_machines(program, index, retained, parameter.type_reference);
        }
        retain_type_machines(program, index, retained, operator.return_type);
        for contract in program
            .tables
            .signature_contracts
            .span_or_empty(operator.contracts)
        {
            retain_contract_machines(program, index, retained, contract);
        }
    }

    for domain in program.domain_definitions() {
        retain_symbol(retained, index, domain.symbol);
        retain_type_machines(program, index, retained, domain.target_type);
        for argument in &domain.index_arguments {
            retain_type_machines(program, index, retained, *argument);
        }
        for fact in program.proof_facts(domain) {
            retain_proof_fact_machines(program, index, retained, fact);
        }
    }

    for schema in program.wire_schemas() {
        retain_symbol(retained, index, schema.symbol);
        for member in program.tables.wire_members.span_or_empty(schema.members) {
            if let typed_trees::wire::WireMember::Field(field) = member {
                retain_type_machines(program, index, retained, field.type_reference);
            }
        }
    }

    // Exact typed ranking-witness expressions are compiler custody for
    // termination evidence; they are machine-keyed rows so retention here is
    // conservative (reachable machines also produce their own edges).
    for custody in &program.ranking_expression_custody {
        for subject in &custody.subjects {
            retain_expression_machines(program, index, retained, *subject);
        }
        for argument in &custody.view_arguments {
            retain_expression_machines(program, index, retained, *argument);
        }
        if let Some(rank_range) = custody.rank_range {
            retain_expression_machines(program, index, retained, rank_range);
        }
    }
}

/// Retain machines referenced by one proof fact: embedded expressions,
/// membership values and domain arguments, or proposition applications.
fn retain_proof_fact_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    fact: &ProofFact,
) {
    match fact {
        ProofFact::Expression(expression) => {
            retain_expression_machines(program, index, retained, *expression);
        }
        ProofFact::Membership(membership) => {
            retain_expression_machines(program, index, retained, membership.value);
            retain_symbol(retained, index, membership.domain_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(membership.domain_arguments)
            {
                retain_type_machines(program, index, retained, *argument);
            }
        }
        ProofFact::Proposition(application) => {
            retain_proposition_application_machines(program, index, retained, application);
        }
    }
}

/// Retain machines named by one proposition application: machine-kind binder
/// arguments plus every expression argument.
fn retain_proposition_application_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    application: &PropositionApplication,
) {
    retain_symbol(retained, index, application.proposition);
    for binder in application.binder_arguments.iter() {
        if binder.kind == PropositionBinderArgumentKind::Machine {
            retain_symbol(retained, index, binder.symbol);
        }
    }
    for argument in program
        .expression_table
        .expression_handles(application.arguments)
    {
        retain_expression_machines(program, index, retained, *argument);
    }
}

/// Retain machines named by one proposition formula body.
fn retain_proposition_formula_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    formula: &PropositionFormula,
) {
    match formula {
        PropositionFormula::Application(application) => {
            retain_proposition_application_machines(program, index, retained, application);
        }
        PropositionFormula::BooleanExpression(expression) => {
            retain_expression_machines(program, index, retained, *expression);
        }
    }
}

/// Retain machines referenced by one state signature or operator signature:
/// parameter types, return type, invokes targets, and contract facts.
fn retain_signature_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    signature: &typed_trees::signature::StateSignature,
) {
    for parameter in program
        .tables
        .state_parameters
        .span_or_empty(signature.parameters)
    {
        retain_type_machines(program, index, retained, parameter.type_reference);
    }
    retain_type_machines(program, index, retained, signature.return_type);
    for invocation in program
        .tables
        .signature_invokes
        .span_or_empty(signature.invokes)
    {
        if let AuthoredInvocationTarget::Service(target) = invocation.target {
            retain_symbol(retained, index, target);
        }
    }
    for contract in program
        .tables
        .signature_contracts
        .span_or_empty(signature.contracts)
    {
        retain_contract_machines(program, index, retained, contract);
    }
}

/// Retain machines referenced by one signature contract's proof facts.
fn retain_contract_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    contract: &SignatureContract,
) {
    for fact in program.tables.proof_facts.span_or_empty(contract.facts) {
        retain_proof_fact_machines(program, index, retained, fact);
    }
}

/// Retain every machine symbol appearing in one type reference, including
/// const-expression index payloads that can call machines.
fn retain_type_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    type_reference: TypeReferenceHandle,
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            retain_type_machines(program, index, retained, *referee);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            retain_type_machines(program, index, retained, *base_type);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    typed_trees::types::TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } => {
                        retain_expression_machines(program, index, retained, *minimum);
                        retain_expression_machines(program, index, retained, *maximum);
                    }
                    typed_trees::types::TypeConstraintNode::Domain(domain) => {
                        retain_symbol(retained, index, domain.symbol);
                        for argument in &domain.arguments {
                            retain_type_machines(program, index, retained, *argument);
                        }
                    }
                    typed_trees::types::TypeConstraintNode::Named(_)
                    | typed_trees::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            retain_type_machines(program, index, retained, *element_type);
            match length {
                FixedArrayLength::ConstParameter { symbol, .. } => {
                    retain_symbol(retained, index, *symbol);
                }
                FixedArrayLength::Literal(_) | FixedArrayLength::ConstCall { .. } => {}
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            retain_type_machines(program, index, retained, *element_type);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            retain_symbol(retained, index, *base_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                retain_type_machines(program, index, retained, *argument);
            }
        }
        TypeReferenceNode::ConstExpression(expression) => {
            retain_expression_machines(program, index, retained, *expression);
        }
        TypeReferenceNode::DynamicTrait {
            symbol,
            conformance,
            ..
        } => {
            retain_symbol(retained, index, *symbol);
            if let Some(conformance) = conformance {
                retain_symbol(retained, index, *conformance);
            }
        }
        TypeReferenceNode::Named { symbol, .. } => {
            retain_symbol(retained, index, *symbol);
        }
        TypeReferenceNode::Unit => {}
    }
}

/// Retain every machine referenced by one typed expression node, recursively.
fn retain_expression_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    expression: ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(match_expression) => {
            retain_expression_machines(program, index, retained, match_expression.subject);
            for arm in program.expression_table.match_arms(match_expression.arms) {
                retain_match_arm_machines(program, index, retained, arm);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                retain_expression_machines(program, index, retained, *element);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            retain_expression_machines(program, index, retained, atomic.value);
            retain_expression_machines(program, index, retained, atomic.result);
        }
        ExpressionNode::Binary(binary) => {
            retain_expression_machines(program, index, retained, binary.left);
            retain_expression_machines(program, index, retained, binary.right);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
        ExpressionNode::Cast(cast) => {
            retain_expression_machines(program, index, retained, cast.value);
            retain_type_machines(program, index, retained, cast.target_type);
            retain_type_machines(program, index, retained, cast.result_type);
            retain_symbol(retained, index, cast.semantic_domain_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(cast.semantic_domain_arguments)
            {
                retain_type_machines(program, index, retained, *argument);
            }
        }
        ExpressionNode::Call(call) => {
            retain_call_expression_machines(program, index, retained, call);
        }
        ExpressionNode::Indexed(indexed) => {
            retain_expression_machines(program, index, retained, indexed.collection);
            retain_expression_machines(program, index, retained, indexed.index);
        }
        ExpressionNode::Member(member) => {
            retain_expression_machines(program, index, retained, member.receiver);
            retain_symbol(retained, index, member.member_symbol);
        }
        ExpressionNode::Borrow(borrow) => {
            retain_expression_machines(program, index, retained, borrow.target);
        }
        ExpressionNode::Name(path) => {
            retain_name_path_machines(program, index, retained, path);
        }
        ExpressionNode::Range(range) => {
            retain_expression_machines(program, index, retained, range.start);
            retain_expression_machines(program, index, retained, range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            retain_symbol(retained, index, literal.type_symbol);
            if let Some(case_symbol) = literal.case_symbol {
                retain_symbol(retained, index, case_symbol);
            }
            for field in program.expression_table.struct_fields(literal.fields) {
                retain_expression_machines(program, index, retained, field.value);
            }
        }
        ExpressionNode::Unary(unary) => {
            retain_expression_machines(program, index, retained, unary.operand);
        }
        ExpressionNode::ZeroValue(type_reference) => {
            retain_type_machines(program, index, retained, *type_reference);
        }
    }
}

/// Retain machines referenced by one match arm pattern and value.
fn retain_match_arm_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    arm: &TableMatchArm,
) {
    if let typed_trees::expression::MatchPattern::Value(pattern) = &arm.pattern {
        retain_expression_machines(program, index, retained, *pattern);
    }
    retain_expression_machines(program, index, retained, arm.value);
}

/// Retain machines named by a resolved name path (head, member, and final
/// symbols can all resolve to machines).
fn retain_name_path_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    path: &TableNamePath,
) {
    retain_symbol(retained, index, path.head_symbol);
    retain_symbol(retained, index, path.symbol);
    for member in program
        .expression_table
        .name_path_member_symbols(path.member_symbols)
    {
        retain_symbol(retained, index, *member);
    }
}

/// Retain the call target and every static argument of one typed call
/// expression, including static requirement dispatch and sealed-operation
/// applications.
fn retain_call_expression_machines(
    program: &TypedTrees,
    index: &MachineIndex,
    retained: &mut HashSet<SymbolHandle>,
    call: &TableCallExpression,
) {
    retain_symbol(retained, index, call.target_symbol);
    retain_symbol(retained, index, call.static_machine_parameter);
    retain_expression_machines(program, index, retained, call.receiver);
    if let Some(dispatch) = &call.static_requirement_dispatch {
        retain_symbol(retained, index, dispatch.realization_machine);
        retain_symbol(retained, index, dispatch.realization_state);
        retain_symbol(retained, index, dispatch.requirement);
        retain_symbol(retained, index, dispatch.declaring_trait);
    }
    for argument in call.machine_arguments.iter() {
        retain_static_machine_argument(retained, index, argument);
    }
    if let Some(quotient) = &call.quotient_operation {
        retain_static_machine_argument(retained, index, &quotient.representative_operation);
        for theorem in quotient.theorem_evidence.iter() {
            retain_static_machine_argument(retained, index, &theorem.application);
        }
    }
    if let Some(layout) = &call.private_layout_operation {
        retain_static_machine_argument(retained, index, &layout.selected_slot);
    }
    for argument in program.expression_table.expression_handles(call.arguments) {
        retain_expression_machines(program, index, retained, *argument);
    }
}

/// The checked machine edge relation: `source -> machines it can reach`.
/// Every entry is an exact symbol identity captured from the checked
/// representation; no edge is derived from names or spellings.
fn collect_machine_edges(
    program: &TypedTrees,
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
        let from = machine.symbol;

        for invocation in program.machine_invokes(machine) {
            if let AuthoredInvocationTarget::Service(target) = invocation.target {
                push_edge(&mut edges, from, index.machine_of(target));
            }
        }

        for row in program
            .authored_service_reach_rows
            .iter()
            .filter(|row| row.owner == from)
        {
            for target in &row.targets {
                push_edge(&mut edges, from, index.machine_of(target.service));
            }
        }

        for conformance in program.machine_trait_conformances(machine) {
            push_edge(
                &mut edges,
                from,
                index.machine_of(conformance.requirement_symbol),
            );
            retain_expression_machines_into(program, index, &mut edges, from, conformance);
        }

        for owned in program.machine_owned_data(machine) {
            collect_type_edges(program, index, &mut edges, from, owned.type_reference);
            collect_expression_edges(program, index, &mut edges, from, owned.initial_value);
        }

        for bound in &machine.conformance_bounds {
            if let Some(selected) = &bound.selected_conformance {
                collect_static_argument_edges(index, &mut edges, from, selected);
            }
        }

        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                collect_type_edges(program, index, &mut edges, from, parameter.type_reference);
            }
            collect_type_edges(program, index, &mut edges, from, state.return_type);
            for contract in program.state_contracts(state) {
                for fact in program.tables.proof_facts.span_or_empty(contract.facts) {
                    collect_proof_fact_edges(program, index, &mut edges, from, fact);
                }
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                collect_statement_edges(program, index, &mut edges, from, statement);
            }
        }
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
        if let checked_trees::ContractProofFactOwner::Machine { machine_symbol }
        | checked_trees::ContractProofFactOwner::MachineState { machine_symbol, .. } = call.owner
        {
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
        if let checked_trees::ContractProofFactOwner::Machine { machine_symbol }
        | checked_trees::ContractProofFactOwner::MachineState { machine_symbol, .. } =
            application.owner
        {
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

    // Machines contained in a carried record field are reachable through the
    // carrier's ownership topology.
    for (_, topology) in facts.carry.machine_topologies.iter() {
        for field in facts.carry.contained_fields.span_or_empty(topology.fields) {
            for target in facts.carry.contained_targets.span_or_empty(field.targets) {
                push_edge(
                    &mut edges,
                    topology.machine,
                    index.machine_of(target.machine),
                );
            }
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

    // Dynamic-dispatch plan rows bind each retained caller to its selected
    // realization machine.
    let dispatch = &facts.flow.terminal_unit_effects.dynamic_dispatch;
    for call in &dispatch.direct_scalar_calls {
        push_edge(
            &mut edges,
            call.caller_machine,
            index.machine_of(call.realization_machine),
        );
    }
    for call in &dispatch.direct_unit_calls {
        push_edge(
            &mut edges,
            call.caller_machine,
            index.machine_of(call.realization_machine),
        );
    }
    for call in &dispatch.rebound_scalar_calls {
        push_edge(
            &mut edges,
            call.latest.caller_machine,
            index.machine_of(call.latest.realization_machine),
        );
    }
    for call in &dispatch.rebound_unit_calls {
        push_edge(
            &mut edges,
            call.latest.caller_machine,
            index.machine_of(call.latest.realization_machine),
        );
    }
    for call in &dispatch.stored_scalar_calls {
        push_edge(
            &mut edges,
            call.call.caller_machine,
            index.machine_of(call.call.realization_machine),
        );
    }
    for call in &dispatch.joined_scalar_calls {
        push_edge(
            &mut edges,
            call.caller_machine,
            index.machine_of(call.when_true.call.realization_machine),
        );
        push_edge(
            &mut edges,
            call.caller_machine,
            index.machine_of(call.when_false.call.realization_machine),
        );
    }
    for call in &dispatch.joined_unit_calls {
        push_edge(
            &mut edges,
            call.caller_machine,
            index.machine_of(call.when_true.call.realization_machine),
        );
        push_edge(
            &mut edges,
            call.caller_machine,
            index.machine_of(call.when_false.call.realization_machine),
        );
    }
    for transfer in &dispatch.transfers {
        push_edge(
            &mut edges,
            transfer.caller_machine,
            index.machine_of(transfer.target_machine),
        );
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

/// Retain `via` expressions on machine satisfies rows as call edges.
fn retain_expression_machines_into(
    program: &TypedTrees,
    index: &MachineIndex,
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    conformance: &TraitConformance,
) {
    collect_expression_edges(program, index, edges, from, conformance.via_expression);
    for argument in program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    {
        collect_type_edges(program, index, edges, from, *argument);
    }
}

/// Edges contributed by one statement node inside a retained machine.
fn collect_statement_edges(
    program: &TypedTrees,
    index: &MachineIndex,
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    statement: &StatementNode,
) {
    match statement {
        StatementNode::RootBinding(binding) => {
            collect_expression_edges(program, index, edges, from, binding.receiver);
        }
        StatementNode::AssemblyFact(fact) => {
            collect_expression_edges(program, index, edges, from, fact.expression);
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_edges(program, index, edges, from, assignment.target);
            collect_expression_edges(program, index, edges, from, assignment.value);
        }
        StatementNode::Call(call) => {
            if let Some(target) = index.machine_of(call.target_symbol) {
                edges.entry(from).or_default().push(target);
            }
            if let Some(target) = index.machine_of(call.static_machine_parameter) {
                edges.entry(from).or_default().push(target);
            }
            if let Some(dispatch) = &call.static_requirement_dispatch {
                if let Some(target) = index.machine_of(dispatch.realization_machine) {
                    edges.entry(from).or_default().push(target);
                }
                if let Some(target) = index.machine_of(dispatch.realization_state) {
                    edges.entry(from).or_default().push(target);
                }
            }
            for argument in call.machine_arguments.iter() {
                collect_static_argument_edges(index, edges, from, argument);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_edges(program, index, edges, from, *argument);
            }
        }
        StatementNode::Expression(expression) => {
            collect_expression_edges(program, index, edges, from, *expression);
        }
        StatementNode::LocalData(local) => {
            collect_type_edges(program, index, edges, from, local.type_reference);
            collect_expression_edges(program, index, edges, from, local.initial_value);
        }
        StatementNode::Transition(transition) => {
            for target_handle in [transition.target, transition.continuation] {
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        if let Some(target) = index.machine_of(path.symbol) {
                            edges.entry(from).or_default().push(target);
                        }
                        if let Some(target) = index.machine_of(path.head_symbol) {
                            edges.entry(from).or_default().push(target);
                        }
                        for argument in program.expression_table.expression_handles(*arguments) {
                            collect_expression_edges(program, index, edges, from, *argument);
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        collect_expression_edges(program, index, edges, from, *expression);
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_edges(program, index, edges, from, guard);
            }
        }
    }
}

/// Edges contributed by machines named inside a proof fact.
fn collect_proof_fact_edges(
    program: &TypedTrees,
    index: &MachineIndex,
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    fact: &ProofFact,
) {
    let mut retained = HashSet::new();
    retain_proof_fact_machines(program, index, &mut retained, fact);
    for target in retained {
        edges.entry(from).or_default().push(target);
    }
}

/// Edges contributed by machines named inside a static machine argument.
fn collect_static_argument_edges(
    index: &MachineIndex,
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    argument: &StaticMachineArgument,
) {
    if let Some(target) = index.machine_of(argument.symbol) {
        edges.entry(from).or_default().push(target);
    }
    if let Some(application) = &argument.application {
        for nested in application.arguments.iter() {
            collect_static_argument_edges(index, edges, from, nested);
        }
    }
}

/// Edges contributed by machines named inside a type reference.
fn collect_type_edges(
    program: &TypedTrees,
    index: &MachineIndex,
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    type_reference: TypeReferenceHandle,
) {
    let mut retained = HashSet::new();
    retain_type_machines(program, index, &mut retained, type_reference);
    for target in retained {
        edges.entry(from).or_default().push(target);
    }
}

/// Edges contributed by machines named inside an expression.
fn collect_expression_edges(
    program: &TypedTrees,
    index: &MachineIndex,
    edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
    from: SymbolHandle,
    expression: ExpressionHandle,
) {
    let mut retained = HashSet::new();
    retain_expression_machines(program, index, &mut retained, expression);
    for target in retained {
        edges.entry(from).or_default().push(target);
    }
}

/// Rebuild the machine declaration surface and every machine-keyed `Vec`
/// table so the product's iteration-visible rosters match the selection.
/// Machine-keyed roster arenas with no inbound `Handle`/`HandleSpan`
/// references are rebuilt the same way; arena rows referenced by spans or
/// handles from other retained rows remain as valid dead evidence and are
/// unreachable through the pruned declaration surface.
fn apply_pruning(
    checked: &mut CheckedTrees,
    retained: &HashSet<SymbolHandle>,
    pruned: &HashSet<SymbolHandle>,
    index: &MachineIndex,
) {
    let program = &mut checked.typed;

    // Machine declarations: rebuild the arena so `roots.machines` covers
    // exactly the retained declaration surface in source order.
    let retained_machines: Vec<Machine> = program
        .machines()
        .iter()
        .filter(|machine| retained.contains(&machine.symbol))
        .cloned()
        .collect();
    let mut machines = arena::Arena::default();
    let machine_roots = machines.insert_many(retained_machines);
    program.tables.machines = machines;
    program.roots.machines = machine_roots;

    // Typed sidecars keyed by a machine identity.
    program
        .machine_specializations
        .retain(|row| retained.contains(&row.instance));
    program
        .authored_service_reach_rows
        .retain(|row| retained.contains(&row.owner));
    program
        .evidence_forwardings
        .retain(|row| retained.contains(&row.machine_symbol));
    program
        .proof_output_calls
        .retain(|row| retained.contains(&row.machine_symbol));
    program
        .boundary_calling_plans
        .retain(|row| retained.contains(&row.requirement_machine));
    program
        .ranking_expression_custody
        .retain(|row| retained.contains(&row.machine));

    let facts = &mut checked.facts;

    facts
        .mutation
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .suspensions
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .blocking
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .synchronous_invocations
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .termination
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .termination
        .build_bound_progress
        .retain(|row| retained.contains(&row.machine));
    // A proof recursive component is one SCC: it survives pruning only when
    // every member survives, which the closed edge relation guarantees for
    // any component containing a retained member.
    facts
        .termination
        .proof_recursive_components
        .retain(|component| {
            component
                .members
                .iter()
                .all(|member| retained.contains(&member.machine))
        });
    facts
        .contract_plans
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .contract_plans
        .realized_envelopes
        .retain(|row| retained.contains(&row.machine));
    facts
        .qualifications
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .qualifications
        .vacuous_uses
        .retain(|row| retained.contains(&row.machine));
    facts
        .qualifications
        .content
        .identity_reshuffles
        .retain(|row| retained.contains(&row.machine_symbol));
    facts
        .qualifications
        .content
        .partition_compositions
        .retain(|row| retained.contains(&row.machine_symbol));
    facts
        .dynamic_conformances
        .selections
        .retain(|row| retained.contains(&row.machine));
    facts
        .dynamic_conformances
        .storages
        .retain(|row| retained.contains(&row.machine));
    facts
        .carry
        .suspension_crossings
        .retain(|row| retained.contains(&row.machine));
    facts
        .carry
        .activation_wide_carry
        .retain(|row| retained.contains(&row.machine));
    facts
        .placed_view_inputs
        .retain(|row| retained.contains(&row.machine));
    facts
        .operators
        .symbolic_boundary_applications
        .retain(|row| retained.contains(&row.machine_symbol));
    facts
        .nominal_machine_uses
        .uses
        .retain(|row| match row.site {
            checked_trees::NominalMachineUseSite::Statement(statement) => index
                .statement_machine
                .get(&statement)
                .is_some_and(|owner| retained.contains(owner)),
            checked_trees::NominalMachineUseSite::Expression(_) => true,
        });

    // Contract evidence rows keyed by an owner machine.
    facts
        .proof
        .contract_expression_evidence_calls
        .retain(|row| contract_owner_retained(row.owner, pruned));
    facts
        .proof
        .contract_expression_static_conformance_applications
        .retain(|row| contract_owner_retained(row.owner, pruned));

    // Boundary adapter rows keyed by the selected realization machine.
    facts.boundary_adapter_dispatch.retain(|row| {
        index
            .machine_of(row.realization_state)
            .is_none_or(|machine| retained.contains(&machine))
    });

    // Machine-keyed roster arenas with no inbound handles or spans are
    // rebuilt so iteration sees exactly the retained product. Arenas whose
    // rows are referenced by `Handle`/`HandleSpan` from other retained rows
    // (`borrow.states`, `service_reaches.states`/`calls`,
    // `carry.contained_fields`/`contained_targets`, the flow call/statement
    // arenas, and every proof/evidence arena) stay untouched: their dead
    // rows are unreachable through the pruned declaration surface and
    // compacting them would invalidate live inbound references.
    {
        let kept: Vec<checked_trees::FlowStateFact> = facts
            .flow
            .control
            .states
            .iter()
            .map(|(_, state)| state)
            .filter(|state| retained.contains(&state.machine_symbol))
            .cloned()
            .collect();
        let mut states = arena::Arena::default();
        states.insert_many(kept);
        facts.flow.control.states = states;
    }
    {
        let kept: Vec<checked_trees::MachineServiceReachRows> = facts
            .service_reaches
            .machines
            .iter()
            .map(|(_, row)| row)
            .filter(|row| retained.contains(&row.machine))
            .cloned()
            .collect();
        let mut machines = arena::Arena::default();
        facts.service_reaches.root_machines = machines.insert_many(kept);
        facts.service_reaches.machines = machines;
    }
    {
        let kept: Vec<checked_trees::MachineCarryTopologyFact> = facts
            .carry
            .machine_topologies
            .iter()
            .map(|(_, row)| row)
            .filter(|row| retained.contains(&row.machine))
            .cloned()
            .collect();
        let mut topologies = arena::Arena::default();
        topologies.insert_many(kept);
        facts.carry.machine_topologies = topologies;
    }

    // Whole-product semantic dependencies: drop rows owned by or naming a
    // pruned machine. The table is independently rederivable via
    // `derive_checked_semantic_dependencies`.
    facts.flow.semantic_dependencies.rows.retain(|row| {
        retained.contains(&row.consumer_machine) && !pruned.contains(&row.dependency)
    });

    // Terminal plan families whose machine rosters enumerate the product.
    facts
        .flow
        .terminal_machines
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_debug
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_scalar_graphs
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .retain(|row| {
            index
                .state_machine
                .get(&row.state)
                .is_none_or(|machine| retained.contains(machine))
        });
    facts
        .flow
        .terminal_unit_effects
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines
        .retain(|row| retained.contains(&row.machine.machine));
    facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .retain(|row| retained.contains(&row.machine.machine));
    facts
        .flow
        .terminal_structural_control_cleanups
        .states
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_control_cleanups
        .projected_edges
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_unit_controls
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_scalar_returns
        .selected_operator_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_scalar_returns
        .trait_operator_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_boundary_scalar_returns
        .boundary_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_returns
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_returns
        .payloadless_case_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines
        .retain(|row| retained.contains(&row.machine));

    // Dynamic-dispatch rows keyed by the calling machine.
    let dispatch = &mut facts.flow.terminal_unit_effects.dynamic_dispatch;
    dispatch
        .transfers
        .retain(|row| retained.contains(&row.caller_machine));
    dispatch
        .direct_scalar_calls
        .retain(|row| retained.contains(&row.caller_machine));
    dispatch
        .rebound_scalar_calls
        .retain(|row| retained.contains(&row.latest.caller_machine));
    dispatch
        .joined_scalar_calls
        .retain(|row| retained.contains(&row.caller_machine));
    dispatch
        .stored_scalar_calls
        .retain(|row| retained.contains(&row.call.caller_machine));
    dispatch
        .direct_unit_calls
        .retain(|row| retained.contains(&row.caller_machine));
    dispatch
        .rebound_unit_calls
        .retain(|row| retained.contains(&row.latest.caller_machine));
    dispatch
        .joined_unit_calls
        .retain(|row| retained.contains(&row.caller_machine));
}

/// Whether a contract-fact owner survives pruning: machine owners must be
/// retained; non-machine owners (trait/operator machinery) are always
/// retained.
fn contract_owner_retained(
    owner: checked_trees::ContractProofFactOwner,
    pruned: &HashSet<SymbolHandle>,
) -> bool {
    match owner {
        checked_trees::ContractProofFactOwner::Machine { machine_symbol }
        | checked_trees::ContractProofFactOwner::MachineState { machine_symbol, .. } => {
            !pruned.contains(&machine_symbol)
        }
        _ => true,
    }
}

/// Independent post-pruning validation: re-extract the edge relation on the
/// pruned product and reject any retained machine that still references a
/// pruned declaration. The check reruns the same extraction rather than
/// trusting the pre-pruning analysis.
fn validate_pruned_product(
    checked: &CheckedTrees,
    retained: &HashSet<SymbolHandle>,
    pruned: &HashSet<SymbolHandle>,
    roots: &CheckedTreeProductRoots,
) -> Result<(), Vec<String>> {
    let mut failures = Vec::new();

    let index = MachineIndex::build(&checked.typed);
    if checked.typed.machines().len() != retained.len() {
        failures.push(format!(
            "pruned machine table holds {} machines but the selection retains {}",
            checked.typed.machines().len(),
            retained.len()
        ));
    }
    for machine in checked.typed.machines() {
        if !retained.contains(&machine.symbol) {
            failures.push(format!(
                "pruned machine table contains unretained machine symbol #{}",
                machine.symbol.arena_index()
            ));
        }
    }
    for root in roots.machines() {
        if !index.machine_symbols.contains(root) {
            failures.push(format!(
                "product root symbol #{} is absent from the pruned machine table",
                root.arena_index()
            ));
        }
    }

    // The pruned edge relation must be closed: no retained machine may reach
    // a symbol outside the pruned product's machine table.
    let edges = collect_machine_edges(&checked.typed, &checked.facts, &index);
    for (source, targets) in &edges {
        if pruned.contains(source) {
            failures.push(format!(
                "edge relation still names pruned machine #{} as a source",
                source.arena_index()
            ));
        }
        for target in targets {
            if pruned.contains(target) {
                failures.push(format!(
                    "retained machine #{} still references pruned machine #{}",
                    source.arena_index(),
                    target.arena_index()
                ));
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

/// Domain-separated SHA-256 identity of one executed product selection.
fn selection_identity(
    roots: &[SymbolHandle],
    retained: &[SymbolHandle],
    pruned: &[SymbolHandle],
) -> CheckedTreeProductSelectionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(PRODUCT_SELECTION_IDENTITY_DOMAIN);
    let mut encode = |symbols: &[SymbolHandle]| {
        hasher.update((symbols.len() as u64).to_le_bytes());
        for symbol in symbols {
            hasher.update(symbol.arena_index().to_le_bytes());
            hasher.update(symbol.generation().to_le_bytes());
        }
    };
    encode(roots);
    encode(retained);
    encode(pruned);
    CheckedTreeProductSelectionIdentity(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use tokens_to_syntax_trees::parse_syntax_trees;

    fn checked(source: &str) -> CheckedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        crate::lower_typed_trees(typed).expect("check")
    }

    fn machine_named<'a>(program: &'a TypedTrees, name: &str) -> &'a Machine {
        program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name} must exist"))
    }

    #[test]
    fn duplicate_roots_are_rejected() {
        let root = SymbolHandle::from_arena_index(4);
        let result = CheckedTreeProductRoots::new([root, root]);
        assert_eq!(
            result,
            Err(CheckedTreeProductRootsError::DuplicateRoot(root))
        );
    }

    #[test]
    fn unknown_root_is_rejected_before_pruning() {
        let program = checked(
            r#"
            pub machine main() -> u64 { 7u64 }
            "#,
        );
        let plan = CheckedTreeProductPruning::new(
            CheckedTreeProductRoots::new([SymbolHandle::from_arena_index(999_999)]).expect("roots"),
        );
        let Err(diagnostics) = prune_checked_tree_product(program, &plan) else {
            panic!("an undeclared root must be rejected");
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            format!("{diagnostic:?}").contains("does not name a declared machine")
        }));
    }

    #[test]
    fn unreachable_checked_body_machine_is_pruned() {
        let program = checked(
            r#"
            machine helper() -> u64 { 40u64 }
            machine unused() -> u64 { helper() }
            pub machine main() -> u64 { helper() }
            "#,
        );
        let before = program.typed.machines().len();
        let root = machine_named(&program.typed, "main").symbol;
        let helper = machine_named(&program.typed, "helper").symbol;
        let unused = machine_named(&program.typed, "unused").symbol;

        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");
        let selection = &outcome.selection;

        assert!(before >= 3);
        assert_eq!(
            outcome.checked.typed.machines().len(),
            selection.retained_machines().len()
        );
        assert!(selection.retained_machines().contains(&root));
        assert!(selection.retained_machines().contains(&helper));
        assert!(selection.pruned_machines().contains(&unused));
        assert_eq!(
            outcome.checked.facts.flow.terminal_machines.machines.len(),
            selection.retained_machines().len()
        );
        assert!(
            outcome
                .checked
                .facts
                .flow
                .terminal_machines
                .machines
                .iter()
                .all(|row| selection.retained_machines().contains(&row.machine))
        );
    }

    #[test]
    fn pruning_is_deterministic_for_the_same_plan() {
        let source = r#"
            machine helper() -> u64 { 40u64 }
            machine unused() -> u64 { 2u64 }
            pub machine main() -> u64 { helper() }
        "#;
        let first = checked(source);
        let second = checked(source);
        let root = machine_named(&first.typed, "main").symbol;
        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));

        let first = prune_checked_tree_product(first, &plan).expect("first prune");
        let second = prune_checked_tree_product(second, &plan).expect("second prune");
        assert_eq!(first.selection, second.selection);
        assert_eq!(first.selection.identity(), second.selection.identity());
    }

    #[test]
    fn transitive_machine_dependencies_are_retained() {
        let program = checked(
            r#"
            machine leaf() -> u64 { 3u64 }
            machine middle() -> u64 { leaf() }
            machine detached() -> u64 { 9u64 }
            pub machine main() -> u64 { middle() }
            "#,
        );
        let main = machine_named(&program.typed, "main").symbol;
        let middle = machine_named(&program.typed, "middle").symbol;
        let leaf = machine_named(&program.typed, "leaf").symbol;
        let detached = machine_named(&program.typed, "detached").symbol;

        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([main]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");

        assert!(outcome.selection.retained_machines().contains(&main));
        assert!(outcome.selection.retained_machines().contains(&middle));
        assert!(outcome.selection.retained_machines().contains(&leaf));
        assert!(outcome.selection.pruned_machines().contains(&detached));
        // The rosters partition the source machine table.
        assert_eq!(
            outcome.selection.retained_machines().len() + outcome.selection.pruned_machines().len(),
            4
        );
    }

    #[test]
    fn selection_identity_binds_the_exact_root_set() {
        let program = checked(
            r#"
            machine other() -> u64 { 1u64 }
            pub machine main() -> u64 { other() }
            pub machine second() -> u64 { other() }
            "#,
        );
        let main = machine_named(&program.typed, "main").symbol;
        let second = machine_named(&program.typed, "second").symbol;

        let main_plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([main]).expect("roots"));
        let pair_plan = CheckedTreeProductPruning::new(
            CheckedTreeProductRoots::new([main, second]).expect("roots"),
        );

        let one_root = prune_checked_tree_product(program.clone(), &main_plan).expect("prune");
        let two_roots = prune_checked_tree_product(program, &pair_plan).expect("prune");
        assert_ne!(
            one_root.selection.identity(),
            two_roots.selection.identity()
        );
        assert_eq!(one_root.selection.roots().machines(), &[main]);
        assert_eq!(two_roots.selection.roots().machines(), &[main, second]);
        assert!(two_roots.selection.pruned_machines().is_empty());
    }

    #[test]
    fn invalid_authored_declarations_fail_checking_before_pruning() {
        // `detached` is unreachable from any plausible root, yet its
        // unproven-arithmetic diagnostic must still surface from authored
        // checking. Pruning is unreachable here: the phase input is a
        // `CheckedTrees`, which only exists after all diagnostics clear.
        let tokens = Lexer::new(
            r#"
            machine helper() -> u64 { 40u64 }
            machine detached() -> u64 { helper() + 2u64 }
            pub machine main() -> u64 { helper() }
            "#,
        )
        .tokenize()
        .expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let Err(diagnostics) = crate::lower_typed_trees(typed) else {
            panic!("an invalid authored declaration must fail checking");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| format!("{diagnostic:?}").contains("may overflow"))
        );
    }

    #[test]
    fn boundary_machines_are_interface_surface_not_pruning_candidates() {
        let mut program = checked(
            r#"
            pub machine entry() -> u64 { 9u64 }
            "#,
        );
        // Synthesize a boundary-declaration machine without source support:
        // pruning must keep it regardless of reachability.
        let boundary_symbol = {
            let mut boundary = Machine::default();
            boundary.symbol = program
                .typed
                .machines()
                .iter()
                .map(|machine| machine.symbol.arena_index())
                .max()
                .map(|index| SymbolHandle::from_arena_index(index + 1))
                .expect("a machine symbol must exist");
            boundary.supply_mode = language_semantics::MachineSupplyMode::Boundary;
            boundary.body_is_present = false;
            let symbol = boundary.symbol;
            program.typed.push_machine(boundary);
            symbol
        };
        let root = machine_named(&program.typed, "entry").symbol;
        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");
        assert!(
            outcome
                .selection
                .retained_machines()
                .contains(&boundary_symbol)
        );
    }
}
