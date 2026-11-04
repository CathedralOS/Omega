//! Declaration-owned dependency discovery before provisional values exist.
//!
//! A static callable has two independent dependencies: its authored binder is
//! the call's logical selection, and its explicit operand supplies the body
//! that specialization may execute. Retain both without treating an abstract
//! binder as an ordinary body or granting execution from source readiness.

use crate::selection::call_identity::{call_custody_source, call_declaration_source};
use crate::tables::body_expressions;
use crate::tables::resolved_declarations;
use language_core::data_layout_queries::is_data_layout_query_spelling;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionTarget, ResolvedAuthoredCallBinding,
};
use source::SourceSpan;
use std::iter::once;
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::expression::StaticMachineArgument;
use symbol_resolved_trees::signature::SignatureContract;
use symbol_resolved_trees::types::TypeReference;
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
    /// Scoped overload choices still owed by ordinary typed call selection.
    /// Candidate availability contributes no selected-body constant dependency.
    pub pending_calls: Vec<(SourceSpan, Vec<SourceSpan>)>,
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
        expressions: expression_roots
            .iter()
            .rev()
            .copied()
            .map(DependencyWork::unscoped)
            .collect(),
        initializer_expressions: expression_roots
            .iter()
            .copied()
            .map(DependencyWork::unscoped)
            .collect(),
        // An exact initializer owner supplies its declared type as readiness
        // input too. Its selected implementation members must be complete
        // before ordinary dependency collection; this grants no invocation.
        types: program
            .const_declarations
            .iter()
            .filter(|declaration| {
                let initializer = if declaration.authored_initializer.is_valid() {
                    declaration.authored_initializer
                } else {
                    declaration.initializer
                };
                expression_roots.contains(&initializer)
            })
            .map(|declaration| DependencyWork::unscoped(&declaration.declared_type))
            .collect(),
        deferred_machines: Some(Vec::new()),
        // This route completes declarations before source or realized custody
        // is collected. Visiting an owned member here grants no invocation.
        purpose: DependencyPurpose::DeclarationReadiness,
        ..Collector::new(program)
    };
    collector.collect()?;
    // Candidate bodies may need completion to type the call, but availability
    // is not an execution root. Keep this closure out of `machines`; the typed
    // operational graph later supplies only the selected candidate's edges.
    let mut pending = collector.available_machines.clone();
    let mut visited = collector.visited_machines.clone();
    while let Some(machine) = pending.pop() {
        if visited.contains(&machine) {
            continue;
        }
        let mut availability = Collector {
            machines: vec![machine],
            deferred_machines: Some(Vec::new()),
            purpose: DependencyPurpose::DeclarationReadiness,
            ..Collector::new(program)
        };
        availability.collect()?;
        visited.extend(availability.visited_machines);
        pending.extend(availability.available_machines);
        let deferred = collector
            .deferred_machines
            .as_mut()
            .expect("body-demand owner");
        for machine in availability.deferred_machines.unwrap_or_default() {
            if !deferred.contains(&machine) {
                deferred.push(machine);
            }
        }
    }
    Ok(ResolvedMachineBodyDemand {
        machines: collector.visited_machines,
        deferred_machines: collector.deferred_machines.unwrap_or_default(),
    })
}

