//! Rejoin a scalar initializer with its complete selected ordinary call closure.
//!
//! Static callable operands keep the binder's logical source selection beside
//! the selected body's dependencies. Receiving replay rejoins that binder to
//! this invocation's exact substitution; a matching signature or result value
//! cannot replace the authored selection.

use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
};
use source::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};
use syntax_trees::{SyntaxTrees, types::ConstArgumentOrigin};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
};

mod call_selections;
mod types;

#[derive(Default)]
pub(super) struct Custody {
    pub origins: Vec<ConstArgumentOrigin>,
    pub operators: Vec<SourceSpan>,
    pub calls: Vec<(SourceSpan, SourceSpan)>,
    call_targets: Vec<(SourceSpan, SymbolHandle)>,
    dependency_values: Vec<DependencyValue>,
}

/// Receiving-only value subjects reconstructed from exact selected declarations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DependencyValue {
    pub declaration: SymbolHandle,
    pub expression: ExpressionHandle,
}

#[derive(Clone, Copy)]
struct Context<'program> {
    machine: &'program Machine,
    state: &'program State,
    initializer: bool,
}

/// Every authored call expression surviving inside detached initializer
/// expression roots, in deterministic walk order. Aggregate-producing leaf
/// probes discharge each of these invocations individually before
/// interpretation; hoisted probe bodies contribute one root per statement.
pub(super) fn call_expressions(
    program: &TypedTrees,
    roots: &[ExpressionHandle],
) -> Result<Vec<ExpressionHandle>, String> {
    let mut calls = Vec::new();
    let mut pending = roots
        .iter()
        .rev()
        .map(|root| (*root, false))
        .collect::<Vec<_>>();
    let mut active = Vec::new();
    let mut complete = Vec::new();
    while let Some((expression, finish)) = pending.pop() {
        if finish {
            if active.pop() != Some(expression) {
                return Err("constant initializer has invalid expression traversal".into());
            }
            complete.push(expression);
            continue;
        }
        if active.contains(&expression) {
            return Err("constant initializer contains a cyclic expression".into());
        }
        if complete.contains(&expression) {
            continue;
        }
        let children = expression_children(program, expression)?;
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Call(_)
        ) {
            calls.push(expression);
        }
        active.push(expression);
        pending.push((expression, true));
        pending.extend(children.into_iter().rev().map(|child| (child, false)));
    }
    Ok(calls)
}

pub(super) fn contains_call(program: &TypedTrees, root: ExpressionHandle) -> Result<bool, String> {
    let mut contains_call = false;
    for_each_initializer_expression(program, &[root], |expression| {
        contains_call |= matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Call(_)
        );
    })?;
    Ok(contains_call)
}

/// Shared ordinary expression edges for custody and demand. This establishes
/// arena shape only; declaration and invocation admission remain separate.
/// The (expression, finish) DFS spine constant-initializer walks share:
/// cycle detection, once-per-expression visits, and malformed-traversal
/// errors. `visit` sees each expression once, pre-order.
fn for_each_initializer_expression(
    program: &TypedTrees,
    roots: &[ExpressionHandle],
    mut visit: impl FnMut(ExpressionHandle),
) -> Result<(), String> {
    let mut pending: Vec<(ExpressionHandle, bool)> =
        roots.iter().rev().map(|root| (*root, false)).collect();
    let mut active = Vec::new();
    let mut complete = Vec::new();
    while let Some((expression, finish)) = pending.pop() {
        if finish {
            if active.pop() != Some(expression) {
                return Err("constant initializer has invalid expression traversal".into());
            }
            complete.push(expression);
            continue;
        }
        if active.contains(&expression) {
            return Err("constant initializer contains a cyclic expression".into());
        }
        if complete.contains(&expression) {
            continue;
        }
        let children = expression_children(program, expression)?;
        visit(expression);
        active.push(expression);
        pending.push((expression, true));
        pending.extend(children.into_iter().rev().map(|child| (child, false)));
    }
    Ok(())
}

