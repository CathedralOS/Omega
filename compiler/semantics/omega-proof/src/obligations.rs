use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::Program;
use omega_typed_trees::expression::{BinaryOperator, Expression, FloatLiteral};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::name::ProgramName;
use omega_typed_trees::signature::StateParameter;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    Assignment, Call, Transition, TransitionGuard, TransitionTarget,
};
use omega_typed_trees::types::{TypeConstraint, TypeReference};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofPlan {
    pub obligations: Vec<ProofObligation>,
    pub type_constraints: Arena<TypeConstraint>,
}

impl ProofPlan {
    fn store_constraints(&mut self, constraints: &[TypeConstraint]) -> HandleSpan<TypeConstraint> {
        self.type_constraints
            .insert_many(constraints.iter().cloned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofObligation {
    BoundedAssignment(BoundedAssignmentObligation),
    BoundedCallArgument(BoundedCallArgumentObligation),
    BoundedInitializer(BoundedInitializerObligation),
    BoundedStateReturn(BoundedStateReturnObligation),
    BoundedValue(BoundedValueObligation),
    BoundedTransitionArgument(BoundedTransitionArgumentObligation),
    GuardedTransition(GuardedTransitionObligation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedValueObligation {
    pub owner: String,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedTransitionObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: String,
    pub state_symbol: SymbolHandle,
    pub state: String,
    pub target: TransitionTarget,
    pub guard: TransitionGuard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedAssignmentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: String,
    pub state_symbol: SymbolHandle,
    pub state: String,
    pub target: Expression,
    pub value: Expression,
    pub value_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedCallArgumentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: String,
    pub state_symbol: SymbolHandle,
    pub state: String,
    pub receiver: Option<String>,
    pub target: String,
    pub parameter: String,
    pub argument: Expression,
    pub argument_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedInitializerObligation {
    pub owner: String,
    pub value: Expression,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedStateReturnObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: String,
    pub state_symbol: SymbolHandle,
    pub state: String,
    pub value: Expression,
    pub value_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedTransitionArgumentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: String,
    pub state_symbol: SymbolHandle,
    pub state: String,
    pub target: TransitionTarget,
    pub parameter: String,
    pub argument: Expression,
    pub argument_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
    pub guard: TransitionGuard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegerRange {
    minimum: i64,
    maximum: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FloatRange {
    minimum: f64,
    maximum: f64,
}

pub fn build_proof_plan(program: &Program) -> ProofPlan {
    let mut proof_plan = ProofPlan::default();

    for machine in &program.machines {
        for owned_data in &machine.owned_data {
            collect_bounded_value_obligation(
                program,
                format!(
                    "machine `{}` owned data `{}`",
                    machine.name, owned_data.name
                ),
                &owned_data.type_reference,
                &mut proof_plan,
            );
            if let Some(initial_value) = &owned_data.initial_value {
                collect_bounded_initializer_obligation(
                    program,
                    format!(
                        "machine `{}` owned data `{}`",
                        machine.name, owned_data.name
                    ),
                    &owned_data.type_reference,
                    initial_value,
                    &mut proof_plan,
                );
            }
        }

        for state in &machine.states {
            for parameter in &state.parameters {
                collect_bounded_value_obligation(
                    program,
                    format!(
                        "machine `{}` state `{}` parameter `{}`",
                        machine.name, state.name, parameter.name
                    ),
                    &parameter.type_reference,
                    &mut proof_plan,
                );
            }

            if let Some(return_type) = &state.return_type {
                collect_bounded_value_obligation(
                    program,
                    format!(
                        "machine `{}` state `{}` return value",
                        machine.name, state.name
                    ),
                    return_type,
                    &mut proof_plan,
                );
                collect_bounded_state_return_obligation(
                    program,
                    machine,
                    state,
                    return_type,
                    &mut proof_plan,
                );
            }

            for statement in &state.statements {
                let transition = match statement {
                    omega_typed_trees::statement::Statement::Assignment(assignment) => {
                        collect_bounded_assignment_obligation(
                            program,
                            machine,
                            state,
                            assignment,
                            &mut proof_plan,
                        );
                        continue;
                    }
                    omega_typed_trees::statement::Statement::Call(call) => {
                        collect_bounded_call_argument_obligations(
                            program,
                            machine,
                            state,
                            call,
                            &mut proof_plan,
                        );
                        continue;
                    }
                    omega_typed_trees::statement::Statement::Transition(transition) => transition,
                    _ => continue,
                };

                if let TransitionGuard::When(_) = &transition.guard {
                    proof_plan
                        .obligations
                        .push(ProofObligation::GuardedTransition(
                            GuardedTransitionObligation {
                                machine_symbol: machine.symbol,
                                machine: machine.name.to_string(),
                                state_symbol: state.symbol,
                                state: state.name.to_string(),
                                target: transition.target.clone(),
                                guard: transition.guard.clone(),
                            },
                        ));
                }

                collect_bounded_transition_argument_obligations(
                    program,
                    machine,
                    state,
                    transition,
                    &mut proof_plan,
                );
            }
        }
    }

    proof_plan
}

fn collect_bounded_value_obligation(
    program: &Program,
    owner: String,
    type_reference: &TypeReference,
    proof_plan: &mut ProofPlan,
) {
    match type_reference {
        TypeReference::Reference { referee, .. } => {
            collect_bounded_value_obligation(program, owner, referee, proof_plan);
        }
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));
            proof_plan
                .obligations
                .push(ProofObligation::BoundedValue(BoundedValueObligation {
                    owner,
                    base_type: base_type.as_ref().clone(),
                    constraints,
                }));
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_bounded_value_obligation(program, owner, element_type, proof_plan);
        }
        TypeReference::Slice { element_type } => {
            collect_bounded_value_obligation(program, owner, element_type, proof_plan);
        }
        TypeReference::Generic { arguments, .. } => {
            for argument in arguments {
                collect_bounded_value_obligation(program, owner.clone(), argument, proof_plan);
            }
        }
        TypeReference::Named { name: _, .. } => {}
        TypeReference::Unit => {}
    }
}

fn collect_bounded_initializer_obligation(
    program: &Program,
    owner: String,
    type_reference: &TypeReference,
    value: &Expression,
    proof_plan: &mut ProofPlan,
) {
    match type_reference {
        TypeReference::Reference { referee, .. } => {
            collect_bounded_initializer_obligation(program, owner, referee, value, proof_plan);
        }
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));
            proof_plan
                .obligations
                .push(ProofObligation::BoundedInitializer(
                    BoundedInitializerObligation {
                        owner,
                        value: value.clone(),
                        base_type: base_type.as_ref().clone(),
                        constraints,
                    },
                ));
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_bounded_initializer_obligation(program, owner, element_type, value, proof_plan);
        }
        TypeReference::Slice { element_type } => {
            collect_bounded_initializer_obligation(program, owner, element_type, value, proof_plan);
        }
        TypeReference::Generic { arguments, .. } => {
            for argument in arguments {
                collect_bounded_initializer_obligation(
                    program,
                    owner.clone(),
                    argument,
                    value,
                    proof_plan,
                );
            }
        }
        TypeReference::Named { name: _, .. } => {}
        TypeReference::Unit => {}
    }
}

fn collect_bounded_assignment_obligation(
    program: &Program,
    machine: &Machine,
    state: &State,
    assignment: &Assignment,
    proof_plan: &mut ProofPlan,
) {
    let Some(TypeReference::Constrained {
        base_type,
        constraints,
    }) = expression_type_reference(program, machine, state, &assignment.target)
    else {
        return;
    };

    let value_constraints = expression_constraints(program, machine, state, &assignment.value);
    let value_constraints = proof_plan.store_constraints(&value_constraints);
    let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));

