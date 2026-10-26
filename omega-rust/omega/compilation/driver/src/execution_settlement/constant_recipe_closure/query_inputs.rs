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

pub(super) fn prepare(
    checked: &CheckedTrees,
    target: target::TargetProfile,
    providers: &[provider_planning::SelectedProviderReviewProvenance],
    mut applications: Vec<SymbolHandle>,
) -> Result<Inputs, Vec<Diagnostic>> {
    loop {
        let mut roots: Vec<(SymbolHandle, Vec<ExpressionHandle>)> = Vec::new();
        for row in &checked.facts.constant_predicate_obligations {
            if applications.contains(&row.caller()) {
                append_roots(
                    &mut roots,
                    row.caller(),
                    row.observation_dependencies()
                        .map(|(expression, _)| expression),
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
                append_roots(&mut roots, machine, [expression]);
            }
        }
        let mut owners = Vec::new();
        let mut queries = HashSet::new();
        let mut added_application = false;
        for (owner, expressions) in roots {
            let dependencies = dependencies(checked, target, providers, &expressions);
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
                owners.push((owner, dependencies.queries));
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

fn append_roots(
    roots: &mut Vec<(SymbolHandle, Vec<ExpressionHandle>)>,
    owner: SymbolHandle,
    expressions: impl IntoIterator<Item = ExpressionHandle>,
) {
    if let Some((_, existing)) = roots.iter_mut().find(|(candidate, _)| *candidate == owner) {
        existing.extend(expressions);
    } else {
        roots.push((owner, expressions.into_iter().collect()));
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
    constants: Vec<SymbolHandle>,
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
            constants: Vec::new(),
            calls: Vec::new(),
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
        typed_trees::references::visit_program(program, &mut references);
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
            self.symbols.insert(symbol);
            if self.program.const_declaration_by_symbol(symbol).is_some() {
                self.constants.push(symbol);
            }
        }
    }
}
impl TypedReferences for References<'_> {
    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        self.selection
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
    roots: &[ExpressionHandle],
) -> Dependencies {
    let program = &checked.typed;
    let mut references = References::new(program, None);
    for root in roots {
        typed_trees::references::visit_expression(program, &mut references, *root);
    }
    let mut constants = HashSet::new();
    let mut machines = HashSet::new();
    while !references.constants.is_empty() || !references.calls.is_empty() {
        while let Some(symbol) = references.constants.pop() {
            if constants.insert(symbol)
                && let Some(declaration) = program.const_declaration_by_symbol(symbol)
            {
                typed_trees::references::visit_expression(
                    program,
                    &mut references,
                    declaration.materialized_initializer,
                );
            }
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
            typed_trees::references::visit_program(program, &mut found);
            references.expressions.extend(found.expressions);
            references.types.extend(found.types);
            references.symbols.extend(found.symbols);
            references.constants.extend(found.constants);
            references.calls.extend(found.calls);
        }
    }
    let queries = references.expressions.into_iter().filter(|expression| matches!(
        program.expression_table.expression(*expression), ExpressionNode::Call(call) if call.layout_query.is_some()
    )).collect();
    Dependencies { queries, machines }
}
