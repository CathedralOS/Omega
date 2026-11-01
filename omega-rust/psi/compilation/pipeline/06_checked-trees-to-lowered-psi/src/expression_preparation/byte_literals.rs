//! Constructor literals establish owned containers or image-backed descriptors,
//! not moves from existing places. Replay bytes against the declared carrier and every
//! selected byte predicate before emission attaches its qualification roster.

use checked_trees::types::{
    FixedArrayLength, PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use checked_trees::{
    CheckedTrees, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use language_semantics::SemanticDomainId;

use crate::lowering_error::{LoweringError, unsupported};

pub(crate) fn validate(
    checked: &CheckedTrees,
    expected: TypeReferenceHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<Vec<SemanticDomainId>, LoweringError> {
    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } = &argument.source
    else {
        return unsupported("record byte literal has no literal source");
    };
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
                    let definition = checked.domain_definition(domain.symbol).ok_or(
                        LoweringError::Unsupported("record byte literal domain is absent"),
                    )?;
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
            // This lane establishes a qualified exact fixed byte array;
            // checking proved that the literal has its declared width.
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(_),
            } if !domains.is_empty()
                && checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8) =>
            {
                domains.sort_by_key(|domain| domain.0);
                domains.dedup();
                return Ok(domains);
            }
            _ => {
                return unsupported(
                    "record byte literal carrier is not a domain-qualified byte array",
                );
            }
        }
    }
}