    proof_plan
        .obligations
        .push(ProofObligation::BoundedAssignment(
            BoundedAssignmentObligation {
                machine_symbol: machine.symbol,
                machine: machine.name.to_string(),
                state_symbol: state.symbol,
                state: state.name.to_string(),
                target: assignment.target.clone(),
                value: assignment.value.clone(),
                value_constraints,
                base_type: base_type.as_ref().clone(),
                constraints,
            },
        ));
}

fn collect_bounded_transition_argument_obligations(
    program: &Program,
    machine: &Machine,
    state: &State,
    transition: &Transition,
    proof_plan: &mut ProofPlan,
) {
    let Some((target_state, arguments)) =
        transition_target_state_and_arguments(program, machine, state, &transition.target)
    else {
        return;
    };

    for (parameter, argument) in callable_parameters(target_state).zip(arguments.iter()) {
        let TypeReference::Constrained {
            base_type,
            constraints,
        } = &parameter.type_reference
        else {
            continue;
        };

        let argument_constraints = expression_constraints(program, machine, state, argument);
        let argument_constraints = proof_plan.store_constraints(&argument_constraints);
        let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));

        proof_plan
            .obligations
            .push(ProofObligation::BoundedTransitionArgument(
                BoundedTransitionArgumentObligation {
                    machine_symbol: machine.symbol,
                    machine: machine.name.to_string(),
                    state_symbol: state.symbol,
                    state: state.name.to_string(),
                    target: transition.target.clone(),
                    parameter: parameter.name.to_string(),
                    argument: argument.clone(),
                    argument_constraints,
                    base_type: base_type.as_ref().clone(),
                    constraints,
                    guard: transition.guard.clone(),
                },
            ));
    }
}

