use crate::SyntaxTrees;
use crate::identifier::Identifier;
use crate::item::{
    BoundaryLevel, CapabilityContractKind, CapabilityMember, Item, ProofFact,
    TargetHostSettingValue,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AstIdentityStorageCounts {
    pub identifiers: usize,
    pub source_identifiers: usize,
    pub generated_identifiers: usize,
    pub path_members: usize,
    pub string_literals: usize,
    pub float_literals: usize,
    pub source_float_literals: usize,
    pub generated_float_literals: usize,
}

impl AstIdentityStorageCounts {
    pub fn owned_identifier_strings(self) -> usize {
        self.generated_identifiers
    }
}

pub fn count_ast_identity_storage(syntax_trees: &SyntaxTrees) -> AstIdentityStorageCounts {
    let mut counts = AstIdentityStorageCounts::default();

    for item in syntax_trees.root_items() {
        count_item(syntax_trees, item, &mut counts);
    }

    counts
}

fn count_item(syntax_trees: &SyntaxTrees, item: &Item, counts: &mut AstIdentityStorageCounts) {
    match item {
        Item::Capability(capability) => {
            count_identifier(&capability.name, counts);
            for member in syntax_trees.items.capability_members(capability.members) {
                match member {
                    CapabilityMember::Field(field) => {
                        count_identifier(&field.name, counts);
                        count_type_reference_handle(syntax_trees, field.type_reference, counts);
                    }
                    CapabilityMember::State(state) => {
                        count_state_signature(syntax_trees, &state.signature, counts);
                        for contract in syntax_trees.items.capability_contracts(state.contracts) {
                            count_contract(syntax_trees, contract, counts);
                        }
                    }
                }
            }
        }
        Item::Data(data_definition) => {
            count_identifier(&data_definition.name, counts);
            for member in syntax_trees.items.data_members(data_definition.members) {
                match member {
                    crate::item::DataMember::Field(field) => {
                        count_identifier(&field.name, counts);
                        count_type_reference_handle(syntax_trees, field.type_reference, counts);
                        if field.initial_value.is_valid() {
                            count_expression_handle(syntax_trees, field.initial_value, counts);
                        }
                    }
                    crate::item::DataMember::Variant(variant) => {
                        count_identifier(&variant.name, counts);
                    }
                }
            }
        }
        Item::Domain(domain) => {
            count_identifier(&domain.name, counts);
            count_type_reference_handle(syntax_trees, domain.target_type, counts);
            if domain.classifier.is_valid() {
                count_expression_handle(syntax_trees, domain.classifier, counts);
            }
            for fact in syntax_trees.items.proof_facts(domain.facts) {
                match fact {
                    ProofFact::Expression(expression) => {
                        count_expression_handle(syntax_trees, *expression, counts);
                    }
                    ProofFact::Membership(membership) => {
                        count_expression_handle(syntax_trees, membership.value, counts);
                        count_identifier_members(
                            syntax_trees
                                .items
                                .identifier_path_members(membership.domain),
                            counts,
                        );
                    }
                }
            }
            for operator in syntax_trees.items.operators(domain.operators) {
                count_operator(syntax_trees, operator, counts);
            }
        }
        Item::Invariant(invariant) => {
            count_identifier(&invariant.name, counts);
            for constraint in syntax_trees
                .type_references
                .constraints(invariant.constraints)
            {
                count_type_constraint_handle(syntax_trees, constraint, counts);
            }
        }
        Item::Library(library) => {
            if let Some(name) = &library.name {
                count_identifier(name, counts);
            }
            counts.string_literals += 1;
            count_identifier(&library.calling_convention, counts);
            for function in syntax_trees.items.library_functions(library.functions) {
                count_state_signature(syntax_trees, &function.signature, counts);
                if function.symbol.is_some() {
                    counts.string_literals += 1;
                }
                if let Some(calling_convention) = &function.calling_convention {
                    count_identifier(calling_convention, counts);
                }
                for boundary in syntax_trees.items.boundary_levels(function.boundaries) {
                    if let BoundaryLevel::Named(name) = boundary {
                        count_identifier(name, counts);
                    }
                }
            }
        }
        Item::Measure(measure) => {
            count_identifier_members(
                syntax_trees.items.identifier_path_members(measure.name),
                counts,
            );
            if measure.parameter.is_valid() {
                count_state_parameter(syntax_trees, measure.parameter, counts);
            }
            if measure.return_type.is_valid() {
                count_type_reference_handle(syntax_trees, measure.return_type, counts);
            }
            for expression in syntax_trees.expressions.expression_handles(measure.body) {
                count_expression_handle(syntax_trees, *expression, counts);
            }
        }
        Item::Module(module) => count_identifier_members(
            syntax_trees.items.identifier_path_members(module.path),
            counts,
        ),
        Item::Operator(operator) => count_operator(syntax_trees, operator, counts),
        Item::Package(package) => count_identifier_members(
            syntax_trees.items.identifier_path_members(package.path),
            counts,
        ),
        Item::Provider(provider) => count_identifier_members(
            syntax_trees.items.identifier_path_members(provider.name),
            counts,
        ),
        Item::Use(use_item) => count_identifier_members(
            syntax_trees.items.identifier_path_members(use_item.path),
            counts,
        ),
        Item::Export(export_item) => {
            count_identifier_members(
                syntax_trees.items.identifier_path_members(export_item.path),
                counts,
            );
            if let Some(alias) = &export_item.alias {
                count_identifier(alias, counts);
            }
        }
        Item::Machine(machine) => {
            count_identifier(&machine.name, counts);
            if machine.abi.is_some() {
                counts.string_literals += 1;
            }
            for parameter in syntax_trees.items.type_parameters(machine.type_parameters) {
                count_identifier(&parameter.name, counts);
            }
            for state in syntax_trees.items.state_handles(machine.states) {
                let state = syntax_trees.items.state(*state);
                count_identifier(&state.name, counts);
                for parameter in syntax_trees.items.state_parameters(state.parameters) {
                    count_state_parameter(syntax_trees, *parameter, counts);
                }
                if state.return_type.is_valid() {
                    count_type_reference_handle(syntax_trees, state.return_type, counts);
                }
                for statement in syntax_trees.items.statements(state.statements) {
                    count_statement_node(syntax_trees, *statement, counts);
                }
            }
        }
        Item::Platform(platform) => {
            count_identifier(&platform.name, counts);
            for signature in syntax_trees.items.state_signatures(platform.states) {
                let signature = syntax_trees.items.state_signature(*signature);
                count_state_signature_node(syntax_trees, signature, counts);
            }
        }
        Item::Trait(trait_definition) => {
            count_identifier(&trait_definition.name, counts);
            for parameter in syntax_trees
                .items
                .type_parameters(trait_definition.type_parameters)
            {
                count_identifier(&parameter.name, counts);
            }
            for fact in syntax_trees.items.proof_facts(trait_definition.invariants) {
                count_proof_fact(syntax_trees, fact, counts);
            }
            for signature in syntax_trees
                .items
                .state_signatures(trait_definition.machines)
            {
                let signature = syntax_trees.items.state_signature(*signature);
                count_state_signature_node(syntax_trees, signature, counts);
            }
        }
        Item::Target(target) => {
            count_identifier(&target.name, counts);
            if let Some(host) = &target.host {
                count_identifier_members(
                    syntax_trees.items.identifier_path_members(host.provider),
                    counts,
                );
                for setting in syntax_trees.items.target_host_settings(host.settings) {
                    count_identifier(&setting.name, counts);
                    match &setting.value {
                        TargetHostSettingValue::Call { name, .. }
                        | TargetHostSettingValue::Named(name) => count_identifier(name, counts),
                    }
                }
            }
            for policy in syntax_trees
                .items
                .boundary_policies(target.boundary_policies)
            {
                count_identifier_members(
                    syntax_trees.items.identifier_path_members(policy.path),
                    counts,
                );
            }
        }
    }
}

fn count_operator(
    syntax_trees: &SyntaxTrees,
    operator: &crate::item::OperatorDefinition,
    counts: &mut AstIdentityStorageCounts,
) {
    count_identifier_members(
        syntax_trees.items.identifier_path_members(operator.name),
        counts,
    );
    for parameter in syntax_trees.items.type_parameters(operator.type_parameters) {
        count_identifier(&parameter.name, counts);
    }
    for parameter in syntax_trees.items.state_parameters(operator.parameters) {
        count_state_parameter(syntax_trees, *parameter, counts);
    }
    if operator.return_type.is_valid() {
        count_type_reference_handle(syntax_trees, operator.return_type, counts);
    }
    for contract in syntax_trees.items.capability_contracts(operator.contracts) {
        count_contract(syntax_trees, contract, counts);
    }
    if let Some(provider) = operator.provider {
        count_identifier_members(syntax_trees.items.identifier_path_members(provider), counts);
    }
}

fn count_statement_node(
    syntax_trees: &SyntaxTrees,
    statement: crate::statement::StatementHandle,
    counts: &mut AstIdentityStorageCounts,
) {
    match syntax_trees.statements.statement(statement) {
        crate::statement::StatementNode::Assignment(assignment) => {
            count_expression_handle(syntax_trees, assignment.target, counts);
            count_expression_handle(syntax_trees, assignment.value, counts);
        }
        crate::statement::StatementNode::Call(call) => {
            for member in syntax_trees
                .statements
                .identifier_path_members(call.receiver)
            {
                count_identifier(member, counts);
            }
            count_identifier(&call.target, counts);
            for argument in syntax_trees.statements.expression_handles(call.arguments) {
                count_expression_handle(syntax_trees, *argument, counts);
            }
        }
        crate::statement::StatementNode::Expression(expression) => {
            count_expression_handle(syntax_trees, *expression, counts)
        }
        crate::statement::StatementNode::LocalData(local_data) => {
            count_identifier(&local_data.name, counts);
            count_type_reference_handle(syntax_trees, local_data.type_reference, counts);
            if local_data.initial_value.is_valid() {
                count_expression_handle(syntax_trees, local_data.initial_value, counts);
            }
        }
        crate::statement::StatementNode::Relax(relax) => {
            count_expression_handle(syntax_trees, relax.target, counts);
            for statement in syntax_trees.items.statements(relax.statements) {
                count_statement_node(syntax_trees, *statement, counts);
            }
        }
        crate::statement::StatementNode::Transition(transition) => {
            count_transition_target_node(syntax_trees, transition.target, counts);
            if transition.continuation.is_valid() {
                count_transition_target_node(syntax_trees, transition.continuation, counts);
            }
            if let crate::statement::TransitionGuardNode::When(expression) = transition.guard {
                count_expression_handle(syntax_trees, expression, counts);
            }
        }
    }
}

fn count_state_signature(
    syntax_trees: &SyntaxTrees,
    signature: &crate::item::StateSignature,
    counts: &mut AstIdentityStorageCounts,
) {
    count_identifier(&signature.name, counts);
    for parameter in syntax_trees.items.state_parameters(signature.parameters) {
        count_state_parameter(syntax_trees, *parameter, counts);
    }
    if signature.return_type.is_valid() {
        count_type_reference_handle(syntax_trees, signature.return_type, counts);
    }
    for effect in syntax_trees
        .items
        .identifier_path_members(signature.effects)
    {
        count_identifier(effect, counts);
    }
    for contract in syntax_trees.items.capability_contracts(signature.contracts) {
        count_contract(syntax_trees, contract, counts);
    }
}

fn count_state_signature_node(
    syntax_trees: &SyntaxTrees,
    signature: &crate::item::StateSignatureNode,
    counts: &mut AstIdentityStorageCounts,
) {
    count_identifier(&signature.name, counts);
    for parameter in syntax_trees.items.state_parameters(signature.parameters) {
        count_state_parameter(syntax_trees, *parameter, counts);
    }
    if signature.return_type.is_valid() {
        count_type_reference_handle(syntax_trees, signature.return_type, counts);
    }
    for effect in syntax_trees
        .items
        .identifier_path_members(signature.effects)
    {
        count_identifier(effect, counts);
    }
    for contract in syntax_trees.items.capability_contracts(signature.contracts) {
        count_contract(syntax_trees, contract, counts);
    }
}

fn count_contract(
    syntax_trees: &SyntaxTrees,
    contract: &crate::item::CapabilityContract,
    counts: &mut AstIdentityStorageCounts,
) {
    if let CapabilityContractKind::Boundary(BoundaryLevel::Named(name)) = &contract.kind {
        count_identifier(name, counts);
    }
    for fact in syntax_trees.items.proof_facts(contract.facts) {
        count_proof_fact(syntax_trees, fact, counts);
    }
}

fn count_proof_fact(
    syntax_trees: &SyntaxTrees,
    fact: &ProofFact,
    counts: &mut AstIdentityStorageCounts,
) {
    match fact {
        ProofFact::Expression(expression) => {
            count_expression_handle(syntax_trees, *expression, counts);
        }
        ProofFact::Membership(membership) => {
            count_expression_handle(syntax_trees, membership.value, counts);
            count_identifier_members(
                syntax_trees
                    .items
                    .identifier_path_members(membership.domain),
                counts,
            );
        }
    }
}

fn count_state_parameter(
    syntax_trees: &SyntaxTrees,
    parameter: crate::item::StateParameterHandle,
    counts: &mut AstIdentityStorageCounts,
) {
    let parameter = syntax_trees.items.state_parameter(parameter);
    count_identifier(&parameter.name, counts);
    count_type_reference_handle(syntax_trees, parameter.type_reference, counts);
}

fn count_expression_handle(
    syntax_trees: &SyntaxTrees,
    expression: crate::expression::ExpressionHandle,
    counts: &mut AstIdentityStorageCounts,
) {
    match syntax_trees.expressions.expression(expression) {
        crate::expression::ExpressionNode::ArrayLiteral(values) => {
            for value in syntax_trees.expressions.expression_handles(*values) {
                count_expression_handle(syntax_trees, *value, counts);
            }
        }
        crate::expression::ExpressionNode::Binary(binary) => {
            count_expression_handle(syntax_trees, binary.left, counts);
            count_expression_handle(syntax_trees, binary.right, counts);
        }
        crate::expression::ExpressionNode::Boolean(_)
        | crate::expression::ExpressionNode::Integer(_) => {}
        crate::expression::ExpressionNode::Cast(cast) => {
            count_expression_handle(syntax_trees, cast.value, counts);
            for member in syntax_trees
                .expressions
                .identifier_path_members(cast.target_type)
            {
                count_identifier(member, counts);
            }
        }
        crate::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                count_expression_handle(syntax_trees, call.receiver, counts);
            }
            count_identifier(&call.target, counts);
            for argument in syntax_trees.expressions.expression_handles(call.arguments) {
                count_expression_handle(syntax_trees, *argument, counts);
            }
        }
        crate::expression::ExpressionNode::Float(value) => count_source_text_float(value, counts),
        crate::expression::ExpressionNode::Indexed(indexed) => {
            count_expression_handle(syntax_trees, indexed.collection, counts);
            count_expression_handle(syntax_trees, indexed.index, counts);
        }
        crate::expression::ExpressionNode::Membership(membership) => {
            count_expression_handle(syntax_trees, membership.value, counts);
            for member in syntax_trees
                .expressions
                .identifier_path_members(membership.domain)
            {
                count_identifier(member, counts);
            }
        }
        crate::expression::ExpressionNode::Member(member) => {
            count_expression_handle(syntax_trees, member.receiver, counts);
            count_identifier(&member.member, counts);
        }
        crate::expression::ExpressionNode::Mutable(expression) => {
            count_expression_handle(syntax_trees, *expression, counts)
        }
        crate::expression::ExpressionNode::Name(path) => {
            for member in syntax_trees.expressions.identifier_path_members(*path) {
                count_identifier(member, counts);
            }
        }
        crate::expression::ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                count_expression_handle(syntax_trees, range.start, counts);
            }
            if range.end.is_valid() {
                count_expression_handle(syntax_trees, range.end, counts);
            }
        }
        crate::expression::ExpressionNode::SelfValue => {}
        crate::expression::ExpressionNode::StructLiteral(struct_literal) => {
            count_identifier(&struct_literal.type_name, counts);
            for field in syntax_trees
                .expressions
                .struct_fields(struct_literal.fields)
            {
                count_identifier(&field.name, counts);
                count_expression_handle(syntax_trees, field.value, counts);
            }
        }
        crate::expression::ExpressionNode::String(_) => counts.string_literals += 1,
        crate::expression::ExpressionNode::Unary(unary) => {
            count_expression_handle(syntax_trees, unary.operand, counts);
        }
    }
}

