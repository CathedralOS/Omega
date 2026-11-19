/*
    Complete selected implementations under one prepared declaration frontier.

    Generated declarations append a new source stratum. Authored bodies instead
    complete existing owners: their source and declaration identities must not be
    rebuilt or passed off as new roots. This carrier keeps those transitions
    separate and retains the actual pending syntax until a consumer demands it.
*/

#[cfg(any(test, feature = "test-support"))]
use crate::resolution::constants::initializer_dependencies::ResolvedMachineBodyDemand;
use crate::resolution::constants::initializer_dependencies::collect_machine_body_demand;
use crate::resolution::declarations::declaration_selection::ConstantSelection;
use crate::resolution::declarations::machine_equations::{self, SourceLinks};
use crate::resolution::lowering::lowerer::{ConstResolutionMode, Lowerer};
use crate::resolution::lowering::machine::{PendingMachineBody, complete_machine_body};
use crate::resolution::names::assign_constant_expression_symbols;
use crate::source_resolved::continuations::{
    RebasedResolvedBodyCompletion, ResolvedSourceActivation,
};
#[cfg(any(test, feature = "test-support"))]
use crate::source_resolved::expression::ExpressionHandle;
use crate::source_resolved::types::OperandLifetimeOwnerHandle;
use crate::source_resolved::{
    AuthoredDeclarationSelections, AuthoredSelectionExtensionFrontier,
    AuthoredSelectionExtensionRebaseError, SymbolResolvedTrees,
};
use diagnostics::Diagnostic;
use std::mem::take;
use std::sync::Arc;
use symbols::SymbolHandle;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::ItemHandle;
use syntax_trees::types::TypeReferenceHandle;

/*
    An inferred callable type makes its defining uses declaration inputs.
    Demand those existing bodies (and their lexical children) through the
    ordinary completion owner; selection itself grants no checking evidence.
*/
pub(super) fn complete_callable_type_dependencies(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    if lowerer.pending_callable_types.is_empty() {
        return Ok(());
    }
    let program = &lowerer.symbol_resolved_trees;
    let mut demanded = Vec::with_capacity(program.machines.len());
    let mut by_symbol: Vec<Option<(SymbolHandle, bool)>> = Vec::new();
    // Lowering stores lexical parents before their nested declarations.
    for machine in program.machines.iter() {
        let inherited = if machine.lexical_parent.is_valid() {
            let parent = program.machines.get(machine.lexical_parent).symbol;
            by_symbol
                .get(parent.arena_index() as usize)
                .copied()
                .flatten()
                .filter(|(known, _)| *known == parent)
                .map(|(_, demand)| demand)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "callable type inference lost its preceding lexical declaration owner",
                    )]
                })?
        } else {
            false
        };
        let demand = inherited
            || program
                .machine_type_parameters(machine)
                .iter()
                .any(crate::resolution::selections::callable_inference::pending);
        demanded.push(demand);
        let slot = machine.symbol.arena_index() as usize;
        if by_symbol.len() <= slot {
            by_symbol.resize(slot + 1, None);
        }
        by_symbol[slot] = Some((machine.symbol, demand));
    }
    let pending = lowerer
        .pending_machine_bodies
        .iter()
        .filter(|body| demanded[body.machine_ordinal])
        .cloned()
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(());
    }
    let mut completed = Vec::with_capacity(pending.len());
    for body in &pending {
        complete_machine_body(lowerer, syntax, body).map_err(|error| vec![error])?;
        completed.push(lowerer.symbol_resolved_trees.machines[body.machine_ordinal].symbol);
    }
    crate::resolution::names::assign_body_symbols(lowerer, &completed)?;
    lowerer
        .pending_machine_bodies
        .retain(|body| !demanded.get(body.machine_ordinal).copied().unwrap_or(false));
    Ok(())
}

/*
    Receipt replay needs the original helper closure before a resolved frontier
    can publish. Complete that demand on its current owner: constructing a
    frontier first would replay receipts against Deferred implementations. This
    is dependency ordering, not receipt-based authorization or relaxed replay.
*/
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
    assign_constant_expression_symbols(&mut lowerer.symbol_resolved_trees, roots.iter().copied());
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
        crate::resolution::names::assign_body_symbols(lowerer, &demand.deferred_machines)?;
        lowerer.pending_machine_bodies.retain(|pending| {
            !demand
                .deferred_machines
                .contains(&lowerer.symbol_resolved_trees.machines[pending.machine_ordinal].symbol)
        });
    }
}

