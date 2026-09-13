//! Declaration-owned dependency discovery before provisional values exist.

use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use source::SourceSpan;
use symbol_resolved_trees::{
    SymbolResolvedTrees,
    expression::{ExpressionHandle, ExpressionNode, MatchPattern},
};
use symbols::{SymbolHandle, SymbolKind};

/// Exact source occurrence/declaration pairs. Body dependencies are retained
/// even when execution later selects another branch.
#[derive(Debug, Default)]
pub struct ConstInitializerDependencies {
    pub constants: Vec<(SourceSpan, SourceSpan)>,
    pub calls: Vec<(SourceSpan, SourceSpan)>,
    pub(crate) call_targets: Vec<(SourceSpan, SymbolHandle)>,
}

pub(crate) fn collect(
    program: &SymbolResolvedTrees,
    root: ExpressionHandle,
) -> Result<ConstInitializerDependencies, String> {
    let mut collector = Collector {
        program,
        dependencies: ConstInitializerDependencies::default(),
        expressions: vec![root],
        machines: Vec::new(),
        visited_expressions: Vec::new(),
        visited_machines: Vec::new(),
        follow_dependencies: true,
    };
    while !collector.expressions.is_empty() || !collector.machines.is_empty() {
        if let Some(expression) = collector.expressions.pop() {
            if collector.visited_expressions.contains(&expression) {
                continue;
            }
            collector.visited_expressions.push(expression);
            collector.expression(expression)?;
        } else if let Some(machine) = collector.machines.pop() {
            if collector.visited_machines.contains(&machine) {
                continue;
            }
            collector.visited_machines.push(machine);
            collector.machine(machine)?;
        }
    }
    Ok(collector.dependencies)
}

pub(crate) fn expression_at_source(
    program: &SymbolResolvedTrees,
    root: ExpressionHandle,
    source: SourceSpan,
) -> Result<ExpressionHandle, String> {
    let mut collector = Collector {
        program,
        dependencies: ConstInitializerDependencies::default(),
        expressions: vec![root],
        machines: Vec::new(),
        visited_expressions: Vec::new(),
        visited_machines: Vec::new(),
        follow_dependencies: false,
    };
    let mut selected = None;
    while let Some(expression) = collector.expressions.pop() {
        if collector.visited_expressions.contains(&expression) {
            continue;
        }
        collector.visited_expressions.push(expression);
        if program.tables.bodies.expressions.source_span(expression) == source {
            if selected.is_some() {
                return Err("initializer leaf source correspondence is ambiguous".to_owned());
            }
            selected = Some(expression);
        }
        collector.expression(expression)?;
    }
    selected.ok_or_else(|| "initializer leaf is absent from its retained original root".to_owned())
}

struct Collector<'program> {
    program: &'program SymbolResolvedTrees,
    dependencies: ConstInitializerDependencies,
    expressions: Vec<ExpressionHandle>,
    machines: Vec<SymbolHandle>,
    visited_expressions: Vec<ExpressionHandle>,
    visited_machines: Vec<SymbolHandle>,
    follow_dependencies: bool,
}

