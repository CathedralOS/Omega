use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::data::{DataMember, DataShapeKind};
use crate::ir::expression::Expression;
use crate::ir::signature::StateParameter;
use crate::ir::statement::{Statement, TransitionTarget};
use crate::ir::types::{PrimitiveType, TypeConstraint, TypeReference};
use crate::semantic::symbols::{MachineSymbols, ProgramSymbols};

pub fn validate_program(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = ProgramSymbols::build(program, &mut diagnostics);

    validate_invariant_definitions(program, &mut diagnostics);
    validate_callable_state_signatures(program, &symbols, &mut diagnostics);
    validate_data_field_types(program, &symbols, &mut diagnostics);
    validate_entry_point(program, &mut diagnostics);

    for machine in &program.machines {
        let machine_symbols = MachineSymbols::build(machine, &mut diagnostics);

        validate_contained_types(machine, &symbols, &mut diagnostics);
        validate_owned_data(program, machine, &symbols, &mut diagnostics);

        for state in &machine.states {
            validate_local_data_names(
                &state.statements,
                &machine_symbols,
                &state.parameters,
                format!("machine `{}` state `{}`", machine.name, state.name),
                &mut diagnostics,
            );
            let writable_roots = WritableRoots {
                machine_symbols: &machine_symbols,
                statements: &state.statements,
                parameters: &state.parameters,
            };

            for statement in &state.statements {
                validate_state_statement(
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

fn validate_invariant_definitions(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    for invariant in &program.invariant_definitions {
        let Some(constraints) = program.type_constraints.span(invariant.constraints) else {
            diagnostics.push(Diagnostic::error(format!(
                "invariant `{}` references invalid constraint storage",
                invariant.name
            )));
            continue;
        };

        for constraint in constraints {
            match constraint {
                TypeConstraint::Named(name) if name == "finite" => {}
                TypeConstraint::Named(name) => diagnostics.push(Diagnostic::error(format!(
                    "invariant `{}` uses unknown type constraint `{name}`",
                    invariant.name
                ))),
                TypeConstraint::Range { .. } => {}
            }
        }
    }
}

struct WritableRoots<'program, 'state> {
    machine_symbols: &'state MachineSymbols<'program>,
    statements: &'state [Statement],
    parameters: &'state [StateParameter],
}

impl WritableRoots<'_, '_> {
    fn contains(&self, root_name: &str) -> bool {
        self.machine_symbols.has_owned_data(root_name)
            || self.statements.iter().any(|statement| {
                let Statement::LocalData(local_data) = statement else {
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
    statements: &[Statement],
    machine_symbols: &MachineSymbols<'_>,
    parameters: &[StateParameter],
    owner: String,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut local_names = Vec::new();

    for statement in statements {
        let Statement::LocalData(local_data) = statement else {
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

fn validate_state_statement(
    program: &Program,
    machine: &crate::ir::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &ProgramSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    statement: &Statement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Statement::Assignment(assignment) => validate_assignment_target(
            &assignment.target,
            writable_roots,
            diagnostics,
            format!("machine `{}` state `{state_name}` assignment", machine.name),
        ),
        Statement::Call(call) => validate_call(
            program,
            call,
            machine,
            machine_symbols,
            symbols,
            writable_roots,
            diagnostics,
        ),
        Statement::Expression(expression) => {
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

            validate_expression_type(
                program,
                expression,
                return_type,
                diagnostics,
                format!(
                    "machine `{}` state `{state_name}` terminal expression",
                    machine.name
                ),
            );
        }
        Statement::LocalData(local_data) => validate_type_reference(
            program,
            &local_data.type_reference,
            symbols,
            diagnostics,
            format!(
                "machine `{}` state `{state_name}` local data `{}`",
                machine.name, local_data.name
            ),
        ),
        Statement::Transition(transition) => {
            validate_transition_target(
                program,
                &transition.target,
                machine_symbols,
                symbols,
                writable_roots,
                diagnostics,
            );

            if let Some(continuation) = &transition.continuation {
                validate_transition_target(
                    program,
                    continuation,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_assignment_target(
    target: &Expression,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    if !is_mutable_place(target) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} target must be a named place"
        )));
        return;
    }

    let Some(root_name) = expression_root_name(target) else {
        return;
    };

    if !writable_roots.contains(root_name) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} cannot write `{root_name}` because it is not mutable in this state"
        )));
    }
}

fn validate_callable_state_signatures(
    program: &Program,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in &program.machines {
        validate_state_signature_types(
            machine.states.iter().map(|state| StateSignatureView {
                name: state.name.as_str(),
                parameters: &state.parameters,
                return_type: state.return_type.as_ref(),
            }),
            program,
            symbols,
            diagnostics,
            format!("machine `{}`", machine.name),
        );
    }

    for platform in &program.platforms {
        validate_platform_state_names(platform, diagnostics);
        validate_state_signature_types(
            platform.states.iter().map(|state| StateSignatureView {
                name: state.name.as_str(),
                parameters: &state.parameters,
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
    program: &Program,
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

            validate_type_reference(
                program,
                &parameter.type_reference,
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
    platform: &crate::ir::platform::Platform,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut state_names = Vec::new();

    for state in &platform.states {
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
    program: &Program,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in &program.data_definitions {
        validate_data_member_names(data_definition, diagnostics);
        validate_data_shape(data_definition, diagnostics);

        for member in &data_definition.members {
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
    data_definition: &crate::ir::data::DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match data_definition.shape_kind() {
        DataShapeKind::Empty => diagnostics.push(Diagnostic::error(format!(
            "data `{}` must declare at least one field or variant",
            data_definition.name
        ))),
        DataShapeKind::Mixed => diagnostics.push(Diagnostic::error(format!(
            "data `{}` mixes fields and variants; split record data from enum-like data",
            data_definition.name
        ))),
        DataShapeKind::Enum | DataShapeKind::Record => {}
    }
}

fn validate_data_member_names(
    data_definition: &crate::ir::data::DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut member_names = Vec::new();

    for member in &data_definition.members {
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
    program: &Program,
    type_reference: &TypeReference,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    match type_reference {
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
        TypeReference::Named(name) => {
            if PrimitiveType::from_name(name).is_some() {
                return;
            }

            if !symbols.has_data_definition(name) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown data type `{name}`"
                )));
            }
        }
    }
}

fn validate_type_constraints(
    program: &Program,
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
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `finite` on non-primitive type `{}`",
                        base_type.display_name()
                    )));
                    continue;
                };

                if !primitive_type.accepts_finite_constraint() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `finite` on `{}`, but `finite` is only valid on floats",
                        primitive_type.name()
                    )));
                }
            }
            TypeConstraint::Named(name) => diagnostics.push(Diagnostic::error(format!(
                "{owner} uses unknown type constraint `{name}`"
            ))),
            TypeConstraint::Range { .. } => {
                let Some(primitive_type) = primitive_type else {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `range` on non-primitive type `{}`",
                        base_type.display_name()
                    )));
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

fn validate_entry_point(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    let Some(main_machine) = program
        .machines
        .iter()
        .find(|machine| machine.name == "main")
    else {
        diagnostics.push(Diagnostic::error("missing machine main"));
        return;
    };

    if !main_machine
        .states
        .iter()
        .any(|state| state.name == "entry")
    {
        diagnostics.push(Diagnostic::error("machine main is missing state entry"));
    }
}

fn validate_contained_types(
    machine: &crate::ir::machine::Machine,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contained_object in &machine.contains {
        if !symbols.is_callable_receiver_type(&contained_object.type_name) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` contains `{}` with unknown type `{}`",
                machine.name, contained_object.name, contained_object.type_name
            )));
        }
    }
}

fn validate_owned_data(
    program: &Program,
    machine: &crate::ir::machine::Machine,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for owned_data in &machine.owned_data {
        validate_type_reference(
            program,
            &owned_data.type_reference,
            symbols,
            diagnostics,
            format!(
                "machine `{}` owned data `{}`",
                machine.name, owned_data.name
            ),
        );

        if let Some(initial_value) = &owned_data.initial_value {
            validate_initial_value(
                program,
                &owned_data.type_reference,
                initial_value,
                diagnostics,
                format!(
                    "machine `{}` owned data `{}`",
                    machine.name, owned_data.name
                ),
            );
        }
    }
}

fn validate_initial_value(
    program: &Program,
    type_reference: &TypeReference,
    initial_value: &Expression,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    if !argument_matches_type(initial_value, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} initializer expects `{}`, got `{}`",
            type_reference.display_name_with_constraints(&program.type_constraints),
            expression_type_name(initial_value)
        )));
    }
}

fn validate_call(
    program: &Program,
    call: &crate::ir::statement::Call,
    current_machine: &crate::ir::machine::Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &ProgramSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(receiver) = call.receiver.as_deref() else {
        let Some(state) = machine_symbols.state(&call.target) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local state `{}`",
                current_machine.name, call.target
            )));
            return;
        };

        validate_call_arguments(
            program,
            &call.arguments,
            state.name.as_str(),
            &state.parameters,
            writable_roots,
            diagnostics,
        );
        return;
    };

    if receiver == "self" {
        let Some(state) = machine_symbols.state(&call.target) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local state `{}`",
                current_machine.name, call.target
            )));
            return;
        };

        validate_call_arguments(
            program,
            &call.arguments,
            state.name.as_str(),
            &state.parameters,
            writable_roots,
            diagnostics,
        );
        return;
    }

    let receiver_type = machine_symbols.contained_type(receiver);

    if let Some(platform) = receiver_type.and_then(|type_name| symbols.platform(type_name)) {
        let Some(state_signature) = platform
            .states
            .iter()
            .find(|state| state.name == call.target)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has no state `{}`",
                platform.name, call.target
            )));
            return;
        };

        validate_call_arguments(
            program,
            &call.arguments,
            &state_signature.name,
            &state_signature.parameters,
            writable_roots,
            diagnostics,
        );
        return;
    }

    if let Some(machine) = receiver_type
        .and_then(|type_name| symbols.machine(type_name))
        .or_else(|| symbols.machine(receiver))
    {
        if let Some(state) = machine
            .states
            .iter()
            .find(|state| state.name == call.target)
        {
            validate_call_arguments(
                program,
                &call.arguments,
                &state.name,
                &state.parameters,
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

    diagnostics.push(Diagnostic::error(format!(
        "unknown call receiver `{receiver}`"
    )));
}

fn validate_call_arguments(
    program: &Program,
    arguments: &[Expression],
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
        if parameter.is_mutable && !matches!(argument, Expression::Mutable(_)) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` must be passed with `mut`",
                parameter.name, target_name
            )));
            continue;
        }

        if !parameter.is_mutable && matches!(argument, Expression::Mutable(_)) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` is not mutable",
                parameter.name, target_name
            )));
            continue;
        }

        let expected_type = parameter
            .type_reference
            .display_name_with_constraints(&program.type_constraints);

        if !argument_matches_type(argument, &parameter.type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` expects `{}`, got `{}`",
                parameter.name,
                target_name,
                expected_type,
                expression_type_name(argument)
            )));
        }
    }

    validate_argument_borrows(arguments, target_name, writable_roots, diagnostics);
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

