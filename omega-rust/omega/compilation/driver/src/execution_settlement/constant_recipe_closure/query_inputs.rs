//! Exact observations consumed by a private constant/proof application.
//! Declaration references retain constants; admitted call facts retain code.
//! Neither reference discovery nor a matching value grants execution authority.
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use std::collections::HashSet;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::references::{DeclarationSelection, StaticArgumentCall, TypedReferences};

pub(super) struct Inputs {
    pub applications: Vec<SymbolHandle>,
    pub work: DeclarationSelection,
    pub queries: HashSet<ExpressionHandle>,
    pub owners: Vec<(SymbolHandle, HashSet<ExpressionHandle>)>,
}

#[derive(Clone)]
pub(super) struct Roots {
    pub owner: SymbolHandle,
    expressions: Vec<ExpressionHandle>,
    declarations: Vec<SymbolHandle>,
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    target: target::TargetProfile,
    providers: &[provider_planning::SelectedProviderReviewProvenance],
    mut applications: Vec<SymbolHandle>,
) -> Result<Inputs, Vec<Diagnostic>> {
    let materialized = materialized_entry_inputs(checked, Some(target.target_name()));
    for root in &materialized {
        if !applications.contains(&root.owner) {
            applications.push(root.owner);
        }
    }
    loop {
        let mut roots = materialized.clone();
        for row in &checked.facts.constant_predicate_obligations {
            if applications.contains(&row.caller()) {
                append_roots(
                    &mut roots,
                    row.caller(),
                    row.observation_dependencies()
                        .map(|(expression, _)| expression),
                    [],
                );
            }
        }
        for row in &checked.facts.integer_range_obligations {
            if applications.contains(&row.owner()) {
                append_roots(
                    &mut roots,
                    row.owner(),
                    row.observation_dependencies()
                        .map(|(expression, _)| expression),
                    [],
                );
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
            let dependencies = dependencies(checked, target, providers, &root);
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
                        .with_source_span(checked.typed.expression_table.source_span(*expression)),
                    ]);
                }
            }
            queries.extend(dependencies.queries.iter().copied());
            if !dependencies.queries.is_empty() {
                owners.push((root.owner, dependencies.queries));
            }
        }
        if added_application {
            continue;
        }
        let work = work(checked, &applications);
        return Ok(Inputs {
            applications,
            work,
            queries,
            owners,
        });
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
        }
    }
}

/// The same semantic dependencies own evaluation work and proof inputs.
/// Historic declaration references remain available in the lookup catalog,
/// but cannot independently schedule an expanded constant's old initializer.
fn work(checked: &CheckedTrees, applications: &[SymbolHandle]) -> DeclarationSelection {
    let program = &checked.typed;
    let declarations = DeclarationSelection::from_program(program);
    let mut selected = DeclarationSelection::from_symbols(applications.iter().copied());
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
        !matches!(self.program.expression_table.expression(expression),
            ExpressionNode::Call(call) if call.layout_query.is_some())
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
    target: target::TargetProfile,
    providers: &[provider_planning::SelectedProviderReviewProvenance],
    roots: &Roots,
) -> Dependencies {
    let program = &checked.typed;
    let mut references = References::new(program, None);
    for root in &roots.expressions {
        typed_trees::references::visit_expression(program, &mut references, *root);
    }
    references
        .declarations
        .extend(roots.declarations.iter().copied());
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
