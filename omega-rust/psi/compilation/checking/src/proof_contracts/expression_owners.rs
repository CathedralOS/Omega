//! Exact expression owners shared by preparation and checking.
//!
//! The invocation constructs the index before borrowing it. Only the supplied
//! contract arena grants checked ownership; preparation supplies empty rows.
//! No mutable typed input or runtime-borrowed construction crosses this boundary.

mod call_targets;
mod environments;
mod indexing;
mod inference;
mod reachability;
pub use reachability::{expression_contains, reachable_expressions};

pub use call_targets::{
    checked_expression_type_reference_from_exact_owner,
    checked_machine_call_target_from_exact_owner, checked_member_target_from_exact_owner,
};
use checked_trees::ContractProofFact;
use indexing::build_owner_environment_index;
pub use indexing::proof_fact_contains_expression;
use language_semantics::declaration_selection::CollectionMeasure;
/// Handle-keyed indexes and reach sets take the integer hasher.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerMemberTarget {
    Declaration(SymbolHandle),
    CollectionMeasure(CollectionMeasure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InferredType {
    TypeReference(TypeReferenceHandle),
    Nominal(SymbolHandle),
    CollectionView(TypeReferenceHandle),
    CompilerString,
}

#[derive(Debug, Clone)]
struct Binding {
    name: String,
    symbol: SymbolHandle,
    type_reference: TypeReferenceHandle,
}

#[derive(Debug, Clone, Default)]
struct TypeEnvironment {
    self_type: Option<InferredType>,
    result_type: Option<TypeReferenceHandle>,
    bindings: Vec<Binding>,
}

/// One queried expression's exact-owner rows. `environments` carries the
/// collectors' environments in the same order `exact_owner_environments`
/// produced them; `executable_sites` locates the statements containing the
/// expression for `checked_machine_call_target_from_executable_owner`;
/// `contract_facts` names every checked contract fact whose roots reach it.
#[derive(Default)]
struct ExactOwnerEntry {
    environments: Vec<usize>,
    executable_sites: Vec<ExecutableSite>,
    contract_facts: Vec<arena::Handle<ContractProofFact>>,
}

/// Every indexed environment once, and each expression's rows naming them by
/// position. A statement's environment covers every expression in its tree,
/// so rows hold positions rather than a copy of its bindings apiece.
#[derive(Default)]
struct OwnerIndex {
    environments: Vec<TypeEnvironment>,
    entries: HashMap<ExpressionHandle, ExactOwnerEntry>,
}

impl OwnerIndex {
    fn intern(&mut self, environment: TypeEnvironment) -> usize {
        self.environments.push(environment);
        self.environments.len() - 1
    }

    fn add(&mut self, expression: ExpressionHandle, environment: usize) {
        self.entries
            .entry(expression)
            .or_default()
            .environments
            .push(environment);
    }

    /// Record `environment` for each expression, storing it only when at
    /// least one expression reaches it.
    fn add_all(
        &mut self,
        expressions: impl IntoIterator<Item = ExpressionHandle>,
        environment: impl FnOnce() -> TypeEnvironment,
    ) {
        let mut expressions = expressions.into_iter().peekable();
        if expressions.peek().is_none() {
            return;
        }
        let environment = self.intern(environment());
        for expression in expressions {
            self.add(expression, environment);
        }
    }
}

#[derive(Clone, Copy)]
struct ExecutableSite {
    machine: usize,
    state: usize,
    statement: usize,
}

/// Invocation-owned exact environments for an expression walk. Isolated
/// queries may omit the index and scan only their own owners.
pub struct OwnerEnvironmentIndex {
    index: OwnerIndex,
    /// The walk's index over no checked facts, for queries inside it that ask
    /// what the program alone determines.
    program_only: std::cell::OnceCell<Box<OwnerEnvironmentIndex>>,
}

impl OwnerEnvironmentIndex {
    /// Build the invocation's owners from source and the exact supplied contracts.
    pub fn new(
        program: &dyn crate::CheckingInput,
        contract_facts: &arena::Arena<ContractProofFact>,
    ) -> Self {
        Self {
            index: build_owner_environment_index(program, contract_facts),
            program_only: std::cell::OnceCell::new(),
        }
    }

    /// Source-only owners, excluding checked-contract authority.
    pub fn program_only(&self, program: &dyn crate::CheckingInput) -> &OwnerEnvironmentIndex {
        self.program_only
            .get_or_init(|| Box::new(Self::new(program, &arena::Arena::default())))
    }

    /// Every `(machine, state, statement)` position whose statement tree
    /// contains `expression`, in program order. Facts never add a site.
    pub fn containing_statement_sites(
        &self,
        expression: ExpressionHandle,
    ) -> Vec<(usize, usize, usize)> {
        self.executable_sites(expression)
            .into_iter()
            .map(|site| (site.machine, site.state, site.statement))
            .collect()
    }

    /// Whether source roots or the supplied checked contracts reach this
    /// exact expression, independently of retained detached arena copies.
    pub fn contains_expression(&self, expression: ExpressionHandle) -> bool {
        self.index.entries.contains_key(&expression)
    }

    fn environments(&self, expression: ExpressionHandle) -> Option<Vec<TypeEnvironment>> {
        let index = &self.index;
        index
            .entries
            .get(&expression)
            .map(|entry| {
                entry
                    .environments
                    .iter()
                    .map(|environment| index.environments[*environment].clone())
                    .collect::<Vec<_>>()
            })
            .filter(|environments| !environments.is_empty())
    }

    fn executable_sites(&self, expression: ExpressionHandle) -> Vec<ExecutableSite> {
        self.index
            .entries
            .get(&expression)
            .map(|entry| entry.executable_sites.clone())
            .unwrap_or_default()
    }

    /// Every checked contract fact whose roots reach `expression`, in arena
    /// order.
    pub fn containing_contract_facts(
        &self,
        expression: ExpressionHandle,
    ) -> Vec<arena::Handle<ContractProofFact>> {
        self.index
            .entries
            .get(&expression)
            .map(|entry| entry.contract_facts.clone())
            .unwrap_or_default()
    }
}

/// Source scope for one exact expression occurrence. Raw authored contracts
/// identify their public callable; executable roots use the existing statement
/// ownership index. Neither path grants checked facts or callee authority.
/// Shared occurrences with different declaration owners have no single scope.
pub fn source_declaration_owner(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    owners: &OwnerEnvironmentIndex,
) -> Option<SymbolHandle> {
    let mut owner =
        super::contract_results::source_contract_declaration_owner(program, expression).ok()?;
    for (machine, _, _) in owners.containing_statement_sites(expression) {
        let candidate = program.machines().get(machine)?.symbol;
        if !candidate.is_valid() || (owner.is_valid() && owner != candidate) {
            return None;
        }
        owner = candidate;
    }
    owner.is_valid().then_some(owner)
}
