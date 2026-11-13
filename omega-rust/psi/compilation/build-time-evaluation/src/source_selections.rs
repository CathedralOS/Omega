//! Source-issued declaration candidates used only to check compiler-evaluation
//! selection authority. The sole lowered checker still chooses and judges the
//! executable meaning; these queries provide no body or proof evidence.

use arena::Arena;
use symbols::SymbolHandle;
use typed_trees::{
    domain::ProofFact,
    expression::{ExpressionHandle, ExpressionNode, TableNamePath},
    operator::{
        OperandType, SpelledOperator, binary_operator_spelling, resolve_spelling,
        resolve_spelling_for_operand_types,
    },
    types::TypeReferenceHandle,
};
use validation::expression_owners::{
    OwnerMemberTarget, checked_member_target_from_exact_owner, expression_contains,
};
use validation::selection_resolution::exact_named_operator_call;
use validation::{declaration_type_reference, effective_member_symbol};

pub(crate) fn expression_is_intrinsic_primitive_without_origin(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    let type_reference = match program.tables.expression_table.expression(expression) {
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_) => {
            return true;
        }
        ExpressionNode::Name(path) => declaration_type_reference(program, path.symbol),
        ExpressionNode::Call(call) => program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .find_map(|state| (state.symbol == call.target_symbol).then_some(state.return_type)),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Member(member) => {
            if matches!(
                checked_member_target_from_exact_owner(
                    program,
                    &Arena::default(),
                    expression,
                    member,
                    None,
                ),
                Some(OwnerMemberTarget::CollectionMeasure(_))
            ) {
                return true;
            }
            declaration_type_reference(
                program,
                effective_member_symbol(program, member.receiver, member),
            )
        }
        ExpressionNode::Binary(binary) if binary_operator_spelling(binary.operator).is_none() => {
            return true;
        }
        ExpressionNode::Binary(binary) => {
            let Some(spelling) = binary_operator_spelling(binary.operator) else {
                unreachable!("handled above")
            };
            let operand_types = [
                authored_operand_descriptor(program, binary.left),
                authored_operand_descriptor(program, binary.right),
            ];
            if operand_types.iter().all(|operand| operand.is_unknown()) {
                return expression_is_anonymous_arithmetic(program, binary.left)
                    && expression_is_anonymous_arithmetic(program, binary.right);
            }
            if !resolve_spelling_for_operand_types(
                program,
                spelling,
                &operand_types,
                program.tables.expression_table.source_span(expression),
            )
            .is_empty()
            {
                return false;
            }
            return operand_types
                .iter()
                .any(|operand| matches!(operand, OperandType::Primitive(_)))
                || expression_is_intrinsic_primitive_without_origin(program, binary.left)
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

/*
    Whether an expression is anonymous arithmetic: a literal that has not
    landed (no width suffix and no captured carrier), or arithmetic whose
    operands are all anonymous. Such an expression carries no type of its own
    until it lands, so no authored operator spelling applies to it.
*/
fn expression_is_anonymous_arithmetic(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    use typed_trees::expression::BinaryOperator;
    match program.tables.expression_table.expression(expression) {
        ExpressionNode::Float(literal) => literal.landing().is_none(),
        ExpressionNode::Integer(literal) => literal.landing().is_none(),
        ExpressionNode::Binary(binary) => {
            matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
            ) && expression_is_anonymous_arithmetic(program, binary.left)
                && expression_is_anonymous_arithmetic(program, binary.right)
        }
        _ => false,
    }
}

pub(crate) fn authored_operand_descriptor(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> typed_trees::operator::OperandType {
    use typed_trees::operator::OperandType;
    if let Some(type_reference) = authored_operand_type(program, expression) {
        return OperandType::Reference(type_reference);
    }
    authored_operand_carrier(program, expression)
        .map_or(OperandType::Unknown, OperandType::Primitive)
}

/*
    The primitive carrier an operand's VALUE has, for an operand that names no
    type reference. An operation's own result is such a value: a comparison
    produces `bool`, and an arithmetic or bitwise operation produces the
    carrier its operands share.

    Each answer holds only where no authored operator supplies a different
    meaning: a declared `less` may return a mask, a declared `add` may return
    another carrier entirely. Candidates that ALL return the same carrier
    settle it whichever one is selected, and no candidate at all leaves the
    builtin operation.
*/
fn authored_operand_carrier(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    use typed_trees::operator::OperandType;
    use typed_trees::types::PrimitiveType;

    fn operand_carrier(
        program: &typed_trees::TypedTrees,
        operand: OperandType,
    ) -> Option<typed_trees::types::PrimitiveType> {
        match operand {
            OperandType::Reference(reference) => program.primitive_type_reference(reference),
            OperandType::Primitive(primitive) => Some(primitive),
            OperandType::Unknown => None,
        }
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) => Some(PrimitiveType::Bool),
        ExpressionNode::Binary(binary) => {
            if matches!(
                binary.operator,
                BinaryOperator::And | BinaryOperator::Or | BinaryOperator::CaseMembership
            ) {
                return Some(PrimitiveType::Bool);
            }
            let left = authored_operand_descriptor(program, binary.left);
            let right = authored_operand_descriptor(program, binary.right);
            let carrier = match binary.operator {
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual => PrimitiveType::Bool,
                _ => match (
                    operand_carrier(program, left),
                    operand_carrier(program, right),
                ) {
                    (Some(left), Some(right)) if left == right => left,
                    (Some(carrier), None) | (None, Some(carrier)) => carrier,
                    _ => return None,
                },
            };
            candidate_operators(program, expression, &[left, right])
                .iter()
                .all(|candidate| {
                    program.primitive_type_reference(candidate.operator.return_type)
                        == Some(carrier)
                })
                .then_some(carrier)
        }
        ExpressionNode::Unary(unary) => match unary.operator {
            UnaryOperator::LogicalNot => Some(PrimitiveType::Bool),
            UnaryOperator::BitwiseNot => {
                operand_carrier(program, authored_operand_descriptor(program, unary.operand))
            }
        },
        _ => None,
    }
}

pub(crate) fn authored_operand_type(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => {
            program
                .closed_integer_value_in(expression, SymbolHandle::invalid())?
                .type_reference
        }
        ExpressionNode::Atomic(atomic) => authored_operand_type(program, atomic.value),
        ExpressionNode::Borrow(borrow) => authored_operand_type(program, borrow.target),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Member(member) => declaration_type_reference(
            program,
            effective_member_symbol(program, member.receiver, member),
        ),
        ExpressionNode::Call(call) => {
            exact_named_operator_call(program, call).map(|operator| operator.return_type)
        }
        ExpressionNode::Name(path) => declaration_type_reference(program, path.symbol)
            .or_else(|| operator_contract_value_type(program, expression, path)),
        // An element of an array literal has the literal's element type,
        // which every element shares; a contract such as
        // `requires [3u64][0] <= 7` names no other typed operand.
        ExpressionNode::Indexed(indexed)
            if !matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) =>
        {
            let ExpressionNode::ArrayLiteral(elements) =
                program.expression_table.expression(indexed.collection)
            else {
                return None;
            };
            program
                .expression_table
                .expression_handles(*elements)
                .iter()
                .find_map(|element| authored_operand_type(program, *element))
        }
        _ => None,
    }
}

