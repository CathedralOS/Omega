/*
    Declaration-owned dependency discovery before provisional values exist.

    A static callable has two independent dependencies: its authored binder is
    the call's logical selection, and its explicit operand supplies the body
    that specialization may execute. Retain both without treating an abstract
    binder as an ordinary body or granting execution from source readiness.
*/

use crate::declaration_selection::{
    AuthoredDeclarationSelectionTarget, ResolvedAuthoredCallBinding,
};
use crate::resolution::selections::call_identity::{
    call_declaration_source, selected_declaration_source,
};
use crate::resolution::tables::body_expressions;
use crate::resolution::tables::resolved_declarations;
use crate::source_resolved::domain::ProofFact;
use crate::source_resolved::expression::StaticMachineArgument;
use crate::source_resolved::signature::SignatureContract;
use crate::source_resolved::types::TypeReference;
use crate::source_resolved::{
    SymbolResolvedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    machine::MachineBodyStatus,
};
use language_core::data_layout_queries::is_data_layout_query_spelling;
use source_text::SourceSpan;
use std::iter::once;
use symbols::{SymbolHandle, SymbolKind};

mod types {
    /*
        Type work belongs to the demanded computation too. A selected record's
        extent may depend on a constant even when its initializer is never read as
        an expression. Discover that dependency before probe placeholders exist.
    */

    use super::{Collector, DependencyWork};
    use crate::source_resolved::{
        data::{DataMember, MachineParameterContract, TypeParameter, TypeParameterKind},
        expression::StaticMachineArgument,
        signature::StateParameter,
        types::{FixedArrayLength, TypeConstraint, TypeReference},
    };
    use arena::{Handle, HandleSpan};
    use source_text::SourceSpan;
    use symbols::{SymbolHandle, SymbolKind};

    impl<'program> Collector<'program, '_> {
        pub(super) fn constant(
            &mut self,
            symbol: SymbolHandle,
            reference: SourceSpan,
        ) -> Result<(), String> {
            let mut declarations = self
                .program
                .const_declarations
                .iter()
                .filter(|declaration| declaration.symbol == symbol);
            let declaration = declarations
                .next()
                .ok_or("constant dependency lost its retained declaration")?;
            if declarations.next().is_some() {
                return Err("constant dependency has ambiguous declaration custody".to_owned());
            }
            let source = self
                .program
                .symbols
                .symbol_source_span(symbol)
                .ok_or("constant dependency lost its declaration source")?;
            let dependency = (reference, source);
            if !self.dependencies.constants.contains(&dependency) {
                self.dependencies.constants.push(dependency);
            }
            if self.follow_dependencies && !self.expanded_constants.contains(&symbol) {
                self.expanded_constants.push(symbol);
                self.types.push(DependencyWork {
                    value: &declaration.declared_type,
                    lexical_owner: symbol,
                });
                let initializer = if declaration.authored_initializer.is_valid() {
                    declaration.authored_initializer
                } else {
                    declaration.initializer
                };
                self.expressions.push(DependencyWork {
                    value: initializer,
                    lexical_owner: symbol,
                });
                let work = DependencyWork {
                    value: initializer,
                    lexical_owner: symbol,
                };
                if !self.initializer_expressions.contains(&work) {
                    self.initializer_expressions.push(work);
                }
            }
            Ok(())
        }

        pub(super) fn type_child(&mut self, handle: Handle<TypeReference>) -> Result<(), String> {
            if !self.follow_dependencies
                || self
                    .visited_type_children
                    .contains(&(handle, self.lexical_owner))
            {
                return Ok(());
            }
            let children = &self.program.tables.declarations.child_type_references;
            if !children.is_valid(handle) {
                return Err("constant dependency has a stale child type reference".to_owned());
            }
            self.visited_type_children
                .push((handle, self.lexical_owner));
            self.queue_type(children.get(handle));
            Ok(())
        }

        pub(super) fn type_arguments(
            &mut self,
            arguments: HandleSpan<TypeReference>,
        ) -> Result<(), String> {
            if !self.follow_dependencies {
                return Ok(());
            }
            let children = self.program.child_type_references(arguments);
            if children.len() != arguments.len() {
                return Err("constant dependency has stale type arguments".to_owned());
            }
            for argument in arguments.handles() {
                self.type_child(argument)?;
            }
            Ok(())
        }