impl Collector<'_> {
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
                    "constant initializer call has no exact ordinary machine declaration"
                        .to_owned(),
                );
            }
        };
        let declaration = if let Some(source) = self.program.symbols.symbol_source_span(target) {
            source
        } else {
            // The parser's implicit entry has no authored token. Its exact
            // first-state ownership identifies the machine declaration which
            // the source selected; another state cannot inherit this identity.
            let mut owners = self
                .program
                .machines
                .iter()
                .filter(|owner| owner.symbol == machine);
            let owner = owners
                .next()
                .ok_or("implicit constant entry lost its owner")?;
            let states = self.program.machine_state_handles(owner.states);
            if owners.next().is_some()
                || symbol.kind != SymbolKind::State
                || states.len() != owner.states.len()
                || !states
                    .first()
                    .is_some_and(|state| self.program.machine_state(*state).symbol == target)
            {
                return Err("source-less constant target is not its exact owning entry".to_owned());
            }
            self.program
                .symbols
                .symbol_source_span(machine)
                .ok_or("implicit constant entry lost its authored machine declaration")?
        };
        let selection = (reference, declaration);
        if !self.dependencies.calls.contains(&selection) {
            self.dependencies.calls.push(selection);
        }
        if !self
            .dependencies
            .call_targets
            .contains(&(reference, target))
        {
            self.dependencies.call_targets.push((reference, target));
        }
        if self.follow_dependencies {
            self.machines.push(machine);
        }
        Ok(())
    }

    fn expression(&mut self, expression: ExpressionHandle) -> Result<(), String> {
        let program = self.program;
        let table = &program.tables.bodies.expressions;
        if !table.expression_is_valid(expression) {
            return Err("constant initializer dependency has a stale expression".to_owned());
        }
        for occurrence in table.authored_selection_occurrences(expression) {
            let selection = self
                .program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("constant initializer lost its declaration selection")?;
            let AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target() else {
                continue;
            };
            if self.program.symbols.get(selected.selected_symbol()).kind != SymbolKind::Const {
                continue;
            }
            let declaration = self
                .program
                .const_declarations
                .iter()
                .find(|declaration| declaration.symbol == selected.selected_symbol())
                .ok_or("constant dependency lost its retained declaration")?;
            let source = self
                .program
                .symbols
                .symbol_source_span(declaration.symbol)
                .ok_or("constant dependency lost its declaration source")?;
            let dependency = (selection.source_span(), source);
            if !self.dependencies.constants.contains(&dependency) {
                self.dependencies.constants.push(dependency);
            }
            if self.follow_dependencies {
                self.expressions
                    .push(if declaration.authored_initializer.is_valid() {
                        declaration.authored_initializer
                    } else {
                        declaration.initializer
                    });
            }
        }
        match table.expression(expression) {
            ExpressionNode::Call(call) => {
                self.call(call.target_symbol, call.target.source_span())?;
                if call.receiver.is_valid() {
                    self.expressions.push(call.receiver);
                }
                let arguments = table.expression_handles(call.arguments);
                if arguments.len() != call.arguments.len() {
                    return Err("constant call arguments have a stale span".to_owned());
                }
                self.expressions.extend(arguments.iter().rev().copied());
            }
            ExpressionNode::Binary(binary) => self.expressions.extend([binary.right, binary.left]),
            ExpressionNode::Unary(unary) => self.expressions.push(unary.operand),
            ExpressionNode::Cast(cast) => self.expressions.push(cast.value),
            ExpressionNode::Member(member) => self.expressions.push(member.receiver),
            ExpressionNode::Borrow(borrow) => self.expressions.push(borrow.target),
            ExpressionNode::Indexed(indexed) => {
                self.expressions.extend([indexed.index, indexed.collection])
            }
            ExpressionNode::Membership(membership) => self.expressions.push(membership.value),
            ExpressionNode::Atomic(atomic) => {
                self.expressions.extend([atomic.result, atomic.value])
            }
            ExpressionNode::Range(range) => self.expressions.extend(
                [range.end, range.start]
                    .into_iter()
                    .filter(|handle| handle.is_valid()),
            ),
            ExpressionNode::ArrayLiteral(elements) => {
                let values = table.expression_handles(*elements);
                if values.len() != elements.len() {
                    return Err("constant array dependency has a stale span".to_owned());
                }
                self.expressions.extend(values.iter().rev().copied());
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = table.struct_fields(literal.fields);
                if fields.len() != literal.fields.len() {
                    return Err("constant constructor dependency has a stale span".to_owned());
                }
                self.expressions
                    .extend(fields.iter().rev().map(|field| field.value));
            }
            ExpressionNode::Match(dispatch) => {
                let arms = table.match_arms(dispatch.arms);
                if arms.len() != dispatch.arms.len() {
                    return Err("constant dispatch dependency has a stale span".to_owned());
                }
                for arm in arms.iter().rev() {
                    self.expressions.push(arm.value);
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        self.expressions.push(pattern);
                    }
                }
                self.expressions.push(dispatch.subject);
            }
            ExpressionNode::Name(_) => {
                if let Some((reference, selected)) =
                    super::selected_expression_constant(program, expression)
                {
                    let declaration = program
                        .const_declarations
                        .iter()
                        .find(|declaration| declaration.symbol == selected)
                        .ok_or("body constant lost its declaration")?;
                    let source = program
                        .symbols
                        .symbol_source_span(selected)
                        .ok_or("body constant lost its source")?;
                    let dependency = (reference, source);
                    if !self.dependencies.constants.contains(&dependency) {
                        self.dependencies.constants.push(dependency);
                    }
                    if self.follow_dependencies {
                        self.expressions
                            .push(if declaration.authored_initializer.is_valid() {
                                declaration.authored_initializer
                            } else {
                                declaration.initializer
                            });
                    }
                }
            }
            ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
        Ok(())
    }

    fn machine(&mut self, symbol: SymbolHandle) -> Result<(), String> {
        use symbol_resolved_trees::statement::{Statement, TransitionGuard, TransitionTarget};
        let program = self.program;
        let mut machines = program
            .machines
            .iter()
            .filter(|machine| machine.symbol == symbol);
        let machine = machines
            .next()
            .ok_or("constant dependency call lost its machine body")?;
        if machines.next().is_some() {
            return Err("constant dependency machine is ambiguous".to_owned());
        }
        let states = program.machine_state_handles(machine.states);
        if states.len() != machine.states.len() {
            return Err("constant dependency machine has stale states".to_owned());
        }
        let owned_values = program.machine_owned_data(machine.owned_data);
        if owned_values.len() != machine.owned_data.len() {
            return Err("constant dependency machine has stale owned initializers".to_owned());
        }
        for owned in owned_values {
            if owned.initial_value.is_valid() {
                self.expressions.push(owned.initial_value);
            }
        }
        let expressions = &program.tables.bodies.expressions;
        for handle in states {
            let state = program.machine_state(*handle);
            if !state.symbol.is_valid()
                || program.symbols.get(state.symbol).parent != machine.symbol
            {
                return Err("constant dependency has a stale or foreign state".to_owned());
            }
            // Resolution owns these tree statements. The flattened node span
            // is populated only after initializer receipts have been replayed.
            let body = program.state_statements(state.statements);
            if body.len() != state.statements.len() {
                return Err("constant dependency state has stale statements".to_owned());
            }
            for statement in body {
                match statement {
                    Statement::Expression(expression) => self.expressions.push(*expression),
                    Statement::LocalData(local) => {
                        if local.initial_value.is_valid() {
                            self.expressions.push(local.initial_value);
                        }
                    }
                    Statement::Assignment(assignment) => self
                        .expressions
                        .extend([assignment.target, assignment.value]),
                    Statement::RootBinding(binding) => self.expressions.push(binding.receiver),
                    Statement::AssemblyFact(fact) => self.expressions.push(fact.expression),
                    Statement::ProofOutputBindingStatement(binding) => {
                        self.expressions.push(binding.call)
                    }
                    Statement::Call(call) => {
                        self.call(call.target_symbol, call.target.source_span())?;
                        let arguments = expressions.expression_handles(call.arguments);
                        if arguments.len() != call.arguments.len() {
                            return Err("constant statement call has stale arguments".to_owned());
                        }
                        self.expressions.extend(arguments.iter().rev().copied());
                    }
                    Statement::Transition(transition) => {
                        if let TransitionGuard::When(guard) = &transition.guard {
                            self.expressions.push(*guard);
                        }
                        for target in std::iter::once(&transition.target)
                            .chain(transition.continuation.iter())
                        {
                            match target {
                                TransitionTarget::Value(value) => self.expressions.push(*value),
                                TransitionTarget::Named(target) => {
                                    self.call(target.symbol, target.source_span)?;
                                    let values = expressions.expression_handles(target.arguments);
                                    if values.len() != target.arguments.len() {
                                        return Err(
                                            "constant transition has stale arguments".to_owned()
                                        );
                                    }
                                    self.expressions.extend(values.iter().rev().copied());
                                }
                                TransitionTarget::SelfTarget | TransitionTarget::Terminal => {}
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepare(text: &str) -> (syntax_trees::SyntaxTrees, crate::ConstInitializerSelection) {
        let mut sources = source::SourceMap::default();
        let source = sources
            .add(
                std::path::PathBuf::from("constant-dependencies.omg"),
                text.to_owned(),
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .expect("tokens");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).expect("syntax");
        let preparation = crate::lower_syntax_trees_for_const_initializer_selection(
            &syntax,
            Some(std::sync::Arc::new(sources)),
            Vec::new(),
        )
        .expect("source-aware preparation");
        (syntax, preparation)
    }

    #[test]
    fn implicit_entry_call_retains_its_exact_authored_machine_source() {
        let (syntax, preparation) =
            prepare("machine size() -> u64 { 7 } const SIZE: u64 = size();");
        let definition = syntax
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Const(definition) => Some(definition),
                _ => None,
            })
            .expect("constant");
        let dependencies = preparation
            .initializer_dependencies(&syntax, definition)
            .expect("entry custody");
        let owner = preparation
            .trees()
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "size")
            .expect("machine");
        assert_eq!(dependencies.calls.len(), 1);
        assert_eq!(
            Some(dependencies.calls[0].1),
            preparation.trees().symbols.symbol_source_span(owner.symbol)
        );
        let target = dependencies.call_targets[0].1;
        assert_eq!(
            preparation.trees().symbols.get(target).kind,
            SymbolKind::State
        );
        assert_eq!(preparation.trees().symbols.get(target).parent, owner.symbol);
        assert!(
            preparation
                .trees()
                .symbols
                .symbol_source_span(target)
                .is_none()
        );
    }

    #[test]
    fn scalar_leaf_dependencies_follow_exact_calls_and_unselected_body_constants() {
        let text = "data Pair [copy] { first: u64; second: u64; }
            const BASE: u64 = 3 + 4;
            const OTHER: u64 = 8 + 1;
            machine helper() -> u64 { match true { true -> BASE, false -> OTHER } }
            machine identity(value: u64) -> u64 { value }
            const VALUE: Pair = Pair { first: identity(helper()), second: 1 + 2 };";
        let (syntax, preparation) = prepare(text);
        let definition = |name: &str| {
            syntax
                .root_items()
                .find_map(|item| match item {
                    syntax_trees::item::Item::Const(definition)
                        if definition.name.as_str() == name =>
                    {
                        Some(definition)
                    }
                    _ => None,
                })
                .expect("constant")
        };
        let value = definition("VALUE");
        let leaves = preparation
            .pending_leaves(&syntax, value)
            .expect("scalar leaves");
        assert_eq!(leaves.len(), 2);
        let first = preparation
            .initializer_expression_dependencies(&syntax, value, leaves[0].0)
            .expect("first dependencies");
        assert_eq!(first.calls.len(), 2);
        for name in ["BASE", "OTHER"] {
            assert!(
                first
                    .constants
                    .iter()
                    .any(|dependency| dependency.1 == definition(name).name.source_span())
            );
        }
        let second = preparation
            .initializer_expression_dependencies(&syntax, value, leaves[1].0)
            .expect("second dependencies");
        assert!(second.constants.is_empty());
        assert!(second.calls.is_empty());
        assert!(
            preparation
                .initializer_expression_dependencies(&syntax, value, definition("BASE").value)
                .is_err()
        );
    }
}
