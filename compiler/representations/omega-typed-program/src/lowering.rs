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
use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};

struct InvariantAliases<'ast> {
    items: Vec<&'ast ast::item::InvariantDefinition>,
}

impl<'ast> InvariantAliases<'ast> {
    fn build(items: &'ast [ast::item::Item]) -> Result<Self, Diagnostic> {
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

            aliases.items.push(invariant);
        }

        Ok(aliases)
    }

    fn get(&self, name: &str) -> Option<&ast::item::InvariantDefinition> {
        self.items.iter().copied().find(|alias| alias.name == name)
    }
}

pub fn lower_program(items: &[ast::item::Item]) -> Result<Program, Diagnostic> {
    let aliases = InvariantAliases::build(items)?;
    let mut program = Program::default();

    for alias in &aliases.items {
        let mut expansion_stack = vec![alias.name.clone()];
        let constraints =
            lower_type_constraints(&alias.constraints, &aliases, &mut expansion_stack)?;
        let constraints = program.type_constraints.insert_many(constraints);

        program.invariant_definitions.push(InvariantDefinition {
            name: alias.name.clone(),
            constraints,
        });
    }

    for item in items {
        match item {
            ast::item::Item::Capability(_) => {}
            ast::item::Item::Data(data_definition) => {
                program.data_definitions.push(lower_data_definition(
                    data_definition,
                    &aliases,
                    &mut program.type_constraints,
                )?);
            }
            ast::item::Item::Invariant(_) => {}
            ast::item::Item::Use(_) => {}
            ast::item::Item::Machine(machine) => {
                program.machines.push(lower_machine(
                    machine,
                    &aliases,
                    &mut program.type_constraints,
                )?);
            }
            ast::item::Item::Platform(platform) => {
                program.platforms.push(lower_platform(
                    platform,
                    &aliases,
                    &mut program.type_constraints,
                )?);
            }
            ast::item::Item::Target(_) | ast::item::Item::TrustDefinition(_) => {}
        }
    }

    let mut symbols = SymbolTable::new();
    register_program_symbols(&mut symbols, &program);
    program.symbols = symbols;

    Ok(program)
}

fn register_program_symbols(symbols: &mut SymbolTable, program: &Program) {
    let root = symbols.insert_named(SymbolHandle::invalid(), SymbolKind::Root, "program");

    for invariant in &program.invariant_definitions {
        symbols.insert_named(root, SymbolKind::Invariant, invariant.name.as_str());
    }

    for data_definition in &program.data_definitions {
        let data = symbols.insert_named(root, SymbolKind::Data, data_definition.name.as_str());

        for member in &data_definition.members {
            match member {
                DataMember::Field(field) => {
                    symbols.insert_named(data, SymbolKind::Field, field.name.as_str());
                }
                DataMember::Variant(variant) => {
                    symbols.insert_named(data, SymbolKind::Variant, variant.name.as_str());
                }
            }
        }
    }

    for platform in &program.platforms {
        let platform_symbol =
            symbols.insert_named(root, SymbolKind::Platform, platform.name.as_str());

        for state in &platform.states {
            let state_symbol =
                symbols.insert_named(platform_symbol, SymbolKind::State, state.name.as_str());
            register_state_parameters(symbols, state_symbol, &state.parameters);
        }
    }

    for machine in &program.machines {
        let machine_symbol = symbols.insert_named(root, SymbolKind::Machine, machine.name.as_str());

        for contained in &machine.contains {
            symbols.insert_named(machine_symbol, SymbolKind::Object, contained.name.as_str());
        }

        for owned_data in &machine.owned_data {
            symbols.insert_named(machine_symbol, SymbolKind::Field, owned_data.name.as_str());
        }

        for state in &machine.states {
            let state_symbol =
                symbols.insert_named(machine_symbol, SymbolKind::State, state.name.as_str());
            register_state_parameters(symbols, state_symbol, &state.parameters);
        }
    }
}

fn register_state_parameters(
    symbols: &mut SymbolTable,
    state_symbol: SymbolHandle,
    parameters: &[StateParameter],
) {
    for parameter in parameters {
        symbols.insert_named(state_symbol, SymbolKind::Parameter, parameter.name.as_str());
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
        name: data_definition.name.clone(),
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
            name: field.name.clone(),
            type_reference: lower_type_reference(&field.type_reference, aliases, type_constraints)?,
        })),
        ast::item::DataMember::Variant(variant) => Ok(DataMember::Variant(DataVariant {
            name: variant.name.clone(),
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
            name: contained_object.name.clone(),
            type_name: contained_object.type_name.clone(),
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
        name: machine.name.clone(),
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
        name: owned_data.name.clone(),
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
        name: platform.name.clone(),
        states,
    })
}

fn lower_state_signature(
    signature: &ast::item::StateSignature,
    aliases: &InvariantAliases,
    type_constraints: &mut Arena<TypeConstraint>,
) -> Result<StateSignature, Diagnostic> {
    Ok(StateSignature {
        name: signature.name.clone(),
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
                    name: parameter.name.clone(),
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
            base_name: base_name.clone(),
            arguments: arguments
                .iter()
                .map(|argument| lower_type_reference(argument, aliases, type_constraints))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        ast::types::TypeReference::Named(name) => Ok(TypeReference::Named(name.clone())),
        ast::types::TypeReference::Unit => Ok(TypeReference::Unit),
    }
}

fn lower_type_constraint(
    constraint: &ast::types::TypeConstraint,
) -> Result<TypeConstraint, Diagnostic> {
    match constraint {
        ast::types::TypeConstraint::Named(name) => Ok(TypeConstraint::Named(name.clone())),
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
                if let Some(alias) = aliases.get(name) {
                    if expansion_stack.contains(name) {
                        return Err(Diagnostic::error(format!(
                            "recursive invariant alias `{name}`"
                        )));
                    }

                    expansion_stack.push(name.clone());
                    lowered_constraints.extend(lower_type_constraints(
                        &alias.constraints,
                        aliases,
                        expansion_stack,
                    )?);
                    expansion_stack.pop();
                } else {
                    lowered_constraints.push(TypeConstraint::Named(name.clone()));
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
        name: state.name.clone(),
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
                    name: parameter.name.clone(),
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
            receiver: call.receiver.clone(),
            target: call.target.clone(),
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
            name: local_data.name.clone(),
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
        ast::expression::Expression::Name(path) => Ok(Expression::Name(path.clone())),
        ast::expression::Expression::StructLiteral(struct_literal) => {
            Ok(Expression::StructLiteral(StructLiteral {
                type_name: struct_literal.type_name.clone(),
                fields: struct_literal
                    .fields
                    .iter()
                    .map(|field| {
                        Ok(StructLiteralField {
                            name: field.name.clone(),
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
                path: path.clone(),
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