fn expression_children(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Result<Vec<ExpressionHandle>, String> {
    let table = &program.expression_table;
    if !table.expression_is_valid(expression) {
        return Err("constant initializer contains a stale expression".into());
    }
    let mut children = Vec::new();
    match table.expression(expression) {
        ExpressionNode::SavedSubject(_) => {}
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                children.push(call.receiver);
            }
            let arguments = table.expression_handles(call.arguments);
            if arguments.len() != call.arguments.len() {
                return Err("constant call closure has stale argument span".into());
            }
            children.extend(arguments.iter().copied());
        }
        ExpressionNode::Binary(binary) => children.extend([binary.left, binary.right]),
        ExpressionNode::Unary(unary) => children.push(unary.operand),
        ExpressionNode::Cast(cast) => children.push(cast.value),
        ExpressionNode::Member(member) => children.push(member.receiver),
        ExpressionNode::Borrow(borrow) => children.push(borrow.target),
        ExpressionNode::Indexed(indexed) => children.extend([indexed.collection, indexed.index]),
        ExpressionNode::Atomic(atomic) => {
            children.push(atomic.value);
            if atomic.result.is_valid() {
                children.push(atomic.result);
            }
        }
        ExpressionNode::Range(range) => children.extend(
            [range.start, range.end]
                .into_iter()
                .filter(|handle| handle.is_valid()),
        ),
        ExpressionNode::ArrayLiteral(elements) => {
            let values = table.expression_handles(*elements);
            if values.len() != elements.len() {
                return Err("constant call closure has stale array span".into());
            }
            children.extend(values.iter().copied());
        }
        ExpressionNode::StructLiteral(literal) => {
            let fields = table.struct_fields(literal.fields);
            if fields.len() != literal.fields.len() {
                return Err("constant call closure has stale field span".into());
            }
            children.extend(fields.iter().map(|field| field.value));
        }
        ExpressionNode::Match(dispatch) => {
            let arms = table.match_arms(dispatch.arms);
            if arms.len() != dispatch.arms.len() {
                return Err("constant call closure has stale Match span".into());
            }
            children.push(dispatch.subject);
            for arm in arms {
                children.extend(table.match_pattern_dependencies(arm.pattern));
                children.push(arm.value);
            }
        }
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
    Ok(children)
}

pub(super) fn collect(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
    public: bool,
    syntax: &SyntaxTrees,
) -> Result<Custody, String> {
    collect_internal(
        program,
        machine,
        state,
        &[root],
        public,
        Some(syntax),
        true,
        true,
    )
}

/// Declaration-source custody before private realization. This traversal cannot
/// import a concrete implementation body through an abstract requirement.
pub(super) fn collect_authored_roots(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    roots: &[ExpressionHandle],
    syntax: &SyntaxTrees,
) -> Result<Custody, String> {
    collect_internal(
        program,
        machine,
        state,
        roots,
        false,
        Some(syntax),
        false,
        true,
    )
}

/// The same closure walk over several expression roots: a structured leaf
/// probe's normalized body may hoist its terminal call into a `let`
/// initializer beside the return transition, so custody covers every
/// statement root rather than the return value alone.
pub(super) fn collect_roots(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    roots: &[ExpressionHandle],
    public: bool,
    syntax: &SyntaxTrees,
) -> Result<Custody, String> {
    collect_internal(
        program,
        machine,
        state,
        roots,
        public,
        Some(syntax),
        true,
        true,
    )
}

/// Retention checks the original expression's declaration selections before
/// provider closure. Callee implementations remain obligations of the existing
/// checked length continuation; traversing their pending operators here would
/// confuse absence of a realization with a malformed root selection.
pub(super) fn collect_pending_roots(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    roots: &[ExpressionHandle],
    public: bool,
    syntax: &SyntaxTrees,
) -> Result<Custody, String> {
    collect_internal(
        program,
        machine,
        state,
        roots,
        public,
        Some(syntax),
        true,
        false,
    )
}

/// A target-dependent initializer still contains its authored calls: no scalar
/// replacement existed at source resolution. After queries close, retain the
/// exact surviving lexical invocation inventory before private specialization.
/// Collection checks every source selection, including unexecuted branches;
/// this roster grants neither invocation admission nor a selected provider.
pub(crate) fn retain_current_initializer_calls(
    program: &mut TypedTrees,
    owner: symbols::SymbolHandle,
    root: ExpressionHandle,
) -> Result<(), String> {
    let machine = program
        .machine(owner)
        .ok_or("pending initializer lost its custody owner")?;
    let [state] = program.machine_states(machine) else {
        return Err("pending initializer lost its custody state".into());
    };
    let calls =
        collect_internal(program, machine, state, &[root], false, None, false, true)?.call_targets;
    for (reference, selected) in calls {
        let occurrence = program.record_resolved_authored_declaration_selection_once(
            reference,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            Kind::Call,
            selected,
        ).map_err(|reason| format!("pending initializer call custody cannot be retained: {reason:?}"))?;
        program
            .expression_table
            .attach_authored_selection_occurrences(root, [occurrence]);
    }
    Ok(())
}

pub(super) fn validate_retained(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
    materialized: ExpressionHandle,
) -> Result<Vec<DependencyValue>, String> {
    validate(program, machine, state, root, materialized, true)
}

/// Before private specialization, the retained recipe must still select its
/// authored declarations. A different instance of the same template cannot
/// replace that recipe, even if it happens to return the same value.
pub(super) fn validate_authored(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
    materialized: ExpressionHandle,
) -> Result<Vec<DependencyValue>, String> {
    validate(program, machine, state, root, materialized, false)
}