fn collect_bounded_call_argument_obligations(
    program: &Program,
    machine: &Machine,
    state: &State,
    call: &Call,
    proof_plan: &mut ProofPlan,
) {
    let Some(parameters) = call_target_parameters(program, machine, call) else {
        return;
    };

    for (parameter, argument) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(call.arguments.iter())
    {
        let TypeReference::Constrained {
            base_type,
            constraints,
        } = &parameter.type_reference
        else {
            continue;
        };

        let argument_constraints = expression_constraints(program, machine, state, argument);
        let argument_constraints = proof_plan.store_constraints(&argument_constraints);
        let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));

        proof_plan
            .obligations
            .push(ProofObligation::BoundedCallArgument(
                BoundedCallArgumentObligation {
                    machine_symbol: machine.symbol,
                    machine: machine.name.to_string(),
                    state_symbol: state.symbol,
                    state: state.name.to_string(),
                    receiver: call.receiver.as_ref().map(display_name_path),
                    target: call.target.to_string(),
                    parameter: parameter.name.to_string(),
                    argument: argument.clone(),
                    argument_constraints,
                    base_type: base_type.as_ref().clone(),
                    constraints,
                },
            ));
    }
}

fn collect_bounded_state_return_obligation(
    program: &Program,
    machine: &Machine,
    state: &State,
    return_type: &TypeReference,
    proof_plan: &mut ProofPlan,
) {
    let TypeReference::Constrained {
        base_type,
        constraints,
    } = return_type
    else {
        return;
    };
    let Some(omega_typed_trees::statement::Statement::Expression(value)) =
        state.statements.last()
    else {
        return;
    };

    let value_constraints = expression_constraints(program, machine, state, value);
    let value_constraints = proof_plan.store_constraints(&value_constraints);
    let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));

    proof_plan
        .obligations
        .push(ProofObligation::BoundedStateReturn(
            BoundedStateReturnObligation {
                machine_symbol: machine.symbol,
                machine: machine.name.to_string(),
                state_symbol: state.symbol,
                state: state.name.to_string(),
                value: value.clone(),
                value_constraints,
                base_type: base_type.as_ref().clone(),
                constraints,
            },
        ));
}

fn call_target_parameters<'program>(
    program: &'program Program,
    machine: &'program Machine,
    call: &Call,
) -> Option<&'program [StateParameter]> {
    let Some(receiver_path) = call.receiver.as_ref() else {
        return machine
            .states
            .iter()
            .find(|state| state.name == call.target)
            .map(|state| state.parameters.as_slice());
    };

    if receiver_path.as_slice() == ["self"] {
        return machine
            .states
            .iter()
            .find(|state| state.name == call.target)
            .map(|state| state.parameters.as_slice());
    }

    let receiver = receiver_path
        .as_slice()
        .last()
        .map(|member| member.as_str())?;
    let receiver_type = machine
        .contains
        .iter()
        .find(|contained| contained.name == receiver)
        .map(|contained| contained.type_name.as_str());

    if let Some(parameters) = receiver_type
        .and_then(|type_name| platform_state_parameters(program, type_name, &call.target))
    {
        return Some(parameters);
    }

    receiver_type
        .and_then(|type_name| machine_state_parameters(program, type_name, &call.target))
        .or_else(|| machine_state_parameters(program, receiver, &call.target))
}

fn display_name_path(path: &omega_typed_trees::expression::NamePath) -> String {
    path.as_slice()
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn platform_state_parameters<'program>(
    program: &'program Program,
    platform_name: &str,
    state_name: &str,
) -> Option<&'program [StateParameter]> {
    program
        .platforms
        .iter()
        .find(|platform| platform.name == platform_name)?
        .states
        .iter()
        .find(|state| state.name == state_name)
        .map(|state| state.parameters.as_slice())
}

fn machine_state_parameters<'program>(
    program: &'program Program,
    machine_name: &str,
    state_name: &str,
) -> Option<&'program [StateParameter]> {
    program
        .machines
        .iter()
        .find(|machine| machine.name == machine_name)?
        .states
        .iter()
        .find(|state| state.name == state_name)
        .map(|state| state.parameters.as_slice())
}

fn transition_target_state_and_arguments<'program>(
    program: &'program Program,
    machine: &'program Machine,
    state: &'program State,
    target: &'program TransitionTarget,
) -> Option<(&'program State, &'program [Expression])> {
    let TransitionTarget::Named { path, arguments } = target else {
        return None;
    };

    let target_state = match path.as_slice() {
        [state_name] => machine
            .states
            .iter()
            .find(|candidate| candidate.name == *state_name),
        [receiver, state_name] if receiver == "self" => machine
            .states
            .iter()
            .find(|candidate| candidate.name == *state_name),
        [receiver, state_name] => {
            contained_machine(program, machine, receiver).and_then(|target_machine| {
                target_machine
                    .states
                    .iter()
                    .find(|candidate| candidate.name == *state_name)
            })
        }
        _ => None,
    };

    target_state
        .or_else(|| {
            if path.as_slice() == ["self"] {
                Some(state)
            } else {
                None
            }
        })
        .map(|target_state| (target_state, arguments.as_slice()))
}

