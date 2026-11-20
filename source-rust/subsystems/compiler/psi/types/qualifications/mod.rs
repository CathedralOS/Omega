//! Domain declarations and qualifications on structural and scalar values.

mod requirements;
mod scalar;
pub use scalar::{
    ScalarDomainDeclaration, ScalarDomainEstablishmentRoute, ScalarDomainPredicate,
    ScalarFieldQualification, ScalarFloatRange, ScalarIntegerRange,
    ScalarPrimitiveQualification, ScalarQualificationCatalog, ScalarQualificationCoercion,
    ScalarQualificationSet, ScalarReturnOrigin, ScalarReturnQualification, ScalarReturnSite,
};

use crate::StructuralPathSegment;
use crate::TerminalIdentityIndex;
use semantic_vocabulary::{
    ContentAlgebra, ContentProjectionExpression, ContentProjectionIdentity,
    DomainSemanticId, StructuralDomainId, StructuralTypeId,
};

/// Keep the same plan while linking its descriptions to this module's
/// checked fields and functions. Erased fields keep their description
/// but have no storage path. These links supply no backing by themselves.
pub type StructuralDomainAccessPlan = access_admission::AccessPlan<
    (
        schema_reflection::SchemaNodeHandle,
        Option<Vec<StructuralPathSegment>>,
    ),
    (
        schema_reflection::CallableDescription,
        StructuralEstablishmentRoute,
    ),
>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralDomainDeclaration {
    pub required_memberships: Option<Vec<DomainSemanticId>>,
    pub complete_predicate_theory: bool,
    pub predicate: Option<StructuralDomainPredicate>,
    pub id: StructuralDomainId,
    pub semantic_domain: DomainSemanticId,
    pub identity: TerminalIdentityIndex,
    pub carrier: StructuralTypeId,
    /// Identity of the complete type selected by this domain, including
    /// lifetime and static arguments. The storage type can erase these
    /// details and cannot stand in for them.
    pub subject_identity: TerminalIdentityIndex,
    pub content_projection: Option<StructuralContentProjection>,
    pub byte_predicate: Option<crate::ByteSequencePredicate>,
    /// None adds no issuer restriction; Some(empty) permits nobody.
    /// This is separate from predicate truth and backing authority.
    pub establishment_routes: Option<Vec<StructuralEstablishmentRoute>>,
    /// Returned rules are independent of predicates and issuance.
    /// They require exact field/caller checks and backing supply.
    pub access_policy: Option<StructuralDomainAccessPlan>,
}

#[cfg(test)]
mod issuer_choice_tests {
    use super::*;