fn operator_contract_value_type(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    path: &TableNamePath,
) -> Option<TypeReferenceHandle> {
    let name = program
        .expression_table
        .name_path_members(path.members)
        .last()?
        .as_str();
    let mut types = program.all_operators().filter_map(|operator| {
        let type_reference = if language_core::is_reserved_result(name) {
            operator.return_type
        } else {
            program
                .operator_parameters(operator)
                .iter()
                .find(|parameter| parameter.name.as_str() == name)?
                .type_reference
        };
        program
            .operator_contracts(operator)
            .iter()
            .any(|contract| {
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .any(|fact| match fact {
                        ProofFact::Expression(root) => {
                            expression_contains(program, *root, expression)
                        }
                        ProofFact::Membership(membership) => {
                            expression_contains(program, membership.value, expression)
                        }
                        ProofFact::Proposition(_) => false,
                    })
            })
            .then_some(type_reference)
    });
    let first = types.next()?;
    types
        .all(|type_reference| type_reference == first)
        .then_some(first)
}

fn operator_has_no_authored_spelling_candidate(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    node: &ExpressionNode,
) -> bool {
    use language_core::OperatorSpelling;
    let spelling = match node {
        ExpressionNode::Binary(binary) => {
            let Some(spelling) = binary_operator_spelling(binary.operator) else {
                return true;
            };
            spelling
        }
        ExpressionNode::Indexed(indexed) => {
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
                OperatorSpelling::Range
            } else {
                OperatorSpelling::Index
            }
        }
        ExpressionNode::Unary(_) => return true,
        _ => return false,
    };
    resolve_spelling(
        program,
        spelling,
        None,
        program.expression_table.source_span(expression),
    )
    .is_empty()
}

