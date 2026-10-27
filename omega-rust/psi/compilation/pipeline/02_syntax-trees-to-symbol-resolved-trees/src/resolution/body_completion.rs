//! Complete selected implementations under one prepared declaration frontier.
//!
//! Generated declarations append a new source stratum. Authored bodies instead
//! complete existing owners: their source and declaration identities must not be
//! rebuilt or passed off as new roots. This carrier keeps those transitions
//! separate and retains the actual pending syntax until a consumer demands it.

use super::lowerer::{ConstResolutionMode, Lowerer};
use crate::constant::initializer_dependencies::{
    ResolvedMachineBodyDemand, collect_machine_body_demand,
};
use crate::lowering::machine::{PendingMachineBody, complete_machine_body};
use crate::preparation::declaration_selection::ConstantSelection;
use crate::preparation::machine_equations::{self, SourceLinks};
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbol_resolved_trees::{
    AuthoredDeclarationSelections, AuthoredSelectionExtensionFrontier,
    AuthoredSelectionExtensionRebaseError, SymbolResolvedTrees, expression::ExpressionHandle,
};
use symbols::SymbolHandle;
use syntax_trees::SyntaxTrees;

/// Receipt replay needs the original helper closure before a resolved frontier
/// can publish. Complete that demand on its current owner: constructing a
/// frontier first would replay receipts against Deferred implementations. This
/// is dependency ordering, not receipt-based authorization or relaxed replay.
pub(super) fn complete_initializer_receipt_dependencies(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    let roots = lowerer
        .pending_const_initializers
        .iter()
        .map(|initializer| initializer.authored)
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Ok(());
    }
    crate::symbols::assign_constant_expression_symbols(
        &mut lowerer.symbol_resolved_trees,
        roots.iter().copied(),
    );
    loop {
        let demand = collect_machine_body_demand(&lowerer.symbol_resolved_trees, &[], &roots)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        if demand.deferred_machines.is_empty() {
            return Ok(());
        }
        for &symbol in &demand.deferred_machines {
            let pending = lowerer
                .pending_machine_bodies
                .iter()
                .find(|pending| {
                    lowerer.symbol_resolved_trees.machines[pending.machine_ordinal].symbol == symbol
                })
                .cloned()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "initializer receipt dependency has no retained deferred implementation",
                    )]
                })?;
            complete_machine_body(lowerer, syntax, &pending).map_err(|error| vec![error])?;
        }
        crate::symbols::assign_body_symbols(lowerer, &demand.deferred_machines)?;
        lowerer.pending_machine_bodies.retain(|pending| {
            !demand
                .deferred_machines
                .contains(&lowerer.symbol_resolved_trees.machines[pending.machine_ordinal].symbol)
        });
    }
}

#[derive(Debug, Clone)]
pub struct DeferredResolvedFrontier {
    pub(super) syntax: Arc<SyntaxTrees>,
    pub(super) trees: Arc<SymbolResolvedTrees>,
    pub(super) pending: Vec<PendingMachineBody>,
    pub(super) pending_type_inputs: Vec<(SymbolHandle, syntax_trees::types::TypeReferenceHandle)>,
    // Body demand resumes this exact declaration allocation. Reconstructing
    // these associations from spelling would capture fresh invocation binders.
    pub(super) supplied_operand_owners: Vec<(
        syntax_trees::item::ItemHandle,
        symbol_resolved_trees::types::OperandLifetimeOwnerHandle,
    )>,
    pub(super) conformance_inputs: Vec<(syntax_trees::item::ItemHandle, usize)>,
    pub(super) selection: ConstantSelection<'static>,
    pub(super) sources: Option<Arc<source::SourceMap>>,
    pub(super) bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    pub(super) hoist_counter: u32,
    pub(super) const_resolution_mode: ConstResolutionMode,
    pub(super) structural_type_equations_pending: bool,
    /// Only template links survive publication. Application links name scratch
    /// rows and are recorded afresh for each demanded implementation batch.
    pub(super) equation_sources: Option<SourceLinks>,
}