fn count_source_text_float(
    value: &omega_core::source::SourceText,
    counts: &mut AstIdentityStorageCounts,
) {
    counts.float_literals += 1;
    if value.is_source_backed() {
        counts.source_float_literals += 1;
    } else {
        counts.generated_float_literals += 1;
    }
}

fn count_type_reference_handle(
    syntax_trees: &SyntaxTrees,
    type_reference: crate::types::TypeReferenceHandle,
    counts: &mut AstIdentityStorageCounts,
) {
    match syntax_trees.type_references.type_reference(type_reference) {
        crate::types::TypeReferenceNode::Reference {
            referee,
            is_mutable: _,
            is_relaxed: _,
        } => count_type_reference_handle(syntax_trees, *referee, counts),
        crate::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            count_type_reference_handle(syntax_trees, *base_type, counts);
            for constraint in syntax_trees.type_references.constraints(*constraints) {
                match constraint {
                    crate::types::TypeConstraintNode::Named(name) => count_identifier(name, counts),
                    crate::types::TypeConstraintNode::Range { minimum, maximum } => {
                        count_expression_handle(syntax_trees, *minimum, counts);
                        count_expression_handle(syntax_trees, *maximum, counts);
                    }
                }
            }
        }
        crate::types::TypeReferenceNode::FixedArray {
            element_type,
            length: _,
        }
        | crate::types::TypeReferenceNode::Slice { element_type } => {
            count_type_reference_handle(syntax_trees, *element_type, counts)
        }
        crate::types::TypeReferenceNode::Generic {
            base_name,
            arguments,
        } => {
            count_identifier(base_name, counts);
            for argument in syntax_trees
                .type_references
                .type_reference_handles(*arguments)
            {
                count_type_reference_handle(syntax_trees, *argument, counts);
            }
        }
        crate::types::TypeReferenceNode::Named(name) => count_identifier(name, counts),
        crate::types::TypeReferenceNode::SelfType => {}
        crate::types::TypeReferenceNode::Unit => {}
    }
}