        pub(super) fn type_reference(
            &mut self,
            reference: &'program TypeReference,
        ) -> Result<(), String> {
            match reference {
                TypeReference::AliasUse {
                    expansion,
                    application,
                    ..
                } => {
                    self.queue_type(expansion);
                    self.queue_type(application);
                }
                TypeReference::Callable { contract, .. } => {
                    let row = self
                        .program
                        .tables
                        .declarations
                        .runtime_callable_contracts
                        .get(*contract);
                    self.signature(&row.signature)?;
                }
                TypeReference::Reference(reference) => self.type_child(reference.referee)?,
                TypeReference::Slice(reference) => self.type_child(reference.element_type)?,
                TypeReference::FixedArray(array) => {
                    self.type_child(array.element_type)?;
                    match &array.length {
                        FixedArrayLength::ConstParameter { symbol, name }
                            if self.program.symbols.get(*symbol).kind == SymbolKind::Const =>
                        {
                            self.constant(*symbol, name.source_span())?;
                        }
                        FixedArrayLength::Expression(value) => self.type_child(*value)?,
                        FixedArrayLength::ConstCall { machine, name } => {
                            self.call(*machine, name.source_span())?;
                        }
                        FixedArrayLength::ConstParameter { .. } | FixedArrayLength::Literal(_) => {}
                    }
                }
                TypeReference::Constrained(reference) => {
                    self.type_child(reference.base_type)?;
                    let constraints = self
                        .program
                        .tables
                        .types
                        .constraints
                        .span_or_empty(reference.constraints);
                    if constraints.len() != reference.constraints.len() {
                        return Err("constant dependency has stale type constraints".to_owned());
                    }
                    for constraint in constraints {
                        match constraint {
                            TypeConstraint::Domain(domain) => {
                                self.type_arguments(domain.arguments)?
                            }
                            TypeConstraint::Named(_) | TypeConstraint::ArithmeticDomain(_) => {}
                        }
                    }
                }
                TypeReference::Generic(application) => {
                    self.type_declaration(application.base_symbol)?;
                    self.type_arguments(application.arguments)?;
                }
                TypeReference::Named { symbol, name, .. }
                    if self.program.symbols.get(*symbol).kind == SymbolKind::Const =>
                {
                    self.constant(*symbol, name.source_span())?;
                }
                TypeReference::Named { symbol, .. } => {
                    self.type_declaration(*symbol)?;
                }
                TypeReference::ConstExpression(expression) => {
                    self.queue_expression(expression.expression)
                }
                TypeReference::Unit => {}
            }
            Ok(())
        }

        pub(super) fn type_declaration(&mut self, symbol: SymbolHandle) -> Result<(), String> {
            let previous = self.lexical_owner;
            self.lexical_owner = symbol;
            let result = self.type_declaration_in_scope(symbol);
            self.lexical_owner = previous;
            result
        }

        fn type_declaration_in_scope(&mut self, symbol: SymbolHandle) -> Result<(), String> {
            if self.program.symbols.get(symbol).kind == SymbolKind::Conformance {
                return self.conformance_declaration(symbol);
            }
            if self.program.symbols.get(symbol).kind == SymbolKind::TypeAlias {
                if !self.follow_dependencies || self.visited_type_declarations.contains(&symbol) {
                    return Ok(());
                }
                let mut aliases = self
                    .program
                    .type_aliases
                    .iter()
                    .filter(|alias| alias.symbol == symbol);
                let alias = aliases
                    .next()
                    .ok_or("constant dependency lost its selected alias declaration")?;
                if aliases.next().is_some() {
                    return Err(
                        "constant dependency has ambiguous alias declaration custody".into(),
                    );
                }
                self.visited_type_declarations.push(symbol);
                self.type_parameters(alias.type_parameters)?;
                for bound in &alias.conformance_bounds {
                    self.type_arguments(bound.arguments)?;
                }
                self.queue_type(&alias.expansion);
                return Ok(());
            }
            if !self.follow_dependencies
                || self.program.symbols.get(symbol).kind != SymbolKind::Data
                || self.visited_type_declarations.contains(&symbol)
            {
                return Ok(());
            }
            self.visited_type_declarations.push(symbol);
            let mut definitions = self
                .program
                .data_definitions
                .iter()
                .filter(|definition| definition.symbol == symbol);
            let definition = definitions
                .next()
                .ok_or("constant dependency lost its selected data declaration")?;
            if definitions.next().is_some() {
                return Err("constant dependency has ambiguous data declaration custody".to_owned());
            }
            self.type_parameters(definition.type_parameters)?;
            if self.purpose == super::DependencyPurpose::DeclarationReadiness {
                if let Some(application) = &definition.generic_instance {
                    self.queue_type(application);
                }
                self.predicate_facts(definition.requires_facts)?;
            }
            if let Some(quotient) = &definition.quotient {
                self.queue_type(&quotient.carrier);
            }
            let members = self.program.data_members(definition.members);
            if members.len() != definition.members.len() {
                return Err("constant dependency has stale data members".to_owned());
            }
            for member in members {
                match member {
                    DataMember::Field(field) => self.queue_type(&field.type_reference),
                    DataMember::Variant(variant) => {
                        if self.purpose == super::DependencyPurpose::DeclarationReadiness {
                            self.predicate_facts(variant.requires_facts)?;
                        }
                        let fields = self.program.data_payload_fields(variant.payload);
                        if fields.len() != variant.payload.len() {
                            return Err("constant dependency has stale variant fields".to_owned());
                        }
                        self.queue_types(fields.iter().map(|field| &field.type_reference));
                    }
                }
            }
            Ok(())
        }

