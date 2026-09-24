//! Contextual domain, value and statement member targets.

use std::collections::HashSet;

use crate::authored_selections::CheckedResolutionTarget;
use crate::authored_selections::call_targets::declaration_target;
use crate::authored_selections::contexts;
use crate::authored_selections::operator_targets::{
    authored_operand_descriptor, authored_operand_type, type_reference_for_symbol,
};
use crate::semantic::calls::MeasureReceiver;
use checked_trees::CheckFacts;
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
                contexts::OwnerMemberTarget::CollectionMeasure(measure) => {
                    CheckedResolutionTarget::Intrinsic(measure.intrinsic())
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
        .or_else(|| collection_measure_target(program, value.type_reference, member));
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
        .or_else(|| collection_measure_target(program, receiver_type, member))
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
                .or_else(|| collection_measure_target(program, receiver_type, member))?;
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
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let measure = crate::semantic::calls::collection_measure_member(
        program,
        member,
        MeasureReceiver::Declared(type_reference),
    )?;
    Some(CheckedResolutionTarget::Intrinsic(measure.intrinsic()))
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
    !path.symbol.is_valid() && members.len() == 1 && members[0].is_self_receiver()
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
            if expression_contains(program, root, expression) {
                return Some(domain.target_type);
            }
        }
    }
    None
}

/// Whether the containment walk from `root` reaches `target`.
pub(crate) fn expression_contains(
    program: &TypedTrees,
    root: typed_trees::expression::ExpressionHandle,
    target: typed_trees::expression::ExpressionHandle,
) -> bool {
    reaches(program, root, target, &mut HashSet::new())
}

/// Every expression the containment walk reaches from `root`.
///
/// `expression_contains(program, root, target)` holds exactly when this set
/// contains `target`: the walk records each expression it enters before
/// comparing it, and an address no expression occupies never ends the walk
/// early, so the recorded set is the whole reachable set. A caller asking
/// the same containment question of many targets indexes this once instead
/// of rewalking `root` per target.
pub(crate) fn reachable_expressions(
    program: &TypedTrees,
    root: typed_trees::expression::ExpressionHandle,
) -> HashSet<typed_trees::expression::ExpressionHandle> {
    let mut reached = HashSet::new();
    // Arena index zero is the invalid address; `is_valid` rejects it below,
    // so no recorded expression can equal it.
    reaches(
        program,
        root,
        typed_trees::expression::ExpressionHandle::invalid(),
        &mut reached,
    );
    reached
}

/// The shared walk: a membership set, never an ordered history, so an
/// expression reached twice through different parents is entered once.
fn reaches(
    program: &TypedTrees,
    root: typed_trees::expression::ExpressionHandle,
    target: typed_trees::expression::ExpressionHandle,
    visited: &mut HashSet<typed_trees::expression::ExpressionHandle>,
) -> bool {
    if !root.is_valid() || !visited.insert(root) {
        return false;
    }
    if root == target {
        return true;
    }
    match program.expression_table.expression(root) {
        ExpressionNode::Match(dispatch) => {
            reaches(program, dispatch.subject, target, visited)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        (matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern)
                        if reaches(program, pattern, target, visited)))
                            || reaches(program, arm.value, target, visited)
                    })
        }
        ExpressionNode::Atomic(atomic) => reaches(program, atomic.value, target, visited),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|child| reaches(program, *child, target, visited)),
        ExpressionNode::Binary(binary) => {
            reaches(program, binary.left, target, visited)
                || reaches(program, binary.right, target, visited)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && reaches(program, call.receiver, target, visited))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|child| reaches(program, *child, target, visited))
        }
        ExpressionNode::Cast(cast) => reaches(program, cast.value, target, visited),
        ExpressionNode::Indexed(indexed) => {
            reaches(program, indexed.collection, target, visited)
                || reaches(program, indexed.index, target, visited)
        }
        ExpressionNode::Member(member) => reaches(program, member.receiver, target, visited),
        ExpressionNode::Borrow(inner) => reaches(program, inner.target, target, visited),
        ExpressionNode::Range(range) => {
            reaches(program, range.start, target, visited)
                || reaches(program, range.end, target, visited)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| reaches(program, field.value, target, visited)),
        ExpressionNode::Unary(unary) => reaches(program, unary.operand, target, visited),
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
                Some(contexts::OwnerMemberTarget::CollectionMeasure(_))
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
                authored_operand_descriptor(program, binary.left),
                authored_operand_descriptor(program, binary.right),
            ];
            if operand_types.iter().all(|operand| operand.is_unknown())
                || !typed_trees::operator::resolve_spelling_for_operand_types(
                    program,
                    spelling,
                    &operand_types,
                )
                .is_empty()
            {
                return false;
            }
            // An operand whose carrier is known without a type reference --
            // a comparison result, a boolean literal -- answers this question
            // directly; it is exactly a builtin primitive with nothing to
            // point at.
            return operand_types.iter().any(|operand| {
                matches!(operand, typed_trees::operator::OperandType::Primitive(_))
            }) || expression_is_intrinsic_primitive_without_origin(program, binary.left)
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

#[cfg(test)]
mod tests {
    use super::{expression_contains, reachable_expressions};
    use crate::tests::front_end::typed_program;
    use typed_trees::TypedTrees;

    fn fixture() -> TypedTrees {
        let source = r#"
            data Pair { left: u16; right: u16; }
            proposition related(value: u16);
            machine Pair::combine(&self, other: u16) -> u16
                requires other <= 100u16
            {
                let scaled: u16 = ((left + right) * other) - (left + right);
                let chosen: u16 = (scaled + left) - (right + other);
                scaled + chosen
            }
        "#;
        typed_program(source)
    }

    /// The indexed reachable set answers exactly the containment question the
    /// per-target walk answers, for every ordered pair the program records.
    #[test]
    fn reachable_expressions_agree_with_containment_for_every_pair() {
        let program = fixture();
        let expressions = program
            .expression_table
            .iter_expressions()
            .map(|(expression, _)| expression)
            .collect::<Vec<_>>();
        assert!(
            expressions.len() > 20,
            "fixture records enough expressions, not {}",
            expressions.len()
        );
        let mut reached_any = false;
        for root in expressions.iter().copied() {
            let reached = reachable_expressions(&program, root);
            reached_any |= reached.len() > 1;
            for target in expressions.iter().copied() {
                assert_eq!(
                    reached.contains(&target),
                    expression_contains(&program, root, target),
                    "containment disagreed for {root:?} and {target:?}",
                );
            }
        }
        assert!(reached_any, "fixture reaches beyond single expressions");
    }

    /// The unmatchable address the reachable walk uses is never recorded, so
    /// it cannot end that walk early.
    #[test]
    fn no_recorded_expression_holds_the_invalid_address() {
        let program = fixture();
        let invalid = typed_trees::expression::ExpressionHandle::invalid();
        for (expression, _) in program.expression_table.iter_expressions() {
            assert_ne!(expression, invalid);
            assert!(expression.is_valid());
        }
    }
}
