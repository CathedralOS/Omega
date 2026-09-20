//! Contextual domain, value and statement member targets.

use crate::authored_selections::CheckedResolutionTarget;
use crate::authored_selections::call_targets::declaration_target;
use crate::authored_selections::contexts;
use crate::authored_selections::operator_targets::{
    authored_operand_type, type_reference_for_symbol,
};
use checked_trees::CheckFacts;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

fn contextual_domain_member_target(
    program: &TypedTrees,
    containing_expression: typed_trees::expression::ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let target_type = contextual_domain_target_type(program, containing_expression)?;
    contextual_self_member_symbol(program, member, target_type).and_then(declaration_target)
}

pub(crate) fn checked_member_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    declaration_target(crate::flow::effective_member_symbol(
        program,
        member.receiver,
        member,
    ))
    .or_else(|| {
        contexts::checked_member_target_from_exact_owner(program, facts, expression, member).map(
            |target| match target {
                contexts::OwnerMemberTarget::Declaration(symbol) => {
                    CheckedResolutionTarget::Declaration(symbol)
                }
                contexts::OwnerMemberTarget::CollectionLength => {
                    CheckedResolutionTarget::Intrinsic(
                        AuthoredDeclarationSelectionIntrinsic::CollectionLength,
                    )
                }
                contexts::OwnerMemberTarget::CollectionCapacity => {
                    CheckedResolutionTarget::Intrinsic(
                        AuthoredDeclarationSelectionIntrinsic::CollectionCapacity,
                    )
                }
            },
        )
    })
    .or_else(|| {
        let matching = facts
            .fact_call_projections
            .iter()
            .filter(|projection| projection.projection_expression == expression)
            .collect::<Vec<_>>();
        let [projection] = matching.as_slice() else {
            return None;
        };
        declaration_target(projection.field)
    })
    .or_else(|| checked_value_member_target(program, facts, member))
    .or_else(|| authored_member_target(program, member))
    .or_else(|| contextual_domain_member_target(program, expression, member))
    .or_else(|| contextual_statement_member_target(program, expression, member))
}

fn checked_value_member_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let mut resolved = None;
    for (_, value) in facts.values.expression_values(member.receiver) {
        if !value.type_reference.is_valid() {
            continue;
        }
        let target = member_symbol_from_type_reference(
            program,
            value.type_reference,
            member.member.as_str(),
        )
        .and_then(declaration_target)
        .or_else(|| {
            collection_measure_target(program, value.type_reference, member.member.as_str())
        });
        let Some(target) = target else {
            continue;
        };
        if resolved.is_some_and(|candidate| candidate != target) {
            return None;
        }
        resolved = Some(target);
    }
    resolved
}

fn authored_member_target(
    program: &TypedTrees,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let receiver_type = authored_operand_type(program, member.receiver)?;
    member_symbol_from_type_reference(program, receiver_type, member.member.as_str())
        .and_then(declaration_target)
        .or_else(|| collection_measure_target(program, receiver_type, member.member.as_str()))
}

fn contextual_statement_member_target(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let mut resolved = None;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut expressions = Vec::new();
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                if !expressions.contains(&expression) {
                    continue;
                }

                let receiver_type = crate::flow::expression_type_reference_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    member.receiver,
                )?;
                let target = member_symbol_from_type_reference(
                    program,
                    receiver_type,
                    member.member.as_str(),
                )
                .and_then(declaration_target)
                .or_else(|| {
                    collection_measure_target(program, receiver_type, member.member.as_str())
                })?;
                if resolved.is_some_and(|candidate| candidate != target) {
                    return None;
                }
                resolved = Some(target);
            }
        }
    }
    resolved
}

fn member_symbol_from_type_reference(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    use typed_trees::types::TypeReferenceNode;

    let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            return member_symbol_from_type_reference(program, *referee, member_name);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            return member_symbol_from_type_reference(program, *base_type, member_name);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => (*base_symbol, base_name),
        TypeReferenceNode::Named { symbol, name } => (*symbol, name),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => return None,
    };
    let data = program.data_definitions().iter().find(|definition| {
        (symbol.is_valid() && definition.symbol == symbol) || definition.name == *name
    })?;
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == member_name => {
                Some(field.symbol)
            }
            typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == member_name =>
            {
                Some(variant.symbol)
            }
            typed_trees::data::DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .find_map(|field| (field.name.as_str() == member_name).then_some(field.symbol)),
            _ => None,
        })
}

