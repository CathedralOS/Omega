mod data;
mod domains;
mod effects;
mod entry_point;
mod invariants;
mod locals;
mod operators;
mod proof_facts;
mod symbols;
#[cfg(test)]
mod tests;

use crate::data::validate_data_field_types;
use crate::domains::validate_domain_definitions;
use crate::entry_point::validate_entry_point;
use crate::invariants::validate_invariant_definitions;
use crate::locals::{WritableRoots, validate_local_data_names};
use crate::proof_facts::{ProofFactOwner, validate_proof_facts};
use crate::symbols::{MachineSymbols, TopLevelSymbols};
pub use effects::validate_effect_plan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::name::Identifier;
use omega_typed_trees::signature::{SignatureContract, StateParameter, StateSignature};
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableCall, TransitionTargetHandle, TransitionTargetNode,
};
use omega_typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::fmt;

pub fn validate_program(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    let fact_plan = omega_facts::build_definition_fact_plan(program);

    validate_domain_definitions(program, &symbols, &fact_plan, &mut diagnostics);
    validate_invariant_definitions(program, &fact_plan, &mut diagnostics);
    validate_callable_state_signatures(program, &symbols, &mut diagnostics);
    validate_trait_requirements(program, &mut diagnostics);
    validate_data_field_types(program, &symbols, &mut diagnostics);
    operators::validate_operator_declarations(program, &mut diagnostics);
    validate_entry_point(program, &mut diagnostics);

    for machine in program.machines() {
        let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);

        validate_contained_types(program, machine, &symbols, &mut diagnostics);
        validate_owned_data(program, machine, &symbols, &mut diagnostics);
        validate_machine_effects(program, machine, &mut diagnostics);
        validate_machine_contracts(program, machine, &mut diagnostics);
        validate_machine_trait_conformances(program, machine, &mut diagnostics);

        for state in program.machine_states(machine) {
            validate_local_data_names(
                program.statement_table.statements(state.statement_nodes),
                &machine_symbols,
                program.state_parameters(state),
                machine.name.as_str(),
                state.name.as_str(),
                &mut diagnostics,
            );
            let writable_roots = WritableRoots {
                machine_symbols: &machine_symbols,
                statements: program.statement_table.statements(state.statement_nodes),
                parameters: program.state_parameters(state),
            };

            for statement in program.statement_table.statements(state.statement_nodes) {
                validate_state_statement_node(
                    program,
                    machine,
                    &state.name,
                    &machine_symbols,
                    &symbols,
                    &writable_roots,
                    statement,
                    &mut diagnostics,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_state_statement_node(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::Assignment(assignment) => validate_assignment_target_handle(
            program,
            assignment.target,
            writable_roots,
            diagnostics,
            machine.name.as_str(),
            state_name,
        ),
        StatementNode::Call(call) => validate_call_node(
            program,
            call,
            machine,
            machine_symbols,
            symbols,
            writable_roots,
            diagnostics,
        ),
        StatementNode::Expression(expression) => {
            let Some(state) = machine_symbols.state(state_name) else {
                return;
            };

            if !state.return_type.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` has a terminal expression but no return type",
                    machine.name
                )));
                return;
            }

            validate_expression_type_handle(
                program,
                *expression,
                state.return_type,
                diagnostics,
                ExpressionTypeOwner::StateTerminalExpression {
                    machine: machine.name.as_str(),
                    state: state_name,
                },
            );
        }
        StatementNode::LocalData(local_data) => validate_type_reference_handle(
            program,
            local_data.type_reference,
            symbols,
            diagnostics,
            TypeReferenceOwner::StateLocalData {
                machine: machine.name.as_str(),
                state: state_name,
                local: local_data.name.as_str(),
                generic_depth: 0,
            },
        ),
        StatementNode::Transition(transition) => {
            validate_transition_target_node(
                program,
                transition.target,
                machine_symbols,
                symbols,
                writable_roots,
                diagnostics,
            );

            if transition.continuation.is_valid() {
                validate_transition_target_node(
                    program,
                    transition.continuation,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_assignment_target_handle(
    program: &TypedTrees,
    target: ExpressionHandle,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
    machine_name: &str,
    state_name: &str,
) {
    if !is_mutable_place_handle(program, target) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment target must be a named place"
        )));
        return;
    }

    let Some(root_name) = expression_root_name_handle(program, target) else {
        return;
    };

    if !writable_roots.contains(root_name) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment cannot write `{root_name}` because it is not mutable in this state"
        )));
    }
}

fn validate_callable_state_signatures(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        validate_state_signature_types(
            program
                .machine_states(machine)
                .iter()
                .map(|state| StateSignatureView {
                    name: state.name.as_str(),
                    parameters: program.state_parameters(state),
                    return_type: state.return_type,
                    effects: &[],
                    contracts: &[],
                }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::Machine(machine.name.as_str()),
        );
    }

    for platform in program.platforms() {
        let platform_states = program.platform_state_signatures(platform);
        validate_platform_state_names(platform, platform_states, diagnostics);
        validate_state_signature_types(
            platform_states.iter().map(|state| StateSignatureView {
                name: state.name.as_str(),
                parameters: program.state_signature_parameters(state),
                return_type: state.return_type,
                effects: program.state_signature_effects(state),
                contracts: program.state_signature_contracts(state),
            }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::Platform(platform.name.as_str()),
        );
    }

    for trait_definition in program.traits() {
        validate_state_signature_types(
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .map(|machine| StateSignatureView {
                    name: machine.name.as_str(),
                    parameters: program.state_signature_parameters(machine),
                    return_type: machine.return_type,
                    effects: program.state_signature_effects(machine),
                    contracts: program.state_signature_contracts(machine),
                }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::Trait(trait_definition.name.as_str()),
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct StateSignatureView<'program> {
    name: &'program str,
    parameters: &'program [StateParameter],
    return_type: TypeReferenceHandle,
    effects: &'program [Identifier],
    contracts: &'program [SignatureContract],
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StateSignatureOwner<'program> {
    Machine(&'program str),
    Platform(&'program str),
    Trait(&'program str),
}

impl fmt::Display for StateSignatureOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Machine(machine) => write!(formatter, "machine `{machine}`"),
            Self::Platform(platform) => write!(formatter, "platform `{platform}`"),
            Self::Trait(trait_definition) => write!(formatter, "trait `{trait_definition}`"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum TypeReferenceOwner<'program> {
    DomainTarget {
        domain: &'program str,
        generic_depth: usize,
    },
    DataField {
        data: &'program str,
        field: &'program str,
        generic_depth: usize,
    },
    MachineOwnedData {
        machine: &'program str,
        data: &'program str,
        generic_depth: usize,
    },
    StateLocalData {
        machine: &'program str,
        state: &'program str,
        local: &'program str,
        generic_depth: usize,
    },
    StateParameter {
        owner: StateSignatureOwner<'program>,
        state: &'program str,
        parameter: &'program str,
        generic_depth: usize,
    },
    StateReturn {
        owner: StateSignatureOwner<'program>,
        state: &'program str,
        generic_depth: usize,
    },
}

#[derive(Debug, Clone, Copy)]
enum InitialValueOwner<'program> {
    MachineOwnedData {
        machine: &'program str,
        data: &'program str,
    },
}

impl fmt::Display for InitialValueOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MachineOwnedData { machine, data } => {
                write!(formatter, "machine `{machine}` owned data `{data}`")
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ExpressionTypeOwner<'program> {
    StateTerminalExpression {
        machine: &'program str,
        state: &'program str,
    },
}

impl fmt::Display for ExpressionTypeOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateTerminalExpression { machine, state } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` terminal expression"
                )
            }
        }
    }
}

impl TypeReferenceOwner<'_> {
    fn generic_argument(self) -> Self {
        match self {
            Self::DomainTarget {
                domain,
                generic_depth,
            } => Self::DomainTarget {
                domain,
                generic_depth: generic_depth + 1,
            },
            Self::DataField {
                data,
                field,
                generic_depth,
            } => Self::DataField {
                data,
                field,
                generic_depth: generic_depth + 1,
            },
            Self::MachineOwnedData {
                machine,
                data,
                generic_depth,
            } => Self::MachineOwnedData {
                machine,
                data,
                generic_depth: generic_depth + 1,
            },
            Self::StateLocalData {
                machine,
                state,
                local,
                generic_depth,
            } => Self::StateLocalData {
                machine,
                state,
                local,
                generic_depth: generic_depth + 1,
            },
            Self::StateParameter {
                owner,
                state,
                parameter,
                generic_depth,
            } => Self::StateParameter {
                owner,
                state,
                parameter,
                generic_depth: generic_depth + 1,
            },
            Self::StateReturn {
                owner,
                state,
                generic_depth,
            } => Self::StateReturn {
                owner,
                state,
                generic_depth: generic_depth + 1,
            },
        }
    }
}

