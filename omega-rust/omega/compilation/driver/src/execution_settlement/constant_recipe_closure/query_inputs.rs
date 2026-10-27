//! Exact observations consumed by a private constant/proof application.
//! Declaration references retain constants; admitted call facts retain code.
//! Neither reference discovery nor a matching value grants execution authority.
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use std::collections::HashSet;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::references::{DeclarationSelection, StaticArgumentCall, TypedReferences};

/// Exact formed carrier and the admitted source application that supplies its extent.
#[derive(Clone, Copy)]
pub(super) struct ArrayExtentUse {
    pub owner: SymbolHandle,
    pub identity: typed_trees::type_identity::TypeIdentityId,
    pub recipe: typed_trees::constant_recipes::ConstantRecipeHandle,
}

pub(super) struct Inputs {
    pub applications: Vec<SymbolHandle>,
    pub work: DeclarationSelection,
    pub recipes: Vec<typed_trees::constant_recipes::ConstantRecipeHandle>,
    pub array_extent_uses: Vec<ArrayExtentUse>,
    pub storage_roots: Vec<(SymbolHandle, typed_trees::type_identity::TypeIdentityId)>,
    pub recipe_consumers: Vec<(
        SymbolHandle,
        typed_trees::constant_recipes::ConstantRecipeHandle,
    )>,
    pub queries: HashSet<ExpressionHandle>,
    pub owners: Vec<(SymbolHandle, HashSet<ExpressionHandle>)>,
}

impl Inputs {
    /// The retained evaluator owns source jobs, not checked expression slots.
    /// Ask the same dependency operation for those jobs' source observations;
    /// completed constant histories and ordinary runtime queries stay inert.
    pub(super) fn authored_queries(
        &self,
        checked: &CheckedTrees,
        authored: &typed_trees::TypedTrees,
        target: target::TargetProfile,
        providers: &[provider_planning::SelectedProviderReviewProvenance],
        work: &DeclarationSelection,
    ) -> HashSet<ExpressionHandle> {
        let mut roots = Vec::new();
        for declaration in authored.const_declarations() {
            if work.contains(declaration.symbol) && declaration.evaluated_value.is_none() {
                append_roots(
                    &mut roots,
                    declaration.symbol,
                    [
                        declaration.authored_initializer,
                        declaration.materialized_initializer,
                    ]
                    .into_iter()
                    .filter(|root| root.is_valid()),
                    [],
                );
            }
        }
        for recipe in &self.recipes {
            let recipe = authored.constant_recipes.get(*recipe);
            let expression = match recipe.position {
                typed_trees::constant_recipes::RetainedConstantPosition::RangeEndpoint(root)
                | typed_trees::constant_recipes::RetainedConstantPosition::ConstApplication {
                    root,
                    ..
                } => Some(root),
                _ => None,
            };
            append_roots(&mut roots, recipe.machine, expression, [recipe.machine]);
        }
        for layout in &authored.plan_laid_layouts {
            let entries = layout
                .target_layouts(authored)
                .iter()
                .filter(|row| row.target.as_str() == target.target_name())
                .map(|row| row.machine)
                .chain(
                    matches!(
                        layout.realizations,
                        typed_trees::PlanLaidRealizations::Invariant(_)
                    )
                    .then_some(layout.policy_plan_machine_symbol),
                );
            for entry in entries {
                if self.applications.contains(&entry) {
                    append_roots(&mut roots, entry, [], [entry]);
                }
            }
        }
        roots
            .iter()
            .flat_map(|root| dependencies(checked, authored, target, providers, root).queries)
            .collect()
    }
}

#[derive(Clone)]
pub(super) struct Roots {
    pub owner: SymbolHandle,
    expressions: Vec<ExpressionHandle>,
    declarations: Vec<SymbolHandle>,
}

pub(super) struct PreparedInputs<'source> {
    checked: &'source CheckedTrees,
    array_extent_uses: Vec<ArrayExtentUse>,
    storage_roots: Vec<(SymbolHandle, typed_trees::type_identity::TypeIdentityId)>,
    consumers: Vec<(
        SymbolHandle,
        typed_trees::constant_recipes::ConstantRecipeHandle,
    )>,
}

