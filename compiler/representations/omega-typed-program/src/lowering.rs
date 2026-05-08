use crate::Program;
use crate::data::{DataDefinition, DataField, DataMember, DataVariant};
use crate::expression::{
    BinaryExpression, BinaryOperator, Expression, IndexedExpression, StructLiteral,
    StructLiteralField,
};
use crate::invariant::InvariantDefinition;
use crate::machine::{ContainedObject, Machine, OwnedData};
use crate::platform::Platform;
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::statement::{
    Assignment, Call, LocalData, Statement, Transition, TransitionGuard, TransitionTarget,
};
use crate::types::{TypeConstraint, TypeReference};
use omega_abstract_syntax_tree as ast;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_core::symbols::{
    SymbolDefinition, SymbolKind, SymbolTable, builtin_type_symbol_definitions,
};
use std::sync::Arc;

#[derive(Clone)]
struct InvariantAliases {
    items: Vec<ast::item::InvariantDefinition>,
}

impl InvariantAliases {
    fn build(items: &[ast::item::Item]) -> Result<Self, Diagnostic> {
        let mut aliases = Self { items: Vec::new() };

        for item in items {
            let ast::item::Item::Invariant(invariant) = item else {
                continue;
            };

            if aliases.get(&invariant.name).is_some() {
                return Err(Diagnostic::error(format!(
                    "duplicate invariant `{}`",
                    invariant.name
                )));
            }

            aliases.items.push(invariant.clone());
        }

        Ok(aliases)
    }

    fn get(&self, name: &str) -> Option<&ast::item::InvariantDefinition> {
        self.items.iter().find(|alias| alias.name == name)
    }
}

pub fn lower_program(items: &[ast::item::Item]) -> Result<Program, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    lower_program_with_workers(Arc::new(items.to_vec()), workers.handle())
}

pub fn lower_program_with_workers(
    items: Arc<Vec<ast::item::Item>>,
    workers: WorkerPoolHandle,
) -> Result<Program, Diagnostic> {
    let aliases = InvariantAliases::build(&items)?;
    let mut program = Program::default();

    for alias in &aliases.items {
        let mut expansion_stack = vec![alias.name.to_string()];
        let constraints =
            lower_type_constraints(&alias.constraints, &aliases, &mut expansion_stack)?;
        let constraints = program.type_constraints.insert_many(constraints);

        program.invariant_definitions.push(InvariantDefinition {
            name: alias.name.to_string(),
            constraints,
        });
    }

    let aliases = Arc::new(aliases);
    let item_count = items.len();
    let lowered_items = workers.map_ordered(item_count, move |index| {
        let item = items
            .get(index)
            .expect("lowering worker index should be in range");

        lower_top_level_item(item, &aliases)
    });

    for lowered_item in lowered_items {
        if let Some(lowered_item) = lowered_item? {
            merge_lowered_item(&mut program, lowered_item);
        }
    }

    program.symbols = register_program_symbols(&program);

    Ok(program)
}

struct LoweredTopLevelItem {
    type_constraints: Arena<TypeConstraint>,
    item: LoweredTopLevelItemKind,
}

enum LoweredTopLevelItemKind {
    Data(DataDefinition),
    Machine(Machine),
    Platform(Platform),
}

fn lower_top_level_item(
    item: &ast::item::Item,
    aliases: &InvariantAliases,
) -> Result<Option<LoweredTopLevelItem>, Diagnostic> {
    let mut type_constraints = Arena::new();
    let item =
        match item {
            ast::item::Item::Data(data_definition) => Some(LoweredTopLevelItemKind::Data(
                lower_data_definition(data_definition, aliases, &mut type_constraints)?,
            )),
            ast::item::Item::Machine(machine) => Some(LoweredTopLevelItemKind::Machine(
                lower_machine(machine, aliases, &mut type_constraints)?,
            )),
            ast::item::Item::Platform(platform) => Some(LoweredTopLevelItemKind::Platform(
                lower_platform(platform, aliases, &mut type_constraints)?,
            )),
            ast::item::Item::Capability(_)
            | ast::item::Item::Invariant(_)
            | ast::item::Item::Target(_)
            | ast::item::Item::TrustDefinition(_)
            | ast::item::Item::Use(_) => None,
        };

    Ok(item.map(|item| LoweredTopLevelItem {
        type_constraints,
        item,
    }))
}