fn validate(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
    materialized: ExpressionHandle,
    specialized_calls: bool,
) -> Result<Vec<DependencyValue>, String> {
    if !program.expression_table.expression_is_valid(materialized) {
        return Err("constant call receipt has a stale materialized root".into());
    }
    let mut expected = Vec::new();
    for occurrence in
        program
            .expression_table
            .authored_selection_occurrences(if specialized_calls {
                materialized
            } else {
                root
            })
    {
        let selection = program
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or("constant call receipt lost an authored selection")?;
        if selection.kind() != Kind::Call {
            continue;
        }
        let selection_target = selection.target();
        let Some(selected) = selection_target.as_resolved() else {
            return Err("constant call receipt has no exact selected target".into());
        };
        let target = (selection.source_span(), selected.selected_symbol());
        if !expected.contains(&target) {
            expected.push(target);
        }
    }
    let actual = collect_internal(
        program,
        machine,
        state,
        &[root],
        false,
        None,
        specialized_calls,
        true,
    )?;
    if actual.call_targets.len() != expected.len()
        || actual
            .call_targets
            .iter()
            .any(|target| !expected.contains(target))
    {
        return Err(
            "retained constant call roster differs from its authored invocation closure".into(),
        );
    }
    Ok(actual.dependency_values)
}

fn collect_internal(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    roots: &[ExpressionHandle],
    public: bool,
    syntax: Option<&SyntaxTrees>,
    specialized_calls: bool,
    expand_callees: bool,
) -> Result<Custody, String> {
    let context = Context {
        machine,
        state,
        initializer: true,
    };
    let mut collector = Collector {
        program,
        syntax,
        public,
        specialized_calls,
        expand_callees,
        custody: Custody::default(),
        expressions: roots.iter().rev().map(|root| (*root, context)).collect(),
        machines: Vec::new(),
        retained_call_targets: Vec::new(),
        expanded_constants: Vec::new(),
    };
    let mut visited_expressions = Vec::new();
    let mut visited_machines = Vec::new();
    loop {
        if let Some((expression, context)) = collector.expressions.pop() {
            if visited_expressions.contains(&(expression, context.initializer)) {
                continue;
            }
            visited_expressions.push((expression, context.initializer));
            collector.expression(expression, context)?;
        } else if let Some(machine) = collector.machines.pop() {
            if visited_machines.contains(&machine) {
                continue;
            }
            visited_machines.push(machine);
            collector.machine(machine)?;
        } else {
            if collector
                .retained_call_targets
                .iter()
                .any(|target| !collector.custody.call_targets.contains(target))
            {
                return Err(
                    "retained call selection has no authored call in its constant closure".into(),
                );
            }
            return Ok(collector.custody);
        }
    }
}

struct Collector<'program> {
    program: &'program TypedTrees,
    syntax: Option<&'program SyntaxTrees>,
    public: bool,
    specialized_calls: bool,
    expand_callees: bool,
    custody: Custody,
    expressions: Vec<(ExpressionHandle, Context<'program>)>,
    machines: Vec<SymbolHandle>,
    retained_call_targets: Vec<(SourceSpan, SymbolHandle)>,
    expanded_constants: Vec<SymbolHandle>,
}

impl<'program> Collector<'program> {
    fn authored_target(
        &self,
        target: SymbolHandle,
        initializer: bool,
    ) -> Result<SymbolHandle, String> {
        // Only detached recipes retain unmodified call tuples. Executable helper
        // bodies may already be specialized by an earlier ordinary checking pass.
        if initializer && !self.specialized_calls {
            return Ok(target);
        }
        let mut target = target;
        let mut visited = Vec::new();
        loop {
            let symbol = self.program.symbols.get(target);
            let owner = match symbol.kind {
                SymbolKind::Machine => target,
                SymbolKind::State => symbol.parent,
                _ => return Ok(target),
            };
            if visited.contains(&owner) {
                return Err("constant call has cyclic executable source custody".into());
            }
            visited.push(owner);
            let Some(template) = self.program.executable_application_template(owner)? else {
                return Ok(target);
            };
            let captures = self
                .program
                .captured_machine_applications
                .iter()
                .filter(|(_, capture)| capture.instance == owner)
                .map(|(_, capture)| capture)
                .collect::<Vec<_>>();
            if let [capture] = captures.as_slice() {
                let entry = if symbol.kind == SymbolKind::Machine {
                    let machine = self
                        .program
                        .machine_by_symbol(owner)
                        .ok_or("constant capture lost its concrete machine")?;
                    self.program
                        .machine_states(machine)
                        .first()
                        .ok_or("constant capture lost its entry state")?
                        .symbol
                } else {
                    target
                };
                let (source_machine, source_state) = self
                    .program
                    .realization_entry_template(entry, &capture.conformance_application)
                    .ok_or("constant capture lost its exact source-state correspondence")?;
                return Ok(if symbol.kind == SymbolKind::Machine {
                    source_machine
                } else {
                    source_state
                });
            }
            if !captures.is_empty() {
                return Err("constant call has ambiguous capture custody".into());
            }
            // Ordinary invocation copies preserve source-state order. Captured
            // states above use their retained exact mapping instead of ordinals.
            if symbol.kind == SymbolKind::Machine {
                target = template;
                continue;
            }
            let concrete = self
                .program
                .machine_by_symbol(owner)
                .ok_or("constant call lost its concrete specialization")?;
            let source = self
                .program
                .machine_by_symbol(template)
                .ok_or("constant call lost its authored template")?;
            let states = self.program.machine_states(concrete);
            let templates = self.program.machine_states(source);
            if states.len() != concrete.states.len() || templates.len() != states.len() {
                return Err("constant call specialization lost its state correspondence".into());
            }
            let ordinal = states
                .iter()
                .position(|state| state.symbol == target)
                .ok_or("constant call has no exact specialization state")?;
            target = templates[ordinal].symbol;
        }
    }

