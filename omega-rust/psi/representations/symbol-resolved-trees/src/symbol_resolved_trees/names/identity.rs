use crate::SymbolResolvedTrees;
use crate::data::DataMember;
use crate::domain::ProofFact;
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

    for domain in &program.domain_definitions {
        count_declaration_name(&domain.name, &mut counts);
        count_type_reference(
            &domain.target_type,
            child_type_references,
            expression_table,
            &mut counts,
        );
        for operator in program.operator_definitions(domain.operators) {
            count_operator(program, operator, &mut counts);
        }
    }

    for data_definition in &program.data_definitions {
        count_declaration_name(&data_definition.name, &mut counts);
        for parameter in program.data_type_parameters(data_definition.type_parameters) {
            count_type_parameter(
                program,
                parameter,
                child_type_references,
                expression_table,
                &mut counts,
            );
        }
        if let Some(quotient) = &data_definition.quotient {
            count_type_reference(
                &quotient.carrier,
                child_type_references,
                expression_table,
                &mut counts,
            );
            for member in &quotient.relation {
                count_declaration_name(member, &mut counts);
            }
            if let Some(selection) = &quotient.equivalence {
                for member in &selection.relation {
                    count_declaration_name(member, &mut counts);
                }
                count_declaration_name(&selection.trait_name, &mut counts);
                for argument in program.child_type_references(selection.trait_arguments) {
                    count_type_reference(
                        argument,
                        child_type_references,
                        expression_table,
                        &mut counts,
                    );
                }
                count_declaration_name(&selection.conformance_name, &mut counts);
            }
        }
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

    for conformance in &program.conformances {
        if let Some(type_name) = conformance.carrier_name() {
            count_declaration_name(type_name, &mut counts);
        }
        count_declaration_name(&conformance.trait_name, &mut counts);
        if let Some(alias) = &conformance.alias {
            count_declaration_name(alias, &mut counts);
        }
        for argument in child_type_references.span_or_empty(conformance.arguments) {
            count_type_reference(
                argument,
                child_type_references,
                expression_table,
                &mut counts,
            );
        }
    }

    for trait_definition in &program.traits {
        count_declaration_name(&trait_definition.name, &mut counts);
        for parameter in program.trait_type_parameters(trait_definition) {
            count_declaration_name(&parameter.name, &mut counts);
        }
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
            for parameter in &signature.native_callback_parameters {
                count_declaration_name(&parameter.name, &mut counts);
                count_declaration_name(&parameter.binder, &mut counts);
            }
        }
    }

    for proposition in &program.propositions {
        count_declaration_name(&proposition.name, &mut counts);
        for binder in program
            .tables
            .declarations
            .proposition_binders
            .span_or_empty(proposition.binders)
        {
            count_declaration_name(&binder.name, &mut counts);
            if let crate::proposition::PropositionBinderKind::Const { type_reference } =
                &binder.kind
            {
                count_type_reference(
                    type_reference,
                    child_type_references,
                    expression_table,
                    &mut counts,
                );
            }
        }
        for parameter in program.state_parameters(proposition.parameters) {
            count_declaration_name(&parameter.name, &mut counts);
            count_type_reference(
                &parameter.type_reference,
                child_type_references,
                expression_table,
                &mut counts,
            );
        }
        match &proposition.body {
            crate::proposition::PropositionBody::Primitive => {}
            crate::proposition::PropositionBody::Witness { evidence } => {
                count_type_reference(
                    evidence,
                    child_type_references,
                    expression_table,
                    &mut counts,
                );
            }
            crate::proposition::PropositionBody::Transparent { proposition } => {
                count_expression_handle(expression_table, *proposition, &mut counts);
            }
        }
    }

    for machine in &program.machines {
        count_declaration_name(&machine.name, &mut counts);
        for parameter in program.machine_type_parameters(machine) {
            count_type_parameter(
                program,
                parameter,
                child_type_references,
                expression_table,
                &mut counts,
            );
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
        count_type_constraint(
            constraint,
            &program.tables.declarations.child_type_references,
            expression_table,
            &mut counts,
        );
    }

    for operator in &program.operators {
        count_operator(program, operator, &mut counts);
    }

    for measure in &program.measures {
        count_measure(program, measure, &mut counts);
    }

    counts
}

fn count_measure(
    program: &SymbolResolvedTrees,
    measure: &crate::measure::MeasureDefinition,
    counts: &mut IdentityStorageCounts,
) {
    let child_type_references = &program.tables.declarations.child_type_references;
    let expression_table = &program.tables.bodies.expressions;

    for member in program.measure_path_members(measure.name) {
        count_declaration_name(member, counts);
    }
    if let Some(parameter) = &measure.parameter {
        count_declaration_name(&parameter.name, counts);
        count_type_reference(
            &parameter.type_reference,
            child_type_references,
            expression_table,
            counts,
        );
    }
    if let Some(return_type) = &measure.return_type {
        count_type_reference(return_type, child_type_references, expression_table, counts);
    }
    for expression in expression_table.expression_handles(measure.body) {
        count_expression_handle(expression_table, *expression, counts);
    }
}