fn count_type_constraint_handle(
    syntax_trees: &SyntaxTrees,
    constraint: &crate::types::TypeConstraintNode,
    counts: &mut AstIdentityStorageCounts,
) {
    match constraint {
        crate::types::TypeConstraintNode::Named(name) => count_identifier(name, counts),
        crate::types::TypeConstraintNode::Range { minimum, maximum } => {
            count_expression_handle(syntax_trees, *minimum, counts);
            count_expression_handle(syntax_trees, *maximum, counts);
        }
    }
}

fn count_transition_target_node(
    syntax_trees: &SyntaxTrees,
    target: crate::statement::TransitionTargetHandle,
    counts: &mut AstIdentityStorageCounts,
) {
    match syntax_trees.statements.transition_target(target) {
        crate::statement::TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            for member in syntax_trees.statements.identifier_path_members(*path) {
                count_identifier(member, counts);
            }
            for argument in syntax_trees.statements.expression_handles(*arguments) {
                count_expression_handle(syntax_trees, *argument, counts);
            }
        }
        crate::statement::TransitionTargetNode::Value(expression) => {
            count_expression_handle(syntax_trees, *expression, counts)
        }
        crate::statement::TransitionTargetNode::SelfTarget
        | crate::statement::TransitionTargetNode::Terminal => {}
    }
}

fn count_identifier_members(members: &[Identifier], counts: &mut AstIdentityStorageCounts) {
    counts.path_members += members.len();
    for member in members {
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