fn merge_lowered_item(program: &mut Program, lowered_item: LoweredTopLevelItem) {
    match lowered_item.item {
        LoweredTopLevelItemKind::Data(data_definition) => {
            program.data_definitions.push(remap_data_definition(
                data_definition,
                &lowered_item.type_constraints,
                &mut program.type_constraints,
            ));
        }
        LoweredTopLevelItemKind::Machine(machine) => {
            program.machines.push(remap_machine(
                machine,
                &lowered_item.type_constraints,
                &mut program.type_constraints,
            ));
        }
        LoweredTopLevelItemKind::Platform(platform) => {
            program.platforms.push(remap_platform(
                platform,
                &lowered_item.type_constraints,
                &mut program.type_constraints,
            ));
        }
    }
}

fn register_program_symbols(program: &Program) -> SymbolTable {
    let builder = ProgramSymbolDefinitionBuilder { program };

    SymbolTable::from_definition(SymbolDefinition::static_with_children(
        SymbolKind::Root,
        "program",
        builtin_type_symbol_definitions()
            .into_iter()
            .chain(program.invariant_definitions.iter().map(|invariant| {
                SymbolDefinition::named(SymbolKind::Invariant, invariant.name.as_str())
            }))
            .chain(
                program
                    .data_definitions
                    .iter()
                    .map(|data_definition| builder.data_symbol_definition(data_definition)),
            )
            .chain(
                program
                    .platforms
                    .iter()
                    .map(|platform| builder.platform_symbol_definition(platform)),
            )
            .chain(
                program
                    .machines
                    .iter()
                    .map(|machine| builder.machine_symbol_definition(machine)),
            ),
    ))
}

#[derive(Debug, Clone, Copy)]
struct ProgramSymbolDefinitionBuilder<'program> {
    program: &'program Program,
}