fn validate_argument_borrows(
    arguments: &[Expression],
    target_name: &str,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut accesses = Vec::new();

    for argument in arguments {
        collect_argument_accesses(
            argument,
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

fn collect_argument_accesses<'expression>(
    expression: &'expression Expression,
    target_name: &str,
    writable_roots: &WritableRoots<'_, '_>,
    accesses: &mut Vec<ArgumentAccess<'expression>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expression::Mutable(inner_expression) => {
            if !is_mutable_place(inner_expression) {
                diagnostics.push(Diagnostic::error(format!(
                    "mutable argument for state `{target_name}` must be a named place"
                )));
                return;
            }

            if let Some(root_name) = expression_root_name(inner_expression) {
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
        other_expression => collect_read_accesses(other_expression, accesses),
    }
}

fn collect_read_accesses<'expression>(
    expression: &'expression Expression,
    accesses: &mut Vec<ArgumentAccess<'expression>>,
) {
    match expression {
        Expression::ArrayLiteral(values) => {
            for value in values {
                collect_read_accesses(value, accesses);
            }
        }
        Expression::Binary(binary) => {
            collect_read_accesses(&binary.left, accesses);
            collect_read_accesses(&binary.right, accesses);
        }
        Expression::Indexed(indexed) => {
            if let Some(root_name) = expression_root_name(&indexed.collection) {
                accesses.push(ArgumentAccess {
                    root_name,
                    kind: ArgumentAccessKind::Read,
                });
            }

            collect_read_accesses(&indexed.index, accesses);
        }
        Expression::Name(path) => {
            if let Some(root_name) = path.first() {
                accesses.push(ArgumentAccess {
                    root_name,
                    kind: ArgumentAccessKind::Read,
                });
            }
        }
        Expression::Mutable(inner_expression) => collect_read_accesses(inner_expression, accesses),
        Expression::StructLiteral(struct_literal) => {
            for field in &struct_literal.fields {
                collect_read_accesses(&field.value, accesses);
            }
        }
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => {}
    }
}

fn is_mutable_place(expression: &Expression) -> bool {
    match expression {
        Expression::Indexed(indexed) => is_mutable_place(&indexed.collection),
        Expression::Name(_) => true,
        _ => false,
    }
}

fn expression_root_name(expression: &Expression) -> Option<&str> {
    match expression {
        Expression::Indexed(indexed) => expression_root_name(&indexed.collection),
        Expression::Name(path) => path.first().map(String::as_str),
        _ => None,
    }
}

fn argument_matches_type(argument: &Expression, type_reference: &TypeReference) -> bool {
    if let Expression::Mutable(inner_expression) = argument {
        return argument_matches_type(inner_expression, type_reference);
    }

    match type_reference {
        TypeReference::Constrained { base_type, .. } => argument_matches_type(argument, base_type),
        TypeReference::FixedArray { .. } => matches!(
            argument,
            Expression::ArrayLiteral(_) | Expression::Indexed(_) | Expression::Name(_)
        ),
        TypeReference::Named(type_name) => {
            if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
                return matches!(argument, Expression::Boolean(_))
                    && primitive_type == PrimitiveType::Bool
                    || matches!(argument, Expression::String(_))
                        && primitive_type == PrimitiveType::String
                    || matches!(argument, Expression::Float(_))
                        && primitive_type.accepts_float_literal()
                    || matches!(argument, Expression::Integer(_))
                        && primitive_type.accepts_integer_literal()
                    || matches!(
                        argument,
                        Expression::Binary(_)
                            | Expression::Indexed(_)
                            | Expression::Name(_)
                            | Expression::StructLiteral(_)
                    );
            }

            matches!(
                argument,
                Expression::Binary(_)
                    | Expression::Indexed(_)
                    | Expression::Name(_)
                    | Expression::StructLiteral(_)
            )
        }
    }
}

