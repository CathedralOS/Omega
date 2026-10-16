use crate::StructuralPathSegment;
use crate::terminal_module::TerminalIdentityIndex;
use semantic_vocabulary::{
    ContentAlgebra, ContentProjectionExpression, ContentProjectionIdentity, DomainSemanticId,
    StructuralDomainId, StructuralTypeId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralDomainDeclaration {
    /// Complete closed predicate definition with its own declared formal subject.
    /// None is opaque; a retained Truth is an explicitly empty predicate theory.
    pub predicate: Option<StructuralDomainPredicate>,
    pub id: StructuralDomainId,
    /// Stable source-free semantic-domain identity, distinct from this
    /// module-local dense declaration ID.
    pub semantic_domain: DomainSemanticId,
    pub identity: TerminalIdentityIndex,
    /// Exact carrier accepted by this domain. Qualification never changes the
    /// runtime carrier and never authorizes its own establishment.
    pub carrier: StructuralTypeId,
    /// Owner-unique normalized `Content<Owner, A>` definition, when this
    /// qualification is content-bearing. This row is independent of any
    /// boundary route that may introduce a program-local occurrence; those
    /// routes must replay this exact definition rather than restating one.
    pub content_projection: Option<StructuralContentProjection>,
    /// Exact plain primitive definition. Routed, indexed and content domains
    /// cannot acquire this authority merely from a matching byte sequence.
    pub byte_predicate: Option<crate::ByteSequencePredicate>,
    /// Domain-owned catalog of routes authorized to establish this
    /// qualification, strictly ordered. Consumers can neither append routes
    /// nor substitute a same-spelled issuer, and private issuer identities
    /// are retained here without granting consumer call, conformance, or
    /// private-type access.
    pub establishment_routes: Vec<StructuralEstablishmentRoute>,
}

/// A domain-local namespace. The explicit subject is bound to the declaration's
/// carrier, never a machine's ambient place or SSA namespace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralDomainPredicate {
    pub subject: semantic_vocabulary::PlaceId,
    pub proposition: semantic_vocabulary::Proposition,
}

impl StructuralDomainDeclaration {
    /// Authorization belongs to this exact boundary requirement, not another
    /// conformer or a private issuer whose value the provider merely forwards.
    pub fn authorizes_boundary_requirement(&self, requirement: TerminalIdentityIndex) -> bool {
        self.establishment_routes.iter().any(|route| {
            matches!(route, StructuralEstablishmentRoute::BoundaryRequirement {
                requirement: authorized,
            } if *authorized == requirement)
        })
    }

    /// The `identity` of a semantic-domain family declared on one exact
    /// carrier: the family's normalized identity qualified by the carrier's
    /// normalized type identity, such as
    /// `Extent::Granted@named(name(Extent))`. A family can be instantiated
    /// on several carriers in one module, so the identity is always
    /// qualified and pruning one instantiation cannot rename another.
    /// Consumers that join a module's domain row to a family and carrier
    /// named elsewhere rebuild the identity here instead of spelling it.
    pub fn carrier_qualified_identity(family: &str, carrier_identity: &str) -> String {
        format!("{family}@{carrier_identity}")
    }
}

/// One route authorized by a domain declaration to establish its
/// qualification. The retained identity is the replay authority: naming a
/// machine is neither trait sealing nor an implicit grant to other machines
/// in its package.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEstablishmentRoute {
    /// An exact invocation through this requirement under a valid selected
    /// conformance; `requirement` is the requirement's canonical identity.
    Requirement { requirement: TerminalIdentityIndex },
    /// That exact machine's invocation — not another conformer of a
    /// requirement it happens to satisfy. `machine` is the machine's
    /// canonical callable identity.
    ExactMachine { machine: TerminalIdentityIndex },
    /// An installed external-root invocation admitted under this boundary
    /// requirement; `requirement` is its canonical boundary identity.
    BoundaryRequirement { requirement: TerminalIdentityIndex },
}

/// Establishment evidence binding one domain membership on an operation
/// result to the authorized route that issued it. The establishing
/// occurrence is the enclosing operation itself: replay resolves that
/// operation's callee identity and compares it to the authorized route
/// declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResultQualificationEstablishment {
    /// Domain membership established on this result. Must name a member of
    /// the result's `qualifications` or a projected row's domain.
    pub domain: StructuralDomainId,
    /// Index into that domain declaration's `establishment_routes` catalog.
    pub route: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralContentProjection {
    pub identity: ContentProjectionIdentity,
    pub algebra: ContentAlgebra,
    pub expression: ContentProjectionExpression,
}

/// One exact qualification carried by a nonempty structural path beneath a
/// parameter root. The path is occurrence-relative, not a type-wide rule: a
/// qualification on one field never qualifies a sibling, prefix, or root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralPathQualification {
    pub path: Vec<StructuralPathSegment>,
    pub domain: StructuralDomainId,
}

impl StructuralPathQualification {
    /// A proof about every element, independent of descriptor storage and access.
    pub fn is_element_membership(&self) -> bool {
        matches!(self.path.first(), Some(StructuralPathSegment::AllElements))
    }
    /// An authenticated universal row may specialize at a checked element.
    /// A concrete element never generalizes to the whole collection.
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

    /// Select memberships that a same-typed restoration can preserve. A
    /// predicate on a strict ancestor may relate the hole to its siblings;
    /// replacing the hole cannot re-establish that opaque predicate. Exact
    /// hole and descendant memberships move with the selected value instead.
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

    /// Preserve a completed child's memberships below its new parent path.
    /// The prefix identifies storage, not a receiving declaration's requirements;
    /// callers supply actual memberships after selecting the operand subtree.
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

    /// Select the exact subtree's memberships. A descendant's domain becomes
    /// whole-value membership only when the selected path ends at that row;
    /// disjoint siblings and ancestor domains do not qualify the selection.
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