impl<'program> ProgramSymbolDefinitionBuilder<'program> {
    fn data_symbol_definition(
        self,
        data_definition: &'program DataDefinition,
    ) -> SymbolDefinition<'program> {
        SymbolDefinition::with_children(
            SymbolKind::Data,
            data_definition.name.as_str(),
            data_definition
                .members
                .iter()
                .map(|member| self.data_member_symbol_definition(member, 0)),
        )
    }

    fn platform_symbol_definition(
        self,
        platform: &'program Platform,
    ) -> SymbolDefinition<'program> {
        SymbolDefinition::with_children(
            SymbolKind::Platform,
            platform.name.as_str(),
            platform
                .states
                .iter()
                .map(|signature| self.state_signature_symbol_definition(signature)),
        )
    }

    fn machine_symbol_definition(self, machine: &'program Machine) -> SymbolDefinition<'program> {
        SymbolDefinition::with_children(
            SymbolKind::Machine,
            machine.name.as_str(),
            machine
                .contains
                .iter()
                .map(|contained| {
                    SymbolDefinition::with_children(
                        SymbolKind::Object,
                        contained.name.as_str(),
                        self.named_type_children(contained.type_name.as_str(), 0),
                    )
                })
                .chain(machine.owned_data.iter().map(|owned_data| {
                    SymbolDefinition::with_children(
                        SymbolKind::Field,
                        owned_data.name.as_str(),
                        self.type_children(&owned_data.type_reference, 0),
                    )
                }))
                .chain(
                    machine
                        .states
                        .iter()
                        .map(|state| self.state_symbol_definition(state)),
                ),
        )
    }

    fn state_symbol_definition(self, state: &'program State) -> SymbolDefinition<'program> {
        SymbolDefinition::with_children(
            SymbolKind::State,
            state.name.as_str(),
            state
                .parameters
                .iter()
                .map(|parameter| self.parameter_symbol_definition(parameter))
                .chain(
                    state
                        .statements
                        .iter()
                        .filter_map(|statement| self.local_data_symbol_definition(statement)),
                ),
        )
    }

    fn state_signature_symbol_definition(
        self,
        signature: &'program StateSignature,
    ) -> SymbolDefinition<'program> {
        SymbolDefinition::with_children(
            SymbolKind::State,
            signature.name.as_str(),
            signature
                .parameters
                .iter()
                .map(|parameter| self.parameter_symbol_definition(parameter)),
        )
    }

    fn parameter_symbol_definition(
        self,
        parameter: &'program StateParameter,
    ) -> SymbolDefinition<'program> {
        SymbolDefinition::with_children(
            SymbolKind::Parameter,
            parameter.name.as_str(),
            self.type_children(&parameter.type_reference, 0),
        )
    }

    fn local_data_symbol_definition(
        self,
        statement: &'program Statement,
    ) -> Option<SymbolDefinition<'program>> {
        let Statement::LocalData(local_data) = statement else {
            return None;
        };

        Some(SymbolDefinition::with_children(
            SymbolKind::Local,
            local_data.name.as_str(),
            self.type_children(&local_data.type_reference, 0),
        ))
    }

    fn data_member_symbol_definition(
        self,
        member: &'program DataMember,
        depth: usize,
    ) -> SymbolDefinition<'program> {
        match member {
            DataMember::Field(field) => SymbolDefinition::with_children(
                SymbolKind::Field,
                field.name.as_str(),
                self.type_children(&field.type_reference, depth + 1),
            ),
            DataMember::Variant(variant) => {
                SymbolDefinition::named(SymbolKind::Variant, variant.name.as_str())
            }
        }
    }

    fn type_children(
        self,
        type_reference: &'program TypeReference,
        depth: usize,
    ) -> Vec<SymbolDefinition<'program>> {
        if depth > 8 {
            return Vec::new();
        }

        match type_reference {
            TypeReference::Constrained { base_type, .. } => self.type_children(base_type, depth),
            TypeReference::FixedArray { element_type, .. } => {
                self.type_children(element_type, depth + 1)
            }
            TypeReference::Generic { base_name, .. } | TypeReference::Named(base_name) => {
                self.named_type_children(base_name.as_str(), depth + 1)
            }
            TypeReference::Unit => Vec::new(),
        }
    }

    fn named_type_children(self, type_name: &str, depth: usize) -> Vec<SymbolDefinition<'program>> {
        if depth > 8 {
            return Vec::new();
        }

        if let Some(data_definition) = self
            .program
            .data_definitions
            .iter()
            .find(|definition| definition.name == type_name)
        {
            return data_definition
                .members
                .iter()
                .map(|member| self.data_member_symbol_definition(member, depth + 1))
                .collect();
        }

        if let Some(machine) = self
            .program
            .machines
            .iter()
            .find(|machine| machine.name == type_name)
        {
            return machine
                .states
                .iter()
                .map(|state| self.state_symbol_definition(state))
                .collect();
        }

        if let Some(platform) = self
            .program
            .platforms
            .iter()
            .find(|platform| platform.name == type_name)
        {
            return platform
                .states
                .iter()
                .map(|signature| self.state_signature_symbol_definition(signature))
                .collect();
        }

        Vec::new()
    }
}

fn remap_data_definition(
    data_definition: DataDefinition,
    source_constraints: &Arena<TypeConstraint>,
    target_constraints: &mut Arena<TypeConstraint>,
) -> DataDefinition {
    DataDefinition {
        name: data_definition.name,
        members: data_definition
            .members
            .into_iter()
            .map(|member| remap_data_member(member, source_constraints, target_constraints))
            .collect(),
    }
}

fn remap_data_member(
    member: DataMember,
    source_constraints: &Arena<TypeConstraint>,
    target_constraints: &mut Arena<TypeConstraint>,
) -> DataMember {
    match member {
        DataMember::Field(field) => DataMember::Field(DataField {
            name: field.name,
            type_reference: remap_type_reference(
                field.type_reference,
                source_constraints,
                target_constraints,
            ),
        }),
        DataMember::Variant(variant) => DataMember::Variant(variant),
    }
}

