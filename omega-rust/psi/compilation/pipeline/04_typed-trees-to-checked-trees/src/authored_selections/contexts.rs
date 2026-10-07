mod call_targets;
mod environments;
mod indexing;
mod inference;

pub(super) use call_targets::{
    checked_collection_view_intrinsic_from_exact_owner,
    checked_expression_type_reference_from_exact_owner,
    checked_machine_call_target_from_exact_owner, checked_member_target_from_exact_owner,
};
use checked_trees::{CheckFacts, ContractProofFact};
use indexing::build_owner_environment_index;
pub(super) use indexing::proof_fact_contains_expression;
use language_semantics::declaration_selection::CollectionMeasure;
use std::cell::RefCell;
/// Handle-keyed indexes and reach sets take the integer hasher.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OwnerMemberTarget {
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

/// Lazy whole-program index of every expression's exact owner rows. The
/// exact-owner collectors answer each query by rescanning every contract,
/// domain, proposition, measure, parameter, ranking, and statement; a walk
/// that asks the question once per expression shares this single pass
/// instead. Callers outside such a walk pass `None` and keep the per-query
/// rescan — building the index costs more than one scan.
#[derive(Default)]
pub(crate) struct OwnerEnvironmentIndex {
    index: RefCell<Option<OwnerIndex>>,
    /// The walk's index over no checked facts, for queries inside it that ask
    /// what the program alone determines.
    program_only: std::cell::OnceCell<Box<OwnerEnvironmentIndex>>,
}

impl OwnerEnvironmentIndex {
    /// The same walk's index built over `CheckFacts::default()`; queries that
    /// pass default facts use it so both indexes answer as their rescans do.
    pub(crate) fn program_only(&self) -> &OwnerEnvironmentIndex {
        self.program_only.get_or_init(Box::default)
    }

    /// Every `(machine, state, statement)` position whose statement tree
    /// contains `expression`, in program order. Facts never add a site.
    pub(super) fn containing_statement_sites(
        &self,
        program: &TypedTrees,
        facts: &CheckFacts,
        expression: ExpressionHandle,
    ) -> Vec<(usize, usize, usize)> {
        self.executable_sites(program, facts, expression)
            .into_iter()
            .map(|site| (site.machine, site.state, site.statement))
            .collect()
    }

    fn environments(
        &self,
        program: &TypedTrees,
        facts: &CheckFacts,
        expression: ExpressionHandle,
    ) -> Option<Vec<TypeEnvironment>> {
        let index = self.index(program, facts);
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

    fn executable_sites(
        &self,
        program: &TypedTrees,
        facts: &CheckFacts,
        expression: ExpressionHandle,
    ) -> Vec<ExecutableSite> {
        self.index(program, facts)
            .entries
            .get(&expression)
            .map(|entry| entry.executable_sites.clone())
            .unwrap_or_default()
    }

    /// Every checked contract fact whose roots reach `expression`, in arena
    /// order.
    pub(super) fn containing_contract_facts(
        &self,
        program: &TypedTrees,
        facts: &CheckFacts,
        expression: ExpressionHandle,
    ) -> Vec<arena::Handle<ContractProofFact>> {
        self.index(program, facts)
            .entries
            .get(&expression)
            .map(|entry| entry.contract_facts.clone())
            .unwrap_or_default()
    }

    fn index<'a>(
        &'a self,
        program: &TypedTrees,
        facts: &CheckFacts,
    ) -> std::cell::Ref<'a, OwnerIndex> {
        {
            let mut borrow = self.index.borrow_mut();
            if borrow.is_none() {
                *borrow = Some(build_owner_environment_index(program, facts));
            }
        }
        std::cell::Ref::map(self.index.borrow(), |index| index.as_ref().unwrap())
    }
}
