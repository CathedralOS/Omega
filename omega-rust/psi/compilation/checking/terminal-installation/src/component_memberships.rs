//! Artifact-local membership coordinates joined by their complete retained theory.
//!
//! A callable contract names domains but does not embed their definitions.
//! Installation compares those definitions before runtime may rebind existing
//! evidence. Neither a coincident dense ID nor a declaration name is evidence.

use std::collections::BTreeMap;

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerMathTerm, PlaceId, Proposition, ScalarReadProjection,
    ScalarTerm, StructuralCaseId, StructuralDomainId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    StructuralCaseDeclaration, StructuralDomainDeclaration, StructuralEstablishmentRoute,
    StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape, TerminalIdentityIndex,
    TerminalModule,
};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ComponentMemberships(Vec<(StructuralDomainId, StructuralDomainId)>);

impl ComponentMemberships {
    pub(super) fn join(
        receiver: &TerminalModule,
        provider: &TerminalModule,
    ) -> Result<Self, String> {
        let mut rows = Vec::new();
        for receiving in &receiver.structural_domains {
            let mut matching = provider.structural_domains.iter().filter(|supplying| {
                identity(receiver, receiving.identity, provider, supplying.identity)
            });
            let Some(supplying) = matching.next() else {
                continue;
            };
            if matching.next().is_some() {
                return Err("component membership has ambiguous destination theories".into());
            }
            let mut theory = TheoryJoin::new(receiver, provider);
            if !theory.domain(receiving, supplying) {
                return Err("component membership declarations retain different theories".into());
            }
            rows.push((receiving.id, supplying.id));
        }
        Ok(Self(rows))
    }

    pub(super) fn present(
        &self,
        memberships: &mut [StructuralDomainId],
        to_provider: bool,
    ) -> Result<(), &'static str> {
        for membership in memberships {
            let mut matches = self.0.iter().filter_map(|&(receiver, provider)| {
                let (source, destination) = if to_provider {
                    (receiver, provider)
                } else {
                    (provider, receiver)
                };
                (source == *membership).then_some(destination)
            });
            let destination = matches
                .next()
                .ok_or("component membership has no identical receiving theory")?;
            if matches.next().is_some() {
                return Err("component membership has ambiguous receiving theories");
            }
            *membership = destination;
        }
        Ok(())
    }
}

fn identity(
    left: &TerminalModule,
    a: TerminalIdentityIndex,
    right: &TerminalModule,
    b: TerminalIdentityIndex,
) -> bool {
    left.identities
        .text(a)
        .zip(right.identities.text(b))
        .is_some_and(|(a, b)| !a.is_empty() && a == b)
}

struct TheoryJoin<'module> {
    left: &'module TerminalModule,
    right: &'module TerminalModule,
    types: BTreeMap<StructuralTypeId, StructuralTypeId>,
    fields: BTreeMap<StructuralFieldId, StructuralFieldId>,
    field_names: BTreeMap<TerminalIdentityIndex, TerminalIdentityIndex>,
    cases: BTreeMap<StructuralCaseId, StructuralCaseId>,
}

impl<'module> TheoryJoin<'module> {
    fn new(left: &'module TerminalModule, right: &'module TerminalModule) -> Self {
        Self {
            left,
            right,
            types: BTreeMap::new(),
            fields: BTreeMap::new(),
            field_names: BTreeMap::new(),
            cases: BTreeMap::new(),
        }
    }

