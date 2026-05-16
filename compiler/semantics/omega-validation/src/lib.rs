mod symbols;

use crate::symbols::{MachineSymbols, ProgramSymbols};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataMember, DataShapeKind};
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::signature::StateParameter;
use omega_typed_trees::statement::{
    StatementNode, TableCall, TransitionTargetHandle, TransitionTargetNode,
};
use omega_typed_trees::types::{
    PrimitiveType, TypeConstraint, TypeConstraintNode, TypeReference, TypeReferenceHandle,
    TypeReferenceNode,
};

pub fn validate_program(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = ProgramSymbols::build(program, &mut diagnostics);

    validate_invariant_definitions(program, &mut diagnostics);
    validate_callable_state_signatures(program, &symbols, &mut diagnostics);
    validate_data_field_types(program, &symbols, &mut diagnostics);
    validate_entry_point(program, &mut diagnostics);

    for machine in program.machines() {
        let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);

        validate_contained_types(program, machine, &symbols, &mut diagnostics);
        validate_owned_data(program, machine, &symbols, &mut diagnostics);

        for state in program.machine_states(machine) {
            validate_local_data_names(
                program.statement_table.statements(state.statement_nodes),
                &machine_symbols,
                program.state_parameters(state),
                format!("machine `{}` state `{}`", machine.name, state.name),
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

fn validate_invariant_definitions(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for invariant in program.invariant_definitions() {
        let Some(constraints) = program.type_constraints.span(invariant.constraints) else {
            diagnostics.push(Diagnostic::error(format!(
                "invariant `{}` references invalid constraint storage",
                invariant.name
            )));
            continue;
        };

        for constraint in constraints {
            match constraint {
                TypeConstraint::Named(_) => {}
                TypeConstraint::Range { .. } => {}
            }
        }
    }
}

struct WritableRoots<'program, 'state> {
    machine_symbols: &'state MachineSymbols<'program>,
    statements: &'state [StatementNode],
    parameters: &'state [StateParameter],
}

impl WritableRoots<'_, '_> {
    fn contains(&self, root_name: &str) -> bool {
        self.machine_symbols.has_owned_data(root_name)
            || self.statements.iter().any(|statement| {
                let StatementNode::LocalData(local_data) = statement else {
                    return false;
                };

                local_data.name == root_name
            })
            || self
                .parameters
                .iter()
                .any(|parameter| parameter.is_mutable && parameter.name == root_name)
    }
}

fn validate_local_data_names(
    statements: &[StatementNode],
    machine_symbols: &MachineSymbols<'_>,
    parameters: &[StateParameter],
    owner: String,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut local_names = Vec::new();

    for statement in statements {
        let StatementNode::LocalData(local_data) = statement else {
            continue;
        };

        if machine_symbols.has_member(local_data.name.as_str())
            || parameters
                .iter()
                .any(|parameter| parameter.name == local_data.name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} local data `{}` conflicts with an existing name",
                local_data.name
            )));
            continue;
        }

        if local_names.contains(&local_data.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} has duplicate local data `{}`",
                local_data.name
            )));
        }

        local_names.push(local_data.name.as_str());
    }
}

