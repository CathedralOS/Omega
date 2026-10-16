//! Declaration-owned dependency discovery before provisional values exist.

use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use source::SourceSpan;
use symbol_resolved_trees::{
    SymbolResolvedTrees,
    expression::{ExpressionHandle, ExpressionNode, MatchPattern},
    machine::MachineBodyStatus,
};
use symbols::{SymbolHandle, SymbolKind};

mod types;

/// Exact source occurrence/declaration pairs. Body dependencies are retained
/// even when execution later selects another branch.
#[derive(Debug, Default)]
pub struct ConstInitializerDependencies {
    pub constants: Vec<(SourceSpan, SourceSpan)>,
    pub calls: Vec<(SourceSpan, SourceSpan)>,
    pub(crate) call_targets: Vec<(SourceSpan, SymbolHandle)>,
}

/// Exact machine roots and their resolved call dependencies. A deferred body
/// terminates traversal: complete `deferred_machines` and collect again before
/// treating `machines` as the complete execution roster. This is body demand,
/// not permission to execute or evidence of checked call selection.
#[derive(Debug, Default)]
pub struct ResolvedMachineBodyDemand {
    pub machines: Vec<SymbolHandle>,
    pub deferred_machines: Vec<SymbolHandle>,
}

pub(crate) fn collect_machine_body_demand(
    program: &SymbolResolvedTrees,
    machine_roots: &[SymbolHandle],
    expression_roots: &[ExpressionHandle],
) -> Result<ResolvedMachineBodyDemand, String> {
    let mut collector = Collector {
        machines: machine_roots.iter().rev().copied().collect(),
        expressions: expression_roots.iter().rev().copied().collect(),
        deferred_machines: Some(Vec::new()),
        ..Collector::new(program)
    };
    collector.collect()?;
    Ok(ResolvedMachineBodyDemand {
        machines: collector.visited_machines,
        deferred_machines: collector.deferred_machines.unwrap_or_default(),
    })
}

pub(crate) fn collect(
    program: &SymbolResolvedTrees,
    root: ExpressionHandle,
) -> Result<ConstInitializerDependencies, String> {
    let mut collector = Collector {
        expressions: vec![root],
        ..Collector::new(program)
    };
    collector.collect()?;
    Ok(collector.dependencies)
}