fn contained_machine<'program>(
    program: &'program Program,
    machine: &Machine,
    receiver: &str,
) -> Option<&'program Machine> {
    let contained = machine
        .contains
        .iter()
        .find(|contained| contained.name == receiver)?;

    program
        .machines
        .iter()
        .find(|machine| machine.name == contained.type_name)
}

fn callable_parameters(state: &State) -> impl Iterator<Item = &StateParameter> {
    state
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
}

fn expression_constraints(
    program: &Program,
    machine: &Machine,
    state: &State,
    expression: &Expression,
) -> Vec<TypeConstraint> {
    match expression {
        Expression::Binary(binary) => {
            let left = expression_constraints(program, machine, state, &binary.left);
            let right = expression_constraints(program, machine, state, &binary.right);
            derived_binary_constraints(binary.operator, &left, &right)
        }
        Expression::Call(call) => {
            if let Some(constraints) =
                derived_builtin_call_constraints(program, machine, state, call)
            {
                return constraints;
            }

            if let Some(return_type) = call_expression_return_type(program, machine, state, call) {
                let mut constraints = collect_constraints(program, return_type);

                if is_real_from_call(call.receiver.as_deref(), &call.target)
                    && let [argument] = call.arguments.as_slice()
                {
                    let argument_constraints =
                        expression_constraints(program, machine, state, argument);
                    constraints.extend(derived_real_from_constraints(&argument_constraints));
                }

                return constraints;
            }

            Vec::new()
        }
        Expression::Cast(cast) => expression_constraints(program, machine, state, &cast.value),
        Expression::Float(value) => float_literal_constraints(*value),
        Expression::Integer(value) => integer_literal_constraints(*value),
        Expression::Member(_) | Expression::Mutable(_) | Expression::Name(_) => {
            expression_type_reference(program, machine, state, expression)
                .map(|type_reference| collect_constraints(program, type_reference))
                .unwrap_or_default()
        }
        Expression::ArrayLiteral(_)
        | Expression::Boolean(_)
        | Expression::Indexed(_)
        | Expression::String(_)
        | Expression::StructLiteral(_) => Vec::new(),
    }
}

fn expression_type_reference<'program>(
    program: &'program Program,
    machine: &'program Machine,
    state: &'program State,
    expression: &Expression,
) -> Option<&'program TypeReference> {
    match expression {
        Expression::Mutable(inner) => expression_type_reference(program, machine, state, inner),
        Expression::Name(path) => {
            if path.symbol().is_valid() {
                return type_reference_for_symbol(program, machine, state, path.symbol());
            }

            let name = match path.as_slice() {
                [name] => name,
                [receiver, name] if receiver == "self" => name,
                _ => return None,
            };

            state
                .parameters
                .iter()
                .find(|parameter| parameter.name == *name)
                .map(|parameter| &parameter.type_reference)
                .or_else(|| {
                    state.statements.iter().find_map(|statement| {
                        let omega_typed_trees::statement::Statement::LocalData(local_data) =
                            statement
                        else {
                            return None;
                        };

                        (local_data.name == *name).then_some(&local_data.type_reference)
                    })
                })
                .or_else(|| {
                    machine
                        .owned_data
                        .iter()
                        .find(|owned_data| owned_data.name == *name)
                        .map(|owned_data| &owned_data.type_reference)
                })
        }
        Expression::Member(member) => type_reference_for_symbol(
            program,
            machine,
            state,
            member.member_symbol,
        )
        .or_else(|| {
            expression_type_reference(program, machine, state, &member.receiver).and_then(
                |receiver_type| {
                    data_field_type_reference(
                        program,
                        receiver_type,
                        member.member_symbol,
                        &member.member,
                    )
                },
            )
        }),
        _ => None,
    }
}

fn collect_constraints(program: &Program, type_reference: &TypeReference) -> Vec<TypeConstraint> {
    match type_reference {
        TypeReference::Reference { referee, .. } => collect_constraints(program, referee),
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            let mut derived = collect_constraints(program, base_type);
            derived.extend(type_constraints(program, *constraints).iter().cloned());
            augment_constraints_with_named_facts(&mut derived);
            derived
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_constraints(program, element_type)
        }
        TypeReference::Generic { arguments, .. } => arguments
            .iter()
            .flat_map(|argument| collect_constraints(program, argument))
            .collect(),
        TypeReference::Slice { element_type } => collect_constraints(program, element_type),
        TypeReference::Named { name, .. } => primitive_constraints(name),
        TypeReference::Unit => Vec::new(),
    }
}

fn primitive_constraints(name: &ProgramName) -> Vec<TypeConstraint> {
    let mut constraints = match name.as_str() {
        "u32" => vec![TypeConstraint::Range {
            minimum: Expression::Integer(0),
            maximum: Expression::Integer(u32::MAX as i64),
        }],
        "usize" => vec![TypeConstraint::Range {
            minimum: Expression::Integer(0),
            maximum: Expression::Integer(i64::MAX),
        }],
        _ => Vec::new(),
    };
    augment_constraints_with_named_facts(&mut constraints);
    constraints
}