    fn call_selection_matches(
        &self,
        site: &validation::CallSite<'_>,
        context: Context<'program>,
        authored: SymbolHandle,
    ) -> Result<bool, String> {
        let target = call_selections::retained_target(site);
        if authored == self.authored_target(target, context.initializer)? {
            return Ok(true);
        }
        if call_selections::static_callable_matches(
            self.program,
            context.machine.symbol,
            site,
            authored,
        )? {
            return Ok(true);
        }
        call_selections::logical_requirement_matches(
            self.program,
            context.machine.symbol,
            site,
            authored,
        )
    }

    fn selected_call(
        &mut self,
        site: &validation::CallSite<'_>,
        context: Context<'program>,
        authored: SymbolHandle,
        reference: SourceSpan,
    ) -> Result<(), String> {
        let target = call_selections::retained_target(site);
        if authored == self.authored_target(target, context.initializer)? {
            return self.call(target, reference, context.initializer);
        }
        if !self.call_selection_matches(site, context, authored)? {
            return Err("constant call differs from its exact logical selection".into());
        }
        // Retain the source's abstract dependency pair. It supplies no body;
        // the independently checked dispatch identifies the private closure.
        self.call(authored, reference, context.initializer)?;
        if self.expand_callees
            && (site.static_requirement_dispatch().is_some()
                || call_selections::static_callable_matches(
                    self.program,
                    context.machine.symbol,
                    site,
                    authored,
                )?)
        {
            let machine = self
                .program
                .machine_holding_state(target)
                .ok_or("constant logical call lost its selected executable body")?;
            self.machines.push(machine.symbol);
        }
        Ok(())
    }

    fn selected_retained_call(
        &mut self,
        site: validation::CallSite<'_>,
        context: Context<'program>,
        reference: SourceSpan,
        occurrence: Option<typed_trees::AuthoredDeclarationSelectionOccurrenceId>,
    ) -> Result<(), String> {
        let selection = occurrence
            .and_then(|occurrence| {
                self.program
                    .authored_declaration_selections()
                    .get(occurrence)
            })
            .ok_or("constant call lost its authored selection occurrence")?;
        let authored = selection
            .target()
            .as_resolved()
            .ok_or("constant call has no resolved authored target")?
            .selected_symbol();
        if selection.kind() != Kind::Call
            || selection.source_span() != reference
            || !self.call_selection_matches(&site, context, authored)?
        {
            return Err("constant call differs from its exact authored selection".into());
        }
        self.selected_call(&site, context, authored, reference)
    }

    fn retained_call(
        &mut self,
        target: SymbolHandle,
        reference: SourceSpan,
        occurrence: Option<typed_trees::AuthoredDeclarationSelectionOccurrenceId>,
    ) -> Result<(), String> {
        let selection = occurrence
            .and_then(|occurrence| {
                self.program
                    .authored_declaration_selections()
                    .get(occurrence)
            })
            .ok_or("constant call lost its authored selection occurrence")?;
        if selection.kind() != Kind::Call
            || selection.source_span() != reference
            || !matches!(selection.target(), Target::Resolved(selected) if selected.selected_symbol() == self.authored_target(target, false)?)
        {
            return Err("constant call differs from its exact authored selection".into());
        }
        self.call(target, reference, false)
    }

