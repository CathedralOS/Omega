//! Defer placement applications only in their owning lexical telescope.
//! Closed copies still enter ordinary plan discovery; names in unrelated
//! declarations never acquire template status from another scope.

use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use syntax_trees::item::{
    ConformanceMember, DataMember, Item, Machine, TypeParameter, TypeParameterKind,
};
use syntax_trees::statement::{
    StatementHandle, StatementNode, TransitionGuardNode, TransitionTargetNode,
};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn scoped_template_applications(
    syntax: &SyntaxTrees,
    selected_owners: &[(source::SourceSpan, Result<source::SourceSpan, String>)],
) -> Vec<TypeReferenceHandle> {
    let mut deferred = Vec::new();
    for item in syntax.root_items() {
        match item {
            Item::Machine(machine) => {
                visit_machine(syntax, machine, &[], selected_owners, &mut deferred)
            }
            Item::Proposition(definition) => {
                let binders = telescope(
                    syntax.items.type_parameters(definition.type_parameters),
                    &[],
                );
                visit_parameters(
                    syntax,
                    syntax.items.type_parameters(definition.type_parameters),
                    &binders,
                    &mut deferred,
                );
                for parameter in syntax.items.state_parameters(definition.parameters) {
                    visit_type(
                        syntax,
                        syntax.items.state_parameter(*parameter).type_reference,
                        &binders,
                        &mut deferred,
                    );
                }
                match &definition.body {
                    syntax_trees::item::PropositionBody::Witness { evidence } => {
                        visit_type(syntax, *evidence, &binders, &mut deferred);
                    }
                    syntax_trees::item::PropositionBody::Transparent { proposition } => {
                        visit_expression(syntax, *proposition, &binders, &mut deferred)
                    }
                    syntax_trees::item::PropositionBody::Primitive => {}
                }
            }
            Item::Data(data) => {
                let binders = telescope(syntax.items.type_parameters(data.type_parameters), &[]);
                visit_parameters(
                    syntax,
                    syntax.items.type_parameters(data.type_parameters),
                    &binders,
                    &mut deferred,
                );
                visit_facts(
                    syntax,
                    syntax.items.proof_facts(data.where_facts),
                    &binders,
                    &mut deferred,
                );
                if let Some(quotient) = &data.quotient {
                    visit_type(syntax, quotient.carrier, &binders, &mut deferred);
                }
                for member in syntax.items.data_members(data.members) {
                    match member {
                        DataMember::Field(field) => {
                            visit_type(syntax, field.type_reference, &binders, &mut deferred);
                        }
                        DataMember::Variant(variant) => {
                            for field in syntax.items.data_payload_fields(variant.payload) {
                                visit_type(syntax, field.type_reference, &binders, &mut deferred);
                            }
                        }
                        DataMember::Retired(_) => {}
                    }
                }
            }
            Item::Trait(definition) => {
                let binders = telescope(
                    syntax.items.type_parameters(definition.type_parameters),
                    &[],
                );
                visit_parameters(
                    syntax,
                    syntax.items.type_parameters(definition.type_parameters),
                    &binders,
                    &mut deferred,
                );
                for parent in syntax
                    .type_references
                    .type_reference_handles(definition.parents)
                {
                    visit_type(syntax, *parent, &binders, &mut deferred);
                }
                if let Some(reference) = definition.refines {
                    visit_type(syntax, reference, &binders, &mut deferred);
                }
                for bound in &definition.conformance_bounds {
                    for argument in syntax
                        .type_references
                        .type_reference_handles(bound.arguments)
                    {
                        visit_type(syntax, *argument, &binders, &mut deferred);
                    }
                }
                for signature in syntax.items.state_signatures(definition.machines) {
                    let signature = syntax.items.state_signature(*signature);
                    let binders = telescope(
                        syntax.items.type_parameters(signature.type_parameters),
                        &binders,
                    );
                    visit_parameters(
                        syntax,
                        syntax.items.type_parameters(signature.type_parameters),
                        &binders,
                        &mut deferred,
                    );
                    visit_contracts(
                        syntax,
                        syntax.items.capability_contracts(signature.contracts),
                        &binders,
                        &mut deferred,
                    );
                    visit_facts(
                        syntax,
                        syntax.items.proof_facts(signature.where_facts),
                        &binders,
                        &mut deferred,
                    );
                    visit_type(syntax, signature.return_type, &binders, &mut deferred);
                    for parameter in syntax.items.state_parameters(signature.parameters) {
                        visit_type(
                            syntax,
                            syntax.items.state_parameter(*parameter).type_reference,
                            &binders,
                            &mut deferred,
                        );
                    }
                    for statement in syntax.items.statements(signature.default_body) {
                        visit_statement(syntax, *statement, &binders, &mut deferred);
                    }
                }
            }
            Item::Conformance(definition) => {
                let binders = telescope(
                    syntax.items.type_parameters(definition.type_parameters),
                    &[],
                );
                visit_parameters(
                    syntax,
                    syntax.items.type_parameters(definition.type_parameters),
                    &binders,
                    &mut deferred,
                );
                for argument in syntax
                    .type_references
                    .type_reference_handles(definition.trait_arguments)
                {
                    visit_type(syntax, *argument, &binders, &mut deferred);
                }
                for member in syntax.items.conformance_members(definition.members) {
                    match member {
                        ConformanceMember::Machine(machine)
                        | ConformanceMember::Synthesized(machine)
                        | ConformanceMember::TraitDefault { machine, .. } => {
                            visit_machine(syntax, machine, &binders, selected_owners, &mut deferred)
                        }
                        ConformanceMember::Reference { .. } => {}
                    }
                }
            }
            _ => {}
        }
    }
    deferred
}

