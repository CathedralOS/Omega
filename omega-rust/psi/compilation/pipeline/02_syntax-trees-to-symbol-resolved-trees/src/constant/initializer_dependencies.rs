//! Declaration-owned dependency discovery before provisional values exist.

use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use source::SourceSpan;
use symbol_resolved_trees::{
    SymbolResolvedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    machine::MachineBodyStatus,
};
use symbols::{SymbolHandle, SymbolKind};

mod types;

/// Declaration preparation may visit every owned implementation member. An
/// authored invocation closure stops at its logical bound until realization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DependencyPurpose {
    Authored,
    DeclarationReadiness,
}

/// Exact source occurrence/declaration pairs. Body dependencies are retained
/// even when execution later selects another branch.
#[derive(Debug, Default)]
pub struct ConstInitializerDependencies {
    /// A selected layout observation remains symbolic until target closure.
    pub pending_target_observation: bool,
    pub constants: Vec<(SourceSpan, SourceSpan)>,
    pub calls: Vec<(SourceSpan, SourceSpan)>,
    pub initializer_operators: Vec<SourceSpan>,
    /// Exact authored owners whose type applications belong to this dependency closure.
    pub normalization_owners: Vec<SourceSpan>,
    pub(crate) call_targets: Vec<AuthoredCallDependency>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AuthoredCallDependency {
    pub reference: SourceSpan,
    pub declaration: SourceSpan,
    pub target: SymbolHandle,
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
        initializer_expressions: expression_roots.to_vec(),
        deferred_machines: Some(Vec::new()),
        // This route completes declarations before source or realized custody
        // is collected. Visiting an owned member here grants no invocation.
        purpose: DependencyPurpose::DeclarationReadiness,
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
        initializer_expressions: vec![root],
        ..Collector::new(program)
    };
    collector.collect()?;
    Ok(collector.dependencies)
}