impl<'source> PreparedInputs<'source> {
    pub(super) fn new(checked: &'source CheckedTrees) -> Self {
        let storage_roots = checked
            .facts
            .flow
            .terminal_unit_effects
            .stored_type_roots
            .clone();
        let mut array_extent_uses = Vec::new();
        let mut consumers: Vec<_> = checked
            .facts
            .constant_recipe_obligations
            .iter()
            .map(|row| (row.owner, row.recipe))
            .collect();
        // Source formation records all recipe inputs below an actual body root.
        // Realization later excludes payload below a closed zero-length array.
        for (owner, root) in &storage_roots {
            for identity in checked
                .facts
                .flow
                .terminal_unit_effects
                .stored_type_closure(*root, |_| true)
            {
                if let Some(shape) = checked_trees::checked_structural_type_by_identity(
                    &checked.facts.flow.terminal_unit_effects.structural_types,
                    identity,
                ) && let checked_trees::CheckedUnitStructuralTypeShape::FixedArray {
                    length: checked_trees::CheckedExtent::Recipe(recipe),
                    ..
                } = shape.shape
                {
                    consumers.push((*owner, recipe));
                    array_extent_uses.push(ArrayExtentUse {
                        owner: *owner,
                        identity,
                        recipe,
                    });
                }
            }
        }
        consumers.sort_by_key(|(owner, recipe)| {
            (
                owner.arena_index(),
                owner.generation(),
                recipe.arena_index(),
                recipe.generation(),
            )
        });
        consumers.dedup();
        let mut formed = HashSet::new();
        array_extent_uses.retain(|formation| {
            formed.insert((formation.owner, formation.identity, formation.recipe))
        });
        Self {
            checked,
            array_extent_uses,
            storage_roots,
            consumers,
        }
    }