fn type_reference_for_symbol<'program>(
    program: &'program Program,
    machine: &'program Machine,
    state: &'program State,
    symbol: SymbolHandle,
) -> Option<&'program TypeReference> {
    state
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| &parameter.type_reference)
        .or_else(|| {
            state.statements.iter().find_map(|statement| {
                let omega_typed_trees::statement::Statement::LocalData(local_data) = statement
                else {
                    return None;
                };

                (local_data.symbol == symbol).then_some(&local_data.type_reference)
            })
        })
        .or_else(|| {
            machine
                .owned_data
                .iter()
                .find(|owned_data| owned_data.symbol == symbol)
                .map(|owned_data| &owned_data.type_reference)
        })
        .or_else(|| {
            program.data_definitions.iter().find_map(|data_definition| {
                data_definition.members.iter().find_map(|member| {
                    let omega_typed_trees::data::DataMember::Field(field) = member else {
                        return None;
                    };

                    (field.symbol == symbol).then_some(&field.type_reference)
                })
            })
        })
}

fn data_field_type_reference<'program>(
    program: &'program Program,
    type_reference: &'program TypeReference,
    member_symbol: SymbolHandle,
    member_name: &ProgramName,
) -> Option<&'program TypeReference> {
    match type_reference {
        TypeReference::Reference { referee, .. } => {
            data_field_type_reference(program, referee, member_symbol, member_name)
        }
        TypeReference::Constrained { base_type, .. } => {
            data_field_type_reference(program, base_type, member_symbol, member_name)
        }
        TypeReference::Generic {
            base_symbol,
            base_name,
            ..
        } => {
            data_definition_by_symbol_or_name(program, *base_symbol, base_name).and_then(
                |data_definition| data_field_in_definition(data_definition, member_symbol, member_name),
            )
        }
        TypeReference::Named { symbol, name } => {
            data_definition_by_symbol_or_name(program, *symbol, name).and_then(
                |data_definition| data_field_in_definition(data_definition, member_symbol, member_name),
            )
        }
        TypeReference::FixedArray { .. } | TypeReference::Slice { .. } | TypeReference::Unit => None,
    }
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program Program,
    symbol: SymbolHandle,
    name: &ProgramName,
) -> Option<&'program omega_typed_trees::data::DataDefinition> {
    program.data_definitions.iter().find(|data_definition| {
        (symbol.is_valid() && data_definition.symbol == symbol) || data_definition.name == *name
    })
}

fn data_field_in_definition<'program>(
    data_definition: &'program omega_typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &ProgramName,
) -> Option<&'program TypeReference> {
    data_definition.members.iter().find_map(|member| {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };

        ((member_symbol.is_valid() && field.symbol == member_symbol) || field.name == *member_name)
            .then_some(&field.type_reference)
    })
}

fn integer_literal_constraints(value: i64) -> Vec<TypeConstraint> {
    let mut constraints = vec![
        TypeConstraint::Named(ProgramName::generated("exact")),
        TypeConstraint::Range {
            minimum: Expression::Integer(value),
            maximum: Expression::Integer(value),
        },
    ];

    if value >= 0 {
        constraints.push(TypeConstraint::Named(ProgramName::generated("non_negative")));
    }

    if value > 0 {
        constraints.push(TypeConstraint::Named(ProgramName::generated("positive")));
    }

    constraints
}

fn float_literal_constraints(value: FloatLiteral) -> Vec<TypeConstraint> {
    let value = value.value();
    if !value.is_finite() {
        return Vec::new();
    }

    vec![
        TypeConstraint::Named(ProgramName::generated("finite")),
        TypeConstraint::Range {
            minimum: Expression::Float(FloatLiteral::new(value)),
            maximum: Expression::Float(FloatLiteral::new(value)),
        },
    ]
}

fn derived_binary_constraints(
    operator: BinaryOperator,
    left_constraints: &[TypeConstraint],
    right_constraints: &[TypeConstraint],
) -> Vec<TypeConstraint> {
    let mut constraints = Vec::new();

    if integer_constraints_are_exact(left_constraints)
        && integer_constraints_are_exact(right_constraints)
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract
        )
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("exact")));
    }

    if integer_constraints_are_wrapping(left_constraints)
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract
        )
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("wrapping")));
    }

    if let (Some(left_range), Some(right_range)) = (
        integer_range_from_constraints(left_constraints),
        integer_range_from_constraints(right_constraints),
    ) && let Some(range) = integer_binary_range(operator, left_range, right_range)
    {
        constraints.push(TypeConstraint::Range {
            minimum: Expression::Integer(range.minimum),
            maximum: Expression::Integer(range.maximum),
        });
        if range.minimum >= 0 {
            constraints.push(TypeConstraint::Named(ProgramName::generated("non_negative")));
        }
        if range.minimum > 0 {
            constraints.push(TypeConstraint::Named(ProgramName::generated("positive")));
        }
    }

    if let (Some(left_range), Some(right_range)) = (
        float_range_from_constraints(left_constraints),
        float_range_from_constraints(right_constraints),
    ) && let Some(range) = float_binary_range(operator, left_range, right_range)
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("finite")));
        constraints.push(TypeConstraint::Range {
            minimum: Expression::Float(FloatLiteral::new(range.minimum)),
            maximum: Expression::Float(FloatLiteral::new(range.maximum)),
        });
    }

    constraints
}