/// A `len` or `capacity` member on a fixed array or slice selects the
/// compiler-owned standing measure of that name; neither is a package
/// declaration. Anything else yields no intrinsic target.
fn collection_measure_target(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    member_name: &str,
) -> Option<CheckedResolutionTarget> {
    if !type_reference_is_collection(program, type_reference) {
        return None;
    }
    let intrinsic = match member_name {
        "len" => AuthoredDeclarationSelectionIntrinsic::CollectionLength,
        "capacity" => AuthoredDeclarationSelectionIntrinsic::CollectionCapacity,
        _ => return None,
    };
    Some(CheckedResolutionTarget::Intrinsic(intrinsic))
}

pub(crate) fn type_reference_is_collection(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    pub(crate) use typed_trees::types::TypeReferenceNode;
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_is_collection(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_collection(program, *base_type)
        }
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => true,
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => false,
    }
}

pub(crate) fn expression_is_contextual_domain_primitive(
    program: &TypedTrees,
    containing_expression: typed_trees::expression::ExpressionHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(target_type) = contextual_domain_target_type(program, containing_expression) else {
        return false;
    };
    contextual_expression_type_reference(program, expression, target_type)
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
}

pub(crate) fn expression_is_contextual_statement_primitive(
    program: &TypedTrees,
    containing_expression: typed_trees::expression::ExpressionHandle,
    operand: typed_trees::expression::ExpressionHandle,
) -> bool {
    let mut found = false;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut expressions = Vec::new();
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                if !expressions.contains(&containing_expression) {
                    continue;
                }

                let Some(type_reference) = crate::flow::expression_type_reference_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    operand,
                ) else {
                    return false;
                };
                if program.primitive_type_reference(type_reference).is_none() {
                    return false;
                }
                found = true;
            }
        }
    }
    found
}

fn contextual_expression_type_reference(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    domain_target_type: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if contextual_self_path(program, path) => {
            Some(domain_target_type)
        }
        ExpressionNode::Member(member) => {
            contextual_self_member_symbol(program, member, domain_target_type)
                .and_then(|symbol| type_reference_for_symbol(program, symbol))
        }
        ExpressionNode::Borrow(inner) => {
            contextual_expression_type_reference(program, inner.target, domain_target_type)
        }
        _ => None,
    }
}

fn contextual_self_member_symbol(
    program: &TypedTrees,
    member: &typed_trees::expression::TableMemberExpression,
    domain_target_type: typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver) else {
        return None;
    };
    if !contextual_self_path(program, path) {
        return None;
    }
    crate::flow::resolve_member_symbol_from_type_symbol(
        program,
        program.type_reference_table.type_symbol(domain_target_type),
        member.member.as_str(),
    )
}

fn contextual_self_path(
    program: &TypedTrees,
    path: &typed_trees::expression::TableNamePath,
) -> bool {
    let members = program.expression_table.name_path_members(path.members);
    !path.symbol.is_valid() && members.len() == 1 && members[0].as_str() == "self"
}

fn contextual_domain_target_type(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    for domain in program.domain_definitions() {
        for fact in program.proof_facts(domain) {
            let root = match fact {
                typed_trees::domain::ProofFact::Expression(root) => *root,
                typed_trees::domain::ProofFact::Membership(membership) => membership.value,
                typed_trees::domain::ProofFact::Proposition(_) => continue,
            };
            if expression_contains(program, root, expression, &mut Vec::new()) {
                return Some(domain.target_type);
            }
        }
    }
    None
}

