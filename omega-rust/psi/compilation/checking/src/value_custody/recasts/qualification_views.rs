//! Same-carrier qualification views preserve storage rather than reinterpret it.
//!
//! Geometry alone cannot justify a routed domain. This judgment permits only
//! retained declared atoms (or vacuous labels), while the occurrence checker
//! separately demands the operand's live provenance. Mutable views need the
//! reverse implication too: replacement through the view must preserve every
//! invariant of the lender. No owner claim is consumed by a reference view.

use typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

pub(super) fn qualification_view_compatible(
    program: &dyn crate::CheckingInput,
    source: TypeReferenceHandle,
    target: TypeReferenceHandle,
    mutable: bool,
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
    let implies = |source: &[&DomainConstraint], target: &[&DomainConstraint]| {
        target.iter().all(|target| {
            crate::field_domains::domain_is_vacuous(program, target.symbol, &mut Vec::new())
                || source.iter().any(|source| {
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
    };
    Some(
        implies(&source_domains, &target_domains)
            && (!mutable || implies(&target_domains, &source_domains)),
    )
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