    pub(super) fn consumer_owners(&self) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.consumers.iter().map(|(owner, _)| *owner)
    }

    pub(super) fn prepare(
        &self,
        target: target::TargetProfile,
        providers: &[provider_planning::SelectedProviderReviewProvenance],
        opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
        mut applications: Vec<SymbolHandle>,
        declaration_roots: &[SymbolHandle],
    ) -> Result<Inputs, Vec<Diagnostic>> {
        let checked = self.checked;
        let storage_roots = &self.storage_roots;
        for declaration in declaration_roots {
            if !applications.contains(declaration) {
                applications.push(*declaration);
            }
        }

        let materialized = materialized_entry_inputs(checked, Some(target.target_name()));
        for root in &materialized {
            if !applications.contains(&root.owner) {
                applications.push(root.owner);
            }
        }
        let mut query_consumers = Vec::new();
        loop {
            let mut selected_proofs = Vec::new();
            for row in &checked.facts.constant_predicate_obligations {
                if applications.contains(&row.caller()) {
                    selected_proofs.push((
                        row.caller(),
                        row.dependencies_for_target(&checked.typed, target.target_name())
                            .map_err(|reason| vec![Diagnostic::error(reason)])?,
                    ));
                }
            }
            for row in &checked.facts.integer_range_obligations {
                if applications.contains(&row.owner()) {
                    selected_proofs.push((
                        row.owner(),
                        row.dependencies_for_target(&checked.typed, target.target_name())
                            .map_err(|reason| vec![Diagnostic::error(reason)])?,
                    ));
                }
            }
            let work = declaration_work(
                &checked.typed,
                &applications,
                DeclarationSelection::from_symbols(applications.iter().copied()),
            );
            let mut references = References::new(&checked.typed, Some(&work));
            references.visit_selected_declarations();
            let mut recipes: Vec<_> = checked
                .typed
                .constant_recipes
                .iter()
                .filter_map(|(handle, recipe)| {
                    use typed_trees::constant_recipes::RetainedConstantPosition;
                    let selected = match recipe.position {
                        RetainedConstantPosition::None => false,
                        RetainedConstantPosition::ArrayLength(site) => {
                            references.types.contains(&site)
                        }
                        RetainedConstantPosition::RangeEndpoint(root) => {
                            references.expressions.contains(&root)
                        }
                        RetainedConstantPosition::ConstApplication { site, root } => {
                            references.types.contains(&site)
                                || references.expressions.contains(&root)
                        }
                    };
                    selected.then_some(handle)
                })
                .collect();
            let mut recipe_consumers = query_consumers.clone();
            for (_, recipe) in &query_consumers {
                if !recipes.contains(recipe) {
                    recipes.push(*recipe);
                }
            }
            // Captured proof tapes retain exact selected occurrences. Storage
            // shapes instead name canonical array applications: join those to
            // every actual selected source position, never another body's site
            // merely because the shape collector encountered it first.
            for (owner, dependencies) in &selected_proofs {
                for recipe in &dependencies.recipes {
                    if !recipes.contains(recipe) {
                        recipes.push(*recipe);
                    }
                    recipe_consumers.push((*owner, *recipe));
                }
            }
            for (owner, recipe) in &self.consumers {
                if !applications.contains(owner) {
                    continue;
                }
                if let Some(application) = checked.typed.array_extent_application(*recipe) {
                    let mut found = false;
                    for selected in &recipes {
                        if checked.typed.array_extent_application(*selected) == Some(application) {
                            recipe_consumers.push((*owner, *selected));
                            found = true;
                        }
                    }
                    if !found {
                        return Err(vec![Diagnostic::error(
                            "selected storage extent has no admitted authored application position",
                        )]);
                    }
                } else {
                    if !recipes.contains(recipe) {
                        recipes.push(*recipe);
                    }
                    recipe_consumers.push((*owner, *recipe));
                }
            }
            recipe_consumers.sort_by_key(|(owner, recipe)| {
                (
                    owner.arena_index(),
                    owner.generation(),
                    recipe.arena_index(),
                    recipe.generation(),
                )
            });
            recipe_consumers.dedup();
            let mut array_extent_uses = Vec::new();
            for formation in &self.array_extent_uses {
                if !applications.contains(&formation.owner) {
                    continue;
                }
                let application = checked
                    .typed
                    .array_extent_application(formation.recipe)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "formed array lost its checked extent application",
                        )]
                    })?;
                for (owner, recipe) in &recipe_consumers {
                    if *owner == formation.owner
                        && checked.typed.array_extent_application(*recipe) == Some(application)
                    {
                        array_extent_uses.push(ArrayExtentUse {
                            recipe: *recipe,
                            ..*formation
                        });
                    }
                }
            }
            let mut formed = HashSet::new();
            array_extent_uses.retain(|formation| {
                formed.insert((formation.owner, formation.identity, formation.recipe))
            });
            let mut roots = materialized.clone();
            for declaration in checked.typed.const_declarations() {
                if work.contains(declaration.symbol) && declaration.evaluated_value.is_none() {
                    append_roots(
                        &mut roots,
                        declaration.symbol,
                        [
                            declaration.authored_initializer,
                            declaration.materialized_initializer,
                        ]
                        .into_iter()
                        .filter(|root| root.is_valid()),
                        [],
                    );
                }
            }
            for (owner, dependencies) in &selected_proofs {
                append_roots(
                    &mut roots,
                    *owner,
                    dependencies
                        .observations
                        .iter()
                        .map(|(expression, _)| *expression),
                    [],
                );
            }
            // A symbolic type position consumes its helper just as a retained
            // scalar does. The recipe roster records that exact application even
            // though no runtime call or scalar-computation row survives for it.
            // Keep its query dependencies on the same admitted context path.
            for handle in &recipes {
                let recipe = checked.typed.constant_recipes.get(*handle);
                let expressions = match recipe.position {
                    typed_trees::constant_recipes::RetainedConstantPosition::RangeEndpoint(
                        root,
                    )
                    | typed_trees::constant_recipes::RetainedConstantPosition::ConstApplication {
                        root,
                        ..
                    } => Some(root),
                    _ => None,
                };
                append_roots(&mut roots, recipe.machine, expressions, [recipe.machine]);
                for (owner, consumed) in &recipe_consumers {
                    if *consumed == *handle {
                        append_roots(&mut roots, *owner, expressions, [recipe.machine]);
                    }
                }
            }
            for (_, node) in checked.facts.values.scalar_computations.nodes.iter() {
                if let checked_trees::CheckedScalarComputationKind::RetainedConstant {
                    expression,
                    machine,
                } = node.kind
                    && applications.contains(&machine)
                {
                    append_roots(&mut roots, machine, [expression], []);
                }
            }
            let mut owners = Vec::new();
            let mut queries = HashSet::new();
            let mut added_application = false;
            for root in roots {
                let dependencies = dependencies(checked, &checked.typed, target, providers, &root);
                for machine in dependencies.machines {
                    if !applications.contains(&machine) {
                        applications.push(machine);
                        added_application = true;
                    }
                }
                for expression in &dependencies.queries {
                    if !checked
                        .facts
                        .layout_query_subjects
                        .iter()
                        .any(|query| query.expression == *expression)
                    {
                        return Err(vec![
                            Diagnostic::error(
                                "deferred observation has no complete checked layout subject",
                            )
                            .with_source_span(
                                checked.typed.expression_table.source_span(*expression),
                            ),
                        ]);
                    }
                }
                queries.extend(dependencies.queries.iter().copied());
                if !dependencies.queries.is_empty() {
                    owners.push((root.owner, dependencies.queries));
                }
            }
            // The layout producer reports exactly the by-value prerequisites
            // of each observation. Do not reactivate its source type graph:
            // references and descriptors have no pointee-storage demand.
            let mut geometry_requests = queries.clone();
            geometry_requests.extend(references.expressions.iter().copied().filter(|expression| matches!(
                checked.typed.expression_table.expression(*expression), ExpressionNode::Call(call) if call.layout_query.is_some()
            )));
            for (expression, recipe) in
                provider_planning::data_layout_queries::query_constant_inputs(
                    &checked.typed,
                    target,
                    opaque_representation_selections,
                    &work,
                    &geometry_requests,
                )?
            {
                let consumers = owners
                    .iter()
                    .filter(|(_, queries)| queries.contains(&expression))
                    .map(|(owner, _)| *owner)
                    .chain(
                        checked
                            .facts
                            .layout_query_subjects
                            .iter()
                            .filter(|query| {
                                query.expression == expression
                                    && applications.contains(&query.owner)
                            })
                            .map(|query| query.owner),
                    );
                for owner in consumers {
                    if !query_consumers.contains(&(owner, recipe)) {
                        query_consumers.push((owner, recipe));
                        added_application = true;
                    }
                }
            }
            if added_application {
                continue;
            }
            let selected_storage_roots = storage_roots
                .iter()
                .copied()
                .filter(|(owner, _)| applications.contains(owner))
                .collect();
            return Ok(Inputs {
                applications,
                work,
                recipes,
                array_extent_uses,
                recipe_consumers,
                storage_roots: selected_storage_roots,
                queries,
                owners,
            });
        }
    }
}