fn derived_real_from_constraints(argument_constraints: &[TypeConstraint]) -> Vec<TypeConstraint> {
    let Some(range) = integer_range_from_constraints(argument_constraints) else {
        return Vec::new();
    };

    vec![
        TypeConstraint::Named(ProgramName::generated("finite")),
        TypeConstraint::Range {
            minimum: Expression::Float(FloatLiteral::new(range.minimum as f64)),
            maximum: Expression::Float(FloatLiteral::new(range.maximum as f64)),
        },
    ]
}

fn derived_builtin_call_constraints(
    program: &Program,
    machine: &Machine,
    state: &State,
    call: &omega_typed_trees::expression::CallExpression,
) -> Option<Vec<TypeConstraint>> {
    match call.target.as_str() {
        "max" => derived_extrema_call_constraints(program, machine, state, call, true),
        "min" => derived_extrema_call_constraints(program, machine, state, call, false),
        "range" => derived_range_call_constraints(program, machine, state, call),
        _ => None,
    }
}

fn derived_extrema_call_constraints(
    program: &Program,
    machine: &Machine,
    state: &State,
    call: &omega_typed_trees::expression::CallExpression,
    is_max: bool,
) -> Option<Vec<TypeConstraint>> {
    let [left, right] = call.arguments.as_slice() else {
        return None;
    };

    let left_constraints = expression_constraints(program, machine, state, left);
    let right_constraints = expression_constraints(program, machine, state, right);
    let mut constraints = Vec::new();

    if integer_constraints_are_exact(&left_constraints)
        && integer_constraints_are_exact(&right_constraints)
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("exact")));
    }

    if let (Some(left_range), Some(right_range)) = (
        integer_range_from_constraints(&left_constraints),
        integer_range_from_constraints(&right_constraints),
    ) {
        let range = if is_max {
            IntegerRange {
                minimum: left_range.minimum.max(right_range.minimum),
                maximum: left_range.maximum.max(right_range.maximum),
            }
        } else {
            IntegerRange {
                minimum: left_range.minimum.min(right_range.minimum),
                maximum: left_range.maximum.min(right_range.maximum),
            }
        };

        constraints.push(TypeConstraint::Range {
            minimum: Expression::Integer(range.minimum),
            maximum: Expression::Integer(range.maximum),
        });
    }

    if constraints.is_empty() {
        return None;
    }

    augment_constraints_with_named_facts(&mut constraints);
    Some(constraints)
}

fn derived_range_call_constraints(
    program: &Program,
    machine: &Machine,
    state: &State,
    call: &omega_typed_trees::expression::CallExpression,
) -> Option<Vec<TypeConstraint>> {
    let [_, exclusive_max] = call.arguments.as_slice() else {
        return None;
    };

    let upper_constraints = expression_constraints(program, machine, state, exclusive_max);
    let mut constraints = vec![TypeConstraint::Named(ProgramName::generated("exact"))];

    if let Some(upper_range) = integer_range_from_constraints(&upper_constraints) {
        constraints.push(TypeConstraint::Range {
            minimum: Expression::Integer(0),
            maximum: Expression::Integer(upper_range.maximum),
        });
    }

    augment_constraints_with_named_facts(&mut constraints);
    Some(constraints)
}

