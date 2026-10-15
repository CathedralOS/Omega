//! Qualification preservation for a primitive write into exact fixed storage.
//! Sibling memberships are unaffected; an overlapping membership must classify
//! the exact byte array containing the selected element. Representation and
//! access are independent of this receiving restriction.
use semantic_vocabulary::{
    BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, StructuralDomainId,
    StructuralTypeId,
};
use terminal_psi::{
    ByteSequencePredicate, ByteSequencePredicateSet, StructuralDomainDeclaration,
    StructuralFieldType, StructuralPathQualification, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

/// Reconstruct the complete receiving restriction, refusing unsupported
/// overlapping qualifications instead of silently discarding their authority.
/// The outer Option is admission; the inner Option is the optional scalar goal.
pub fn primitive_store_restriction<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
) -> Option<Option<StructuralFieldType>> {
    primitive_store_restriction_with_predicates(
        types,
        root,
        whole,
        projected,
        written,
        |carrier, domain| {
            crate::byte_qualification_predicates_in(domains.clone(), carrier, &[domain])
        },
    )
}

pub fn primitive_store_restriction_with_predicates<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
    domain_predicates: impl FnMut(
        StructuralTypeId,
        StructuralDomainId,
    ) -> Option<ByteSequencePredicateSet>,
) -> Option<Option<StructuralFieldType>> {
    let predicates = enclosing_byte_predicates(
        types.clone(),
        root,
        whole,
        projected,
        written,
        domain_predicates,
    )?;
    let scalar = crate::primitive_projection_restriction(types, root, written).cloned();
    let mut ascii = ByteSequencePredicateSet::EMPTY;
    ascii.insert(ByteSequencePredicate::AsciiOnly);
    // UTF-8 stores additionally require current ASCII backing. Source checking,
    // current-graph replay and execution retain that independent obligation.
    let utf8 = predicates
        .predicates()
        .any(|p| p == ByteSequencePredicate::ValidUtf8);
    let nonzero = predicates
        .predicates()
        .any(|p| p == ByteSequencePredicate::NoNul);
    let ascii = predicates.implies(ascii) || utf8;
    if !ascii && !nonzero {
        return Some(scalar);
    }
    if scalar.is_some() {
        return None;
    }
    Some(Some(StructuralFieldType::BoundedInteger(
        BoundedIntegerType::new(
            IntegerType::new(IntegerSign::Unsigned, 8).ok()?,
            IntegerValue::Unsigned(if nonzero { 1 } else { 0 }),
            IntegerValue::Unsigned(if ascii { 127 } else { 255 }),
        )
        .ok()?,
    )))
}

fn enclosing_byte_predicates<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
    mut domain_predicates: impl FnMut(
        StructuralTypeId,
        StructuralDomainId,
    ) -> Option<ByteSequencePredicateSet>,
) -> Option<ByteSequencePredicateSet> {
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    let memberships = whole.iter().map(|domain| (&[][..], *domain)).chain(
        projected
            .iter()
            .map(|row| (row.path.as_slice(), row.domain)),
    );
    for (path, domain) in memberships {
        if !crate::structural_paths_may_overlap(path, written) {
            continue;
        }
        let prefix = written.get(..path.len())?;
        StructuralPathQualification::selected_suffix(path, prefix)?;
        let suffix = &written[path.len()..];
        if !matches!(
            suffix,
            [StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. }]
        ) {
            return None;
        }
        let carrier_id = crate::qualification_path_carrier(types.clone(), root, path)?;
        let carrier = types.clone().find(|row| row.id == carrier_id)?;
        let StructuralTypeShape::FixedArray { element, .. } = carrier.shape else {
            return None;
        };
        let byte = IntegerType::new(IntegerSign::Unsigned, 8).ok()?;
        if types.clone().find(|row| row.id == element)?.shape
            != StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(byte))
        {
            return None;
        }
        let declared = domain_predicates(carrier.id, domain)?;
        for predicate in declared.predicates() {
            predicates.insert(predicate);
        }
    }
    Some(predicates)
}

/// The exact byte predicates whose backing a primitive replacement must keep.
pub fn primitive_store_byte_predicates<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
) -> Option<ByteSequencePredicateSet> {
    enclosing_byte_predicates(types, root, whole, projected, written, |carrier, domain| {
        crate::byte_qualification_predicates_in(domains.clone(), carrier, &[domain])
    })
}