impl fmt::Display for TypeReferenceOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let generic_depth = match self {
            Self::DomainTarget {
                domain,
                generic_depth,
            } => {
                write!(formatter, "domain `{domain}` target type")?;
                *generic_depth
            }
            Self::DataField {
                data,
                field,
                generic_depth,
            } => {
                write!(formatter, "data `{data}` field `{field}`")?;
                *generic_depth
            }
            Self::MachineOwnedData {
                machine,
                data,
                generic_depth,
            } => {
                write!(formatter, "machine `{machine}` owned data `{data}`")?;
                *generic_depth
            }
            Self::StateLocalData {
                machine,
                state,
                local,
                generic_depth,
            } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` local data `{local}`"
                )?;
                *generic_depth
            }
            Self::StateParameter {
                owner,
                state,
                parameter,
                generic_depth,
            } => {
                write!(formatter, "{owner} state `{state}` parameter `{parameter}`")?;
                *generic_depth
            }
            Self::StateReturn {
                owner,
                state,
                generic_depth,
            } => {
                write!(formatter, "{owner} state `{state}` return type")?;
                *generic_depth
            }
        };

        for _ in 0..generic_depth {
            formatter.write_str(" generic argument")?;
        }

        Ok(())
    }
}

fn validate_state_signature_types<'program>(
    signatures: impl Iterator<Item = StateSignatureView<'program>>,
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: StateSignatureOwner<'program>,
) {
    for signature in signatures {
        validate_state_parameter_names(signature, owner, diagnostics);
        validate_state_signature_effects(signature, owner, diagnostics);
        validate_state_signature_contracts(program, signature, owner, diagnostics);

        for parameter in signature.parameters {
            if parameter.is_self {
                continue;
            }

            validate_type_reference_handle(
                program,
                parameter.type_reference,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateParameter {
                    owner,
                    state: signature.name,
                    parameter: parameter.name.as_str(),
                    generic_depth: 0,
                },
            );
        }

        if signature.return_type.is_valid() {
            validate_type_reference_handle(
                program,
                signature.return_type,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateReturn {
                    owner,
                    state: signature.name,
                    generic_depth: 0,
                },
            );
        }
    }
}

fn validate_state_signature_effects(
    signature: StateSignatureView<'_>,
    owner: StateSignatureOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for effect in signature.effects {
        if !omega_effects::is_standard_effect_name(effect.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} state `{}` declares unknown effect `{}`",
                signature.name, effect
            )));
        }
    }
}

fn validate_state_signature_contracts(
    program: &TypedTrees,
    signature: StateSignatureView<'_>,
    owner: StateSignatureOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in signature.contracts {
        validate_proof_facts(
            program,
            program.proof_facts.span_or_empty(contract.facts),
            diagnostics,
            ProofFactOwner::StateSignatureContract {
                owner,
                state: signature.name,
                kind: contract_kind_label(contract.kind),
            },
        );
    }
}

fn validate_machine_effects(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for effect in program.machine_effects(machine) {
        if !omega_effects::is_standard_effect_name(effect.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` declares unknown effect `{}`",
                machine.name, effect
            )));
        }
    }
}

