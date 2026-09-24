//! Whether an authored two-guard tail needs no fallback because its second
//! guard holds exactly where the first fails. Unit graph edges, scalar graph
//! guards, ordered scalar exits, and ordered scalar completions all re-derive
//! this here. Each guard row is first rejoined to its authored guard, then the
//! pair is judged by the predicate the producers use, with the subject's case
//! roster reconstructed from the authored parameter type.

use super::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, ScalarType, authored_state,
    unsupported, validate_pure,
};
use checked_trees::data::{DataDefinition, DataMember};
use checked_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    CheckedScalarExpression, CheckedStructuralParameterField, CheckedStructuralPredicatePathSegment,
};

#[cfg(test)]
mod tests;

/// The Guard row at `ordinal + 1` is the exact complement of the row at
/// `ordinal`, both in `state`. A missing or unrejoined row is an error.
pub(crate) fn complementary(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    ordinal: u32,
) -> Result<bool, LoweringError> {
    let guard = |statement| {
        let (source, expression) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state, statement, CheckedScalarExpressionRole::Guard)
            .ok_or(LoweringError::Unsupported(
                "complementary fallback lost its pure source guard",
            ))?;
        validate_pure(checked, source, ScalarType::Boolean)?;
        let CheckedScalarExpression::Boolean(expression) = expression else {
            return unsupported("complementary fallback guard is not Boolean");
        };
        Ok(expression.as_ref())
    };
    let next = ordinal
        .checked_add(1)
        .ok_or(LoweringError::Unsupported("fallback ordinal overflow"))?;
    let (first, second) = (guard(ordinal)?, guard(next)?);
    let (_, source) = authored_state(checked, state)?;
    Ok(checked_trees::values::guard_complement::exact_complement(
        first,
        second,
        |subject| declared_cases(checked, source, subject),
    ))
}

/// The declared case keys of the sum the subject path reaches from its
/// authored state parameter, or `None` unless every member of that sum is a
/// case.
fn declared_cases(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    subject: &CheckedStructuralParameterField,
) -> Option<Vec<String>> {
    let parameter = checked
        .state_parameters(state)
        .get(usize::try_from(subject.parameter_position).ok()?)?;
    let mut reference = parameter.type_reference;
    let mut selected_case = None;
    for segment in &subject.path {
        match segment {
            CheckedStructuralPredicatePathSegment::Field(identity) => {
                let field = match selected_case.take() {
                    Some(case) => checked
                        .data_payload_fields(case)
                        .iter()
                        .find(|field| key(field.identity, field.name.as_str()) == *identity)?,
                    None => checked
                        .data_members(nominal(checked, reference)?)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Field(field)
                                if key(field.identity, field.name.as_str()) == *identity =>
                            {
                                Some(field)
                            }
                            _ => None,
                        })?,
                };
                if field.relevance.is_erased() {
                    return None;
                }
                reference = field.type_reference;
            }
            CheckedStructuralPredicatePathSegment::Case(identity) => {
                if selected_case.is_some() {
                    return None;
                }
                selected_case = Some(
                    checked
                        .data_members(nominal(checked, reference)?)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(case)
                                if key(case.identity, case.name.as_str()) == *identity =>
                            {
                                Some(case)
                            }
                            _ => None,
                        })?,
                );
            }
            CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                if selected_case.is_some() {
                    return None;
                }
                reference = fixed_element(checked, reference, *index)?;
            }
        }
    }
    if selected_case.is_some() {
        return None;
    }
    checked
        .data_members(nominal(checked, reference)?)
        .iter()
        .map(|member| match member {
            DataMember::Variant(case) => Some(key(case.identity, case.name.as_str())),
            DataMember::Field(_) => None,
        })
        .collect()
}

/// The data declaration a structural type names through references and
/// constraints. A machine's `Self` names its attached data.
fn nominal(checked: &CheckedTrees, mut reference: TypeReferenceHandle) -> Option<&DataDefinition> {
    let (symbol, name) = loop {
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            TypeReferenceNode::Named { symbol, name }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                base_name: name,
                ..
            } => break (*symbol, name),
            _ => return None,
        }
    };
    let symbol = checked
        .machines()
        .iter()
        .find(|machine| {
            symbol.is_valid() && machine.symbol == symbol && machine.attached_data_symbol.is_valid()
        })
        .map_or(symbol, |machine| machine.attached_data_symbol);
    checked.data_definitions().iter().find(|data| {
        if symbol.is_valid() {
            data.symbol == symbol
        } else {
            data.name == *name
        }
    })
}

fn fixed_element(
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
    index: u64,
) -> Option<TypeReferenceHandle> {
    loop {
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => return (index < u64::try_from(*length).ok()?).then_some(*element_type),
            _ => return None,
        }
    }
}

fn key(identity: Option<u64>, name: &str) -> String {
    identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| name.to_owned())
}