fn telescope<'a>(parameters: &'a [TypeParameter], outer: &[&'a str]) -> Vec<&'a str> {
    let mut binders = outer.to_vec();
    binders.extend(parameters.iter().map(|parameter| parameter.name.as_str()));
    binders
}

fn visit_parameters(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    binders: &[&str],
    deferred: &mut Vec<TypeReferenceHandle>,
) {
    for parameter in parameters {
        match &parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => {
                visit_type(syntax, *type_reference, binders, deferred);
            }
            TypeParameterKind::Machine {
                contract: Some(syntax_trees::item::MachineParameterContract::Structural(signature)),
            } => {
                let binders = telescope(
                    syntax.items.type_parameters(signature.type_parameters),
                    binders,
                );
                visit_type(syntax, signature.return_type, &binders, deferred);
                for parameter in syntax.items.state_parameters(signature.parameters) {
                    visit_type(
                        syntax,
                        syntax.items.state_parameter(*parameter).type_reference,
                        &binders,
                        deferred,
                    );
                }
            }
            _ => {}
        }
    }
}

fn visit_machine(
    syntax: &SyntaxTrees,
    machine: &Machine,
    outer: &[&str],
    selected_owners: &[(source::SourceSpan, Result<source::SourceSpan, String>)],
    deferred: &mut Vec<TypeReferenceHandle>,
) {
    let mut ambient = outer.to_vec();
    if let Some(selected) = selected_owners
        .iter()
        .find(|(declaration, _)| *declaration == machine.name.source_span())
        .and_then(|(_, owner)| owner.as_ref().ok())
    {
        let mut owners = syntax.root_items().filter_map(|item| match item {
            Item::Data(data) if data.name.source_span() == *selected => Some(data),
            _ => None,
        });
        if let (Some(owner), None) = (owners.next(), owners.next()) {
            ambient.extend(
                syntax
                    .items
                    .type_parameters(owner.type_parameters)
                    .iter()
                    .map(|parameter| parameter.name.as_str()),
            );
        }
    }
    let binders = telescope(
        syntax.items.type_parameters(machine.type_parameters),
        &ambient,
    );
    if binders.is_empty() {
        return;
    }
    visit_facts(
        syntax,
        syntax.items.proof_facts(machine.where_facts),
        &binders,
        deferred,
    );
    visit_contracts(
        syntax,
        syntax.items.capability_contracts(machine.contracts),
        &binders,
        deferred,
    );
    for clause in syntax.items.satisfies_clauses(machine.satisfies) {
        for argument in syntax
            .type_references
            .type_reference_handles(clause.arguments)
        {
            visit_type(syntax, *argument, &binders, deferred);
        }
        visit_expression(syntax, clause.via_expression, &binders, deferred);
    }
    for expression in syntax
        .expressions
        .expression_handles(machine.ranking_subjects)
        .iter()
        .chain(
            syntax
                .expressions
                .expression_handles(machine.ranking_view_arguments),
        )
    {
        visit_expression(syntax, *expression, &binders, deferred);
    }
    visit_expression(syntax, machine.ranking_range, &binders, deferred);
    visit_parameters(
        syntax,
        syntax.items.type_parameters(machine.type_parameters),
        &binders,
        deferred,
    );
    for bound in &machine.conformance_bounds {
        for argument in syntax
            .type_references
            .type_reference_handles(bound.arguments)
        {
            visit_type(syntax, *argument, &binders, deferred);
        }
    }
    for state in syntax.items.state_handles(machine.states) {
        let state = syntax.items.state(*state);
        visit_contracts(
            syntax,
            syntax.items.capability_contracts(state.contracts),
            &binders,
            deferred,
        );
        visit_type(syntax, state.return_type, &binders, deferred);
        for parameter in syntax.items.state_parameters(state.parameters) {
            visit_type(
                syntax,
                syntax.items.state_parameter(*parameter).type_reference,
                &binders,
                deferred,
            );
        }
        for statement in syntax.items.statements(state.statements) {
            visit_statement(syntax, *statement, &binders, deferred);
        }
    }
}