fn validate_machine_contracts(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in program.machine_contracts(machine) {
        validate_proof_facts(
            program,
            program.proof_facts.span_or_empty(contract.facts),
            diagnostics,
            ProofFactOwner::MachineContract {
                machine: machine.name.as_str(),
                kind: contract_kind_label(contract.kind),
            },
        );
    }
}

fn contract_kind_label(kind: omega_typed_trees::signature::SignatureContractKind) -> &'static str {
    match kind {
        omega_typed_trees::signature::SignatureContractKind::Requires => "requires",
        omega_typed_trees::signature::SignatureContractKind::Ensures => "ensures",
        omega_typed_trees::signature::SignatureContractKind::Boundary => "boundary",
    }
}

fn validate_platform_state_names(
    platform: &omega_typed_trees::platform::Platform,
    platform_states: &[omega_typed_trees::signature::StateSignature],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (state_index, state) in platform_states.iter().enumerate() {
        if platform_states[..state_index]
            .iter()
            .any(|previous| previous.name == state.name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has duplicate state `{}`",
                platform.name, state.name
            )));
        }
    }
}

fn validate_state_parameter_names(
    state: StateSignatureView<'_>,
    owner: StateSignatureOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (parameter_index, parameter) in state.parameters.iter().enumerate() {
        if state.parameters[..parameter_index]
            .iter()
            .any(|previous| previous.name == parameter.name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} state `{}` has duplicate parameter `{}`",
                state.name, parameter.name
            )));
        }
    }
}