fn validate_state_statement_node(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &ProgramSymbols<'_>,
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
            format!("machine `{}` state `{state_name}` assignment", machine.name),
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

            let Some(return_type) = &state.return_type else {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` has a terminal expression but no return type",
                    machine.name
                )));
                return;
            };

            validate_expression_type_handle(
                program,
                *expression,
                return_type,
                diagnostics,
                format!(
                    "machine `{}` state `{state_name}` terminal expression",
                    machine.name
                ),
            );
        }
        StatementNode::LocalData(local_data) => validate_type_reference_handle(
            program,
            local_data.type_reference,
            symbols,
            diagnostics,
            format!(
                "machine `{}` state `{state_name}` local data `{}`",
                machine.name, local_data.name
            ),
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
    owner: String,
) {
    if !is_mutable_place_handle(program, target) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} target must be a named place"
        )));
        return;
    }

    let Some(root_name) = expression_root_name_handle(program, target) else {
        return;
    };

    if !writable_roots.contains(root_name) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} cannot write `{root_name}` because it is not mutable in this state"
        )));
    }
}

fn validate_callable_state_signatures(
    program: &TypedTrees,
    symbols: &ProgramSymbols<'_>,
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
                    return_type: state.return_type.as_ref(),
                }),
            program,
            symbols,
            diagnostics,
            format!("machine `{}`", machine.name),
        );
    }

    for platform in program.platforms() {
        let platform_states = program.platform_state_signatures(platform);
        validate_platform_state_names(platform, platform_states, diagnostics);
        validate_state_signature_types(
            platform_states.iter().map(|state| StateSignatureView {
                name: state.name.as_str(),
                parameters: program.state_signature_parameters(state),
                return_type: state.return_type.as_ref(),
            }),
            program,
            symbols,
            diagnostics,
            format!("platform `{}`", platform.name),
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct StateSignatureView<'program> {
    name: &'program str,
    parameters: &'program [StateParameter],
    return_type: Option<&'program TypeReference>,
}

fn validate_state_signature_types<'program>(
    signatures: impl Iterator<Item = StateSignatureView<'program>>,
    program: &TypedTrees,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    for signature in signatures {
        validate_state_parameter_names(signature, &owner, diagnostics);

        for parameter in signature.parameters {
            if parameter.is_self {
                continue;
            }

            validate_type_reference_handle(
                program,
                parameter.type_reference,
                symbols,
                diagnostics,
                format!(
                    "{owner} state `{}` parameter `{}`",
                    signature.name, parameter.name
                ),
            );
        }

        if let Some(return_type) = signature.return_type {
            validate_type_reference(
                program,
                return_type,
                symbols,
                diagnostics,
                format!("{owner} state `{}` return type", signature.name),
            );
        }
    }
}

fn validate_platform_state_names(
    platform: &omega_typed_trees::platform::Platform,
    platform_states: &[omega_typed_trees::signature::StateSignature],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut state_names = Vec::new();

    for state in platform_states {
        if state_names.contains(&state.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has duplicate state `{}`",
                platform.name, state.name
            )));
        }

        state_names.push(state.name.as_str());
    }
}

fn validate_state_parameter_names(
    state: StateSignatureView<'_>,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut parameter_names = Vec::new();

    for parameter in state.parameters {
        if parameter_names.contains(&parameter.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} state `{}` has duplicate parameter `{}`",
                state.name, parameter.name
            )));
        }

        parameter_names.push(parameter.name.as_str());
    }
}

fn validate_data_field_types(
    program: &TypedTrees,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in program.data_definitions() {
        let data_members = program.data_members(data_definition);
        validate_data_member_names(data_definition, data_members, diagnostics);
        validate_data_shape(data_definition, data_members, diagnostics);

        for member in data_members {
            let DataMember::Field(field) = member else {
                continue;
            };

            validate_type_reference(
                program,
                &field.type_reference,
                symbols,
                diagnostics,
                format!("data `{}` field `{}`", data_definition.name, field.name),
            );
        }
    }
}

fn validate_data_shape(
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match omega_typed_trees::data::DataDefinition::shape_kind_from_members(data_members) {
        DataShapeKind::Empty => {}
        DataShapeKind::Mixed => diagnostics.push(Diagnostic::error(format!(
            "data `{}` mixes fields and variants; split record data from enum-like data",
            data_definition.name
        ))),
        DataShapeKind::Enum | DataShapeKind::Record => {}
    }
}

fn validate_data_member_names(
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut member_names = Vec::new();

    for member in data_members {
        let member_name = match member {
            DataMember::Field(field) => field.name.as_str(),
            DataMember::Variant(variant) => variant.name.as_str(),
        };

        if member_names.contains(&member_name) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` has duplicate member `{member_name}`",
                data_definition.name
            )));
        }

        member_names.push(member_name);
    }
}