    fn domain(
        &mut self,
        left: &StructuralDomainDeclaration,
        right: &StructuralDomainDeclaration,
    ) -> bool {
        if !identity(self.left, left.identity, self.right, right.identity)
            || !self.carrier(left.carrier, right.carrier)
            || left.byte_predicate != right.byte_predicate
            || Self::routes(self.left, left) != Self::routes(self.right, right)
            || Self::routes(self.left, left).is_none()
        {
            return false;
        }
        match (&left.content_projection, &right.content_projection) {
            (None, None) => {}
            (Some(a), Some(b)) if a.algebra == b.algebra && a.expression == b.expression => {
                // The report fingerprint is deliberately non-authoritative.
                // Resolve each owner-local coordinate in its own catalog.
                let owners =
                    |module: &TerminalModule, domain: &StructuralDomainDeclaration, owner| {
                        let mut rows = module.structural_domains.iter().filter(|row| {
                            row.semantic_domain.get() == owner && row.carrier == domain.carrier
                        });
                        let row = rows.next()?;
                        rows.next()
                            .is_none()
                            .then(|| module.identities.text(row.identity))
                            .flatten()
                            .map(str::to_owned)
                    };
                if owners(self.left, left, a.identity.domain.get())
                    .zip(owners(self.right, right, b.identity.domain.get()))
                    .is_none_or(|(a, b)| a.is_empty() || a != b)
                {
                    return false;
                }
            }
            _ => return false,
        }
        match (&left.predicate, &right.predicate) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                if !terminal_semantics::domain_predicate_is_closed(
                    self.left.structural_types.iter(),
                    left.carrier,
                    a,
                ) || !terminal_semantics::domain_predicate_is_closed(
                    self.right.structural_types.iter(),
                    right.carrier,
                    b,
                ) {
                    return false;
                }
                let mut proposition = a.proposition.clone();
                self.proposition(&mut proposition, a.subject, b.subject)
                    && proposition.same_commutative_predicate(&b.proposition)
            }
            _ => false,
        }
    }

    fn routes(
        module: &TerminalModule,
        domain: &StructuralDomainDeclaration,
    ) -> Option<Vec<(u8, String)>> {
        let mut routes = domain
            .establishment_routes
            .iter()
            .map(|route| {
                let (kind, index) = match *route {
                    StructuralEstablishmentRoute::Requirement { requirement } => (0, requirement),
                    StructuralEstablishmentRoute::ExactMachine { machine } => (1, machine),
                    StructuralEstablishmentRoute::BoundaryRequirement { requirement } => {
                        (2, requirement)
                    }
                };
                let text = module.identities.text(index)?;
                (!text.is_empty()).then(|| (kind, text.to_owned()))
            })
            .collect::<Option<Vec<_>>>()?;
        routes.sort();
        Some(routes)
    }

    fn carrier(&mut self, left: StructuralTypeId, right: StructuralTypeId) -> bool {
        if let Some(previous) = self.types.get(&left) {
            return *previous == right;
        }
        let (Some(a), Some(b)) = (
            self.left.structural_type(left),
            self.right.structural_type(right),
        ) else {
            return false;
        };
        if !identity(self.left, a.identity, self.right, b.identity)
            || a.owns_linear_claim != b.owns_linear_claim
            || a.copyable != b.copyable
            || a.no_code_affine_cleanup != b.no_code_affine_cleanup
            || a.descriptor_cleanup != b.descriptor_cleanup
            || a.scalar_restriction != b.scalar_restriction
        {
            return false;
        }
        self.types.insert(left, right);
        let quotient = match (a.quotient_carrier, b.quotient_carrier) {
            (None, None) => true,
            (Some(a), Some(b)) => self.carrier(a, b),
            _ => false,
        };
        quotient
            && match (&a.shape, &b.shape) {
                (
                    StructuralTypeShape::Reference {
                        referent: a,
                        access: x,
                    },
                    StructuralTypeShape::Reference {
                        referent: b,
                        access: y,
                    },
                ) => {
                    let referent = |module: &TerminalModule, id| {
                        module
                            .structural_type(id)
                            .map(|row| row.identity)
                            .or_else(|| {
                                module
                                    .structural_referents
                                    .iter()
                                    .find(|row| row.id == id)
                                    .map(|row| row.identity)
                            })
                    };
                    x == y
                        && referent(self.left, *a)
                            .zip(referent(self.right, *b))
                            .is_some_and(|(a, b)| identity(self.left, a, self.right, b))
                }
                (
                    StructuralTypeShape::ElementView {
                        element: a,
                        access: x,
                    },
                    StructuralTypeShape::ElementView {
                        element: b,
                        access: y,
                    },
                ) => x == y && self.carrier(*a, *b),
                (
                    StructuralTypeShape::FixedArray {
                        element: a,
                        length: x,
                    },
                    StructuralTypeShape::FixedArray {
                        element: b,
                        length: y,
                    },
                ) => x == y && self.carrier(*a, *b),
                (
                    StructuralTypeShape::Record {
                        fields: a,
                        storage: x,
                    },
                    StructuralTypeShape::Record {
                        fields: b,
                        storage: y,
                    },
                ) => x == y && self.fields(a, b),
                (StructuralTypeShape::Sum { cases: a }, StructuralTypeShape::Sum { cases: b }) => {
                    self.cases(a, b)
                }
                (
                    StructuralTypeShape::Mixed {
                        fields: a,
                        cases: x,
                    },
                    StructuralTypeShape::Mixed {
                        fields: b,
                        cases: y,
                    },
                ) => self.fields(a, b) && self.cases(x, y),
                (a, b) => a == b,
            }
    }

    fn fields(
        &mut self,
        left: &[StructuralFieldDeclaration],
        right: &[StructuralFieldDeclaration],
    ) -> bool {
        left.len() == right.len()
            && left.iter().zip(right).all(|(a, b)| {
                if !identity(self.left, a.identity, self.right, b.identity)
                    || a.relevance != b.relevance
                    || a.storage != b.storage
                {
                    return false;
                }
                if self
                    .fields
                    .insert(a.id, b.id)
                    .is_some_and(|previous| previous != b.id)
                {
                    return false;
                }
                if self
                    .field_names
                    .insert(a.identity, b.identity)
                    .is_some_and(|previous| previous != b.identity)
                {
                    return false;
                }
                match (&a.field_type, &b.field_type) {
                    (StructuralFieldType::Structural(a), StructuralFieldType::Structural(b)) => {
                        self.carrier(*a, *b)
                    }
                    (
                        StructuralFieldType::Erased { type_identity: a },
                        StructuralFieldType::Erased { type_identity: b },
                    ) => identity(self.left, *a, self.right, *b),
                    (a, b) => a == b,
                }
            })
    }

    fn cases(
        &mut self,
        left: &[StructuralCaseDeclaration],
        right: &[StructuralCaseDeclaration],
    ) -> bool {
        left.len() == right.len()
            && left.iter().zip(right).all(|(a, b)| {
                identity(self.left, a.identity, self.right, b.identity)
                    && !self
                        .cases
                        .insert(a.id, b.id)
                        .is_some_and(|previous| previous != b.id)
                    && self.fields(&a.fields, &b.fields)
            })
    }

    fn path(&self, path: &mut [CanonicalStructuralPathSegment]) -> Option<()> {
        for step in path {
            match step {
                CanonicalStructuralPathSegment::Field(field) => *field = *self.fields.get(field)?,
                CanonicalStructuralPathSegment::Case(case) => *case = *self.cases.get(case)?,
                CanonicalStructuralPathSegment::FixedIndex(_) => {}
            }
        }
        Some(())
    }

    fn scalar(&self, term: &mut ScalarTerm, source: PlaceId, destination: PlaceId) -> bool {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. }
            | ScalarTerm::IeeeFloatField { root, path, .. }
            | ScalarTerm::ByteSequencePredicate { root, path, .. } => {
                if *root != source || self.path(path).is_none() {
                    return false;
                }
                *root = destination;
            }
            ScalarTerm::StructuralRead { root, path, .. } => {
                if *root != source {
                    return false;
                }
                *root = destination;
                for step in path {
                    let ScalarReadProjection::Static(step) = step else {
                        return false;
                    };
                    if self.path(std::slice::from_mut(step)).is_none() {
                        return false;
                    }
                }
            }
            _ => {}
        }
        term.operands_mut()
            .into_iter()
            .all(|operand| self.scalar(operand, source, destination))
    }

    fn integer(&self, term: &mut IntegerMathTerm, source: PlaceId, destination: PlaceId) -> bool {
        if let IntegerMathTerm::Embed(scalar) = term {
            return self.scalar(scalar, source, destination);
        }
        term.operands_mut()
            .into_iter()
            .all(|operand| self.integer(operand, source, destination))
    }

    fn proposition(
        &self,
        proposition: &mut Proposition,
        source: PlaceId,
        destination: PlaceId,
    ) -> bool {
        match proposition {
            Proposition::Truth | Proposition::Falsehood => true,
            Proposition::Equal(a, b)
            | Proposition::LessThan(a, b)
            | Proposition::LessOrEqual(a, b)
            | Proposition::ScalarIeeeFloatComparison {
                left: a, right: b, ..
            } => self.scalar(a, source, destination) && self.scalar(b, source, destination),
            Proposition::IntegerMathEqual(a, b)
            | Proposition::IntegerMathLessThan(a, b)
            | Proposition::IntegerMathLessOrEqual(a, b) => {
                self.integer(a, source, destination) && self.integer(b, source, destination)
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                for field in [left, right] {
                    if field.root() != source {
                        return false;
                    }
                    let mut path = field.path().to_vec();
                    if self.path(&mut path).is_none() {
                        return false;
                    }
                    let Ok(rebound) =
                        semantic_vocabulary::IeeeFloatStructuralField::new(destination, path)
                    else {
                        return false;
                    };
                    *field = rebound;
                }
                true
            }
            Proposition::Conjunction(rows) | Proposition::Disjunction(rows) => rows
                .iter_mut()
                .all(|row| self.proposition(row, source, destination)),
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                self.proposition(premise, source, destination)
                    && self.proposition(conclusion, source, destination)
            }
            _ => false,
        }
    }
}

pub(super) fn join_component_types(
    source: &TerminalModule,
    destination: &TerminalModule,
    source_type: StructuralTypeId,
    destination_type: StructuralTypeId,
) -> Result<crate::component_types::ComponentTypePresentation, String> {
    let mut joined = TheoryJoin::new(source, destination);
    if !joined.carrier(source_type, destination_type) {
        return Err("component carrier declarations retain different logical graphs".into());
    }
    Ok(crate::component_types::ComponentTypePresentation::new(
        source_type,
        destination_type,
        joined.types,
        joined.field_names,
        joined.cases,
    ))
}