pub(crate) fn validate_type_reference_handle(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
) {
    validate_type_reference_handle_with_context(
        program,
        type_reference,
        symbols,
        diagnostics,
        owner,
        false,
    );
}

fn validate_type_reference_handle_with_context(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
    allow_bare_str: bool,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            validate_type_reference_handle_with_context(
                program,
                *referee,
                symbols,
                diagnostics,
                owner,
                type_reference_is_named_str(program, *referee),
            );
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            validate_type_reference_handle_with_context(
                program,
                *base_type,
                symbols,
                diagnostics,
                owner,
                allow_bare_str,
            );
            validate_type_constraints_node(program, *base_type, *constraints, diagnostics, owner);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            validate_type_reference_handle_with_context(
                program,
                *element_type,
                symbols,
                diagnostics,
                owner,
                false,
            );
        }
        TypeReferenceNode::Slice { element_type } => {
            validate_type_reference_handle_with_context(
                program,
                *element_type,
                symbols,
                diagnostics,
                owner,
                false,
            );
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            if !symbols.has_type(base_name) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown generic type `{base_name}`"
                )));
            }

            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                validate_type_reference_handle(
                    program,
                    *argument,
                    symbols,
                    diagnostics,
                    owner.generic_argument(),
                );
            }
        }
        TypeReferenceNode::Named { name, .. } => {
            if name.as_str() == "str" && !allow_bare_str {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses unsized text view type `str` by value; use `&str`"
                )));
                return;
            }

            if !symbols.has_type(name) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown data type `{name}`"
                )));
            }
        }
        TypeReferenceNode::Unit => {}
    }
}

fn type_reference_is_named_str(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "str"
    )
}

fn validate_type_constraints_node(
    program: &TypedTrees,
    base_type: TypeReferenceHandle,
    constraints: omega_core::arena::HandleSpan<TypeConstraintNode>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
) {
    let primitive_type = program.type_reference_table.primitive_type(base_type);

    for constraint in program.type_reference_table.constraints(constraints) {
        match constraint {
            TypeConstraintNode::Named(name) if name.as_str() == "finite" => {
                let Some(primitive_type) = primitive_type else {
                    continue;
                };

                if !primitive_type.accepts_finite_constraint() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `finite` on `{}`, but `finite` is only valid on floats",
                        primitive_type.name()
                    )));
                }
            }
            TypeConstraintNode::Named(_) => {}
            TypeConstraintNode::Range { .. } => {
                let Some(primitive_type) = primitive_type else {
                    continue;
                };

                if !primitive_type.accepts_range_constraint() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `range` on `{}`, but `range` is only valid on numeric types",
                        primitive_type.name()
                    )));
                }
            }
        }
    }
}