fn remap_machine(
    machine: Machine,
    source_constraints: &Arena<TypeConstraint>,
    target_constraints: &mut Arena<TypeConstraint>,
) -> Machine {
    Machine {
        name: machine.name,
        contains: machine.contains,
        owned_data: machine
            .owned_data
            .into_iter()
            .map(|owned_data| remap_owned_data(owned_data, source_constraints, target_constraints))
            .collect(),
        states: machine
            .states
            .into_iter()
            .map(|state| remap_state(state, source_constraints, target_constraints))
            .collect(),
    }
}

fn remap_owned_data(
    owned_data: OwnedData,
    source_constraints: &Arena<TypeConstraint>,
    target_constraints: &mut Arena<TypeConstraint>,
) -> OwnedData {
    OwnedData {
        name: owned_data.name,
        type_reference: remap_type_reference(
            owned_data.type_reference,
            source_constraints,
            target_constraints,
        ),
        initial_value: owned_data.initial_value,
    }
}

fn remap_platform(
    platform: Platform,
    source_constraints: &Arena<TypeConstraint>,
    target_constraints: &mut Arena<TypeConstraint>,
) -> Platform {
    Platform {
        name: platform.name,
        states: platform
            .states
            .into_iter()
            .map(|state| remap_state_signature(state, source_constraints, target_constraints))
            .collect(),
    }
}

fn remap_state(
    state: State,
    source_constraints: &Arena<TypeConstraint>,
    target_constraints: &mut Arena<TypeConstraint>,
) -> State {
    State {
        name: state.name,
        return_type: state.return_type.map(|return_type| {
            remap_type_reference(return_type, source_constraints, target_constraints)
        }),
        parameters: state
            .parameters
            .into_iter()
            .map(|parameter| {
                remap_state_parameter(parameter, source_constraints, target_constraints)
            })
            .collect(),
        statements: state
            .statements
            .into_iter()
            .map(|statement| remap_statement(statement, source_constraints, target_constraints))
            .collect(),
    }
}

fn remap_state_signature(
    signature: StateSignature,
    source_constraints: &Arena<TypeConstraint>,
    target_constraints: &mut Arena<TypeConstraint>,
) -> StateSignature {
    StateSignature {
        name: signature.name,
        return_type: signature.return_type.map(|return_type| {
            remap_type_reference(return_type, source_constraints, target_constraints)
        }),
        parameters: signature
            .parameters
            .into_iter()
            .map(|parameter| {
                remap_state_parameter(parameter, source_constraints, target_constraints)
            })
            .collect(),
    }
}

fn remap_state_parameter(
    parameter: StateParameter,
    source_constraints: &Arena<TypeConstraint>,
    target_constraints: &mut Arena<TypeConstraint>,
) -> StateParameter {
    StateParameter {
        name: parameter.name,
        type_reference: remap_type_reference(
            parameter.type_reference,
            source_constraints,
            target_constraints,
        ),
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    }
}

fn remap_statement(
    statement: Statement,
    source_constraints: &Arena<TypeConstraint>,
    target_constraints: &mut Arena<TypeConstraint>,
) -> Statement {
    match statement {
        Statement::LocalData(local_data) => Statement::LocalData(LocalData {
            name: local_data.name,
            type_reference: remap_type_reference(
                local_data.type_reference,
                source_constraints,
                target_constraints,
            ),
        }),
        Statement::Assignment(_)
        | Statement::Call(_)
        | Statement::Expression(_)
        | Statement::Transition(_) => statement,
    }
}