fn validate_type_reference(
    program: &TypedTrees,
    type_reference: &TypeReference,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    match type_reference {
        TypeReference::Reference { referee, .. } => {
            validate_type_reference(program, referee, symbols, diagnostics, owner);
        }
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            validate_type_reference(program, base_type, symbols, diagnostics, owner.clone());
            validate_type_constraints(program, base_type, *constraints, diagnostics, owner);
        }
        TypeReference::FixedArray { element_type, .. } => {
            validate_type_reference(program, element_type, symbols, diagnostics, owner);
        }
        TypeReference::Slice { element_type } => {
            validate_type_reference(program, element_type, symbols, diagnostics, owner);
        }
        TypeReference::Generic { base_name, .. } => {
            if !symbols.has_type(base_name) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown generic type `{base_name}`"
                )));
            }

            if base_name != "IndexOf" {
                for argument in program.type_reference_arguments(type_reference) {
                    validate_type_reference(
                        program,
                        argument,
                        symbols,
                        diagnostics,
                        format!("{owner} generic argument"),
                    );
                }
            }
        }
        TypeReference::Named { name, .. } => {
            if !symbols.has_type(name) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown data type `{name}`"
                )));
            }
        }
        TypeReference::Unit => {}
    }
}

fn validate_type_reference_handle(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            validate_type_reference_handle(program, *referee, symbols, diagnostics, owner);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            validate_type_reference_handle(
                program,
                *base_type,
                symbols,
                diagnostics,
                owner.clone(),
            );
            validate_type_constraints_node(program, *base_type, *constraints, diagnostics, owner);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            validate_type_reference_handle(program, *element_type, symbols, diagnostics, owner);
        }
        TypeReferenceNode::Slice { element_type } => {
            validate_type_reference_handle(program, *element_type, symbols, diagnostics, owner);
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

            if base_name != "IndexOf" {
                for argument in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    validate_type_reference_handle(
                        program,
                        *argument,
                        symbols,
                        diagnostics,
                        format!("{owner} generic argument"),
                    );
                }
            }
        }
        TypeReferenceNode::Named { name, .. } => {
            if !symbols.has_type(name) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown data type `{name}`"
                )));
            }
        }
        TypeReferenceNode::Unit => {}
    }
}