fn visit_type(
    syntax: &SyntaxTrees,
    reference: TypeReferenceHandle,
    binders: &[&str],
    deferred: &mut Vec<TypeReferenceHandle>,
) -> bool {
    if !reference.is_valid() {
        return false;
    }
    match syntax.type_references.type_reference(reference) {
        TypeReferenceNode::Named(name) => binders.contains(&name.as_str()),
        TypeReferenceNode::Reference { referee, .. } => {
            visit_type(syntax, *referee, binders, deferred)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut depends = visit_type(syntax, *base_type, binders, deferred);
            for constraint in syntax.type_references.constraints(*constraints) {
                if let TypeConstraintNode::Range {
                    minimum, maximum, ..
                } = constraint
                {
                    visit_expression(syntax, *minimum, binders, deferred);
                    visit_expression(syntax, *maximum, binders, deferred);
                }
                if let TypeConstraintNode::Domain(domain) = constraint {
                    for argument in syntax
                        .type_references
                        .type_reference_handles(domain.arguments)
                    {
                        depends |= visit_type(syntax, *argument, binders, deferred);
                    }
                }
            }
            depends
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            visit_type(syntax, *element_type, binders, deferred)
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            let mut depends = false;
            for argument in syntax.type_references.type_reference_handles(*arguments) {
                depends |= visit_type(syntax, *argument, binders, deferred);
            }
            if depends && !deferred.contains(&reference) {
                deferred.push(reference);
            }
            depends
        }
        TypeReferenceNode::DynamicTrait {
            trait_arguments, ..
        } => {
            let mut depends = false;
            for argument in syntax
                .type_references
                .type_reference_handles(*trait_arguments)
            {
                depends |= visit_type(syntax, *argument, binders, deferred);
            }
            depends
        }
        TypeReferenceNode::ConstExpression(expression) => {
            visit_expression(syntax, *expression, binders, deferred);
            false
        }
        TypeReferenceNode::Unit => false,
    }
}

fn visit_static_arguments(
    syntax: &SyntaxTrees,
    arguments: &[StaticMachineArgument],
    binders: &[&str],
    deferred: &mut Vec<TypeReferenceHandle>,
) {
    for argument in arguments {
        visit_type(syntax, argument.type_reference, binders, deferred);
        if let Some(application) = &argument.application {
            visit_static_arguments(syntax, &application.arguments, binders, deferred);
        }
    }
}