    // Both choices have zero alternative routes, but only an absent
    // issuer restriction can be treated as a pure predicate domain.
    #[test]
    fn explicit_empty_issuance_is_not_an_unrestricted_predicate() {
        let mut domain = StructuralDomainDeclaration {
            subject_identity: Default::default(),
            access_policy: None,
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: true,
            predicate: None,
            id: StructuralDomainId::new(1).unwrap(),
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            carrier: StructuralTypeId::new(1).unwrap(),
            content_projection: None,
            byte_predicate: None,
            establishment_routes: None,
        };
        assert!(domain.is_pure_predicate());
        domain.establishment_routes = Some(Vec::new());
        assert!(!domain.is_pure_predicate());
        assert!(!domain.authorizes_boundary_requirement(TerminalIdentityIndex(2), None));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralDomainPredicate {
    pub subject: semantic_vocabulary::PlaceId,
    pub proposition: semantic_vocabulary::Proposition,
}

impl StructuralDomainDeclaration {
    pub fn is_pure_predicate(&self) -> bool {
        self.complete_predicate_theory
            && self.has_no_required_memberships()
            && self.establishment_routes.is_none()
            && self.content_projection.is_none()
            && self
                .access_policy
                .as_ref()
                .is_none_or(|plan| plan.issuance.is_none() && !plan.has_operation_rules())
    }

    pub fn carrier_qualified_identity(family: &str, carrier_identity: &str) -> String {
        format!("{family}@{carrier_identity}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEstablishmentRoute {
    CheckedRequirement {
        declaration: crate::NominalDeclarationIdentity,
        requirement_identity: TerminalIdentityIndex,
    },
    ExactMachine {
        declaration: crate::NominalDeclarationIdentity,
        machine_identity: TerminalIdentityIndex,
    },
    BoundaryRequirement {
        declaration: crate::NominalDeclarationIdentity,
        requirement_identity: TerminalIdentityIndex,
    },
}

impl StructuralEstablishmentRoute {
    pub fn identity(&self) -> TerminalIdentityIndex {
        match self {
            Self::CheckedRequirement {
                requirement_identity,
                ..
            }
            | Self::BoundaryRequirement {
                requirement_identity,
                ..
            } => *requirement_identity,
            Self::ExactMachine {
                machine_identity, ..
            } => *machine_identity,
        }
    }

    pub fn declaration(&self) -> &crate::NominalDeclarationIdentity {
        match self {
            Self::CheckedRequirement { declaration, .. }
            | Self::BoundaryRequirement { declaration, .. }
            | Self::ExactMachine { declaration, .. } => declaration,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResultQualificationEstablishment {
    pub domain: StructuralDomainId,
    pub route: u32,
}

/// `value as +D` or `value as -D` on a structural value: the one
/// successor argument, at `argument_ordinal` of `edge`, whose fresh
/// destination parameter carries a different whole-value domain set
/// than its source place. The change strictly adds or strictly erases
/// whole-value memberships; projected memberships pass unchanged.
/// Each added domain with issuer routes names the route the enclosing
/// callable establishes it by; erasure and route-free additions carry
/// none. An ordinary transfer never implies a change.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralQualificationChange {
    pub machine: semantic_vocabulary::MachineId,
    pub edge: semantic_vocabulary::EdgeId,
    pub argument_ordinal: u32,
    pub source: semantic_vocabulary::PlaceId,
    pub destination: semantic_vocabulary::PlaceId,
    pub establishments: Vec<ResultQualificationEstablishment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralContentProjection {
    pub identity: ContentProjectionIdentity,
    pub algebra: ContentAlgebra,
    pub expression: ContentProjectionExpression,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct StructuralPathQualification {
    pub path: Vec<StructuralPathSegment>,
    pub domain: StructuralDomainId,
}

impl StructuralPathQualification {
    pub fn is_element_membership(&self) -> bool {
        matches!(self.path.first(), Some(StructuralPathSegment::AllElements))
    }
    pub fn selected_suffix<'a>(
        declared: &'a [StructuralPathSegment],
        selected: &[StructuralPathSegment],
    ) -> Option<&'a [StructuralPathSegment]> {
        if declared.len() < selected.len() {
            return None;
        }
        declared
            .iter()
            .zip(selected)
            .all(|(row, selection)| {
                row == selection
                    || matches!(
                        (row, selection),
                        (
                            StructuralPathSegment::AllElements,
                            StructuralPathSegment::FixedIndex(_)
                                | StructuralPathSegment::RuntimeIndex { .. }
                        )
                    )
            })
            .then_some(&declared[selected.len()..])
    }

    pub fn select_restorable(
        whole: &[StructuralDomainId],
        projected: &[Self],
        path: &[StructuralPathSegment],
    ) -> Option<(Vec<StructuralDomainId>, Vec<Self>)> {
        if (!path.is_empty() && !whole.is_empty())
            || projected.iter().any(|row| {
                row.path.len() < path.len()
                    && Self::selected_suffix(&row.path, &path[..row.path.len()]).is_some()
            })
        {
            return None;
        }
        Some(Self::select(whole, projected, path))
    }

    pub fn append_under(
        whole: &[StructuralDomainId],
        projected: &[Self],
        prefix: &[StructuralPathSegment],
        destination: &mut Vec<Self>,
    ) {
        for domain in whole {
            destination.push(Self {
                path: prefix.to_vec(),
                domain: *domain,
            });
        }
        for row in projected {
            let mut path = prefix.to_vec();
            path.extend(row.path.iter().cloned());
            destination.push(Self {
                path,
                domain: row.domain,
            });
        }
    }

    pub fn select(
        whole: &[StructuralDomainId],
        projected: &[Self],
        path: &[StructuralPathSegment],
    ) -> (Vec<StructuralDomainId>, Vec<Self>) {
        let mut qualifications = if path.is_empty() {
            whole.to_vec()
        } else {
            Vec::new()
        };
        let mut descendants = Vec::new();
        for row in projected {
            if let Some(suffix) = Self::selected_suffix(&row.path, path) {
                if suffix.is_empty() {
                    qualifications.push(row.domain);
                } else {
                    descendants.push(Self {
                        path: suffix.to_vec(),
                        domain: row.domain,
                    });
                }
            }
        }
        qualifications.sort();
        qualifications.dedup();
        descendants.sort();
        descendants.dedup();
        (qualifications, descendants)
    }
}