fn validate_contained_types(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contained_object in program.machine_contained_objects(machine) {
        if !symbols.is_callable_receiver_type(&contained_object.type_name) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` contains `{}` with unknown type `{}`",
                machine.name, contained_object.name, contained_object.type_name
            )));
        }
    }
}

fn validate_owned_data(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for owned_data in program.machine_owned_data(machine) {
        validate_type_reference_handle(
            program,
            owned_data.type_reference,
            symbols,
            diagnostics,
            TypeReferenceOwner::MachineOwnedData {
                machine: machine.name.as_str(),
                data: owned_data.name.as_str(),
                generic_depth: 0,
            },
        );

        if owned_data.initial_value.is_valid() {
            validate_initial_value_handle(
                program,
                owned_data.type_reference,
                owned_data.initial_value,
                diagnostics,
                InitialValueOwner::MachineOwnedData {
                    machine: machine.name.as_str(),
                    data: owned_data.name.as_str(),
                },
            );
        }
    }
}

fn validate_machine_trait_conformances(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies unknown trait `{}`",
                machine.name, conformance.name
            )));
            continue;
        };

        let mut visited_traits = Vec::new();
        validate_machine_satisfies_trait(
            program,
            machine,
            trait_definition,
            diagnostics,
            &mut visited_traits,
        );
    }
}

fn validate_trait_requirements(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for trait_definition in program.traits() {
        for requirement in program.trait_requirements(trait_definition) {
            if trait_definition_by_symbol(program, requirement.symbol).is_none() {
                diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` requires unknown trait `{}`",
                    trait_definition.name, requirement.name
                )));
            }
        }
    }

    let mut reported_cycle_symbols = Vec::new();
    for trait_definition in program.traits() {
        let mut path = Vec::new();
        validate_trait_requirement_cycles(
            program,
            trait_definition,
            &mut path,
            &mut reported_cycle_symbols,
            diagnostics,
        );
    }
}

fn validate_trait_requirement_cycles(
    program: &TypedTrees,
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    path: &mut Vec<SymbolHandle>,
    reported_cycle_symbols: &mut Vec<SymbolHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if reported_cycle_symbols
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return;
    }

    if let Some(cycle_start) = path
        .iter()
        .position(|symbol| *symbol == trait_definition.symbol)
    {
        let cycle_symbols = path[cycle_start..]
            .iter()
            .copied()
            .chain(std::iter::once(trait_definition.symbol))
            .collect::<Vec<_>>();
        let mut cycle = path[cycle_start..]
            .iter()
            .filter_map(|symbol| trait_definition_by_symbol(program, *symbol))
            .map(|trait_definition| trait_definition.name.to_string())
            .collect::<Vec<_>>();
        cycle.push(trait_definition.name.to_string());

        diagnostics.push(Diagnostic::error(format!(
            "trait requirement cycle detected: {}",
            cycle.join(" -> ")
        )));
        reported_cycle_symbols.extend(cycle_symbols);
        return;
    }

    path.push(trait_definition.symbol);
    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };

        validate_trait_requirement_cycles(
            program,
            required_trait,
            path,
            reported_cycle_symbols,
            diagnostics,
        );
    }
    path.pop();
}

fn validate_machine_satisfies_trait(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    diagnostics: &mut Vec<Diagnostic>,
    visited_traits: &mut Vec<SymbolHandle>,
) {
    if visited_traits
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return;
    }

    visited_traits.push(trait_definition.symbol);

    for requirement in program.trait_machine_signatures(trait_definition) {
        let Some((state_machine, state)) = trait_requirement_state(program, machine, requirement)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies trait `{}` but is missing machine `{}`",
                machine.name, trait_definition.name, requirement.name
            )));
            continue;
        };

        validate_machine_state_satisfies_trait_signature(
            program,
            state_machine,
            state,
            trait_definition.name.as_str(),
            requirement,
            diagnostics,
        );
    }

    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };

        validate_machine_satisfies_trait(
            program,
            machine,
            required_trait,
            diagnostics,
            visited_traits,
        );
    }

    visited_traits.pop();
}

fn trait_requirement_state<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    requirement: &StateSignature,
) -> Option<(&'program Machine, &'program State)> {
    trait_conformance_candidate_machines(program, machine)
        .into_iter()
        .find_map(|candidate| {
            program
                .machine_states(candidate)
                .iter()
                .find(|state| state.name == requirement.name)
                .map(|state| (candidate, state))
        })
}