fn remap_type_reference(
    type_reference: TypeReference,
    source_constraints: &Arena<TypeConstraint>,
    target_constraints: &mut Arena<TypeConstraint>,
) -> TypeReference {
    match type_reference {
        TypeReference::Constrained {
            base_type,
            constraints,
        } => TypeReference::Constrained {
            base_type: Box::new(remap_type_reference(
                *base_type,
                source_constraints,
                target_constraints,
            )),
            constraints: target_constraints.insert_many(
                source_constraints
                    .span_or_empty(constraints)
                    .iter()
                    .cloned(),
            ),
        },
        TypeReference::FixedArray {
            element_type,
            length,
        } => TypeReference::FixedArray {
            element_type: Box::new(remap_type_reference(
                *element_type,
                source_constraints,
                target_constraints,
            )),
            length,
        },
        TypeReference::Generic {
            base_name,
            arguments,
        } => TypeReference::Generic {
            base_name,
            arguments: arguments
                .into_iter()
                .map(|argument| {
                    remap_type_reference(argument, source_constraints, target_constraints)
                })
                .collect(),
        },
        TypeReference::Named(_) | TypeReference::Unit => type_reference,
    }
}

fn lower_data_definition(
    data_definition: &ast::item::DataDefinition,
    aliases: &InvariantAliases,
    type_constraints: &mut Arena<TypeConstraint>,
) -> Result<DataDefinition, Diagnostic> {
    let members = data_definition
        .members
        .iter()
        .map(|member| lower_data_member(member, aliases, type_constraints))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DataDefinition {
        name: data_definition.name.to_string(),
        members,
    })
}

fn lower_data_member(
    member: &ast::item::DataMember,
    aliases: &InvariantAliases,
    type_constraints: &mut Arena<TypeConstraint>,
) -> Result<DataMember, Diagnostic> {
    match member {
        ast::item::DataMember::Field(field) => Ok(DataMember::Field(DataField {
            name: field.name.to_string(),
            type_reference: lower_type_reference(&field.type_reference, aliases, type_constraints)?,
        })),
        ast::item::DataMember::Variant(variant) => Ok(DataMember::Variant(DataVariant {
            name: variant.name.to_string(),
        })),
    }
}

fn lower_machine(
    machine: &ast::item::Machine,
    aliases: &InvariantAliases,
    type_constraints: &mut Arena<TypeConstraint>,
) -> Result<Machine, Diagnostic> {
    let contains = machine
        .contains
        .iter()
        .map(|contained_object| ContainedObject {
            name: contained_object.name.to_string(),
            type_name: contained_object.type_name.to_string(),
        })
        .collect();

    let owned_data = machine
        .owned_data
        .iter()
        .map(|owned_data| lower_owned_data(owned_data, aliases, type_constraints))
        .collect::<Result<Vec<_>, _>>()?;

    let states = machine
        .states
        .iter()
        .map(|state| lower_state(state, aliases, type_constraints))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Machine {
        name: machine.name.to_string(),
        contains,
        owned_data,
        states,
    })
}

fn lower_owned_data(
    owned_data: &ast::item::OwnedData,
    aliases: &InvariantAliases,
    type_constraints: &mut Arena<TypeConstraint>,
) -> Result<OwnedData, Diagnostic> {
    Ok(OwnedData {
        name: owned_data.name.to_string(),
        type_reference: lower_type_reference(
            &owned_data.type_reference,
            aliases,
            type_constraints,
        )?,
        initial_value: owned_data
            .initial_value
            .as_ref()
            .map(lower_expression)
            .transpose()?,
    })
}

fn lower_platform(
    platform: &ast::item::Platform,
    aliases: &InvariantAliases,
    type_constraints: &mut Arena<TypeConstraint>,
) -> Result<Platform, Diagnostic> {
    let states = platform
        .states
        .iter()
        .map(|signature| lower_state_signature(signature, aliases, type_constraints))
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    Ok(Platform {
        name: platform.name.to_string(),
        states,
    })
}

