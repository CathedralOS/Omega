//! Type inference over authored expressions: the inferred nominal symbol,
//! collection view, and member resolution each environment can answer.

use super::{InferredType, OwnerMemberTarget, TypeEnvironment};
use crate::collection_operations::MeasureReceiver;
use language_semantics::declaration_selection::CollectionMeasure;
/// Handle-keyed indexes and reach sets take the integer hasher.
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn type_reference_contains_expression(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    expression: ExpressionHandle,
    visited: &mut Vec<TypeReferenceHandle>,
) -> bool {
    if !type_reference.is_valid() || visited.contains(&type_reference) {
        return false;
    }
    visited.push(type_reference);
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_contains_expression(program, *referee, expression, visited)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            type_reference_contains_expression(program, *base_type, expression, visited)
                || program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| match constraint {
                        TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } => {
                            super::expression_contains(program, *minimum, expression)
                                || super::expression_contains(program, *maximum, expression)
                        }
                        TypeConstraintNode::Domain(domain) => {
                            domain.arguments.iter().any(|argument| {
                                type_reference_contains_expression(
                                    program, *argument, expression, visited,
                                )
                            })
                        }
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {
                            false
                        }
                    })
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            type_reference_contains_expression(program, *element_type, expression, visited)
                || matches!(length, typed_trees::types::FixedArrayLength::Expression(value)
                    if type_reference_contains_expression(program, *value, expression, visited))
        }
        TypeReferenceNode::Slice { element_type } => {
            type_reference_contains_expression(program, *element_type, expression, visited)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| {
                type_reference_contains_expression(program, *argument, expression, visited)
            }),
        TypeReferenceNode::ConstExpression(root) => {
            super::expression_contains(program, *root, expression)
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => false,
    }
}

pub(super) fn member_target_in_environment(
    program: &dyn crate::CheckingInput,
    member: &TableMemberExpression,
    environment: &TypeEnvironment,
    source_span: source::SourceSpan,
) -> Option<OwnerMemberTarget> {
    let receiver_type =
        infer_expression_type(program, member.receiver, environment, &mut Vec::new())?;
    resolve_member_symbol(program, receiver_type, member)
        .or_else(|| {
            resolve_member_symbol_from_authored_source(program, receiver_type, member, source_span)
        })
        .map(OwnerMemberTarget::Declaration)
        .or_else(|| {
            collection_measure_in_environment(program, member, receiver_type)
                .map(OwnerMemberTarget::CollectionMeasure)
        })
}

/// The measure a member selects on its inferred receiver, asked of
/// [`crate::collection_operations::collection_measure_member`] in that query's
/// receiver vocabulary. A nominal or compiler-text receiver has no measure.
pub(super) fn collection_measure_in_environment(
    program: &dyn crate::CheckingInput,
    member: &TableMemberExpression,
    receiver_type: InferredType,
) -> Option<CollectionMeasure> {
    let receiver = match receiver_type {
        InferredType::TypeReference(type_reference) => MeasureReceiver::Declared(type_reference),
        InferredType::CollectionView(_) => MeasureReceiver::Window,
        InferredType::Nominal(_) | InferredType::CompilerString => return None,
    };
    crate::collection_operations::collection_measure_member(program, member, receiver)
}

pub(super) fn resolve_member_symbol_from_authored_source(
    program: &dyn crate::CheckingInput,
    receiver_type: InferredType,
    member: &TableMemberExpression,
    source_span: source::SourceSpan,
) -> Option<SymbolHandle> {
    let InferredType::TypeReference(type_reference) = receiver_type else {
        return None;
    };
    let type_name = unresolved_nominal_type_name(program, type_reference)?;
    let nominal = program
        .symbols
        .find_top_level_by_name_and_kinds_from_source(
            type_name,
            &[symbols::SymbolKind::Data],
            source_span,
        )?;
    resolve_member_symbol(program, InferredType::Nominal(nominal), member)
}

pub(super) fn unresolved_nominal_type_name(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<&str> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => unresolved_nominal_type_name(program, *referee),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } if !base_symbol.is_valid() => Some(base_name.as_str()),
        TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => Some(name.as_str()),
        _ => None,
    }
}

