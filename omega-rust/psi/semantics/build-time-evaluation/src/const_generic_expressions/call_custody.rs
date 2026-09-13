//! Rejoin a scalar initializer with its complete selected ordinary call closure.

use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic as Intrinsic, AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionTarget as Target,
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

pub(super) fn contains_call(program: &TypedTrees, root: ExpressionHandle) -> Result<bool, String> {
    let mut pending = vec![(root, false)];
    let mut active = Vec::new();
    let mut complete = Vec::new();
    let mut contains_call = false;
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
        contains_call |= matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Call(_)
        );
        active.push(expression);
        pending.push((expression, true));
        pending.extend(children.into_iter().rev().map(|child| (child, false)));
    }
    Ok(contains_call)
}

/// Shared ordinary expression edges for custody and demand. This establishes
/// arena shape only; declaration and invocation admission remain separate.
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
    collect_internal(program, machine, state, root, public, Some(syntax))
}

pub(super) fn validate_retained(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
    materialized: ExpressionHandle,
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
        let Target::Resolved(selected) = selection.target() else {
            return Err("constant call receipt has no exact selected target".into());
        };
        let target = (selection.source_span(), selected.selected_symbol());
        if !expected.contains(&target) {
            expected.push(target);
        }
    }
    let actual = collect_internal(program, machine, state, root, false, None)?;
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
    root: ExpressionHandle,
    public: bool,
    syntax: Option<&SyntaxTrees>,
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
        custody: Custody::default(),
        expressions: vec![(root, context)],
        machines: Vec::new(),
        retained_call_targets: Vec::new(),
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
    custody: Custody,
    expressions: Vec<(ExpressionHandle, Context<'program>)>,
    machines: Vec<SymbolHandle>,
    retained_call_targets: Vec<(SourceSpan, SymbolHandle)>,
}