pub(crate) fn expression_contains(
    program: &TypedTrees,
    root: typed_trees::expression::ExpressionHandle,
    target: typed_trees::expression::ExpressionHandle,
    visited: &mut Vec<typed_trees::expression::ExpressionHandle>,
) -> bool {
    if !root.is_valid() || visited.contains(&root) {
        return false;
    }
    if root == target {
        return true;
    }
    visited.push(root);
    match program.expression_table.expression(root) {
        ExpressionNode::Match(dispatch) => {
            expression_contains(program, dispatch.subject, target, visited)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        (matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern)
                        if expression_contains(program, pattern, target, visited)))
                            || expression_contains(program, arm.value, target, visited)
                    })
        }
        ExpressionNode::Atomic(atomic) => {
            expression_contains(program, atomic.value, target, visited)
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|child| expression_contains(program, *child, target, visited)),
        ExpressionNode::Binary(binary) => {
            expression_contains(program, binary.left, target, visited)
                || expression_contains(program, binary.right, target, visited)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_contains(program, call.receiver, target, visited))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|child| expression_contains(program, *child, target, visited))
        }
        ExpressionNode::Cast(cast) => expression_contains(program, cast.value, target, visited),
        ExpressionNode::Indexed(indexed) => {
            expression_contains(program, indexed.collection, target, visited)
                || expression_contains(program, indexed.index, target, visited)
        }
        ExpressionNode::Member(member) => {
            expression_contains(program, member.receiver, target, visited)
        }
        ExpressionNode::Borrow(inner) => {
            expression_contains(program, inner.target, target, visited)
        }
        ExpressionNode::Range(range) => {
            expression_contains(program, range.start, target, visited)
                || expression_contains(program, range.end, target, visited)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_contains(program, field.value, target, visited)),
        ExpressionNode::Unary(unary) => {
            expression_contains(program, unary.operand, target, visited)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

pub(crate) fn expression_is_intrinsic_primitive_without_origin(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let type_reference = match program.tables.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) | ExpressionNode::Float(_) | ExpressionNode::Integer(_) => {
            return true;
        }
        ExpressionNode::Name(path) => type_reference_for_symbol(program, path.symbol),
        ExpressionNode::Call(call) => program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .find_map(|state| (state.symbol == call.target_symbol).then_some(state.return_type)),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Member(member) => {
            if matches!(
                contexts::checked_member_target_from_exact_owner(
                    program,
                    &CheckFacts::default(),
                    expression,
                    member,
                ),
                Some(
                    contexts::OwnerMemberTarget::CollectionLength
                        | contexts::OwnerMemberTarget::CollectionCapacity
                )
            ) {
                return true;
            }
            type_reference_for_symbol(
                program,
                crate::flow::effective_member_symbol(program, member.receiver, member),
            )
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::And
                    | typed_trees::expression::BinaryOperator::BitwiseAnd
                    | typed_trees::expression::BinaryOperator::BitwiseOr
                    | typed_trees::expression::BinaryOperator::BitwiseXor
                    | typed_trees::expression::BinaryOperator::Or
                    | typed_trees::expression::BinaryOperator::ShiftLeft
                    | typed_trees::expression::BinaryOperator::ShiftRight
                    | typed_trees::expression::BinaryOperator::CaseMembership
            ) =>
        {
            return true;
        }
        ExpressionNode::Binary(binary) => {
            use language_core::OperatorSpelling;
            use typed_trees::expression::BinaryOperator;

            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                BinaryOperator::Equal => OperatorSpelling::Equal,
                BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                BinaryOperator::And
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::Or
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::CaseMembership => unreachable!("handled above"),
            };
            let operand_types = [
                authored_operand_type(program, binary.left),
                authored_operand_type(program, binary.right),
            ];
            if operand_types.iter().all(Option::is_none)
                || !typed_trees::operator::resolve_spelling_for_operands(
                    program,
                    spelling,
                    &operand_types,
                )
                .is_empty()
            {
                return false;
            }
            return expression_is_intrinsic_primitive_without_origin(program, binary.left)
                || expression_is_intrinsic_primitive_without_origin(program, binary.right);
        }
        ExpressionNode::Unary(_) => return true,
        ExpressionNode::Borrow(inner) => {
            return expression_is_intrinsic_primitive_without_origin(program, inner.target);
        }
        _ => None,
    };
    type_reference
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
}