impl PartialEq for DeferredResolvedFrontier {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            syntax,
            trees,
            pending,
            pending_type_inputs,
            supplied_operand_owners,
            conformance_inputs,
            selection,
            sources,
            bindings,
            hoist_counter,
            const_resolution_mode,
            structural_type_equations_pending,
            equation_sources,
        } = self;
        syntax == &other.syntax
            && trees == &other.trees
            && pending.len() == other.pending.len()
            && pending
                .iter()
                .zip(&other.pending)
                .all(|(left, right)| left.same_syntax(right, &syntax.text, &other.syntax.text))
            && pending_type_inputs == &other.pending_type_inputs
            && supplied_operand_owners == &other.supplied_operand_owners
            && conformance_inputs == &other.conformance_inputs
            && selection == &other.selection
            && sources == &other.sources
            && bindings == &other.bindings
            && hoist_counter == &other.hoist_counter
            && const_resolution_mode == &other.const_resolution_mode
            && structural_type_equations_pending == &other.structural_type_equations_pending
            && match (equation_sources, &other.equation_sources) {
                (Some(left), Some(right)) => {
                    left.same_syntax(right, &syntax.text, &other.syntax.text)
                }
                (None, None) => true,
                _ => false,
            }
    }
}
impl Eq for DeferredResolvedFrontier {}

impl DeferredResolvedFrontier {
    /// Resume typing against the exact predecessor owned by this resolver.
    /// Ordinary probes append to that pair; they must not retype its completed
    /// declarations or reinterpret its finalized authored selections.
    pub fn into_extension(self) -> Result<super::DeferredResolvedExtension, Vec<Diagnostic>> {
        let predecessor = self.selection.shared_retained_base().ok_or_else(|| {
            vec![Diagnostic::error(
                "deferred resolution has no owned predecessor for typing",
            )]
        })?;
        Ok(super::DeferredResolvedExtension {
            successor: self,
            predecessor,
        })
    }

    /// Transfer probe-only input custody on this exact completed declaration
    /// roster. Symbol handles remain those consumed by its seeded typed base.
    pub fn into_pending_type_inputs(
        self,
    ) -> Vec<(SymbolHandle, syntax_trees::types::TypeReferenceHandle)> {
        self.pending_type_inputs
    }

    pub fn trees(&self) -> &SymbolResolvedTrees {
        &self.trees
    }

    /// Share the canonical resolved predecessor with its typed checkpoint.
    pub fn shared_trees(&self) -> Arc<SymbolResolvedTrees> {
        Arc::clone(&self.trees)
    }

    /// Replace lookup membership without filtering declaration vectors: pending
    /// syntax still names their original ordinals. The paired typing owner
    /// admits the exact predecessor before invoking this operation.
    pub fn activate_sources(
        mut self,
        sources: Arc<source::SourceMap>,
        bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let trees = Arc::make_mut(&mut self.trees);
        trees.symbols = trees
            .symbols
            .clone()
            .begin_source_activation(Arc::clone(&sources), bindings.clone())
            .map_err(|reason| vec![Diagnostic::error(reason)])?
            .finish();
        trees.resolution_indexes = Default::default();
        self.selection = self
            .selection
            .activate_sources(Arc::clone(&sources), bindings.clone())?;
        self.sources = Some(sources);
        self.bindings = bindings;
        Ok(self)
    }