    fn call(
        &mut self,
        target: SymbolHandle,
        reference: SourceSpan,
        initializer: bool,
    ) -> Result<(), String> {
        let executable = target;
        let target = self.authored_target(target, initializer)?;
        let symbol = self.program.symbols.get(target);
        let bound_requirement = symbol.kind == SymbolKind::State
            && symbol.parent.is_valid()
            && self.program.symbols.get(symbol.parent).kind == SymbolKind::ConformanceParameter
            && validation::selected_requirement_signature(self.program, target).is_some();
        if matches!(
            symbol.kind,
            SymbolKind::Operator | SymbolKind::MachineParameter
        ) || bound_requirement
        {
            // An abstract bound call retains the exact public declaration
            // selected by its authored template. It owns no body to evaluate.
            // The ordinary checked specialization/dispatch and interpreter still
            // owe a concrete admitted implementation; this custody row grants
            // neither that selection nor permission to execute it. Boundary
            // operators use the same source-pair retention without a body.
            let declaration = self
                .program
                .symbols
                .symbol_source_span(target)
                .ok_or("constant logical call lost its exact declaration source")?;
            let selection = (reference, declaration);
            if !self.custody.calls.contains(&selection) {
                self.custody.calls.push(selection);
            }
            if !self.custody.call_targets.contains(&(reference, target)) {
                self.custody.call_targets.push((reference, target));
            }
            return Ok(());
        }
        let machine = match symbol.kind {
            SymbolKind::Machine => target,
            SymbolKind::State
                if self.program.symbols.get(symbol.parent).kind == SymbolKind::Machine =>
            {
                symbol.parent
            }
            _ => {
                return Err(
                    "constant call has no exact ordinary machine or state selection".into(),
                );
            }
        };
        let declaration = self
            .program
            .symbols
            .symbol_source_span(target)
            .or_else(|| {
                // A generated entry state has no authored token. Its callable
                // declaration is the owning machine, while the retained selection
                // continues to name this exact entry-state symbol.
                if symbol.kind != SymbolKind::State {
                    return None;
                }
                let mut owners = self
                    .program
                    .machines()
                    .iter()
                    .filter(|owner| owner.symbol == machine);
                let owner = owners.next()?;
                if owners.next().is_some() {
                    return None;
                }
                let states = self.program.machine_states(owner);
                if states.len() != owner.states.len() || states.first()?.symbol != target {
                    return None;
                }
                self.program.symbols.symbol_source_span(machine)
            })
            .ok_or("constant call lost its exact selected declaration source")?;
        let selection = (reference, declaration);
        if !self.custody.calls.contains(&selection) {
            self.custody.calls.push(selection);
        }
        if !self.custody.call_targets.contains(&(reference, target)) {
            self.custody.call_targets.push((reference, target));
        }
        let executable_symbol = self.program.symbols.get(executable);
        if self.expand_callees {
            self.machines
                .push(if executable_symbol.kind == SymbolKind::State {
                    executable_symbol.parent
                } else {
                    executable
                });
        }
        Ok(())
    }

