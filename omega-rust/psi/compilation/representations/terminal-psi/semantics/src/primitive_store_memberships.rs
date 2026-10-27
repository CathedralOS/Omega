//! Qualification preservation for a primitive write into exact fixed storage.
//! Sibling memberships are unaffected; an overlapping membership must classify
//! the exact replaced scalar or byte array containing the selected element. Representation and
//! access are independent of this receiving restriction.
mod scalar;

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
    let mut scalar = crate::primitive_projection_restriction(types.clone(), root, written).cloned();
    for domain in whole_scalar_domains(
        types.clone(),
        domains.clone(),
        root,
        whole,
        projected,
        written,
    )? {
        scalar = scalar::receive(scalar, scalar::refinement(types.clone(), domain)?)?;
    }
    let predicates =
        enclosing_byte_predicates(types.clone(), domains, root, whole, projected, written)?;
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

fn enclosing_byte_predicates<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
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
        let carrier_id = crate::qualification_path_carrier(types.clone(), root, path)?;
        if suffix.is_empty() && scalar::primitive_carrier(types.clone(), carrier_id).is_some() {
            let domain = exact_domain(domains.clone(), carrier_id, domain)?;
            scalar::refinement(types.clone(), domain)?;
            continue;
        }
        if !matches!(
            suffix,
            [StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. }]
        ) {
            return None;
        }
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
        let declared =
            crate::byte_qualification_predicates_in(domains.clone(), carrier.id, &[domain])?;
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
    enclosing_byte_predicates(types, domains, root, whole, projected, written)
}

fn exact_domain<'domain>(
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration>,
    carrier: StructuralTypeId,
    identity: StructuralDomainId,
) -> Option<&'domain StructuralDomainDeclaration> {
    let mut matching = domains.filter(|domain| domain.id == identity);
    let domain = matching.next()?;
    (matching.next().is_none() && domain.carrier == carrier).then_some(domain)
}

/// Exact replacements retain their own scalar-domain obligations. Ancestor
/// and partial projections remain with the byte/content preservation owners.
fn whole_scalar_domains<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
) -> Option<Vec<&'domain StructuralDomainDeclaration>> {
    let mut result = Vec::new();
    for (path, identity) in whole.iter().map(|identity| (&[][..], *identity)).chain(
        projected
            .iter()
            .map(|row| (row.path.as_slice(), row.domain)),
    ) {
        if path.len() != written.len() || !crate::structural_paths_may_overlap(path, written) {
            continue;
        }
        StructuralPathQualification::selected_suffix(path, written)?;
        let carrier = crate::qualification_path_carrier(types.clone(), root, path)?;
        if scalar::primitive_carrier(types.clone(), carrier).is_some() {
            let domain = exact_domain(domains.clone(), carrier, identity)?;
            result.extend(scalar::required_domains(domains.clone(), domain)?);
        }
    }
    Some(result)
}

/// Nominal preservation is separate from the numeric receiving predicate.
/// External admission matches each routed destination domain to the actual
/// SSA value's established catalog membership. A true predicate is insufficient.
#[allow(clippy::too_many_arguments)]
pub fn primitive_store_preserves_scalar_memberships<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    catalog: &terminal_psi::ScalarQualificationCatalog,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
    value: &terminal_psi::ValueDeclaration,
) -> bool {
    let Some(required) = whole_scalar_domains(
        types.clone(),
        domains.clone(),
        root,
        whole,
        projected,
        written,
    ) else {
        return false;
    };
    let actual = if value.qualifications.is_empty() {
        &[][..]
    } else {
        let mut matching = catalog
            .sets
            .iter()
            .filter(|set| set.id == value.qualifications);
        let Some(set) = matching.next() else {
            return false;
        };
        if matching.next().is_some() {
            return false;
        }
        set.domains.as_slice()
    };
    required.into_iter().all(|required| {
        required.establishment_routes.is_empty()
            || scalar_domain_matches(
                types.clone(),
                domains.clone(),
                required,
                catalog,
                actual,
                value.scalar_type,
            )
    })
}

fn scalar_domain_matches<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    required: &'domain StructuralDomainDeclaration,
    catalog: &terminal_psi::ScalarQualificationCatalog,
    actual: &[semantic_vocabulary::ScalarDomainId],
    carrier: semantic_vocabulary::ScalarType,
) -> bool {
    let mut matching = catalog.domains.iter().filter(|domain| {
        domain.semantic_domain == required.semantic_domain && domain.carrier == carrier
    });
    let Some(domain) = matching.next() else {
        return false;
    };
    matching.next().is_none()
        && domain
            .missing_membership_requirements(&catalog.domains, actual)
            .is_some_and(|missing| missing.is_empty())
        && scalar::same_theory(types, domains, required, domain, catalog)
}

/// Store an already established scalar in addressable primitive storage without
/// changing its theory or pretending that the home issued it. The returned
/// structural memberships come solely from this exact SSA qualification set.
pub fn primitive_scalar_memberships<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    catalog: &terminal_psi::ScalarQualificationCatalog,
    destination: StructuralTypeId,
    value: &terminal_psi::ValueDeclaration,
) -> Option<Vec<StructuralDomainId>> {
    if scalar::primitive_carrier(types.clone(), destination)? != value.scalar_type {
        return None;
    }
    if value.qualifications.is_empty() {
        return Some(Vec::new());
    }
    let mut sets = catalog
        .sets
        .iter()
        .filter(|set| set.id == value.qualifications);
    let actual = sets.next()?.domains.as_slice();
    if sets.next().is_some() {
        return None;
    }
    let mut memberships = Vec::with_capacity(actual.len());
    for identity in actual {
        let mut declarations = catalog
            .domains
            .iter()
            .filter(|domain| domain.id == *identity);
        let declaration = declarations.next()?;
        if declarations.next().is_some() || declaration.carrier != value.scalar_type {
            return None;
        }
        let mut matching = domains.clone().filter(|domain| {
            domain.carrier == destination && domain.semantic_domain == declaration.semantic_domain
        });
        let domain = matching.next()?;
        if matching.next().is_some()
            || !scalar_domain_matches(
                types.clone(),
                domains.clone(),
                domain,
                catalog,
                actual,
                value.scalar_type,
            )
        {
            return None;
        }
        memberships.push(domain.id);
    }
    memberships.sort();
    memberships.dedup();
    Some(memberships)
}
