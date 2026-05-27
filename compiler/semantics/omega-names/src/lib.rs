//! Name and module resolution.
//!
//! This crate owns the first post-parse view of names. The current report is
//! intentionally shallow: it records imports, top-level definitions, and
//! syntactic references without pretending that every reference is fully bound.
//! That gives later phases a concrete spine to grow from.

use omega_core::arena::{Arena, HandleSpan};
use omega_core::source::{SourceMap, SourceSpan};
use omega_core::symbols::{
    BuiltinType, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder,
    builtin_type_member_symbols, builtin_type_symbols,
};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{CapabilityMember, DataMember, Item};
use omega_syntax_trees::statement::{StatementHandle, StatementNode, TransitionTargetHandle};
use omega_syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use std::sync::Arc;

type SymbolSeed<'name> = (SymbolKind, SymbolNameRef<'name>);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolveReport {
    pub definitions: Arena<ResolvedDefinition>,
    pub imports: Arena<ResolvedImport>,
    pub references: Arena<ResolvedReference>,
    pub name_members: Arena<ResolvedNameMember>,
    pub symbols: SymbolTable,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedDefinition {
    pub kind: ResolvedDefinitionKind,
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolvedDefinitionKind {
    Capability,
    Data,
    Domain,
    Invariant,
    Library,
    Machine,
    Operator,
    Platform,
    Target,
    Trait,
    Trust,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedImport {
    pub path: HandleSpan<ResolvedNameMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedReference {
    pub name: HandleSpan<ResolvedNameMember>,
    pub kind: ResolvedReferenceKind,
    pub owner: String,
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ResolvedNameMember {
    #[default]
    Missing,
    Source(SourceSpan),
    Generated(Arc<str>),
}

impl ResolvedNameMember {
    pub fn from_identifier(identifier: &Identifier) -> Self {
        if identifier.is_source_backed() {
            Self::Source(identifier.source_span())
        } else {
            Self::Generated(Arc::from(identifier.as_str()))
        }
    }

    pub fn as_str<'source>(&'source self, symbols: &'source SymbolTable) -> &'source str {
        match self {
            Self::Missing => "",
            Self::Source(source_span) => symbols.source_text(*source_span),
            Self::Generated(value) => value.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolvedReferenceKind {
    CallTarget,
    ExpressionName,
    Invariant,
    StructLiteral,
    TransitionTarget,
    Type,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResolvedNameStorageCounts {
    pub missing: usize,
    pub source_members: usize,
    pub generated_members: usize,
}

impl ResolveReport {
    pub fn import_path(&self, import: &ResolvedImport) -> String {
        self.name_from_members(import.path)
    }

    pub fn reference_name(&self, reference: &ResolvedReference) -> String {
        self.name_from_members(reference.name)
    }

    fn name_from_members(&self, name: HandleSpan<ResolvedNameMember>) -> String {
        let members = self.name_members.span_or_empty(name);
        let byte_count = members
            .iter()
            .map(|member| member.as_str(&self.symbols).len())
            .sum::<usize>()
            + "::".len().saturating_mul(members.len().saturating_sub(1));
        let mut name = String::with_capacity(byte_count);

        for (index, member) in members.iter().enumerate() {
            if index > 0 {
                name.push_str("::");
            }

            name.push_str(member.as_str(&self.symbols));
        }

        name
    }

    pub fn name_storage_counts(&self) -> ResolvedNameStorageCounts {
        let mut counts = ResolvedNameStorageCounts::default();

        for (_, member) in self.name_members.iter() {
            match member {
                ResolvedNameMember::Missing => counts.missing += 1,
                ResolvedNameMember::Source(_) => counts.source_members += 1,
                ResolvedNameMember::Generated(_) => counts.generated_members += 1,
            }
        }

        counts
    }
}

pub fn build_resolve_report(syntax_trees: &SyntaxTrees, sources: Arc<SourceMap>) -> ResolveReport {
    build_resolve_report_with_optional_sources(syntax_trees, Some(sources))
}

pub fn build_resolve_report_without_sources(syntax_trees: &SyntaxTrees) -> ResolveReport {
    build_resolve_report_with_optional_sources(syntax_trees, None)
}

fn build_resolve_report_with_optional_sources(
    syntax_trees: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
) -> ResolveReport {
    let mut report = ResolveReport::default();
    report.symbols = build_source_symbol_table(syntax_trees, sources);

    for item in syntax_trees.root_items() {
        match item {
            Item::Capability(capability) => {
                insert_definition(
                    &mut report,
                    capability.name.as_str(),
                    ResolvedDefinitionKind::Capability,
                );

                for member in syntax_trees.items.capability_members(capability.members) {
                    match member {
                        CapabilityMember::Field(field) => {
                            collect_type_reference(
                                &mut report,
                                syntax_trees,
                                field.type_reference,
                                &format!("capability `{}` field `{}`", capability.name, field.name),
                            );
                        }
                        CapabilityMember::State(state) => {
                            collect_inline_state_signature_references(
                                &mut report,
                                syntax_trees,
                                &state.signature,
                                &format!(
                                    "capability `{}` state `{}`",
                                    capability.name, state.signature.name
                                ),
                            );
                        }
                    }
                }
            }
            Item::Data(data_definition) => {
                insert_definition(
                    &mut report,
                    data_definition.name.as_str(),
                    ResolvedDefinitionKind::Data,
                );

                for member in syntax_trees.items.data_members(data_definition.members) {
                    if let DataMember::Field(field) = member {
                        collect_type_reference(
                            &mut report,
                            syntax_trees,
                            field.type_reference,
                            &format!("data `{}` field `{}`", data_definition.name, field.name),
                        );
                    }
                }
            }
            Item::Domain(domain) => {
                insert_definition(
                    &mut report,
                    domain.name.as_str(),
                    ResolvedDefinitionKind::Domain,
                );
                let domain_symbol = report
                    .symbols
                    .find_child_by_name(report.symbols.root(), domain.name.as_str())
                    .unwrap_or_else(SymbolHandle::invalid);
                collect_type_reference(
                    &mut report,
                    syntax_trees,
                    domain.target_type,
                    &format!("domain `{}` target type", domain.name),
                );
                collect_domain_operator_definitions(
                    &mut report,
                    syntax_trees,
                    domain.name.as_str(),
                    domain_symbol,
                    domain.operators,
                );
            }
            Item::Invariant(invariant) => {
                insert_definition(
                    &mut report,
                    invariant.name.as_str(),
                    ResolvedDefinitionKind::Invariant,
                );

                collect_constraints(
                    &mut report,
                    syntax_trees,
                    invariant.constraints,
                    &format!("invariant `{}`", invariant.name),
                );
            }
            Item::Library(library) => {
                if let Some(name) = &library.name {
                    insert_definition(&mut report, name.as_str(), ResolvedDefinitionKind::Library);
                }

                for function in syntax_trees.items.library_functions(library.functions) {
                    collect_inline_state_signature_references(
                        &mut report,
                        syntax_trees,
                        &function.signature,
                        &format!(
                            "library `{}` function `{}`",
                            library
                                .name
                                .as_ref()
                                .map(ToString::to_string)
                                .unwrap_or_else(|| library.path.clone()),
                            function.signature.name
                        ),
                    );
                }
            }
            Item::Use(use_item) => {
                let path = insert_name_members(
                    &mut report,
                    syntax_trees.items.identifier_path_members(use_item.path),
                );
                report.imports.insert(ResolvedImport { path });
            }
            Item::Machine(machine) => {
                insert_definition(
                    &mut report,
                    machine.name.as_str(),
                    ResolvedDefinitionKind::Machine,
                );
                collect_machine_references(&mut report, syntax_trees, machine);
            }
            Item::Operator(operator) => {
                insert_definition(
                    &mut report,
                    &operator_name(syntax_trees, operator.name),
                    ResolvedDefinitionKind::Operator,
                );
                collect_state_signature_parts(
                    &mut report,
                    syntax_trees,
                    operator.parameters,
                    operator.return_type,
                    "operator declaration",
                );
            }
            Item::Platform(platform) => {
                insert_definition(
                    &mut report,
                    platform.name.as_str(),
                    ResolvedDefinitionKind::Platform,
                );

                for signature_handle in syntax_trees.items.state_signatures(platform.states) {
                    collect_state_signature_references(
                        &mut report,
                        syntax_trees,
                        *signature_handle,
                        &format!(
                            "platform `{}` state `{}`",
                            platform.name,
                            syntax_trees.items.state_signature(*signature_handle).name
                        ),
                    );
                }
            }
            Item::Trait(trait_definition) => {
                insert_definition(
                    &mut report,
                    trait_definition.name.as_str(),
                    ResolvedDefinitionKind::Trait,
                );

                for signature_handle in syntax_trees
                    .items
                    .state_signatures(trait_definition.machines)
                {
                    collect_state_signature_references(
                        &mut report,
                        syntax_trees,
                        *signature_handle,
                        &format!(
                            "trait `{}` machine `{}`",
                            trait_definition.name,
                            syntax_trees.items.state_signature(*signature_handle).name
                        ),
                    );
                }
            }
            Item::Target(target) => {
                insert_definition(
                    &mut report,
                    target.name.as_str(),
                    ResolvedDefinitionKind::Target,
                );
            }
            Item::TrustDefinition(trust_definition) => {
                insert_definition(
                    &mut report,
                    trust_definition.name.as_str(),
                    ResolvedDefinitionKind::Trust,
                );
            }
            Item::Export(_) => {}
        }
    }

    report
}

fn insert_definition(report: &mut ResolveReport, name: &str, kind: ResolvedDefinitionKind) {
    let symbol = report
        .symbols
        .find_child_by_name(report.symbols.root(), name)
        .unwrap_or_else(SymbolHandle::invalid);
    report
        .definitions
        .insert(ResolvedDefinition { kind, symbol });
}

fn insert_child_definition(
    report: &mut ResolveReport,
    parent: SymbolHandle,
    name: &str,
    kind: ResolvedDefinitionKind,
) {
    let symbol = report
        .symbols
        .find_child_by_name(parent, name)
        .unwrap_or_else(SymbolHandle::invalid);
    report
        .definitions
        .insert(ResolvedDefinition { kind, symbol });
}

fn collect_domain_operator_definitions(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    domain_name: &str,
    domain_symbol: SymbolHandle,
    operators: HandleSpan<omega_syntax_trees::item::OperatorDefinition>,
) {
    for operator in syntax_trees.items.operators(operators) {
        let operator_name = operator_name(syntax_trees, operator.name);
        insert_child_definition(
            report,
            domain_symbol,
            &operator_name,
            ResolvedDefinitionKind::Operator,
        );
        collect_state_signature_parts(
            report,
            syntax_trees,
            operator.parameters,
            operator.return_type,
            &format!("domain `{domain_name}` operator `{operator_name}`"),
        );
    }
}

fn collect_machine_references(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    machine: &omega_syntax_trees::item::Machine,
) {
    let machine_symbol = report
        .symbols
        .find_child_by_name(report.symbols.root(), machine.name.as_str())
        .unwrap_or_else(SymbolHandle::invalid);

    for state_handle in syntax_trees.items.state_handles(machine.states) {
        collect_state_references(report, syntax_trees, machine, machine_symbol, *state_handle);
    }
}

fn collect_state_references(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    machine: &omega_syntax_trees::item::Machine,
    machine_symbol: SymbolHandle,
    state_handle: omega_syntax_trees::item::StateHandle,
) {
    let state = syntax_trees.items.state(state_handle);
    let state_symbol = report
        .symbols
        .find_child_by_name(machine_symbol, state.name.as_str())
        .unwrap_or_else(SymbolHandle::invalid);
    let context = ResolveContext::from_symbols(machine_symbol, state_symbol);

    collect_state_signature_parts(
        report,
        syntax_trees,
        state.parameters,
        state.return_type,
        &format!("machine `{}` state `{}`", machine.name, state.name),
    );

    for statement_handle in syntax_trees.items.statements(state.statements) {
        collect_statement(
            report,
            syntax_trees,
            *statement_handle,
            &format!("machine `{}` state `{}`", machine.name, state.name),
            context,
        );
    }
}

fn collect_state_signature_references(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    signature_handle: omega_syntax_trees::item::StateSignatureHandle,
    owner: &str,
) {
    let signature = syntax_trees.items.state_signature(signature_handle);
    collect_state_signature_parts(
        report,
        syntax_trees,
        signature.parameters,
        signature.return_type,
        owner,
    );
}

fn collect_inline_state_signature_references(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    signature: &omega_syntax_trees::item::StateSignature,
    owner: &str,
) {
    collect_state_signature_parts(
        report,
        syntax_trees,
        signature.parameters,
        signature.return_type,
        owner,
    );
}

fn collect_state_signature_parts(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    parameters: HandleSpan<omega_syntax_trees::item::StateParameterHandle>,
    return_type: TypeReferenceHandle,
    owner: &str,
) {
    for parameter_handle in syntax_trees.items.state_parameters(parameters) {
        let parameter = syntax_trees.items.state_parameter(*parameter_handle);
        collect_type_reference(
            report,
            syntax_trees,
            parameter.type_reference,
            &format!("{owner} parameter `{}`", parameter.name),
        );
    }

    if return_type.is_valid() {
        collect_type_reference(
            report,
            syntax_trees,
            return_type,
            &format!("{owner} return type"),
        );
    }
}

fn collect_statement(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    statement: StatementHandle,
    owner: &str,
    context: ResolveContext,
) {
    match syntax_trees.statements.statement(statement) {
        StatementNode::Assignment(assignment) => {
            collect_expression(
                report,
                syntax_trees,
                assignment.target,
                &format!("{owner} assignment target"),
                context,
            );
            collect_expression(
                report,
                syntax_trees,
                assignment.value,
                &format!("{owner} assignment value"),
                context,
            );
        }
        StatementNode::Call(call) => {
            let receiver = syntax_trees
                .statements
                .identifier_path_members(call.receiver);
            let symbol = context.resolve_call_target(
                &report.symbols,
                if receiver.is_empty() {
                    None
                } else {
                    Some(receiver)
                },
                call.target.as_str(),
            );

            let name =
                insert_name_members(report, receiver.iter().chain(std::iter::once(&call.target)));
            report.references.insert(ResolvedReference {
                name,
                kind: ResolvedReferenceKind::CallTarget,
                owner: owner.to_owned(),
                symbol,
            });

            for argument in syntax_trees.statements.expression_handles(call.arguments) {
                collect_expression(
                    report,
                    syntax_trees,
                    *argument,
                    &format!("{owner} call argument"),
                    context,
                );
            }
        }
        StatementNode::Expression(expression) => {
            collect_expression(report, syntax_trees, *expression, owner, context);
        }
        StatementNode::LocalData(local_data) => {
            collect_type_reference(
                report,
                syntax_trees,
                local_data.type_reference,
                &format!("{owner} local `{}`", local_data.name),
            );
        }
        StatementNode::Transition(transition) => {
            collect_transition_target(report, syntax_trees, transition.target, owner, context);

            if transition.continuation.is_valid() {
                collect_transition_target(
                    report,
                    syntax_trees,
                    transition.continuation,
                    owner,
                    context,
                );
            }

            if let omega_syntax_trees::statement::TransitionGuardNode::When(guard) =
                transition.guard
            {
                collect_expression(
                    report,
                    syntax_trees,
                    guard,
                    &format!("{owner} transition guard"),
                    context,
                );
            }
        }
    }
}

fn collect_transition_target(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    target: TransitionTargetHandle,
    owner: &str,
    context: ResolveContext,
) {
    if let omega_syntax_trees::statement::TransitionTargetNode::Named {
        path, arguments, ..
    } = syntax_trees.statements.transition_target(target)
    {
        let path_members = syntax_trees.statements.identifier_path_members(*path);
        let symbol = context.resolve_identifier_members(&report.symbols, path_members);
        insert_reference_from_members(
            report,
            path_members,
            ResolvedReferenceKind::TransitionTarget,
            owner,
            symbol,
        );

        for argument in syntax_trees.statements.expression_handles(*arguments) {
            collect_expression(
                report,
                syntax_trees,
                *argument,
                &format!("{owner} transition argument"),
                context,
            );
        }
    }
}

fn collect_type_reference(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    owner: &str,
) {
    match syntax_trees.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_reference(report, syntax_trees, *referee, owner);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_reference(report, syntax_trees, *base_type, owner);
            collect_constraints(report, syntax_trees, *constraints, owner);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_type_reference(report, syntax_trees, *element_type, owner);
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_type_reference(report, syntax_trees, *element_type, owner);
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
        } => {
            let symbol = resolve_global_name(report, base_name.as_str());
            insert_reference(
                report,
                base_name,
                ResolvedReferenceKind::Type,
                owner,
                symbol,
            );
            for argument in syntax_trees
                .type_references
                .type_reference_handles(*arguments)
            {
                collect_type_reference(report, syntax_trees, *argument, owner);
            }
        }
        TypeReferenceNode::Named(name) => {
            let symbol = resolve_global_name(report, name.as_str());
            insert_reference(report, name, ResolvedReferenceKind::Type, owner, symbol);
        }
        TypeReferenceNode::SelfType | TypeReferenceNode::Unit => {}
    }
}

fn collect_constraints(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    constraints: HandleSpan<TypeConstraintNode>,
    owner: &str,
) {
    for constraint in syntax_trees.type_references.constraints(constraints) {
        match constraint {
            TypeConstraintNode::Named(name) => {
                let symbol = resolve_global_name(report, name.as_str());
                insert_reference(
                    report,
                    name,
                    ResolvedReferenceKind::Invariant,
                    owner,
                    symbol,
                );
            }
            TypeConstraintNode::Range { minimum, maximum } => {
                collect_expression(
                    report,
                    syntax_trees,
                    *minimum,
                    &format!("{owner} range minimum"),
                    ResolveContext::default(),
                );
                collect_expression(
                    report,
                    syntax_trees,
                    *maximum,
                    &format!("{owner} range maximum"),
                    ResolveContext::default(),
                );
            }
        }
    }
}

fn collect_expression(
    report: &mut ResolveReport,
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
    owner: &str,
    context: ResolveContext,
) {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in syntax_trees.expressions.expression_handles(*values) {
                collect_expression(report, syntax_trees, *value, owner, context);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression(report, syntax_trees, binary.left, owner, context);
            collect_expression(report, syntax_trees, binary.right, owner, context);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::SelfValue
        | ExpressionNode::String(_) => {}
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                match syntax_trees.expressions.expression(call.receiver) {
                    ExpressionNode::Name(path) => {
                        let path_members = syntax_trees.expressions.identifier_path_members(*path);
                        let symbol = context.resolve_call_target(
                            &report.symbols,
                            Some(path_members),
                            call.target.as_str(),
                        );
                        let name = insert_name_members(
                            report,
                            path_members.iter().chain(std::iter::once(&call.target)),
                        );
                        report.references.insert(ResolvedReference {
                            name,
                            kind: ResolvedReferenceKind::CallTarget,
                            owner: owner.to_owned(),
                            symbol,
                        });
                    }
                    _ => collect_expression(report, syntax_trees, call.receiver, owner, context),
                }
            } else {
                let symbol =
                    context.resolve_call_target(&report.symbols, None, call.target.as_str());
                insert_reference(
                    report,
                    &call.target,
                    ResolvedReferenceKind::CallTarget,
                    owner,
                    symbol,
                );
            }

            for argument in syntax_trees.expressions.expression_handles(call.arguments) {
                collect_expression(report, syntax_trees, *argument, owner, context);
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_expression(report, syntax_trees, cast.value, owner, context);
            for member in syntax_trees
                .expressions
                .identifier_path_members(cast.target_type)
            {
                let symbol = resolve_global_name(report, member.as_str());
                insert_reference(report, member, ResolvedReferenceKind::Type, owner, symbol);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression(report, syntax_trees, indexed.collection, owner, context);
            collect_expression(report, syntax_trees, indexed.index, owner, context);
        }
        ExpressionNode::Membership(membership) => {
            collect_expression(report, syntax_trees, membership.value, owner, context);
            let path_members = syntax_trees.items.identifier_path_members(membership.domain);
            let symbol = context.resolve_identifier_members(&report.symbols, path_members);
            insert_reference_from_members(
                report,
                path_members,
                ResolvedReferenceKind::Type,
                owner,
                symbol,
            );
        }
        ExpressionNode::Member(member) => {
            collect_expression(report, syntax_trees, member.receiver, owner, context);
        }
        ExpressionNode::Mutable(inner) => {
            collect_expression(report, syntax_trees, *inner, owner, context);
        }
        ExpressionNode::Name(path) => {
            let path_members = syntax_trees.expressions.identifier_path_members(*path);
            let symbol = context.resolve_identifier_members(&report.symbols, path_members);
            insert_reference_from_members(
                report,
                path_members,
                ResolvedReferenceKind::ExpressionName,
                owner,
                symbol,
            );
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                collect_expression(report, syntax_trees, range.start, owner, context);
            }
            if range.end.is_valid() {
                collect_expression(report, syntax_trees, range.end, owner, context);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            let symbol = resolve_global_name(report, struct_literal.type_name.as_str());
            insert_reference(
                report,
                &struct_literal.type_name,
                ResolvedReferenceKind::StructLiteral,
                owner,
                symbol,
            );

            for field in syntax_trees
                .expressions
                .struct_fields(struct_literal.fields)
            {
                collect_expression(report, syntax_trees, field.value, owner, context);
            }
        }
    }
}

fn insert_reference(
    report: &mut ResolveReport,
    name: &Identifier,
    kind: ResolvedReferenceKind,
    owner: &str,
    symbol: SymbolHandle,
) {
    insert_reference_from_members(report, [name], kind, owner, symbol);
}

fn insert_reference_from_members<'identifier>(
    report: &mut ResolveReport,
    identifiers: impl IntoIterator<Item = &'identifier Identifier>,
    kind: ResolvedReferenceKind,
    owner: &str,
    symbol: SymbolHandle,
) {
    let name = insert_name_members(report, identifiers);
    report.references.insert(ResolvedReference {
        name,
        kind,
        owner: owner.to_owned(),
        symbol,
    });
}

fn insert_name_members<'identifier>(
    report: &mut ResolveReport,
    identifiers: impl IntoIterator<Item = &'identifier Identifier>,
) -> HandleSpan<ResolvedNameMember> {
    report.name_members.insert_many(
        identifiers
            .into_iter()
            .map(ResolvedNameMember::from_identifier),
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ResolveContext {
    machine: SymbolHandle,
    state: SymbolHandle,
}

impl ResolveContext {
    fn from_symbols(machine: SymbolHandle, state: SymbolHandle) -> Self {
        Self { machine, state }
    }

    fn resolve_call_target(
        self,
        symbols: &SymbolTable,
        receiver: Option<&[Identifier]>,
        target: &str,
    ) -> SymbolHandle {
        if let Some(receiver) = receiver {
            let path = receiver
                .iter()
                .map(|member| member.as_str())
                .chain(std::iter::once(target))
                .collect::<Vec<_>>();
            return self.resolve_symbol(symbols, path);
        }

        self.resolve_symbol(symbols, [target])
    }

    fn resolve_identifier_members(
        self,
        symbols: &SymbolTable,
        path: &[Identifier],
    ) -> SymbolHandle {
        let Some(first_member) = path.first().map(|member| member.as_str()) else {
            return SymbolHandle::invalid();
        };

        if first_member == "self" && self.machine.is_valid() {
            return symbols
                .find_descendant_by_path(
                    self.machine,
                    path.iter().skip(1).map(|member| member.as_str()),
                )
                .unwrap_or_else(SymbolHandle::invalid);
        }

        for root in [self.state, self.machine, symbols.root()] {
            if !root.is_valid() {
                continue;
            }

            if let Some(symbol) =
                symbols.find_descendant_by_path(root, path.iter().map(|member| member.as_str()))
            {
                return symbol;
            }
        }

        SymbolHandle::invalid()
    }

    fn resolve_symbol<'path>(
        self,
        symbols: &SymbolTable,
        path: impl IntoIterator<Item = &'path str> + Clone,
    ) -> SymbolHandle {
        let mut members = path.clone().into_iter();
        let Some(first_member) = members.next() else {
            return SymbolHandle::invalid();
        };

        if first_member == "self" && self.machine.is_valid() {
            return symbols
                .find_descendant_by_path(self.machine, members)
                .unwrap_or_else(SymbolHandle::invalid);
        }

        for root in [self.state, self.machine, symbols.root()] {
            if !root.is_valid() {
                continue;
            }

            if let Some(symbol) = symbols.find_descendant_by_path(root, path.clone()) {
                return symbol;
            }
        }

        SymbolHandle::invalid()
    }
}

fn resolve_global_name(report: &ResolveReport, name: &str) -> SymbolHandle {
    let root = report.symbols.root();
    report
        .symbols
        .find_descendant_by_path(root, [name])
        .unwrap_or_else(SymbolHandle::invalid)
}

fn build_source_symbol_table(
    syntax_trees: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
) -> SymbolTable {
    SourceSymbolTableBuilder::new(syntax_trees, sources).build()
}

#[derive(Debug)]
struct SourceSymbolTableBuilder<'syntax> {
    syntax_trees: &'syntax SyntaxTrees,
    builder: SymbolTableBuilder,
}

#[derive(Debug)]
enum RootSymbolSeed<'syntax> {
    Identifier(SymbolKind, &'syntax Identifier),
    Name(SymbolKind, String),
}

impl RootSymbolSeed<'_> {
    fn as_symbol_seed(&self) -> SymbolSeed<'_> {
        match self {
            Self::Identifier(kind, identifier) => source_symbol_seed(*kind, identifier),
            Self::Name(kind, name) => (*kind, SymbolNameRef::Borrowed(name.as_str())),
        }
    }
}

fn source_symbol_seed(kind: SymbolKind, identifier: &Identifier) -> SymbolSeed<'_> {
    if has_source_name(identifier.source_span()) {
        (kind, SymbolNameRef::Source(identifier.source_span()))
    } else {
        (kind, SymbolNameRef::Borrowed(identifier.as_str()))
    }
}

fn has_source_name(source_span: SourceSpan) -> bool {
    source_span.span.start != source_span.span.end
}

impl<'syntax> SourceSymbolTableBuilder<'syntax> {
    fn new(syntax_trees: &'syntax SyntaxTrees, sources: Option<Arc<SourceMap>>) -> Self {
        Self {
            syntax_trees,
            builder: SymbolTableBuilder::with_sources(sources),
        }
    }

    fn build(mut self) -> SymbolTable {
        let root = self
            .builder
            .insert_root(SymbolKind::Root, SymbolNameRef::Static("program"));
        let syntax_trees = self.syntax_trees;
        let symbolized_items = syntax_trees
            .root_items()
            .filter_map(|item| root_item_symbol_seed(syntax_trees, item).map(|seed| (item, seed)))
            .collect::<Vec<_>>();
        let root_children = self.builder.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(symbolized_items.iter().map(|(_, seed)| seed.as_symbol_seed())),
        );
        let mut root_children = SymbolTableBuilder::child_handles(root_children);

        for builtin_type in builtin_type_symbols() {
            if let Some(builtin_symbol) = root_children.next() {
                self.insert_builtin_type_children(builtin_symbol, builtin_type);
            }
        }
        for (item, _) in symbolized_items {
            if let Some(item_symbol) = root_children.next() {
                self.insert_item_children(item_symbol, item);
            }
        }

        self.builder.finish()
    }

    fn insert_builtin_type_children(
        &mut self,
        builtin_symbol: SymbolHandle,
        builtin_type: (SymbolKind, SymbolNameRef<'static>),
    ) {
        let SymbolNameRef::Static(name) = builtin_type.1 else {
            return;
        };
        let Some(builtin_type) = BuiltinType::from_name(name) else {
            return;
        };

        self.builder
            .insert_children(builtin_symbol, builtin_type_member_symbols(builtin_type));
    }

    fn insert_item_children(&mut self, parent: SymbolHandle, item: &'syntax Item) {
        match item {
            Item::Capability(capability) => {
                self.insert_capability_children(parent, capability.members, 0);
            }
            Item::Data(data_definition) => {
                self.insert_data_children(parent, data_definition.members, 0);
            }
            Item::Domain(domain) => {
                self.insert_domain_children(parent, domain);
            }
            Item::Library(library) => {
                self.insert_library_children(parent, library.functions);
            }
            Item::Machine(machine) => {
                self.insert_machine_children(parent, machine.states);
            }
            Item::Operator(operator) => {
                self.insert_operator_children(parent, operator);
            }
            Item::Platform(platform) => {
                self.insert_platform_children(parent, platform.states);
            }
            Item::Trait(trait_definition) => {
                self.insert_platform_children(parent, trait_definition.machines);
            }
            Item::Export(_)
            | Item::Invariant(_)
            | Item::Target(_)
            | Item::TrustDefinition(_)
            | Item::Use(_) => {}
        }
    }

    fn insert_capability_children(
        &mut self,
        parent: SymbolHandle,
        members: HandleSpan<omega_syntax_trees::item::CapabilityMember>,
        depth: usize,
    ) {
        let syntax_trees = self.syntax_trees;
        let members = syntax_trees.items.capability_members(members);
        let children = self.builder.insert_children(
            parent,
            members.iter().map(|member| match member {
                CapabilityMember::Field(field) => {
                    source_symbol_seed(SymbolKind::Field, &field.name)
                }
                CapabilityMember::State(state) => {
                    source_symbol_seed(SymbolKind::State, &state.signature.name)
                }
            }),
        );

        for (child, member) in SymbolTableBuilder::child_handles(children).zip(members.iter()) {
            match member {
                CapabilityMember::Field(field) => {
                    self.insert_type_children(child, field.type_reference, depth);
                }
                CapabilityMember::State(state) => {
                    self.insert_inline_state_signature_children(child, &state.signature);
                }
            }
        }
    }

    fn insert_data_children(
        &mut self,
        parent: SymbolHandle,
        members: HandleSpan<omega_syntax_trees::item::DataMember>,
        member_depth: usize,
    ) {
        let syntax_trees = self.syntax_trees;
        let members = syntax_trees.items.data_members(members);
        let children = self.builder.insert_children(
            parent,
            members.iter().map(|member| match member {
                DataMember::Field(field) => source_symbol_seed(SymbolKind::Field, &field.name),
                DataMember::Variant(variant) => {
                    source_symbol_seed(SymbolKind::Variant, &variant.name)
                }
            }),
        );

        for (child, member) in SymbolTableBuilder::child_handles(children).zip(members.iter()) {
            if let DataMember::Field(field) = member {
                self.insert_type_children(child, field.type_reference, member_depth + 1);
            }
        }
    }

    fn insert_domain_children(
        &mut self,
        parent: SymbolHandle,
        domain: &'syntax omega_syntax_trees::item::DomainDefinition,
    ) {
        let syntax_trees = self.syntax_trees;
        let operators = syntax_trees.items.operators(domain.operators);
        let operator_names = operators
            .iter()
            .map(|operator| operator_name(syntax_trees, operator.name))
            .collect::<Vec<_>>();
        let children = self.builder.insert_children(
            parent,
            operator_names
                .iter()
                .map(|name| (SymbolKind::Operator, SymbolNameRef::Borrowed(name.as_str()))),
        );

        for (child, operator) in SymbolTableBuilder::child_handles(children).zip(operators.iter()) {
            self.insert_operator_children(child, operator);
        }
    }

    fn insert_library_children(
        &mut self,
        parent: SymbolHandle,
        functions: HandleSpan<omega_syntax_trees::item::LibraryFunction>,
    ) {
        let syntax_trees = self.syntax_trees;
        let functions = syntax_trees.items.library_functions(functions);
        let children = self.builder.insert_children(
            parent,
            functions
                .iter()
                .map(|function| source_symbol_seed(SymbolKind::Function, &function.signature.name)),
        );

        for (child, function) in SymbolTableBuilder::child_handles(children).zip(functions.iter()) {
            self.insert_inline_state_signature_children(child, &function.signature);
        }
    }

    fn insert_machine_children(
        &mut self,
        parent: SymbolHandle,
        states: HandleSpan<omega_syntax_trees::item::StateHandle>,
    ) {
        let syntax_trees = self.syntax_trees;
        let state_handles = syntax_trees.items.state_handles(states);
        let children = self.builder.insert_children(
            parent,
            state_handles.iter().map(|state| {
                let state = syntax_trees.items.state(*state);
                source_symbol_seed(SymbolKind::State, &state.name)
            }),
        );

        for (child, state) in SymbolTableBuilder::child_handles(children).zip(state_handles.iter())
        {
            self.insert_state_children(child, *state);
        }
    }

    fn insert_platform_children(
        &mut self,
        parent: SymbolHandle,
        signatures: HandleSpan<omega_syntax_trees::item::StateSignatureHandle>,
    ) {
        let syntax_trees = self.syntax_trees;
        let signatures = syntax_trees.items.state_signatures(signatures);
        let children = self.builder.insert_children(
            parent,
            signatures.iter().map(|signature| {
                let signature = syntax_trees.items.state_signature(*signature);
                source_symbol_seed(SymbolKind::State, &signature.name)
            }),
        );

        for (child, signature) in SymbolTableBuilder::child_handles(children).zip(signatures.iter())
        {
            self.insert_state_signature_children(child, *signature);
        }
    }

    fn insert_operator_children(
        &mut self,
        parent: SymbolHandle,
        operator: &'syntax omega_syntax_trees::item::OperatorDefinition,
    ) {
        let syntax_trees = self.syntax_trees;
        let type_parameters = syntax_trees.items.type_parameters(operator.type_parameters);
        let parameters = syntax_trees.items.state_parameters(operator.parameters);
        let children = self.builder.insert_children(
            parent,
            type_parameters
                .iter()
                .map(|parameter| source_symbol_seed(SymbolKind::TypeParameter, &parameter.name))
                .chain(parameters.iter().map(|parameter| {
                    let parameter = syntax_trees.items.state_parameter(*parameter);
                    source_symbol_seed(SymbolKind::Parameter, &parameter.name)
                })),
        );
        let mut children = SymbolTableBuilder::child_handles(children);

        for _ in type_parameters {
            let Some(_) = children.next() else {
                return;
            };
        }

        for parameter in parameters {
            let Some(parameter_symbol) = children.next() else {
                return;
            };
            let parameter = syntax_trees.items.state_parameter(*parameter);
            self.insert_type_children(parameter_symbol, parameter.type_reference, 0);
        }
    }

    fn insert_state_children(
        &mut self,
        parent: SymbolHandle,
        state_handle: omega_syntax_trees::item::StateHandle,
    ) {
        let state = self.syntax_trees.items.state(state_handle);
        let syntax_trees = self.syntax_trees;
        let parameters = syntax_trees.items.state_parameters(state.parameters);
        let statements = syntax_trees.items.statements(state.statements);
        let children = self.builder.insert_children(
            parent,
            parameters
                .iter()
                .map(|parameter| {
                    let parameter = syntax_trees.items.state_parameter(*parameter);
                    source_symbol_seed(SymbolKind::Parameter, &parameter.name)
                })
                .chain(statements.iter().filter_map(|statement| {
                    let StatementNode::LocalData(local_data) =
                        syntax_trees.statements.statement(*statement)
                    else {
                        return None;
                    };
                    Some(source_symbol_seed(SymbolKind::Local, &local_data.name))
                })),
        );
        let mut children = SymbolTableBuilder::child_handles(children);

        for parameter in parameters {
            let Some(parameter_symbol) = children.next() else {
                return;
            };
            let parameter = syntax_trees.items.state_parameter(*parameter);
            self.insert_type_children(parameter_symbol, parameter.type_reference, 0);
        }
        for statement in statements {
            let StatementNode::LocalData(local_data) =
                syntax_trees.statements.statement(*statement)
            else {
                continue;
            };
            let Some(local_symbol) = children.next() else {
                return;
            };
            self.insert_type_children(local_symbol, local_data.type_reference, 0);
        }
    }

    fn insert_state_signature_children(
        &mut self,
        parent: SymbolHandle,
        signature_handle: omega_syntax_trees::item::StateSignatureHandle,
    ) {
        let signature = self.syntax_trees.items.state_signature(signature_handle);
        self.insert_state_signature_parts(parent, signature.parameters);
    }

    fn insert_inline_state_signature_children(
        &mut self,
        parent: SymbolHandle,
        signature: &'syntax omega_syntax_trees::item::StateSignature,
    ) {
        self.insert_state_signature_parts(parent, signature.parameters);
    }

    fn insert_state_signature_parts(
        &mut self,
        parent: SymbolHandle,
        parameters: HandleSpan<omega_syntax_trees::item::StateParameterHandle>,
    ) {
        let syntax_trees = self.syntax_trees;
        let parameters = syntax_trees.items.state_parameters(parameters);
        let children = self.builder.insert_children(
            parent,
            parameters.iter().map(|parameter| {
                let parameter = syntax_trees.items.state_parameter(*parameter);
                source_symbol_seed(SymbolKind::Parameter, &parameter.name)
            }),
        );

        for (child, parameter) in SymbolTableBuilder::child_handles(children).zip(parameters.iter())
        {
            let parameter = syntax_trees.items.state_parameter(*parameter);
            self.insert_type_children(child, parameter.type_reference, 0);
        }
    }

    fn insert_type_children(
        &mut self,
        parent: SymbolHandle,
        type_reference: TypeReferenceHandle,
        depth: usize,
    ) {
        if !type_reference.is_valid() || depth > 8 {
            return;
        }

        match self
            .syntax_trees
            .type_references
            .type_reference(type_reference)
        {
            TypeReferenceNode::Reference { referee, .. } => {
                self.insert_type_children(parent, *referee, depth);
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.insert_type_children(parent, *base_type, depth);
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                self.insert_type_children(parent, *element_type, depth + 1);
            }
            TypeReferenceNode::Slice { element_type } => {
                self.insert_type_children(parent, *element_type, depth + 1);
            }
            TypeReferenceNode::Generic { base_name, .. } | TypeReferenceNode::Named(base_name) => {
                self.insert_named_type_children(parent, base_name.as_str(), depth + 1);
            }
            TypeReferenceNode::SelfType | TypeReferenceNode::Unit => {}
        }
    }

    fn insert_named_type_children(&mut self, parent: SymbolHandle, type_name: &str, depth: usize) {
        if depth > 8 {
            return;
        }

        let Some(item) = self
            .syntax_trees
            .root_items()
            .find(|item| top_level_item_name(item) == Some(type_name))
        else {
            return;
        };

        match item {
            Item::Capability(capability) => {
                self.insert_capability_children(parent, capability.members, depth + 1);
            }
            Item::Data(data_definition) => {
                self.insert_data_children(parent, data_definition.members, depth + 1);
            }
            Item::Domain(domain) => {
                self.insert_domain_children(parent, domain);
            }
            Item::Library(library) => {
                self.insert_library_children(parent, library.functions);
            }
            Item::Machine(machine) => {
                self.insert_machine_children(parent, machine.states);
            }
            Item::Operator(operator) => {
                self.insert_operator_children(parent, operator);
            }
            Item::Platform(platform) => {
                self.insert_platform_children(parent, platform.states);
            }
            Item::Trait(trait_definition) => {
                self.insert_platform_children(parent, trait_definition.machines);
            }
            Item::Export(_)
            | Item::Invariant(_)
            | Item::Target(_)
            | Item::TrustDefinition(_)
            | Item::Use(_) => {}
        }
    }
}

fn root_item_symbol_seed<'syntax>(
    syntax_trees: &'syntax SyntaxTrees,
    item: &'syntax Item,
) -> Option<RootSymbolSeed<'syntax>> {
    match item {
        Item::Capability(capability) => Some(RootSymbolSeed::Identifier(
            SymbolKind::HostCapability,
            &capability.name,
        )),
        Item::Data(data_definition) => {
            Some(RootSymbolSeed::Identifier(SymbolKind::Data, &data_definition.name))
        }
        Item::Domain(domain) => Some(RootSymbolSeed::Identifier(SymbolKind::Domain, &domain.name)),
        Item::Invariant(invariant) => {
            Some(RootSymbolSeed::Identifier(SymbolKind::Invariant, &invariant.name))
        }
        Item::Library(library) => library
            .name
            .as_ref()
            .map(|name| RootSymbolSeed::Identifier(SymbolKind::Import, name)),
        Item::Machine(machine) => {
            Some(RootSymbolSeed::Identifier(SymbolKind::Machine, &machine.name))
        }
        Item::Operator(operator) => Some(RootSymbolSeed::Name(
            SymbolKind::Operator,
            operator_name(syntax_trees, operator.name),
        )),
        Item::Platform(platform) => Some(RootSymbolSeed::Identifier(
            SymbolKind::Platform,
            &platform.name,
        )),
        Item::Trait(trait_definition) => Some(RootSymbolSeed::Identifier(
            SymbolKind::Trait,
            &trait_definition.name,
        )),
        Item::Target(target) => Some(RootSymbolSeed::Identifier(SymbolKind::Object, &target.name)),
        Item::TrustDefinition(trust_definition) => Some(RootSymbolSeed::Identifier(
            SymbolKind::Trust,
            &trust_definition.name,
        )),
        Item::Export(_) | Item::Use(_) => None,
    }
}