        fn conformance_declaration(&mut self, symbol: SymbolHandle) -> Result<(), String> {
            if !self.follow_dependencies
                || self.purpose != super::DependencyPurpose::DeclarationReadiness
                || self.visited_type_declarations.contains(&symbol)
            {
                return Ok(());
            }
            let definitions = self
                .program
                .conformances
                .iter()
                .filter(|definition| definition.symbol == symbol)
                .collect::<Vec<_>>();
            let [definition] = definitions.as_slice() else {
                return Err(
                    "constant dependency lost its exact selected implementation declaration".into(),
                );
            };
            self.visited_type_declarations.push(symbol);
            self.type_parameters(definition.type_parameters)?;
            self.type_arguments(definition.arguments)?;
            for row in &definition.rows {
                let machine =
                    crate::resolution::names::owned_conformance_realization(self.program, row)?;
                if crate::resolution::names::owned_conformance_namespace(self.program, machine)?
                    != symbol
                {
                    return Err(
                        "constant dependency implementation member lost its exact owner".into(),
                    );
                }
                self.machines.push(machine);
            }
            Ok(())
        }

        pub(super) fn type_parameters(
            &mut self,
            parameters: HandleSpan<TypeParameter>,
        ) -> Result<(), String> {
            let parameters_slice = self
                .program
                .tables
                .declarations
                .data_type_parameters
                .span_or_empty(parameters);
            if parameters_slice.len() != parameters.len() {
                return Err("constant dependency has stale type parameters".to_owned());
            }
            for parameter in parameters_slice {
                match &parameter.kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => {
                        self.queue_type(type_reference)
                    }
                    TypeParameterKind::Machine {
                        contract: MachineParameterContract::Structural(signature),
                    } => {
                        self.signature(signature)?;
                    }
                    TypeParameterKind::Proposition { contract } => {
                        self.state_parameters(contract.parameters)?;
                    }
                    TypeParameterKind::Type
                    | TypeParameterKind::Implementation
                    | TypeParameterKind::Machine { .. } => {}
                }
            }
            Ok(())
        }

        fn signature(
            &mut self,
            signature: &'program crate::source_resolved::signature::StateSignature,
        ) -> Result<(), String> {
            let previous = self.lexical_owner;
            self.lexical_owner = signature.symbol;
            let result = (|| {
                self.contracts(signature.contracts)?;
                self.type_parameters(signature.type_parameters)?;
                self.state_parameters(signature.parameters)?;
                if let Some(reference) = &signature.return_type {
                    self.queue_type(reference);
                }
                for bound in &signature.conformance_bounds {
                    self.type_arguments(bound.arguments)?;
                }
                Ok(())
            })();
            self.lexical_owner = previous;
            result
        }

        pub(super) fn state_parameters(
            &mut self,
            parameters: HandleSpan<StateParameter>,
        ) -> Result<(), String> {
            let parameters_slice = self
                .program
                .tables
                .declarations
                .state_parameters
                .span_or_empty(parameters);
            if parameters_slice.len() != parameters.len() {
                return Err("constant dependency has stale state parameters".to_owned());
            }
            self.queue_types(
                parameters_slice
                    .iter()
                    .map(|parameter| &parameter.type_reference),
            );
            Ok(())
        }

        pub(super) fn static_arguments(
            &mut self,
            arguments: &'program [StaticMachineArgument],
        ) -> Result<(), String> {
            if !self.follow_dependencies {
                return Ok(());
            }
            for argument in arguments {
                let selected = self.program.symbols.get(argument.symbol);
                match selected.kind {
                    SymbolKind::Conformance => self.conformance_declaration(argument.symbol)?,
                    SymbolKind::Machine => self.machines.push(argument.symbol),
                    SymbolKind::State
                        if self.program.symbols.get(selected.parent).kind
                            == SymbolKind::Machine =>
                    {
                        self.machines.push(selected.parent);
                    }
                    _ => {}
                }
                if argument.type_reference.is_valid() {
                    self.type_child(argument.type_reference)?;
                }
                if let Some(application) = &argument.application {
                    self.static_arguments(&application.arguments)?;
                }
            }
            Ok(())
        }
    }
}

