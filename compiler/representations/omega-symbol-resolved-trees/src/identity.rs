use crate::SymbolResolvedTrees;
use crate::data::DataMember;
use crate::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use crate::name::DiagnosticName;
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

pub fn count_identity_storage(program: &SymbolResolvedTrees) -> IdentityStorageCounts {
    let mut counts = IdentityStorageCounts::default();
    let child_type_references = &program.tables.declarations.child_type_references;
    let expression_table = &program.tables.bodies.expressions;

    for invariant in &program.invariant_definitions {
        count_declaration_name(&invariant.name, &mut counts);
    }

    for domain in &program.domain_definitions {
        count_declaration_name(&domain.name, &mut counts);
        count_type_reference(
            &domain.target_type,
            child_type_references,
            expression_table,
            &mut counts,
        );
    }

    for data_definition in &program.data_definitions {
        count_declaration_name(&data_definition.name, &mut counts);
        for member in program.data_members(data_definition.members) {
            match member {
                DataMember::Field(field) => {
                    count_declaration_name(&field.name, &mut counts);
                    count_type_reference(
                        &field.type_reference,
                        child_type_references,
                        expression_table,
                        &mut counts,
                    );
                }
                DataMember::Variant(variant) => count_declaration_name(&variant.name, &mut counts),
            }
        }
    }

    for platform in &program.platforms {
        count_declaration_name(&platform.name, &mut counts);
        for signature in program.platform_state_signatures(platform.states) {
            count_declaration_name(&signature.name, &mut counts);
            count_optional_type_reference(
                signature.return_type.as_ref(),
                child_type_references,
                expression_table,
                &mut counts,
            );
            for parameter in program.state_parameters(signature.parameters) {
                count_declaration_name(&parameter.name, &mut counts);
                count_type_reference(
                    &parameter.type_reference,
                    child_type_references,
                    expression_table,
                    &mut counts,
                );
            }
        }
    }

    for trait_definition in &program.traits {
        count_declaration_name(&trait_definition.name, &mut counts);
        for signature in program.trait_machine_signatures(trait_definition.machines) {
            count_declaration_name(&signature.name, &mut counts);
            count_optional_type_reference(
                signature.return_type.as_ref(),
                child_type_references,
                expression_table,
                &mut counts,
            );
            for parameter in program.state_parameters(signature.parameters) {
                count_declaration_name(&parameter.name, &mut counts);
                count_type_reference(
                    &parameter.type_reference,
                    child_type_references,
                    expression_table,
                    &mut counts,
                );
            }
        }
    }

    for machine in &program.machines {
        count_declaration_name(&machine.name, &mut counts);
        for contained in program.machine_contained_objects(machine.contains) {
            count_declaration_name(&contained.name, &mut counts);
            count_type_name(&contained.type_name, &mut counts);
        }
        for owned_data in program.machine_owned_data(machine.owned_data) {
            count_declaration_name(&owned_data.name, &mut counts);
            count_type_reference(
                &owned_data.type_reference,
                child_type_references,
                expression_table,
                &mut counts,
            );
            if owned_data.initial_value.is_valid() {
                count_expression_handle(expression_table, owned_data.initial_value, &mut counts);
            }
        }
        for state in program
            .machine_state_handles(machine.states)
            .iter()
            .map(|state| program.machine_state(*state))
        {
            count_declaration_name(&state.name, &mut counts);
            count_optional_type_reference(
                state.return_type.as_ref(),
                child_type_references,
                expression_table,
                &mut counts,
            );
            for parameter in program.state_parameters(state.parameters) {
                count_declaration_name(&parameter.name, &mut counts);
                count_type_reference(
                    &parameter.type_reference,
                    child_type_references,
                    expression_table,
                    &mut counts,
                );
            }
            for statement in program
                .tables
                .bodies
                .statements
                .statements(state.statement_nodes)
            {
                count_statement_node(
                    &program.tables.bodies.statements,
                    &program.tables.bodies.expressions,
                    &program.tables.types.references,
                    statement,
                    &mut counts,
                );
            }
        }
    }

    for (_, constraint) in program.tables.types.constraints.iter() {
        count_type_constraint(constraint, expression_table, &mut counts);
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
        TypeReferenceNode::SelfType { .. } => {}
        TypeReferenceNode::Unit => {}
    }
}