    /// Reserve the paired typing owner's allocation suffix before resolution
    /// allocates more symbols, domain identities or service rows. This adds no
    /// declarations or evidence; existing references retain their exact IDs.
    pub fn with_prepared_allocations(
        mut self,
        predecessor: &SymbolResolvedTrees,
        symbols: &symbols::SymbolTable,
        domains: &language_semantics::SemanticDomainTable,
        service_rows: &language_semantics::ServiceReachRowTable,
    ) -> Result<Self, Vec<Diagnostic>> {
        if self.trees.as_ref() != predecessor {
            return Err(vec![Diagnostic::error(
                "prepared allocation join does not match the exact deferred resolved predecessor",
            )]);
        }
        if !symbols.is_generated_extension_of(&predecessor.symbols) {
            return Err(vec![Diagnostic::error(
                "prepared allocation join changed retained source custody or appended authored symbols",
            )]);
        }
        for ordinal in 1..=predecessor.semantic_domains.len() {
            let identity = language_semantics::SemanticDomainId(ordinal as u32);
            if domains.name(identity) != predecessor.semantic_domains.name(identity) {
                return Err(vec![Diagnostic::error(format!(
                    "prepared allocation join changed retained semantic domain {ordinal}"
                ))]);
            }
        }
        if !service_rows.starts_with(&predecessor.service_reach_rows) {
            return Err(vec![Diagnostic::error(
                "prepared allocation join changed retained service-row storage",
            )]);
        }
        if self.trees.symbols != *symbols
            || self.trees.semantic_domains != *domains
            || self.trees.service_reach_rows != *service_rows
        {
            let prepared = Arc::make_mut(&mut self.trees);
            prepared.symbols = symbols.clone();
            prepared.semantic_domains = domains.clone();
            prepared.service_reach_rows = service_rows.clone();
        }
        Ok(self)
    }

    pub fn pending_machines(&self) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.pending
            .iter()
            .map(|pending| self.trees.machines[pending.machine_ordinal].symbol)
    }

    /// Discover exact executable-body demand from resolved machines and
    /// expressions. Deferred implementations stop traversal and remain owned
    /// here; complete the reported batch and repeat until none remain before
    /// admitting the returned machine roster for typing or execution.
    pub fn body_demand(
        &self,
        machine_roots: &[SymbolHandle],
        expression_roots: &[ExpressionHandle],
    ) -> Result<ResolvedMachineBodyDemand, Vec<Diagnostic>> {
        let demand = collect_machine_body_demand(&self.trees, machine_roots, expression_roots)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        for symbol in &demand.deferred_machines {
            if !self.pending_machines().any(|pending| pending == *symbol) {
                return Err(vec![Diagnostic::error(
                    "constant dependency has no retained deferred implementation",
                )]);
            }
        }
        Ok(demand)
    }

    /// Completion is transactional. A failed body cannot partly advance the
    /// retained frontier or change which source its next consumer receives.
    pub fn complete_machine_bodies(
        mut self,
        selected: &[SymbolHandle],
    ) -> Result<ResolvedBodyCompletion, Vec<Diagnostic>> {
        let mut pending = Vec::new();
        for &symbol in selected {
            if pending.iter().any(|body: &PendingMachineBody| {
                self.trees.machines[body.machine_ordinal].symbol == symbol
            }) {
                continue;
            }
            let body = self
                .pending
                .iter()
                .find(|body| self.trees.machines[body.machine_ordinal].symbol == symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "body completion selected a declaration with no pending implementation",
                    )]
                })?;
            pending.push(body.clone());
        }
        let predecessor = self.trees.clone();
        let frontier = predecessor.authored_selection_extension_frontier();
        let syntax_frontier = self.syntax.root_item_handles().len();
        if self.equation_sources.is_some() {
            loop {
                let mut provisional = self.seed_body_lowerer()?;
                provisional.const_resolution_mode = ConstResolutionMode::ArgumentSelection;
                provisional.structural_type_equations_pending = true;
                append_generated_declarations(&mut provisional, &self.syntax, syntax_frontier)?;
                lower_selected_bodies(&mut provisional, &self.syntax, &pending, selected)?;
                super::finish_resolution(&mut provisional, &self.syntax, &[], selected)?;
                let selection = provisional.take_constant_selection()?;
                let links = provisional.equation_sources.take().unwrap_or_default();
                let before = self.syntax.root_item_handles().len();
                machine_equations::complete(
                    Arc::make_mut(&mut self.syntax),
                    &provisional.symbol_resolved_trees,
                    &links,
                    &selection,
                )?;
                // A completed argument can synthesize data and attached
                // machines. Their headers enter ordinary selection before
                // publication, but their implementations remain deferred.
                if self.syntax.root_item_handles().len() == before {
                    break;
                }
            }
        }
        let mut lowerer = self.seed_body_lowerer()?;
        append_generated_declarations(&mut lowerer, &self.syntax, syntax_frontier)?;
        lower_selected_bodies(&mut lowerer, &self.syntax, &pending, selected)?;
        super::finish_resolution(&mut lowerer, &self.syntax, &[], selected)?;
        let hoist_counter = lowerer.hoist_counter;
        self.supplied_operand_owners = std::mem::take(&mut lowerer.supplied_operand_owners);
        self.conformance_inputs = std::mem::take(&mut lowerer.conformance_inputs);
        self.pending.append(&mut lowerer.pending_machine_bodies);
        self.equation_sources = lowerer.equation_sources.take().map(|mut links| {
            links.expressions.clear();
            links.statements.clear();
            links
        });
        self.pending_type_inputs.extend(super::type_inputs::bind(
            &lowerer.symbol_resolved_trees,
            &lowerer.pending_type_inputs,
        )?);
        let trees = lowerer.into_trees();
        self.trees = Arc::new(trees);
        self.hoist_counter = hoist_counter;
        self.pending
            .retain(|body| !selected.contains(&self.trees.machines[body.machine_ordinal].symbol));
        Ok(ResolvedBodyCompletion {
            successor: self,
            predecessor,
            frontier,
            selected: selected.to_vec(),
        })
    }

    fn seed_body_lowerer(&self) -> Result<Lowerer, Vec<Diagnostic>> {
        let mut lowerer = Lowerer::new(self.sources.clone(), self.bindings.clone());
        lowerer.seed_resolved_base(Arc::clone(&self.trees))?;
        lowerer.supplied_operand_owners = self.supplied_operand_owners.clone();
        lowerer.conformance_inputs = self.conformance_inputs.clone();
        lowerer.constant_selection = Some(self.selection.clone());
        lowerer.hoist_counter = self.hoist_counter;
        lowerer.const_resolution_mode = self.const_resolution_mode;
        lowerer.structural_type_equations_pending = self.structural_type_equations_pending;
        lowerer.equation_sources = self.equation_sources.clone();
        Ok(lowerer)
    }
}

