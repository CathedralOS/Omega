use crate::Program;
use crate::data::DataMember;
use crate::expression::{Expression, ExpressionHandle, ExpressionNode, ExpressionTable};
use crate::name::ProgramName;
use crate::statement::{StatementNode, StatementTable, TransitionGuardNode, TransitionTargetNode};
use crate::types::{
    TypeConstraint, TypeReference, TypeReferenceHandle, TypeReferenceNode, TypeReferenceTable,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdentityStorageCounts {
    pub declaration_names: usize,
    pub source_declaration_names: usize,
    pub generated_declaration_names: usize,
    pub type_names: usize,
    pub source_type_names: usize,
    pub generated_type_names: usize,
    pub expression_path_members: usize,
    pub source_expression_path_members: usize,
    pub generated_expression_path_members: usize,
    pub transition_path_members: usize,
    pub source_transition_path_members: usize,
    pub generated_transition_path_members: usize,
    pub call_names: usize,
    pub source_call_names: usize,
    pub generated_call_names: usize,
    pub struct_literal_names: usize,
    pub source_struct_literal_names: usize,
    pub generated_struct_literal_names: usize,
    pub string_literals: usize,
    pub float_literals: usize,
    pub parsed_float_literals: usize,
}

impl IdentityStorageCounts {
    pub fn owned_identity_strings(self) -> usize {
        self.generated_declaration_names
            + self.generated_type_names
            + self.generated_expression_path_members
            + self.generated_transition_path_members
            + self.generated_call_names
            + self.generated_struct_literal_names
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
            count_type_name(&contained.type_name, &mut counts);
        }
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
            for statement in program.statement_table.statements(state.statement_nodes) {
                count_statement_node(
                    &program.statement_table,
                    &program.expression_table,
                    &program.type_reference_table,
                    statement,
                    &mut counts,
                );
            }
        }
    }

    for (_, constraint) in program.type_constraints.iter() {
        count_type_constraint(constraint, &mut counts);
    }

    counts
}

fn count_statement_node(
    statements: &StatementTable,
    expressions: &ExpressionTable,
    type_references: &TypeReferenceTable,
    statement: &StatementNode,
    counts: &mut IdentityStorageCounts,
) {
    match statement {
        StatementNode::Assignment(assignment) => {
            count_expression_handle(expressions, assignment.target, counts);
            count_expression_handle(expressions, assignment.value, counts);
        }
        StatementNode::Call(call) => {
            count_call_name(&call.target, counts);
            for receiver in statements.name_path_members(call.receiver) {
                count_call_name(receiver, counts);
            }
            for argument in statements.expression_handles(call.arguments) {
                count_expression_handle(expressions, *argument, counts);
            }
        }
        StatementNode::Expression(expression) => {
            count_expression_handle(expressions, *expression, counts)
        }
        StatementNode::LocalData(local_data) => {
            count_declaration_name(&local_data.name, counts);
            count_type_reference_handle(type_references, local_data.type_reference, counts);
            if local_data.initial_value.is_valid() {
                count_expression_handle(expressions, local_data.initial_value, counts);
            }
        }
        StatementNode::Transition(transition) => {
            count_transition_target_node(
                statements,
                expressions,
                statements.transition_target(transition.target),
                counts,
            );
            if transition.continuation.is_valid() {
                count_transition_target_node(
                    statements,
                    expressions,
                    statements.transition_target(transition.continuation),
                    counts,
                );
            }
            if let TransitionGuardNode::When(expression) = transition.guard {
                count_expression_handle(expressions, expression, counts);
            }
        }
    }
}

fn count_type_reference_handle(
    table: &TypeReferenceTable,
    type_reference: TypeReferenceHandle,
    counts: &mut IdentityStorageCounts,
) {
    count_type_reference_node(table, table.type_reference(type_reference), counts);
}

fn count_type_reference_node(
    table: &TypeReferenceTable,
    type_reference: &TypeReferenceNode,
    counts: &mut IdentityStorageCounts,
) {
    match type_reference {
        TypeReferenceNode::Reference { referee, .. } => {
            count_type_reference_handle(table, *referee, counts);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            count_type_reference_handle(table, *base_type, counts);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            count_type_reference_handle(table, *element_type, counts);
        }
        TypeReferenceNode::Slice { element_type } => {
            count_type_reference_handle(table, *element_type, counts);
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            count_type_name(base_name, counts);
            for argument in table.type_reference_handles(*arguments) {
                count_type_reference_handle(table, *argument, counts);
            }
        }
        TypeReferenceNode::Named { name, .. } => count_type_name(name, counts),
        TypeReferenceNode::Unit => {}
    }
}

fn count_declaration_name(name: &ProgramName, counts: &mut IdentityStorageCounts) {
    counts.declaration_names += 1;

    if !name.as_str().is_empty() {
        counts.generated_declaration_names += 1;
    }
}

