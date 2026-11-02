//! Same-carrier qualification views preserve storage rather than reinterpret it.
//!
//! Geometry alone cannot justify a routed domain. This judgment permits only
//! retained declared atoms (or vacuous labels), while the occurrence checker
//! separately demands the operand's live provenance. Additional input meaning
//! is not a return promise: explicit erasure may forget it. Writable views owe
//! the reverse implication for actual lender declarations, not every input
//! precondition. The recast owner obtains those declarations from exact backing
//! origins before comparing physical carriers. No owner claim is consumed here.

use typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

pub(super) fn qualification_view_compatible(
    program: &dyn crate::CheckingInput,
    source: TypeReferenceHandle,
    target: TypeReferenceHandle,
) -> Option<bool> {
    let (source_carrier, source_domains) = declared_qualifications(program, source)?;
    let (target_carrier, target_domains) = declared_qualifications(program, target)?;
    if program.normalized_type_identity(source_carrier)
        != program.normalized_type_identity(target_carrier)
    {
        // Cross-carrier validity belongs to the representation judgment.
        // This relation only transports qualifications of the same carrier;
        // the occurrence checker separately refuses minted route evidence.
        return None;
    }
    Some(domains_imply(program, &source_domains, &target_domains))
}

/// A lender declaration may be an earlier presentation of the same bytes.
/// Geometry is checked separately; this query retains its semantic obligations.
pub(super) fn lender_qualifications_implied(
    program: &dyn crate::CheckingInput,
    view: TypeReferenceHandle,
    lender: TypeReferenceHandle,
) -> Option<bool> {
    let (_, view_domains) = declared_qualifications(program, view)?;
    let (_, lender_domains) = declared_qualifications(program, lender)?;
    Some(domains_imply(program, &view_domains, &lender_domains))
}

fn domains_imply(
    program: &dyn crate::CheckingInput,
    source_domains: &[&DomainConstraint],
    target_domains: &[&DomainConstraint],
) -> bool {
    target_domains.iter().all(|target| {
        crate::field_domains::domain_is_vacuous(program, target.symbol, &mut Vec::new())
            || source_domains.iter().any(|source| {
                (source.symbol == target.symbol
                    && source.semantic_id.is_valid()
                    && source.semantic_id == target.semantic_id)
                    || (source.arguments.is_empty()
                        && target.arguments.is_empty()
                        && crate::field_domains::domain_membership_implies(
                            program,
                            source.symbol,
                            target.symbol,
                        ))
            })
    })
}

/// Remove only the domain layers already discharged by the qualification
/// judgment. Ranges and other carrier constraints are not erasure candidates.
pub(super) fn qualification_carrier(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    declared_qualifications(program, reference)
        .map(|(carrier, _)| carrier)
        .unwrap_or(reference)
}

fn declared_qualifications(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Vec<&DomainConstraint>)> {
    let mut domains = Vec::new();
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Domain(domain) = constraint else {
                // Ranges, arithmetic policies and other constraint families
                // keep their existing representation judgment.
                return None;
            };
            if !domain.symbol.is_valid() || !domain.semantic_id.is_valid() {
                return None;
            }
            domains.push(domain);
        }
        reference = *base_type;
    }
    Some((reference, domains))
}