fn call_expression_return_type<'program>(
    program: &'program Program,
    machine: &'program Machine,
    state: &'program State,
    call: &'program omega_typed_trees::expression::CallExpression,
) -> Option<&'program TypeReference> {
    let receiver_path = call.receiver.as_deref().and_then(expression_name_path);

    if receiver_path.is_none() || receiver_path.as_deref().is_some_and(|path| path == ["self"]) {
        return machine
            .states
            .iter()
            .find(|candidate| {
                (call.target_symbol.is_valid() && candidate.symbol == call.target_symbol)
                    || candidate.name == call.target
            })
            .and_then(|candidate| candidate.return_type.as_ref());
    }

    let receiver_path = receiver_path?;
    let receiver_symbol = receiver_symbol(call.receiver.as_deref())?;

    if let Some(contained) = machine.contains.iter().find(|contained| {
        contained.symbol == receiver_symbol
            || receiver_path
                .last()
                .is_some_and(|receiver_name| contained.name == *receiver_name)
    }) {
        if let Some(target_machine) = program
            .machines
            .iter()
            .find(|candidate| candidate.symbol == contained.type_symbol)
        {
            return target_machine
                .states
                .iter()
                .find(|candidate| {
                    (call.target_symbol.is_valid() && candidate.symbol == call.target_symbol)
                        || candidate.name == call.target
                })
                .and_then(|candidate| candidate.return_type.as_ref());
        }
    }

    if let Some(target_machine) = program
        .machines
        .iter()
        .find(|candidate| candidate.symbol == receiver_symbol)
    {
        return target_machine
            .states
            .iter()
            .find(|candidate| {
                (call.target_symbol.is_valid() && candidate.symbol == call.target_symbol)
                    || candidate.name == call.target
            })
            .and_then(|candidate| candidate.return_type.as_ref());
    }

    if let Some(target_platform) = program
        .platforms
        .iter()
        .find(|candidate| candidate.symbol == receiver_symbol)
    {
        return target_platform
            .states
            .iter()
            .find(|candidate| {
                (call.target_symbol.is_valid() && candidate.symbol == call.target_symbol)
                    || candidate.name == call.target
            })
            .and_then(|candidate| candidate.return_type.as_ref());
    }

    if let Some(parameter_machine_symbol) = state
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .and_then(|parameter| machine_symbol_from_type_reference(&parameter.type_reference))
        && let Some(target_machine) = program
            .machines
            .iter()
            .find(|candidate| candidate.symbol == parameter_machine_symbol)
    {
        return target_machine
            .states
            .iter()
            .find(|candidate| {
                (call.target_symbol.is_valid() && candidate.symbol == call.target_symbol)
                    || candidate.name == call.target
            })
            .and_then(|candidate| candidate.return_type.as_ref());
    }

    None
}

fn is_real_from_call(receiver: Option<&Expression>, target: &ProgramName) -> bool {
    target == "from"
        && matches!(
            receiver,
            Some(Expression::Name(path)) if path.as_slice() == ["Real"]
        )
}

fn expression_name_path(expression: &Expression) -> Option<Vec<ProgramName>> {
    expression_name_path_owned(expression)
}

fn receiver_symbol(receiver: Option<&Expression>) -> Option<SymbolHandle> {
    match receiver? {
        Expression::Name(path) => Some(path.symbol()),
        Expression::Member(member) => Some(member.member_symbol),
        Expression::Mutable(inner) => receiver_symbol(Some(inner)),
        _ => None,
    }
}

fn expression_name_path_owned(expression: &Expression) -> Option<Vec<ProgramName>> {
    match expression {
        Expression::Name(path) => Some(path.as_slice().to_vec()),
        Expression::Member(member) => {
            let mut path = expression_name_path_owned(&member.receiver)?;
            path.push(member.member.clone());
            Some(path)
        }
        Expression::Mutable(inner) => expression_name_path_owned(inner),
        _ => None,
    }
}

fn machine_symbol_from_type_reference(type_reference: &TypeReference) -> Option<SymbolHandle> {
    match type_reference {
        TypeReference::Reference { referee, .. } => machine_symbol_from_type_reference(referee),
        TypeReference::Constrained { base_type, .. } => {
            machine_symbol_from_type_reference(base_type)
        }
        TypeReference::Generic { base_symbol, .. } | TypeReference::Named { symbol: base_symbol, .. } => {
            base_symbol.is_valid().then_some(*base_symbol)
        }
        TypeReference::FixedArray { .. } | TypeReference::Slice { .. } | TypeReference::Unit => None,
    }
}

fn augment_constraints_with_named_facts(constraints: &mut Vec<TypeConstraint>) {
    if constraints.iter().any(|constraint| {
        matches!(constraint, TypeConstraint::Range { minimum, maximum } if minimum == maximum)
    }) && !has_named_constraint(constraints, "exact")
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("exact")));
    }

    if let Some(range) = integer_range_from_constraints(constraints) {
        if range.minimum >= 0 && !has_named_constraint(constraints, "non_negative") {
            constraints.push(TypeConstraint::Named(ProgramName::generated("non_negative")));
        }
        if range.minimum > 0 && !has_named_constraint(constraints, "positive") {
            constraints.push(TypeConstraint::Named(ProgramName::generated("positive")));
        }
    }

    if float_range_from_constraints(constraints).is_some() && !has_named_constraint(constraints, "finite") {
        constraints.push(TypeConstraint::Named(ProgramName::generated("finite")));
    }
}

fn integer_constraints_are_exact(constraints: &[TypeConstraint]) -> bool {
    has_named_constraint(constraints, "exact") || integer_range_from_constraints(constraints).is_some()
}

fn integer_constraints_are_wrapping(constraints: &[TypeConstraint]) -> bool {
    has_named_constraint(constraints, "wrapping")
}

fn has_named_constraint(constraints: &[TypeConstraint], name: &str) -> bool {
    constraints.iter().any(|constraint| {
        matches!(constraint, TypeConstraint::Named(constraint_name) if constraint_name == name)
    })
}