fn count_transition_target_node(
    statements: &StatementTable,
    expressions: &ExpressionTable,
    target: &TransitionTargetNode,
    counts: &mut IdentityStorageCounts,
) {
    match target {
        TransitionTargetNode::Named { path, arguments } => {
            for name in statements.name_path_members(path.members) {
                count_transition_path_member(name, counts);
            }
            for argument in statements.expression_handles(*arguments) {
                count_expression_handle(expressions, *argument, counts);
            }
        }
        TransitionTargetNode::Value(expression) => {
            count_expression_handle(expressions, *expression, counts);
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn count_expression_handle(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    counts: &mut IdentityStorageCounts,
) {
    count_expression_node(table, table.expression(expression), counts);
}

fn count_expression_node(
    table: &ExpressionTable,
    expression: &ExpressionNode,
    counts: &mut IdentityStorageCounts,
) {
    match expression {
        ExpressionNode::ArrayLiteral(values) => {
            for value in table.expression_handles(*values) {
                count_expression_handle(table, *value, counts);
            }
        }
        ExpressionNode::Binary(binary) => {
            count_expression_handle(table, binary.left, counts);
            count_expression_handle(table, binary.right, counts);
        }
        ExpressionNode::Cast(cast) => {
            count_expression_handle(table, cast.value, counts);
            for name in table.name_path_members(cast.target_type) {
                count_expression_path_member(name, counts);
            }
        }
        ExpressionNode::Call(call) => {
            count_call_name(&call.target, counts);
            if call.receiver.is_valid() {
                count_expression_handle(table, call.receiver, counts);
            }
            for argument in table.expression_handles(call.arguments) {
                count_expression_handle(table, *argument, counts);
            }
        }
        ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) => {}
        ExpressionNode::Float(value) => {
            counts.float_literals += 1;
            let _ = value;
            counts.parsed_float_literals += 1;
        }
        ExpressionNode::Indexed(indexed) => {
            count_expression_handle(table, indexed.collection, counts);
            count_expression_handle(table, indexed.index, counts);
        }
        ExpressionNode::Mutable(expression) => count_expression_handle(table, *expression, counts),
        ExpressionNode::Member(member) => {
            count_expression_handle(table, member.receiver, counts);
            count_expression_path_member(&member.member, counts);
        }
        ExpressionNode::Name(path) => {
            for name in table.name_path_members(path.members) {
                count_expression_path_member(name, counts);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            count_struct_literal_name(&struct_literal.type_name, counts);
            for field in table.struct_fields(struct_literal.fields) {
                count_struct_literal_name(&field.name, counts);
                count_expression_handle(table, field.value, counts);
            }
        }
        ExpressionNode::String(_) => counts.string_literals += 1,
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
        Expression::Cast(cast) => {
            count_expression(&cast.value, counts);
            for name in cast.target_type.members() {
                count_expression_path_member(name, counts);
            }
        }
        Expression::Call(call) => {
            count_call_name(&call.target, counts);
            if let Some(receiver) = &call.receiver {
                count_expression(receiver, counts);
            }
            for argument in &call.arguments {
                count_expression(argument, counts);
            }
        }
        Expression::Boolean(_) | Expression::Integer(_) => {}
        Expression::Float(value) => {
            counts.float_literals += 1;
            let _ = value;
            counts.parsed_float_literals += 1;
        }
        Expression::Indexed(indexed) => {
            count_expression(&indexed.collection, counts);
            count_expression(&indexed.index, counts);
        }
        Expression::Member(member) => {
            count_expression(&member.receiver, counts);
            count_expression_path_member(&member.member, counts);
        }
        Expression::Mutable(expression) => count_expression(expression, counts),
        Expression::Name(path) => {
            for name in path.members() {
                count_expression_path_member(name, counts);
            }
        }
        Expression::StructLiteral(struct_literal) => {
            count_struct_literal_name(&struct_literal.type_name, counts);
            for field in &struct_literal.fields {
                count_struct_literal_name(&field.name, counts);
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
        TypeReference::Reference { referee, .. } => count_type_reference(referee, counts),
        TypeReference::Constrained { base_type, .. } => count_type_reference(base_type, counts),
        TypeReference::FixedArray { element_type, .. } => {
            count_type_reference(element_type, counts);
        }
        TypeReference::Slice { element_type } => {
            count_type_reference(element_type, counts);
        }
        TypeReference::Generic {
            base_name,
            arguments,
            ..
        } => {
            count_type_name(base_name, counts);
            for argument in arguments {
                count_type_reference(argument, counts);
            }
        }
        TypeReference::Named { name, .. } => count_type_name(name, counts),
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

    if !name.as_str().is_empty() {
        counts.generated_type_names += 1;
    }
}

fn count_expression_path_member(name: &ProgramName, counts: &mut IdentityStorageCounts) {
    counts.expression_path_members += 1;

    if !name.as_str().is_empty() {
        counts.generated_expression_path_members += 1;
    }
}

fn count_transition_path_member(name: &ProgramName, counts: &mut IdentityStorageCounts) {
    counts.transition_path_members += 1;

    if !name.as_str().is_empty() {
        counts.generated_transition_path_members += 1;
    }
}

fn count_call_name(name: &ProgramName, counts: &mut IdentityStorageCounts) {
    counts.call_names += 1;

    if !name.as_str().is_empty() {
        counts.generated_call_names += 1;
    }
}

fn count_struct_literal_name(name: &ProgramName, counts: &mut IdentityStorageCounts) {
    counts.struct_literal_names += 1;

    if !name.as_str().is_empty() {
        counts.generated_struct_literal_names += 1;
    }
}