fn count_operator(
    program: &SymbolResolvedTrees,
    operator: &crate::operator::OperatorDefinition,
    counts: &mut IdentityStorageCounts,
) {
    let child_type_references = &program.tables.declarations.child_type_references;
    let expression_table = &program.tables.bodies.expressions;

    for member in program.operator_path_members(operator.name) {
        count_declaration_name(member, counts);
    }
    for parameter in program.data_type_parameters(operator.type_parameters) {
        count_declaration_name(&parameter.name, counts);
    }
    for parameter in program.state_parameters(operator.parameters) {
        count_declaration_name(&parameter.name, counts);
        count_type_reference(
            &parameter.type_reference,
            child_type_references,
            expression_table,
            counts,
        );
    }
    if let Some(return_type) = &operator.return_type {
        count_type_reference(return_type, child_type_references, expression_table, counts);
    }
}

fn count_type_parameter(
    program: &SymbolResolvedTrees,
    parameter: &crate::data::TypeParameter,
    child_type_references: &arena::Arena<crate::types::TypeReference>,
    expression_table: &ExpressionTable,
    counts: &mut IdentityStorageCounts,
) {
    count_declaration_name(&parameter.name, counts);
    match &parameter.kind {
        crate::data::TypeParameterKind::Type => {}
        crate::data::TypeParameterKind::Const { type_reference } => {
            count_type_reference(
                type_reference,
                child_type_references,
                expression_table,
                counts,
            );
        }
        crate::data::TypeParameterKind::Machine { contract } => match contract {
            crate::data::MachineParameterContract::RequirementIdentity => {}
            crate::data::MachineParameterContract::Structural(contract) => {
                count_declaration_name(&contract.name, counts);
                for nested in program.data_type_parameters(contract.type_parameters) {
                    count_type_parameter(
                        program,
                        nested,
                        child_type_references,
                        expression_table,
                        counts,
                    );
                }
                for contract_parameter in program.state_parameters(contract.parameters) {
                    count_declaration_name(&contract_parameter.name, counts);
                    count_type_reference(
                        &contract_parameter.type_reference,
                        child_type_references,
                        expression_table,
                        counts,
                    );
                }
                count_optional_type_reference(
                    contract.return_type.as_ref(),
                    child_type_references,
                    expression_table,
                    counts,
                );
                for binding in program.signature_invokes(contract.invokes) {
                    count_declaration_name(binding, counts);
                }
                for contract in program.signature_contracts(contract.contracts) {
                    for fact in program.proof_facts(contract.facts) {
                        count_proof_fact(program, fact, expression_table, counts);
                    }
                }
            }
            crate::data::MachineParameterContract::AuthoredNominal { requirement } => {
                for member in requirement {
                    count_declaration_name(member, counts);
                }
            }
            crate::data::MachineParameterContract::Nominal { .. } => {}
        },
        crate::data::TypeParameterKind::Proposition { contract } => {
            count_declaration_name(&contract.name, counts);
            for contract_parameter in program.state_parameters(contract.parameters) {
                count_declaration_name(&contract_parameter.name, counts);
                count_type_reference(
                    &contract_parameter.type_reference,
                    child_type_references,
                    expression_table,
                    counts,
                );
            }
        }
    }
}

fn count_proof_fact(
    program: &SymbolResolvedTrees,
    fact: &ProofFact,
    expression_table: &ExpressionTable,
    counts: &mut IdentityStorageCounts,
) {
    match fact {
        ProofFact::Expression(expression) => {
            count_expression_handle(expression_table, *expression, counts);
        }
        ProofFact::Membership(membership) => {
            count_expression_handle(expression_table, membership.value, counts);
            for member in program.domain_path_members(membership.domain) {
                count_declaration_name(member, counts);
            }
        }
    }
}