fn lower_state_signature(
    signature: &ast::item::StateSignature,
    aliases: &InvariantAliases,
    type_constraints: &mut Arena<TypeConstraint>,
) -> Result<StateSignature, Diagnostic> {
    Ok(StateSignature {
        name: signature.name.to_string(),
        return_type: signature
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference(type_reference, aliases, type_constraints))
            .transpose()?,
        parameters: signature
            .parameters
            .iter()
            .map(|parameter| {
                Ok(StateParameter {
                    name: parameter.name.to_string(),
                    type_reference: lower_type_reference(
                        &parameter.type_reference,
                        aliases,
                        type_constraints,
                    )?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                    is_self: parameter.is_self,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?,
    })
}

fn lower_type_reference(
    type_reference: &ast::types::TypeReference,
    aliases: &InvariantAliases,
    type_constraints: &mut Arena<TypeConstraint>,
) -> Result<TypeReference, Diagnostic> {
    match type_reference {
        ast::types::TypeReference::Constrained {
            base_type,
            constraints,
        } => Ok(TypeReference::Constrained {
            base_type: Box::new(lower_type_reference(base_type, aliases, type_constraints)?),
            constraints: {
                let lowered_constraints =
                    lower_type_constraints(constraints, aliases, &mut Vec::new())?;
                type_constraints.insert_many(lowered_constraints)
            },
        }),
        ast::types::TypeReference::FixedArray {
            element_type,
            length,
        } => Ok(TypeReference::FixedArray {
            element_type: Box::new(lower_type_reference(
                element_type,
                aliases,
                type_constraints,
            )?),
            length: *length,
        }),
        ast::types::TypeReference::Generic {
            base_name,
            arguments,
        } => Ok(TypeReference::Generic {
            base_name: base_name.to_string(),
            arguments: arguments
                .iter()
                .map(|argument| lower_type_reference(argument, aliases, type_constraints))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        ast::types::TypeReference::Named(name) => Ok(TypeReference::Named(name.to_string())),
        ast::types::TypeReference::Unit => Ok(TypeReference::Unit),
    }
}

fn lower_type_constraint(
    constraint: &ast::types::TypeConstraint,
) -> Result<TypeConstraint, Diagnostic> {
    match constraint {
        ast::types::TypeConstraint::Named(name) => Ok(TypeConstraint::Named(name.to_string())),
        ast::types::TypeConstraint::Range { minimum, maximum } => Ok(TypeConstraint::Range {
            minimum: lower_expression(minimum)?,
            maximum: lower_expression(maximum)?,
        }),
    }
}

fn lower_type_constraints(
    constraints: &[ast::types::TypeConstraint],
    aliases: &InvariantAliases,
    expansion_stack: &mut Vec<String>,
) -> Result<Vec<TypeConstraint>, Diagnostic> {
    let mut lowered_constraints = Vec::new();

    for constraint in constraints {
        match constraint {
            ast::types::TypeConstraint::Named(name) => {
                if let Some(alias) = aliases.get(name.as_str()) {
                    if expansion_stack.iter().any(|entry| entry == name.as_str()) {
                        return Err(Diagnostic::error(format!(
                            "recursive invariant alias `{name}`"
                        )));
                    }

                    expansion_stack.push(name.to_string());
                    lowered_constraints.extend(lower_type_constraints(
                        &alias.constraints,
                        aliases,
                        expansion_stack,
                    )?);
                    expansion_stack.pop();
                } else {
                    lowered_constraints.push(TypeConstraint::Named(name.to_string()));
                }
            }
            ast::types::TypeConstraint::Range { .. } => {
                lowered_constraints.push(lower_type_constraint(constraint)?);
            }
        }
    }

    Ok(lowered_constraints)
}

fn lower_state(
    state: &ast::item::State,
    aliases: &InvariantAliases,
    type_constraints: &mut Arena<TypeConstraint>,
) -> Result<State, Diagnostic> {
    let statements = state
        .statements
        .iter()
        .map(|statement| lower_statement(statement, aliases, type_constraints))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(State {
        name: state.name.to_string(),
        return_type: state
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference(type_reference, aliases, type_constraints))
            .transpose()?,
        parameters: state
            .parameters
            .iter()
            .map(|parameter| {
                Ok(StateParameter {
                    name: parameter.name.to_string(),
                    type_reference: lower_type_reference(
                        &parameter.type_reference,
                        aliases,
                        type_constraints,
                    )?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                    is_self: parameter.is_self,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?,
        statements,
    })
}

fn lower_statement(
    statement: &ast::statement::Statement,
    aliases: &InvariantAliases,
    type_constraints: &mut Arena<TypeConstraint>,
) -> Result<Statement, Diagnostic> {
    match statement {
        ast::statement::Statement::Assignment(assignment) => {
            Ok(Statement::Assignment(Assignment {
                target: lower_expression(&assignment.target)?,
                value: lower_expression(&assignment.value)?,
            }))
        }
        ast::statement::Statement::Call(call) => Ok(Statement::Call(Call {
            receiver: call.receiver.as_ref().map(ToString::to_string),
            target: call.target.to_string(),
            arguments: call
                .arguments
                .iter()
                .map(lower_expression)
                .collect::<Result<Vec<_>, _>>()?,
        })),
        ast::statement::Statement::Expression(expression) => {
            Ok(Statement::Expression(lower_expression(expression)?))
        }
        ast::statement::Statement::LocalData(local_data) => Ok(Statement::LocalData(LocalData {
            name: local_data.name.to_string(),
            type_reference: lower_type_reference(
                &local_data.type_reference,
                aliases,
                type_constraints,
            )?,
        })),
        ast::statement::Statement::Transition(transition) => {
            Ok(Statement::Transition(Transition {
                target: lower_transition_target(&transition.target)?,
                continuation: transition
                    .continuation
                    .as_ref()
                    .map(lower_transition_target)
                    .transpose()?,
                guard: lower_transition_guard(&transition.guard)?,
            }))
        }
    }
}

fn lower_transition_guard(
    guard: &ast::statement::TransitionGuard,
) -> Result<TransitionGuard, Diagnostic> {
    match guard {
        ast::statement::TransitionGuard::Always => Ok(TransitionGuard::Always),
        ast::statement::TransitionGuard::When(expression) => {
            Ok(TransitionGuard::When(lower_expression(expression)?))
        }
    }
}

fn lower_identifier_path(path: &ast::identifier::IdentifierPath) -> Vec<String> {
    path.iter().map(ToString::to_string).collect()
}

fn lower_expression(expression: &ast::expression::Expression) -> Result<Expression, Diagnostic> {
    match expression {
        ast::expression::Expression::ArrayLiteral(values) => Ok(Expression::ArrayLiteral(
            values
                .iter()
                .map(lower_expression)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        ast::expression::Expression::Binary(binary) => {
            Ok(Expression::Binary(Box::new(BinaryExpression {
                left: lower_expression(&binary.left)?,
                operator: lower_binary_operator(binary.operator),
                right: lower_expression(&binary.right)?,
            })))
        }
        ast::expression::Expression::Boolean(value) => Ok(Expression::Boolean(*value)),
        ast::expression::Expression::Indexed(indexed) => {
            Ok(Expression::Indexed(Box::new(IndexedExpression {
                collection: lower_expression(&indexed.collection)?,
                index: lower_expression(&indexed.index)?,
            })))
        }
        ast::expression::Expression::Integer(value) => Ok(Expression::Integer(*value)),
        ast::expression::Expression::Float(value) => Ok(Expression::Float(value.clone())),
        ast::expression::Expression::Mutable(inner_expression) => Ok(Expression::Mutable(
            Box::new(lower_expression(inner_expression)?),
        )),
        ast::expression::Expression::Name(path) => {
            Ok(Expression::Name(lower_identifier_path(path)))
        }
        ast::expression::Expression::StructLiteral(struct_literal) => {
            Ok(Expression::StructLiteral(StructLiteral {
                type_name: struct_literal.type_name.to_string(),
                fields: struct_literal
                    .fields
                    .iter()
                    .map(|field| {
                        Ok(StructLiteralField {
                            name: field.name.to_string(),
                            value: lower_expression(&field.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?,
            }))
        }
        ast::expression::Expression::String(value) => Ok(Expression::String(value.clone())),
    }
}

fn lower_binary_operator(operator: ast::expression::BinaryOperator) -> BinaryOperator {
    match operator {
        ast::expression::BinaryOperator::Add => BinaryOperator::Add,
        ast::expression::BinaryOperator::And => BinaryOperator::And,
        ast::expression::BinaryOperator::Equal => BinaryOperator::Equal,
        ast::expression::BinaryOperator::Greater => BinaryOperator::Greater,
        ast::expression::BinaryOperator::GreaterOrEqual => BinaryOperator::GreaterOrEqual,
        ast::expression::BinaryOperator::Less => BinaryOperator::Less,
        ast::expression::BinaryOperator::LessOrEqual => BinaryOperator::LessOrEqual,
        ast::expression::BinaryOperator::NotEqual => BinaryOperator::NotEqual,
        ast::expression::BinaryOperator::Or => BinaryOperator::Or,
    }
}

fn lower_transition_target(
    target: &ast::statement::TransitionTarget,
) -> Result<TransitionTarget, Diagnostic> {
    match target {
        ast::statement::TransitionTarget::Named { path, arguments } => {
            Ok(TransitionTarget::Named {
                path: lower_identifier_path(path),
                arguments: arguments
                    .iter()
                    .map(lower_expression)
                    .collect::<Result<Vec<_>, Diagnostic>>()?,
            })
        }
        ast::statement::TransitionTarget::SelfTarget => Ok(TransitionTarget::SelfTarget),
        ast::statement::TransitionTarget::Terminal => Ok(TransitionTarget::Terminal),
    }
}

#[cfg(test)]
mod tests {
    use crate::Program;
    use crate::data::{DataDefinition, DataField, DataMember};
    use crate::machine::{ContainedObject, Machine};
    use crate::platform::Platform;
    use crate::signature::StateSignature;
    use crate::state::State;
    use crate::statement::{LocalData, Statement};
    use crate::types::TypeReference;

    use super::register_program_symbols;

    #[test]
    fn typed_program_symbols_project_children_from_declared_types() {
        let mut program = Program {
            data_definitions: vec![DataDefinition {
                name: "Room".to_owned(),
                members: vec![DataMember::Field(DataField {
                    name: "label".to_owned(),
                    type_reference: TypeReference::Named("String".to_owned()),
                })],
            }],
            machines: vec![Machine {
                name: "main".to_owned(),
                contains: vec![ContainedObject {
                    name: "console".to_owned(),
                    type_name: "Console".to_owned(),
                }],
                owned_data: Vec::new(),
                states: vec![State {
                    name: "entry".to_owned(),
                    parameters: Vec::new(),
                    return_type: None,
                    statements: vec![Statement::LocalData(LocalData {
                        name: "room".to_owned(),
                        type_reference: TypeReference::Named("Room".to_owned()),
                    })],
                }],
            }],
            platforms: vec![Platform {
                name: "Console".to_owned(),
                states: vec![StateSignature {
                    name: "write_line".to_owned(),
                    parameters: Vec::new(),
                    return_type: None,
                }],
            }],
            ..Program::default()
        };
        program.symbols = register_program_symbols(&program);

        let root = program.symbols.root();
        let main = program
            .symbols
            .find_child_by_name(root, "main")
            .expect("main should resolve");
        let console = program
            .symbols
            .find_child_by_name(main, "console")
            .expect("console object should resolve");
        let console_write_line = program
            .symbols
            .find_child_by_name(console, "write_line")
            .expect("contained platform states should project under the object");
        let entry = program
            .symbols
            .find_child_by_name(main, "entry")
            .expect("entry should resolve");
        let room = program
            .symbols
            .find_child_by_name(entry, "room")
            .expect("local room should resolve");
        let room_label = program
            .symbols
            .find_child_by_name(room, "label")
            .expect("local data fields should project from their type");

        assert_eq!(program.symbols.name(console_write_line), "write_line");
        assert_eq!(program.symbols.name(room_label), "label");
    }
}
