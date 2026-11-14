/*
    Start here: the resolution route.

    [`resolve`] takes one [`ResolutionRequest`]; [`begin`] prepares the forest
    and a lowerer, and [`drive`] is the route: translate every root item, then
    let each owner settle its phase in the one order their preconditions
    allow. The other operations run the same route for a different product.
    [`resolve_const_argument_selection`] selects const arguments without
    synthesis or evaluation. [`resolve_numeric_probe`] normalizes constants
    while retaining machine-equation execution obligations.
    [`prepare_const_initializer_selection`] stops at
    preparation evidence that grants no typing authority. [`resolve_extension`]
    resolves a later stratum against a retained base and returns the carrier
    the typed continuation rebases; see `continuations`. `lowerer` is the
    working state this route drives.
*/

mod body_completion {
    /*
        Complete selected implementations under one prepared declaration frontier.

        Generated declarations append a new source stratum. Authored bodies instead
        complete existing owners: their source and declaration identities must not be
        rebuilt or passed off as new roots. This carrier keeps those transitions
        separate and retains the actual pending syntax until a consumer demands it.
    */

    use super::lowerer::{ConstResolutionMode, Lowerer};
    use crate::constant::initializer_dependencies::{
        ResolvedMachineBodyDemand, collect_machine_body_demand,
    };
    use crate::lowering::machine::{PendingMachineBody, complete_machine_body};
    use crate::preparation::declaration_selection::ConstantSelection;
    use crate::preparation::machine_equations::{self, SourceLinks};
    use crate::symbols::assign_constant_expression_symbols;
    use diagnostics::Diagnostic;
    use std::mem::take;
    use std::sync::Arc;
    use symbol_resolved_trees::types::OperandLifetimeOwnerHandle;
    use symbol_resolved_trees::{
        AuthoredDeclarationSelections, AuthoredSelectionExtensionFrontier,
        AuthoredSelectionExtensionRebaseError, SymbolResolvedTrees, expression::ExpressionHandle,
    };
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
                    .any(crate::selection::callable_inference::pending);
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
        crate::symbols::assign_body_symbols(lowerer, &completed)?;
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
        assign_constant_expression_symbols(
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
                !demand.deferred_machines.contains(
                    &lowerer.symbol_resolved_trees.machines[pending.machine_ordinal].symbol,
                )
            });
        }
    }

    #[derive(Debug, Clone, compiler_storage::Record)]
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
        pub(super) declaration_inputs: Vec<(ItemHandle, super::operand_owners::DeclarationInput)>,
        pub(super) selection: ConstantSelection<'static>,
        pub(super) sources: Option<Arc<source::SourceMap>>,
        pub(super) bindings: Vec<symbols::SourceScopedTopLevelBinding>,
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

    impl DeferredResolvedFrontier {
        /*
            Resume typing against the exact predecessor owned by this resolver.
            Ordinary probes append to that pair; they must not retype its completed
            declarations or reinterpret its finalized authored selections.
        */
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

        /*
            Transfer probe-only input custody on this exact completed declaration
            roster. Symbol handles remain those consumed by its seeded typed base.
        */
        pub fn into_pending_type_inputs(self) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
            self.pending_type_inputs
        }

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

        /*
            Reserve the paired typing owner's allocation suffix before resolution
            allocates more symbols, domain identities or service rows. This adds no
            declarations or evidence; existing references retain their exact IDs.
        */
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

        /*
            Discover exact executable-body demand from resolved machines and
            expressions. Deferred implementations stop traversal and remain owned
            here; complete the reported batch and repeat until none remain before
            admitting the returned machine roster for typing or execution.
        */
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
            self.pending_type_inputs.extend(super::type_inputs::bind(
                &lowerer.symbol_resolved_trees,
                &lowerer.pending_type_inputs,
            )?);
            let trees = lowerer.into_trees();
            self.trees = Arc::new(trees);
            self.hoist_counter = hoist_counter;
            self.pending.retain(|body| {
                !selected.contains(&self.trees.machines[body.machine_ordinal].symbol)
            });
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
            crate::lowering::item::lower_item(lowerer, syntax, item)
                .map_err(|error| vec![error])?;
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

    #[derive(Debug)]
    pub struct RebasedResolvedBodyCompletion {
        successor: DeferredResolvedFrontier,
        predecessor: Arc<SymbolResolvedTrees>,
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
        ) -> Result<RebasedResolvedBodyCompletion, (Self, AuthoredSelectionExtensionRebaseError)>
        {
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
        /*
            The returned frontier and the typed checkpoint share this exact rebased
            successor. There is no separately advanced raw resolver ledger.
        */
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
            let state =
                predecessor.machine_state(predecessor.machine_state_handles(machine.states)[0]);
            let retained_nodes = state.statement_nodes;
            let retained_statements = crate::tables::body_statements(&predecessor)
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
}
mod continuations {
    /*
        Carriers a later phase continues from.

        `ConstInitializerSelection` is what semantic initializer evaluation reads:
        resolved trees whose computed initializers are still authored expression
        roots, beside the selector that named their declarations. The seeded
        carriers hold an extension resolved against a retained base; the typed
        continuation must rebase the extension's authored selections onto its own
        ledger before the trees can enter it.
    */

    use diagnostics::Diagnostic;
    use std::sync::Arc;
    use symbol_resolved_trees::{
        AuthoredDeclarationSelections, AuthoredSelectionExtensionFrontier,
        AuthoredSelectionExtensionRebaseError, SymbolResolvedTrees,
    };
    use syntax_trees::SyntaxTrees;

    /*
        Private preparation evidence for semantic initializer evaluation. The forest
        retains unresolved values and operator obligations; it is not a completed
        resolution result and grants no authority to type, execute, or publish it.
        Declaration relationships and their pending source/body work remain under
        the same owner; a read-only graph query does not discard that frontier.
    */
    pub struct ConstInitializerSelection {
        pub(super) frontier: super::DeferredResolvedFrontier,
    }

    /*
        Encoding for private initializer preparation. Its bytes carry no domain
        evidence; the exact pending destination must still pass ordinary checking.
    */
    pub struct ConstInitializerPayload {
        pub value: language_semantics::const_value::CanonicalConstValue,
        pub pending_destination: Option<ConstDestinationObligation>,
    }

    use crate::constant::initializer_dependencies::ConstInitializerDependencies;
    use crate::constant::initializer_dependencies::DependencyPurpose;
    use crate::constant::initializer_dependencies::collect;
    use crate::constant::pending_const_initializer_leaves;
    use crate::preparation::generic_data::canonicalize_pending_initializer_payload;
    use crate::preparation::generic_data::const_destination_requires_proof;
    pub use syntax_trees::item::ConstDestinationObligation;