/// A declaration's destination participates in readiness independently of the
/// expression leaves. In particular an empty/literal array still demands its
/// exact extent before its value can be encoded or selected by another job.
pub(crate) fn collect_declaration(
    program: &SymbolResolvedTrees,
    root: ExpressionHandle,
    destination: &symbol_resolved_trees::types::TypeReference,
    purpose: DependencyPurpose,
) -> Result<ConstInitializerDependencies, String> {
    let mut collector = Collector {
        expressions: vec![root],
        initializer_expressions: vec![root],
        types: vec![destination],
        purpose,
        ..Collector::new(program)
    };
    collector.collect()?;
    for symbol in collector
        .visited_machines
        .iter()
        .chain(&collector.visited_type_declarations)
        .chain(&collector.expanded_constants)
    {
        let declaration = collector.program.symbols.get(*symbol);
        let namespace = if declaration.kind == SymbolKind::Machine {
            crate::symbols::owned_conformance_namespace(collector.program, *symbol)?
        } else {
            SymbolHandle::invalid()
        };
        let owner = if namespace.is_valid() {
            namespace
        } else {
            *symbol
        };
        let source = collector
            .program
            .symbols
            .symbol_source_span(owner)
            .ok_or("initializer normalization owner lost its declaration source")?;
        if !collector
            .dependencies
            .normalization_owners
            .contains(&source)
        {
            collector.dependencies.normalization_owners.push(source);
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
    /// Exact expression-tree membership, independent of diagnostic spans.
    /// Machine bodies and declaration type work enter their separate queues;
    /// following a call does not make its body an initializer expression.
    initializer_expressions: Vec<ExpressionHandle>,
    machines: Vec<SymbolHandle>,
    types: Vec<&'program symbol_resolved_trees::types::TypeReference>,
    visited_type_children: Vec<arena::Handle<symbol_resolved_trees::types::TypeReference>>,
    visited_type_declarations: Vec<SymbolHandle>,
    expanded_constants: Vec<SymbolHandle>,
    visited_expressions: Vec<ExpressionHandle>,
    visited_initializer_expressions: Vec<ExpressionHandle>,
    visited_machines: Vec<SymbolHandle>,
    /// Only a retained-body owner can request an incomplete dependency frontier.
    /// Ordinary initializer collection requires every reached body to exist.
    deferred_machines: Option<Vec<SymbolHandle>>,
    follow_dependencies: bool,
    purpose: DependencyPurpose,
}

impl<'program> Collector<'program> {
    fn new(program: &'program SymbolResolvedTrees) -> Self {
        Self {
            program,
            dependencies: ConstInitializerDependencies::default(),
            expressions: Vec::new(),
            initializer_expressions: Vec::new(),
            machines: Vec::new(),
            types: Vec::new(),
            visited_type_children: Vec::new(),
            visited_type_declarations: Vec::new(),
            expanded_constants: Vec::new(),
            visited_expressions: Vec::new(),
            visited_initializer_expressions: Vec::new(),
            visited_machines: Vec::new(),
            deferred_machines: None,
            follow_dependencies: true,
            purpose: DependencyPurpose::Authored,
        }
    }

    fn collect(&mut self) -> Result<(), String> {
        while !self.expressions.is_empty() || !self.machines.is_empty() || !self.types.is_empty() {
            if let Some(expression) = self.expressions.pop() {
                let initializer = self.initializer_expressions.contains(&expression);
                if self.visited_expressions.contains(&expression)
                    && (!initializer || self.visited_initializer_expressions.contains(&expression))
                {
                    continue;
                }
                if !self.visited_expressions.contains(&expression) {
                    self.visited_expressions.push(expression);
                }
                if initializer {
                    // A shared handle may first be reached by type/body work.
                    // Revisit its children once under initializer ownership;
                    // the two finite visited frontiers still terminate cycles.
                    self.visited_initializer_expressions.push(expression);
                }
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
        let bound_requirement = symbol.kind == SymbolKind::State
            && symbol.parent.is_valid()
            && self.program.symbols.get(symbol.parent).kind == SymbolKind::ConformanceParameter
            && self.program.machines.iter().any(|machine| {
                machine.symbol == self.program.symbols.get(symbol.parent).parent
                    && machine
                        .conformance_bounds
                        .iter()
                        .filter(|bound| bound.binder == symbol.parent)
                        .count()
                        == 1
            })
            && self
                .program
                .symbols
                .child_handles(symbol.parent)
                .is_some_and(|mut children| children.any(|child| child == target));
        if symbol.kind == SymbolKind::Operator || bound_requirement {
            // A logical requirement is an exact authored selection, not an
            // ordinary body. Evidence-binder children were installed from this
            // machine's explicit bound; the typed invocation must independently
            // rejoin its complete application before selecting executable code.
            // Named boundary operators likewise retain their declaration here.
            // This source dependency row grants no execution permission and
            // cannot substitute for the checked selected-body custody closure.
            let declaration = self
                .program
                .symbols
                .symbol_source_span(target)
                .ok_or("constant logical call lost its exact declaration source")?;
            let selection = (reference, declaration);
            if !self.dependencies.calls.contains(&selection) {
                self.dependencies.calls.push(selection);
            }
            let dependency = AuthoredCallDependency {
                reference,
                declaration,
                target,
            };
            if !self.dependencies.call_targets.contains(&dependency) {
                self.dependencies.call_targets.push(dependency);
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
        let dependency = AuthoredCallDependency {
            reference,
            declaration,
            target,
        };
        if !self.dependencies.call_targets.contains(&dependency) {
            self.dependencies.call_targets.push(dependency);
        }
        if self.follow_dependencies {
            self.machines.push(machine);
        }
        Ok(())
    }

    fn expression(&mut self, expression: ExpressionHandle) -> Result<(), String> {
        let initializer = self.initializer_expressions.contains(&expression);
        let child_frontier = self.expressions.len();
        self.expression_dependencies(expression)?;
        if initializer {
            // Only expression children appended by this visit inherit this
            // boundary. Deferred machine bodies and type geometry do not.
            for index in child_frontier..self.expressions.len() {
                let child = self.expressions[index];
                if !self.initializer_expressions.contains(&child) {
                    self.initializer_expressions.push(child);
                }
            }
        }
        Ok(())
    }

    fn expression_dependencies(&mut self, expression: ExpressionHandle) -> Result<(), String> {
        let program = self.program;
        let table = &program.tables.bodies.expressions;
        if !table.expression_is_valid(expression) {
            return Err("constant initializer dependency has a stale expression".to_owned());
        }
        if matches!(
            table.expression(expression),
            ExpressionNode::Binary(_) | ExpressionNode::Unary(_)
        ) {
            let reference = table.source_span(expression);
            // A binary's diagnostic span may name only its operator token:
            // the outer '*' span cannot contain the inner '/' in 7 / 2 * 2.
            // Custody follows exact expression children, never span nesting.
            if self.initializer_expressions.contains(&expression)
                && !self.dependencies.initializer_operators.contains(&reference)
            {
                self.dependencies.initializer_operators.push(reference);
            }
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
                let native_query = call.target.as_str() == "code_layout"
                    && !call.target_symbol.is_valid()
                    && !call.receiver.is_valid();
                if native_query && self.deferred_machines.is_none() {
                    return Err("late native code-layout metadata is unavailable to same-product semantic constant, type, layout, or optimization evaluation".to_owned());
                }
                // `size_of<T>()`/`align_of<T>()` carry no callee: the
                // compiler answers them from the selected target's layout
                // after constant dependency ordering has already run.
                let layout_query =
                    language_core::data_layout_queries::is_data_layout_query_spelling(
                        call.target.as_str(),
                        call.target_symbol.is_valid(),
                        call.receiver.is_valid(),
                    );
                self.dependencies.pending_target_observation |= layout_query;
                if !native_query && !layout_query {
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
                    self.expressions
                        .extend(table.match_pattern_dependencies(arm.pattern));
                }
                self.expressions.push(dispatch.subject);
            }
            ExpressionNode::SavedSubject(saved) => {
                self.expressions.extend([
                    table.match_subject(saved.capture).subject,
                    saved.pattern_head,
                ]);
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
        if machine.target_family_interface {
            // A portable head selects no deployment's private body. Dependency
            // discovery retains every exact family member; this grants no
            // permission to interpret one member as a shared constant value.
            self.machines.extend(
                program
                    .machines
                    .iter()
                    .filter(|member| {
                        member.target_family == symbol && !member.target_family_interface
                    })
                    .map(|member| member.symbol),
            );
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
                        let native_query = call.target.as_str() == "code_layout"
                            && !call.target_symbol.is_valid()
                            && call.receiver.is_empty();
                        if native_query && self.deferred_machines.is_none() {
                            return Err("late native code-layout metadata is unavailable to same-product semantic constant, type, layout, or optimization evaluation".to_owned());
                        }
                        if !native_query {
                            self.call(call.target_symbol, call.target.source_span())?;
                        }
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
