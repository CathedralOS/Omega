use crate::data::DataMember;
use crate::domain::ProofFact;
use crate::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use crate::name::Identifier;
use crate::statement::{StatementNode, StatementTable, TransitionGuardNode, TransitionTargetNode};
use crate::typed_trees::TypedTrees;
use crate::types::{TypeReferenceHandle, TypeReferenceNode, TypeReferenceTable};

/// Compact report fingerprint for one exact typed data definition.
///
/// Numbered member names are presentation-only; unnumbered members retain
/// their declaration position and source name. Runtime case ordinals are not
/// stable schema identities and are deliberately absent. This FNV value is not
/// authority: typed consumers retain and structurally replay the exact schema.
pub fn normalized_schema_report_fingerprint(
    typed: &TypedTrees,
    data: &crate::data::DataDefinition,
) -> u64 {
    fn byte(hash: &mut u64, value: u8) {
        *hash ^= u64::from(value);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
    fn bytes(hash: &mut u64, value: &[u8]) {
        for value in value {
            byte(hash, *value);
        }
    }
    fn uint(hash: &mut u64, value: u64) {
        bytes(hash, &value.to_le_bytes());
    }
    fn text(hash: &mut u64, value: &str) {
        uint(hash, value.len() as u64);
        bytes(hash, value.as_bytes());
    }
    fn member_name(hash: &mut u64, identity: Option<u64>, name: &str, position: usize) {
        match identity {
            Some(identity) => {
                byte(hash, 1);
                uint(hash, identity);
            }
            None => {
                byte(hash, 0);
                uint(hash, position as u64);
                text(hash, name);
            }
        }
    }

    let mut hash = 0xcbf29ce484222325u64;
    bytes(&mut hash, b"omega.schema.v2");
    let members = typed.data_members(data);
    let mut fields = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let mut cases = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Vec<_>>();
    if fields.iter().all(|field| field.identity.is_some()) {
        fields.sort_by_key(|field| field.identity);
    }
    if cases.iter().all(|case| case.identity.is_some()) {
        cases.sort_by_key(|case| case.identity);
    }
    uint(&mut hash, fields.len() as u64);
    for (position, field) in fields.iter().enumerate() {
        member_name(&mut hash, field.identity, field.name.as_str(), position);
        byte(
            &mut hash,
            match field.relevance {
                psi_language_core::BindingRelevance::Relevant => 0,
                psi_language_core::BindingRelevance::Erased => 1,
            },
        );
        text(
            &mut hash,
            typed.display_type_reference(field.type_reference).as_str(),
        );
    }
    uint(&mut hash, cases.len() as u64);
    for (position, case) in cases.iter().enumerate() {
        member_name(&mut hash, case.identity, case.name.as_str(), position);
        let mut payload = typed.data_payload_fields(case).iter().collect::<Vec<_>>();
        if payload.iter().all(|field| field.identity.is_some()) {
            payload.sort_by_key(|field| field.identity);
        }
        uint(&mut hash, payload.len() as u64);
        for (payload_position, field) in payload.iter().enumerate() {
            member_name(
                &mut hash,
                field.identity,
                field.name.as_str(),
                payload_position,
            );
            byte(
                &mut hash,
                match field.relevance {
                    psi_language_core::BindingRelevance::Relevant => 0,
                    psi_language_core::BindingRelevance::Erased => 1,
                },
            );
            text(
                &mut hash,
                typed.display_type_reference(field.type_reference).as_str(),
            );
        }
        let mut retired = case.retired_payload_identities.clone();
        retired.sort_unstable();
        uint(&mut hash, retired.len() as u64);
        for identity in retired {
            uint(&mut hash, identity);
        }
    }
    let mut retired = data.retired_identities.clone();
    retired.sort_unstable();
    uint(&mut hash, retired.len() as u64);
    for identity in retired {
        uint(&mut hash, identity);
    }
    if hash == 0 { 1 } else { hash }
}

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

pub fn count_identity_storage(typed_trees: &TypedTrees) -> IdentityStorageCounts {
    let mut counts = IdentityStorageCounts::default();

    for domain in typed_trees.domain_definitions() {
        count_declaration_name(&domain.name, &mut counts);
        count_type_reference_handle(
            &typed_trees.type_reference_table,
            domain.target_type,
            &mut counts,
        );
        for operator in typed_trees.domain_operators(domain) {
            count_operator(typed_trees, operator, &mut counts);
        }
    }

    for data_definition in typed_trees.data_definitions() {
        count_declaration_name(&data_definition.name, &mut counts);
        for parameter in typed_trees.data_type_parameters(data_definition) {
            count_type_parameter(typed_trees, parameter, &mut counts);
        }
        if let Some(quotient) = &data_definition.quotient {
            count_type_reference_handle(
                &typed_trees.type_reference_table,
                quotient.carrier,
                &mut counts,
            );
            for member in &quotient.relation {
                count_declaration_name(member, &mut counts);
            }
        }
        for member in typed_trees.data_members(data_definition) {
            match member {
                DataMember::Field(field) => {
                    count_declaration_name(&field.name, &mut counts);
                    count_type_reference_handle(
                        &typed_trees.type_reference_table,
                        field.type_reference,
                        &mut counts,
                    );
                }
                DataMember::Variant(variant) => count_declaration_name(&variant.name, &mut counts),
            }
        }
    }

    for conformance in typed_trees.conformances() {
        for lifetime in &conformance.lifetime_parameters {
            count_declaration_name(lifetime, &mut counts);
        }
        for parameter in typed_trees.conformance_type_parameters(conformance) {
            count_type_parameter(typed_trees, parameter, &mut counts);
        }
        if let crate::trait_definition::ConformanceSubject::Carrier(type_name) =
            &conformance.subject
        {
            count_declaration_name(type_name, &mut counts);
        }
        count_declaration_name(&conformance.trait_name, &mut counts);
        if let Some(alias) = &conformance.alias {
            count_declaration_name(alias, &mut counts);
        }
        for argument in typed_trees
            .type_reference_table
            .type_reference_handles(conformance.arguments)
        {
            count_type_reference_handle(&typed_trees.type_reference_table, *argument, &mut counts);
        }
        if let crate::trait_definition::ConformanceImplementation::Closed { rows } =
            &conformance.implementation
        {
            for row in rows {
                count_declaration_name(&row.declaring_trait_name, &mut counts);
                count_declaration_name(&row.requirement_name, &mut counts);
                count_declaration_name(&row.realization_name, &mut counts);
            }
        }
    }

    for trait_definition in typed_trees.traits() {
        count_declaration_name(&trait_definition.name, &mut counts);
        for parameter in typed_trees.trait_type_parameters(trait_definition) {
            count_declaration_name(&parameter.name, &mut counts);
        }
        for signature in typed_trees.trait_machine_signatures(trait_definition) {
            count_declaration_name(&signature.name, &mut counts);
            if signature.return_type.is_valid() {
                count_type_reference_handle(
                    &typed_trees.type_reference_table,
                    signature.return_type,
                    &mut counts,
                );
            }
            for parameter in typed_trees.state_signature_parameters(signature) {
                count_declaration_name(&parameter.name, &mut counts);
                count_type_reference_handle(
                    &typed_trees.type_reference_table,
                    parameter.type_reference,
                    &mut counts,
                );
            }
            for parameter in &signature.native_callback_parameters {
                count_declaration_name(&parameter.name, &mut counts);
                count_declaration_name(&parameter.binder, &mut counts);
            }
        }
    }

    for proposition in typed_trees.propositions() {
        count_declaration_name(&proposition.name, &mut counts);
        for binder in typed_trees.proposition_binders(proposition) {
            count_declaration_name(&binder.name, &mut counts);
            if let crate::proposition::PropositionBinderKind::Const { type_reference } = binder.kind
            {
                count_type_reference_handle(
                    &typed_trees.type_reference_table,
                    type_reference,
                    &mut counts,
                );
            }
        }
        for parameter in typed_trees.proposition_parameters(proposition) {
            count_declaration_name(&parameter.name, &mut counts);
            count_type_reference_handle(
                &typed_trees.type_reference_table,
                parameter.type_reference,
                &mut counts,
            );
        }
        match &proposition.body {
            crate::proposition::PropositionBody::Primitive => {}
            crate::proposition::PropositionBody::Witness { evidence } => {
                count_type_reference_handle(
                    &typed_trees.type_reference_table,
                    *evidence,
                    &mut counts,
                );
            }
            crate::proposition::PropositionBody::Transparent { proposition } => match proposition {
                crate::proposition::PropositionFormula::Application(application) => {
                    count_proposition_application(typed_trees, application, &mut counts)
                }
                crate::proposition::PropositionFormula::BooleanExpression(expression) => {
                    count_expression_handle(
                        &typed_trees.expression_table,
                        *expression,
                        &mut counts,
                    );
                }
            },
        }
    }

    for machine in typed_trees.machines() {
        count_declaration_name(&machine.name, &mut counts);
        for parameter in typed_trees.machine_type_parameters(machine) {
            count_type_parameter(typed_trees, parameter, &mut counts);
        }
        for owned_data in typed_trees.machine_owned_data(machine) {
            count_declaration_name(&owned_data.name, &mut counts);
            count_type_reference_handle(
                &typed_trees.type_reference_table,
                owned_data.type_reference,
                &mut counts,
            );
            if owned_data.initial_value.is_valid() {
                count_expression_handle(
                    &typed_trees.expression_table,
                    owned_data.initial_value,
                    &mut counts,
                );
            }
        }
        for state in typed_trees.machine_states(machine) {
            count_declaration_name(&state.name, &mut counts);
            if state.return_type.is_valid() {
                count_type_reference_handle(
                    &typed_trees.type_reference_table,
                    state.return_type,
                    &mut counts,
                );
            }
            for parameter in typed_trees.state_parameters(state) {
                count_declaration_name(&parameter.name, &mut counts);
                count_type_reference_handle(
                    &typed_trees.type_reference_table,
                    parameter.type_reference,
                    &mut counts,
                );
            }
            for statement in typed_trees
                .statement_table
                .statements(state.statement_nodes)
            {
                count_statement_node(
                    &typed_trees.statement_table,
                    &typed_trees.expression_table,
                    &typed_trees.type_reference_table,
                    statement,
                    &mut counts,
                );
            }
        }
    }

    for operator in typed_trees.operators() {
        count_operator(typed_trees, operator, &mut counts);
    }

    for package in &typed_trees.proof_output_calls {
        for binding in &package.bindings {
            count_declaration_name(&binding.output_field, &mut counts);
            count_declaration_name(&binding.binding, &mut counts);
        }
        count_expression_handle(&typed_trees.expression_table, package.call, &mut counts);
    }

    counts
}

fn count_operator(
    typed_trees: &TypedTrees,
    operator: &crate::operator::OperatorDefinition,
    counts: &mut IdentityStorageCounts,
) {
    for member in typed_trees.operator_path_members(operator.name) {
        count_declaration_name(member, counts);
    }
    for parameter in typed_trees
        .data_type_parameters
        .span_or_empty(operator.type_parameters)
    {
        count_declaration_name(&parameter.name, counts);
    }
    for parameter in typed_trees
        .state_parameters
        .span_or_empty(operator.parameters)
    {
        count_declaration_name(&parameter.name, counts);
        count_type_reference_handle(
            &typed_trees.type_reference_table,
            parameter.type_reference,
            counts,
        );
    }
    if operator.return_type.is_valid() {
        count_type_reference_handle(
            &typed_trees.type_reference_table,
            operator.return_type,
            counts,
        );
    }
}

fn count_type_parameter(
    typed_trees: &TypedTrees,
    parameter: &crate::data::TypeParameter,
    counts: &mut IdentityStorageCounts,
) {
    count_declaration_name(&parameter.name, counts);
    match &parameter.kind {
        crate::data::TypeParameterKind::Type => {}
        crate::data::TypeParameterKind::Const { type_reference } => {
            count_type_reference_handle(&typed_trees.type_reference_table, *type_reference, counts);
        }
        crate::data::TypeParameterKind::Machine { contract } => {
            if let crate::data::MachineParameterContract::Structural(contract) = contract {
                count_declaration_name(&contract.name, counts);
                for nested in typed_trees.state_signature_type_parameters(contract) {
                    count_type_parameter(typed_trees, nested, counts);
                }
                for contract_parameter in typed_trees.state_signature_parameters(contract) {
                    count_declaration_name(&contract_parameter.name, counts);
                    count_type_reference_handle(
                        &typed_trees.type_reference_table,
                        contract_parameter.type_reference,
                        counts,
                    );
                }
                if contract.return_type.is_valid() {
                    count_type_reference_handle(
                        &typed_trees.type_reference_table,
                        contract.return_type,
                        counts,
                    );
                }
                for binding in typed_trees.state_signature_invokes(contract) {
                    count_declaration_name(&binding.name, counts);
                }
                for contract in typed_trees.state_signature_contracts(contract) {
                    for fact in typed_trees.tables.proof_facts.span_or_empty(contract.facts) {
                        count_proof_fact(typed_trees, fact, counts);
                    }
                }
            }
        }
        crate::data::TypeParameterKind::Proposition { contract } => {
            count_declaration_name(&contract.name, counts);
            for contract_parameter in typed_trees
                .state_parameters
                .span_or_empty(contract.parameters)
            {
                count_declaration_name(&contract_parameter.name, counts);
                count_type_reference_handle(
                    &typed_trees.type_reference_table,
                    contract_parameter.type_reference,
                    counts,
                );
            }
        }
    }
}

fn count_proof_fact(
    typed_trees: &TypedTrees,
    fact: &ProofFact,
    counts: &mut IdentityStorageCounts,
) {
    match fact {
        ProofFact::Expression(expression) => {
            count_expression_handle(&typed_trees.expression_table, *expression, counts);
        }
        ProofFact::Membership(membership) => {
            count_expression_handle(&typed_trees.expression_table, membership.value, counts);
            for member in typed_trees.domain_path_members(membership.domain) {
                count_declaration_name(member, counts);
            }
        }
        ProofFact::Proposition(application) => {
            count_proposition_application(typed_trees, application, counts);
        }
    }
}

fn count_proposition_application(
    typed_trees: &TypedTrees,
    application: &crate::proposition::PropositionApplication,
    counts: &mut IdentityStorageCounts,
) {
    count_declaration_name(&application.name, counts);
    for binder in &application.binder_arguments {
        for member in &binder.path {
            count_declaration_name(member, counts);
        }
        if let Some(projection) = &binder.evidence_projection {
            count_declaration_name(&projection.term, counts);
            count_declaration_name(&projection.member, counts);
        }
    }
    for argument in typed_trees
        .expression_table
        .expression_handles(application.arguments)
    {
        count_expression_handle(&typed_trees.expression_table, *argument, counts);
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
        TypeReferenceNode::Unit => {}
    }
}

fn count_declaration_name(name: &Identifier, counts: &mut IdentityStorageCounts) {
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
            if let Some(operation) = &call.private_layout_operation {
                count_static_argument(&operation.selected_slot, counts);
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

fn count_type_name(name: &Identifier, counts: &mut IdentityStorageCounts) {
    counts.type_names += 1;

    if !name.as_str().is_empty() {
        counts.generated_type_names += 1;
    }
}

fn count_expression_path_member(name: &Identifier, counts: &mut IdentityStorageCounts) {
    counts.expression_path_members += 1;

    if !name.as_str().is_empty() {
        counts.generated_expression_path_members += 1;
    }
}

fn count_transition_path_member(name: &Identifier, counts: &mut IdentityStorageCounts) {
    counts.transition_path_members += 1;

    if !name.as_str().is_empty() {
        counts.generated_transition_path_members += 1;
    }
}

fn count_call_name(name: &Identifier, counts: &mut IdentityStorageCounts) {
    counts.call_names += 1;

    if !name.as_str().is_empty() {
        counts.generated_call_names += 1;
    }
}

fn count_struct_literal_name(name: &Identifier, counts: &mut IdentityStorageCounts) {
    counts.struct_literal_names += 1;

    if !name.as_str().is_empty() {
        counts.generated_struct_literal_names += 1;
    }
}