fn top_level_item_name(item: &Item) -> Option<&str> {
    match item {
        Item::Capability(capability) => Some(capability.name.as_str()),
        Item::Data(data_definition) => Some(data_definition.name.as_str()),
        Item::Domain(domain) => Some(domain.name.as_str()),
        Item::Invariant(invariant) => Some(invariant.name.as_str()),
        Item::Library(library) => library.name.as_ref().map(|name| name.as_str()),
        Item::Machine(machine) => Some(machine.name.as_str()),
        Item::Platform(platform) => Some(platform.name.as_str()),
        Item::Trait(trait_definition) => Some(trait_definition.name.as_str()),
        Item::Target(target) => Some(target.name.as_str()),
        Item::TrustDefinition(trust_definition) => Some(trust_definition.name.as_str()),
        Item::Export(_) | Item::Operator(_) | Item::Use(_) => None,
    }
}

fn operator_name(syntax_trees: &SyntaxTrees, name: HandleSpan<Identifier>) -> String {
    syntax_trees
        .items
        .identifier_path_members(name)
        .iter()
        .map(Identifier::as_str)
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use super::{
        ResolvedDefinitionKind, ResolvedReferenceKind, build_resolve_report_without_sources,
    };
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolKind;
    use omega_syntax_trees::SyntaxTrees;
    use omega_syntax_trees::expression::{ExpressionNode, TableCallExpression};
    use omega_syntax_trees::identifier::Identifier;
    use omega_syntax_trees::item::{
        DataDefinition, DataField, DataMember, DomainDefinition, Item, Machine,
        OperatorDefinition, State, StateParameterNode, TypeParameter, UseItem,
    };
    use omega_syntax_trees::statement::{
        StatementNode, TableAssignment, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };
    use omega_syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[test]
    fn collects_root_operator_definitions_as_symbols() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());

        let name = syntax_trees.items.insert_identifier_path_members([
            Identifier::generated("Slice"),
            Identifier::generated("index"),
        ]);
        let type_parameter = syntax_trees.items.append_type_parameter(TypeParameter {
            name: Identifier::generated("T"),
        });
        let generic_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("T")));
        let slice_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Slice {
                element_type: generic_type,
            });
        let borrowed_slice_type = syntax_trees.type_references.insert(TypeReferenceNode::Reference {
            referee: slice_type,
            is_mutable: false,
        });
        let index_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("usize")));
        let items_parameter =
            syntax_trees
                .items
                .insert_state_parameter_node(StateParameterNode {
                    name: Identifier::generated("items"),
                    type_reference: borrowed_slice_type,
                    is_const: false,
                    is_mutable: false,
                    is_self: false,
                });
        let items_parameter = syntax_trees.items.append_state_parameter_handle(items_parameter);
        let index_parameter =
            syntax_trees
                .items
                .insert_state_parameter_node(StateParameterNode {
                    name: Identifier::generated("index"),
                    type_reference: index_type,
                    is_const: false,
                    is_mutable: false,
                    is_self: false,
                });
        let _index_parameter = syntax_trees.items.append_state_parameter_handle(index_parameter);
        syntax_trees.push_root_item(Item::Operator(OperatorDefinition {
            name,
            type_parameters: HandleSpan::from_parts(type_parameter, 1),
            parameters: HandleSpan::from_parts(items_parameter, 2),
            return_type: generic_type,
            contracts: HandleSpan::empty(),
            token_count: 1,
        }));

        let report = build_resolve_report_without_sources(&syntax_trees);

        let (_, definition) = report
            .definitions
            .iter()
            .find(|(_, definition)| report.symbols.name(definition.symbol) == "Slice::index")
            .expect("root operator definition should be collected");
        assert_eq!(definition.kind, ResolvedDefinitionKind::Operator);
        assert_eq!(
            report.symbols.get(definition.symbol).kind,
            SymbolKind::Operator
        );
        assert!(report.references.iter().any(|(_, reference)| {
            report.reference_name(reference) == "usize"
                && reference.kind == ResolvedReferenceKind::Type
        }));

        let type_parameter = report
            .symbols
            .find_child_by_name(definition.symbol, "T")
            .expect("operator type parameter should get a symbol");
        assert_eq!(report.symbols.get(type_parameter).kind, SymbolKind::TypeParameter);

        let value_parameter = report
            .symbols
            .find_child_by_name(definition.symbol, "items")
            .expect("operator value parameter should get a symbol");
        assert_eq!(report.symbols.get(value_parameter).kind, SymbolKind::Parameter);
    }

    #[test]
    fn collects_trust_definitions_as_trust_symbols() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        syntax_trees.push_root_item(Item::TrustDefinition(
            omega_syntax_trees::item::TrustDefinition {
                name: Identifier::generated("compiler_slice_index"),
                token_count: 1,
            },
        ));

        let report = build_resolve_report_without_sources(&syntax_trees);

        let (_, definition) = report
            .definitions
            .iter()
            .find(|(_, definition)| report.symbols.name(definition.symbol) == "compiler_slice_index")
            .expect("trust definition should be collected");
        assert_eq!(definition.kind, ResolvedDefinitionKind::Trust);
        assert_eq!(report.symbols.get(definition.symbol).kind, SymbolKind::Trust);
    }

    #[test]
    fn collects_domain_definitions_and_target_type_references() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let target_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("String")));
        let operator_name = syntax_trees
            .items
            .insert_identifier_path_members([Identifier::generated("normalize")]);
        let parameter = syntax_trees
            .items
            .insert_state_parameter_node(StateParameterNode {
                name: Identifier::generated("value"),
                type_reference: target_type,
                is_const: false,
                is_mutable: false,
                is_self: false,
            });
        let parameter = syntax_trees.items.append_state_parameter_handle(parameter);
        let operator = syntax_trees.items.append_operator(OperatorDefinition {
            name: operator_name,
            type_parameters: HandleSpan::empty(),
            parameters: HandleSpan::from_parts(parameter, 1),
            return_type: target_type,
            contracts: HandleSpan::empty(),
            token_count: 1,
        });

        syntax_trees.push_root_item(Item::Domain(DomainDefinition {
            name: Identifier::generated("NonEmpty"),
            target_type,
            facts: omega_core::arena::HandleSpan::empty(),
            operators: HandleSpan::from_parts(operator, 1),
            body_token_count: 3,
        }));

        let report = build_resolve_report_without_sources(&syntax_trees);

        let (_, definition) = report
            .definitions
            .iter()
            .find(|(_, definition)| report.symbols.name(definition.symbol) == "NonEmpty")
            .expect("domain definition should be collected");
        assert_eq!(definition.kind, ResolvedDefinitionKind::Domain);
        let domain_symbol = definition.symbol;
        let operator_symbol = report
            .symbols
            .find_child_by_name(domain_symbol, "normalize")
            .expect("domain operator should get a child symbol");
        assert_eq!(report.symbols.get(operator_symbol).kind, SymbolKind::Operator);
        assert!(
            report.definitions.iter().any(|(_, definition)| {
                definition.symbol == operator_symbol
                    && definition.kind == ResolvedDefinitionKind::Operator
            }),
            "domain operator definition should be collected"
        );
        assert!(
            report.references.iter().any(|(_, reference)| {
                report.reference_name(reference) == "String"
                    && reference.kind == ResolvedReferenceKind::Type
            }),
            "domain target type should be collected as a type reference"
        );
    }

    #[test]
    fn collects_definitions_imports_and_references() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());

        let import_path = syntax_trees.items.insert_identifier_path_members([
            Identifier::generated("platform"),
            Identifier::generated("console"),
        ]);
        syntax_trees.push_root_item(Item::Use(UseItem { path: import_path }));

        let parameter_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("i32")));
        let parameter = syntax_trees
            .items
            .insert_state_parameter_node(StateParameterNode {
                name: Identifier::generated("amount"),
                type_reference: parameter_type,
                is_const: false,
                is_mutable: false,
                is_self: false,
            });
        let parameter = syntax_trees.items.append_state_parameter_handle(parameter);

        let target_path_start = syntax_trees
            .statements
            .append_identifier_path_member(Identifier::generated("finish"));
        let target_path = HandleSpan::from_parts(target_path_start, 1);
        let target =
            syntax_trees
                .statements
                .insert_transition_target(TransitionTargetNode::Named {
                    path: target_path,
                    path_starts_at_self: false,
                    arguments: HandleSpan::empty(),
                });
        let transition =
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: omega_syntax_trees::statement::TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::Always,
                }));
        let transition = syntax_trees.items.append_statement_handle(transition);

        let entry_state = syntax_trees
            .items
            .insert_state(&omega_syntax_trees::item::State {
                name: Identifier::generated("entry"),
                parameters: HandleSpan::from_parts(parameter, 1),
                return_type: TypeReferenceHandle::invalid(),
                statements: HandleSpan::from_parts(transition, 1),
            });
        let entry_state = syntax_trees.items.append_state_handle(entry_state);

        let finish_state = syntax_trees
            .items
            .insert_state(&omega_syntax_trees::item::State {
                name: Identifier::generated("finish"),
                parameters: HandleSpan::empty(),
                return_type: TypeReferenceHandle::invalid(),
                statements: HandleSpan::empty(),
            });
        let _finish_state = syntax_trees.items.append_state_handle(finish_state);

        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("main"),
            attached_data: None,
            satisfies: HandleSpan::empty(),
            terminates: false,
            decreases: HandleSpan::empty(),
            decrease_order: HandleSpan::empty(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
            states: HandleSpan::from_parts(entry_state, 2),
        }));

        let report = build_resolve_report_without_sources(&syntax_trees);

        assert_eq!(report.imports.len(), 1);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.references.len(), 2);

        let (_, definition) = report
            .definitions
            .iter()
            .find(|(_, definition)| report.symbols.name(definition.symbol) == "main")
            .expect("main definition should be collected");
        assert_eq!(definition.kind, ResolvedDefinitionKind::Machine);
        assert!(definition.symbol.is_valid());

        assert!(
            report.references.iter().any(|(_, reference)| {
                report.reference_name(reference) == "finish"
                    && reference.kind == ResolvedReferenceKind::TransitionTarget
                    && reference.symbol.is_valid()
            }),
            "state transition target should be collected and bound to a symbol"
        );
    }

    #[test]
    fn resolves_builtin_type_member_call_targets() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let real_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("Real")));
        let field = syntax_trees
            .items
            .append_data_member(DataMember::Field(DataField {
                name: Identifier::generated("value"),
                type_reference: real_type,
                initial_value: omega_syntax_trees::expression::ExpressionHandle::invalid(),
            }));
        syntax_trees.push_root_item(Item::Data(DataDefinition {
            name: Identifier::generated("main"),
            type_parameters: HandleSpan::empty(),
            members: HandleSpan::from_parts(field, 1),
        }));

        let receiver_member = syntax_trees
            .expressions
            .append_identifier_path_member(Identifier::generated("Real"));
        let receiver =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Name(HandleSpan::from_parts(
                    receiver_member,
                    1,
                )));
        let argument = syntax_trees.expressions.insert(ExpressionNode::Integer(1));
        let arguments = syntax_trees
            .expressions
            .insert_expression_handles([argument]);
        let call = syntax_trees
            .expressions
            .insert(ExpressionNode::Call(TableCallExpression {
                receiver,
                target: Identifier::generated("from"),
                arguments,
            }));
        let target_path_start = syntax_trees
            .expressions
            .append_identifier_path_member(Identifier::generated("self"));
        let target_path_member = syntax_trees
            .expressions
            .append_identifier_path_member(Identifier::generated("value"));
        let target = syntax_trees
            .expressions
            .insert(ExpressionNode::Name(HandleSpan::from_parts(
                target_path_start,
                target_path_member
                    .arena_index()
                    .checked_sub(target_path_start.arena_index())
                    .expect("target path should be contiguous")
                    + 1,
            )));
        let assignment =
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(TableAssignment {
                    target,
                    value: call,
                }));
        let assignment = syntax_trees.items.append_statement_handle(assignment);
        let entry_state = syntax_trees.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::empty(),
            return_type: TypeReferenceHandle::invalid(),
            statements: HandleSpan::from_parts(assignment, 1),
        });
        let entry_state = syntax_trees.items.append_state_handle(entry_state);
        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("main"),
            attached_data: None,
            satisfies: HandleSpan::empty(),
            terminates: false,
            decreases: HandleSpan::empty(),
            decrease_order: HandleSpan::empty(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
            states: HandleSpan::from_parts(entry_state, 1),
        }));

        let report = build_resolve_report_without_sources(&syntax_trees);

        assert!(
            report.references.iter().any(|(_, reference)| {
                report.reference_name(reference) == "Real::from"
                    && reference.kind == ResolvedReferenceKind::CallTarget
                    && reference.symbol.is_valid()
            }),
            "builtin type member call target should resolve to a symbol"
        );
    }
}
