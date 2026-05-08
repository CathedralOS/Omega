use crate::Program;
use crate::data::DataMember;
use crate::expression::Expression;
use crate::name::ProgramName;
use crate::statement::{Statement, TransitionGuard, TransitionTarget};
use crate::types::{TypeConstraint, TypeReference};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdentityStorageCounts {
    pub declaration_names: usize,
    pub source_declaration_names: usize,
    pub generated_declaration_names: usize,
    pub type_names: usize,
    pub source_type_names: usize,
    pub generated_type_names: usize,
    pub expression_path_members: usize,
    pub transition_path_members: usize,
    pub call_names: usize,
    pub struct_literal_names: usize,
    pub string_literals: usize,
    pub float_literals: usize,
}

impl IdentityStorageCounts {
    pub fn owned_identity_strings(self) -> usize {
        self.generated_declaration_names
            + self.generated_type_names
            + self.expression_path_members
            + self.transition_path_members
            + self.call_names
            + self.struct_literal_names
    }
}

pub fn count_identity_storage(program: &Program) -> IdentityStorageCounts {
    let mut counts = IdentityStorageCounts::default();

    for invariant in &program.invariant_definitions {
        count_declaration_name(&invariant.name, &mut counts);
    }

    for data_definition in &program.data_definitions {
        count_declaration_name(&data_definition.name, &mut counts);
        for member in &data_definition.members {
            match member {
                DataMember::Field(field) => {
                    count_declaration_name(&field.name, &mut counts);
                    count_type_reference(&field.type_reference, &mut counts);
                }
                DataMember::Variant(variant) => count_declaration_name(&variant.name, &mut counts),
            }
        }
    }

    for platform in &program.platforms {
        count_declaration_name(&platform.name, &mut counts);
        for signature in &platform.states {
            count_declaration_name(&signature.name, &mut counts);
            count_optional_type_reference(signature.return_type.as_ref(), &mut counts);
            for parameter in &signature.parameters {
                count_declaration_name(&parameter.name, &mut counts);
                count_type_reference(&parameter.type_reference, &mut counts);
            }
        }
    }

    for machine in &program.machines {
        count_declaration_name(&machine.name, &mut counts);
        for contained in &machine.contains {
            count_declaration_name(&contained.name, &mut counts);
        }
        counts.type_names += machine.contains.len();
        for owned_data in &machine.owned_data {
            count_declaration_name(&owned_data.name, &mut counts);
            count_type_reference(&owned_data.type_reference, &mut counts);
            count_optional_expression(owned_data.initial_value.as_ref(), &mut counts);
        }
        for state in &machine.states {
            count_declaration_name(&state.name, &mut counts);
            count_optional_type_reference(state.return_type.as_ref(), &mut counts);
            for parameter in &state.parameters {
                count_declaration_name(&parameter.name, &mut counts);
                count_type_reference(&parameter.type_reference, &mut counts);
            }
            for statement in &state.statements {
                count_statement(statement, &mut counts);
            }
        }
    }

    for (_, constraint) in program.type_constraints.iter() {
        count_type_constraint(constraint, &mut counts);
    }

    counts
}

fn count_statement(statement: &Statement, counts: &mut IdentityStorageCounts) {
    match statement {
        Statement::Assignment(assignment) => {
            count_expression(&assignment.target, counts);
            count_expression(&assignment.value, counts);
        }
        Statement::Call(call) => {
            counts.call_names += 1;
            if call.receiver.is_some() {
                counts.call_names += 1;
            }
            for argument in &call.arguments {
                count_expression(argument, counts);
            }
        }
        Statement::Expression(expression) => count_expression(expression, counts),
        Statement::LocalData(local_data) => {
            count_declaration_name(&local_data.name, counts);
            count_type_reference(&local_data.type_reference, counts);
        }
        Statement::Transition(transition) => {
            count_transition_target(&transition.target, counts);
            if let Some(continuation) = &transition.continuation {
                count_transition_target(continuation, counts);
            }
            if let TransitionGuard::When(expression) = &transition.guard {
                count_expression(expression, counts);
            }
        }
    }
}

fn count_declaration_name(name: &ProgramName, counts: &mut IdentityStorageCounts) {
    counts.declaration_names += 1;

    if name.is_source_backed() {
        counts.source_declaration_names += 1;
    } else {
        counts.generated_declaration_names += 1;
    }
}

fn count_transition_target(target: &TransitionTarget, counts: &mut IdentityStorageCounts) {
    match target {
        TransitionTarget::Named { path, arguments } => {
            counts.transition_path_members += path.len();
            for argument in arguments {
                count_expression(argument, counts);
            }
        }
        TransitionTarget::SelfTarget | TransitionTarget::Terminal => {}
    }
}

fn count_expression(expression: &Expression, counts: &mut IdentityStorageCounts) {
    match expression {
        Expression::ArrayLiteral(values) => {
            for value in values {
                count_expression(value, counts);
            }
        }
        Expression::Binary(binary) => {
            count_expression(&binary.left, counts);
            count_expression(&binary.right, counts);
        }
        Expression::Boolean(_) | Expression::Integer(_) => {}
        Expression::Float(_) => counts.float_literals += 1,
        Expression::Indexed(indexed) => {
            count_expression(&indexed.collection, counts);
            count_expression(&indexed.index, counts);
        }
        Expression::Mutable(expression) => count_expression(expression, counts),
        Expression::Name(path) => counts.expression_path_members += path.len(),
        Expression::StructLiteral(struct_literal) => {
            counts.struct_literal_names += 1;
            for field in &struct_literal.fields {
                counts.struct_literal_names += 1;
                count_expression(&field.value, counts);
            }
        }
        Expression::String(_) => counts.string_literals += 1,
    }
}

fn count_optional_expression(expression: Option<&Expression>, counts: &mut IdentityStorageCounts) {
    if let Some(expression) = expression {
        count_expression(expression, counts);
    }
}

fn count_optional_type_reference(
    type_reference: Option<&TypeReference>,
    counts: &mut IdentityStorageCounts,
) {
    if let Some(type_reference) = type_reference {
        count_type_reference(type_reference, counts);
    }
}

fn count_type_reference(type_reference: &TypeReference, counts: &mut IdentityStorageCounts) {
    match type_reference {
        TypeReference::Constrained { base_type, .. } => count_type_reference(base_type, counts),
        TypeReference::FixedArray { element_type, .. } => {
            count_type_reference(element_type, counts);
        }
        TypeReference::Generic {
            base_name,
            arguments,
        } => {
            count_type_name(base_name, counts);
            for argument in arguments {
                count_type_reference(argument, counts);
            }
        }
        TypeReference::Named(name) => count_type_name(name, counts),
        TypeReference::Unit => {}
    }
}

fn count_type_constraint(constraint: &TypeConstraint, counts: &mut IdentityStorageCounts) {
    match constraint {
        TypeConstraint::Named(name) => count_type_name(name, counts),
        TypeConstraint::Range { minimum, maximum } => {
            count_expression(minimum, counts);
            count_expression(maximum, counts);
        }
    }
}

fn count_type_name(name: &ProgramName, counts: &mut IdentityStorageCounts) {
    counts.type_names += 1;

    if name.is_source_backed() {
        counts.source_type_names += 1;
    } else {
        counts.generated_type_names += 1;
    }
}