    fn constant(
        &mut self,
        symbol: SymbolHandle,
        reference: SourceSpan,
        expression: ExpressionHandle,
        context: Context<'program>,
    ) -> Result<(), String> {
        let mut declarations = self
            .program
            .const_declarations()
            .iter()
            .filter(|declaration| declaration.symbol == symbol);
        let declaration = declarations
            .next()
            .ok_or("constant selection lost its declaration")?;
        if declarations.next().is_some() {
            return Err("constant selection has ambiguous declaration custody".into());
        }
        let source = self
            .program
            .symbols
            .symbol_source_span(symbol)
            .ok_or("constant selection lost its declaration source")?;
        if !declaration.is_public
            && ((self.public
                && context.initializer
                && !self
                    .syntax
                    .is_some_and(|syntax| syntax.constant_initializer_owns_selection(reference)))
                || !self.program.symbols.same_source_package(reference, source))
        {
            return Err(
                "private constant declaration cannot be selected by this initializer owner".into(),
            );
        }
        // Read the current concrete declaration, never copy a caller's expected
        // origin into this result. Readiness comparison happens before execution.
        let origin = ConstArgumentOrigin {
            reference,
            declaration: source,
            initializer: declaration.initializer_source_span,
            evaluated_value: declaration
                .evaluated_value
                .clone()
                .ok_or("selected constant has no ready canonical value")?,
        };
        if !self.custody.origins.contains(&origin) {
            self.custody.origins.push(origin);
        }
        // Preparation may still contain provisional declaration values. Only
        // the receiving traversal asks replay to validate materialized subjects.
        if self.syntax.is_none() {
            let table = &self.program.expression_table;
            let materialized = declaration.materialized_initializer;
            if !table.expression_is_valid(materialized)
                || table.source_span(materialized) != declaration.initializer_source_span
            {
                return Err("selected constant lost its exact materialized initializer".into());
            }
            let subject = DependencyValue {
                declaration: symbol,
                expression: materialized,
            };
            if !self.custody.dependency_values.contains(&subject) {
                self.custody.dependency_values.push(subject);
            }
            // A larger computed root inherits dependency occurrences, but its
            // value is not the dependency's value. Exact token equality selects
            // only the original use which was replaced by that constant.
            if table.expression_is_valid(expression) && table.source_span(expression) == reference {
                let subject = DependencyValue {
                    declaration: symbol,
                    expression,
                };
                if !self.custody.dependency_values.contains(&subject) {
                    self.custody.dependency_values.push(subject);
                }
            }
        }
        if self.expanded_constants.contains(&symbol) {
            return Ok(());
        }
        self.expanded_constants.push(symbol);
        self.type_dependencies(types::Root::Type(declaration.declared_type), context)?;
        if self.specialized_calls && declaration.authored_initializer.is_valid() {
            // This ready declaration owns another normalization job. Its
            // materialized capsule supplies that job's retained realized roster;
            // receiving replay validates every such owner before evaluating any
            // dependency value. Never specialize its detached source recipe as
            // though it were the current invocation's body.
            let materialized = declaration.materialized_initializer;
            if !self
                .program
                .expression_table
                .expression_is_valid(materialized)
                || self.program.expression_table.source_span(materialized)
                    != declaration.initializer_source_span
            {
                return Err("constant dependency lost its owned materialized custody".into());
            }
            let calls = self
                .program
                .expression_table
                .authored_selection_occurrences(materialized)
                .map(|occurrence| {
                    let selection = self
                        .program
                        .authored_declaration_selections()
                        .get(occurrence)
                        .ok_or("constant dependency lost its retained selection")?;
                    if selection.kind() != Kind::Call {
                        return Ok(None);
                    }
                    let target = selection
                        .target()
                        .as_resolved()
                        .ok_or("constant dependency retained an unresolved call")?
                        .selected_symbol();
                    Ok(Some((selection.source_span(), target)))
                })
                .collect::<Result<Vec<_>, String>>()?;
            for (reference, target) in calls.into_iter().flatten() {
                self.call(target, reference, false)?;
            }
            self.expressions.push((
                materialized,
                Context {
                    initializer: true,
                    ..context
                },
            ));
        } else if declaration.authored_initializer.is_valid() {
            self.expressions.push((
                declaration.authored_initializer,
                Context {
                    initializer: true,
                    ..context
                },
            ));
        }
        Ok(())
    }

