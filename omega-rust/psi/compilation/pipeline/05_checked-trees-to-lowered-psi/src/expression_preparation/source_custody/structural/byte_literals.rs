//! Constructor literals establish owned containers or image-backed descriptors,
//! not moves from existing places. Replay bytes against the declared carrier and every
//! selected byte predicate before emission attaches its qualification roster.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::types::{
    FixedArrayLength, PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use checked_trees::{
    CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use language_semantics::{Multiplicity, SemanticDomainId};

use crate::lowering_error::{LoweringError, unsupported};

/// Catalog qualifications and report whether construction needs a literal view.
/// Record children carry their declaration's type; selection arms keep the same
/// destination. This registers theories, while emission and Terminal replay
/// independently establish the actual value's membership.
pub(crate) fn collect_operation_requirements(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
    domains: &mut Vec<SemanticDomainId>,
) -> Result<bool, LoweringError> {
    let checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value,
        ..
    } = operation
    else {
        return Ok(false);
    };
    let values = &checked.facts.values.structural_values;
    let mut roots = values.roots.iter().map(|(_, root)| root).filter(|root| {
        root.machine == machine
            && root.statement_ordinal == result.statement_index
            && root.root == *value
    });
    let reference = roots
        .next()
        .map(|root| root.type_reference)
        .unwrap_or_default();
    if roots.next().is_some() {
        return unsupported("literal domain catalog has ambiguous value roots");
    }
    collect_domains(checked, *value, reference, domains)
}

fn collect_domains(
    checked: &CheckedTrees,
    value: checked_trees::CheckedStructuralValueHandle,
    expected: TypeReferenceHandle,
    domains: &mut Vec<SemanticDomainId>,
) -> Result<bool, LoweringError> {
    use checked_trees::{CheckedStructuralRecordFieldValue, CheckedStructuralValueKind};
    let plans = &checked.facts.values.structural_values;
    let mut pending = vec![(value, expected)];
    let mut visited = Vec::new();
    let mut requires_literal_view = false;
    while let Some((value, reference)) = pending.pop() {
        if !plans.nodes.is_valid(value) {
            return unsupported("record byte literal catalog has stale construction");
        }
        if visited.contains(&(value, reference)) {
            continue;
        }
        visited.push((value, reference));
        let node = plans.nodes.get(value);
        match &node.kind {
            CheckedStructuralValueKind::ByteLiteralView { .. } => {
                requires_literal_view = true;
            }
            CheckedStructuralValueKind::Place(argument)
                if matches!(
                    argument.source,
                    CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
                ) =>
            {
                requires_literal_view = true;
                for domain in validate(checked, node.expression, reference, argument)? {
                    if !domains.contains(&domain) {
                        domains.push(domain);
                    }
                }
            }
            CheckedStructuralValueKind::Record { fields, .. }
            | CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans
                    .record_fields
                    .span(*fields)
                    .ok_or(LoweringError::Unsupported(
                        "record byte literal catalog has stale fields",
                    ))?
                {
                    if let CheckedStructuralRecordFieldValue::Structural(child) = field.value {
                        pending.push((child, field.type_reference));
                    }
                }
            }
            CheckedStructuralValueKind::Dispatch { arms, .. } => {
                for arm in plans
                    .dispatch_arms
                    .span(*arms)
                    .ok_or(LoweringError::Unsupported(
                        "record byte literal catalog has stale arms",
                    ))?
                {
                    pending.push((arm.value, reference));
                }
            }
            CheckedStructuralValueKind::FixedArray { elements } => {
                let element = if checked
                    .type_reference_table
                    .contains_type_reference(reference)
                    && let TypeReferenceNode::FixedArray { element_type, .. } =
                        checked.type_reference_table.type_reference(reference)
                {
                    *element_type
                } else {
                    TypeReferenceHandle::default()
                };
                pending.extend(
                    elements
                        .iter()
                        .map(|element_value| (*element_value, element)),
                );
            }
            _ => {}
        }
    }
    Ok(requires_literal_view)
}

pub(crate) fn validate(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<Vec<SemanticDomainId>, LoweringError> {
    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } = &argument.source
    else {
        return unsupported("record byte literal has no literal source");
    };
    if !checked
        .type_reference_table
        .contains_type_reference(expected)
        || !checked.expression_table.expression_is_valid(expression)
        || !matches!(checked.expression_table.expression(expression), ExpressionNode::String(authored) if authored.as_ref() == bytes.as_slice())
        || argument.access != CheckedStructuralAccess::Owned
        || !argument.path.is_empty()
        || checked.type_multiplicity(expected) != Multiplicity::Unrestricted
    {
        return unsupported("record byte literal changed its authored bytes or owned access");
    }
    let mut reference = expected;
    let mut visited = Vec::new();
    let mut domains = Vec::new();
    loop {
        if !checked
            .type_reference_table
            .contains_type_reference(reference)
            || visited.contains(&reference)
        {
            return unsupported("record byte literal has an invalid carrier chain");
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let constraints = checked
                    .type_reference_table
                    .constraint_span(*constraints)
                    .ok_or(LoweringError::Unsupported(
                        "record byte literal has stale constraints",
                    ))?;
                for constraint in constraints {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        return unsupported("record byte literal has an unsupported qualification");
                    };
                    let definition = checked
                        .domain_definitions()
                        .iter()
                        .find(|definition| definition.symbol == domain.symbol)
                        .ok_or(LoweringError::Unsupported(
                            "record byte literal domain is absent",
                        ))?;
                    if !domain.semantic_id.is_valid()
                        || definition.semantic_id != domain.semantic_id
                        || !domain.arguments.is_empty()
                        || !domain.establishment_routes.is_empty()
                        || !definition.establishment_routes.is_empty()
                        || definition.semantic_roles != Default::default()
                        || !checked_trees::byte_predicates::domain_byte_predicate(
                            &checked.typed,
                            domain.symbol,
                        )
                        .is_some_and(|predicate| predicate.holds_for(bytes))
                    {
                        return unsupported(
                            "record byte literal does not establish its declared domain",
                        );
                    }
                    domains.push(domain.semantic_id);
                }
                reference = *base_type;
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(capacity),
            } if !domains.is_empty()
                && checked
                    .type_reference_table
                    .contains_type_reference(*element_type)
                && checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
                && bytes.len() <= *capacity
                && checked.normalized_type_identity(reference).as_str()
                    == argument.type_identity =>
            {
                domains.sort_by_key(|domain| domain.0);
                domains.dedup();
                return Ok(domains);
            }
            _ => return unsupported("record byte literal changed its bounded owned carrier"),
        }
    }
}