pub(super) fn infer_expression_type(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    environment: &TypeEnvironment,
    visited: &mut Vec<ExpressionHandle>,
) -> Option<InferredType> {
    if !expression.is_valid() || visited.contains(&expression) {
        return None;
    }
    visited.push(expression);
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            let mut result = None;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let arm_type =
                    infer_expression_type(program, arm.value, environment, &mut visited.clone())?;
                if result.is_some_and(|previous| previous != arm_type) {
                    return None;
                }
                result = Some(arm_type);
            }
            result
        }
        ExpressionNode::Atomic(atomic) => {
            infer_expression_type(program, atomic.value, environment, visited)
        }
        ExpressionNode::Borrow(inner) => {
            infer_expression_type(program, inner.target, environment, visited)
        }
        ExpressionNode::Cast(cast) => Some(InferredType::TypeReference(cast.target_type)),
        ExpressionNode::Name(path) => {
            if path.symbol.is_valid()
                && let Some(type_reference) =
                    crate::declaration_type_reference(program, path.symbol)
            {
                return Some(InferredType::TypeReference(type_reference));
            }
            let [name] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            if !path.symbol.is_valid() && !path.head_symbol.is_valid() && name.is_self_receiver() {
                return environment.self_type;
            }
            if program.expression_table.names_reserved_result(expression) {
                return environment.result_type.map(InferredType::TypeReference);
            }
            let mut matches = environment
                .bindings
                .iter()
                .filter(|binding| {
                    (path.head_symbol.is_valid() && binding.symbol == path.head_symbol)
                        || (path.symbol.is_valid() && binding.symbol == path.symbol)
                        || (!path.head_symbol.is_valid()
                            && !path.symbol.is_valid()
                            && binding.name == name.as_str())
                })
                .map(|binding| binding.type_reference);
            let first = matches.next()?;
            matches
                .all(|type_reference| type_reference == first)
                .then_some(InferredType::TypeReference(first))
        }
        ExpressionNode::Member(member) => {
            let receiver = infer_expression_type(program, member.receiver, environment, visited)?;
            let symbol = resolve_member_symbol(program, receiver, member)?;
            crate::declaration_type_reference(program, symbol).map(InferredType::TypeReference)
        }
        ExpressionNode::Call(call) => {
            if let Some(operator) =
                typed_trees::operator::resolve_named_expression_call(program, call)
            {
                return operator
                    .return_type
                    .is_valid()
                    .then_some(InferredType::TypeReference(operator.return_type));
            }
            program
                .state_by_symbol(call.target_symbol)
                .and_then(|state| {
                    state
                        .return_type
                        .is_valid()
                        .then_some(InferredType::TypeReference(state.return_type))
                })
        }
        ExpressionNode::Indexed(indexed) => {
            let collection =
                infer_expression_type(program, indexed.collection, environment, visited)?;
            let element_type = collection_element_type(program, collection)?;
            Some(
                if matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    InferredType::CollectionView(element_type)
                } else {
                    InferredType::TypeReference(element_type)
                },
            )
        }
        ExpressionNode::StructLiteral(literal) => literal
            .type_symbol
            .is_valid()
            .then_some(InferredType::Nominal(literal.type_symbol)),
        ExpressionNode::String(_) => Some(InferredType::CompilerString),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => None,
    }
}

pub(super) fn resolve_member_symbol(
    program: &dyn crate::CheckingInput,
    receiver_type: InferredType,
    member: &TableMemberExpression,
) -> Option<SymbolHandle> {
    let nominal = inferred_nominal_symbol(program, receiver_type)?;
    let data = program.data_definition_by_symbol(nominal)?;

    let mut candidates = Vec::new();
    for data_member in program.data_members(data) {
        match data_member {
            typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == member.member.as_str() =>
            {
                if !candidates.contains(&field.symbol) {
                    candidates.push(field.symbol);
                }
            }
            typed_trees::data::DataMember::Variant(variant) => {
                if member
                    .case_variant
                    .as_ref()
                    .is_some_and(|case| case != &variant.name)
                {
                    continue;
                }
                for field in program
                    .data_payload_fields(variant)
                    .iter()
                    .filter(|field| field.name.as_str() == member.member.as_str())
                {
                    if !candidates.contains(&field.symbol) {
                        candidates.push(field.symbol);
                    }
                }
            }
            _ => {}
        }
    }
    let [selected] = candidates.as_slice() else {
        return None;
    };
    Some(*selected)
}

pub(super) fn inferred_nominal_symbol(
    program: &dyn crate::CheckingInput,
    inferred: InferredType,
) -> Option<SymbolHandle> {
    match inferred {
        InferredType::Nominal(symbol) => symbol.is_valid().then_some(symbol),
        InferredType::TypeReference(type_reference) => {
            let symbol = program.type_reference_table.type_symbol(type_reference);
            symbol.is_valid().then_some(symbol)
        }
        InferredType::CollectionView(_) => None,
        InferredType::CompilerString => None,
    }
}

pub(super) fn collection_element_type(
    program: &dyn crate::CheckingInput,
    inferred: InferredType,
) -> Option<TypeReferenceHandle> {
    let type_reference = match inferred {
        InferredType::TypeReference(type_reference) => type_reference,
        InferredType::CollectionView(element_type) => return Some(element_type),
        InferredType::Nominal(_) | InferredType::CompilerString => return None,
    };
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => collection_element_type(program, InferredType::TypeReference(*referee)),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}
