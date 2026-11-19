pub(crate) mod scalar;

use crate::exactly_one::exactly_one;
use semantic_vocabulary::{
    BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, StructuralDomainId,
    StructuralTypeId,
};
use terminal_psi::{
    ByteSequencePredicate, ByteSequencePredicateSet, StructuralDomainDeclaration,
    StructuralFieldType, StructuralPathQualification, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

pub fn scalar_projection_entails_membership<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    membership: StructuralDomainId,
) -> bool {
    let Some(carrier) = crate::qualification_path_carrier(types.clone(), root, path) else {
        return false;
    };
    let Some(domain) = exact_domain(domains, carrier, membership) else {
        return false;
    };
    if !domain.complete_predicate_theory
        || !domain.has_no_required_memberships()
        || domain.establishment_routes.is_some()
        || domain.content_projection.is_some()
        || domain.byte_predicate.is_some()
    {
        return false;
    }
    let Some(restriction) = crate::primitive_projection_restriction(types.clone(), root, path)
        .or_else(|| {
            crate::primitive_read_field(types.clone(), root, path)
                .map(|field| &field.field_type)
        })
        .filter(|field| crate::scalar_field_requires_evidence(field))
    else {
        return false;
    };
    let Some(required) = scalar::refinement(types, domain) else {
        return false;
    };
    crate::scalar_field_entails(restriction, &StructuralFieldType::RefinedScalar(required))
}

pub fn primitive_store_restriction<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    additional_input: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
) -> Option<Option<StructuralFieldType>> {
    let mut scalar =
        crate::primitive_projection_restriction(types.clone(), root, written).cloned();
    for domain in whole_scalar_domains(
        types.clone(),
        domains.clone(),
        root,
        whole,
        projected,
        additional_input,
        written,
    )? {
        scalar = scalar::receive(scalar, scalar::refinement(types.clone(), domain)?)?;
    }
    let predicates = enclosing_byte_predicates(
        types.clone(),
        domains,
        root,
        whole,
        projected,
        additional_input,
        written,
    )?;
    let mut ascii = ByteSequencePredicateSet::EMPTY;
    ascii.insert(ByteSequencePredicate::AsciiOnly);
    let nonzero = predicates
        .predicates()
        .any(|p| p == ByteSequencePredicate::NoNul);
    // UTF-8 validity does not bound each octet to ASCII: a valid scalar
    // can use several bytes whose values exceed 127.
    let ascii = predicates.implies(ascii);
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
    additional_input: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
) -> Option<ByteSequencePredicateSet> {
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    let memberships = whole.iter().map(|domain| (&[][..], *domain)).chain(
        projected
            .iter()
            .map(|row| (row.path.as_slice(), row.domain)),
    );
    for (path, domain) in memberships {
        if expiring_input_predicate(
            types.clone(),
            domains.clone(),
            root,
            path,
            domain,
            additional_input,
        ) {
            continue;
        }
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
        // A domain stated only by clauses, `[u8; 3]::Low requires
        // self[0] < 128`, governs no single write: a write inside it opens
        // the owning data type's invariant window, and the checker proves
        // the clauses again before the value is used (dependent_values.md,
        // "Invariant windows").
        let declared = exact_domain(domains.clone(), carrier_id, domain)?;
        if declared.byte_predicate.is_none()
            && declared.content_projection.is_none()
            && declared.establishment_routes.is_none()
            && declared.has_no_required_memberships()
        {
            continue;
        }
        if !matches!(
            suffix,
            [
                StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. }
            ]
        ) {
            return None;
        }
        let carrier = types.clone().find(|row| row.id == carrier_id)?;
        let StructuralTypeShape::FixedArray { element, .. } = carrier.shape else {
            return None;
        };
        let byte = IntegerType::new(IntegerSign::Unsigned, 8).ok()?;
        if types.clone().find(|row| row.id == element)?.shape
            != StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(
                byte,
            ))
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

pub fn primitive_store_byte_predicates<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    additional_input: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
) -> Option<ByteSequencePredicateSet> {
    enclosing_byte_predicates(
        types,
        domains,
        root,
        whole,
        projected,
        additional_input,
        written,
    )
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

fn whole_scalar_domains<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    additional_input: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
) -> Option<Vec<&'domain StructuralDomainDeclaration>> {
    let mut result = Vec::new();
    for (path, identity) in whole.iter().map(|identity| (&[][..], *identity)).chain(
        projected
            .iter()
            .map(|row| (row.path.as_slice(), row.domain)),
    ) {
        if expiring_input_predicate(
            types.clone(),
            domains.clone(),
            root,
            path,
            identity,
            additional_input,
        ) {
            continue;
        }
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

#[allow(clippy::too_many_arguments)]
pub fn primitive_store_preserves_scalar_memberships<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    catalog: &terminal_psi::ScalarQualificationCatalog,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    additional_input: &[StructuralPathQualification],
    written: &[StructuralPathSegment],
    value: &terminal_psi::ValueDeclaration,
) -> bool {
    let Some(required) = whole_scalar_domains(
        types.clone(),
        domains.clone(),
        root,
        whole,
        projected,
        additional_input,
        written,
    ) else {
        return false;
    };
    let actual = if value.qualifications.is_empty() {
        &[][..]
    } else {
        let Some(set) = exactly_one(catalog.sets.iter(), |set| set.id == value.qualifications)
        else {
            return false;
        };
        set.domains.as_slice()
    };
    required.into_iter().all(|required| {
        required.establishment_routes.is_none()
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
        let declaration = exactly_one(catalog.domains.iter(), |domain| domain.id == *identity)?;
        if declaration.carrier != value.scalar_type {
            return None;
        }
        let mut matching = domains.clone().filter(|domain| {
            domain.carrier == destination
                && domain.semantic_domain == declaration.semantic_domain
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

fn expiring_input_predicate<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    membership: StructuralDomainId,
    additional: &[StructuralPathQualification],
) -> bool {
    if !additional
        .iter()
        .any(|row| row.domain == membership && row.path == path)
    {
        return false;
    }
    crate::qualification_path_carrier(types, root, path)
        .and_then(|carrier| exact_domain(domains, carrier, membership))
        .is_some_and(StructuralDomainDeclaration::is_pure_predicate)
}
#[cfg(test)]
mod tests;