fn count_declaration_name(name: &DiagnosticName, counts: &mut IdentityStorageCounts) {
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
        ExpressionNode::Membership(membership) => {
            count_expression_handle(table, membership.value, counts);
            for name in table.name_path_members(membership.domain) {
                count_expression_path_member(name, counts);
            }
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

fn count_optional_type_reference(
    type_reference: Option<&TypeReference>,
    generic_arguments: &omega_core::arena::Arena<TypeReference>,
    expression_table: &ExpressionTable,
    counts: &mut IdentityStorageCounts,
) {
    if let Some(type_reference) = type_reference {
        count_type_reference(type_reference, generic_arguments, expression_table, counts);
    }
}

fn count_type_reference(
    type_reference: &TypeReference,
    generic_arguments: &omega_core::arena::Arena<TypeReference>,
    expression_table: &ExpressionTable,
    counts: &mut IdentityStorageCounts,
) {
    match type_reference {
        TypeReference::Reference(reference) => count_type_reference(
            generic_arguments.get(reference.referee),
            generic_arguments,
            expression_table,
            counts,
        ),
        TypeReference::Constrained(constrained) => count_type_reference(
            generic_arguments.get(constrained.base_type),
            generic_arguments,
            expression_table,
            counts,
        ),
        TypeReference::FixedArray(fixed_array) => {
            count_type_reference(
                generic_arguments.get(fixed_array.element_type),
                generic_arguments,
                expression_table,
                counts,
            );
        }
        TypeReference::Slice(slice) => {
            count_type_reference(
                generic_arguments.get(slice.element_type),
                generic_arguments,
                expression_table,
                counts,
            );
        }
        TypeReference::Generic(generic) => {
            count_type_name(&generic.base_name, counts);
            for argument in generic_arguments.span_or_empty(generic.arguments) {
                count_type_reference(argument, generic_arguments, expression_table, counts);
            }
        }
        TypeReference::Named { name, .. } => count_type_name(name, counts),
        TypeReference::SelfType { .. } => {}
        TypeReference::Unit => {}
    }
}

fn count_type_constraint(
    constraint: &TypeConstraint,
    expression_table: &ExpressionTable,
    counts: &mut IdentityStorageCounts,
) {
    match constraint {
        TypeConstraint::Named(name) => count_type_name(name, counts),
        TypeConstraint::Range { minimum, maximum } => {
            count_expression_handle(expression_table, *minimum, counts);
            count_expression_handle(expression_table, *maximum, counts);
        }
    }
}

fn count_type_name(name: &DiagnosticName, counts: &mut IdentityStorageCounts) {
    counts.type_names += 1;

    if !name.as_str().is_empty() {
        counts.generated_type_names += 1;
    }
}

fn count_expression_path_member(name: &DiagnosticName, counts: &mut IdentityStorageCounts) {
    counts.expression_path_members += 1;

    if !name.as_str().is_empty() {
        counts.generated_expression_path_members += 1;
    }
}

fn count_transition_path_member(name: &DiagnosticName, counts: &mut IdentityStorageCounts) {
    counts.transition_path_members += 1;

    if !name.as_str().is_empty() {
        counts.generated_transition_path_members += 1;
    }
}

fn count_call_name(name: &DiagnosticName, counts: &mut IdentityStorageCounts) {
    counts.call_names += 1;

    if !name.as_str().is_empty() {
        counts.generated_call_names += 1;
    }
}

fn count_struct_literal_name(name: &DiagnosticName, counts: &mut IdentityStorageCounts) {
    counts.struct_literal_names += 1;

    if !name.as_str().is_empty() {
        counts.generated_struct_literal_names += 1;
    }
}