/*
    Declaration preparation may visit every owned implementation member. An
    authored invocation closure stops at its logical bound until realization.
*/
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DependencyPurpose {
    Authored,
    DeclarationReadiness,
}

/*
    Exact source occurrence/declaration pairs. Body dependencies are retained
    even when execution later selects another branch.
*/
#[derive(Debug, Default)]
pub struct ConstInitializerDependencies {
    /* A selected layout observation remains symbolic until target closure. */
    pub pending_target_observation: bool,
    /*
        Scoped overload choices still owed by ordinary typed call selection.
        Candidate availability contributes no selected-body constant dependency.
    */
    pub pending_calls: Vec<(SourceSpan, Vec<SourceSpan>)>,
    pub constants: Vec<(SourceSpan, SourceSpan)>,
    pub calls: Vec<(SourceSpan, SourceSpan)>,
    pub initializer_operators: Vec<SourceSpan>,
    /* Exact authored owners whose type applications belong to this dependency closure. */
    pub normalization_owners: Vec<SourceSpan>,
    pub(crate) call_targets: Vec<AuthoredCallDependency>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AuthoredCallDependency {
    pub reference: SourceSpan,
    pub declaration: SourceSpan,
    pub target: SymbolHandle,
}

/*
    Exact machine roots and their resolved call dependencies. A deferred body
    terminates traversal: complete `deferred_machines` and collect again before
    treating `machines` as the complete execution roster. This is body demand,
    not permission to execute or evidence of checked call selection.
*/
#[derive(Debug, Default)]
pub struct ResolvedMachineBodyDemand {
    #[cfg_attr(not(any(test, feature = "test-support")), allow(dead_code))]
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
        purpose: DependencyPurpose::DeclarationReadiness,
        ..Collector::new(program)
    };
    collector.collect()?;
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

/*
    A declaration's destination participates in readiness independently of the
    expression leaves. In particular an empty/literal array still demands its
    exact extent before its value can be encoded or selected by another job.
*/
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
            crate::resolution::names::owned_conformance_namespace(collector.program, *symbol)?
        } else {
            SymbolHandle::invalid()
        };
        let owner = if namespace.is_valid() {
            namespace
        } else {
            *symbol
        };
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

/*
    Bind constant-initializer calls through a named implementation
    to its public requirement before authored-selection recording. Every copy
    of those source tokens is bound too: selection finalization groups copies
    by token, and one unresolved copy keeps a late call selection. The private
    realization supplies body demand separately, just as for machine-body calls.
*/
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
        crate::resolution::selections::conformance_blocks::ProvisionalBodyCallRouting::new(program);
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
            &crate::resolution::selections::body_calls::CallSite {
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

/* The node a handle names, copied out before a walk mutates the table. */
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
    /*
        Default bodies can share source nodes across distinct realizations.
        Their queued work and visited keys must keep each row's exact owner.
    */
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
    body_call_routing:
        crate::resolution::selections::conformance_blocks::ProvisionalBodyCallRouting<'program>,
    dependencies: ConstInitializerDependencies,
    expressions: Vec<DependencyWork<ExpressionHandle>>,
    lexical_owner: SymbolHandle,
    /*
        Exact expression-tree membership, independent of diagnostic spans.
        Machine bodies and declaration type work enter their separate queues;
        following a call does not make its body an initializer expression.
    */
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
    /*
        Only a retained-body owner can request an incomplete dependency frontier.
        Ordinary initializer collection requires every reached body to exist.
    */
    deferred_machines: Option<Vec<SymbolHandle>>,
    follow_dependencies: bool,
    purpose: DependencyPurpose,
    /*
        Typed choices disambiguate unresolved overload families. Readiness can
        also visit independently selected member bodies absent from that hint.
        Final normalization compares the complete reconstructed source roster.
    */
    selected_calls: Option<&'selection [(SourceSpan, SourceSpan)]>,
}