impl<'program> Collector<'program> {
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
            || !matches!(selection.target(), Target::Resolved(selected) if selected.selected_symbol() == target)
        {
            return Err("constant call differs from its exact authored selection".into());
        }
        self.call(target, reference)
    }

    fn call(&mut self, target: SymbolHandle, reference: SourceSpan) -> Result<(), String> {
        let symbol = self.program.symbols.get(target);
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
        self.machines.push(machine);
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
            canonical_value_encoding: declaration
                .canonical_value_encoding
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
        for occurrence in table.authored_selection_occurrences(expression) {
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("constant call closure lost an authored selection")?;
            if selection.kind() == Kind::Call {
                let Target::Resolved(selected) = selection.target() else {
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
                let builtin =
                    matches!(
                        selection.target(),
                        Target::Intrinsic(Intrinsic::BuiltinOperator)
                    ) || matches!(table.expression(expression), ExpressionNode::Binary(_))
                        && validation::has_builtin_binary_expression_meaning(
                            program,
                            context.machine,
                            Some(context.state),
                            expression,
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
                self.constant(
                    selected.selected_symbol(),
                    selection.source_span(),
                    expression,
                    context,
                )?;
            }
        }
        let children = expression_children(program, expression)?;
        match table.expression(expression) {
            ExpressionNode::Call(call) => {
                let mut selections = table
                    .authored_selection_occurrences(expression)
                    .filter_map(|occurrence| {
                        program.authored_declaration_selections().get(occurrence)
                    })
                    .filter(|selection| {
                        selection.kind() == Kind::Call
                            && matches!(selection.target(), Target::Resolved(selected)
                            if selected.selected_symbol() == call.target_symbol)
                    });
                let reference = selections
                    .next()
                    .ok_or("constant call lost its exact authored target selection")?
                    .source_span();
                if selections.next().is_some() {
                    return Err("constant call has ambiguous authored target selections".into());
                }
                self.call(call.target_symbol, reference)?;
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
                    StatementNode::RootBinding(binding) => expressions.push(binding.receiver),
                    StatementNode::AssemblyFact(fact) => expressions.push(fact.expression),
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
                        for target in [transition.target, transition.continuation]
                            .into_iter()
                            .filter(|handle| handle.is_valid())
                        {
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
                                TransitionTargetNode::SelfTarget
                                | TransitionTargetNode::Terminal => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_closure_keeps_helper_constants_and_calls_without_reclassifying_body_operators() {
        let source = "const BASE: u8 = 7;
            machine probe() -> u8 { match true { _ -> helper() } }
            machine helper() -> u8 { touch(); BASE + 0u8 }
            machine touch() { let ignored: u8 = BASE; }";
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokens");
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add(std::path::PathBuf::from("main.omg"), source.to_owned())
            .source_id;
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).expect("syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
            &syntax,
            std::sync::Arc::new(sources),
        )
        .expect("resolved");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("typed");
        let checked =
            typed_trees_to_checked_trees::lower_typed_trees(typed).expect("checked closure");
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "probe")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let [StatementNode::Expression(expression)] =
            program.statement_table.statements(state.statement_nodes)
        else {
            panic!("probe source");
        };
        let custody =
            collect(program, machine, state, *expression, false, &syntax).expect("exact closure");
        assert_eq!(custody.calls.len(), 2, "expression and statement calls");
        assert_eq!(
            custody.origins.len(),
            2,
            "both helper-owned constant references"
        );
        assert!(
            custody.operators.is_empty(),
            "helper body operators are not initializer receipts"
        );
        let declaration = &program.const_declarations()[0];
        for origin in &custody.origins {
            assert_eq!(
                Some(origin.declaration),
                program.symbols.symbol_source_span(declaration.symbol)
            );
            assert_eq!(
                Some(&origin.canonical_value_encoding),
                declaration.canonical_value_encoding.as_ref()
            );
        }
        let mut changed = program.clone();
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(*expression)
        else {
            panic!("probe dispatch");
        };
        let call_expression = program.expression_table.match_arms(dispatch.arms)[0].value;
        let ExpressionNode::Call(call) = changed.expression_table.expression_mut(call_expression)
        else {
            panic!("probe call");
        };
        call.target_symbol = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "touch")
            .unwrap()
            .symbol;
        assert!(
            collect(&changed, machine, state, *expression, false, &syntax).is_err(),
            "a changed callee cannot reuse the original authored selection"
        );
        let mut retained = program.clone();
        let materialized = retained.expression_table.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(7),
        ));
        let occurrences = program.authored_declaration_selections().iter()
            .filter(|selection| selection.kind() == Kind::Call
                && matches!(selection.target(), Target::Resolved(selected)
                    if custody.call_targets.contains(&(selection.source_span(), selected.selected_symbol()))))
            .map(|selection| selection.occurrence_id())
            .collect::<Vec<_>>();
        retained
            .expression_table
            .attach_authored_selection_occurrences(materialized, occurrences);
        assert_eq!(contains_call(&retained, *expression), Ok(true));
        assert_eq!(
            contains_call(&retained, materialized),
            Ok(false),
            "receipt is not an executable call node"
        );
        let elements = retained
            .expression_table
            .insert_expression_handles([*expression, materialized]);
        let aggregate = retained
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(elements));
        assert_eq!(
            contains_call(&retained, aggregate),
            Ok(true),
            "aggregate initializer children retain demand"
        );
        for replacement in [ExpressionHandle::invalid(), aggregate] {
            let mut malformed = retained.clone();
            malformed
                .expression_table
                .set_expression_handle_at_offset(elements, 1, replacement);
            assert!(
                contains_call(&malformed, aggregate).is_err(),
                "finding a call cannot skip a stale or cyclic sibling"
            );
        }
        let mut malformed = retained.clone();
        *malformed.expression_table.expression_mut(aggregate) =
            ExpressionNode::ArrayLiteral(arena::HandleSpan::from_parts(
                arena::Handle::from_parts(
                    elements.start().arena_index(),
                    elements.start().generation() + 1,
                ),
                elements.count(),
            ));
        assert!(
            contains_call(&malformed, aggregate).is_err(),
            "stale aggregate span rejects"
        );
        validate_retained(&retained, machine, state, *expression, materialized)
            .expect("receiving replay needs only the retained typed roots");
        assert!(
            validate_retained(&retained, machine, state, *expression, call_expression).is_err(),
            "a receipt omitting the helper's statement call rejects"
        );
        *retained.expression_table.expression_mut(call_expression) =
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(7));
        assert!(
            collect_internal(&retained, machine, state, *expression, false, None).is_err(),
            "erasing the call cannot leave its authored occurrence unaccounted for"
        );
        assert!(
            validate_retained(&retained, machine, state, *expression, materialized).is_err(),
            "same-valued materialization does not authorize a removed original call"
        );
    }
}