fn count_statement_node(
    statements: &StatementTable,
    expressions: &ExpressionTable,
    type_references: &TypeReferenceTable,
    statement: &StatementNode,
    counts: &mut IdentityStorageCounts,
) {
    match statement {
        StatementNode::AssemblyFact(fact) => {
            count_expression_handle(expressions, fact.expression, counts);
        }
        StatementNode::Assignment(assignment) => {
            count_expression_handle(expressions, assignment.target, counts);
            count_expression_handle(expressions, assignment.value, counts);
        }
        StatementNode::Call(call) => {
            count_call_name(&call.target, counts);
            for argument in &call.machine_arguments {
                count_static_argument(argument, counts);
            }
            for receiver in statements.name_path_members(call.receiver) {
                count_call_name(receiver, counts);
            }
            for argument in statements.expression_handles(call.arguments) {
                count_expression_handle(expressions, *argument, counts);
            }
        }
        StatementNode::ProofOutputBindingStatement(package) => {
            for binding in &package.bindings {
                count_declaration_name(&binding.output_field, counts);
                count_declaration_name(&binding.binding, counts);
            }
            count_expression_handle(expressions, package.call, counts);
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
        TypeReferenceNode::ConstExpression(_) => {}
        TypeReferenceNode::DynamicTrait {
            name,
            conformance_carrier,
            conformance_name,
            ..
        } => {
            count_type_name(name, counts);
            if let Some(carrier) = conformance_carrier {
                count_type_name(carrier, counts);
            }
            if let Some(conformance) = conformance_name {
                count_type_name(conformance, counts);
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
        TransitionTargetNode::Named {
            path,
            arguments,
            evidence_arguments,
            ..
        } => {
            for name in statements.name_path_members(path.members) {
                count_transition_path_member(name, counts);
            }
            for argument in statements.expression_handles(*arguments) {
                count_expression_handle(expressions, *argument, counts);
            }
            for evidence in evidence_arguments.iter() {
                count_transition_path_member(evidence, counts);
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
        ExpressionNode::Atomic(atomic) => {
            count_expression_handle(table, atomic.value, counts);
            if atomic.result.is_valid() {
                count_expression_handle(table, atomic.result, counts);
            }
        }
        ExpressionNode::Binary(binary) => {
            count_expression_handle(table, binary.left, counts);
            count_expression_handle(table, binary.right, counts);
        }
        ExpressionNode::Cast(cast) => {
            count_expression_handle(table, cast.value, counts);
            for name in table.name_path_members(cast.target_label) {
                count_expression_path_member(name, counts);
            }
            for name in table.name_path_members(cast.semantic_domain) {
                count_expression_path_member(name, counts);
            }
        }
        ExpressionNode::Call(call) => {
            count_call_name(&call.target, counts);
            for argument in &call.machine_arguments {
                count_static_argument(argument, counts);
            }
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
        ExpressionNode::Borrow(expression) => {
            count_expression_handle(table, expression.target, counts)
        }
        ExpressionNode::Member(member) => {
            count_expression_handle(table, member.receiver, counts);
            count_expression_path_member(&member.member, counts);
        }
        ExpressionNode::Name(path) => {
            for name in table.name_path_members(path.members) {
                count_expression_path_member(name, counts);
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                count_expression_handle(table, range.start, counts);
            }
            if range.end.is_valid() {
                count_expression_handle(table, range.end, counts);
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
        ExpressionNode::Unary(unary) => count_expression_handle(table, unary.operand, counts),
        ExpressionNode::ZeroValue(_) => {}
    }
}

fn count_static_argument(
    argument: &crate::expression::StaticMachineArgument,
    counts: &mut IdentityStorageCounts,
) {
    for member in &argument.path {
        count_call_name(member, counts);
    }
    if let Some(projection) = &argument.evidence_projection {
        count_call_name(&projection.term, counts);
        count_call_name(&projection.member, counts);
    }
    if let Some(application) = &argument.application {
        for lifetime in &application.lifetime_arguments {
            count_call_name(lifetime, counts);
        }
        for nested in &application.arguments {
            count_static_argument(nested, counts);
        }
    }
}

fn count_optional_type_reference(
    type_reference: Option<&TypeReference>,
    generic_arguments: &arena::Arena<TypeReference>,
    expression_table: &ExpressionTable,
    counts: &mut IdentityStorageCounts,
) {
    if let Some(type_reference) = type_reference {
        count_type_reference(type_reference, generic_arguments, expression_table, counts);
    }
}

fn count_type_reference(
    type_reference: &TypeReference,
    generic_arguments: &arena::Arena<TypeReference>,
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
        TypeReference::ConstExpression(expression) => {
            count_expression_handle(expression_table, *expression, counts);
        }
        TypeReference::DynamicTrait {
            name,
            conformance_carrier,
            conformance_name,
            ..
        } => {
            count_type_name(name, counts);
            if let Some(carrier) = conformance_carrier {
                count_type_name(carrier, counts);
            }
            if let Some(conformance) = conformance_name {
                count_type_name(conformance, counts);
            }
        }
        TypeReference::Named { name, .. } => count_type_name(name, counts),
        TypeReference::SelfType { .. } => {}
        TypeReference::Unit => {}
    }
}

fn count_type_constraint(
    constraint: &TypeConstraint,
    type_references: &arena::Arena<TypeReference>,
    expression_table: &ExpressionTable,
    counts: &mut IdentityStorageCounts,
) {
    match constraint {
        TypeConstraint::Named(name) => count_type_name(name, counts),
        TypeConstraint::Domain(domain) => {
            count_type_name(&domain.name, counts);
            for argument in type_references.span_or_empty(domain.arguments) {
                count_type_reference(argument, type_references, expression_table, counts);
            }
        }
        TypeConstraint::Range { minimum, maximum } => {
            count_expression_handle(expression_table, *minimum, counts);
            count_expression_handle(expression_table, *maximum, counts);
        }
        TypeConstraint::ArithmeticDomain(_) => {}
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