/// A materialized policy call consumed its entry contract while making a
/// carrier. Its query inputs remain conditions of that carrier even though no
/// source call remains to produce a deferred caller obligation. Only recorded
/// materializations contribute: ordinary runtime entry contracts still belong
/// to their callers, never to a global geometry preflight.
pub(super) fn materialized_entry_inputs(
    checked: &CheckedTrees,
    target: Option<&str>,
) -> Vec<Roots> {
    let mut roots = Vec::new();
    for layout in &checked.typed.plan_laid_layouts {
        let entries = layout
            .target_layouts(&checked.typed)
            .iter()
            .filter(|row| target.is_none_or(|target| row.target.as_str() == target))
            .map(|row| row.machine)
            .chain(
                matches!(
                    layout.realizations,
                    typed_trees::PlanLaidRealizations::Invariant(_)
                )
                .then_some(layout.policy_plan_machine_symbol),
            );
        for entry in entries {
            let Some(machine) = checked.typed.machine_by_symbol(entry) else {
                continue;
            };
            let mut references = References::new(&checked.typed, None);
            // Scalar contract projection is optional and whole-clause: a
            // membership beside a scalar fact can make that projection absent.
            // The semantic fact roster still owns all consumed dependencies.
            for contract in checked.typed.machine_contracts(machine).iter().chain(
                checked
                    .typed
                    .machine_states(machine)
                    .first()
                    .into_iter()
                    .flat_map(|state| checked.typed.state_contracts(state)),
            ) {
                if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
                    continue;
                }
                for fact in checked.typed.proof_facts.span_or_empty(contract.facts) {
                    typed_trees::references::visit_proof_fact(
                        &checked.typed,
                        &mut references,
                        fact,
                    );
                }
            }
            if !references.expressions.is_empty() || !references.declarations.is_empty() {
                append_roots(
                    &mut roots,
                    entry,
                    references.expressions,
                    references.declarations,
                );
            }
        }
    }
    roots
}