    fn expression(
        &mut self,
        expression: ExpressionHandle,
        context: Context<'program>,
    ) -> Result<(), String> {
        let program = self.program;
        let table = &program.expression_table;
        if !table.expression_is_valid(expression) {
            return Err("constant call closure contains a stale expression".into());
        }
        self.type_dependencies(types::Root::Expression(expression), context)?;
        for occurrence in table.authored_selection_occurrences(expression) {
            if !self.specialized_calls {
                let reference = table.source_span(expression);
                let inherited = table
                    .authored_selection_occurrences(expression)
                    .any(|direct| {
                        program
                            .authored_declaration_selections()
                            .get(direct)
                            .is_some_and(|selection| {
                                if selection.source_span() != reference {
                                    return false;
                                }
                                let selected_target = selection.target();
                                let Some(selected) = selected_target.as_resolved() else {
                                    return false;
                                };
                                let Some(owner) = program
                                    .const_declarations()
                                    .iter()
                                    .find(|owner| owner.symbol == selected.selected_symbol())
                                else {
                                    return false;
                                };
                                owner.materialized_initializer.is_valid()
                                    && table
                                        .authored_selection_occurrences(
                                            owner.materialized_initializer,
                                        )
                                        .any(|owned| owned == occurrence)
                            })
                    });
                if inherited {
                    continue;
                }
            }
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("constant call closure lost an authored selection")?;
            if selection.kind() == Kind::Call {
                let selection_target = selection.target();
                let Some(selected) = selection_target.as_resolved() else {
                    return Err("constant closure retained an unresolved call selection".into());
                };
                let mut symbol = selected.selected_symbol();
                // The ledger names the trait requirement. Execution and the
                // dependency roster name the supplying row's entry. Retain
                // that entry for this call's own token; nested selections
                // keep the declaration they were recorded under.
                if selection.source_span() == table.source_span(expression)
                    && let ExpressionNode::Call(call) = table.expression(expression)
                {
                    if let Some(entry) =
                        call_selections::owned_realization_entry(program, call, symbol)
                    {
                        symbol = entry;
                    } else if call_selections::requirement_realized_by(
                        program,
                        symbol,
                        call.target_symbol,
                    ) {
                        symbol = call.target_symbol;
                    }
                }
                let target = (selection.source_span(), symbol);
                if !self.retained_call_targets.contains(&target) {
                    self.retained_call_targets.push(target);
                }
            } else if selection.kind() == Kind::Operator {
                // Helper operators remain ordinary checked body evidence, not
                // initializer builtin-operator receipts.
                if !context.initializer
                    && !self.syntax.is_some_and(|syntax| {
                        syntax.constant_initializer_owns_selection(selection.source_span())
                    })
                {
                    continue;
                }
                let builtin = super::has_builtin_operator_selection(
                    program,
                    context.machine,
                    context.state,
                    expression,
                    selection.target(),
                );
                if !builtin {
                    return Err(
                        "evaluated initializer operator has no checked builtin meaning".into(),
                    );
                }
                if !self.custody.operators.contains(&selection.source_span()) {
                    self.custody.operators.push(selection.source_span());
                }
            } else if let Target::Resolved(selected) = selection.target()
                && program.symbols.get(selected.selected_symbol()).kind == SymbolKind::Const
            {
                // A detached recipe can retain both the terminal path segment
                // and the substitution's full-path selection, even after its
                // Name became an aggregate literal. Join that segment only to
                // an attached direct use of the exact same declaration. Other
                // inherited origins keep their original occurrence identities.
                let source = table.source_span(expression);
                let terminal = selection.source_span();
                let leaf = program
                    .symbols
                    .name(selected.selected_symbol())
                    .rsplit("::")
                    .next()
                    .unwrap_or_default();
                let direct_segment = terminal.source_id == source.source_id
                    && terminal.span.end == source.span.end
                    && source.span.end.checked_sub(leaf.len()) == Some(terminal.span.start)
                    && terminal.span.start >= source.span.start
                    && table
                        .authored_selection_occurrences(expression)
                        .any(|occurrence| {
                            program
                                .authored_declaration_selections()
                                .get(occurrence)
                                .is_some_and(|other| {
                                    other.source_span() == source
                                        && matches!(other.target(), Target::Resolved(other)
                                    if other.selected_symbol() == selected.selected_symbol())
                                })
                        });
                let reference = if direct_segment { source } else { terminal };
                self.constant(selected.selected_symbol(), reference, expression, context)?;
            }
        }
        let children = expression_children(program, expression)?;
        match table.expression(expression) {
            ExpressionNode::Call(call) => {
                self.static_arguments(&call.machine_arguments);
                if call.authored_machine_application.is_valid() {
                    self.static_arguments(
                        table.static_machine_application_arguments(
                            call.authored_machine_application,
                        ),
                    );
                    self.static_arguments(table.static_machine_application_selected_machines(
                        call.authored_machine_application,
                    ));
                }
                let site = validation::CallSite::Expression { expression, call };
                let mut selections = Vec::new();
                for occurrence in table.authored_selection_occurrences(expression) {
                    let Some(selection) = program.authored_declaration_selections().get(occurrence)
                    else {
                        return Err("constant call lost an attached authored selection".into());
                    };
                    if selection.kind() != Kind::Call {
                        continue;
                    }
                    // A retained recipe root also carries its transitive
                    // authored inventory. Only this expression's own source
                    // point annotates its call; nested occurrences are checked
                    // when their exact helper or dependency is traversed.
                    let reference = selection.source_span();
                    if reference != table.source_span(expression) {
                        continue;
                    }
                    let Some(target) = selection
                        .target()
                        .as_resolved()
                        .map(|target| target.selected_symbol())
                    else {
                        continue;
                    };
                    let matches = self.call_selection_matches(&site, context, target)?;
                    // Preparation may rewrite the call to the row's entry
                    // after the ledger recorded the trait requirement.
                    let symbol = if call_selections::owned_realization_entry(program, call, target)
                        == Some(call.target_symbol)
                        || call_selections::requirement_realized_by(
                            program,
                            target,
                            call.target_symbol,
                        ) {
                        call.target_symbol
                    } else if matches {
                        target
                    } else {
                        continue;
                    };
                    let selected = (symbol, selection.source_span());
                    if !selections.contains(&selected) {
                        selections.push(selected);
                    }
                }
                let [(authored, reference)] = selections.as_slice() else {
                    return Err(if selections.is_empty() {
                        "constant call lost its exact authored target selection"
                    } else {
                        "constant call has ambiguous authored target selections"
                    }
                    .into());
                };
                // A trait requirement names no body. A supplying map's row
                // entry is the call custody executes. A map that does not
                // supply the trait rejects here, before that body is walked.
                if let Some(entry) =
                    call_selections::owned_realization_entry(program, call, *authored)
                {
                    self.call(entry, *reference, context.initializer)?;
                } else if let Some(message) =
                    call_selections::unsupplied_requirement(program, call, *authored)
                {
                    return Err(message);
                } else {
                    self.selected_call(&site, context, *authored, *reference)?;
                }
            }
            ExpressionNode::Name(name)
                if program.symbols.get(name.symbol).kind == SymbolKind::Const =>
            {
                self.constant(
                    name.symbol,
                    table.source_span(expression),
                    ExpressionHandle::invalid(),
                    context,
                )?;
            }
            _ => {}
        }
        self.expressions
            .extend(children.into_iter().rev().map(|child| (child, context)));
        Ok(())
    }