pub(crate) fn collect(
    program: &SymbolResolvedTrees,
    root: ExpressionHandle,
    selected_calls: Option<&[(SourceSpan, SourceSpan)]>,
) -> Result<ConstInitializerDependencies, String> {
    let mut collector = Collector {
        expressions: vec![DependencyWork::unscoped(root)],
        initializer_expressions: vec![DependencyWork::unscoped(root)],
        selected_calls,
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
    destination: &TypeReference,
    purpose: DependencyPurpose,
    retained_base: Option<&SymbolResolvedTrees>,
    selected_calls: Option<&[(SourceSpan, SourceSpan)]>,
) -> Result<ConstInitializerDependencies, String> {
    let mut collector = Collector {
        expressions: vec![DependencyWork::unscoped(root)],
        initializer_expressions: vec![DependencyWork::unscoped(root)],
        selected_calls,
        types: vec![DependencyWork::unscoped(destination)],
        purpose,
        ..Collector::new(program)
    };
    collector.collect()?;
    for symbol in collector
        .visited_machines
        .iter()
        .chain(&collector.available_machines)
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
        // Normalization work belongs to this syntax cohort. A selected owner
        // already resolved in the predecessor supplies its real declarations
        // and bodies to the probe, not syntax that this cohort must recreate.
        // Its calls/constants remain in the dependency and admission rosters.
        if retained_base.is_some_and(|base| {
            base.symbols.symbols().nodes().is_valid(owner)
                && base.symbols.symbol_source_is_selected(owner)
        }) {
            continue;
        }
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
        expressions: vec![DependencyWork::unscoped(root)],
        follow_dependencies: false,
        ..Collector::new(program)
    };
    let mut selected = None;
    while let Some(work) = collector.expressions.pop() {
        collector.lexical_owner = work.lexical_owner;
        let expression = work.value;
        if collector.visited_expressions.contains(&work) {
            continue;
        }
        collector.visited_expressions.push(work);
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

/// Bind constant-initializer calls through a named implementation
/// to its public requirement before authored-selection recording. Every copy
/// of those source tokens is bound too: selection finalization groups copies
/// by token, and one unresolved copy keeps a late call selection. The private
/// realization supplies body demand separately, just as for machine-body calls.
pub(crate) fn bind_named_implementation_const_calls(
    program: &mut SymbolResolvedTrees,
    roots: &[ExpressionHandle],
) -> Result<(), String> {
    let mut spans = Vec::new();
    collect_call_target_spans(program, roots, &mut spans);
    if spans.is_empty() {
        return Ok(());
    }
    let expressions = body_expressions(program)
        .iter_expressions()
        .map(|(expression, _)| expression)
        .collect::<Vec<_>>();
    let mut routing =
        crate::selection::conformance_blocks::ProvisionalBodyCallRouting::new(program);
    let mut targets = Vec::new();
    for expression in expressions {
        let node = expression_node(program, expression);
        let ExpressionNode::Call(call) = &node else {
            continue;
        };
        if !spans.contains(&call.target.source_span()) {
            continue;
        }
        let receiver_symbol = body_expressions(program)
            .name_path_node(call.receiver)
            .map_or_else(SymbolHandle::invalid, |path| path.symbol);
        let Some(binding) = routing.named_binding(
            &crate::selection::body_calls::CallSite {
                receiver_symbol,
                target_symbol: call.target_symbol,
                target: &call.target,
                member: &call.target,
                requirement_application: call.requirement_application,
                local_receiver: !call.receiver.is_valid(),
                argument_count: body_expressions(program)
                    .expression_handles(call.arguments)
                    .len(),
            },
            &call.machine_arguments,
        )?
        else {
            continue;
        };
        targets.push((expression, binding.original));
    }
    drop(routing);
    for (expression, requirement) in targets {
        let ExpressionNode::Call(call) =
            program.tables.bodies.expressions.expression_mut(expression)
        else {
            continue;
        };
        call.target_symbol = requirement;
    }
    Ok(())
}

fn collect_call_target_spans(
    program: &SymbolResolvedTrees,
    roots: &[ExpressionHandle],
    spans: &mut Vec<SourceSpan>,
) {
    let mut pending = roots.to_vec();
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        if !body_expressions(program).expression_is_valid(expression) {
            continue;
        }
        let node = expression_node(program, expression);
        if let ExpressionNode::Call(call) = &node
            && !spans.contains(&call.target.source_span())
        {
            spans.push(call.target.source_span());
        }
        pending.extend(expression_child_handles(program, &node));
    }
}

/// The node a handle names, copied out before a walk mutates the table.
fn expression_node(program: &SymbolResolvedTrees, handle: ExpressionHandle) -> ExpressionNode {
    program.tables.bodies.expressions.expression(handle).clone()
}

fn expression_child_handles(
    program: &SymbolResolvedTrees,
    node: &ExpressionNode,
) -> Vec<ExpressionHandle> {
    let table = &program.tables.bodies.expressions;
    match node {
        ExpressionNode::Call(call) => {
            let mut children = Vec::new();
            if call.receiver.is_valid() {
                children.push(call.receiver);
            }
            children.extend(table.expression_handles(call.arguments).iter().copied());
            children
        }
        ExpressionNode::Binary(binary) => vec![binary.left, binary.right],
        ExpressionNode::Unary(unary) => vec![unary.operand],
        ExpressionNode::Cast(cast) => vec![cast.value],
        ExpressionNode::Member(member) => vec![member.receiver],
        ExpressionNode::Borrow(borrow) => vec![borrow.target],
        ExpressionNode::Indexed(indexed) => vec![indexed.collection, indexed.index],
        ExpressionNode::Membership(membership) => vec![membership.value],
        ExpressionNode::Atomic(atomic) => vec![atomic.value, atomic.result],
        ExpressionNode::Range(range) => [range.start, range.end]
            .into_iter()
            .filter(|handle| handle.is_valid())
            .collect(),
        ExpressionNode::ArrayLiteral(elements) => table.expression_handles(*elements).to_vec(),
        ExpressionNode::StructLiteral(literal) => table
            .struct_fields(literal.fields)
            .iter()
            .map(|field| field.value)
            .collect(),
        ExpressionNode::Match(dispatch) => {
            let mut children = vec![dispatch.subject];
            for arm in table.match_arms(dispatch.arms) {
                children.extend(table.match_pattern_dependencies(arm.pattern));
                children.push(arm.value);
            }
            children
        }
        ExpressionNode::SavedSubject(saved) => vec![
            table.match_subject(saved.capture).subject,
            saved.pattern_head,
        ],
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::TypeExpression(_)
        | ExpressionNode::ZeroValue(_) => Vec::new(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DependencyWork<Value> {
    value: Value,
    // Default bodies can share source nodes across distinct realizations.
    // Their queued work and visited keys must keep each row's exact owner.
    lexical_owner: SymbolHandle,
}

impl<Value> DependencyWork<Value> {
    fn unscoped(value: Value) -> Self {
        Self {
            value,
            lexical_owner: SymbolHandle::invalid(),
        }
    }
}

struct Collector<'program, 'selection> {
    program: &'program SymbolResolvedTrees,
    body_call_routing: crate::selection::conformance_blocks::ProvisionalBodyCallRouting<'program>,
    dependencies: ConstInitializerDependencies,
    expressions: Vec<DependencyWork<ExpressionHandle>>,
    lexical_owner: SymbolHandle,
    /// Exact expression-tree membership, independent of diagnostic spans.
    /// Machine bodies and declaration type work enter their separate queues;
    /// following a call does not make its body an initializer expression.
    initializer_expressions: Vec<DependencyWork<ExpressionHandle>>,
    machines: Vec<SymbolHandle>,
    available_machines: Vec<SymbolHandle>,
    types: Vec<DependencyWork<&'program TypeReference>>,
    visited_type_children: Vec<(arena::Handle<TypeReference>, SymbolHandle)>,
    visited_type_declarations: Vec<SymbolHandle>,
    expanded_constants: Vec<SymbolHandle>,
    visited_expressions: Vec<DependencyWork<ExpressionHandle>>,
    visited_initializer_expressions: Vec<DependencyWork<ExpressionHandle>>,
    visited_machines: Vec<SymbolHandle>,
    /// Only a retained-body owner can request an incomplete dependency frontier.
    /// Ordinary initializer collection requires every reached body to exist.
    deferred_machines: Option<Vec<SymbolHandle>>,
    follow_dependencies: bool,
    purpose: DependencyPurpose,
    // Typed choices disambiguate unresolved overload families. Readiness can
    // also visit independently selected member bodies absent from that hint.
    // Final normalization compares the complete reconstructed source roster.
    selected_calls: Option<&'selection [(SourceSpan, SourceSpan)]>,
}

impl<'program, 'selection> Collector<'program, 'selection> {
    fn new(program: &'program SymbolResolvedTrees) -> Self {
        Self {
            program,
            body_call_routing:
                crate::selection::conformance_blocks::ProvisionalBodyCallRouting::new(program),
            dependencies: ConstInitializerDependencies::default(),
            expressions: Vec::new(),
            lexical_owner: SymbolHandle::invalid(),
            initializer_expressions: Vec::new(),
            machines: Vec::new(),
            available_machines: Vec::new(),
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
            selected_calls: None,
        }
    }

    fn expression_work(&self, value: ExpressionHandle) -> DependencyWork<ExpressionHandle> {
        DependencyWork {
            value,
            lexical_owner: self.lexical_owner,
        }
    }

    fn queue_expression(&mut self, value: ExpressionHandle) {
        self.expressions.push(DependencyWork {
            value,
            lexical_owner: self.lexical_owner,
        });
    }

    fn queue_expressions(&mut self, values: impl IntoIterator<Item = ExpressionHandle>) {
        let lexical_owner = self.lexical_owner;
        self.expressions
            .extend(values.into_iter().map(|value| DependencyWork {
                value,
                lexical_owner,
            }));
    }

    fn queue_type(&mut self, value: &'program TypeReference) {
        self.types.push(DependencyWork {
            value,
            lexical_owner: self.lexical_owner,
        });
    }

    fn queue_types(&mut self, values: impl IntoIterator<Item = &'program TypeReference>) {
        let lexical_owner = self.lexical_owner;
        self.types
            .extend(values.into_iter().map(|value| DependencyWork {
                value,
                lexical_owner,
            }));
    }

    fn collect(&mut self) -> Result<(), String> {
        while !self.expressions.is_empty() || !self.machines.is_empty() || !self.types.is_empty() {
            if let Some(work) = self.expressions.pop() {
                self.lexical_owner = work.lexical_owner;
                let expression = work.value;
                let initializer = self.initializer_expressions.contains(&work);
                if self.visited_expressions.contains(&work)
                    && (!initializer || self.visited_initializer_expressions.contains(&work))
                {
                    continue;
                }
                if !self.visited_expressions.contains(&work) {
                    self.visited_expressions.push(work);
                }
                if initializer {
                    // A shared handle may first be reached by type/body work.
                    // Revisit its children once under initializer ownership;
                    // the two finite visited frontiers still terminate cycles.
                    self.visited_initializer_expressions.push(work);
                }
                self.expression(expression)?;
            } else if let Some(machine) = self.machines.pop() {
                if self.visited_machines.contains(&machine) {
                    continue;
                }
                self.visited_machines.push(machine);
                self.machine(machine)?;
            } else if let Some(work) = self.types.pop() {
                self.lexical_owner = work.lexical_owner;
                self.type_reference(work.value)?;
            }
        }
        Ok(())
    }

    fn call(&mut self, target: SymbolHandle, reference: SourceSpan) -> Result<(), String> {
        let symbol = self.program.symbols.get(target);
        let trait_requirement = symbol.kind == SymbolKind::State
            && self.program.traits.iter().any(|definition| {
                definition.symbol == symbol.parent
                    && self
                        .program
                        .trait_machine_signatures(definition.machines)
                        .iter()
                        .any(|requirement| requirement.symbol == target)
            });
        if trait_requirement {
            // A requirement is an exact source dependency, not executable
            // code. Its selected provider and contract formation settle later.
            self.dependencies.pending_target_observation = true;
            return self.record_logical_call(target, reference);
        }
        let bound_requirement = symbol.kind == SymbolKind::State
            && symbol.parent.is_valid()
            && self.program.symbols.get(symbol.parent).kind == SymbolKind::ConformanceParameter;
        if matches!(
            symbol.kind,
            SymbolKind::Operator | SymbolKind::MachineParameter
        ) || bound_requirement
        {
            // A logical requirement is an exact authored selection, not an
            // ordinary body. Evidence-binder children were installed from this
            // declaration's explicit bound; the typed invocation must independently
            // rejoin its complete application before selecting executable code.
            // Named boundary operators likewise retain their declaration here.
            // This source dependency row grants no execution permission and
            // cannot substitute for the checked selected-body custody closure.
            return self.record_logical_call(target, reference);
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
        let declaration = call_declaration_source(self.program, target)?;
        let selection = (reference, declaration);
        if !self.dependencies.calls.contains(&selection) {
            self.dependencies.calls.push(selection);
        }
        // Invocation syntax retains a terminal call's machine coordinate.
        // Its declaration selection names the exact callable entry, as do
        // expression calls and authored transition-call classification. Body
        // demand may still visit a deferred declaration with no entry yet;
        // only the closed source-custody roster owes that entry identity.
        let target = if symbol.kind == SymbolKind::Machine && self.deferred_machines.is_none() {
            let mut owners = self
                .program
                .machines
                .iter()
                .filter(|owner| owner.symbol == machine);
            let owner = owners
                .next()
                .ok_or("constant callable selection lost its machine")?;
            if owners.next().is_some() {
                return Err("constant callable selection has ambiguous machine identity".into());
            }
            let states = self.program.machine_state_handles(owner.states);
            if states.len() != owner.states.len() {
                return Err("constant callable selection has stale machine states".into());
            }
            let entry = states
                .first()
                .ok_or("constant callable selection has no entry state")?;
            self.program.machine_state(*entry).symbol
        } else {
            target
        };
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

    fn record_logical_call(
        &mut self,
        target: SymbolHandle,
        reference: SourceSpan,
    ) -> Result<(), String> {
        let declaration = call_custody_source(self.program, target)?;
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
        Ok(())
    }

    fn call_dependency(
        &mut self,
        site: crate::selection::body_calls::CallSite<'_>,
        target_span: SourceSpan,
        static_arguments: &[StaticMachineArgument],
    ) -> Result<(), String> {
        let receiver_symbol = site.receiver_symbol;
        let target_symbol = site.target_symbol;
        let target_name = site.member.as_str();
        // Named implementation routing runs after lexical routing and owns the
        // final choice for an explicit map call. Honor that choice first when
        // discovering dependencies in either fresh or already-routed source.
        if let Some(binding) = self
            .body_call_routing
            .named_binding(&site, static_arguments)?
        {
            return self.realized_call_dependency(binding, target_span);
        }
        if let Some(binding) = self.body_call_routing.binding(self.lexical_owner, &site)? {
            return self.realized_call_dependency(binding, target_span);
        }
        if !target_symbol.is_valid() {
            let symbols = &self.program.symbols;
            let candidates = if symbols.get(receiver_symbol).kind == SymbolKind::Module {
                symbols
                    .child_handles(receiver_symbol)
                    .into_iter()
                    .flatten()
                    .filter(|candidate| {
                        symbols.get(*candidate).kind == SymbolKind::Machine
                            && symbols.name(*candidate) == target_name
                    })
                    .collect::<Vec<_>>()
            } else if !receiver_symbol.is_valid() {
                symbols.top_level_candidates_from_source(
                    target_name,
                    &[SymbolKind::Machine],
                    target_span,
                )
            } else {
                Vec::new()
            };
            if !candidates.is_empty() {
                // Completing bodies is availability, not invocation or readiness.
                // The typed probe selects the body before its constants may
                // participate in the initializer dependency graph.
                if self.deferred_machines.is_some() {
                    if self.follow_dependencies {
                        self.available_machines.extend(candidates);
                    }
                    return Ok(());
                }
                let declarations = candidates
                    .iter()
                    .map(|candidate| call_declaration_source(self.program, *candidate))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(selected_calls) = self.selected_calls
                    && selected_calls
                        .iter()
                        .any(|(reference, _)| *reference == target_span)
                {
                    let choices = selected_calls
                        .iter()
                        .filter(|(reference, _)| *reference == target_span)
                        .collect::<Vec<_>>();
                    let [(_, declaration)] = choices.as_slice() else {
                        return Err("constant overload has no unique typed source selection".into());
                    };
                    let selected = candidates
                        .iter()
                        .zip(&declarations)
                        .filter(|(_, source)| *source == declaration)
                        .map(|(candidate, _)| *candidate)
                        .collect::<Vec<_>>();
                    let [selected] = selected.as_slice() else {
                        return Err(
                            "constant overload selection is outside its exact scoped family".into(),
                        );
                    };
                    return self.call(*selected, target_span);
                }
                for candidate in candidates {
                    if !self.available_machines.contains(&candidate) {
                        self.available_machines.push(candidate);
                    }
                }
                let pending = (target_span, declarations);
                if !self.dependencies.pending_calls.contains(&pending) {
                    self.dependencies.pending_calls.push(pending);
                }
                return Ok(());
            }
        }
        self.call(target_symbol, target_span)
    }

    fn realized_call_dependency(
        &mut self,
        binding: ResolvedAuthoredCallBinding,
        reference: SourceSpan,
    ) -> Result<(), String> {
        if binding.original == binding.selected {
            return self.call(binding.original, reference);
        }
        // The public requirement remains the source dependency. Its exact
        // supplying row contributes body demand without replacing that contract.
        self.record_logical_call(binding.original, reference)?;
        if self.follow_dependencies {
            self.machines
                .push(self.program.symbols.get(binding.selected).parent);
        }
        Ok(())
    }

    fn expression(&mut self, expression: ExpressionHandle) -> Result<(), String> {
        let initializer = self
            .initializer_expressions
            .contains(&self.expression_work(expression));
        let child_frontier = self.expressions.len();
        self.expression_dependencies(expression)?;
        if initializer {
            // Constructor discovery can append declaration predicates as well
            // as value operands. Only the actual expression edges inherit this
            // initializer's operator custody. Using the worklist suffix alone
            // makes the inventory depend on whether an unfurled template or a
            // closed instance was discovered first; declaration work retains
            // its own readiness owner in either order.
            let children = expression_child_handles(
                self.program,
                self.program
                    .tables
                    .bodies
                    .expressions
                    .expression(expression),
            );
            for child in children {
                if self.expressions[child_frontier..]
                    .iter()
                    .any(|work| *work == self.expression_work(child))
                    && !self
                        .initializer_expressions
                        .contains(&self.expression_work(child))
                {
                    self.initializer_expressions
                        .push(self.expression_work(child));
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
        let authored = table.materialization_source(expression);
        if authored.is_valid() {
            // A completed static operand keeps its original computation in
            // both source readiness and typed custody. Materialization erases
            // execution demand, never the initializer's authored dependency.
            self.queue_expression(authored);
            if self
                .initializer_expressions
                .contains(&self.expression_work(expression))
                && !self
                    .initializer_expressions
                    .contains(&self.expression_work(authored))
            {
                self.initializer_expressions
                    .push(self.expression_work(authored));
            }
        }
        if matches!(
            table.expression(expression),
            ExpressionNode::Binary(_) | ExpressionNode::Unary(_)
        ) {
            let reference = table.source_span(expression);
            // A binary's diagnostic span may name only its operator token:
            // the outer '*' span cannot contain the inner '/' in 7 / 2 * 2.
            // Custody follows exact expression children, never span nesting.
            if self
                .initializer_expressions
                .contains(&self.expression_work(expression))
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
                let layout_query = is_data_layout_query_spelling(
                    call.target.as_str(),
                    call.target_symbol.is_valid(),
                    call.receiver.is_valid(),
                );
                self.dependencies.pending_target_observation |= layout_query;
                if !native_query && !layout_query {
                    let receiver_symbol = table
                        .name_path_node(call.receiver)
                        .map_or_else(SymbolHandle::invalid, |path| path.symbol);
                    self.call_dependency(
                        crate::selection::body_calls::CallSite {
                            receiver_symbol,
                            target_symbol: call.target_symbol,
                            target: &call.target,
                            member: &call.target,
                            requirement_application: call.requirement_application,
                            local_receiver: !call.receiver.is_valid(),
                            argument_count: table.expression_handles(call.arguments).len(),
                        },
                        call.target.source_span(),
                        &call.machine_arguments,
                    )?;
                }
                if call.receiver.is_valid() {
                    self.queue_expression(call.receiver);
                }
                let arguments = table.expression_handles(call.arguments);
                if arguments.len() != call.arguments.len() {
                    return Err("constant call arguments have a stale span".to_owned());
                }
                self.queue_expressions(arguments.iter().rev().copied());
            }
            ExpressionNode::Binary(binary) => self.queue_expressions([binary.right, binary.left]),
            ExpressionNode::Unary(unary) => self.queue_expression(unary.operand),
            ExpressionNode::Cast(cast) => {
                self.type_child(cast.target_type)?;
                self.type_arguments(cast.semantic_domain_arguments)?;
                self.queue_expression(cast.value);
            }
            ExpressionNode::Member(member) => self.queue_expression(member.receiver),
            ExpressionNode::Borrow(borrow) => self.queue_expression(borrow.target),
            ExpressionNode::Indexed(indexed) => {
                self.queue_expressions([indexed.index, indexed.collection])
            }
            ExpressionNode::Membership(membership) => self.queue_expression(membership.value),
            ExpressionNode::Atomic(atomic) => self.queue_expressions([atomic.result, atomic.value]),
            ExpressionNode::Range(range) => self.queue_expressions(
                [range.end, range.start]
                    .into_iter()
                    .filter(|handle| handle.is_valid()),
            ),
            ExpressionNode::ArrayLiteral(elements) => {
                let values = table.expression_handles(*elements);
                if values.len() != elements.len() {
                    return Err("constant array dependency has a stale span".to_owned());
                }
                self.queue_expressions(values.iter().rev().copied());
            }
            ExpressionNode::StructLiteral(literal) => {
                self.type_declaration(literal.type_symbol)?;
                let fields = table.struct_fields(literal.fields);
                if fields.len() != literal.fields.len() {
                    return Err("constant constructor dependency has a stale span".to_owned());
                }
                self.queue_expressions(fields.iter().rev().map(|field| field.value));
            }
            ExpressionNode::Match(dispatch) => {
                let arms = table.match_arms(dispatch.arms);
                if arms.len() != dispatch.arms.len() {
                    return Err("constant dispatch dependency has a stale span".to_owned());
                }
                for arm in arms.iter().rev() {
                    self.queue_expression(arm.value);
                    self.queue_expressions(table.match_pattern_dependencies(arm.pattern));
                }
                self.queue_expression(dispatch.subject);
            }
            ExpressionNode::SavedSubject(saved) => {
                self.queue_expressions([
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
            ExpressionNode::TypeExpression(reference) | ExpressionNode::ZeroValue(reference) => {
                self.type_child(*reference)?
            }
            ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::String(_) => {}
        }
        Ok(())
    }

    /// Invocation formation demands the complete published clauses before a
    /// body can execute. A helper whose value body is independent can still
    /// need selected providers to form its requires/ensures operands.
    fn contracts(&mut self, contracts: arena::HandleSpan<SignatureContract>) -> Result<(), String> {
        let program = self.program;
        let clauses = program.signature_contracts(contracts);
        if clauses.len() != contracts.len() {
            return Err("constant dependency has stale signature clauses".to_owned());
        }
        for clause in clauses {
            self.predicate_facts(clause.facts)?;
        }
        Ok(())
    }

    /// Type/default-domain and invocation predicates share the same exact
    /// dependency roster. Readiness selects their bodies; it proves no clause.
    fn predicate_facts(&mut self, span: arena::HandleSpan<ProofFact>) -> Result<(), String> {
        let facts = self.program.proof_facts(span);
        if facts.len() != span.len() {
            return Err("constant dependency has stale predicate facts".to_owned());
        }
        for fact in facts {
            match fact {
                ProofFact::Expression(expression) => {
                    self.queue_expression(*expression);
                }
                ProofFact::Membership(membership) => {
                    self.queue_expression(membership.value);
                    self.queue_types(
                        self.program
                            .child_type_references(membership.domain_arguments),
                    );
                }
            }
        }
        Ok(())
    }

    fn machine(&mut self, symbol: SymbolHandle) -> Result<(), String> {
        use symbol_resolved_trees::statement::{Statement, TransitionGuard, TransitionTarget};
        self.lexical_owner = symbol;
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
        self.contracts(machine.contracts)?;
        if machine.target_family_interface {
            self.dependencies.pending_target_observation = true;
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
        for equation in resolved_declarations(program)
            .declared_type_equations
            .span_or_empty(machine.declared_type_equations)
        {
            self.queue_types([&equation.parameter, &equation.definition]);
        }
        if let Some(reference) = &machine.attached_data_application {
            self.queue_type(reference);
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
            self.queue_type(&owned.type_reference);
            if owned.initial_value.is_valid() {
                self.queue_expression(owned.initial_value);
            }
        }
        let expressions = &program.tables.bodies.expressions;
        for handle in states {
            let state = program.machine_state(*handle);
            self.contracts(state.contracts)?;
            self.state_parameters(state.parameters)?;
            if let Some(reference) = &state.return_type {
                self.queue_type(reference);
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
                                self.queue_expressions(expressions.iter().copied());
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
                    Statement::Expression(expression) => self.queue_expression(*expression),
                    Statement::LocalData(local) => {
                        self.queue_type(&local.type_reference);
                        if local.initial_value.is_valid() {
                            self.queue_expression(local.initial_value);
                        }
                    }
                    Statement::Assignment(assignment) => {
                        self.queue_expressions([assignment.target, assignment.value])
                    }
                    Statement::RootBinding(binding) => {
                        self.queue_expression(binding.receiver);
                        if binding.implementation_operand.is_valid() {
                            self.queue_expression(binding.implementation_operand);
                        }
                    }
                    Statement::AssemblyFact(fact) => self.queue_expression(fact.expression),
                    Statement::ProofOutputBindingStatement(binding) => {
                        self.queue_expression(binding.call)
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
                            self.call_dependency(
                                crate::selection::body_calls::CallSite {
                                    receiver_symbol: call.receiver_symbol,
                                    target_symbol: call.target_symbol,
                                    target: &call.target,
                                    member: &call.target,
                                    requirement_application: arena::Handle::invalid(),
                                    local_receiver: call.receiver.is_empty(),
                                    argument_count: expressions
                                        .expression_handles(call.arguments)
                                        .len(),
                                },
                                call.target.source_span(),
                                &call.machine_arguments,
                            )?;
                        }
                        let arguments = expressions.expression_handles(call.arguments);
                        if arguments.len() != call.arguments.len() {
                            return Err("constant statement call has stale arguments".to_owned());
                        }
                        self.queue_expressions(arguments.iter().rev().copied());
                    }
                    Statement::Transition(transition) => {
                        if let TransitionGuard::When(guard) = &transition.guard {
                            self.queue_expression(*guard);
                        }
                        for target in once(&transition.target).chain(transition.continuation.iter())
                        {
                            match target {
                                TransitionTarget::Value(value) => self.queue_expression(*value),
                                TransitionTarget::Named(target) => {
                                    let path = resolved_declarations(program)
                                        .statement_path_members
                                        .span_or_empty(target.path);
                                    let name = path
                                        .last()
                                        .ok_or("constant transition lost its callable name")?;
                                    self.call_dependency(
                                        crate::selection::body_calls::CallSite {
                                            receiver_symbol: target.receiver_symbol,
                                            target_symbol: target.symbol,
                                            target: name,
                                            member: name,
                                            requirement_application: arena::Handle::invalid(),
                                            local_receiver: path.len() == 1,
                                            argument_count: expressions
                                                .expression_handles(target.arguments)
                                                .len(),
                                        },
                                        target.source_span,
                                        &[],
                                    )?;
                                    let values = expressions.expression_handles(target.arguments);
                                    if values.len() != target.arguments.len() {
                                        return Err(
                                            "constant transition has stale arguments".to_owned()
                                        );
                                    }
                                    self.queue_expressions(values.iter().rev().copied());
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
