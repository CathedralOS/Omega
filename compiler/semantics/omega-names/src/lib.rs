//! Name and module resolution.
//!
//! This crate owns the first post-parse view of names. The current report is
//! intentionally shallow: it records imports, top-level definitions, and
//! syntactic references without pretending that every reference is fully bound.
//! That gives later phases a concrete spine to grow from.

use omega_abstract_syntax_tree::expression::Expression;
use omega_abstract_syntax_tree::item::{
    CapabilityMember, DataMember, Item, Machine, State, StateSignature,
};
use omega_abstract_syntax_tree::statement::{Statement, TransitionGuard, TransitionTarget};
use omega_abstract_syntax_tree::types::{TypeConstraint, TypeReference};
use omega_core::arena::Arena;
use omega_core::symbols::{SymbolDefinition, SymbolHandle, SymbolKind, SymbolPath, SymbolTable};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolveReport {
    pub definitions: Arena<ResolvedDefinition>,
    pub imports: Arena<ResolvedImport>,
    pub references: Arena<ResolvedReference>,
    pub symbols: SymbolTable,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedDefinition {
    pub name: String,
    pub kind: ResolvedDefinitionKind,
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolvedDefinitionKind {
    Capability,
    Data,
    Invariant,
    Machine,
    Platform,
    Target,
    Trust,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedImport {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedReference {
    pub name: String,
    pub kind: ResolvedReferenceKind,
    pub owner: String,
    pub symbol_path: SymbolPath,
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

pub fn build_resolve_report(items: &[Item]) -> ResolveReport {
    let mut report = ResolveReport::default();
    report.symbols = build_source_symbol_table(items);

    for item in items {
        match item {
            Item::Capability(capability) => {
                insert_definition(
                    &mut report,
                    capability.name.as_str(),
                    ResolvedDefinitionKind::Capability,
                );

                for member in &capability.members {
                    match member {
                        CapabilityMember::Field(field) => {
                            collect_type_reference(
                                &mut report,
                                &field.type_reference,
                                &format!("capability `{}` field `{}`", capability.name, field.name),
                            );
                        }
                        CapabilityMember::State(state) => {
                            collect_state_signature_references(
                                &mut report,
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

                for member in &data_definition.members {
                    if let DataMember::Field(field) = member {
                        collect_type_reference(
                            &mut report,
                            &field.type_reference,
                            &format!("data `{}` field `{}`", data_definition.name, field.name),
                        );
                    }
                }
            }
            Item::Invariant(invariant) => {
                insert_definition(
                    &mut report,
                    invariant.name.as_str(),
                    ResolvedDefinitionKind::Invariant,
                );

                collect_constraints(
                    &mut report,
                    &invariant.constraints,
                    &format!("invariant `{}`", invariant.name),
                );
            }
            Item::Use(use_item) => {
                report.imports.insert(ResolvedImport {
                    path: use_item.path.join("::"),
                });
            }
            Item::Machine(machine) => {
                insert_definition(
                    &mut report,
                    machine.name.as_str(),
                    ResolvedDefinitionKind::Machine,
                );

                collect_machine_references(&mut report, machine);
            }
            Item::Platform(platform) => {
                insert_definition(
                    &mut report,
                    platform.name.as_str(),
                    ResolvedDefinitionKind::Platform,
                );

                for state in &platform.states {
                    collect_state_signature_references(
                        &mut report,
                        state,
                        &format!("platform `{}` state `{}`", platform.name, state.name),
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
        }
    }

    report
}

fn insert_definition(report: &mut ResolveReport, name: &str, kind: ResolvedDefinitionKind) {
    let symbol = report
        .symbols
        .find_child_by_name(report.symbols.root(), name)
        .unwrap_or_else(SymbolHandle::invalid);
    report.definitions.insert(ResolvedDefinition {
        name: name.to_owned(),
        kind,
        symbol,
    });
}

fn collect_machine_references(report: &mut ResolveReport, machine: &Machine) {
    let machine_symbol = report
        .symbols
        .find_child_by_name(report.symbols.root(), machine.name.as_str())
        .unwrap_or_else(SymbolHandle::invalid);

    for contained_object in &machine.contains {
        let symbol_path = resolve_global_name(report, contained_object.type_name.as_str());
        insert_reference(
            report,
            &contained_object.type_name,
            ResolvedReferenceKind::Type,
            &format!(
                "machine `{}` contains `{}`",
                machine.name, contained_object.name
            ),
            symbol_path,
        );
    }

    for owned_data in &machine.owned_data {
        collect_type_reference(
            report,
            &owned_data.type_reference,
            &format!("machine `{}` owns `{}`", machine.name, owned_data.name),
        );

        if let Some(initial_value) = &owned_data.initial_value {
            collect_expression(
                report,
                initial_value,
                &format!(
                    "machine `{}` owned `{}` initializer",
                    machine.name, owned_data.name
                ),
                ResolveContext::from_symbols(machine_symbol, SymbolHandle::invalid()),
            );
        }
    }

    for state in &machine.states {
        collect_state_references(report, machine, machine_symbol, state);
    }
}

fn collect_state_references(
    report: &mut ResolveReport,
    machine: &Machine,
    machine_symbol: SymbolHandle,
    state: &State,
) {
    let state_symbol = report
        .symbols
        .find_child_by_name(machine_symbol, state.name.as_str())
        .unwrap_or_else(SymbolHandle::invalid);
    let context = ResolveContext::from_symbols(machine_symbol, state_symbol);

    collect_state_signature_parts(
        report,
        &state.parameters,
        state.return_type.as_ref(),
        &format!("machine `{}` state `{}`", machine.name, state.name),
    );

    for statement in &state.statements {
        collect_statement(
            report,
            statement,
            &format!("machine `{}` state `{}`", machine.name, state.name),
            context,
        );
    }
}

fn collect_state_signature_references(
    report: &mut ResolveReport,
    state: &StateSignature,
    owner: &str,
) {
    collect_state_signature_parts(report, &state.parameters, state.return_type.as_ref(), owner);
}

fn collect_state_signature_parts(
    report: &mut ResolveReport,
    parameters: &[omega_abstract_syntax_tree::item::StateParameter],
    return_type: Option<&TypeReference>,
    owner: &str,
) {
    for parameter in parameters {
        collect_type_reference(
            report,
            &parameter.type_reference,
            &format!("{owner} parameter `{}`", parameter.name),
        );
    }

    if let Some(return_type) = return_type {
        collect_type_reference(report, return_type, &format!("{owner} return type"));
    }
}

fn collect_statement(
    report: &mut ResolveReport,
    statement: &Statement,
    owner: &str,
    context: ResolveContext,
) {
    match statement {
        Statement::Assignment(assignment) => {
            collect_expression(
                report,
                &assignment.target,
                &format!("{owner} assignment target"),
                context,
            );
            collect_expression(
                report,
                &assignment.value,
                &format!("{owner} assignment value"),
                context,
            );
        }
        Statement::Call(call) => {
            let target = call
                .receiver
                .as_ref()
                .map(|receiver| format!("{receiver}::{}", call.target))
                .unwrap_or_else(|| call.target.to_string());

            let path = call
                .receiver
                .as_ref()
                .map(|receiver| vec![receiver.as_str(), call.target.as_str()])
                .unwrap_or_else(|| vec![call.target.as_str()]);
            let symbol_path = context.resolve_path(&mut report.symbols, &path);

            insert_reference(
                report,
                &target,
                ResolvedReferenceKind::CallTarget,
                owner,
                symbol_path,
            );

            for argument in &call.arguments {
                collect_expression(report, argument, &format!("{owner} call argument"), context);
            }
        }
        Statement::Expression(expression) => collect_expression(report, expression, owner, context),
        Statement::LocalData(local_data) => collect_type_reference(
            report,
            &local_data.type_reference,
            &format!("{owner} local `{}`", local_data.name),
        ),
        Statement::Transition(transition) => {
            collect_transition_target(report, &transition.target, owner, context);

            if let Some(continuation) = &transition.continuation {
                collect_transition_target(report, continuation, owner, context);
            }

            if let TransitionGuard::When(guard) = &transition.guard {
                collect_expression(report, guard, &format!("{owner} transition guard"), context);
            }
        }
    }
}

fn collect_transition_target(
    report: &mut ResolveReport,
    target: &TransitionTarget,
    owner: &str,
    context: ResolveContext,
) {
    if let TransitionTarget::Named { path, arguments } = target {
        let path_members = path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>();
        let symbol_path = context.resolve_path(&mut report.symbols, &path_members);
        insert_reference(
            report,
            &path.join("::"),
            ResolvedReferenceKind::TransitionTarget,
            owner,
            symbol_path,
        );

        for argument in arguments {
            collect_expression(
                report,
                argument,
                &format!("{owner} transition argument"),
                context,
            );
        }
    }
}

fn collect_type_reference(report: &mut ResolveReport, type_reference: &TypeReference, owner: &str) {
    match type_reference {
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_reference(report, base_type, owner);
            collect_constraints(report, constraints, owner);
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_type_reference(report, element_type, owner);
        }
        TypeReference::Generic {
            base_name,
            arguments,
        } => {
            let symbol_path = resolve_global_name(report, base_name.as_str());
            insert_reference(
                report,
                base_name,
                ResolvedReferenceKind::Type,
                owner,
                symbol_path,
            );

            for argument in arguments {
                collect_type_reference(report, argument, owner);
            }
        }
        TypeReference::Named(name) => {
            let symbol_path = resolve_global_name(report, name.as_str());
            insert_reference(
                report,
                name,
                ResolvedReferenceKind::Type,
                owner,
                symbol_path,
            );
        }
        TypeReference::Unit => {}
    }
}

fn collect_constraints(report: &mut ResolveReport, constraints: &[TypeConstraint], owner: &str) {
    for constraint in constraints {
        match constraint {
            TypeConstraint::Named(name) => {
                let symbol_path = resolve_global_name(report, name.as_str());
                insert_reference(
                    report,
                    name,
                    ResolvedReferenceKind::Invariant,
                    owner,
                    symbol_path,
                );
            }
            TypeConstraint::Range { minimum, maximum } => {
                collect_expression(
                    report,
                    minimum,
                    &format!("{owner} range minimum"),
                    ResolveContext::default(),
                );
                collect_expression(
                    report,
                    maximum,
                    &format!("{owner} range maximum"),
                    ResolveContext::default(),
                );
            }
        }
    }
}

fn collect_expression(
    report: &mut ResolveReport,
    expression: &Expression,
    owner: &str,
    context: ResolveContext,
) {
    match expression {
        Expression::ArrayLiteral(values) => {
            for value in values {
                collect_expression(report, value, owner, context);
            }
        }
        Expression::Binary(binary) => {
            collect_expression(report, &binary.left, owner, context);
            collect_expression(report, &binary.right, owner, context);
        }
        Expression::Indexed(indexed) => {
            collect_expression(report, &indexed.collection, owner, context);
            collect_expression(report, &indexed.index, owner, context);
        }
        Expression::Mutable(inner_expression) => {
            collect_expression(report, inner_expression, owner, context)
        }
        Expression::Name(path) => {
            let path_members = path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>();
            let symbol_path = context.resolve_path(&mut report.symbols, &path_members);
            insert_reference(
                report,
                &path.join("::"),
                ResolvedReferenceKind::ExpressionName,
                owner,
                symbol_path,
            );
        }
        Expression::StructLiteral(struct_literal) => {
            let symbol_path = resolve_global_name(report, struct_literal.type_name.as_str());
            insert_reference(
                report,
                &struct_literal.type_name,
                ResolvedReferenceKind::StructLiteral,
                owner,
                symbol_path,
            );

            for field in &struct_literal.fields {
                collect_expression(report, &field.value, owner, context);
            }
        }
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => {}
    }
}

fn insert_reference(
    report: &mut ResolveReport,
    name: &str,
    kind: ResolvedReferenceKind,
    owner: &str,
    symbol_path: SymbolPath,
) {
    report.references.insert(ResolvedReference {
        name: name.to_owned(),
        kind,
        owner: owner.to_owned(),
        symbol_path,
    });
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

    fn resolve_path(self, symbols: &mut SymbolTable, path: &[&str]) -> SymbolPath {
        if path.is_empty() {
            return SymbolPath::default();
        }

        if path.first() == Some(&"self") && self.machine.is_valid() {
            return symbols.resolve_child_path(self.machine, path[1..].iter().copied());
        }

        for root in [self.state, self.machine, symbols.root()] {
            if !root.is_valid() {
                continue;
            }

            let symbol_path = symbols.resolve_child_path(root, path.iter().copied());
            if !symbols.path_members(symbol_path).is_empty() {
                return symbol_path;
            }
        }

        SymbolPath::default()
    }
}

fn resolve_global_name(report: &mut ResolveReport, name: &str) -> SymbolPath {
    let root = report.symbols.root();
    report.symbols.resolve_child_path(root, [name])
}

fn build_source_symbol_table(items: &[Item]) -> SymbolTable {
    SymbolTable::from_definition(SymbolDefinition::with_children(
        SymbolKind::Root,
        "program",
        items.iter().filter_map(item_symbol_definition),
    ))
}

fn item_symbol_definition(item: &Item) -> Option<SymbolDefinition<'_>> {
    match item {
        Item::Capability(capability) => Some(SymbolDefinition::with_children(
            SymbolKind::HostCapability,
            capability.name.as_str(),
            capability.members.iter().map(|member| match member {
                CapabilityMember::Field(field) => {
                    SymbolDefinition::named(SymbolKind::Field, field.name.as_str())
                }
                CapabilityMember::State(state) => {
                    state_signature_symbol_definition(&state.signature)
                }
            }),
        )),
        Item::Data(data_definition) => Some(SymbolDefinition::with_children(
            SymbolKind::Data,
            data_definition.name.as_str(),
            data_definition.members.iter().map(|member| match member {
                DataMember::Field(field) => {
                    SymbolDefinition::named(SymbolKind::Field, field.name.as_str())
                }
                DataMember::Variant(variant) => {
                    SymbolDefinition::named(SymbolKind::Variant, variant.name.as_str())
                }
            }),
        )),
        Item::Invariant(invariant) => Some(SymbolDefinition::named(
            SymbolKind::Invariant,
            invariant.name.as_str(),
        )),
        Item::Machine(machine) => Some(SymbolDefinition::with_children(
            SymbolKind::Machine,
            machine.name.as_str(),
            machine
                .contains
                .iter()
                .map(|contained| {
                    SymbolDefinition::named(SymbolKind::Object, contained.name.as_str())
                })
                .chain(machine.owned_data.iter().map(|owned_data| {
                    SymbolDefinition::named(SymbolKind::Field, owned_data.name.as_str())
                }))
                .chain(machine.states.iter().map(state_symbol_definition)),
        )),
        Item::Platform(platform) => Some(SymbolDefinition::with_children(
            SymbolKind::Platform,
            platform.name.as_str(),
            platform
                .states
                .iter()
                .map(state_signature_symbol_definition),
        )),
        Item::Target(target) => Some(SymbolDefinition::named(
            SymbolKind::Object,
            target.name.as_str(),
        )),
        Item::TrustDefinition(trust_definition) => Some(SymbolDefinition::named(
            SymbolKind::Object,
            trust_definition.name.as_str(),
        )),
        Item::Use(_) => None,
    }
}

fn state_symbol_definition(state: &State) -> SymbolDefinition<'_> {
    SymbolDefinition::with_children(
        SymbolKind::State,
        state.name.as_str(),
        state.parameters.iter().map(|parameter| {
            SymbolDefinition::named(SymbolKind::Parameter, parameter.name.as_str())
        }),
    )
}

fn state_signature_symbol_definition(signature: &StateSignature) -> SymbolDefinition<'_> {
    SymbolDefinition::with_children(
        SymbolKind::State,
        signature.name.as_str(),
        signature.parameters.iter().map(|parameter| {
            SymbolDefinition::named(SymbolKind::Parameter, parameter.name.as_str())
        }),
    )
}

#[cfg(test)]
mod tests {
    use omega_abstract_syntax_tree::identifier::{Identifier, IdentifierPath};
    use omega_abstract_syntax_tree::item::{
        Contains, Item, Machine, OwnedData, State, StateParameter, UseItem,
    };
    use omega_abstract_syntax_tree::statement::{
        Statement, Transition, TransitionGuard, TransitionTarget,
    };
    use omega_abstract_syntax_tree::types::TypeReference;

    use super::{ResolvedDefinitionKind, ResolvedReferenceKind, build_resolve_report};

    fn identifier_path(members: &[&str]) -> IdentifierPath {
        members
            .iter()
            .copied()
            .map(Identifier::generated)
            .collect::<Vec<_>>()
            .into()
    }

    #[test]
    fn collects_definitions_imports_and_references() {
        let report = build_resolve_report(&[
            Item::Use(UseItem {
                path: identifier_path(&["platform", "console"]),
            }),
            Item::Machine(Machine {
                name: Identifier::generated("main"),
                contains: vec![Contains {
                    name: Identifier::generated("console"),
                    type_name: Identifier::generated("Console"),
                }],
                owned_data: vec![OwnedData {
                    name: Identifier::generated("score"),
                    type_reference: TypeReference::named("i32"),
                    initial_value: None,
                }],
                states: vec![
                    State {
                        name: Identifier::generated("entry"),
                        parameters: vec![StateParameter {
                            name: Identifier::generated("amount"),
                            type_reference: TypeReference::named("i32"),
                            is_const: false,
                            is_mutable: false,
                            is_self: false,
                        }],
                        return_type: None,
                        statements: vec![Statement::Transition(Transition {
                            target: TransitionTarget::Named {
                                path: identifier_path(&["finish"]),
                                arguments: Vec::new(),
                            },
                            continuation: None,
                            guard: TransitionGuard::Always,
                        })],
                    },
                    State {
                        name: Identifier::generated("finish"),
                        parameters: Vec::new(),
                        return_type: None,
                        statements: Vec::new(),
                    },
                ],
            }),
        ]);

        assert_eq!(report.imports.len(), 1);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.references.len(), 4);

        let (_, definition) = report
            .definitions
            .iter()
            .find(|(_, definition)| definition.name == "main")
            .expect("main definition should be collected");
        assert_eq!(definition.kind, ResolvedDefinitionKind::Machine);
        assert!(definition.symbol.is_valid());

        assert!(
            report.references.iter().any(|(_, reference)| {
                reference.name == "finish"
                    && reference.kind == ResolvedReferenceKind::TransitionTarget
                    && !report
                        .symbols
                        .path_members(reference.symbol_path)
                        .is_empty()
            }),
            "state transition target should be collected and bound to a symbol path"
        );
    }
}