fn visit_statement(
    syntax: &SyntaxTrees,
    statement: StatementHandle,
    binders: &[&str],
    deferred: &mut Vec<TypeReferenceHandle>,
) {
    match syntax.statements.statement(statement) {
        StatementNode::Assembly(handle) => {
            let graph = syntax.statements.assembly(*handle);
            for block in syntax.statements.assembly_blocks(graph.blocks) {
                for expression in syntax
                    .statements
                    .assembly_expressions(block.requires)
                    .iter()
                    .chain(syntax.statements.assembly_expressions(block.ensures))
                {
                    visit_expression(syntax, *expression, binders, deferred);
                }
                for instruction in syntax.statements.assembly_instructions(block.instructions) {
                    visit_statement(syntax, *instruction, binders, deferred);
                }
            }
        }
        StatementNode::RootBinding(binding) => {
            visit_expression(syntax, binding.receiver, binders, deferred);
            visit_expression(syntax, binding.implementation_operand, binders, deferred);
        }
        StatementNode::AssemblyFact(fact) => {
            visit_expression(syntax, fact.expression, binders, deferred)
        }
        StatementNode::Assignment(assignment) => {
            visit_expression(syntax, assignment.target, binders, deferred);
            visit_expression(syntax, assignment.value, binders, deferred);
        }
        StatementNode::Call(call) => {
            visit_static_arguments(syntax, &call.machine_arguments, binders, deferred);
            for argument in syntax.statements.expression_handles(call.arguments) {
                visit_expression(syntax, *argument, binders, deferred);
            }
        }
        StatementNode::ProofOutputBindingStatement(binding) => {
            visit_expression(syntax, binding.call, binders, deferred)
        }
        StatementNode::Expression(expression) => {
            visit_expression(syntax, *expression, binders, deferred)
        }
        StatementNode::LocalData(local) => {
            visit_type(syntax, local.type_reference, binders, deferred);
            visit_expression(syntax, local.initial_value, binders, deferred);
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                visit_expression(syntax, guard, binders, deferred);
            }
            for target in transition.targets() {
                match syntax.statements.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in syntax.statements.expression_handles(*arguments) {
                            visit_expression(syntax, *argument, binders, deferred);
                        }
                    }
                    TransitionTargetNode::Value(value) => {
                        visit_expression(syntax, *value, binders, deferred)
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

fn visit_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    binders: &[&str],
    deferred: &mut Vec<TypeReferenceHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    match syntax.expressions.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            visit_expression(syntax, dispatch.subject, binders, deferred);
            for arm in syntax.expressions.match_arms(dispatch.arms) {
                if let syntax_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    visit_expression(syntax, pattern, binders, deferred);
                }
                visit_expression(syntax, arm.value, binders, deferred);
            }
        }
        ExpressionNode::ArrayLiteral(expressions) => {
            for expression in syntax.expressions.expression_handles(*expressions) {
                visit_expression(syntax, *expression, binders, deferred);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            visit_expression(syntax, atomic.value, binders, deferred);
            visit_expression(syntax, atomic.result, binders, deferred);
        }
        ExpressionNode::Binary(binary) => {
            visit_expression(syntax, binary.left, binders, deferred);
            visit_expression(syntax, binary.right, binders, deferred);
        }
        ExpressionNode::Cast(cast) => {
            visit_type(syntax, cast.target_type, binders, deferred);
            for argument in syntax
                .type_references
                .type_reference_handles(cast.semantic_domain_arguments)
            {
                visit_type(syntax, *argument, binders, deferred);
            }
            visit_expression(syntax, cast.value, binders, deferred);
        }
        ExpressionNode::Call(call) => {
            visit_static_arguments(syntax, &call.machine_arguments, binders, deferred);
            visit_expression(syntax, call.receiver, binders, deferred);
            for argument in syntax.expressions.expression_handles(call.arguments) {
                visit_expression(syntax, *argument, binders, deferred);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            visit_expression(syntax, indexed.collection, binders, deferred);
            visit_expression(syntax, indexed.index, binders, deferred);
        }
        ExpressionNode::Membership(membership) => {
            visit_expression(syntax, membership.value, binders, deferred)
        }
        ExpressionNode::Member(member) => {
            visit_expression(syntax, member.receiver, binders, deferred)
        }
        ExpressionNode::Borrow(borrow) => {
            visit_expression(syntax, borrow.target, binders, deferred)
        }
        ExpressionNode::Range(range) => {
            visit_expression(syntax, range.start, binders, deferred);
            visit_expression(syntax, range.end, binders, deferred);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in syntax.expressions.struct_fields(literal.fields) {
                visit_expression(syntax, field.value, binders, deferred);
            }
        }
        ExpressionNode::Unary(unary) => visit_expression(syntax, unary.operand, binders, deferred),
        ExpressionNode::TypeExpression(reference) | ExpressionNode::ZeroValue(reference) => {
            visit_type(syntax, *reference, binders, deferred);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn visit_facts(
    syntax: &SyntaxTrees,
    facts: &[syntax_trees::item::ProofFact],
    binders: &[&str],
    deferred: &mut Vec<TypeReferenceHandle>,
) {
    for fact in facts {
        match fact {
            syntax_trees::item::ProofFact::Expression(expression) => {
                visit_expression(syntax, *expression, binders, deferred)
            }
            syntax_trees::item::ProofFact::Membership(membership) => {
                visit_expression(syntax, membership.value, binders, deferred);
                for argument in syntax
                    .type_references
                    .type_reference_handles(membership.domain_arguments)
                {
                    visit_type(syntax, *argument, binders, deferred);
                }
            }
        }
    }
}

fn visit_contracts(
    syntax: &SyntaxTrees,
    contracts: &[syntax_trees::item::CapabilityContract],
    binders: &[&str],
    deferred: &mut Vec<TypeReferenceHandle>,
) {
    for contract in contracts {
        visit_facts(
            syntax,
            syntax.items.proof_facts(contract.facts),
            binders,
            deferred,
        );
    }
}
