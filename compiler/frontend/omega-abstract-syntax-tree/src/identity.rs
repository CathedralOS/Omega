use crate::expression::Expression;
use crate::identifier::{Identifier, IdentifierPath};
use crate::item::{
    CapabilityContractKind, CapabilityMember, Item, TargetHostSettingValue, TrustLevel,
};
use crate::statement::{Statement, TransitionGuard, TransitionTarget};
use crate::types::{TypeConstraint, TypeReference};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AstIdentityStorageCounts {
    pub identifiers: usize,
    pub source_identifiers: usize,
    pub generated_identifiers: usize,
    pub path_members: usize,
    pub string_literals: usize,
    pub float_literals: usize,
}

impl AstIdentityStorageCounts {
    pub fn owned_identifier_strings(self) -> usize {
        self.generated_identifiers
    }
}

pub fn count_ast_identity_storage(items: &[Item]) -> AstIdentityStorageCounts {
    let mut counts = AstIdentityStorageCounts::default();

    for item in items {
        count_item(item, &mut counts);
    }

    counts
}

fn count_item(item: &Item, counts: &mut AstIdentityStorageCounts) {
    match item {
        Item::Capability(capability) => {
            count_identifier(&capability.name, counts);
            for member in &capability.members {
                match member {
                    CapabilityMember::Field(field) => {
                        count_identifier(&field.name, counts);
                        count_type_reference(&field.type_reference, counts);
                    }
                    CapabilityMember::State(state) => {
                        count_state_signature(&state.signature, counts);
                        for contract in &state.contracts {
                            if let CapabilityContractKind::Trusted(TrustLevel::Named(name)) =
                                &contract.kind
                            {
                                count_identifier(name, counts);
                            }
                        }
                    }
                }
            }
        }
        Item::Data(data_definition) => {
            count_identifier(&data_definition.name, counts);
            for member in &data_definition.members {
                match member {
                    crate::item::DataMember::Field(field) => {
                        count_identifier(&field.name, counts);
                        count_type_reference(&field.type_reference, counts);
                    }
                    crate::item::DataMember::Variant(variant) => {
                        count_identifier(&variant.name, counts);
                    }
                }
            }
        }
        Item::Invariant(invariant) => {
            count_identifier(&invariant.name, counts);
            for constraint in &invariant.constraints {
                count_type_constraint(constraint, counts);
            }
        }
        Item::TrustDefinition(trust_definition) => {
            count_identifier(&trust_definition.name, counts);
        }
        Item::Use(use_item) => count_identifier_path(&use_item.path, counts),
        Item::Machine(machine) => {
            count_identifier(&machine.name, counts);
            for contained in &machine.contains {
                count_identifier(&contained.name, counts);
                count_identifier(&contained.type_name, counts);
            }
            for owned_data in &machine.owned_data {
                count_identifier(&owned_data.name, counts);
                count_type_reference(&owned_data.type_reference, counts);
                if let Some(initial_value) = &owned_data.initial_value {
                    count_expression(initial_value, counts);
                }
            }
            for state in &machine.states {
                count_identifier(&state.name, counts);
                for parameter in &state.parameters {
                    count_state_parameter(parameter, counts);
                }
                if let Some(return_type) = &state.return_type {
                    count_type_reference(return_type, counts);
                }
                for statement in &state.statements {
                    count_statement(statement, counts);
                }
            }
        }
        Item::Platform(platform) => {
            count_identifier(&platform.name, counts);
            for signature in &platform.states {
                count_state_signature(signature, counts);
            }
        }
        Item::Target(target) => {
            count_identifier(&target.name, counts);
            if let Some(host) = &target.host {
                count_identifier_path(&host.provider, counts);
                for setting in &host.settings {
                    count_identifier(&setting.name, counts);
                    match &setting.value {
                        TargetHostSettingValue::Call { name, .. }
                        | TargetHostSettingValue::Named(name) => count_identifier(name, counts),
                    }
                }
            }
            for policy in &target.trust_policies {
                count_identifier_path(&policy.path, counts);
            }
        }
    }
}

fn count_state_signature(
    signature: &crate::item::StateSignature,
    counts: &mut AstIdentityStorageCounts,
) {
    count_identifier(&signature.name, counts);
    for parameter in &signature.parameters {
        count_state_parameter(parameter, counts);
    }
    if let Some(return_type) = &signature.return_type {
        count_type_reference(return_type, counts);
    }
}

fn count_state_parameter(
    parameter: &crate::item::StateParameter,
    counts: &mut AstIdentityStorageCounts,
) {
    count_identifier(&parameter.name, counts);
    count_type_reference(&parameter.type_reference, counts);
}

fn count_statement(statement: &Statement, counts: &mut AstIdentityStorageCounts) {
    match statement {
        Statement::Assignment(assignment) => {
            count_expression(&assignment.target, counts);
            count_expression(&assignment.value, counts);
        }
        Statement::Call(call) => {
            if let Some(receiver) = &call.receiver {
                count_identifier(receiver, counts);
            }
            count_identifier(&call.target, counts);
            for argument in &call.arguments {
                count_expression(argument, counts);
            }
        }
        Statement::Expression(expression) => count_expression(expression, counts),
        Statement::LocalData(local_data) => {
            count_identifier(&local_data.name, counts);
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

fn count_transition_target(target: &TransitionTarget, counts: &mut AstIdentityStorageCounts) {
    match target {
        TransitionTarget::Named { path, arguments } => {
            count_identifier_path(path, counts);
            for argument in arguments {
                count_expression(argument, counts);
            }
        }
        TransitionTarget::SelfTarget | TransitionTarget::Terminal => {}
    }
}

fn count_expression(expression: &Expression, counts: &mut AstIdentityStorageCounts) {
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
        Expression::Name(path) => count_identifier_path(path, counts),
        Expression::StructLiteral(struct_literal) => {
            count_identifier(&struct_literal.type_name, counts);
            for field in &struct_literal.fields {
                count_identifier(&field.name, counts);
                count_expression(&field.value, counts);
            }
        }
        Expression::String(_) => counts.string_literals += 1,
    }
}

fn count_type_reference(type_reference: &TypeReference, counts: &mut AstIdentityStorageCounts) {
    match type_reference {
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            count_type_reference(base_type, counts);
            for constraint in constraints {
                count_type_constraint(constraint, counts);
            }
        }
        TypeReference::FixedArray { element_type, .. } => {
            count_type_reference(element_type, counts);
        }
        TypeReference::Generic {
            base_name,
            arguments,
        } => {
            count_identifier(base_name, counts);
            for argument in arguments {
                count_type_reference(argument, counts);
            }
        }
        TypeReference::Named(name) => count_identifier(name, counts),
        TypeReference::Unit => {}
    }
}

fn count_type_constraint(constraint: &TypeConstraint, counts: &mut AstIdentityStorageCounts) {
    match constraint {
        TypeConstraint::Named(name) => count_identifier(name, counts),
        TypeConstraint::Range { minimum, maximum } => {
            count_expression(minimum, counts);
            count_expression(maximum, counts);
        }
    }
}

fn count_identifier_path(path: &IdentifierPath, counts: &mut AstIdentityStorageCounts) {
    counts.path_members += path.len();
    for member in path {
        count_identifier(member, counts);
    }
}

fn count_identifier(identifier: &Identifier, counts: &mut AstIdentityStorageCounts) {
    counts.identifiers += 1;
    if identifier.is_source_backed() {
        counts.source_identifiers += 1;
    } else {
        counts.generated_identifiers += 1;
    }
}