fn append_roots(
    roots: &mut Vec<Roots>,
    owner: SymbolHandle,
    expressions: impl IntoIterator<Item = ExpressionHandle>,
    declarations: impl IntoIterator<Item = SymbolHandle>,
) {
    if let Some(existing) = roots.iter_mut().find(|candidate| candidate.owner == owner) {
        existing.expressions.extend(expressions);
        existing.declarations.extend(declarations);
    } else {
        roots.push(Roots {
            owner,
            expressions: expressions.into_iter().collect(),
            declarations: declarations.into_iter().collect(),
        });
    }
}

struct Dependencies {
    queries: HashSet<ExpressionHandle>,
    machines: HashSet<SymbolHandle>,
}

struct References<'program> {
    program: &'program typed_trees::TypedTrees,
    selection: Option<&'program DeclarationSelection>,
    symbols: HashSet<SymbolHandle>,
    expressions: HashSet<ExpressionHandle>,
    types: HashSet<typed_trees::types::TypeReferenceHandle>,
    declarations: Vec<SymbolHandle>,
    declaration_roots: DeclarationSelection,
    calls: Vec<SymbolHandle>,
}

impl<'program> References<'program> {
    fn new(
        program: &'program typed_trees::TypedTrees,
        selection: Option<&'program DeclarationSelection>,
    ) -> Self {
        Self {
            program,
            selection,
            symbols: HashSet::new(),
            expressions: HashSet::new(),
            types: HashSet::new(),
            declarations: Vec::new(),
            declaration_roots: DeclarationSelection::from_program(program),
            calls: Vec::new(),
        }
    }

    /// Share the semantic declaration walk between work selection and demand
    /// capture. An expanded constant's authored initializer is lookup history;
    /// only its materialized value can supply a current proof input.
    fn visit_selected_declarations(&mut self) {
        let program = self.program;
        typed_trees::references::visit_program(program, self);
        for declaration in program.const_declarations() {
            if self
                .selection
                .is_some_and(|selection| !selection.contains(declaration.symbol))
            {
                continue;
            }
            self.extend([declaration.symbol]);
            typed_trees::references::visit_type_reference(program, self, declaration.declared_type);
            typed_trees::references::visit_expression(
                program,
                self,
                declaration.materialized_initializer,
            );
            // A pending declaration still owes its authored expression's
            // observation and call custody. Only completed values make that
            // earlier initializer historical rather than evaluation work.
            if declaration.evaluated_value.is_none() && declaration.authored_initializer.is_valid()
            {
                typed_trees::references::visit_expression(
                    program,
                    self,
                    declaration.authored_initializer,
                );
            }
        }
    }
}

/// The same semantic dependencies own evaluation work and proof inputs.
/// Historic declaration references remain available in the lookup catalog,
/// but cannot independently schedule an expanded constant's old initializer.
pub(super) fn declaration_work(
    program: &typed_trees::TypedTrees,
    applications: &[SymbolHandle],
    mut selected: DeclarationSelection,
) -> DeclarationSelection {
    let declarations = DeclarationSelection::from_program(program);
    loop {
        let before = selected.clone();
        let mut references = References::new(program, Some(&before));
        references.visit_selected_declarations();
        for symbol in references.symbols {
            if declarations.contains(symbol)
                && (program.machine_by_symbol(symbol).is_none() || applications.contains(&symbol))
            {
                selected.include(symbol);
            }
        }
        if selected == before {
            return selected;
        }
    }
}

