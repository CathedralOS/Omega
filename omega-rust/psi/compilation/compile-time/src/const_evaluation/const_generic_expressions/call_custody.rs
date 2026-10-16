//! Rejoin a scalar initializer with its complete selected ordinary call closure.

use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
};
use source::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};
use syntax_trees::{SyntaxTrees, types::ConstArgumentOrigin};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode, MatchPattern},
    machine::Machine,
    state::State,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
};

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
                if let MatchPattern::Value(pattern) = arm.pattern {
                    children.push(pattern);
                }
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
    for occurrence in program
        .expression_table
        .authored_selection_occurrences(materialized)
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
        let symbol = self.program.symbols.get(target);
        let owner = match symbol.kind {
            SymbolKind::Machine => target,
            SymbolKind::State => symbol.parent,
            _ => return Ok(target),
        };
        let mut instances = self
            .program
            .machine_specializations
            .iter()
            .filter(|instance| instance.instance == owner);
        let Some(instance) = instances.next() else {
            return Ok(target);
        };
        if instances.next().is_some() {
            return Err("constant call has ambiguous specialization custody".into());
        }
        if symbol.kind == SymbolKind::Machine {
            return Ok(instance.template);
        }
        let concrete = self
            .program
            .machine(instance.instance)
            .ok_or("constant call lost its concrete specialization")?;
        let template = self
            .program
            .machine(instance.template)
            .ok_or("constant call lost its authored template")?;
        let states = self.program.machine_states(concrete);
        let templates = self.program.machine_states(template);
        if states.len() != concrete.states.len() || templates.len() != states.len() {
            return Err("constant call specialization lost its state correspondence".into());
        }
        let ordinal = states
            .iter()
            .position(|state| state.symbol == target)
            .ok_or("constant call has no exact specialization state")?;
        Ok(templates[ordinal].symbol)
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
        if symbol.kind == SymbolKind::Operator {
            // A named boundary operator (`F32::square_root`) has no machine
            // body to traverse, but the call still owes its exact authored
            // declaration pair: the interpreter's named-requirement route
            // supplies the value while custody keeps the occurrence/operator
            // join for receipt comparison.
            let declaration = self
                .program
                .symbols
                .symbol_source_span(target)
                .ok_or("constant operator call lost its exact declaration source")?;
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
        if declaration.authored_initializer.is_valid() {
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
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("constant call closure lost an authored selection")?;
            if selection.kind() == Kind::Call {
                let selection_target = selection.target();
                let Some(selected) = selection_target.as_resolved() else {
                    return Err("constant closure retained an unresolved call selection".into());
                };
                let target = (selection.source_span(), selected.selected_symbol());
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
                let authored_target =
                    self.authored_target(call.target_symbol, context.initializer)?;
                let mut selections = table
                    .authored_selection_occurrences(expression)
                    .filter_map(|occurrence| {
                        program.authored_declaration_selections().get(occurrence)
                    })
                    .filter(|selection| {
                        selection.kind() == Kind::Call
                            && matches!(selection.target(), Target::Resolved(selected)
                            if selected.selected_symbol() == authored_target)
                    });
                let reference = selections
                    .next()
                    .ok_or("constant call lost its exact authored target selection")?
                    .source_span();
                if selections.next().is_some() {
                    return Err("constant call has ambiguous authored target selections".into());
                }
                self.call(call.target_symbol, reference, context.initializer)?;
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
                    StatementNode::RootBinding(binding) => {
                        expressions.push(binding.receiver);
                        if binding.implementation_operand.is_valid() {
                            expressions.push(binding.implementation_operand);
                        }
                    }
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
                        self.retained_call(
                            call.target_symbol,
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
}