impl<'program, 'selection> Collector<'program, 'selection> {
    fn new(program: &'program SymbolResolvedTrees) -> Self {
        Self {
            program,
            body_call_routing:
                crate::resolution::selections::conformance_blocks::ProvisionalBodyCallRouting::new(
                    program,
                ),
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
            return self.record_logical_call(target, reference);
        }
        let machine = match symbol.kind {
            SymbolKind::Machine => target,
            SymbolKind::State
                if self.program.symbols.get(symbol.parent).kind == SymbolKind::Machine =>
            {
                symbol.parent
            }
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
        let declaration = selected_declaration_source(self.program, target)?;
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
        site: crate::resolution::selections::body_calls::CallSite<'_>,
        target_span: SourceSpan,
        static_arguments: &[StaticMachineArgument],
    ) -> Result<(), String> {
        let receiver_symbol = site.receiver_symbol;
        let target_symbol = site.target_symbol;
        let target_name = site.member.as_str();
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
                let native_query = program
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    == Some(symbols::BuiltinFunction::CodeLayout)
                    && !call.receiver.is_valid();
                if native_query && self.deferred_machines.is_none() {
                    return Err("late native code-layout metadata is unavailable to same-product semantic constant, type, layout, or optimization evaluation".to_owned());
                }
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
                        crate::resolution::selections::body_calls::CallSite {
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
                } else if let ExpressionNode::Name(path) = table.expression(expression) {
                    match program.symbols.get(path.symbol).kind {
                        SymbolKind::Variant => {
                            self.type_declaration(program.symbols.get(path.symbol).parent)?
                        }
                        SymbolKind::Machine | SymbolKind::State => {
                            // Taking code needs its exact body available,
                            // without publishing an invocation or provider edge.
                            let symbol = program.symbols.get(path.symbol);
                            let direct = if symbol.kind == SymbolKind::Machine {
                                Some(path.symbol)
                            } else if program.symbols.get(symbol.parent).kind == SymbolKind::Machine
                            {
                                Some(symbol.parent)
                            } else {
                                None
                            };
                            let owner = if let Some(owner) = direct {
                                Some(owner)
                            } else if let Some(conformance) = program
                                .conformances
                                .iter()
                                .find(|conformance| conformance.symbol == path.head_symbol)
                            {
                                let mut rows = conformance
                                    .rows
                                    .iter()
                                    .filter(|row| row.requirement == path.symbol);
                                if let Some(row) = rows.next() {
                                    if rows.next().is_some() {
                                        return Err("named code dependency has ambiguous implementation rows".into());
                                    }
                                    Some(if row.source.owns_realization() {
                                        crate::resolution::names::owned_conformance_realization(
                                            program, row,
                                        )
                                        .map_err(str::to_owned)?
                                    } else {
                                        row.realization_machine
                                    })
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            if let Some(owner) = owner.filter(|owner| owner.is_valid())
                                && self.follow_dependencies
                                && !self.available_machines.contains(&owner)
                            {
                                self.available_machines.push(owner);
                            }
                        }
                        _ => {}
                    }
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

    /*
        Invocation formation demands the complete published clauses before a
        body can execute. A helper whose value body is independent can still
        need selected providers to form its requires/ensures operands.
    */
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

    /*
        Type/default-domain and invocation predicates share the same exact
        dependency roster. Readiness selects their bodies; it proves no clause.
    */
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
        use crate::source_resolved::statement::{Statement, TransitionGuard, TransitionTarget};
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
            let body = program.construction_statements(state.construction_statement_span());
            if body.len() != state.construction_statement_span().len() {
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
                        let native_query = program
                            .symbols
                            .builtin_function_for_symbol(call.target_symbol)
                            == Some(symbols::BuiltinFunction::CodeLayout)
                            && call.receiver.is_empty();
                        if native_query && self.deferred_machines.is_none() {
                            return Err("late native code-layout metadata is unavailable to same-product semantic constant, type, layout, or optimization evaluation".to_owned());
                        }
                        if !native_query {
                            self.call_dependency(
                                crate::resolution::selections::body_calls::CallSite {
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
                                        crate::resolution::selections::body_calls::CallSite {
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