    fn machine(&mut self, symbol: SymbolHandle) -> Result<(), String> {
        let program = self.program;
        let mut machines = program
            .machines()
            .iter()
            .filter(|machine| machine.symbol == symbol);
        let machine = machines
            .next()
            .ok_or("constant call closure lost its selected machine")?;
        if machines.next().is_some() {
            return Err("constant call closure has ambiguous machine identity".into());
        }
        let states = program.machine_states(machine);
        if states.len() != machine.states.len() {
            return Err("constant call closure has stale states".into());
        }
        let entry = states
            .first()
            .ok_or("constant call closure has no entry state")?;
        self.type_dependencies(
            types::Root::Machine(machine),
            Context {
                machine,
                state: entry,
                initializer: false,
            },
        )?;
        let owned = program.machine_owned_data(machine);
        if owned.len() != machine.owned_data.len() {
            return Err("constant call closure has stale owned initializers".into());
        }
        for owned in owned {
            if owned.initial_value.is_valid() {
                self.expressions.push((
                    owned.initial_value,
                    Context {
                        machine,
                        state: entry,
                        initializer: false,
                    },
                ));
            }
        }
        let table = &program.statement_table;
        for state in states {
            let context = Context {
                machine,
                state,
                initializer: false,
            };
            let body = table.statements(state.statement_nodes);
            if body.len() != state.statement_nodes.len() {
                return Err("constant call closure has stale statements".into());
            }
            for statement in body {
                let mut expressions = Vec::new();
                match statement {
                    StatementNode::Assembly(_) => {}
                    StatementNode::AssemblyFact(fact) => {
                        expressions.extend(program.tables.proof_fact_expression_roots(fact.fact))
                    }
                    StatementNode::Expression(expression) => expressions.push(*expression),
                    StatementNode::LocalData(local) => {
                        if local.initial_value.is_valid() {
                            expressions.push(local.initial_value);
                        }
                    }
                    StatementNode::Assignment(assignment) => {
                        expressions.extend([assignment.target, assignment.value])
                    }
                    StatementNode::Call(call) => {
                        self.static_arguments(&call.machine_arguments);
                        if call.authored_machine_application.is_valid() {
                            self.static_arguments(
                                program
                                    .expression_table
                                    .static_machine_application_arguments(
                                        call.authored_machine_application,
                                    ),
                            );
                            self.static_arguments(
                                program
                                    .expression_table
                                    .static_machine_application_selected_machines(
                                        call.authored_machine_application,
                                    ),
                            );
                        }
                        self.selected_retained_call(
                            validation::CallSite::Statement(call),
                            context,
                            call.source_span,
                            call.authored_call_selection,
                        )?;
                        let arguments = table.expression_handles(call.arguments);
                        if arguments.len() != call.arguments.len() {
                            return Err("constant statement call has stale arguments".into());
                        }
                        expressions.extend(arguments.iter().copied());
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = transition.guard {
                            expressions.push(guard);
                        }
                        for target in transition.targets() {
                            if !table.transition_target_is_valid(target) {
                                return Err("constant call closure has stale transition".into());
                            }
                            match table.transition_target(target) {
                                TransitionTargetNode::Named {
                                    path,
                                    arguments,
                                    source_span,
                                    authored_call_selection,
                                    ..
                                } => {
                                    self.retained_call(
                                        path.symbol,
                                        *source_span,
                                        *authored_call_selection,
                                    )?;
                                    let values = table.expression_handles(*arguments);
                                    if values.len() != arguments.len() {
                                        return Err(
                                            "constant transition call has stale arguments".into()
                                        );
                                    }
                                    expressions.extend(values.iter().copied());
                                }
                                TransitionTargetNode::Value(value) => expressions.push(*value),
                                TransitionTargetNode::Terminal => {}
                            }
                        }
                    }
                }
                self.expressions.extend(
                    expressions
                        .into_iter()
                        .rev()
                        .map(|expression| (expression, context)),
                );
            }
        }
        Ok(())
    }

    fn static_arguments(&mut self, arguments: &[typed_trees::expression::StaticMachineArgument]) {
        if !self.expand_callees {
            return;
        }
        for argument in arguments {
            let selected = self.program.symbols.get(argument.symbol);
            match selected.kind {
                SymbolKind::Machine => self.machines.push(argument.symbol),
                SymbolKind::State
                    if self.program.symbols.get(selected.parent).kind == SymbolKind::Machine =>
                {
                    self.machines.push(selected.parent);
                }
                _ => {}
            }
            if let Some(application) = &argument.application {
                self.static_arguments(&application.arguments);
            }
        }
    }
}