fn trait_conformance_candidate_machines<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
) -> Vec<&'program Machine> {
    let Some(attached_data) = machine.attached_data.as_ref() else {
        return vec![machine];
    };

    let mut candidates = Vec::new();
    candidates.push(machine);
    candidates.extend(program.machines().iter().filter(|candidate| {
        !std::ptr::eq(*candidate, machine)
            && candidate.attached_data.as_ref() == Some(attached_data)
    }));
    candidates
}

fn trait_definition_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::trait_definition::TraitDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == symbol)
}

fn validate_machine_state_satisfies_trait_signature(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let actual_parameters = program.state_parameters(state);
    let required_parameters = program.state_signature_parameters(requirement);
    if actual_parameters.len() != required_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: expected {} parameter(s), got {}",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            required_parameters.len(),
            actual_parameters.len()
        )));
        return;
    }

    for (index, (actual, required)) in actual_parameters
        .iter()
        .zip(required_parameters.iter())
        .enumerate()
    {
        validate_trait_parameter_match(
            program,
            machine,
            state,
            trait_name,
            requirement,
            index,
            actual,
            required,
            diagnostics,
        );
    }

    if !type_references_match(program, state.return_type, requirement.return_type) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: expected return `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            type_reference_label(program, requirement.return_type),
            type_reference_label(program, state.return_type)
        )));
    }

    validate_trait_effect_ceiling(
        program,
        machine,
        state,
        trait_name,
        requirement,
        diagnostics,
    );
}

fn validate_trait_effect_ceiling(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let allowed_effects = program.state_signature_effects(requirement);

    for effect in program.machine_effects(machine) {
        if !allowed_effects
            .iter()
            .any(|allowed| allowed.as_str() == effect.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: effect `{}` is not allowed by the trait requirement",
                machine.name,
                state.name,
                trait_name,
                requirement.name,
                effect
            )));
        }
    }
}

fn validate_trait_parameter_match(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    index: usize,
    actual: &StateParameter,
    required: &StateParameter,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if actual.is_self != required.is_self || actual.is_mutable != required.is_mutable {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter {}: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            index,
            parameter_shape_label(program, required),
            parameter_shape_label(program, actual)
        )));
        return;
    }

    if !type_references_match(program, actual.type_reference, required.type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter `{}`: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            required.name,
            type_reference_label(program, required.type_reference),
            type_reference_label(program, actual.type_reference)
        )));
    }
}

fn parameter_shape_label(program: &TypedTrees, parameter: &StateParameter) -> String {
    let qualifier = if parameter.is_mutable { "mut " } else { "" };
    if parameter.is_self {
        format!("&{qualifier}self")
    } else {
        format!(
            "{}: {}",
            parameter.name,
            type_reference_label(program, parameter.type_reference)
        )
    }
}

pub(crate) fn type_references_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }

    program.display_type_reference_with_constraints(actual)
        == program.display_type_reference_with_constraints(required)
}

pub(crate) fn type_reference_label(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> String {
    if type_reference.is_valid() {
        program.display_type_reference_with_constraints(type_reference)
    } else {
        "()".to_owned()
    }
}

fn validate_initial_value_handle(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    initial_value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: InitialValueOwner<'_>,
) {
    if !argument_matches_type_reference_handle(program, initial_value, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} initializer expects `{}`, got `{}`",
            program.display_type_reference_with_constraints(type_reference),
            expression_type_name_handle(program, initial_value)
        )));
    }
}

fn validate_call_node(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &omega_typed_trees::machine::Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver_members = program.statement_table.name_path_members(call.receiver);
    let arguments = program.statement_table.expression_handles(call.arguments);

    if receiver_members.is_empty()
        || matches!(receiver_members, [receiver] if receiver.as_str() == "self")
    {
        if let Some(state) = machine_symbols.state(&call.target) {
            validate_call_arguments_handles(
                program,
                arguments,
                state.name.as_str(),
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            return;
        }

        let Some((_, state)) = current_machine
            .attached_data
            .as_ref()
            .and_then(|attached_data| {
                symbols.attached_machine_state(
                    program,
                    attached_data.as_str(),
                    call.target.as_str(),
                )
            })
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local state `{}`",
                current_machine.name, call.target
            )));
            return;
        };

        validate_call_arguments_handles(
            program,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let receiver = receiver_members
        .last()
        .map(|member| member.as_str())
        .unwrap_or_default();
    let receiver_type = machine_symbols.contained_type(receiver);

    if let Some(platform) = receiver_type.and_then(|type_name| symbols.platform(type_name)) {
        let Some(state_signature) = program
            .platform_state_signatures(platform)
            .iter()
            .find(|state| state.name == call.target)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has no state `{}`",
                platform.name, call.target
            )));
            return;
        };

        validate_call_arguments_handles(
            program,
            arguments,
            &state_signature.name,
            program.state_signature_parameters(state_signature),
            writable_roots,
            diagnostics,
        );
        return;
    }

    if let Some(machine) = receiver_type
        .and_then(|type_name| symbols.machine(type_name))
        .or_else(|| symbols.machine(receiver))
    {
        if let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name == call.target)
        {
            validate_call_arguments_handles(
                program,
                arguments,
                &state.name,
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            return;
        };

        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` has no state `{}`",
            machine.name, call.target
        )));
        return;
    }

    if let Some((_, state)) = receiver_type.and_then(|type_name| {
        symbols.attached_machine_state(program, type_name, call.target.as_str())
    }) {
        validate_call_arguments_handles(
            program,
            arguments,
            &state.name,
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let _ = diagnostics;
}

fn validate_call_arguments_handles(
    program: &TypedTrees,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let callable_parameter_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();

    if arguments.len() != callable_parameter_count {
        diagnostics.push(Diagnostic::error(format!(
            "state `{}` expects {} argument(s), got {}",
            target_name,
            callable_parameter_count,
            arguments.len()
        )));
        return;
    }

    for (argument, parameter) in arguments
        .iter()
        .zip(parameters.iter().filter(|parameter| !parameter.is_self))
    {
        let is_mutable = matches!(
            program.expression_table.expression(*argument),
            ExpressionNode::Mutable(_)
        );

        if parameter.is_mutable && !is_mutable {
            continue;
        }

        if !parameter.is_mutable && is_mutable {
            continue;
        }

        let expected_type =
            program.display_type_reference_with_constraints(parameter.type_reference);

        if !argument_matches_type_reference_handle(program, *argument, parameter.type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` expects `{}`, got `{}`",
                parameter.name,
                target_name,
                expected_type,
                expression_type_name_handle(program, *argument)
            )));
        }
    }

    let _ = (writable_roots, diagnostics);
}
fn is_mutable_place_handle(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => is_mutable_place_handle(program, indexed.collection),
        ExpressionNode::Member(member) => is_mutable_place_handle(program, member.receiver),
        ExpressionNode::Name(_) => true,
        _ => false,
    }
}

fn expression_root_name_handle(program: &TypedTrees, expression: ExpressionHandle) -> Option<&str> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_name_handle(program, indexed.collection)
        }
        ExpressionNode::Member(member) => {
            match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path)
                    if path.members.count() == 1
                        && program
                            .expression_table
                            .name_path_members(path.members)
                            .first()
                            .is_some_and(|name| name.as_str() == "self") =>
                {
                    Some(member.member.as_str())
                }
                _ => expression_root_name_handle(program, member.receiver),
            }
        }
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .map(|name| name.as_str()),
        _ => None,
    }
}

fn argument_matches_type_reference_handle(
    program: &TypedTrees,
    argument: ExpressionHandle,
    type_reference: TypeReferenceHandle,
) -> bool {
    if let ExpressionNode::Mutable(inner_expression) = program.expression_table.expression(argument)
    {
        return argument_matches_type_reference_handle(program, *inner_expression, type_reference);
    }

    let argument_node = program.expression_table.expression(argument);

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            argument_matches_type_reference_handle(program, argument, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            argument_matches_type_reference_handle(program, argument, *base_type)
        }
        TypeReferenceNode::FixedArray { .. } => matches!(
            argument_node,
            ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Slice { .. } => matches!(
            argument_node,
            ExpressionNode::Call(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Generic { .. } => matches!(
            argument_node,
            ExpressionNode::Binary(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
                | ExpressionNode::StructLiteral(_)
        ),
        TypeReferenceNode::Named {
            name: type_name, ..
        } => {
            if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
                return matches!(argument_node, ExpressionNode::Boolean(_))
                    && primitive_type == PrimitiveType::Bool
                    || matches!(argument_node, ExpressionNode::String(_))
                        && primitive_type == PrimitiveType::String
                    || matches!(argument_node, ExpressionNode::Float(_))
                        && primitive_type.accepts_float_literal()
                    || matches!(argument_node, ExpressionNode::Integer(_))
                        && primitive_type.accepts_integer_literal()
                    || matches!(
                        argument_node,
                        ExpressionNode::Binary(_)
                            | ExpressionNode::Call(_)
                            | ExpressionNode::Cast(_)
                            | ExpressionNode::Indexed(_)
                            | ExpressionNode::Member(_)
                            | ExpressionNode::Name(_)
                            | ExpressionNode::StructLiteral(_)
                    );
            }

            matches!(
                argument_node,
                ExpressionNode::Binary(_)
                    | ExpressionNode::Call(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
                    | ExpressionNode::StructLiteral(_)
            )
        }
        TypeReferenceNode::Unit => false,
    }
}

fn validate_expression_type_handle(
    program: &TypedTrees,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: ExpressionTypeOwner<'_>,
) {
    if !argument_matches_type_reference_handle(program, expression, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} expects `{}`, got `{}`",
            program.display_type_reference_with_constraints(type_reference),
            expression_type_name_handle(program, expression)
        )));
    }
}

fn expression_type_name_handle(program: &TypedTrees, argument: ExpressionHandle) -> &'static str {
    match program.expression_table.expression(argument) {
        ExpressionNode::ArrayLiteral(_) => "array literal",
        ExpressionNode::Binary(_) => "binary expression",
        ExpressionNode::Boolean(_) => "bool",
        ExpressionNode::Call(_) => "call expression",
        ExpressionNode::Cast(_) => "cast expression",
        ExpressionNode::Float(_) => "float literal",
        ExpressionNode::Indexed(_) => "indexed value",
        ExpressionNode::Integer(_) => "integer literal",
        ExpressionNode::Member(_) => "member access",
        ExpressionNode::Mutable(inner_expression) => {
            expression_type_name_handle(program, *inner_expression)
        }
        ExpressionNode::Name(_) => "named value",
        ExpressionNode::Range(_) => "range expression",
        ExpressionNode::StructLiteral(_) => "struct literal",
        ExpressionNode::String(_) => "String",
    }
}

fn validate_transition_target_node(
    program: &TypedTrees,
    target: TransitionTargetHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TransitionTargetNode::Named { path, arguments } =
        program.statement_table.transition_target(target)
    else {
        return;
    };

    let path = program.statement_table.name_path_members(path.members);
    let arguments = program.statement_table.expression_handles(*arguments);

    if path.len() == 1 {
        let Some(state) = machine_symbols.state(path[0].as_str()) else {
            return;
        };

        validate_transition_arguments_handles(
            program,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );

        return;
    }

    if path.len() == 2 && path[0].as_str() == "self" {
        let Some(state) = machine_symbols.state(path[1].as_str()) else {
            return;
        };

        validate_transition_arguments_handles(
            program,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let Some(receiver_type) = machine_symbols.contained_type(path[0].as_str()) else {
        return;
    };

    if path.len() == 2 {
        let Some(machine) = symbols.machine(receiver_type) else {
            return;
        };

        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name == path[1])
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no state `{}`",
                machine.name, path[1]
            )));
            return;
        };

        validate_transition_arguments_handles(
            program,
            arguments,
            &state.name,
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
    }
}

fn validate_transition_arguments_handles(
    program: &TypedTrees,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_call_arguments_handles(
        program,
        arguments,
        target_name,
        parameters,
        writable_roots,
        diagnostics,
    );
}