impl Extend<SymbolHandle> for References<'_> {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: Symbols) {
        for symbol in symbols {
            if self.symbols.insert(symbol)
                && self.declaration_roots.contains(symbol)
                && self.program.machine_by_symbol(symbol).is_none()
            {
                self.declarations.push(symbol);
            }
        }
    }
}
impl TypedReferences for References<'_> {
    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        self.program.const_declaration_by_symbol(symbol).is_none()
            && self
                .selection
                .is_none_or(|selection| selection.contains(symbol))
    }
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if !self.expressions.insert(expression) {
            return false;
        }
        // A formed query is one semantic observation. Its authored selection
        // ledger can still describe a constant that normalization expanded;
        // that history does not evaluate the old initializer a second time.
        if matches!(self.program.expression_table.expression(expression),
            ExpressionNode::Call(call) if call.layout_query.is_some())
        {
            typed_trees::references::visit_pending_constant_inputs(self.program, self, expression);
            return false;
        }
        true
    }
    fn type_reference(&mut self, reference: typed_trees::types::TypeReferenceHandle) -> bool {
        self.types.insert(reference)
    }
    fn call_arguments(
        &mut self,
        _: &typed_trees::TypedTrees,
        call: StaticArgumentCall<'_>,
    ) -> bool {
        self.calls.push(match call {
            StaticArgumentCall::Expression(_, call) => call.target_symbol,
            StaticArgumentCall::Statement(call) => call.target_symbol,
        });
        true
    }
}

fn dependencies(
    checked: &CheckedTrees,
    program: &typed_trees::TypedTrees,
    target: target::TargetProfile,
    providers: &[provider_planning::SelectedProviderReviewProvenance],
    roots: &Roots,
) -> Dependencies {
    let mut references = References::new(program, None);
    for root in &roots.expressions {
        typed_trees::references::visit_expression(program, &mut references, *root);
    }
    for &declaration in &roots.declarations {
        if program.machine_by_symbol(declaration).is_some() {
            references.calls.push(declaration);
        } else if declaration.is_valid() {
            references.declarations.push(declaration);
        }
    }
    let mut declarations = HashSet::new();
    let mut machines = HashSet::new();
    while !references.declarations.is_empty() || !references.calls.is_empty() {
        let fresh: Vec<_> = references
            .declarations
            .drain(..)
            .filter(|symbol| declarations.insert(*symbol))
            .collect();
        if !fresh.is_empty() {
            // Domain, proposition, data and mathematical declarations retain
            // their own requirements. Membership must not hide a consumed
            // observation just because the entry clause has no scalar reading.
            let selection = DeclarationSelection::from_symbols(fresh);
            let mut found = References::new(program, Some(&selection));
            found.visit_selected_declarations();
            references.expressions.extend(found.expressions);
            references.types.extend(found.types);
            references.symbols.extend(found.symbols);
            references.declarations.extend(found.declarations);
            references.calls.extend(found.calls);
        }
        while let Some(called) = references.calls.pop() {
            let dependencies = provider_planning::target_machines::proof_application_dependencies(
                checked, target, called, providers,
            );
            let mut fresh = Vec::new();
            for machine in dependencies {
                if machines.insert(machine) {
                    fresh.push(machine);
                }
            }
            if fresh.is_empty() {
                continue;
            }
            let selection = DeclarationSelection::from_symbols(fresh);
            let mut found = References::new(program, Some(&selection));
            found.visit_selected_declarations();
            references.expressions.extend(found.expressions);
            references.types.extend(found.types);
            references.symbols.extend(found.symbols);
            references.declarations.extend(found.declarations);
            references.calls.extend(found.calls);
        }
    }
    let queries = references.expressions.into_iter().filter(|expression| matches!(
        program.expression_table.expression(*expression), ExpressionNode::Call(call) if call.layout_query.is_some()
    )).collect();
    Dependencies { queries, machines }
}