fn validate_expression_type(
    program: &Program,
    expression: &Expression,
    type_reference: &TypeReference,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    if !argument_matches_type(expression, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} expects `{}`, got `{}`",
            type_reference.display_name_with_constraints(&program.type_constraints),
            expression_type_name(expression)
        )));
    }
}

fn expression_type_name(argument: &Expression) -> &'static str {
    match argument {
        Expression::ArrayLiteral(_) => "array literal",
        Expression::Binary(_) => "binary expression",
        Expression::Boolean(_) => "bool",
        Expression::Float(_) => "float literal",
        Expression::Indexed(_) => "indexed value",
        Expression::Integer(_) => "integer literal",
        Expression::Mutable(inner_expression) => expression_type_name(inner_expression),
        Expression::Name(_) => "named value",
        Expression::StructLiteral(_) => "struct literal",
        Expression::String(_) => "String",
    }
}

fn validate_transition_target(
    program: &Program,
    target: &TransitionTarget,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &ProgramSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TransitionTarget::Named { path, arguments } = target else {
        return;
    };

    if path.len() == 1 {
        let Some(state) = machine_symbols.state(path[0].as_str()) else {
            diagnostics.push(Diagnostic::error(format!(
                "unknown state transition target `{}`",
                path[0]
            )));
            return;
        };

        validate_transition_arguments(
            program,
            arguments,
            state.name.as_str(),
            &state.parameters,
            writable_roots,
            diagnostics,
        );

        return;
    }

    if path.len() == 2 && path[0] == "self" {
        let Some(state) = machine_symbols.state(path[1].as_str()) else {
            diagnostics.push(Diagnostic::error(format!(
                "unknown state transition target `{}`",
                path[1]
            )));
            return;
        };

        validate_transition_arguments(
            program,
            arguments,
            state.name.as_str(),
            &state.parameters,
            writable_roots,
            diagnostics,
        );
        return;
    }

    let Some(receiver_type) = machine_symbols.contained_type(path[0].as_str()) else {
        diagnostics.push(Diagnostic::error(format!(
            "unknown nested transition receiver `{}`",
            path[0]
        )));
        return;
    };

    if path.len() == 2 {
        let Some(machine) = symbols.machine(receiver_type) else {
            return;
        };

        let Some(state) = machine.states.iter().find(|state| state.name == path[1]) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no state `{}`",
                machine.name, path[1]
            )));
            return;
        };

        validate_transition_arguments(
            program,
            arguments,
            &state.name,
            &state.parameters,
            writable_roots,
            diagnostics,
        );
    }
}

fn validate_transition_arguments(
    program: &Program,
    arguments: &[Expression],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_call_arguments(
        program,
        arguments,
        target_name,
        parameters,
        writable_roots,
        diagnostics,
    );
}