#[derive(Debug, Clone, record_storage::Record)]
pub struct DeferredResolvedFrontier {
    pub(super) syntax: Arc<SyntaxTrees>,
    pub(super) trees: Arc<SymbolResolvedTrees>,
    pub(super) pending: Vec<PendingMachineBody>,
    pub(super) pending_type_inputs: Vec<(SymbolHandle, TypeReferenceHandle)>,
    /*
        Body demand resumes this exact declaration allocation. Reconstructing
        these associations from spelling would capture fresh invocation binders.
    */
    pub(super) supplied_operand_owners: Vec<(ItemHandle, OperandLifetimeOwnerHandle)>,
    pub(super) declaration_inputs: Vec<(
        ItemHandle,
        crate::resolution::lowering::operand_owners::DeclarationInput,
    )>,
    pub(super) selection: ConstantSelection<'static>,
    pub(super) sources: Option<Arc<crate::source_inputs::SourceMap>>,
    pub(super) bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
    pub(super) hoist_counter: u32,
    pub(super) const_resolution_mode: ConstResolutionMode,
    pub(super) structural_type_equations_pending: bool,
    /*
        Only template links survive publication. Application links name scratch
        rows and are recorded afresh for each demanded implementation batch.
    */
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
            declaration_inputs,
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
            && declaration_inputs == &other.declaration_inputs
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

/// Which authored bodies the caller needs from this declaration frontier.
#[derive(Clone, Copy)]
pub enum DeferredBodyScope {
    All,
    BuildAndToolchain,
    SelectedFiles,
}

impl DeferredResolvedFrontier {
    pub fn trees(&self) -> &SymbolResolvedTrees {
        &self.trees
    }

    /* Share the canonical resolved predecessor with its typed checkpoint. */
    pub fn shared_trees(&self) -> Arc<SymbolResolvedTrees> {
        Arc::clone(&self.trees)
    }

    /*
        Replace lookup membership without filtering declaration vectors: pending
        syntax still names their original ordinals. The paired typing owner
        admits the exact predecessor before invoking this operation.
    */
    pub fn activate_sources(
        mut self,
        sources: Arc<crate::source_inputs::SourceMap>,
        bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
    ) -> Result<(Self, ResolvedSourceActivation), Vec<Diagnostic>> {
        let activation = ResolvedSourceActivation::new(
            self.shared_trees(),
            Arc::clone(&sources),
            bindings.clone(),
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        self.trees = activation.successor();
        self.selection = self
            .selection
            .activate_sources(Arc::clone(&sources), bindings.clone())?;
        self.sources = Some(sources);
        self.bindings = bindings;
        Ok((self, activation))
    }

    /*
        Reserve the paired typing owner's allocation suffix before resolution
        allocates more symbols, domain identities or service rows. This adds no
        declarations or evidence; existing references retain their exact IDs.
    */
    pub fn with_prepared_allocations(
        mut self,
        update: crate::source_resolved::continuations::ResolvedAllocationExtension,
    ) -> Result<Self, Vec<Diagnostic>> {
        let (predecessor, successor) = update.into_parts();
        if self.trees != predecessor {
            return Err(vec![Diagnostic::error(
                "prepared allocation join does not match the exact deferred resolved predecessor",
            )]);
        }
        self.trees = successor;
        Ok(self)
    }

    pub fn pending_machines(&self) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.pending
            .iter()
            .map(|pending| self.trees.machines[pending.machine_ordinal].symbol)
    }

    pub fn pending_machines_in(
        &self,
        scope: DeferredBodyScope,
    ) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
        use source_admission::SourceOrigin;
        use source_provenance::DependencyScope;