fn append_generated_declarations(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    frontier: usize,
) -> Result<(), Vec<Diagnostic>> {
    if syntax.root_item_handles().len() == frontier {
        return Ok(());
    }
    lowerer.defer_machine_bodies = true;
    for &item in syntax.root_item_handles().iter().skip(frontier) {
        crate::lowering::item::lower_item(lowerer, syntax, item).map_err(|error| vec![error])?;
    }
    crate::selection::select_operator_homes(lowerer)?;
    crate::symbols::assign_declarations(lowerer)
}

fn lower_selected_bodies(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    pending: &[PendingMachineBody],
    selected: &[SymbolHandle],
) -> Result<(), Vec<Diagnostic>> {
    for body in pending {
        complete_machine_body(lowerer, syntax, body).map_err(|error| vec![error])?;
    }
    crate::symbols::assign_body_symbols(lowerer, selected)
}

/// Private predecessor/successor pairing. Only the resolver can construct a
/// completion; later typing rejoins it to its own retained predecessor.
#[derive(Debug)]
pub struct ResolvedBodyCompletion {
    successor: DeferredResolvedFrontier,
    predecessor: Arc<SymbolResolvedTrees>,
    frontier: AuthoredSelectionExtensionFrontier,
    selected: Vec<SymbolHandle>,
}

#[derive(Debug)]
pub struct RebasedResolvedBodyCompletion {
    successor: DeferredResolvedFrontier,
    predecessor: Arc<SymbolResolvedTrees>,
    selected: Vec<SymbolHandle>,
}