    impl ConstInitializerSelection {
        pub fn retained_constant(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Result<&symbol_resolved_trees::constant::ConstDeclaration, Vec<Diagnostic>> {
            let base = self.frontier.selection.retained_base().ok_or_else(|| {
                vec![Diagnostic::error(
                    "constant input has no retained predecessor",
                )]
            })?;
            base.const_declaration(symbol)
                .filter(|_| base.symbols.symbol_source_is_selected(symbol))
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "constant input is not a selected predecessor declaration",
                    )]
                })
        }

        /*
            Exact completed input from this selection's predecessor. New declarations
            cannot borrow readiness or custody merely by having the same name.
        */
        pub fn retained_constant_custody(
            &self,
            symbol: symbols::SymbolHandle,
            purpose: crate::constant::initializer_normalization::InitializerCustodyPurpose,
        ) -> Result<syntax_trees::item::ConstInitializerCustody, Vec<Diagnostic>> {
            let base = self.frontier.selection.retained_base().ok_or_else(|| {
                vec![Diagnostic::error(
                    "constant input has no retained predecessor",
                )]
            })?;
            let declaration = self.retained_constant(symbol)?;
            crate::constant::initializer_normalization::retained_constant_custody(
                base,
                declaration,
                purpose,
            )
            .map_err(|diagnostic| vec![diagnostic])
        }

        pub fn initializer_expression_dependencies(
            &self,
            syntax: &SyntaxTrees,
            definition: &syntax_trees::item::ConstDefinition,
            expression: syntax_trees::expression::ExpressionHandle,
        ) -> Result<ConstInitializerDependencies, Vec<Diagnostic>> {
            if !self
                .pending_leaves(syntax, definition)?
                .iter()
                .any(|leaf| leaf.expression == expression)
            {
                return Err(vec![
                    Diagnostic::error(
                        "dependency request is not an authored scalar initializer leaf",
                    )
                    .with_source_span(definition.name.source_span()),
                ]);
            }
            self.pending_leaf_dependencies(syntax, definition, expression, None)
        }

        pub fn pending_leaf_dependencies(
            &self,
            syntax: &SyntaxTrees,
            definition: &syntax_trees::item::ConstDefinition,
            expression: syntax_trees::expression::ExpressionHandle,
            selected_calls: Option<&[(source::SourceSpan, source::SourceSpan)]>,
        ) -> Result<ConstInitializerDependencies, Vec<Diagnostic>> {
            let declaration = self
                .frontier
                .trees
                .const_declaration_at_source_span(definition.name.source_span())
                .ok_or_else(|| vec![Diagnostic::error("initializer leaf lost its declaration")])?;
            let root = if declaration.authored_initializer.is_valid() {
                declaration.authored_initializer
            } else {
                declaration.initializer
            };
            crate::constant::initializer_dependencies::expression_at_source(
                &self.frontier.trees,
                root,
                syntax.expressions.source_span(expression),
            )
            .and_then(|expression| collect(&self.frontier.trees, expression, selected_calls))
            .map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(definition.name.source_span())]
            })
        }

        pub fn initializer_dependencies(
            &self,
            _syntax: &SyntaxTrees,
            definition: &syntax_trees::item::ConstDefinition,
        ) -> Result<ConstInitializerDependencies, Vec<Diagnostic>> {
            self.declaration_dependencies(
                definition,
                DependencyPurpose::DeclarationReadiness,
                definition
                    .normalization
                    .as_ref()
                    .map(|receipt| receipt.declaration_custody.call_selections.as_slice()),
            )
        }

        pub fn selected_initializer_dependencies(
            &self,
            definition: &syntax_trees::item::ConstDefinition,
            selected_calls: &[(source::SourceSpan, source::SourceSpan)],
        ) -> Result<ConstInitializerDependencies, Vec<Diagnostic>> {
            self.declaration_dependencies(
                definition,
                DependencyPurpose::DeclarationReadiness,
                Some(selected_calls),
            )
        }

        pub fn initializer_authored_dependencies(
            &self,
            definition: &syntax_trees::item::ConstDefinition,
            selected_calls: Option<&[(source::SourceSpan, source::SourceSpan)]>,
        ) -> Result<ConstInitializerDependencies, Vec<Diagnostic>> {
            self.declaration_dependencies(definition, DependencyPurpose::Authored, selected_calls)
        }

        fn declaration_dependencies(
            &self,
            definition: &syntax_trees::item::ConstDefinition,
            purpose: DependencyPurpose,
            selected_calls: Option<&[(source::SourceSpan, source::SourceSpan)]>,
        ) -> Result<ConstInitializerDependencies, Vec<Diagnostic>> {
            let declaration = self
                .frontier
                .trees
                .const_declaration_at_source_span(definition.name.source_span())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "initializer dependencies lost their exact declaration",
                    )]
                })?;
            crate::constant::initializer_dependencies::collect_declaration(
                &self.frontier.trees,
                if declaration.authored_initializer.is_valid() {
                    declaration.authored_initializer
                } else {
                    declaration.initializer
                },
                &declaration.declared_type,
                purpose,
                self.frontier.selection.retained_base(),
                selected_calls,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(definition.name.source_span())]
            })
        }

        pub fn trees(&self) -> &SymbolResolvedTrees {
            &self.frontier.trees
        }

        /*
            Every declaration that needs initializer evaluation kept its exact
            declaration and can name its pending scalar leaves.
        */
        pub(crate) fn validate_initializer_leaves(
            &self,
            syntax: &SyntaxTrees,
        ) -> Result<(), Vec<Diagnostic>> {
            for definition in syntax.root_items().filter_map(|item| match item {
                syntax_trees::item::Item::Const(definition)
                    if crate::constant::requires_const_initializer_evaluation(
                        syntax, definition,
                    ) =>
                {
                    Some(definition)
                }
                _ => None,
            }) {
                self.frontier
                    .trees
                    .const_declaration_at_source_span(definition.name.source_span())
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "initializer preparation lost its exact declaration",
                        )]
                    })?;
                self.pending_leaves(syntax, definition)?;
            }
            Ok(())
        }

        pub fn pending_leaves(
            &self,
            syntax: &SyntaxTrees,
            definition: &syntax_trees::item::ConstDefinition,
        ) -> Result<Vec<crate::constant::PendingConstInitializerLeaf>, Vec<Diagnostic>> {
            pending_const_initializer_leaves(syntax, definition, &self.frontier.selection).map_err(
                |reason| {
                    vec![Diagnostic::error(reason).with_source_span(definition.name.source_span())]
                },
            )
        }

        /*
            Encode completed initializer payloads, without asserting the destination's
            qualification. The compile-time owner checks that separate result judgment
            before returning the authoritative declaration to normal resolution.
        */
        pub fn canonicalize_initializer_payload(
            &self,
            syntax: &SyntaxTrees,
            definition: &syntax_trees::item::ConstDefinition,
        ) -> Result<ConstInitializerPayload, String> {
            canonicalize_pending_initializer_payload(
                syntax,
                definition,
                Some(&self.frontier.selection),
            )
        }

        pub fn initializer_requires_destination_proof(
            &self,
            syntax: &SyntaxTrees,
            definition: &syntax_trees::item::ConstDefinition,
        ) -> bool {
            const_destination_requires_proof(syntax, definition, Some(&self.frontier.selection))
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct SeededSymbolResolvedTrees {
        pub(crate) trees: SymbolResolvedTrees,
        pub(crate) authored_selection_frontier: AuthoredSelectionExtensionFrontier,
        pub(crate) retained_base: Arc<SymbolResolvedTrees>,
    }

    /*
        A seeded resolved extension whose authored-selection suffix has been
        joined to the destination typed base ledger. The exact resolved base stays
        inside the carrier so the next phase can reject cross-paired continuations.
    */
    #[derive(Debug, PartialEq, Eq)]
    pub struct RebasedSeededSymbolResolvedTrees {
        trees: SymbolResolvedTrees,
        retained_base: Arc<SymbolResolvedTrees>,
    }

    impl SeededSymbolResolvedTrees {
        pub fn trees(&self) -> &SymbolResolvedTrees {
            &self.trees
        }

        pub fn rebase_authored_selections_for_typed_continuation(
            self,
            destination_base: &AuthoredDeclarationSelections,
        ) -> Result<RebasedSeededSymbolResolvedTrees, (Self, AuthoredSelectionExtensionRebaseError)>
        {
            match self.trees.rebase_authored_selection_extension(
                self.authored_selection_frontier,
                destination_base,
            ) {
                Ok(trees) => Ok(RebasedSeededSymbolResolvedTrees {
                    trees,
                    retained_base: self.retained_base,
                }),
                Err((trees, error)) => Err((Self { trees, ..self }, error)),
            }
        }
    }

    impl RebasedSeededSymbolResolvedTrees {
        pub fn trees(&self) -> &SymbolResolvedTrees {
            &self.trees
        }

        pub fn into_typing_continuation_parts(
            self,
        ) -> (SymbolResolvedTrees, Arc<SymbolResolvedTrees>) {
            (self.trees, self.retained_base)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::resolution::{ResolutionRequest, prepare_const_initializer_selection, resolve};
        use std::sync::Arc;

        fn parse(text: &str, source: source::SourceId) -> SyntaxTrees {
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap()
        }

        #[test]
        fn initializer_normalization_keeps_new_work_and_retained_call_dependencies() {
            let mut sources = source::SourceMap::default();
            let base_text = "public machine bound(value: u64) -> u64 { value }";
            let base_source = sources
                .add("initializers/base.omg".into(), base_text.into())
                .source_id;
            let base = Arc::new(
                resolve(ResolutionRequest {
                    syntax: parse(base_text, base_source),
                    sources: Some(Arc::new(sources.clone())),
                    top_level_bindings: Vec::new(),
                    retained_base: None,
                })
                .unwrap(),
            );
            let text = "machine local(value: u64) -> u64 { bound(value) }\nconst Count: u64 = local(3u64);";
            let source = sources
                .add("initializers/current.omg".into(), text.into())
                .source_id;
            let syntax = parse(text, source);
            let selected = prepare_const_initializer_selection(ResolutionRequest {
                syntax: syntax.clone(),
                sources: Some(Arc::new(sources)),
                top_level_bindings: Vec::new(),
                retained_base: Some(base),
            })
            .unwrap();
            let definition = syntax
                .root_items()
                .find_map(|item| match item {
                    syntax_trees::item::Item::Const(definition) => Some(definition),
                    _ => None,
                })
                .unwrap();
            let dependencies = selected
                .initializer_dependencies(&syntax, definition)
                .unwrap();
            assert_eq!(
                dependencies.calls.len(),
                2,
                "both helper calls remain obligations"
            );
            assert_eq!(dependencies.normalization_owners.len(), 1);
            assert_eq!(dependencies.normalization_owners[0].source_id, source);
            assert!(
                dependencies
                    .calls
                    .iter()
                    .any(|(_, declaration)| declaration.source_id == base_source)
            );
        }

        #[test]
        fn retained_constant_custody_cannot_supply_a_new_declarations_value() {
            let mut sources = source::SourceMap::default();
            let text = "public const Ceiling: u64 = 3u64;";
            let source = sources.add("inputs/base.omg".into(), text.into()).source_id;
            let base = Arc::new(
                resolve(ResolutionRequest {
                    syntax: parse(text, source),
                    sources: Some(Arc::new(sources.clone())),
                    top_level_bindings: Vec::new(),
                    retained_base: None,
                })
                .unwrap(),
            );
            let retained = base.const_declarations[0].symbol;
            let current_text = "const Fresh: u64 = Ceiling;";
            let current = sources
                .add("inputs/current.omg".into(), current_text.into())
                .source_id;
            let selected = prepare_const_initializer_selection(ResolutionRequest {
                syntax: parse(current_text, current),
                sources: Some(Arc::new(sources)),
                top_level_bindings: Vec::new(),
                retained_base: Some(base),
            })
            .unwrap();
            assert!(
                selected
                    .retained_constant(retained)
                    .unwrap()
                    .evaluated_value
                    .is_some()
            );
            for purpose in [
                crate::pre_resolution::InitializerCustodyPurpose::Authored,
                crate::pre_resolution::InitializerCustodyPurpose::Declaration,
                crate::pre_resolution::InitializerCustodyPurpose::Value,
            ] {
                let custody = selected
                    .retained_constant_custody(retained, purpose)
                    .unwrap();
                assert!(custody.selections.is_empty());
                assert!(custody.builtin_operators.is_empty());
                assert!(custody.call_selections.is_empty());
            }
            let fresh = selected
                .trees()
                .const_declarations
                .iter()
                .find(|declaration| {
                    selected
                        .trees()
                        .symbols
                        .symbol_source_span(declaration.symbol)
                        .is_some_and(|span| span.source_id == current)
                })
                .unwrap();
            assert!(selected.retained_constant(fresh.symbol).is_err());
        }

        #[test]
        fn implicit_call_identity_requires_the_exact_first_state() {
            let text = "machine helper(value: u64) -> u64 { value }";
            let mut sources = source::SourceMap::default();
            let source = sources.add("calls/main.omg".into(), text.into()).source_id;
            let mut program = resolve(ResolutionRequest {
                syntax: parse(text, source),
                sources: Some(Arc::new(sources)),
                top_level_bindings: Vec::new(),
                retained_base: None,
            })
            .unwrap();
            let machine = &program.machines[0];
            let machine_symbol = machine.symbol;
            let entry = program
                .machine_state(program.machine_state_handles(machine.states)[0])
                .symbol;
            assert!(program.symbols.symbol_source_span(entry).is_none());
            assert_eq!(
                crate::selection::call_identity::call_declaration_source(&program, entry).unwrap(),
                program.symbols.symbol_source_span(machine_symbol).unwrap()
            );
            let extra = program.symbols.append_children(
                machine_symbol,
                [(
                    symbols::SymbolKind::State,
                    symbols::SymbolNameRef::Static("not_entry"),
                )],
            );
            let extra = symbols::SymbolTableBuilder::child_handles(extra)
                .next()
                .unwrap();
            assert!(
                crate::selection::call_identity::call_declaration_source(&program, extra)
                    .unwrap_err()
                    .contains("not its exact owning entry")
            );
        }
    }
}
pub(crate) mod lowerer {
    /*
        The lowerer's working state.

        `Lowerer` carries the carrier under construction plus the pending sidecars
        each translator leaves for a later phase: authored expressions awaiting
        selection, constant declarations and selections, service-reach names,
        outcome-specific contracts, synthesized continuation states. `resolution`
        drives it; nothing here decides phase order. Seeding fills the same
        sidecars from a retained base so an extension resolves against it.
    */

    use crate::preparation::declaration_selection::ConstantSelection;
    use crate::selection::service_reaches::PendingAuthoredServiceReach;
    use crate::tables::resolved_declarations;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
    use source::{SourceMap, SourceSpan};
    use std::sync::Arc;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbol_resolved_trees::expression::ExpressionHandle;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::types::OperandLifetimeOwnerHandle;
    use symbol_resolved_trees::types::TypeReference;

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct PendingAuthoredProofMembership {
        pub(crate) fact: arena::Handle<symbol_resolved_trees::domain::ProofFact>,
        pub(crate) exposure: AuthoredDeclarationSelectionExposure,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct PendingAuthoredExpression {
        pub(crate) expression: ExpressionHandle,
        pub(crate) exposure: AuthoredDeclarationSelectionExposure,
    }

    /*
        An arm's lexical aliases can only read their own enclosing capture and
        exact structural head. This stack is construction scratch, not a name map.
    */
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct MatchArmCapture {
        pub(crate) syntax_capture: arena::Handle<syntax_trees::expression::MatchSubjectCapture>,
        pub(crate) syntax_head: syntax_trees::expression::ExpressionHandle,
        pub(crate) resolved_capture:
            arena::Handle<symbol_resolved_trees::expression::MatchSubjectCapture>,
        pub(crate) resolved_head: ExpressionHandle,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct PendingConstDeclaration {
        pub(crate) declaration: arena::Handle<symbol_resolved_trees::constant::ConstDeclaration>,
        pub(crate) scope: syntax_trees::identifier::Identifier,
        pub(crate) semantic_name: String,
        pub(crate) source_span: source::SourceSpan,
        pub(crate) is_public: bool,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct PendingConstSelection {
        pub(crate) expression: ExpressionHandle,
        pub(crate) source_span: source::SourceSpan,
        pub(crate) declaration: symbols::SymbolHandle,
        pub(crate) exposure: AuthoredDeclarationSelectionExposure,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct PendingConstArgumentSelection {
        pub(crate) origin: syntax_trees::types::ConstArgumentOrigin,
        /*
            Copying into a generated instance retains semantic custody without
            creating a new authored interface occurrence.
        */
        pub(crate) exposure: Option<AuthoredDeclarationSelectionExposure>,
        /*
            The selected declaration supplies the complete argument at this
            occurrence. Other retained origins are dependencies of its computation;
            their declared carriers do not become the receiving parameter's type.
        */
        pub(crate) is_argument_value: bool,
    }

    /*
        A use-site constant occurrence and its actual retained generic argument slot.
        This private link survives symbol allocation; encoded value labels never
        reconstruct which nominal binder received the selected declaration.
    */
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct PendingConstArgumentSlot {
        pub(crate) selection: usize,
        pub(crate) arguments: arena::HandleSpan<TypeReference>,
        pub(crate) ordinal: usize,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct PendingOutcomeSpecificContract {
        pub(crate) contract: arena::Handle<symbol_resolved_trees::signature::SignatureContract>,
        pub(crate) result_data_name: String,
        pub(crate) result_data_source_span: source::SourceSpan,
        pub(crate) result_case_name: String,
    }
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub(crate) enum ConstResolutionMode {
        #[default]
        Complete,
        ArgumentSelection,
        InitializerSelection,
    }
    pub(crate) struct Lowerer {
        pub(crate) defer_machine_bodies: bool,
        pub(crate) pending_machine_bodies: Vec<crate::lowering::machine::PendingMachineBody>,
        pub(crate) pending_callable_types:
            Vec<arena::Handle<symbol_resolved_trees::data::TypeParameter>>,
        pub(crate) pending_type_inputs: Vec<super::type_inputs::LoweredTypeInputs>,
        pub(crate) supplied_operand_owners:
            Vec<(syntax_trees::item::ItemHandle, OperandLifetimeOwnerHandle)>,
        pub(crate) declaration_inputs: Vec<(
            syntax_trees::item::ItemHandle,
            super::operand_owners::DeclarationInput,
        )>,
        pub(crate) encountered_pending_type_arguments:
            Vec<syntax_trees::types::TypeReferenceHandle>,
        pub(crate) equation_sources: Option<crate::preparation::machine_equations::SourceLinks>,
        pub(crate) const_resolution_mode: ConstResolutionMode,
        /*
            Constant values may be normalized before structural equations have
            their canonical range inputs. Only the complete route clears this.
        */
        pub(crate) structural_type_equations_pending: bool,
        pub(crate) constant_selection: Option<ConstantSelection<'static>>,
        pub(crate) namespace_declarations: crate::symbols::NamespaceDeclarations,
        pub(crate) pending_static_module_calls: Vec<(ExpressionHandle, Vec<DiagnosticName>)>,
        pub(crate) pending_static_module_statement_calls:
            Vec<(source::SourceSpan, Vec<DiagnosticName>)>,
        pub(crate) symbol_resolved_trees: SymbolResolvedTrees,
        /*
            Origin rows before this frontier already own exact declaration symbols.
            Body completion must bind its suffix without reinterpreting that prefix.
        */
        pub(crate) bound_generic_application_origins: usize,
        /*
            Authored machine `reaches` clauses retained until symbol assignment
            binds every member occurrence to its exact boundary-trait identity.
        */
        pub(crate) pending_machine_service_reaches: Vec<PendingAuthoredServiceReach>,
        pub(crate) pending_signature_service_reaches: Vec<PendingSignatureServiceReach>,
        /*
            Authored expressions whose exact declaration selections are collected
            after symbol assignment. Expressions absent from this list are either
            compiler-generated or belong to a source surface whose public/private
            disposition has not yet been classified by its owning declaration.
        */
        pub(crate) pending_authored_expressions: Vec<PendingAuthoredExpression>,
        pub(crate) pending_authored_proof_memberships: Vec<PendingAuthoredProofMembership>,
        /*
            Const values disappear during lowering, but their declaration identity
            must remain available to package-selection admission.
            Owners whose previously deferred bodies this producer completed.
        */
        pub(crate) completed_machines: Vec<symbols::SymbolHandle>,
        pub(crate) pending_const_declarations: Vec<PendingConstDeclaration>,
        pub(crate) pending_const_argument_selections: Vec<PendingConstArgumentSelection>,
        pub(crate) pending_const_argument_slots: Vec<PendingConstArgumentSlot>,
        /*
            Resolved declaration-construction owners from deferred lexical probes.
            Their Generic parents must outlive the pending const argument slots;
            they are never installed as executable statements in the resolved tree.
        */
        pub(crate) resolved_lexical_inputs: Vec<crate::lowering::state::StateLexicalInputs>,
        pub(crate) derived_const_argument_origins: Vec<syntax_trees::types::ConstArgumentOrigin>,
        pub(crate) derived_const_argument_expressions:
            Vec<syntax_trees::expression::ExpressionHandle>,
        pub(crate) pending_const_argument_expressions: Vec<ExpressionHandle>,
        pub(crate) derived_const_argument_builtin_operators: Vec<source::SourceSpan>,
        pub(crate) pending_const_selections: Vec<PendingConstSelection>,
        /*
            Newly authored initializer roots awaiting declaration-side resolution.
            Retained base roots are already resolved and must not enter this list.
        */
        pub(crate) pending_const_values: Vec<ExpressionHandle>,
        pub(crate) pending_const_initializers:
            Vec<crate::constant::initializer_normalization::PendingInitializer>,
        /*
            Outcome paths are validated against the declared result sum during
            lowering, then stamped with exact declaration symbols after the shared
            symbol-assignment pass has minted those handles.
        */
        pub(crate) pending_outcome_specific_contracts: Vec<PendingOutcomeSpecificContract>,
        pub(crate) current_authored_expression_exposure:
            Option<AuthoredDeclarationSelectionExposure>,
        /*
            Transient partition for one compiler-instantiated trait-default
            application. It separates copied authored selections that may resolve
            differently in distinct conformances and is cleared between machines.
        */
        pub(crate) current_compiler_selection_partition:
            Option<language_semantics::declaration_selection::CompilerDerivedSelectionPartition>,
        pub(crate) sources: Option<Arc<SourceMap>>,
        pub(crate) source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
        /*
            Per-lowering counter that mints unique names for synthetic `let`
            temporaries hoisted out of operands, calls and match subjects (see
            `statement::hoist_temps`). `__hoist_` prefixed so the generated names
            cannot collide with source identifiers.
        */
        pub(crate) hoist_counter: u32,
        /*
            Names of the CURRENT state's parameters declared as a shared reference
            to a NAMED type (`table: &EfiSystemTable`). A member read through one
            (`table.con_out`) must dereference the pointer slot; the flat fold reads
            frame garbage (the entry-ref-param face). The guard/operand hoists use
            this to materialize such reads into `let` temps, which lower through the
            boot-verified pointee path. Overwritten at each state's lowering.
        */
        pub(crate) reference_struct_parameters: Vec<String>,
        /*
            ALL of the current state's parameter names -- the computed-index hoist
            gate uses this to tell a typeable bare-Name operand (a param, whose
            declared type the typed layer resolves via `parameter_type`) from a
            LOCAL (untypeable at the hoist-temp layer; hoisting would mint a Unit
            temp and a confusing layout error). Overwritten at each state.
        */
        pub(crate) current_state_parameter_names: Vec<String>,
        /*
            Whether the machine currently being lowered is a BOUNDARY machine. Only
            boundary params are REAL pointer slots (the entry hand-off vouches
            them); a non-boundary `&Struct` param is a call-site ALIAS slot sharing
            the caller's storage, and materializing a pointee deref through one
            loads a non-pointer as an address (segfault -- probed 2026-07-04). The
            ref-param hoist is scoped to boundary machines.
        */
        pub(crate) current_machine_is_boundary: bool,
        /*
            Machine contract evidence names are a distinct erased namespace. They
            classify bare-name assignments before ordinary value resolution.
        */
        pub(crate) current_machine_root_index: Option<usize>,
        pub(crate) current_machine_name: Option<String>,
        pub(crate) current_state_name: Option<String>,
        pub(crate) current_evidence_term_names: Vec<String>,
        /*
            Maps a match SUBJECT syntax expression handle to the name and diagnostic
            source span of its single hoisted temp. All arms of one enum-variant match share the same
            syntax subject handle (the parser reuses it across arms), so the first
            arm mints `let __hoist_N = <subject>` and the siblings reuse the name --
            keeping ONE shared subject so match exhaustiveness still groups the arms
            (`statement::hoist_membership_match_subject`).
        */
        match_subject_temps: std::collections::HashMap<
            syntax_trees::expression::ExpressionHandle,
            (String, SourceSpan),
        >,
        pub(crate) expression_match_captures: Vec<MatchArmCapture>,
        /*
            The CURRENT state's parameters (name, resolved type,
            mutability). The scalar-computation classification of transition
            values reads them. Overwritten at each state.
        */
        pub(crate) current_state_parameters: Vec<(String, TypeReference, bool)>,
        /* The CURRENT state's declared return type. Overwritten at each state. */
        pub(crate) current_state_return_type: Option<TypeReference>,
        /* Present when this lowerer extends a retained base. */
        pub(crate) seed: Option<BaseSeed>,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct RootWatermarks {
        pub(crate) const_declarations: usize,
        pub(crate) data_definitions: usize,
        pub(crate) type_aliases: usize,
        pub(crate) domain_definitions: usize,
        pub(crate) machines: usize,
        pub(crate) operators: usize,
        pub(crate) measures: usize,
        pub(crate) propositions: usize,
        pub(crate) mathematical_definitions: usize,
        pub(crate) traits: usize,
        pub(crate) conformances: usize,
        pub(crate) wire_schemas: usize,
    }

    impl RootWatermarks {
        pub(crate) fn capture(program: &SymbolResolvedTrees) -> Self {
            Self {
                const_declarations: program.const_declarations.len(),
                data_definitions: program.data_definitions.len(),
                type_aliases: program.type_aliases.len(),
                domain_definitions: program.domain_definitions.len(),
                machines: program.machines.len(),
                operators: program.operators.len(),
                measures: program.measures.len(),
                propositions: program.propositions.len(),
                mathematical_definitions: program.mathematical_definitions.len(),
                traits: program.traits.len(),
                conformances: program.conformances.len(),
                wire_schemas: program.wire_schemas.len(),
            }
        }
    }
    /* What a seeded lowerer keeps from its base until finishing consumes it. */
    pub(crate) struct BaseSeed {
        pub(crate) roots: RootWatermarks,
        pub(crate) source: Arc<SymbolResolvedTrees>,
    }

    impl Lowerer {
        pub(crate) fn new(
            sources: Option<Arc<SourceMap>>,
            source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
        ) -> Self {
            Self {
                defer_machine_bodies: false,
                pending_machine_bodies: Vec::new(),
                pending_callable_types: Vec::new(),
                pending_type_inputs: Vec::new(),
                supplied_operand_owners: Vec::new(),
                declaration_inputs: Vec::new(),
                encountered_pending_type_arguments: Vec::new(),
                equation_sources: None,
                const_resolution_mode: ConstResolutionMode::Complete,
                structural_type_equations_pending: true,
                constant_selection: None,
                namespace_declarations: crate::symbols::NamespaceDeclarations::default(),
                pending_static_module_calls: Vec::new(),
                pending_static_module_statement_calls: Vec::new(),
                symbol_resolved_trees: SymbolResolvedTrees::default(),
                bound_generic_application_origins: 0,
                pending_machine_service_reaches: Vec::new(),
                pending_signature_service_reaches: Vec::new(),
                pending_authored_expressions: Vec::new(),
                pending_authored_proof_memberships: Vec::new(),
                completed_machines: Vec::new(),
                pending_const_declarations: Vec::new(),
                pending_const_selections: Vec::new(),
                pending_const_values: Vec::new(),
                pending_const_initializers: Vec::new(),
                pending_outcome_specific_contracts: Vec::new(),
                current_authored_expression_exposure: None,
                pending_const_argument_selections: Vec::new(),
                pending_const_argument_slots: Vec::new(),
                resolved_lexical_inputs: Vec::new(),
                derived_const_argument_origins: Vec::new(),
                derived_const_argument_expressions: Vec::new(),
                pending_const_argument_expressions: Vec::new(),
                derived_const_argument_builtin_operators: Vec::new(),
                current_compiler_selection_partition: None,
                sources,
                source_scoped_top_level_bindings,
                hoist_counter: 0,
                reference_struct_parameters: Vec::new(),
                current_state_parameter_names: Vec::new(),
                current_machine_is_boundary: false,
                current_machine_root_index: None,
                current_machine_name: None,
                current_state_name: None,
                current_evidence_term_names: Vec::new(),
                match_subject_temps: std::collections::HashMap::new(),
                expression_match_captures: Vec::new(),
                current_state_parameters: Vec::new(),
                current_state_return_type: None,
                seed: None,
            }
        }

        /*
            Extend a retained base: its source frontier must be the exact prefix of
            this lowerer's sources, and its declarations become the pending
            sidecars a later stratum resolves against.
        */
        pub(crate) fn seed_resolved_base(
            &mut self,
            base: Arc<SymbolResolvedTrees>,
        ) -> Result<(), Vec<Diagnostic>> {
            let Some(sources) = &self.sources else {
                return Err(vec![Diagnostic::error(
                    "seeded symbol resolution requires retained source custody",
                )]);
            };
            if base.symbols.source_map().is_some_and(|retained| {
                !sources.retains_source_storage(&retained)
                    || retained
                        .selected_files()
                        .any(|file| !sources.source_is_selected(file.source_id))
            }) {
                return Err(vec![Diagnostic::error(
                    "seeded symbol resolution source map does not retain the exact base frontier",
                )]);
            }
            self.seed = Some(BaseSeed {
                roots: RootWatermarks::capture(&base),
                source: Arc::clone(&base),
            });
            let mut authored_by_owner: std::collections::HashMap<
                symbols::SymbolHandle,
                &symbol_resolved_trees::signature::AuthoredServiceReachRow,
                symbols::BuildSymbolHasher,
            > = std::collections::HashMap::with_capacity_and_hasher(
                base.authored_service_reach_rows.len(),
                symbols::BuildSymbolHasher,
            );
            for row in &base.authored_service_reach_rows {
                authored_by_owner.entry(row.owner).or_insert(row);
            }
            let retained_reach =
                |owner: symbols::SymbolHandle, row: language_semantics::ServiceReachRowId| {
                    PendingAuthoredServiceReach::Resolved {
                        row,
                        authored: authored_by_owner.get(&owner).copied().cloned(),
                    }
                };
            self.pending_machine_service_reaches = base
                .machines
                .iter()
                .map(|machine| retained_reach(machine.symbol, machine.service_reach_row))
                .collect();
            for definition in &base.traits {
                for offset in 0..definition.machines.len() {
                    let handle = handle_at_offset(definition.machines, offset);
                    let signature = resolved_declarations(&base)
                        .trait_machine_signatures
                        .get(handle);
                    let reaches = retained_reach(signature.symbol, signature.service_reach_row);
                    self.pending_signature_service_reaches
                        .push(PendingSignatureServiceReach {
                            location: PendingSignatureLocation::Trait(handle),
                            owner: PendingSignatureOwner::Trait(definition.name.clone()),
                            reaches,
                        });
                }
            }
            for (handle, parameter) in base.tables.declarations.data_type_parameters.iter() {
                let TypeParameterKind::Machine { contract } = &parameter.kind else {
                    continue;
                };
                let Some(signature) = contract.structural() else {
                    continue;
                };
                let reaches = retained_reach(signature.symbol, signature.service_reach_row);
                self.pending_signature_service_reaches
                    .push(PendingSignatureServiceReach {
                        location: PendingSignatureLocation::MachineParameter(handle),
                        owner: PendingSignatureOwner::Requirement(parameter.name.clone()),
                        reaches,
                    });
            }
            self.bound_generic_application_origins =
                base.tables.types.generic_application_origins.len();
            self.symbol_resolved_trees = base.as_ref().clone();
            Ok(())
        }

        pub(crate) fn source_reference_can_see_declaration(
            &self,
            reference: source::SourceSpan,
            declaration: source::SourceSpan,
        ) -> bool {
            self.sources
                .as_deref()
                .is_none_or(|sources| sources.reference_can_see_declaration(reference, declaration))
        }

        pub(crate) fn source_resolution_strata_separate(
            &self,
            left: source::SourceSpan,
            right: source::SourceSpan,
        ) -> bool {
            self.sources
                .as_deref()
                .is_some_and(|sources| sources.resolution_strata_separate(left, right))
        }

        pub(crate) fn with_authored_expression_exposure<T>(
            &mut self,
            exposure: AuthoredDeclarationSelectionExposure,
            operation: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
        ) -> Result<T, Diagnostic> {
            let previous = self.current_authored_expression_exposure.replace(exposure);
            let result = operation(self);
            self.current_authored_expression_exposure = previous;
            result
        }

        /*
            The shared hoist-temp name and source occurrence, if the first arm
            already captured this exact match subject.
        */
        pub(crate) fn match_subject_temp(
            &self,
            subject: syntax_trees::expression::ExpressionHandle,
        ) -> Option<(String, SourceSpan)> {
            self.match_subject_temps.get(&subject).cloned()
        }

        pub(crate) fn record_match_subject_temp(
            &mut self,
            subject: syntax_trees::expression::ExpressionHandle,
            name: String,
            source_span: SourceSpan,
        ) {
            self.match_subject_temps
                .insert(subject, (name, source_span));
        }

        pub(crate) fn next_hoist_name(&mut self) -> String {
            let name = format!("__hoist_{}", self.hoist_counter);
            self.hoist_counter += 1;
            name
        }

        /* The constant selector lowering ran under, handed back before finishing. */
        pub(crate) fn take_constant_selection(
            &mut self,
        ) -> Result<ConstantSelection<'static>, Vec<Diagnostic>> {
            self.constant_selection.take().ok_or_else(|| {
                vec![Diagnostic::error(
                    "constant preparation lost its source-aware selector",
                )]
            })
        }

        /*
            The declaration arena owning every lowered child type reference. Callers
            reach it by name instead of respelling the four-hop field path.
        */
        pub(crate) fn declaration_child_type_references(
            &mut self,
        ) -> &mut arena::Arena<TypeReference> {
            &mut self
                .symbol_resolved_trees
                .tables
                .declarations
                .child_type_references
        }

        /*
            The finished trees: tables rebuilt from the lowered roots, with the
            interned semantic rows and domains built during lowering kept.
        */
        pub(crate) fn into_trees(self) -> SymbolResolvedTrees {
            let SymbolResolvedTrees {
                roots,
                tables,
                symbols,
                service_reaches,
                service_reach_rows,
                authored_service_reach_rows,
                semantic_domains,
                external_bindings,
                evidence_forwardings,
                resolution_indexes: _,
            } = self.symbol_resolved_trees;
            let mut trees = SymbolResolvedTrees::with_roots(roots, tables, symbols);
            trees.service_reaches = service_reaches;
            trees.service_reach_rows = service_reach_rows;
            trees.authored_service_reach_rows = authored_service_reach_rows;
            trees.semantic_domains = semantic_domains;
            trees.external_bindings = external_bindings;
            trees.evidence_forwardings = evidence_forwardings;
            trees
        }

        /*
            Pair every root machine and pending signature with its authored
            service-reach names once their symbols exist.
        */
        pub(crate) fn pending_service_reaches(
            &self,
        ) -> Result<
            (
                Vec<(symbols::SymbolHandle, PendingAuthoredServiceReach)>,
                Vec<crate::selection::service_reaches::PendingSignatureServiceReach>,
            ),
            Vec<Diagnostic>,
        > {
            assert_eq!(
                self.symbol_resolved_trees.machines.len(),
                self.pending_machine_service_reaches.len(),
                "each initially resolved root machine has one pending authored service row"
            );
            let pending_machine_service_reaches = self
                .symbol_resolved_trees
                .machines
                .iter()
                .zip(&self.pending_machine_service_reaches)
                .map(|(machine, reaches)| (machine.symbol, reaches.clone()))
                .collect::<Vec<_>>();
            let pending_signature_service_reaches = self
            .pending_signature_service_reaches
            .iter()
            .map(|pending| {
                let symbol = match pending.location {
                    PendingSignatureLocation::Trait(handle) => {
                        resolved_declarations(&self.symbol_resolved_trees).trait_machine_signatures
                            .get(handle)
                            .symbol
                    }
                    PendingSignatureLocation::MachineParameter(handle) => {
                        let parameter = resolved_declarations(&self.symbol_resolved_trees).data_type_parameters
                            .get(handle);
                        let TypeParameterKind::Machine { contract } = &parameter.kind else {
                            return Err(vec![Diagnostic::error("pending service reach no longer names a machine parameter")]);
                        };
                        contract.structural().ok_or_else(|| vec![Diagnostic::error("pending service reach lost its structural machine-parameter signature")])?.symbol
                    }
                };
                Ok(crate::selection::service_reaches::PendingSignatureServiceReach {
                    symbol,
                    owner: pending.owner.clone(),
                    reaches: pending.reaches.clone(),
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
            Ok((
                pending_machine_service_reaches,
                pending_signature_service_reaches,
            ))
        }
    }

    fn handle_at_offset<T>(span: arena::HandleSpan<T>, offset: usize) -> arena::Handle<T> {
        arena::Handle::from_parts(
            span.start()
                .arena_index()
                .checked_add(u32::try_from(offset).expect("handle-span offset fits u32"))
                .expect("handle-span offset overflow"),
            span.start().generation(),
        )
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum PendingSignatureLocation {
        Trait(arena::Handle<symbol_resolved_trees::signature::StateSignature>),
        MachineParameter(arena::Handle<symbol_resolved_trees::data::TypeParameter>),
    }

    #[derive(Debug, Clone)]
    pub(crate) enum PendingSignatureOwner {
        Trait(DiagnosticName),
        Requirement(DiagnosticName),
    }

    #[derive(Debug, Clone)]
    pub(crate) struct PendingSignatureServiceReach {
        pub(crate) location: PendingSignatureLocation,
        pub(crate) owner: PendingSignatureOwner,
        pub(crate) reaches: PendingAuthoredServiceReach,
    }
}
pub(crate) mod operand_owners {
    /*
        Translate copied operand custody at the declaration-allocation boundary.
        Syntax roots identify supplying declarations before symbols exist. Each
        produced trait or conformance records that input and its allocation coordinate;
        no name, source span or matching type shape reconstructs the association.
    */

    use super::lowerer::Lowerer;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::types::{OperandLifetimeOwner, OperandLifetimeOwnerHandle};
    use syntax_trees::item::ItemHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub(crate) enum DeclarationInput {
        Conformance(usize),
        Trait(usize),
    }

    impl DeclarationInput {
        fn symbol(
            self,
            program: &symbol_resolved_trees::SymbolResolvedTrees,
        ) -> symbols::SymbolHandle {
            match self {
                Self::Conformance(ordinal) => program.conformances[ordinal].symbol,
                Self::Trait(ordinal) => program.traits[ordinal].symbol,
            }
        }
    }

    pub(crate) fn retain(lowerer: &mut Lowerer, input: ItemHandle) -> OperandLifetimeOwnerHandle {
        if !input.is_valid() {
            return OperandLifetimeOwnerHandle::invalid();
        }
        if let Some((_, owner)) = lowerer
            .supplied_operand_owners
            .iter()
            .find(|(known, _)| *known == input)
        {
            return *owner;
        }
        let declaration = lowerer
            .declaration_inputs
            .iter()
            .find(|(known, _)| *known == input)
            .map(|(_, declaration)| declaration.symbol(&lowerer.symbol_resolved_trees))
            .unwrap_or_default();
        let owner = lowerer
            .symbol_resolved_trees
            .tables
            .types
            .operand_lifetime_owners
            .insert(OperandLifetimeOwner { declaration });
        lowerer.supplied_operand_owners.push((input, owner));
        owner
    }

    /*
        The allocation bridge is exact input custody, prepared before name walks.
        It carries no inferred owner or checked admission.
    */
    pub(crate) fn bindings(
        lowerer: &Lowerer,
    ) -> Result<Vec<(OperandLifetimeOwnerHandle, DeclarationInput)>, Vec<Diagnostic>> {
        lowerer
            .supplied_operand_owners
            .iter()
            .map(|(input, owner)| {
                let (_, ordinal) = lowerer
                    .declaration_inputs
                    .iter()
                    .find(|(known, _)| known == input)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "supplied operand lost its exact declaration input",
                        )]
                    })?;
                Ok((*owner, *ordinal))
            })
            .collect()
    }

    /*
        Declaration allocators already own these symbols. Publish that existing
        input-to-declaration bridge before resolving any copied value expression.
    */
    pub(crate) fn bind_program(
        program: &mut symbol_resolved_trees::SymbolResolvedTrees,
        bindings: &[(OperandLifetimeOwnerHandle, DeclarationInput)],
    ) -> Result<(), Vec<Diagnostic>> {
        for &(owner, ordinal) in bindings {
            let declaration = ordinal.symbol(program);
            if !declaration.is_valid() {
                return Err(vec![Diagnostic::error(
                    "supplied operand has an unbound declaration owner",
                )]);
            }
            program
                .tables
                .types
                .operand_lifetime_owners
                .get_mut(owner)
                .declaration = declaration;
        }
        Ok(())
    }
}
pub(crate) mod type_inputs {
    /*
        Probe input ownership captured at root production, before symbols exist.

        Allocation ranges bind a syntax use to this frontend's declarations after
        indexing. No source spelling, span containment, or another frontend's symbol
        ordinal participates. The frontier transfers these temporary dependencies to
        its typed probe consumer; ordinary resolved trees retain no probe metadata.
    */

    use super::lowerer::RootWatermarks;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::SymbolHandle;
    use syntax_trees::types::TypeReferenceHandle;

    pub(crate) struct LoweredTypeInputs {
        pub(crate) start: RootWatermarks,
        pub(crate) end: RootWatermarks,
        pub(crate) arguments: Vec<TypeReferenceHandle>,
        pub(crate) owner: SymbolHandle,
    }

    /*
        The ordinary type lowerer records actual input uses. Both declaration
        production and deferred body completion close the same ownership scope.
    */
    pub(crate) struct TypeInputScope {
        roots: RootWatermarks,
        argument_frontier: usize,
    }

    impl TypeInputScope {
        pub(crate) fn new(lowerer: &super::lowerer::Lowerer) -> Self {
            Self {
                roots: RootWatermarks::capture(&lowerer.symbol_resolved_trees),
                argument_frontier: lowerer.encountered_pending_type_arguments.len(),
            }
        }

        pub(crate) fn finish(self, lowerer: &mut super::lowerer::Lowerer, owner: SymbolHandle) {
            let mut arguments =
                lowerer.encountered_pending_type_arguments[self.argument_frontier..].to_vec();
            arguments
                .sort_unstable_by_key(|argument| (argument.generation(), argument.arena_index()));
            arguments.dedup();
            if !arguments.is_empty() {
                lowerer.pending_type_inputs.push(LoweredTypeInputs {
                    start: self.roots,
                    end: RootWatermarks::capture(&lowerer.symbol_resolved_trees),
                    arguments,
                    owner,
                });
            }
        }
    }

    pub(crate) fn bind(
        trees: &SymbolResolvedTrees,
        inputs: &[LoweredTypeInputs],
    ) -> Result<Vec<(SymbolHandle, TypeReferenceHandle)>, Vec<diagnostics::Diagnostic>> {
        let mut result = Vec::new();
        for input in inputs {
            let start = result.len();
            if input.owner.is_valid() {
                result.extend(
                    input
                        .arguments
                        .iter()
                        .map(|argument| (input.owner, *argument)),
                );
                continue;
            }
            macro_rules! bind_roots {
            ($($field:ident),* $(,)?) => {$({
                for ordinal in input.start.$field..input.end.$field {
                    let symbol = trees.$field[ordinal].symbol;
                    if !symbol.is_valid() {
                        return Err(vec![diagnostics::Diagnostic::error("pending type input has an unbound declaration owner")]);
                    }
                    result.extend(input.arguments.iter().map(|argument| (symbol, *argument)));
                }
            })*};
        }
            bind_roots!(
                const_declarations,
                data_definitions,
                type_aliases,
                domain_definitions,
                machines,
                operators,
                measures,
                propositions,
                mathematical_definitions,
                traits,
                conformances
            );
            if result.len() == start {
                return Err(vec![diagnostics::Diagnostic::error(
                    "pending type input lost its produced declaration owner",
                )]);
            }
        }
        Ok(result)
    }
}

use crate::lowering::state::LexicalInputDemand;
use crate::preparation::declaration_selection::ConstantSelection;
use crate::symbols::assign_lexical_inputs;
use crate::symbols::target_siblings::bind_sibling_calls;
use crate::symbols::type_predicates::retain_operands;
use crate::tables::body_expressions;
use crate::{constant, lowering, preparation, selection};
pub use body_completion::{
    DeferredResolvedFrontier, RebasedResolvedBodyCompletion, ResolvedBodyCompletion,
};
pub use continuations::{
    ConstDestinationObligation, ConstInitializerPayload, ConstInitializerSelection,
    RebasedSeededSymbolResolvedTrees, SeededSymbolResolvedTrees,
};
use diagnostics::Diagnostic;
use lowerer::{ConstResolutionMode, Lowerer};
use lowering::machine::mark_token_bound_domain_homes;
use lowering::machine::reject_duplicate_direct_token_bindings;
use lowering::mathematical::lower_mathematical_definitions;
use preparation::generic_data::const_destination_requires_proof;
use preparation::generic_data::domain_heads::normalize;
use preparation::machine_equations::SourceLinks;
use preparation::machine_equations::complete;
use preparation::machine_equations::required;
use preparation::machine_equations::validate_declarations;
use preparation::module_normalization::validate_with_const_resolution_mode;
use preparation::trait_defaults::synthesize_trait_defaults_after_module_validation;
use selection::body_calls::CallSelectionInput;
use selection::conformance_blocks::route_conformance_requirement_calls;
use selection::conformance_blocks::route_named_implementation_calls;
use source::SourceMap;
use std::mem::take;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionHandle;
use symbol_resolved_trees::expression::ExpressionNode;
use symbols::SymbolHandle;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::ConstDefinition;

/*
    One syntax forest and the custody it resolves under.

    The forest arrives by value, as `GenericDataRequest`'s does: preparation
    rewrites syntax to syntax before any symbol exists, and only the rewritten
    forest carries the declarations the symbol table assigns identities to.
    Taking ownership keeps that rewrite from being paid for with a copy of the
    whole program and keeps orchestration code from retaining the unprepared
    sibling as if a later stage could still read it.
*/
pub struct ResolutionRequest {
    pub syntax: SyntaxTrees,
    /*
        Source custody for visibility and stratum checks. A source-free forest
        has none, and those checks stay permissive for it.
    */
    pub sources: Option<Arc<SourceMap>>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    /*
        Immutable declarations selected by this source activation. Early probes
        and complete resolution use the same namespace and executable bodies;
        retaining source coordinates alone cannot supply either.
    */
    pub retained_base: Option<Arc<SymbolResolvedTrees>>,
}

impl ResolutionRequest {
    /* A source-free forest with no scoped top-level bindings. */
    pub fn new(syntax: SyntaxTrees) -> Self {
        Self {
            syntax,
            sources: None,
            top_level_bindings: Vec::new(),
            retained_base: None,
        }
    }
}

/* A later-stratum forest appended to an exact retained base. */
pub struct ExtensionRequest {
    /*
        Shared with the typed base that owns it; the extension reads it and
        never needs a private copy of the whole predecessor.
    */
    pub base: Arc<SymbolResolvedTrees>,
    pub syntax: SyntaxTrees,
    /* Must retain the base's exact source frontier. */
    pub sources: Arc<SourceMap>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
}

/* Resolve every name in the forest to its exact declaration. */
pub fn resolve(request: ResolutionRequest) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let (syntax, lowerer) = begin_complete(request)?;
    drive(lowerer, &syntax).map(|(trees, _)| trees)
}

/*
    Append authored declarations to an already activated source base. Their
    implementations remain pending, exactly as in a fresh source request.
*/
pub fn resolve_deferred_extension(
    request: ExtensionRequest,
) -> Result<DeferredResolvedExtension, Vec<Diagnostic>> {
    let ExtensionRequest {
        base,
        syntax,
        sources,
        top_level_bindings,
    } = request;
    let predecessor = base;
    let successor = resolve_deferred(ResolutionRequest {
        syntax,
        sources: Some(sources),
        top_level_bindings,
        retained_base: Some(Arc::clone(&predecessor)),
    })?;
    Ok(DeferredResolvedExtension {
        successor,
        predecessor,
    })
}

/*
    Index declarations without granting their implementations execution. The
    returned owner retains the exact prepared source for subsequent body demand.
*/
pub fn resolve_deferred(
    request: ResolutionRequest,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    let base = request.retained_base.clone();
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (mut syntax, mut lowerer) = begin(request, ConstResolutionMode::Complete)?;
    if required(&syntax, base.as_deref()) {
        loop {
            let mut provisional = Lowerer::new(sources.clone(), bindings.clone());
            provisional.constant_selection = lowerer.constant_selection.clone();
            provisional.const_resolution_mode = ConstResolutionMode::ArgumentSelection;
            provisional.equation_sources = Some(Default::default());
            if let Some(base) = &base {
                provisional.seed_resolved_base(Arc::clone(base))?;
            }
            index_declarations(&mut provisional, &syntax)?;
            finish_resolution(&mut provisional, &syntax, &[], &[])?;
            let selection = provisional.take_constant_selection()?;
            let links = provisional.equation_sources.take().unwrap_or_default();
            let before = syntax.root_item_handles().len();
            complete(
                &mut syntax,
                &provisional.symbol_resolved_trees,
                &links,
                &selection,
            )?;
            if syntax.root_item_handles().len() == before {
                break;
            }
        }
        lowerer.equation_sources = Some(Default::default());
    }
    lowerer.structural_type_equations_pending = false;
    index_deferred(syntax, lowerer, sources, bindings)
}

/* New deferred syntax and its exact resolved predecessor travel together. */
pub struct DeferredResolvedExtension {
    successor: DeferredResolvedFrontier,
    predecessor: Arc<SymbolResolvedTrees>,
}

impl DeferredResolvedExtension {
    /*
        Join the typed predecessor's selection suffix once, on both the
        declaration input and the frontier which will later complete its bodies.
    */
    pub fn rebase_for_typed_continuation(
        mut self,
        selections: &symbol_resolved_trees::AuthoredDeclarationSelections,
    ) -> Result<(RebasedSeededSymbolResolvedTrees, DeferredResolvedFrontier), Vec<Diagnostic>> {
        let source = SeededSymbolResolvedTrees {
            trees: Arc::unwrap_or_clone(std::mem::take(&mut self.successor.trees)),
            authored_selection_frontier: self.predecessor.authored_selection_extension_frontier(),
            retained_base: self.predecessor,
        }
        .rebase_authored_selections_for_typed_continuation(selections)
        .map_err(|(_, reason)| {
            vec![Diagnostic::error(format!(
                "authored declaration extension lost its typed selection prefix: {reason:?}"
            ))]
        })?;
        self.successor.trees = Arc::new(source.trees().clone());
        Ok((source, self.successor))
    }
}

/*
    Normalize numeric-probe declarations while retaining equation obligations
    and original implementations. The caller demands bodies by exact owner.
*/
pub fn resolve_deferred_numeric_probe(
    request: ResolutionRequest,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (syntax, lowerer) = begin(request, ConstResolutionMode::Complete)?;
    index_deferred(syntax, lowerer, sources, bindings)
}

fn index_deferred(
    syntax: SyntaxTrees,
    mut lowerer: Lowerer,
    sources: Option<Arc<SourceMap>>,
    bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    index_declarations(&mut lowerer, &syntax)?;
    finish_resolution(&mut lowerer, &syntax, &[], &[])?;
    let pending = take(&mut lowerer.pending_machine_bodies);
    let pending_type_inputs =
        type_inputs::bind(&lowerer.symbol_resolved_trees, &lowerer.pending_type_inputs)?;
    let selection = lowerer.take_constant_selection()?;
    let hoist_counter = lowerer.hoist_counter;
    let const_resolution_mode = lowerer.const_resolution_mode;
    let structural_type_equations_pending = lowerer.structural_type_equations_pending;
    let equation_sources = lowerer.equation_sources.take().map(|mut sources| {
        sources.expressions.clear();
        sources.statements.clear();
        sources
    });
    let supplied_operand_owners = take(&mut lowerer.supplied_operand_owners);
    let declaration_inputs = take(&mut lowerer.declaration_inputs);
    Ok(DeferredResolvedFrontier {
        syntax: Arc::new(syntax),
        trees: Arc::new(lowerer.into_trees()),
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
    })
}

fn index_declarations(lowerer: &mut Lowerer, syntax: &SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    lowerer.defer_machine_bodies = true;
    lowering::lower_items(lowerer, syntax)?;
    lower_mathematical_definitions(lowerer, syntax)?;
    selection::select_operator_homes(lowerer)?;
    crate::symbols::assign_declarations(lowerer)
}

/*
    One lexical selection drive beside its complete retained lookup catalog.
    Predecessor expressions are lookup history, not fresh selections in this
    drive. In particular, local type operands live outside deferred runtime
    bodies, so a traversal of executable roots cannot recover this inventory.
    The resolution producer retains its classified authored expressions instead.
    This product grants neither body typing nor execution authority.
*/
pub struct ConstArgumentSelection {
    program: SymbolResolvedTrees,
    expressions: Vec<ExpressionHandle>,
}

impl ConstArgumentSelection {
    pub fn original_call_target(&self, expression: ExpressionHandle) -> Option<SymbolHandle> {
        if !self.expressions.contains(&expression) {
            return None;
        }
        match self
            .program
            .tables
            .bodies
            .expressions
            .expression(expression)
        {
            ExpressionNode::Call(call) => {
                call.authored_call_binding.map(|binding| binding.original)
            }
            _ => None,
        }
    }

    pub fn program(&self) -> &SymbolResolvedTrees {
        &self.program
    }

    pub fn expressions(&self) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
        self.expressions.iter().copied().map(|expression| {
            (
                expression,
                body_expressions(&self.program).expression(expression),
            )
        })
    }
}

/*
    Resolve raw index expressions in their authored owners through normal
    lexical selection. This performs no generic instance synthesis or evaluation;
    callers must still admit every selected leaf and checked operator meaning.
    Local headers and nested type inputs borrow their original preceding-local
    scope; runtime bodies remain Deferred. This is a one-shot selection carrier,
    not a continuation whose declaration scratch can be executed or completed.
*/
pub fn resolve_const_argument_selection(
    request: ResolutionRequest,
) -> Result<ConstArgumentSelection, Vec<Diagnostic>> {
    let (syntax, mut lowerer) = begin(request, ConstResolutionMode::ArgumentSelection)?;
    index_declarations(&mut lowerer, &syntax)?;
    assign_lexical_inputs(&mut lowerer, &syntax, LexicalInputDemand::TypeOperands)?;
    let expressions: Vec<_> = lowerer
        .pending_authored_expressions
        .iter()
        .map(|authored| authored.expression)
        .collect();
    finish_resolution(&mut lowerer, &syntax, &[], &[])?;
    let program = lowerer.into_trees();
    Ok(ConstArgumentSelection {
        program,
        expressions,
    })
}

/*
    Normalize ordinary constants for a numeric probe while retaining machine
    equations as pending execution obligations. Range normalization supplies
    inputs to equation completion, so these two phase axes are independent.
*/
pub fn resolve_numeric_probe(
    request: ResolutionRequest,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let (syntax, lowerer) = begin(request, ConstResolutionMode::Complete)?;
    drive(lowerer, &syntax).map(|(trees, _)| trees)
}

/*
    Retain unfinished constant values while resolving a declaration-check probe.
    No pending initializer becomes a substituted value or public identity. The
    consumer demands only its exact completed probe roots; their ordinary typed
    checking cannot read an unavailable initializer as a completed constant.
*/
pub fn resolve_deferred_initializer_probe(
    request: ResolutionRequest,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (syntax, lowerer) = begin(request, ConstResolutionMode::InitializerSelection)?;
    index_deferred(syntax, lowerer, sources, bindings)
}

/*
    Resolve declaration dependencies without inventing provisional values.
    Computed fixed-integer/Boolean initializers remain authored expression roots
    with no canonical encoding. Every operator occurrence remains an obligation
    for ordinary typed selection, not an assertion of builtin execution meaning.
    Only helpers reached from those roots acquire bodies. Dependency collection
    still requires every reached implementation to exist; deferred unrelated
    machines cannot affect preparation or become executable through this carrier.
*/
pub fn prepare_const_initializer_selection(
    request: ResolutionRequest,
) -> Result<ConstInitializerSelection, Vec<Diagnostic>> {
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (syntax, lowerer) = begin(request, ConstResolutionMode::InitializerSelection)?;
    finish_const_initializer_selection(syntax, lowerer, sources, bindings)
}

/*
    Discover destination judgments before demanding their declaration graph.
    These are the original destinations, before the private forest deferred their
    qualifications. Selection and leaf classification only schedule work: every
    selected destination still returns through ordinary typed checking.
*/
pub fn prepare_const_qualification_selection<'authored>(
    request: ResolutionRequest,
    authored: &'authored SyntaxTrees,
    destinations: impl IntoIterator<Item = &'authored ConstDefinition>,
) -> Result<Option<ConstInitializerSelection>, Vec<Diagnostic>> {
    let destinations = destinations.into_iter().collect::<Vec<_>>();
    if destinations.is_empty() {
        return Ok(None);
    }
    let ResolutionRequest {
        mut syntax,
        sources,
        top_level_bindings,
        retained_base,
    } = request;
    preparation::target_families::normalize(&mut syntax, sources.as_deref())?;
    let constant_selection =
        preparation::declaration_selection::ConstantSelection::with_shared_base(
            &syntax,
            sources.clone(),
            top_level_bindings.clone(),
            retained_base.clone(),
        )?;
    normalize(&mut syntax, Some(&constant_selection))?;
    let mut demanded = false;
    for definition in destinations {
        if (definition.pending_qualification.is_some()
            || const_destination_requires_proof(authored, definition, Some(&constant_selection)))
            && constant::pending_const_initializer_leaves(authored, definition, &constant_selection)
                .map_err(|reason| {
                    vec![Diagnostic::error(reason).with_source_span(definition.name.source_span())]
                })?
                .is_empty()
        {
            demanded = true;
            break;
        }
    }
    if !demanded {
        return Ok(None);
    }
    validate_with_const_resolution_mode(
        &syntax,
        &constant_selection,
        ConstResolutionMode::InitializerSelection,
    )?;
    synthesize_trait_defaults_after_module_validation(&mut syntax, &constant_selection)?;
    validate_declarations(&syntax, Some(&constant_selection))?;
    let mut lowerer = Lowerer::new(sources.clone(), top_level_bindings.clone());
    lowerer.constant_selection = Some(constant_selection);
    lowerer.const_resolution_mode = ConstResolutionMode::InitializerSelection;
    if let Some(base) = retained_base {
        lowerer.seed_resolved_base(base)?;
    }
    finish_const_initializer_selection(syntax, lowerer, sources, top_level_bindings).map(Some)
}

fn finish_const_initializer_selection(
    syntax: SyntaxTrees,
    lowerer: Lowerer,
    sources: Option<Arc<SourceMap>>,
    bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<ConstInitializerSelection, Vec<Diagnostic>> {
    let mut frontier = index_deferred(syntax, lowerer, sources, bindings)?;
    let initializers = frontier
        .trees()
        .const_declarations
        .iter()
        .map(|declaration| {
            if declaration.authored_initializer.is_valid() {
                declaration.authored_initializer
            } else {
                declaration.initializer
            }
        })
        .filter(|initializer| initializer.is_valid())
        .collect::<Vec<_>>();
    loop {
        let demand = frontier.body_demand(&[], &initializers)?;
        if demand.deferred_machines.is_empty() {
            break;
        }
        frontier = frontier
            .complete_machine_bodies(&demand.deferred_machines)?
            .into_resolved_frontier();
    }
    let preparation = ConstInitializerSelection { frontier };
    preparation.validate_initializer_leaves(&preparation.frontier.syntax)?;
    Ok(preparation)
}

/*
    Resolve one syntax extension against its retained base while keeping the
    exact append frontier of every authored-selection occurrence store.

    Existing arenas and symbol tables are consumed and extended in place; no
    source bytes are read and neither forest is parsed again. The returned
    carrier is readable, but its trees can enter a later seeded phase only by
    transactionally rebasing the extension suffix against that phase's exact
    retained authored-selection ledger.
*/
pub fn resolve_extension(
    request: ExtensionRequest,
) -> Result<SeededSymbolResolvedTrees, Vec<Diagnostic>> {
    let ExtensionRequest {
        base,
        syntax,
        sources,
        top_level_bindings,
    } = request;
    let authored_selection_frontier = base.authored_selection_extension_frontier();
    let retained_base = Arc::clone(&base);
    let request = ResolutionRequest {
        syntax,
        sources: Some(sources),
        top_level_bindings,
        retained_base: Some(base),
    };
    let (syntax, lowerer) = begin_complete(request)?;
    let (trees, _) = drive(lowerer, &syntax)?;
    Ok(SeededSymbolResolvedTrees {
        trees,
        authored_selection_frontier,
        retained_base,
    })
}

/*
    Equation completion uses ordinary selection with identical source and base
    custody. Its private links are discarded before the final lowering pass.
*/
fn begin_complete(request: ResolutionRequest) -> Result<(SyntaxTrees, Lowerer), Vec<Diagnostic>> {
    let base = request.retained_base.clone();
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (mut syntax, mut lowerer) = begin(request, ConstResolutionMode::Complete)?;
    if required(&syntax, base.as_deref()) {
        let mut provisional = Lowerer::new(sources, bindings);
        provisional.constant_selection = lowerer.constant_selection.take();
        provisional.const_resolution_mode = ConstResolutionMode::ArgumentSelection;
        provisional.equation_sources = Some(Default::default());
        if let Some(base) = &base {
            provisional.seed_resolved_base(Arc::clone(base))?;
        }
        let (trees, selection, links) = drive_recorded(provisional, &syntax)?;
        complete(&mut syntax, &trees, &links.unwrap_or_default(), &selection)?;
        lowerer.constant_selection = Some(selection);
    }
    lowerer.structural_type_equations_pending = false;
    Ok((syntax, lowerer))
}

/*
    Prepare the forest under its constant selector and a lowerer that owns the
    request's custody.
*/
fn begin(
    request: ResolutionRequest,
    constants: ConstResolutionMode,
) -> Result<(SyntaxTrees, Lowerer), Vec<Diagnostic>> {
    let ResolutionRequest {
        syntax,
        sources,
        top_level_bindings,
        retained_base,
    } = request;
    let prepared = preparation::prepare(
        syntax,
        sources.clone(),
        top_level_bindings.clone(),
        constants,
        retained_base.clone(),
    )?;
    validate_declarations(&prepared.syntax, Some(&prepared.constant_selection))?;
    let mut lowerer = Lowerer::new(sources, top_level_bindings);
    lowerer.constant_selection = Some(prepared.constant_selection);
    lowerer.const_resolution_mode = constants;
    if let Some(base) = retained_base {
        lowerer.seed_resolved_base(base)?;
    }
    Ok((prepared.syntax, lowerer))
}

/*
    The route. Each phase is one call into its owner, in the one order their
    preconditions allow: operator homes need no symbols; constants need the
    table; the authored-selection ledger needs substituted constants; operator
    obligations need the ledger; every remaining selection needs all of it;
    domain homes of token-bearing machines need assigned attached symbols;
    duplicate machine token bindings compare settled operand identities.
*/
fn drive(
    lowerer: Lowerer,
    syntax: &SyntaxTrees,
) -> Result<(SymbolResolvedTrees, ConstantSelection<'static>), Vec<Diagnostic>> {
    drive_recorded(lowerer, syntax).map(|(trees, selection, _)| (trees, selection))
}

fn drive_recorded(
    mut lowerer: Lowerer,
    syntax: &SyntaxTrees,
) -> Result<
    (
        SymbolResolvedTrees,
        ConstantSelection<'static>,
        Option<SourceLinks>,
    ),
    Vec<Diagnostic>,
> {
    lowering::lower_items(&mut lowerer, syntax)?;
    lower_mathematical_definitions(&mut lowerer, syntax)?;
    selection::select_operator_homes(&mut lowerer)?;
    let constant_selection = lowerer.take_constant_selection()?;
    crate::symbols::assign(&mut lowerer)?;
    finish_resolution(&mut lowerer, syntax, &[], &[])?;
    let links = lowerer.equation_sources.take();
    Ok((lowerer.into_trees(), constant_selection, links))
}

/*
    One normalization route for new declarations and completed implementations.
    Source activation can shrink roots while retaining backing coordinates;
    rebuilding those tables would move the typed continuation's ledger frontier.
    Exact completed owners append their newly available bodies beside that prefix.
*/
fn finish_resolution(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    inputs: &[CallSelectionInput],
    completed_machines: &[SymbolHandle],
) -> Result<(), Vec<Diagnostic>> {
    let predecessor = lowerer.seed.as_ref().map(|seed| Arc::clone(&seed.source));
    settle_resolution(lowerer, syntax)?;
    let mut completed_machines = completed_machines.to_vec();
    completed_machines.extend_from_slice(&lowerer.completed_machines);
    let symbol_key = |symbol: SymbolHandle| (symbol.arena_index(), symbol.generation());
    let mut completed_keys = completed_machines
        .iter()
        .map(|symbol| symbol_key(*symbol))
        .collect::<Vec<_>>();
    completed_keys.sort_unstable();
    let machines = lowerer
        .symbol_resolved_trees
        .machines
        .iter()
        .filter(|machine| {
            predecessor
                .as_ref()
                .is_none_or(|previous| previous.machine(machine.symbol).is_none())
                || completed_keys
                    .binary_search(&symbol_key(machine.symbol))
                    .is_ok()
        })
        .map(|machine| machine.symbol)
        .collect::<Vec<_>>();
    match predecessor {
        Some(predecessor) => lowerer
            .symbol_resolved_trees
            .append_completed_body_tables(&predecessor, &completed_machines),
        None => lowerer.symbol_resolved_trees.rebuild_tables(),
    }
    finish_resolved_tables(lowerer, inputs, &machines)?;
    selection::callable_inference::infer(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_callable_types,
    )?;
    lowerer.pending_callable_types.clear();
    Ok(())
}

/*
    Shared name, obligation and declaration settlement precedes both complete
    table normalization and append-only authored implementation completion.
*/
fn settle_resolution(lowerer: &mut Lowerer, syntax: &SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    body_completion::complete_callable_type_dependencies(lowerer, syntax)?;
    constant::finalize_const_declarations(
        syntax,
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_const_declarations,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    body_completion::complete_initializer_receipt_dependencies(lowerer, syntax)?;
    crate::symbols::reject_unqualified_case_values(&lowerer.symbol_resolved_trees)?;
    mark_token_bound_domain_homes(&mut lowerer.symbol_resolved_trees);
    constant::finalize(lowerer)?;
    selection::finalize_authored_selections(lowerer)?;
    let expressions = lowerer
        .pending_authored_expressions
        .iter()
        .map(|pending| pending.expression)
        .collect::<Vec<_>>();
    retain_operands(&mut lowerer.symbol_resolved_trees, &expressions)?;

    constant::finalize_operator_obligations(lowerer)?;
    selection::finalize(lowerer)?;
    reject_duplicate_direct_token_bindings(&lowerer.symbol_resolved_trees)?;
    Ok(())
}

fn finish_resolved_tables(
    lowerer: &mut Lowerer,
    inputs: &[CallSelectionInput],
    machines: &[SymbolHandle],
) -> Result<(), Vec<Diagnostic>> {
    route_conformance_requirement_calls(&mut lowerer.symbol_resolved_trees, inputs, machines)?;
    route_named_implementation_calls(&mut lowerer.symbol_resolved_trees, inputs, machines);
    crate::symbols::target_machine_families::bind(&mut lowerer.symbol_resolved_trees)?;
    bind_sibling_calls(&mut lowerer.symbol_resolved_trees, inputs, machines);
    crate::symbols::recursive_tail_calls::normalize(&mut lowerer.symbol_resolved_trees, machines);
    crate::symbols::target_machine_families::prepare_premises(&mut lowerer.symbol_resolved_trees)?;
    Ok(())
}

/*
    Select call-result inputs without materializing unrelated runtime expressions.
    Local headers establish the preceding-local scope; selected call roots retain
    their receivers, arguments and machine owner through ordinary final routing.
    This one-shot preparation product grants no body typing or execution authority.
    Receipt-required helper bodies still complete through normal settlement.
*/
pub fn resolve_call_result_selection(
    request: ResolutionRequest,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let (syntax, mut lowerer) = begin(request, ConstResolutionMode::ArgumentSelection)?;
    index_declarations(&mut lowerer, &syntax)?;
    let inputs = assign_lexical_inputs(&mut lowerer, &syntax, LexicalInputDemand::CallResults)?;
    finish_resolution(&mut lowerer, &syntax, &inputs, &[])?;
    Ok(lowerer.into_trees())
}

#[cfg(test)]
mod call_result_selection_tests {
    /*
        Compare actual selected declarations, not just expression counts. Preparation
        must retain lexical and final routing while keeping implementation bodies out
        of its product. Full resolution is the independent reference operation.
    */

    use super::{ResolutionRequest, resolve_call_result_selection, resolve_numeric_probe};
    use crate::tables::body_expressions;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees::{
        SymbolResolvedTrees, expression::ExpressionNode, machine::MachineBodyStatus,
    };

    fn call_identities(program: &SymbolResolvedTrees) -> Vec<String> {
        body_expressions(program)
            .iter_expressions()
            .filter_map(|(_, expression)| {
                let ExpressionNode::Call(call) = expression else {
                    return None;
                };
                assert!(
                    call.target_symbol.is_valid(),
                    "unselected call: {:?}",
                    call.target
                );
                let target = program.symbols.get(call.target_symbol);
                let declaration = program.symbols.symbol_source_span(call.target_symbol);
                let parent = if target.parent.is_valid() {
                    program.symbols.symbol_source_span(target.parent)
                } else {
                    None
                };
                assert!(
                    declaration.is_some() || parent.is_some(),
                    "selected declaration needs source identity"
                );
                Some(format!(
                    "{:?} {:?} {:?} {:?}",
                    call.target.source_span(),
                    target.kind,
                    declaration,
                    parent
                ))
            })
            .collect()
    }

    fn compare(source_text: &str) -> (SymbolResolvedTrees, SymbolResolvedTrees) {
        let tokens = Lexer::new(source_text).tokenize().unwrap();
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add("call-results/main.omg".into(), source_text.to_owned())
            .source_id;
        let sources = std::sync::Arc::new(sources);
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let request = |syntax| ResolutionRequest {
            syntax,
            sources: Some(sources.clone()),
            top_level_bindings: Vec::new(),
            retained_base: None,
        };
        let full = resolve_numeric_probe(request(syntax.clone())).unwrap();
        let selected = resolve_call_result_selection(request(syntax)).unwrap();
        let expected = call_identities(&full);
        assert!(!expected.is_empty());
        assert_eq!(call_identities(&selected), expected);
        assert!(
            selected
                .machines
                .iter()
                .filter(|machine| machine.body_is_present)
                .all(|machine| machine.body_status == MachineBodyStatus::Deferred)
        );
        (full, selected)
    }

    #[test]
    fn call_results_keep_nested_calls_and_preceding_locals_without_runtime_body_rows() {
        let (full, selected) = compare(
            r#"
        machine identity<T>(value: T) -> T { value }
        machine increment(value: u64) -> u64 { value + 1 }
        machine caller(value: u64) -> u64 {
            let first: u64 = value + 2;
            let second: u64 = first + 3;
            identity<u64>(increment(second))
        }
    "#,
        );
        assert!(
            body_expressions(&selected).iter_expressions().count()
                < full.tables.bodies.expressions.iter_expressions().count()
        );
    }

    #[test]
    fn call_results_retain_match_capture_around_selected_payload_calls() {
        compare(
            r#"
        data Cell { value: i32; }
        data Envelope { case Active(cell: &mut Cell); case Empty; }
        machine store(cell: &mut Cell) -> bool { cell.value == 41 }
        machine caller(envelope: Envelope) -> bool {
            match envelope {
                Envelope::Active { cell } -> store(&mut cell)
                Envelope::Empty -> false
            }
        }
    "#,
        );
    }

    #[test]
    fn call_results_keep_each_target_body_selected_declaration() {
        compare(
            r#"
        linux_x86_64 machine helper(value: u64) -> u64 { value + 1 }
        macos_arm64 machine helper(value: u64) -> u64 { value + 2 }
        linux_x86_64 machine caller(value: u64) -> u64 { helper(value) }
        macos_arm64 machine caller(value: u64) -> u64 { helper(value) }
    "#,
        );
    }

    #[test]
    fn call_results_keep_owned_and_inherited_conformance_routing() {
        compare(
            r#"
        trait Reading {
            machine read(value: u64) -> u64;
            machine forward(value: u64) -> u64 { Reading::read(value) }
        }
        First satisfies Reading {
            machine read(value: u64) -> u64 { value }
            machine forward(value: u64) -> u64 { Reading::read(value) }
        }
        Second satisfies Reading {
            machine read(value: u64) -> u64 { value + 1 }
        }
    "#,
        );
    }
}
#[cfg(test)]
mod primitive_family_binding_tests {
    /*
        An authored token binding whose operands name no declared type or domain
        joins a primitive operator family, which only the core semantic-home owner
        publishes. The spelled path is not a home: use sites select by operand
        types alone. Every introducer answers the same rule.
    */

    use super::{ResolutionRequest, resolve};
    use source_files_to_tokens::Lexer;

    const PRIMITIVE_FAMILY: &str = "joins a primitive operator family";

    fn diagnostics(source_text: &str) -> Vec<String> {
        let tokens = Lexer::new(source_text).tokenize().unwrap();
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add("primitive-family/main.omg".into(), source_text.to_owned())
            .source_id;
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        match resolve(ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        }) {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect(),
        }
    }

    fn rejects_as_primitive_family(source_text: &str) -> bool {
        diagnostics(source_text)
            .iter()
            .any(|message| message.contains(PRIMITIVE_FAMILY))
    }

    #[test]
    fn every_introducer_over_primitive_operands_joins_the_primitive_family() {
        for declaration in [
            "boundary operator - Mine::replacement(left: u32, right: u32) -> u32;",
            "boundary operator - u32::replacement(left: u32, right: u32) -> u32;",
            "boundary machine == Comparison::equal(left: i32, right: i32) -> bool;",
            "boundary machine == Float::equal(left: f64, right: f64) -> bool;",
            "operator + Mine::add(left: i32, right: i32) -> i32;",
            "machine + add(left: u8, right: u8) -> u8 { left }",
        ] {
            assert!(rejects_as_primitive_family(declaration), "{declaration}");
        }
    }

    #[test]
    fn a_compiler_policy_qualifier_is_not_a_declared_home() {
        assert!(rejects_as_primitive_family(
            "boundary operator - Mine::replacement(left: u32 in Wrapping, right: u32 in Wrapping) -> u32 in Wrapping;"
        ));
    }

    #[test]
    fn a_declared_data_or_domain_operand_is_a_home_whatever_the_path() {
        for source_text in [
            "data Meter { value: u32; }\nboundary machine + Float::add(left: Meter, right: Meter) -> Meter;",
            "domain u32::Mine;\nboundary machine - Comparison::subtract(left: u32 in Mine, right: u32 in Mine) -> u32;",
        ] {
            assert_eq!(
                diagnostics(source_text),
                Vec::<String>::new(),
                "{source_text}"
            );
        }
    }

    fn scoped_sources(
        units: &[(&str, &str, source::DependencyScope, source::SourceOrigin)],
    ) -> Result<symbol_resolved_trees::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
        let mut sources = source::SourceMap::default();
        let mut syntax = syntax_trees::SyntaxTrees::default();
        for (path, text, scope, origin) in units {
            let path = std::path::PathBuf::from(path);
            let root = path.parent().unwrap().to_path_buf();
            let source = sources
                .add_checked_instance(
                    path,
                    *text,
                    root,
                    None,
                    *origin,
                    source::SourceResolutionStratum::Base,
                    *scope,
                )
                .source_id;
            let tokens = Lexer::new(text).tokenize().unwrap();
            let unit = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
            syntax.extend_from(&unit);
        }
        resolve(ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
    }

    #[test]
    fn domain_token_homes_keep_distinct_build_and_product_instances() {
        use source::{DependencyScope, SourceOrigin};
        let text = "domain<T, const U: u64> T::Quantity<U>;\n\
            machine + Quantity::add(left: u32 in Quantity<1>, right: u32 in Quantity<1>) \
            -> u32 in Quantity<1> { left }";
        let program = scoped_sources(&[
            (
                "core/units.omg",
                text,
                DependencyScope::Build,
                SourceOrigin::Toolchain,
            ),
            (
                "core/units.omg",
                text,
                DependencyScope::Product,
                SourceOrigin::Toolchain,
            ),
        ])
        .unwrap();
        let domains = program.domain_definitions.iter().collect::<Vec<_>>();
        assert_eq!(domains.len(), 2);
        assert_ne!(domains[0].symbol, domains[1].symbol);
        for domain in domains {
            assert_eq!(
                domain.semantic_roles.denotation_dimension,
                Some(domain.semantic_id)
            );
        }
    }

    #[test]
    fn inactive_retained_token_bindings_do_not_rejoin_a_product_frontier() {
        use source::{DependencyScope, SourceOrigin};
        let text = "domain<T, const U: u64> T::Quantity<U>;\n\
            machine + Quantity::add(left: u32 in Quantity<1>, right: u32 in Quantity<1>) \
            -> u32 in Quantity<1> { left }";
        let mut program = scoped_sources(&[
            (
                "core/units.omg",
                text,
                DependencyScope::Build,
                SourceOrigin::Toolchain,
            ),
            (
                "core/units.omg",
                text,
                DependencyScope::Product,
                SourceOrigin::Toolchain,
            ),
        ])
        .unwrap();
        let mut sources = program.symbols.source_map().unwrap().as_ref().clone();
        sources.retain_selected_sources(|file| file.dependency_scope == DependencyScope::Product);
        program.symbols = program
            .symbols
            .begin_source_activation(std::sync::Arc::new(sources), Vec::new())
            .unwrap()
            .finish();
        assert_eq!(
            program
                .machines
                .iter()
                .filter(|machine| program.symbols.symbol_source_is_selected(machine.symbol))
                .count(),
            1
        );
        super::reject_duplicate_direct_token_bindings(&program).unwrap();

        // A newly selected binding still has to name its semantic home. The
        // retained inactive copy must neither reject the valid twin nor supply
        // missing qualification to the invalid one.
        for qualified in [true, false] {
            let suffix = if qualified { " in Quantity<1>" } else { "" };
            let text = format!(
                "machine - Quantity::subtract(left: u32{suffix}, right: u32{suffix}) -> u32{suffix} {{ left }}"
            );
            let mut sources = program.symbols.source_map().unwrap().as_ref().clone();
            let source = sources
                .add_checked_instance(
                    "core/current.omg".into(),
                    text.clone(),
                    "core".into(),
                    None,
                    SourceOrigin::Toolchain,
                    source::SourceResolutionStratum::Base,
                    DependencyScope::Product,
                )
                .source_id;
            let tokens = Lexer::new(&text).tokenize().unwrap();
            let syntax =
                tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
            let result = resolve(ResolutionRequest {
                syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
                retained_base: Some(std::sync::Arc::new(program.clone())),
            });
            if qualified {
                result.unwrap();
            } else {
                let errors = result.unwrap_err();
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("no operand names the carrier of")),
                    "{errors:?}"
                );
            }
        }
    }

    #[test]
    fn an_unimported_same_leaf_domain_does_not_supply_an_operand_home() {
        use source::{DependencyScope, SourceOrigin};
        let errors = scoped_sources(&[
            ("foreign/domain.omg", "public domain u32::Mine;", DependencyScope::Product, SourceOrigin::User),
            ("requester/main.omg", "boundary machine - Comparison::subtract(left: u32 in Mine, right: u32 in Mine) -> u32;", DependencyScope::Product, SourceOrigin::User),
        ]).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains(PRIMITIVE_FAMILY)),
            "{errors:?}"
        );
    }
}