        let symbols = &self.trees.symbols;
        let selected = match scope {
            DeferredBodyScope::All => self.pending_machines().collect(),
            DeferredBodyScope::BuildAndToolchain => self
                .pending_machines()
                .filter(|symbol| {
                    symbols
                        .symbol_provenance_source_span(*symbol)
                        .and_then(|span| symbols.source_file(span))
                        .is_some_and(|file| {
                            file.origin == SourceOrigin::Toolchain
                                || file.dependency_scope == DependencyScope::Build
                        })
                })
                .collect(),
            DeferredBodyScope::SelectedFiles => {
                let sources = self.sources.as_ref().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selecting deferred bodies requires the frontier's admitted source files",
                    )]
                })?;
                self.pending_machines()
                    .filter(|symbol| {
                        symbols
                            .symbol_provenance_source_span(*symbol)
                            .is_some_and(|span| sources.source_is_selected(span.source_id))
                    })
                    .collect()
            }
        };
        Ok(selected)
    }

    /*
        Discover exact executable-body demand from resolved machines and
        expressions. Deferred implementations stop traversal and remain owned
        here; complete the reported batch and repeat until none remain before
        admitting the returned machine roster for typing or execution.
    */
    #[cfg(any(test, feature = "test-support"))]
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

    /*
        Completion is transactional. A failed body cannot partly advance the
        retained frontier or change which source its next consumer receives.
    */
    pub fn complete_machine_bodies(
        mut self,
        selected: &[SymbolHandle],
    ) -> Result<ResolvedBodyCompletion, Vec<Diagnostic>> {
        let symbol_key = |symbol: SymbolHandle| (symbol.arena_index(), symbol.generation());
        let mut pending_by_symbol = self
            .pending
            .iter()
            .enumerate()
            .map(|(position, body)| {
                (
                    symbol_key(self.trees.machines[body.machine_ordinal].symbol),
                    position,
                )
            })
            .collect::<Vec<_>>();
        pending_by_symbol.sort_unstable();
        let mut taken = std::collections::BTreeSet::new();
        let mut pending = Vec::new();
        for &symbol in selected {
            let key = symbol_key(symbol);
            if taken.contains(&key) {
                continue;
            }
            let start = pending_by_symbol.partition_point(|(row, _)| *row < key);
            let body = pending_by_symbol
                .get(start)
                .filter(|(row, _)| *row == key)
                .map(|(_, position)| &self.pending[*position])
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "body completion selected a declaration with no pending implementation",
                    )]
                })?;
            taken.insert(key);
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
        self.supplied_operand_owners = take(&mut lowerer.supplied_operand_owners);
        self.declaration_inputs = take(&mut lowerer.declaration_inputs);
        self.pending.append(&mut lowerer.pending_machine_bodies);
        self.equation_sources = lowerer.equation_sources.take().map(|mut links| {
            links.expressions.clear();
            links.statements.clear();
            links
        });
        self.pending_type_inputs
            .extend(crate::resolution::lowering::type_inputs::bind(
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
        lowerer.declaration_inputs = self.declaration_inputs.clone();
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
        crate::resolution::lowering::item::lower_item(lowerer, syntax, item)
            .map_err(|error| vec![error])?;
    }
    crate::resolution::selections::select_operator_homes(lowerer)?;
    crate::resolution::names::assign_declarations(lowerer, syntax)
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
    crate::resolution::names::assign_body_symbols(lowerer, selected)
}

/*
    Private predecessor/successor pairing. Only the resolver can construct a
    completion; later typing rejoins it to its own retained predecessor.
*/
#[derive(Debug)]
pub struct ResolvedBodyCompletion {
    successor: DeferredResolvedFrontier,
    predecessor: Arc<SymbolResolvedTrees>,
    frontier: AuthoredSelectionExtensionFrontier,
    selected: Vec<SymbolHandle>,
}

impl ResolvedBodyCompletion {
    /*
        Before a typed checkpoint exists, the completed resolved frontier is
        itself the next owner. Once typing exists, use its completion join.
    */
    pub fn into_resolved_frontier(self) -> DeferredResolvedFrontier {
        self.successor
    }

    pub fn rebase_authored_selections_for_typed_completion(
        self,
        destination: &AuthoredDeclarationSelections,
    ) -> Result<
        (RebasedResolvedBodyCompletion, DeferredResolvedFrontier),
        (Self, AuthoredSelectionExtensionRebaseError),
    > {
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
                Ok((
                    RebasedResolvedBodyCompletion {
                        successor: successor.shared_trees(),
                        predecessor,
                        selected,
                    },
                    successor,
                ))
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
        let tokens = crate::resolution::frontend_fixture::Lexer::new(source_text)
            .tokenize()
            .unwrap();
        let mut sources = crate::source_inputs::SourceMap::default();
        let source_id = sources
            .add("completion/main.omg".into(), source_text.to_owned())
            .source_id;
        let syntax =
            crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens)
                .unwrap();
        let frontier = crate::resolution::pre_resolution::resolve_deferred_numeric_probe(
            super::super::ResolutionRequest {
                syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
                retained_base: None,
            },
        )
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
        let retained_statements = crate::resolution::tables::body_statements(&predecessor)
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