impl ResolvedBodyCompletion {
    /// Before a typed checkpoint exists, the completed resolved frontier is
    /// itself the next owner. Once typing exists, use its completion join.
    pub fn into_resolved_frontier(self) -> DeferredResolvedFrontier {
        self.successor
    }

    pub fn rebase_authored_selections_for_typed_completion(
        self,
        destination: &AuthoredDeclarationSelections,
    ) -> Result<RebasedResolvedBodyCompletion, (Self, AuthoredSelectionExtensionRebaseError)> {
        let Self {
            mut successor,
            predecessor,
            frontier,
            selected,
        } = self;
        let trees = Arc::unwrap_or_clone(successor.trees);
        match trees.rebase_authored_selection_extension(frontier, destination) {
            Ok(trees) => {
                successor.trees = Arc::new(trees);
                Ok(RebasedResolvedBodyCompletion {
                    successor,
                    predecessor,
                    selected,
                })
            }
            Err((trees, error)) => {
                successor.trees = Arc::new(trees);
                Err((
                    Self {
                        successor,
                        predecessor,
                        frontier,
                        selected,
                    },
                    error,
                ))
            }
        }
    }
}

impl RebasedResolvedBodyCompletion {
    /// The returned frontier and the typed checkpoint share this exact rebased
    /// successor. There is no separately advanced raw resolver ledger.
    pub fn into_typing_completion_parts(
        self,
    ) -> (
        DeferredResolvedFrontier,
        Arc<SymbolResolvedTrees>,
        Vec<SymbolHandle>,
    ) {
        (self.successor, self.predecessor, self.selected)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn completion_batches_preserve_retained_statements_and_predecessors() {
        let source_text = r#"
            machine first(value: u64) -> u64 {
                let incremented: u64 = value + 1;
                incremented
            }
            machine second(value: u64) -> u64 {
                let doubled: u64 = value + value;
                doubled
            }
        "#;
        let tokens = source_files_to_tokens::Lexer::new(source_text)
            .tokenize()
            .unwrap();
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add("completion/main.omg".into(), source_text.to_owned())
            .source_id;
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let frontier =
            super::super::resolve_deferred_numeric_probe(super::super::ResolutionRequest {
                syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
                retained_base: None,
            })
            .unwrap();
        let pending: Vec<_> = frontier.pending_machines().collect();
        assert_eq!(pending.len(), 2);
        let initial = frontier.shared_trees();
        let initial_snapshot = initial.as_ref().clone();
        let first = frontier
            .complete_machine_bodies(&pending[..1])
            .unwrap()
            .into_resolved_frontier();
        assert_eq!(initial.as_ref(), &initial_snapshot);
        let predecessor = first.shared_trees();
        let predecessor_snapshot = predecessor.as_ref().clone();
        let machine = predecessor
            .machines
            .iter()
            .find(|machine| machine.symbol == pending[0])
            .unwrap();
        let state = predecessor.machine_state(predecessor.machine_state_handles(machine.states)[0]);
        let retained_nodes = state.statement_nodes;
        let retained_statements = predecessor
            .tables
            .bodies
            .statements
            .statements(retained_nodes)
            .to_vec();
        assert!(!retained_statements.is_empty());
        let second = first
            .complete_machine_bodies(&pending[1..])
            .unwrap()
            .into_resolved_frontier();
        assert_eq!(predecessor.as_ref(), &predecessor_snapshot);
        assert_eq!(
            second
                .trees()
                .tables
                .bodies
                .statements
                .statements(retained_nodes),
            retained_statements
        );
        assert!(
            second.trees().tables.bodies.statements.statement_count()
                > predecessor.tables.bodies.statements.statement_count()
        );
        assert_eq!(second.pending_machines().count(), 0);
        let completed = second.shared_trees();
        let completed_snapshot = completed.as_ref().clone();
        assert!(second.complete_machine_bodies(&pending[..1]).is_err());
        assert_eq!(completed.as_ref(), &completed_snapshot);
    }
}