/*
    Return whether an early typed operator expression cannot select an authored
    operator declaration. Ordinary checked lowering remains responsible for
    rejecting a semantically invalid builtin use.
*/
pub(crate) fn typed_operator_has_no_authored_selection(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    let node = program.expression_table.expression(expression);
    operator_has_no_authored_spelling_candidate(program, expression, node)
        || (matches!(node, ExpressionNode::Binary(_))
            && expression_is_intrinsic_primitive_without_origin(program, expression))
}

pub(crate) fn typed_operator_authored_selection_candidates(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Vec<SymbolHandle> {
    candidate_operators(program, expression, &[])
        .into_iter()
        .map(|candidate| candidate.operator.symbol)
        .collect()
}

/*
    The declared operator candidates of `expression`, for a caller that
    already holds a BINARY expression's two operand descriptors. Deriving them
    again would walk each operand once per enclosing operation, which turns a
    nested arithmetic expression's carrier into an exponential walk.
*/
fn candidate_operators<'program>(
    program: &'program typed_trees::TypedTrees,
    expression: ExpressionHandle,
    binary_operands: &[typed_trees::operator::OperandType],
) -> Vec<SpelledOperator<'program>> {
    use language_core::OperatorSpelling;

    let (spelling, operand_types) = match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let Some(spelling) = binary_operator_spelling(binary.operator) else {
                return Vec::new();
            };
            (
                spelling,
                match binary_operands {
                    [left, right] => vec![*left, *right],
                    _ => vec![
                        authored_operand_descriptor(program, binary.left),
                        authored_operand_descriptor(program, binary.right),
                    ],
                },
            )
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = authored_operand_descriptor(program, indexed.collection);
            match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => (
                    OperatorSpelling::Range,
                    vec![
                        collection,
                        authored_operand_descriptor(program, range.start),
                        authored_operand_descriptor(program, range.end),
                    ],
                ),
                _ => (
                    OperatorSpelling::Index,
                    vec![
                        collection,
                        authored_operand_descriptor(program, indexed.index),
                    ],
                ),
            }
        }
        ExpressionNode::Unary(_) => return Vec::new(),
        _ => return Vec::new(),
    };

    resolve_spelling_for_operand_types(
        program,
        spelling,
        &operand_types,
        program.expression_table.source_span(expression),
    )
}
pub(crate) fn exact_owner_member_declaration(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let member = program.expression_table.member_node(expression)?;
    if member.member_symbol.is_valid() {
        return Some(member.member_symbol);
    }
    match validation::expression_owners::checked_member_target_from_exact_owner(
        program,
        &arena::Arena::default(),
        expression,
        member,
        None,
    )? {
        validation::expression_owners::OwnerMemberTarget::Declaration(symbol) => Some(symbol),
        validation::expression_owners::OwnerMemberTarget::CollectionMeasure(_) => None,
    }
}
