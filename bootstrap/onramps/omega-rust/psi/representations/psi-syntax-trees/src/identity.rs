use crate::SyntaxTrees;
use crate::identifier::Identifier;
use crate::item::{
    BoundaryLevel, CapabilityContractKind, CapabilityMember, Item, ProofFact, PropositionBody,
    TargetHostSettingValue, WireDataMember,
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
        Item::Conformance(conformance) => {
            if let crate::item::ConformanceSubject::Carrier(type_name) = &conformance.subject {
                count_identifier(type_name, counts);
            }
            count_identifier(&conformance.trait_name, counts);
            if let Some(alias) = &conformance.alias {
                count_identifier(alias, counts);
            }
            for argument in syntax_trees
                .type_references
                .type_reference_handles(conformance.trait_arguments)
            {
                count_type_reference_handle(syntax_trees, *argument, counts);
            }
            if let crate::item::ConformanceBody::Closed { members } = conformance.body {
                for member in syntax_trees.items.conformance_members(members) {
                    match member {
                        crate::item::ConformanceMember::Machine(machine) => {
                            count_machine(syntax_trees, machine, counts)
                        }
                        crate::item::ConformanceMember::TraitDefault {
                            declaring_trait,
                            requirement_ordinal: _,
                            machine,
                        } => {
                            count_identifier(declaring_trait, counts);
                            count_machine(syntax_trees, machine, counts);
                        }
                        crate::item::ConformanceMember::Reference {
                            declaring_trait,
                            requirement,
                            target,
                        } => {
                            count_identifier(declaring_trait, counts);
                            count_identifier(requirement, counts);
                            count_identifier_members(
                                syntax_trees.items.identifier_path_members(*target),
                                counts,
                            );
                        }
                    }
                }
            }
        }
        Item::Const(constant) => {
            count_identifier(&constant.scope, counts);
            count_identifier(&constant.name, counts);
            count_type_reference_handle(syntax_trees, constant.type_reference, counts);
            count_expression_handle(syntax_trees, constant.value, counts);
        }
        Item::Data(data_definition) => {
            count_identifier(&data_definition.name, counts);
            for parameter in syntax_trees
                .items
                .type_parameters(data_definition.type_parameters)
            {
                count_identifier(&parameter.name, counts);
                count_type_parameter_kind(syntax_trees, &parameter.kind, counts);
            }
            if let Some(quotient) = &data_definition.quotient {
                count_type_reference_handle(syntax_trees, quotient.carrier, counts);
                for member in syntax_trees
                    .items
                    .identifier_path_members(quotient.relation)
                {
                    count_identifier(member, counts);
                }
                if let Some(selection) = &quotient.equivalence {
                    for member in syntax_trees
                        .items
                        .identifier_path_members(selection.relation)
                    {
                        count_identifier(member, counts);
                    }
                    count_identifier(&selection.trait_name, counts);
                    for argument in syntax_trees
                        .type_references
                        .type_reference_handles(selection.trait_arguments)
                    {
                        count_type_reference_handle(syntax_trees, *argument, counts);
                    }
                    count_identifier(&selection.conformance_name, counts);
                }
            }
            for member in syntax_trees.items.data_members(data_definition.members) {
                match member {
                    crate::item::DataMember::Field(field) => {
                        count_identifier(&field.name, counts);
                        count_type_reference_handle(syntax_trees, field.type_reference, counts);
                    }
                    crate::item::DataMember::Variant(variant) => {
                        count_identifier(&variant.name, counts);
                    }
                    crate::item::DataMember::Retired(_) => {}
                }
            }
        }
        Item::Domain(domain) => {
            count_identifier(&domain.name, counts);
            count_type_reference_handle(syntax_trees, domain.target_type, counts);
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
        Item::Proposition(proposition) => {
            count_identifier(&proposition.name, counts);
            for parameter in syntax_trees
                .items
                .type_parameters(proposition.type_parameters)
            {
                count_identifier(&parameter.name, counts);
                count_type_parameter_kind(syntax_trees, &parameter.kind, counts);
            }
            for parameter in syntax_trees.items.state_parameters(proposition.parameters) {
                count_state_parameter(syntax_trees, *parameter, counts);
            }
            match proposition.body {
                PropositionBody::Primitive => {}
                PropositionBody::Witness { evidence } => {
                    count_type_reference_handle(syntax_trees, evidence, counts);
                }
                PropositionBody::Transparent { proposition } => {
                    count_expression_handle(syntax_trees, proposition, counts);
                }
            }
        }
        Item::Use(use_item) => count_identifier_members(
            syntax_trees.items.identifier_path_members(use_item.path),
            counts,
        ),
        Item::Machine(machine) => count_machine(syntax_trees, machine, counts),

        Item::Trait(trait_definition) => {
            count_identifier(&trait_definition.name, counts);
            for parameter in syntax_trees
                .items
                .type_parameters(trait_definition.type_parameters)
            {
                count_identifier(&parameter.name, counts);
                count_type_parameter_kind(syntax_trees, &parameter.kind, counts);
            }
            for parent in syntax_trees
                .type_references
                .type_reference_handles(trait_definition.parents)
            {
                count_type_reference_handle(syntax_trees, *parent, counts);
            }
            count_identifier_members(
                syntax_trees
                    .items
                    .identifier_path_members(trait_definition.requires),
                counts,
            );
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
        Item::WireData(wire_data) => {
            count_identifier(&wire_data.name, counts);
            if let Some(encoding) = &wire_data.encoding {
                count_identifier(encoding, counts);
            }
            count_wire_data_members(syntax_trees, wire_data.members, counts);
        }
    }
}

fn count_machine(
    syntax_trees: &SyntaxTrees,
    machine: &crate::item::Machine,
    counts: &mut AstIdentityStorageCounts,
) {
    count_identifier(&machine.name, counts);
    for parameter in syntax_trees.items.type_parameters(machine.type_parameters) {
        count_identifier(&parameter.name, counts);
        count_type_parameter_kind(syntax_trees, &parameter.kind, counts);
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

fn count_wire_data_members(
    syntax_trees: &SyntaxTrees,
    members: psi_arena::HandleSpan<WireDataMember>,
    counts: &mut AstIdentityStorageCounts,
) {
    for member in syntax_trees.items.wire_data_members(members) {
        match member {
            WireDataMember::Field(field) => {
                count_identifier(&field.name, counts);
                count_type_reference_handle(syntax_trees, field.type_reference, counts);
            }
            WireDataMember::Reserved(_) => {}
            WireDataMember::Version(version) => {
                count_identifier(&version.name, counts);
                count_wire_data_members(syntax_trees, version.members, counts);
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
        count_type_parameter_kind(syntax_trees, &parameter.kind, counts);
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
}

fn count_statement_node(
    syntax_trees: &SyntaxTrees,
    statement: crate::statement::StatementHandle,
    counts: &mut AstIdentityStorageCounts,
) {
    match syntax_trees.statements.statement(statement) {
        crate::statement::StatementNode::AssemblyFact(fact) => {
            count_expression_handle(syntax_trees, fact.expression, counts);
        }
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
            for argument in &call.machine_arguments {
                count_static_argument(argument, counts);
            }
            for argument in syntax_trees.statements.expression_handles(call.arguments) {
                count_expression_handle(syntax_trees, *argument, counts);
            }
        }
        crate::statement::StatementNode::ProofOutputBindingStatement(binding) => {
            for binding in &binding.bindings {
                count_identifier(&binding.output_field, counts);
                count_identifier(&binding.binding, counts);
            }
            count_expression_handle(syntax_trees, binding.call, counts);
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
    for parameter in syntax_trees
        .items
        .type_parameters(signature.type_parameters)
    {
        count_identifier(&parameter.name, counts);
        count_type_parameter_kind(syntax_trees, &parameter.kind, counts);
    }
    for parameter in syntax_trees.items.state_parameters(signature.parameters) {
        count_state_parameter(syntax_trees, *parameter, counts);
    }
    if signature.return_type.is_valid() {
        count_type_reference_handle(syntax_trees, signature.return_type, counts);
    }
    for service in syntax_trees
        .items
        .identifier_path_members(signature.service_reaches)
    {
        count_identifier(service, counts);
    }
    for contract in syntax_trees.items.capability_contracts(signature.contracts) {
        count_contract(syntax_trees, contract, counts);
    }
    for statement in syntax_trees.items.statements(signature.default_body) {
        count_statement_node(syntax_trees, *statement, counts);
    }
}

fn count_state_signature_node(
    syntax_trees: &SyntaxTrees,
    signature: &crate::item::StateSignatureNode,
    counts: &mut AstIdentityStorageCounts,
) {
    count_identifier(&signature.name, counts);
    for parameter in syntax_trees
        .items
        .type_parameters(signature.type_parameters)
    {
        count_identifier(&parameter.name, counts);
        count_type_parameter_kind(syntax_trees, &parameter.kind, counts);
    }
    for parameter in syntax_trees.items.state_parameters(signature.parameters) {
        count_state_parameter(syntax_trees, *parameter, counts);
    }
    if signature.return_type.is_valid() {
        count_type_reference_handle(syntax_trees, signature.return_type, counts);
    }
    for service in syntax_trees
        .items
        .identifier_path_members(signature.service_reaches)
    {
        count_identifier(service, counts);
    }
    for contract in syntax_trees.items.capability_contracts(signature.contracts) {
        count_contract(syntax_trees, contract, counts);
    }
    for statement in syntax_trees.items.statements(signature.default_body) {
        count_statement_node(syntax_trees, *statement, counts);
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
        crate::expression::ExpressionNode::Atomic(atomic) => {
            count_expression_handle(syntax_trees, atomic.value, counts);
            if atomic.result.is_valid() {
                count_expression_handle(syntax_trees, atomic.result, counts);
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
            count_type_reference_handle(syntax_trees, cast.target_type, counts);
            for member in syntax_trees
                .expressions
                .identifier_path_members(cast.semantic_domain)
            {
                count_identifier(member, counts);
            }
            for argument in syntax_trees
                .type_references
                .type_reference_handles(cast.semantic_domain_arguments)
            {
                count_type_reference_handle(syntax_trees, *argument, counts);
            }
        }
        crate::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                count_expression_handle(syntax_trees, call.receiver, counts);
            }
            count_identifier(&call.target, counts);
            for argument in &call.machine_arguments {
                count_static_argument(argument, counts);
            }
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
        crate::expression::ExpressionNode::Borrow(expression) => {
            count_expression_handle(syntax_trees, expression.target, counts)
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
        crate::expression::ExpressionNode::ZeroValue(type_reference) => {
            count_type_reference_handle(syntax_trees, *type_reference, counts);
        }
    }
}

fn count_static_argument(
    argument: &crate::expression::StaticMachineArgument,
    counts: &mut AstIdentityStorageCounts,
) {
    for member in &argument.path {
        count_identifier(member, counts);
    }
    if let Some(projection) = &argument.evidence_projection {
        count_identifier(&projection.term, counts);
        count_identifier(&projection.member, counts);
    }
    if let Some(application) = &argument.application {
        for lifetime in &application.lifetime_arguments {
            count_identifier(lifetime, counts);
        }
        for nested in &application.arguments {
            count_static_argument(nested, counts);
        }
    }
}

fn count_source_text_float(value: &psi_source::SourceText, counts: &mut AstIdentityStorageCounts) {
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
            access: _,
            lifetime: _,
        } => count_type_reference_handle(syntax_trees, *referee, counts),
        crate::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            count_type_reference_handle(syntax_trees, *base_type, counts);
            for constraint in syntax_trees.type_references.constraints(*constraints) {
                match constraint {
                    crate::types::TypeConstraintNode::Named(name) => count_identifier(name, counts),
                    crate::types::TypeConstraintNode::Domain(domain) => {
                        count_identifier(&domain.name, counts);
                        for argument in syntax_trees
                            .type_references
                            .type_reference_handles(domain.arguments)
                        {
                            count_type_reference_handle(syntax_trees, *argument, counts);
                        }
                    }
                    crate::types::TypeConstraintNode::Range { minimum, maximum } => {
                        count_expression_handle(syntax_trees, *minimum, counts);
                        count_expression_handle(syntax_trees, *maximum, counts);
                    }
                    crate::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        crate::types::TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            count_type_reference_handle(syntax_trees, *element_type, counts);
            match length {
                crate::types::FixedArrayLength::ConstParameter(name)
                | crate::types::FixedArrayLength::ConstCall(name) => {
                    count_identifier(name, counts);
                }
                crate::types::FixedArrayLength::Literal(_) => {}
            }
        }
        crate::types::TypeReferenceNode::Slice { element_type } => {
            count_type_reference_handle(syntax_trees, *element_type, counts);
        }
        crate::types::TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            count_identifier(base_name, counts);
            for lifetime in lifetime_arguments {
                count_identifier(lifetime, counts);
            }
            for argument in syntax_trees
                .type_references
                .type_reference_handles(*arguments)
            {
                count_type_reference_handle(syntax_trees, *argument, counts);
            }
        }
        crate::types::TypeReferenceNode::ConstExpression(expression) => {
            count_expression_handle(syntax_trees, *expression, counts);
        }
        crate::types::TypeReferenceNode::DynamicTrait { name, conformance } => {
            count_identifier(name, counts);
            if let Some(conformance) = conformance {
                count_identifier(conformance, counts);
            }
        }
        crate::types::TypeReferenceNode::Named(name) => count_identifier(name, counts),
        crate::types::TypeReferenceNode::SelfType => {}
        crate::types::TypeReferenceNode::Unit => {}
    }
}

fn count_type_parameter_kind(
    syntax_trees: &SyntaxTrees,
    kind: &crate::item::TypeParameterKind,
    counts: &mut AstIdentityStorageCounts,
) {
    match kind {
        crate::item::TypeParameterKind::Type => {}
        crate::item::TypeParameterKind::Const { type_reference } => {
            count_type_reference_handle(syntax_trees, *type_reference, counts);
        }
        crate::item::TypeParameterKind::Machine { contract } => {
            if let Some(contract) = contract {
                match contract {
                    crate::item::MachineParameterContract::Structural(signature) => {
                        count_state_signature(syntax_trees, signature, counts);
                    }
                    crate::item::MachineParameterContract::Nominal { requirement } => {
                        for member in syntax_trees.items.identifier_path_members(*requirement) {
                            count_identifier(member, counts);
                        }
                    }
                }
            }
        }
        crate::item::TypeParameterKind::Proposition { contract } => {
            if let Some(contract) = contract {
                count_identifier(&contract.name, counts);
                for parameter in syntax_trees.items.state_parameters(contract.parameters) {
                    count_state_parameter(syntax_trees, *parameter, counts);
                }
            }
        }
    }
}

fn count_type_constraint_handle(
    syntax_trees: &SyntaxTrees,
    constraint: &crate::types::TypeConstraintNode,
    counts: &mut AstIdentityStorageCounts,
) {
    match constraint {
        crate::types::TypeConstraintNode::Named(name) => count_identifier(name, counts),
        crate::types::TypeConstraintNode::Domain(domain) => {
            count_identifier(&domain.name, counts);
            for argument in syntax_trees
                .type_references
                .type_reference_handles(domain.arguments)
            {
                count_type_reference_handle(syntax_trees, *argument, counts);
            }
        }
        crate::types::TypeConstraintNode::Range { minimum, maximum } => {
            count_expression_handle(syntax_trees, *minimum, counts);
            count_expression_handle(syntax_trees, *maximum, counts);
        }
        crate::types::TypeConstraintNode::ArithmeticDomain(_) => {}
    }
}

fn count_transition_target_node(
    syntax_trees: &SyntaxTrees,
    target: crate::statement::TransitionTargetHandle,
    counts: &mut AstIdentityStorageCounts,
) {
    match syntax_trees.statements.transition_target(target) {
        crate::statement::TransitionTargetNode::Named {
            path,
            arguments,
            evidence_arguments,
            ..
        } => {
            for member in syntax_trees.statements.identifier_path_members(*path) {
                count_identifier(member, counts);
            }
            for argument in syntax_trees.statements.expression_handles(*arguments) {
                count_expression_handle(syntax_trees, *argument, counts);
            }
            for evidence in evidence_arguments.iter() {
                count_identifier(evidence, counts);
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