fn integer_range_from_constraints(constraints: &[TypeConstraint]) -> Option<IntegerRange> {
    let mut range = constraints.iter().find_map(|constraint| {
        let TypeConstraint::Range { minimum, maximum } = constraint else {
            return None;
        };

        Some(IntegerRange {
            minimum: integer_constant_value(minimum)?,
            maximum: integer_constant_value(maximum)?,
        })
    });

    for constraint in constraints {
        let TypeConstraint::Named(name) = constraint else {
            continue;
        };

        let implied = match name.as_str() {
            "non_negative" => Some(IntegerRange {
                minimum: 0,
                maximum: i64::MAX,
            }),
            "positive" => Some(IntegerRange {
                minimum: 1,
                maximum: i64::MAX,
            }),
            _ => None,
        };

        let Some(implied) = implied else {
            continue;
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(implied.minimum),
                maximum: existing.maximum.min(implied.maximum),
            },
            None => implied,
        });
    }

    range
}

fn float_range_from_constraints(constraints: &[TypeConstraint]) -> Option<FloatRange> {
    constraints.iter().find_map(|constraint| {
        let TypeConstraint::Range { minimum, maximum } = constraint else {
            return None;
        };

        Some(FloatRange {
            minimum: float_constant_value(minimum)?,
            maximum: float_constant_value(maximum)?,
        })
    })
}

fn integer_binary_range(
    operator: BinaryOperator,
    left: IntegerRange,
    right: IntegerRange,
) -> Option<IntegerRange> {
    match operator {
        BinaryOperator::Add => Some(IntegerRange {
            minimum: left.minimum.saturating_add(right.minimum),
            maximum: left.maximum.saturating_add(right.maximum),
        }),
        BinaryOperator::Subtract => Some(IntegerRange {
            minimum: left.minimum.saturating_sub(right.maximum),
            maximum: left.maximum.saturating_sub(right.minimum),
        }),
        BinaryOperator::Multiply => {
            let products = [
                left.minimum.saturating_mul(right.minimum),
                left.minimum.saturating_mul(right.maximum),
                left.maximum.saturating_mul(right.minimum),
                left.maximum.saturating_mul(right.maximum),
            ];
            Some(IntegerRange {
                minimum: *products.iter().min()?,
                maximum: *products.iter().max()?,
            })
        }
        BinaryOperator::Modulo => {
            if right.minimum <= 0 {
                return None;
            }

            Some(IntegerRange {
                minimum: 0,
                maximum: right.maximum.saturating_sub(1),
            })
        }
        BinaryOperator::ShiftRight => {
            if right.minimum < 0 {
                return None;
            }

            Some(IntegerRange {
                minimum: 0.max(left.minimum),
                maximum: left.maximum.max(0),
            })
        }
        BinaryOperator::And
        | BinaryOperator::Divide
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft => None,
    }
}

fn float_binary_range(
    operator: BinaryOperator,
    left: FloatRange,
    right: FloatRange,
) -> Option<FloatRange> {
    match operator {
        BinaryOperator::Add => Some(FloatRange {
            minimum: left.minimum + right.minimum,
            maximum: left.maximum + right.maximum,
        }),
        BinaryOperator::Subtract => Some(FloatRange {
            minimum: left.minimum - right.maximum,
            maximum: left.maximum - right.minimum,
        }),
        BinaryOperator::Multiply => {
            let products = [
                left.minimum * right.minimum,
                left.minimum * right.maximum,
                left.maximum * right.minimum,
                left.maximum * right.maximum,
            ];
            Some(FloatRange {
                minimum: products.iter().copied().fold(f64::INFINITY, f64::min),
                maximum: products.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            })
        }
        BinaryOperator::Divide => {
            if right.minimum <= 0.0 && right.maximum >= 0.0 {
                return None;
            }

            let quotients = [
                left.minimum / right.minimum,
                left.minimum / right.maximum,
                left.maximum / right.minimum,
                left.maximum / right.maximum,
            ];
            Some(FloatRange {
                minimum: quotients.iter().copied().fold(f64::INFINITY, f64::min),
                maximum: quotients
                    .iter()
                    .copied()
                    .fold(f64::NEG_INFINITY, f64::max),
            })
        }
        BinaryOperator::And
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Modulo
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => None,
    }
}

fn integer_constant_value(expression: &Expression) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        Expression::Name(path) if path.as_slice() == ["u32", "MAX"] => Some(u32::MAX as i64),
        _ => None,
    }
}

fn float_constant_value(expression: &Expression) -> Option<f64> {
    match expression {
        Expression::Float(value) => Some(value.value()),
        Expression::Integer(value) => Some(*value as f64),
        Expression::Name(path) if path.as_slice() == ["u32", "MAX"] => Some(u32::MAX as f64),
        _ => None,
    }
}

fn type_constraints(
    program: &Program,
    constraints: omega_core::arena::HandleSpan<TypeConstraint>,
) -> &[TypeConstraint] {
    program.type_constraints.span(constraints).unwrap_or(&[])
}