fn validate_type_constraints(
    program: &TypedTrees,
    base_type: &TypeReference,
    constraints: omega_core::arena::HandleSpan<TypeConstraint>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    let primitive_type = base_type.primitive_type();
    let Some(constraints) = program.type_constraints.span(constraints) else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} references invalid constraint storage"
        )));
        return;
    };

    for constraint in constraints {
        match constraint {
            TypeConstraint::Named(name) if name == "finite" => {
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
            TypeConstraint::Named(_) => {}
            TypeConstraint::Range { .. } => {
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

fn validate_type_constraints_node(
    program: &TypedTrees,
    base_type: TypeReferenceHandle,
    constraints: omega_core::arena::HandleSpan<TypeConstraintNode>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    let primitive_type = program.type_reference_table.primitive_type(base_type);

    for constraint in program.type_reference_table.constraints(constraints) {
        match constraint {
            TypeConstraintNode::Named(name) if name == "finite" => {
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

fn validate_entry_point(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let Some(main_machine) = program
        .machines()
        .iter()
        .find(|machine| machine.name == "main")
    else {
        diagnostics.push(Diagnostic::error("missing machine main"));
        return;
    };

    if !program
        .machine_states(main_machine)
        .iter()
        .any(|state| state.name == "entry")
    {
        diagnostics.push(Diagnostic::error("machine main is missing state entry"));
    }
}

#[cfg(test)]
mod tests {
    use super::validate_program;
    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn validates_main_entry_surface_from_source_pipeline() {
        let source = r#"
        machine main {
            pub entry() {}
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        assert_eq!(typed.machines().len(), 1);
        assert_eq!(typed.machines()[0].name.as_str(), "main");
        assert_eq!(typed.machine_states(&typed.machines()[0]).len(), 1);
        assert_eq!(
            typed.machine_states(&typed.machines()[0])[0].name.as_str(),
            "entry"
        );
        validate_program(&typed).expect("validation should succeed");
    }

    #[test]
    fn validates_local_state_call_arguments_from_source_pipeline() {
        let source = r#"
        machine main {
            pub entry(&mut self) {
                take_non_negative(0);
            }

            state take_non_negative(
                &mut self,
                value: u32[exact, non_negative]
            ) {
            }
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        let entry = typed
            .machine_states(&typed.machines()[0])
            .iter()
            .find(|state| state.name.as_str() == "entry")
            .expect("entry state");
        let call_argument_count = typed
            .statement_table
            .statements(entry.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                omega_typed_trees::statement::StatementNode::Call(call) => {
                    Some(call.arguments.len())
                }
                omega_typed_trees::statement::StatementNode::Expression(expression) => {
                    let omega_typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                    else {
                        return None;
                    };
                    Some(call.arguments.len())
                }
                _ => None,
            })
            .expect("expected call statement");
        assert_eq!(call_argument_count, 1);
        validate_program(&typed).expect("validation should succeed");
    }
}

fn validate_contained_types(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    symbols: &ProgramSymbols<'_>,
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
    machine: &omega_typed_trees::machine::Machine,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for owned_data in program.machine_owned_data(machine) {
        validate_type_reference_handle(
            program,
            owned_data.type_reference,
            symbols,
            diagnostics,
            format!(
                "machine `{}` owned data `{}`",
                machine.name, owned_data.name
            ),
        );

        if owned_data.initial_value.is_valid() {
            validate_initial_value_handle(
                program,
                owned_data.type_reference,
                owned_data.initial_value,
                diagnostics,
                format!(
                    "machine `{}` owned data `{}`",
                    machine.name, owned_data.name
                ),
            );
        }
    }
}

fn validate_initial_value_handle(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    initial_value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
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
    symbols: &ProgramSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver_members = program.statement_table.name_path_members(call.receiver);
    let arguments = program.statement_table.expression_handles(call.arguments);

    if receiver_members.is_empty() || receiver_members == ["self"] {
        let Some(state) = machine_symbols.state(&call.target) else {
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
    validate_argument_borrows_handles(program, arguments, target_name, writable_roots, diagnostics);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArgumentAccessKind {
    Mutable,
    Read,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArgumentAccess<'expression> {
    root_name: &'expression str,
    kind: ArgumentAccessKind,
}

fn validate_argument_borrows_handles(
    program: &TypedTrees,
    arguments: &[ExpressionHandle],
    target_name: &str,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut accesses = Vec::new();

    for argument in arguments {
        collect_argument_accesses_handle(
            program,
            *argument,
            target_name,
            writable_roots,
            &mut accesses,
            diagnostics,
        );
    }

    for (index, access) in accesses.iter().enumerate() {
        if access.kind != ArgumentAccessKind::Mutable {
            continue;
        }

        for other_access in accesses.iter().skip(index + 1) {
            if access.root_name != other_access.root_name {
                continue;
            }

            match other_access.kind {
                ArgumentAccessKind::Mutable => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as mutable more than once",
                    access.root_name
                ))),
                ArgumentAccessKind::Read => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as both mutable and read-only",
                    access.root_name
                ))),
            }
        }
    }
}

fn collect_argument_accesses_handle<'expression>(
    program: &'expression TypedTrees,
    expression: ExpressionHandle,
    target_name: &str,
    writable_roots: &WritableRoots<'_, '_>,
    accesses: &mut Vec<ArgumentAccess<'expression>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            if !is_mutable_place_handle(program, *inner_expression) {
                diagnostics.push(Diagnostic::error(format!(
                    "mutable argument for state `{target_name}` must be a named place"
                )));
                return;
            }

            if let Some(root_name) = expression_root_name_handle(program, *inner_expression) {
                if !writable_roots.contains(root_name) {
                    diagnostics.push(Diagnostic::error(format!(
                        "mutable argument `{root_name}` for state `{target_name}` is not writable in this state"
                    )));
                }

                accesses.push(ArgumentAccess {
                    root_name,
                    kind: ArgumentAccessKind::Mutable,
                });
            }
        }
        _ => collect_read_accesses_handle(program, expression, accesses),
    }
}

fn collect_read_accesses_handle<'expression>(
    program: &'expression TypedTrees,
    expression: ExpressionHandle,
    accesses: &mut Vec<ArgumentAccess<'expression>>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_read_accesses_handle(program, *value, accesses);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_read_accesses_handle(program, binary.left, accesses);
            collect_read_accesses_handle(program, binary.right, accesses);
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_read_accesses_handle(program, call.receiver, accesses);
            }

            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_read_accesses_handle(program, *argument, accesses);
            }
        }
        ExpressionNode::Cast(cast) => collect_read_accesses_handle(program, cast.value, accesses),
        ExpressionNode::Indexed(indexed) => {
            if let Some(root_name) = expression_root_name_handle(program, indexed.collection) {
                accesses.push(ArgumentAccess {
                    root_name,
                    kind: ArgumentAccessKind::Read,
                });
            }

            collect_read_accesses_handle(program, indexed.index, accesses);
        }
        ExpressionNode::Member(member) => {
            collect_read_accesses_handle(program, member.receiver, accesses)
        }
        ExpressionNode::Name(path) => {
            if let Some(root_name) = program
                .expression_table
                .name_path_members(path.members)
                .first()
            {
                accesses.push(ArgumentAccess {
                    root_name: root_name.as_str(),
                    kind: ArgumentAccessKind::Read,
                });
            }
        }
        ExpressionNode::Mutable(inner_expression) => {
            collect_read_accesses_handle(program, *inner_expression, accesses)
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_read_accesses_handle(program, field.value, accesses);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
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
        TypeReferenceNode::Generic { base_name, .. } if base_name == "IndexOf" => {
            matches!(
                argument_node,
                ExpressionNode::Integer(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
            )
        }
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

fn argument_matches_type_handle(
    program: &TypedTrees,
    argument: ExpressionHandle,
    type_reference: &TypeReference,
) -> bool {
    if let ExpressionNode::Mutable(inner_expression) = program.expression_table.expression(argument)
    {
        return argument_matches_type_handle(program, *inner_expression, type_reference);
    }

    let argument_node = program.expression_table.expression(argument);

    match type_reference {
        TypeReference::Reference { referee, .. } => {
            argument_matches_type_handle(program, argument, referee)
        }
        TypeReference::Constrained { base_type, .. } => {
            argument_matches_type_handle(program, argument, base_type)
        }
        TypeReference::FixedArray { .. } => matches!(
            argument_node,
            ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReference::Slice { .. } => matches!(
            argument_node,
            ExpressionNode::Call(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReference::Generic { base_name, .. } if base_name == "IndexOf" => {
            matches!(
                argument_node,
                ExpressionNode::Integer(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
            )
        }
        TypeReference::Generic { .. } => matches!(
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
        TypeReference::Named {
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
        TypeReference::Unit => false,
    }
}

fn validate_expression_type_handle(
    program: &TypedTrees,
    expression: ExpressionHandle,
    type_reference: &TypeReference,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    if !argument_matches_type_handle(program, expression, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} expects `{}`, got `{}`",
            type_reference.display_name_with_constraints(&program.type_constraints),
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
        ExpressionNode::StructLiteral(_) => "struct literal",
        ExpressionNode::String(_) => "String",
    }
}

fn validate_transition_target_node(
    program: &TypedTrees,
    target: TransitionTargetHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &ProgramSymbols<'_>,
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

    if path.len() == 2 && path[0] == "self" {
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