pub(crate) fn expression_at_source(
    program: &SymbolResolvedTrees,
    root: ExpressionHandle,
    source: SourceSpan,
) -> Result<ExpressionHandle, String> {
    let mut collector = Collector {
        expressions: vec![root],
        follow_dependencies: false,
        ..Collector::new(program)
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
    types: Vec<&'program symbol_resolved_trees::types::TypeReference>,
    visited_type_children: Vec<arena::Handle<symbol_resolved_trees::types::TypeReference>>,
    visited_type_declarations: Vec<SymbolHandle>,
    expanded_constants: Vec<SymbolHandle>,
    visited_expressions: Vec<ExpressionHandle>,
    visited_machines: Vec<SymbolHandle>,
    /// Only a retained-body owner can request an incomplete dependency frontier.
    /// Ordinary initializer collection requires every reached body to exist.
    deferred_machines: Option<Vec<SymbolHandle>>,
    follow_dependencies: bool,
}

impl<'program> Collector<'program> {
    fn new(program: &'program SymbolResolvedTrees) -> Self {
        Self {
            program,
            dependencies: ConstInitializerDependencies::default(),
            expressions: Vec::new(),
            machines: Vec::new(),
            types: Vec::new(),
            visited_type_children: Vec::new(),
            visited_type_declarations: Vec::new(),
            expanded_constants: Vec::new(),
            visited_expressions: Vec::new(),
            visited_machines: Vec::new(),
            deferred_machines: None,
            follow_dependencies: true,
        }
    }

    fn collect(&mut self) -> Result<(), String> {
        while !self.expressions.is_empty() || !self.machines.is_empty() || !self.types.is_empty() {
            if let Some(expression) = self.expressions.pop() {
                if self.visited_expressions.contains(&expression) {
                    continue;
                }
                self.visited_expressions.push(expression);
                self.expression(expression)?;
            } else if let Some(machine) = self.machines.pop() {
                if self.visited_machines.contains(&machine) {
                    continue;
                }
                self.visited_machines.push(machine);
                self.machine(machine)?;
            } else if let Some(reference) = self.types.pop() {
                self.type_reference(reference)?;
            }
        }
        Ok(())
    }

    fn call(&mut self, target: SymbolHandle, reference: SourceSpan) -> Result<(), String> {
        let symbol = self.program.symbols.get(target);
        if symbol.kind == SymbolKind::Operator {
            // A named boundary operator (`F32::square_root`) keeps its exact
            // declaration identity but owns no machine body to traverse: its
            // contract is admission evidence, not an evaluation dependency.
            // Typed named-call selection and the checked interpreter own the
            // exact selected execution of this call.
            let declaration = self
                .program
                .symbols
                .symbol_source_span(target)
                .ok_or("constant operator call lost its exact declaration source")?;
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
            return Ok(());
        }
        let machine = match symbol.kind {
            SymbolKind::Machine => target,
            SymbolKind::State
                if self.program.symbols.get(symbol.parent).kind == SymbolKind::Machine =>
            {
                symbol.parent
            }
            // Body demand is not call admission. A builtin or operand-directed
            // target has no ordinary implementation at this frontier; typing
            // supplies its selected dependency before the caller checks and
            // admits execution. Initializer provenance collection below still
            // requires an exact ordinary declaration.
            _ if self.deferred_machines.is_some() => return Ok(()),
            _ => {
                return Err(format!(
                    "constant initializer call has no exact ordinary machine declaration: selected {:?} at {reference:?}",
                    symbol.kind,
                ));
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
        let direct_constant = super::selected_expression_constant(program, expression);
        let terminal_segment = match table.expression(expression) {
            ExpressionNode::Name(path) => table
                .name_path_members(path.members)
                .last()
                .map(|member| member.source_span()),
            _ => None,
        };
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
            // The authored ledger records individual path segments, while
            // substitution and initializer receipts name the whole use. Join
            // only this Name's exact terminal selection to its full path;
            // inherited origins and separate uses must remain distinct.
            let reference = direct_constant
                .filter(|(_, selected_symbol)| {
                    *selected_symbol == selected.selected_symbol()
                        && terminal_segment == Some(selection.source_span())
                })
                .map_or(selection.source_span(), |(reference, _)| reference);
            self.constant(selected.selected_symbol(), reference)?;
        }
        match table.expression(expression) {
            ExpressionNode::Call(call) => {
                self.static_arguments(&call.machine_arguments)?;
                // `size_of<T>()`/`align_of<T>()` carry no callee: the
                // compiler answers them from the selected target's layout
                // after constant dependency ordering has already run.
                if !language_core::data_layout_queries::is_data_layout_query_spelling(
                    call.target.as_str(),
                    call.target_symbol.is_valid(),
                    call.receiver.is_valid(),
                ) {
                    self.call(call.target_symbol, call.target.source_span())?;
                }
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
            ExpressionNode::Cast(cast) => {
                self.type_child(cast.target_type)?;
                self.type_arguments(cast.semantic_domain_arguments)?;
                self.expressions.push(cast.value);
            }
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
                self.type_declaration(literal.type_symbol)?;
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
                    self.constant(selected, reference)?;
                } else if let ExpressionNode::Name(path) = table.expression(expression)
                    && program.symbols.get(path.symbol).kind == SymbolKind::Variant
                {
                    self.type_declaration(program.symbols.get(path.symbol).parent)?;
                }
            }
            ExpressionNode::ZeroValue(reference) => self.type_child(*reference)?,
            ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::String(_) => {}
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
        self.type_parameters(machine.type_parameters)?;
        if let Some(reference) = &machine.attached_data_application {
            self.types.push(reference);
        }
        if machine.body_status == MachineBodyStatus::Deferred {
            let Some(deferred) = &mut self.deferred_machines else {
                return Err(format!(
                    "constant dependency requires a deferred machine body: `{}` at {:?}",
                    program.symbols.display_path(symbol, "::"),
                    program.symbols.symbol_source_span(symbol),
                ));
            };
            deferred.push(symbol);
            return Ok(());
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
            self.types.push(&owned.type_reference);
            if owned.initial_value.is_valid() {
                self.expressions.push(owned.initial_value);
            }
        }
        let expressions = &program.tables.bodies.expressions;
        for handle in states {
            let state = program.machine_state(*handle);
            self.state_parameters(state.parameters)?;
            if let Some(reference) = &state.return_type {
                self.types.push(reference);
            }
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
            let mut statements: Vec<_> = body.iter().rev().collect();
            while let Some(statement) = statements.pop() {
                match statement {
                    Statement::Assembly(handle) => {
                        let declarations = &program.tables.declarations;
                        let graph = declarations.assembly_graphs.get(*handle);
                        let blocks = declarations.assembly_blocks.span_or_empty(graph.blocks);
                        if blocks.len() != graph.blocks.len() {
                            return Err("constant dependency assembly has stale blocks".to_owned());
                        }
                        for block in blocks.iter().rev() {
                            for span in [block.requires, block.ensures] {
                                let expressions =
                                    declarations.assembly_expressions.span_or_empty(span);
                                if expressions.len() != span.len() {
                                    return Err("constant dependency assembly has stale contracts"
                                        .to_owned());
                                }
                                self.expressions.extend(expressions.iter().copied());
                            }
                            let body = declarations
                                .state_statements
                                .span_or_empty(block.instructions);
                            if body.len() != block.instructions.len() {
                                return Err("constant dependency assembly has stale instructions"
                                    .to_owned());
                            }
                            statements.extend(body.iter().rev());
                        }
                    }
                    Statement::Expression(expression) => self.expressions.push(*expression),
                    Statement::LocalData(local) => {
                        self.types.push(&local.type_reference);
                        if local.initial_value.is_valid() {
                            self.expressions.push(local.initial_value);
                        }
                    }
                    Statement::Assignment(assignment) => self
                        .expressions
                        .extend([assignment.target, assignment.value]),
                    Statement::RootBinding(binding) => {
                        self.expressions.push(binding.receiver);
                        if binding.implementation_operand.is_valid() {
                            self.expressions.push(binding.implementation_operand);
                        }
                    }
                    Statement::AssemblyFact(fact) => self.expressions.push(fact.expression),
                    Statement::ProofOutputBindingStatement(binding) => {
                        self.expressions.push(binding.call)
                    }
                    Statement::Call(call) => {
                        self.static_arguments(&call.machine_arguments)?;
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
                                TransitionTarget::Terminal => {}
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
