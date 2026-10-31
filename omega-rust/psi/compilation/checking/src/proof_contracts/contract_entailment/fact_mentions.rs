//! Whether an ensures fact mentions proof-only data or the zero value.

use crate::frozen_program::expression_table;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

/// Does this contract conjunct SPEAK ABOUT proof-only data? Structural
/// detection over the expression tree: a machine parameter (or the `result`
/// atom) whose declared type mentions proof-only data, a classifier or case
/// literal naming a proof-only definition (`Nat::Zero`,
/// `Nat::Succ { .. }`), or a call whose target machine returns one. Returns
/// the named proof-only type for the diagnostic. Used by the ensures fence
/// above: such conjuncts are outside every judging tier today, and standing
/// down would silently certify them (math roster N3 owns the real tier).
pub(crate) fn fact_mentions_proof_only_data(
    program: &dyn crate::CheckingInput,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<typed_trees::name::Identifier> {
    if !expression.is_valid() {
        return None;
    }
    let recurse = |handle: ExpressionHandle| {
        fact_mentions_proof_only_data(program, classification, machine, handle)
    };
    let proof_only_definition = |name: &str| {
        program
            .data_definition_by_name_str(name)
            .filter(|definition| classification.is_proof_only(definition.symbol))
            .map(|definition| definition.name.clone())
    };
    let node = expression_table(program).expression(expression);
    match node {
        ExpressionNode::Match(dispatch) => {
            crate::expression_traversal::match_children(program, *dispatch).find_map(recurse)
        }
        ExpressionNode::Atomic(atomic) => recurse(atomic.value),
        ExpressionNode::Name(path) => {
            let members = expression_table(program).name_path_members(path.members);
            match members {
                [single] => {
                    let entry = program.machine_states(machine).first()?;
                    if expression_table(program).names_reserved_result(expression) {
                        if entry.return_type.is_valid() {
                            return classification.proof_only_mention(program, entry.return_type);
                        }
                        return None;
                    }
                    program
                        .state_parameters(entry)
                        .iter()
                        .find(|parameter| parameter.name.as_str() == single.as_str())
                        .and_then(|parameter| {
                            classification.proof_only_mention(program, parameter.type_reference)
                        })
                }
                [first, ..] => proof_only_definition(first.as_str()),
                [] => None,
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            proof_only_definition(struct_literal.type_name.as_str()).or_else(|| {
                expression_table(program)
                    .struct_fields(struct_literal.fields)
                    .iter()
                    .find_map(|field| recurse(field.value))
            })
        }
        ExpressionNode::Call(call) => {
            typed_trees::operator::resolve_named_expression_call(program, call)
                // A selected operator can return proof-only data without owning
                // a source machine body. Classify its actual return type before
                // searching ordinary machine calls or their operands.
                .and_then(|operator| {
                    classification.proof_only_mention(program, operator.return_type)
                })
                .or_else(|| {
                    program
                        .machines()
                        .iter()
                        .find(|target| {
                            target.attached_data.is_none()
                                && target.name.as_str() == call.target.as_str()
                        })
                        .and_then(|target| {
                            let entry = program.machine_states(target).first()?;
                            if !entry.return_type.is_valid() {
                                return None;
                            }
                            classification.proof_only_mention(program, entry.return_type)
                        })
                })
                .or_else(|| recurse(call.receiver))
                .or_else(|| {
                    expression_table(program)
                        .expression_handles(call.arguments)
                        .iter()
                        .find_map(|argument| recurse(*argument))
                })
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => None,
        ExpressionNode::ZeroValue(type_reference) => {
            classification.proof_only_mention(program, *type_reference)
        }
        _ => find_in_composite_children(program, node, recurse),
    }
}

/// `find_map` over the standard composite children — atomic, binary, unary,
/// cast, member, borrow, indexed, range, array literal — shared by the
/// contract-mention searches; non-composite nodes return `None`.
fn find_in_composite_children<T>(
    program: &dyn crate::CheckingInput,
    node: &ExpressionNode,
    recurse: impl Fn(ExpressionHandle) -> Option<T>,
) -> Option<T> {
    match node {
        ExpressionNode::Atomic(atomic) => recurse(atomic.value),
        ExpressionNode::Binary(binary) => recurse(binary.left).or_else(|| recurse(binary.right)),
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        ExpressionNode::Cast(cast) => recurse(cast.value),
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Borrow(inner) => recurse(inner.target),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection).or_else(|| recurse(indexed.index))
        }
        ExpressionNode::Range(range) => recurse(range.start).or_else(|| recurse(range.end)),
        ExpressionNode::ArrayLiteral(items) => expression_table(program)
            .expression_handles(*items)
            .iter()
            .find_map(|item| recurse(*item)),
        _ => None,
    }
}

/// Return the observed data name when a contract fact contains
/// `zero_value<T>()`. Unlike the proof-only-data fence above, this route also
/// covers ordinary runtime data: the observation is structural because its
/// meaning comes from authored home representation, not from integer
/// arithmetic.
pub(crate) fn fact_mentions_zero_value(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<String> {
    use typed_trees::types::TypeReferenceNode;

    if !expression.is_valid() {
        return None;
    }
    let recurse = |handle: ExpressionHandle| fact_mentions_zero_value(program, handle);
    let node = expression_table(program).expression(expression);
    match node {
        ExpressionNode::Match(dispatch) => {
            crate::expression_traversal::match_children(program, *dispatch).find_map(recurse)
        }
        ExpressionNode::StructLiteral(literal) => expression_table(program)
            .struct_fields(literal.fields)
            .iter()
            .find_map(|field| recurse(field.value)),
        ExpressionNode::Call(call) => recurse(call.receiver).or_else(|| {
            expression_table(program)
                .expression_handles(call.arguments)
                .iter()
                .find_map(|argument| recurse(*argument))
        }),
        ExpressionNode::ZeroValue(type_reference) => {
            let observed = *type_reference;
            let mut current = *type_reference;
            loop {
                match program.type_reference_table.type_reference(current) {
                    TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                    TypeReferenceNode::Generic { base_name, .. } => {
                        return Some(base_name.as_str().to_owned());
                    }
                    TypeReferenceNode::Named { name, .. } => {
                        return Some(name.as_str().to_owned());
                    }
                    TypeReferenceNode::Reference { .. }
                    | TypeReferenceNode::FixedArray { .. }
                    | TypeReferenceNode::Slice { .. }
                    | TypeReferenceNode::DynamicTrait { .. }
                    | TypeReferenceNode::ConstExpression(_)
                    | TypeReferenceNode::Unit => {
                        return Some(program.display_type_reference(observed));
                    }
                }
            }
        }
        typed_trees::executable_leaf_expression_pattern!() => None,
        _ => find_in_composite_children(program, node, recurse),
    }
}
