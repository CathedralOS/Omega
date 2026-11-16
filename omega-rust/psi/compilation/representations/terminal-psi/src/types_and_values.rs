/*
Scalar and structural type declarations (`ScalarType`, `StructuralTypeDeclaration`,
`StructuralTypeShape`), domain qualifications, structural places, path projections, reference
origins, and machine signatures in `TerminalModule`.
*/

pub mod types {

    pub(crate) mod atomic_outcomes {
        use crate::{
            StructuralCaseDeclaration, StructuralFieldType, StructuralTypeDeclaration,
            StructuralTypeShape,
        };
        use semantic_vocabulary::ScalarType;

        pub fn atomic_once_outcome_cases(
            declaration: &StructuralTypeDeclaration,
            resident: ScalarType,
            custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
        ) -> Option<&[StructuralCaseDeclaration; 3]> {
            if !custody.is_canonical()
                || declaration.quotient_carrier.is_some()
                || declaration.owns_linear_claim
                || declaration.scalar_restriction.is_some()
            {
                return None;
            }
            let StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return None;
            };
            let cases: &[StructuralCaseDeclaration; 3] = cases.as_slice().try_into().ok()?;
            for (case, schema) in cases.iter().zip(custody.outcome_identity.cases()) {
                match schema.payload {
                    None if case.fields.is_empty() => {}
                    Some(_) => {
                        let [field] = case.fields.as_slice() else {
                            return None;
                        };
                        if field.relevance.is_erased()
                            || field.field_type != StructuralFieldType::Scalar(resident)
                            || field.storage.is_some()
                        {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
            Some(cases)
        }

        pub fn atomic_once_result_is_plain(
            result: &crate::StructuralOperationResult,
            custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
        ) -> bool {
            use language_core::atomic::AtomicCompareExchangeObservationAxis;
            custody.is_canonical()
                && result.qualification_establishments.is_empty()
                && match custody.observation {
                    AtomicCompareExchangeObservationAxis::Observing => {
                        result.is_unrestricted_plain()
                    }
                    AtomicCompareExchangeObservationAxis::NonObserving => result.is_affine_plain(),
                }
        }
    }
    pub use atomic_outcomes::{atomic_once_outcome_cases, atomic_once_result_is_plain};

    pub(crate) mod domain_requirements {

        use super::{ScalarDomainDeclaration, StructuralDomainDeclaration};
        use semantic_vocabulary::{DomainSemanticId, ScalarDomainId, StructuralDomainId};

        trait RequirementNode {
            type Carrier: Copy + Eq + Ord;

            fn semantic_domain(&self) -> DomainSemanticId;
            fn carrier(&self) -> Self::Carrier;
            fn required(&self) -> Option<&[DomainSemanticId]>;
        }

        impl RequirementNode for StructuralDomainDeclaration {
            type Carrier = semantic_vocabulary::StructuralTypeId;

            fn semantic_domain(&self) -> DomainSemanticId {
                self.semantic_domain
            }
            fn carrier(&self) -> Self::Carrier {
                self.carrier
            }
            fn required(&self) -> Option<&[DomainSemanticId]> {
                self.required_memberships.as_deref()
            }
        }

        impl RequirementNode for ScalarDomainDeclaration {
            type Carrier = semantic_vocabulary::ScalarType;

            fn semantic_domain(&self) -> DomainSemanticId {
                self.semantic_domain
            }
            fn carrier(&self) -> Self::Carrier {
                self.carrier
            }
            fn required(&self) -> Option<&[DomainSemanticId]> {
                self.required_memberships.as_deref()
            }
        }

        fn resolve<Node: RequirementNode>(
            rows: &[Node],
            domain: DomainSemanticId,
            carrier: Node::Carrier,
        ) -> Option<&Node> {
            let mut matches = rows
                .iter()
                .filter(|row| row.semantic_domain() == domain && row.carrier() == carrier);
            let row = matches.next()?;
            matches.next().is_none().then_some(row)
        }

        struct Closure<'a, Node> {
            nodes: Vec<&'a Node>,
            complete: bool,
        }

        fn walk<'a, Node: RequirementNode>(
            root: &Node,
            rows: &'a [Node],
            mut prune: impl FnMut(DomainSemanticId) -> bool,
        ) -> Option<Vec<&'a Node>> {
            let carrier = root.carrier();
            let mut pending = vec![(root.semantic_domain(), false)];
            let mut active = Vec::new();
            let mut visited = Vec::new();
            let mut nodes = Vec::new();
            while let Some((domain, completed)) = pending.pop() {
                if completed {
                    if active.pop() != Some(domain) {
                        return None;
                    }
                    visited.push(domain);
                    continue;
                }
                if prune(domain) {
                    continue;
                }
                if active.contains(&domain) {
                    return None;
                }
                if visited.contains(&domain) {
                    continue;
                }
                let row = resolve(rows, domain, carrier)?;
                nodes.push(row);
                active.push(domain);
                pending.push((domain, true));
                if let Some(required) = row.required() {
                    if !required.windows(2).all(|pair| pair[0] < pair[1]) {
                        return None;
                    }
                    pending.extend(required.iter().rev().map(|domain| (*domain, false)));
                }
            }
            Some(nodes)
        }

        fn closure<'a, Node: RequirementNode>(
            root: &Node,
            rows: &'a [Node],
        ) -> Option<Closure<'a, Node>> {
            let nodes = walk(root, rows, |_| false)?;
            let complete = nodes.iter().all(|node| node.required().is_some());
            Some(Closure { nodes, complete })
        }

        fn supplied_closures<'a, Node: RequirementNode, Id: Copy + Eq>(
            root: &Node,
            rows: &'a [Node],
            supplied: &[Id],
            local_id: impl Fn(&Node) -> Id,
        ) -> Option<Vec<Closure<'a, Node>>> {
            supplied
                .iter()
                .map(|id| {
                    let mut matches = rows
                        .iter()
                        .filter(|row| local_id(row) == *id && row.carrier() == root.carrier());
                    let row = matches.next()?;
                    matches.next().is_none().then_some(())?;
                    closure(resolve(rows, row.semantic_domain(), root.carrier())?, rows)
                })
                .collect()
        }

        // Prepare one answer per declaration. Resolve each required identity
        // once; missing/ambiguous edges and cycles cannot satisfy the query.
        fn requirements_satisfy<Node: RequirementNode>(
            rows: &[Node],
            local: impl Fn(&Node) -> bool,
        ) -> Vec<bool> {
            let mut index: Vec<_> = rows
                .iter()
                .enumerate()
                .map(|(position, row)| ((row.carrier(), row.semantic_domain()), position))
                .collect();
            index.sort_unstable_by_key(|row| row.0);
            let mut answers: Vec<_> = rows.iter().map(&local).collect();
            let mut remaining = vec![0usize; rows.len()];
            let mut dependents = vec![Vec::new(); rows.len()];
            for (position, row) in rows.iter().enumerate() {
                let own = index
                    .binary_search_by_key(&(row.carrier(), row.semantic_domain()), |row| row.0)
                    .expect("the index contains each input declaration");
                if (own > 0 && index[own - 1].0 == index[own].0)
                    || index
                        .get(own + 1)
                        .is_some_and(|next| next.0 == index[own].0)
                {
                    answers[position] = false;
                }
                let Some(required) = row
                    .required()
                    .filter(|required| required.windows(2).all(|pair| pair[0] < pair[1]))
                else {
                    answers[position] = false;
                    continue;
                };
                for required in required {
                    let key = (row.carrier(), *required);
                    let Ok(found) = index.binary_search_by_key(&key, |row| row.0) else {
                        answers[position] = false;
                        continue;
                    };
                    if (found > 0 && index[found - 1].0 == key)
                        || index.get(found + 1).is_some_and(|next| next.0 == key)
                    {
                        answers[position] = false;
                        continue;
                    }
                    let dependency = index[found].1;
                    remaining[position] += 1;
                    dependents[dependency].push(position);
                }
            }
            let mut pending: std::collections::VecDeque<_> = remaining
                .iter()
                .enumerate()
                .filter_map(|(position, count)| (*count == 0).then_some(position))
                .collect();
            while let Some(position) = pending.pop_front() {
                for dependent in &dependents[position] {
                    answers[*dependent] &= answers[position];
                    remaining[*dependent] -= 1;
                    if remaining[*dependent] == 0 {
                        pending.push_back(*dependent);
                    }
                }
            }
            for (answer, remaining) in answers.iter_mut().zip(remaining) {
                *answer &= remaining == 0;
            }
            answers
        }

        impl StructuralDomainDeclaration {
            /// Prepare which already held memberships have no content facts
            /// to invalidate. This is not authority to establish a membership.
            pub fn content_independent_memberships(domains: &[Self]) -> Vec<StructuralDomainId> {
                let mut identities: Vec<_> = domains.iter().map(|row| row.id).collect();
                identities.sort_unstable();
                if identities.windows(2).any(|pair| pair[0] == pair[1]) {
                    return Vec::new();
                }
                let answers = requirements_satisfy(domains, |row| {
                    row.complete_predicate_theory
                        && row.predicate.as_ref().is_some_and(|predicate| {
                            matches!(
                                predicate.proposition,
                                semantic_vocabulary::Proposition::Truth
                            )
                        })
                        && row.byte_predicate.is_none()
                        && row.content_projection.is_none()
                });
                let mut held: Vec<_> = domains
                    .iter()
                    .zip(answers)
                    .filter_map(|(row, holds)| holds.then_some(row.id))
                    .collect();
                held.sort_unstable();
                held
            }
        }

        macro_rules! requirement_queries {
            ($declaration:ty, $id:ty) => {
                impl $declaration {
                    pub fn has_no_required_memberships(&self) -> bool {
                        self.required().is_some_and(<[DomainSemanticId]>::is_empty)
                    }

                    pub fn required_memberships_are_well_formed(&self, domains: &[Self]) -> bool {
                        closure(self, domains).is_some()
                    }

                    pub fn required_domain_closure<'a>(
                        &self,
                        domains: &'a [Self],
                    ) -> Option<Vec<&'a Self>> {
                        let closure = closure(self, domains)?;
                        closure.complete.then_some(closure.nodes)
                    }

                    pub fn missing_membership_requirements<'a>(
                        &self,
                        domains: &'a [Self],
                        supplied: &[$id],
                    ) -> Option<Vec<&'a Self>> {
                        let supplied = supplied_closures(self, domains, supplied, |row| row.id)?;
                        let nodes = walk(self, domains, |domain| {
                            supplied.iter().any(|closure| {
                                closure
                                    .nodes
                                    .iter()
                                    .any(|row| row.semantic_domain() == domain)
                            })
                        })?;
                        nodes
                            .iter()
                            .all(|node| node.required().is_some())
                            .then_some(nodes)
                    }
                }
            };
        }

        requirement_queries!(StructuralDomainDeclaration, StructuralDomainId);
        requirement_queries!(ScalarDomainDeclaration, ScalarDomainId);
    }
    pub(crate) mod qualifications {
        use crate::StructuralPathSegment;
        use crate::terminal_module::TerminalIdentityIndex;
        use semantic_vocabulary::{
            ContentAlgebra, ContentProjectionExpression, ContentProjectionIdentity,
            DomainSemanticId, StructuralDomainId, StructuralTypeId,
        };

        /// Keep the same plan while linking its descriptions to this module's
        /// checked fields and functions. Erased fields keep their description
        /// but have no storage path. These links supply no backing by themselves.
        pub type StructuralDomainAccessPlan = access_plans::AccessPlan<
            (
                language_semantics::reflection::SchemaNodeHandle,
                Option<Vec<StructuralPathSegment>>,
            ),
            (
                language_semantics::reflection::CallableDescription,
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
                    && self.access_policy.as_ref().is_none_or(|plan| {
                        plan.issuance.is_none()
                            && plan.readers.is_empty()
                            && plan.writers.is_empty()
                            && plan.observations.is_empty()
                            && plan.reach.services().len() == 0
                    })
            }

            pub fn authorizes_boundary_requirement(
                &self,
                requirement: TerminalIdentityIndex,
                declaration: Option<TerminalIdentityIndex>,
            ) -> bool {
                self.establishment_routes
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .any(|route| {
                        matches!(route, StructuralEstablishmentRoute::BoundaryRequirement {
                        requirement_identity: authorized,
                        declaration: owner,
                    } if *authorized == requirement && Some(owner.identity()) == declaration)
                    })
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

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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
    }
    pub(crate) mod scalar_qualifications {

        use crate::terminal_module::TerminalIdentityIndex;
        use semantic_vocabulary::{
            DomainSemanticId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType, IntegerValue,
            MachineId, ScalarDomainId, ScalarQualificationSetId, ScalarType, ValueId,
        };

        #[derive(Debug, Clone, Default, PartialEq, Eq)]
        pub struct ScalarQualificationCatalog {
            pub fields: Vec<ScalarFieldQualification>,
            /// Complete scalar types of addressable primitive storage. A
            /// reference or scalar home cannot erase its referent's domains.
            pub primitives: Vec<ScalarPrimitiveQualification>,
            pub returns: Vec<ScalarReturnQualification>,
            pub domains: Vec<ScalarDomainDeclaration>,
            pub sets: Vec<ScalarQualificationSet>,
            pub coercions: Vec<ScalarQualificationCoercion>,
            pub float_entry_ranges: Vec<ScalarFloatRange>,
            pub integer_entry_ranges: Vec<ScalarIntegerRange>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ScalarFieldQualification {
            pub field: semantic_vocabulary::StructuralFieldId,
            pub value_type: semantic_vocabulary::QualifiedScalarType,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ScalarPrimitiveQualification {
            pub structural_type: semantic_vocabulary::StructuralTypeId,
            pub value_type: semantic_vocabulary::QualifiedScalarType,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ScalarDomainDeclaration {
            pub required_memberships: Option<Vec<DomainSemanticId>>,
            pub predicate: Option<ScalarDomainPredicate>,
            pub id: ScalarDomainId,
            pub semantic_domain: DomainSemanticId,
            pub identity: TerminalIdentityIndex,
            pub carrier: ScalarType,
            /// None adds no issuer restriction; Some(empty) permits nobody.
            pub establishment_routes: Option<Vec<ScalarDomainEstablishmentRoute>>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ScalarDomainPredicate {
            pub subject: ValueId,
            pub proposition: semantic_vocabulary::Proposition,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub enum ScalarReturnOrigin {
            Forwarded,
            Established {
                route: u32,
            },
            Vacuous,
            /// An unrestricted predicate domain introduced at the exact
            /// completion whose arrival proves the retained domain predicate.
            Proved,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ScalarReturnQualification {
            pub site: ScalarReturnSite,
            pub machine: MachineId,
            pub edge: EdgeId,
            pub source: ValueId,
            pub result: ValueId,
            pub domain: ScalarDomainId,
            pub origins: Vec<ScalarReturnOrigin>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ScalarReturnSite {
            NormalExit,
            Completion { argument_ordinal: u32 },
        }

        pub use super::qualifications::StructuralEstablishmentRoute as ScalarDomainEstablishmentRoute;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ScalarQualificationSet {
            pub id: ScalarQualificationSetId,
            pub domains: Vec<ScalarDomainId>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ScalarQualificationCoercion {
            pub machine: MachineId,
            pub edge: EdgeId,
            pub argument_ordinal: u32,
            pub source: ValueId,
            pub destination: ValueId,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct ScalarFloatRange {
            pub machine: MachineId,
            pub parameter: ValueId,
            pub minimum: IeeeFloatValue,
            pub maximum: IeeeFloatValue,
            pub maximum_inclusive: bool,
        }

        impl ScalarFloatRange {
            fn decode(value: IeeeFloatValue) -> f64 {
                match value {
                    IeeeFloatValue::Binary32(bits) => f32::from_bits(bits) as f64,
                    IeeeFloatValue::Binary64(bits) => f64::from_bits(bits),
                }
            }

            pub fn format(&self) -> IeeeFloatFormat {
                self.minimum.format()
            }

            pub fn ordered(&self) -> bool {
                self.minimum.format() == self.maximum.format()
                    && Self::decode(self.minimum) <= Self::decode(self.maximum)
            }

            pub fn contains(&self, value: IeeeFloatValue) -> bool {
                if value.format() != self.format() {
                    return false;
                }
                let candidate = Self::decode(value);
                candidate >= Self::decode(self.minimum)
                    && (candidate < Self::decode(self.maximum)
                        || (self.maximum_inclusive && candidate == Self::decode(self.maximum)))
            }

            pub fn contains_range(&self, inner: &Self) -> bool {
                if inner.format() != self.format() {
                    return false;
                }
                let inner_minimum = Self::decode(inner.minimum);
                let outer_minimum = Self::decode(self.minimum);
                let inner_maximum = Self::decode(inner.maximum);
                let outer_maximum = Self::decode(self.maximum);
                inner_minimum >= outer_minimum
                    && (inner_maximum < outer_maximum
                        || (inner_maximum == outer_maximum
                            && (self.maximum_inclusive || !inner.maximum_inclusive)))
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct ScalarIntegerRange {
            pub machine: MachineId,
            pub parameter: ValueId,
            pub integer_type: IntegerType,
            pub minimum: IntegerValue,
            pub maximum: IntegerValue,
        }

        impl ScalarIntegerRange {
            pub fn ordered(&self) -> bool {
                !self.integer_type.is_address()
                    && self
                        .integer_type
                        .compare(self.minimum, self.maximum)
                        .is_some_and(|ordering| !ordering.is_gt())
            }

            pub fn contains(&self, value: IntegerValue) -> bool {
                self.integer_type.admits(value)
                    && self
                        .integer_type
                        .compare(self.minimum, value)
                        .is_some_and(|ordering| !ordering.is_gt())
                    && self
                        .integer_type
                        .compare(value, self.maximum)
                        .is_some_and(|ordering| !ordering.is_gt())
            }

            pub fn contains_range(&self, inner: &Self) -> bool {
                self.integer_type == inner.integer_type
                    && self
                        .integer_type
                        .compare(self.minimum, inner.minimum)
                        .is_some_and(|ordering| !ordering.is_gt())
                    && self
                        .integer_type
                        .compare(inner.maximum, self.maximum)
                        .is_some_and(|ordering| !ordering.is_gt())
            }
        }
    }
    pub(crate) mod structural {
        use crate::terminal_module::TerminalIdentityIndex;
        use language_core::BindingRelevance;
        use semantic_vocabulary::{
            IeeeFloatFormat, ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId,
        };

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct StructuralTypeDeclaration {
            /// Exact source owner; absent for compiler-generated declarations.
            pub declaration_identity: TerminalIdentityIndex,
            pub quotient_carrier: Option<StructuralTypeId>,
            pub id: StructuralTypeId,
            pub identity: TerminalIdentityIndex,
            /// This declaration's intrinsic root debt, independent of its
            /// declared multiplicity and claims retained in owned components.
            pub owns_linear_claim: bool,
            pub multiplicity: crate::StructuralMultiplicity,
            pub no_code_affine_cleanup: bool,
            pub descriptor_cleanup: bool,
            pub nominal_cleanup: Option<crate::NominalCleanupSelection>,
            pub shape: StructuralTypeShape,
            pub scalar_restriction: Option<StructuralFieldType>,
            /// The data declaration's `requires` clauses, in authored order:
            /// the declaration-owned requirement every construction and
            /// scalar-field replacement proves and every validated read
            /// instantiates (structural_access.md, "Complete record
            /// construction"). A clause names the declaration's own fields
            /// by `StructuralFieldId`, with `self` as place 1, and its cases
            /// by `StructuralCaseId`. Empty for every type of a module
            /// whose producer proves them elsewhere; only a record or a sum
            /// declares any.
            pub requires: Vec<semantic_vocabulary::Proposition>,
        }

        impl StructuralTypeDeclaration {
            pub const fn is_copyable(&self) -> bool {
                matches!(
                    self.multiplicity,
                    crate::StructuralMultiplicity::Unrestricted
                )
            }

            pub fn is_float_class_shape(&self) -> bool {
                let StructuralTypeShape::Sum { cases } = &self.shape else {
                    return false;
                };
                let [nan, infinity, normal, subnormal, zero] = cases.as_slice() else {
                    return false;
                };
                nan.fields.is_empty()
                    && [infinity, normal, subnormal, zero].iter().all(|case| {
                        matches!(case.fields.as_slice(), [field] if !field.relevance.is_erased()
                        && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean))
                    })
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct StructuralReferentIdentity {
            pub id: StructuralTypeId,
            pub identity: TerminalIdentityIndex,
            pub foreign_descriptor: bool,
            pub abstract_opaque: bool,
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum StructuralTypeShape {
            /// A nominal value supplied across an admitted boundary. It has no
            /// guest-visible fields or implied storage geometry; moving it
            /// preserves the supplying host's identity, rather than constructing
            /// an empty record with the same name.
            Opaque,
            Reference {
                referent: StructuralTypeId,
                access: crate::StructuralAccess,
            },
            PrimitiveScalar(ScalarType),
            ByteSequence(ByteSequenceCarrier),
            ElementView {
                element: StructuralTypeId,
                access: Option<crate::StructuralAccess>,
            },
            Record {
                fields: Vec<StructuralFieldDeclaration>,
                storage: crate::StructuralRecordStorage,
            },
            FixedArray {
                element: StructuralTypeId,
                length: crate::ArrayExtent,
            },
            Sum {
                cases: Vec<StructuralCaseDeclaration>,
            },
            Mixed {
                fields: Vec<StructuralFieldDeclaration>,
                cases: Vec<StructuralCaseDeclaration>,
            },
            AtomicCell(semantic_vocabulary::AtomicCellType),
        }

        impl StructuralTypeShape {
            pub fn cases(&self) -> &[StructuralCaseDeclaration] {
                match self {
                    Self::Sum { cases } | Self::Mixed { cases, .. } => cases,
                    _ => &[],
                }
            }

            pub fn cases_mut(&mut self) -> &mut [StructuralCaseDeclaration] {
                match self {
                    Self::Sum { cases } | Self::Mixed { cases, .. } => cases,
                    _ => &mut [],
                }
            }

            pub fn fields_with_case(
                &self,
            ) -> impl Iterator<
                Item = (
                    Option<&StructuralCaseDeclaration>,
                    &StructuralFieldDeclaration,
                ),
            > {
                let fields = match self {
                    Self::Record { fields, .. } | Self::Mixed { fields, .. } => fields.as_slice(),
                    _ => &[],
                };
                fields.iter().map(|field| (None, field)).chain(
                    self.cases()
                        .iter()
                        .flat_map(|case| case.fields.iter().map(move |field| (Some(case), field))),
                )
            }

            pub fn fields(&self) -> impl Iterator<Item = &StructuralFieldDeclaration> {
                self.fields_with_case().map(|(_, field)| field)
            }

            pub fn fields_mut(&mut self) -> impl Iterator<Item = &mut StructuralFieldDeclaration> {
                let (fields, cases): (
                    &mut [StructuralFieldDeclaration],
                    &mut [StructuralCaseDeclaration],
                ) = match self {
                    Self::Record { fields, .. } => (fields, &mut []),
                    Self::Sum { cases } => (&mut [], cases),
                    Self::Mixed { fields, cases } => (fields, cases),
                    _ => (&mut [], &mut []),
                };
                fields
                    .iter_mut()
                    .chain(cases.iter_mut().flat_map(|case| &mut case.fields))
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct StructuralCaseDeclaration {
            /// Exact source owner; absent for compiler-generated declarations.
            pub declaration_identity: TerminalIdentityIndex,
            pub id: StructuralCaseId,
            pub identity: TerminalIdentityIndex,
            pub fields: Vec<StructuralFieldDeclaration>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct StructuralFieldDeclaration {
            /// A compiler-generated field has no source declaration and cannot
            /// satisfy a policy naming a source field, even if its name matches.
            pub declaration_identity: TerminalIdentityIndex,
            pub inline_carrier: Option<StructuralTypeId>,
            pub id: StructuralFieldId,
            pub identity: TerminalIdentityIndex,
            pub relevance: BindingRelevance,
            pub field_type: StructuralFieldType,
            pub storage: Option<semantic_vocabulary::AuthoredFieldStorage>,
        }

        pub fn field_by_identity(
            fields: &[StructuralFieldDeclaration],
            identity: TerminalIdentityIndex,
        ) -> Option<&StructuralFieldDeclaration> {
            fields.iter().find(|field| field.identity == identity)
        }

        pub fn live_field_by_identity(
            fields: &[StructuralFieldDeclaration],
            identity: TerminalIdentityIndex,
        ) -> Option<&StructuralFieldDeclaration> {
            fields
                .iter()
                .find(|field| field.identity == identity && !field.relevance.is_erased())
        }

        pub fn unique_structural_declaration<'d>(
            declarations: &'d [StructuralTypeDeclaration],
            id: StructuralTypeId,
            visiting: &[StructuralTypeId],
        ) -> Option<&'d StructuralTypeDeclaration> {
            if visiting.contains(&id) {
                return None;
            }
            let mut matches = declarations
                .iter()
                .filter(|declaration| declaration.id == id);
            let declaration = matches.next()?;
            matches.next().is_none().then_some(declaration)
        }

        impl StructuralTypeDeclaration {
            pub fn is_record_shape(&self) -> bool {
                matches!(self.shape, StructuralTypeShape::Record { .. })
            }

            pub fn is_bounded_owned_byte_sequence(&self) -> bool {
                matches!(
                    self.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
                )
            }
        }

        impl StructuralTypeShape {
            /// A view value carries its descriptor, including retained payload
            /// access. A reference to descriptor storage has a Reference shape.
            pub const fn is_view_descriptor(&self) -> bool {
                self.is_borrowed_byte_carrier() || matches!(self, Self::ElementView { .. })
            }

            /// Descriptors own a carrier whose retirement ends its retained permission.
            pub const fn is_borrowed_descriptor(&self) -> bool {
                matches!(self, Self::Reference { .. }) || self.is_view_descriptor()
            }

            pub fn is_canonical_view_descriptor(&self) -> bool {
                matches!(
                    self,
                    StructuralTypeShape::ElementView { access: None, .. }
                        | StructuralTypeShape::ByteSequence(
                            ByteSequenceCarrier::BorrowedView { access: None }
                                | ByteSequenceCarrier::BoundedView { access: None, .. }
                        )
                )
            }

            pub fn is_composite_shape(&self) -> bool {
                matches!(
                    self,
                    crate::record_or_fixed_array_shape_pattern!()
                        | crate::sum_or_mixed_shape_pattern!()
                )
            }

            pub fn is_borrowed_view_byte_sequence(&self) -> bool {
                matches!(
                    self,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
                )
            }
        }

        impl StructuralTypeShape {
            pub fn direct_fields(&self) -> Option<&[StructuralFieldDeclaration]> {
                match self {
                    Self::Record { fields, .. } | Self::Mixed { fields, .. } => Some(fields),
                    _ => None,
                }
            }

            pub fn record_fields(&self) -> Option<&[StructuralFieldDeclaration]> {
                match self {
                    Self::Record { fields, .. } => Some(fields),
                    _ => None,
                }
            }

            pub fn sum_cases(&self) -> Option<&[StructuralCaseDeclaration]> {
                match self {
                    Self::Sum { cases } | Self::Mixed { cases, .. } => Some(cases),
                    _ => None,
                }
            }

            pub const fn is_bounded_owned_byte_sequence(&self) -> bool {
                matches!(
                    self,
                    Self::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
                )
            }

            pub const fn is_shared_readable_byte_view(&self) -> bool {
                matches!(
                    self,
                    Self::ByteSequence(carrier) if carrier.is_shared_readable_view()
                )
            }

            pub const fn is_borrowed_byte_view(&self) -> bool {
                matches!(
                    self,
                    Self::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
                )
            }

            pub const fn is_borrowed_byte_carrier(&self) -> bool {
                matches!(
                    self,
                    Self::ByteSequence(crate::byte_sequence_view_carrier_pattern!())
                )
            }

            pub const fn is_owned_or_shared_byte_carrier(&self) -> bool {
                matches!(
                    self,
                    Self::ByteSequence(carrier) if carrier.is_owned_or_shared_byte_carrier()
                )
            }
        }

        impl ByteSequenceCarrier {
            pub const fn supports_field_predicates(&self) -> bool {
                matches!(
                    self,
                    Self::BoundedOwned { .. }
                        | Self::BorrowedView {
                            access: Some(crate::StructuralAccess::SharedBorrow),
                        }
                )
            }

            pub const fn is_borrowed(&self) -> bool {
                matches!(self, Self::BorrowedView { .. } | Self::BoundedView { .. })
            }

            pub const fn is_shared_readable_view(&self) -> bool {
                matches!(
                    self,
                    Self::BorrowedView {
                        access: None | Some(crate::StructuralAccess::SharedBorrow),
                    }
                )
            }

            pub const fn is_owned_or_shared_byte_carrier(&self) -> bool {
                matches!(
                    self,
                    Self::BoundedOwned { .. }
                        | Self::BorrowedView {
                            access: None | Some(crate::StructuralAccess::SharedBorrow),
                        }
                )
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum StructuralFieldType {
            Scalar(ScalarType),
            Atomic(ScalarType),
            AtomicBoundedInteger(semantic_vocabulary::BoundedIntegerType),
            BoundedInteger(semantic_vocabulary::BoundedIntegerType),
            RefinedScalar(semantic_vocabulary::ScalarRefinement),
            IeeeFloat(IeeeFloatFormat),
            ByteSequence(ByteSequenceCarrier),
            PredicateByteSequence {
                carrier: ByteSequenceCarrier,
                predicates: crate::ByteSequencePredicateSet,
            },
            Structural(StructuralTypeId),
            Erased {
                type_identity: TerminalIdentityIndex,
            },
        }

        impl From<semantic_vocabulary::AtomicCellType> for StructuralFieldType {
            fn from(cell: semantic_vocabulary::AtomicCellType) -> Self {
                match cell {
                    semantic_vocabulary::AtomicCellType::BoundedInteger(bounds) => {
                        Self::AtomicBoundedInteger(bounds)
                    }
                    cell => Self::Atomic(cell.scalar_type()),
                }
            }
        }

        impl StructuralFieldType {
            pub fn atomic_cell_type(&self) -> Option<semantic_vocabulary::AtomicCellType> {
                use semantic_vocabulary::AtomicCellType;
                let cell = match self {
                    Self::Atomic(scalar) => AtomicCellType::from_scalar(*scalar)?,
                    Self::AtomicBoundedInteger(bounds) => AtomicCellType::BoundedInteger(*bounds),
                    _ => return None,
                };
                cell.is_supported().then_some(cell)
            }

            pub fn as_structural(&self) -> Option<StructuralTypeId> {
                let StructuralFieldType::Structural(structural) = self else {
                    return None;
                };
                Some(*structural)
            }

            pub fn as_scalar(&self) -> Option<ScalarType> {
                let StructuralFieldType::Scalar(scalar) = self else {
                    return None;
                };
                Some(*scalar)
            }
        }

        impl StructuralFieldType {
            pub fn atomic_scalar_type(&self) -> Option<ScalarType> {
                self.atomic_cell_type().map(|cell| cell.scalar_type())
            }

            pub const fn byte_sequence_carrier(&self) -> Option<ByteSequenceCarrier> {
                match self {
                    Self::ByteSequence(carrier) | Self::PredicateByteSequence { carrier, .. } => {
                        Some(*carrier)
                    }
                    _ => None,
                }
            }

            pub const fn byte_predicates(&self) -> crate::ByteSequencePredicateSet {
                match self {
                    Self::PredicateByteSequence { predicates, .. } => *predicates,
                    _ => crate::ByteSequencePredicateSet::EMPTY,
                }
            }

            pub fn scalar_type(&self) -> Option<ScalarType> {
                match self {
                    Self::Scalar(scalar_type) | Self::Atomic(scalar_type) => Some(*scalar_type),
                    Self::BoundedInteger(integer) | Self::AtomicBoundedInteger(integer) => {
                        Some(ScalarType::Integer(integer.integer_type()))
                    }
                    Self::RefinedScalar(refinement) => Some(refinement.carrier()),
                    Self::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
                    Self::ByteSequence(_)
                    | Self::PredicateByteSequence { .. }
                    | Self::Structural(_)
                    | Self::Erased { .. } => None,
                }
            }

            pub fn canonical_leaf_shape(&self) -> Option<StructuralTypeShape> {
                match self {
                    Self::Scalar(scalar_type) => {
                        Some(StructuralTypeShape::PrimitiveScalar(*scalar_type))
                    }
                    Self::IeeeFloat(format) => Some(StructuralTypeShape::PrimitiveScalar(
                        ScalarType::IeeeFloat(*format),
                    )),
                    Self::ByteSequence(_)
                    | Self::PredicateByteSequence { .. }
                    | Self::BoundedInteger(_)
                    | Self::RefinedScalar(_)
                    | Self::Atomic(_)
                    | Self::AtomicBoundedInteger(_)
                    | Self::Structural(_)
                    | Self::Erased { .. } => None,
                }
            }

            pub fn canonical_leaf_type(&self) -> Option<Self> {
                match self {
                    Self::BoundedInteger(_) | Self::RefinedScalar(_) => Some(self.clone()),
                    _ => self.canonical_leaf_shape().and_then(Self::from_leaf_shape),
                }
            }

            pub fn shared_observation_type(&self) -> Option<Self> {
                self.shared_observation_shape()
                    .and_then(Self::from_leaf_shape)
            }

            pub fn borrowed_presentation_type(
                &self,
                access: crate::StructuralAccess,
                expected: Option<&StructuralTypeDeclaration>,
            ) -> Option<Self> {
                if access != crate::StructuralAccess::Owned
                    && let Some(cell) = self.atomic_cell_type()
                    && let Some(expected) = expected
                    && expected.shape == StructuralTypeShape::AtomicCell(cell)
                    && expected.scalar_restriction.is_none()
                {
                    return Some(Self::Structural(expected.id));
                }
                if access != crate::StructuralAccess::Owned
                    && let Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) =
                        self.byte_sequence_carrier()
                    && let Some(expected) = expected
                    && expected.shape == StructuralTypeShape::ByteSequence(carrier)
                {
                    return Some(Self::Structural(expected.id));
                }
                if let Some(expected) = expected
                    && expected.scalar_restriction.as_ref() == Some(self)
                {
                    return Some(Self::Structural(expected.id));
                }
                let presentation = if access == crate::StructuralAccess::SharedBorrow {
                    match self.byte_sequence_carrier() {
                        Some(ByteSequenceCarrier::BorrowedView {
                            access: Some(crate::StructuralAccess::SharedBorrow),
                        }) => Some(Self::ByteSequence(ByteSequenceCarrier::BorrowedView {
                            access: None,
                        })),
                        _ => self.shared_observation_type(),
                    }
                } else if access == crate::StructuralAccess::Owned {
                    None
                } else {
                    match self {
                        Self::BoundedInteger(_) | Self::RefinedScalar(_) => {
                            self.shared_observation_type()
                        }
                        _ => self.canonical_leaf_type(),
                    }
                }?;
                if let Some(expected) = expected
                    && presentation.declares_leaf(expected)
                {
                    Some(Self::Structural(expected.id))
                } else {
                    Some(presentation)
                }
            }

            pub fn leaf_copy_type(
                &self,
                expected: Option<&StructuralTypeDeclaration>,
            ) -> Option<Self> {
                let contract = self.leaf_copy_shape().and_then(Self::from_leaf_shape)?;
                if let Some(expected) = expected
                    && (contract.declares_leaf(expected)
                        || self
                            .borrowed_presentation_type(
                                crate::StructuralAccess::SharedBorrow,
                                Some(expected),
                            )
                            .is_some_and(|presentation| presentation.declares_leaf(expected)))
                {
                    Some(Self::Structural(expected.id))
                } else {
                    Some(contract)
                }
            }

            fn from_leaf_shape(shape: StructuralTypeShape) -> Option<Self> {
                match shape {
                    StructuralTypeShape::PrimitiveScalar(scalar) => Some(Self::Scalar(scalar)),
                    StructuralTypeShape::ByteSequence(carrier) => Some(Self::ByteSequence(carrier)),
                    _ => None,
                }
            }

            pub fn leaf_declaration<'a>(
                &self,
                declarations: impl Iterator<Item = &'a StructuralTypeDeclaration>,
            ) -> Option<&'a StructuralTypeDeclaration> {
                let mut matching =
                    declarations.filter(|declaration| self.declares_leaf(declaration));
                let declaration = matching.next()?;
                matching.next().is_none().then_some(declaration)
            }

            pub fn declares_leaf(&self, declaration: &StructuralTypeDeclaration) -> bool {
                if let Self::Structural(identity) = self {
                    return *identity == declaration.id;
                }
                let Some((shape, restriction)) = (match self {
                    Self::BoundedInteger(_) | Self::RefinedScalar(_) => self
                        .scalar_type()
                        .map(|scalar| (StructuralTypeShape::PrimitiveScalar(scalar), Some(self))),
                    Self::ByteSequence(carrier) => {
                        Some((StructuralTypeShape::ByteSequence(*carrier), None))
                    }
                    _ => self.canonical_leaf_shape().map(|shape| (shape, None)),
                }) else {
                    return false;
                };
                declaration.shape == shape && declaration.scalar_restriction.as_ref() == restriction
            }

            pub fn shared_observation_shape(&self) -> Option<StructuralTypeShape> {
                match self {
                    Self::BoundedInteger(integer) => Some(StructuralTypeShape::PrimitiveScalar(
                        ScalarType::Integer(integer.integer_type()),
                    )),
                    Self::RefinedScalar(refinement) => {
                        Some(StructuralTypeShape::PrimitiveScalar(refinement.carrier()))
                    }
                    _ => self.canonical_leaf_shape(),
                }
            }

            pub fn leaf_copy_shape(&self) -> Option<StructuralTypeShape> {
                match self.byte_sequence_carrier() {
                    Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) => {
                        Some(StructuralTypeShape::ByteSequence(carrier))
                    }
                    Some(carrier @ ByteSequenceCarrier::BorrowedView { .. }) => {
                        Some(StructuralTypeShape::ByteSequence(carrier))
                    }
                    _ => self.canonical_leaf_shape(),
                }
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum ByteSequenceCarrier {
            BorrowedView {
                access: Option<crate::terminal_module::ownership::StructuralAccess>,
            },
            BoundedView {
                access: Option<crate::terminal_module::ownership::StructuralAccess>,
                capacity: u64,
            },
            BoundedOwned {
                capacity: u64,
            },
        }

        pub fn structural_children(
            fields: &[StructuralFieldDeclaration],
        ) -> impl Iterator<Item = StructuralTypeId> + '_ {
            fields.iter().filter_map(|field| match field.field_type {
                StructuralFieldType::Structural(child) => Some(child),
                _ => None,
            })
        }

        pub fn live_field(
            fields: &[StructuralFieldDeclaration],
            identity: TerminalIdentityIndex,
        ) -> Option<&StructuralFieldDeclaration> {
            let mut matching = fields.iter().filter(|field| field.identity == identity);
            let field = matching.next()?;
            if matching.next().is_some()
                || field.relevance.is_erased()
                || fields
                    .iter()
                    .filter(|candidate| candidate.id == field.id)
                    .count()
                    != 1
            {
                return None;
            }
            Some(field)
        }

        pub fn find_structural_type(
            types: &[StructuralTypeDeclaration],
            id: StructuralTypeId,
        ) -> Option<&StructuralTypeDeclaration> {
            types.iter().find(|declaration| declaration.id == id)
        }

        pub fn structural_type_contains_reference(
            types: &[StructuralTypeDeclaration],
            root: StructuralTypeId,
        ) -> bool {
            structural_type_contains_reference_by(|id| find_structural_type(types, id), root)
        }

        pub fn structural_type_contains_reference_by<'types>(
            lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
            root: StructuralTypeId,
        ) -> bool {
            structural_type_contains_by(
                lookup,
                root,
                |shape| matches!(shape, StructuralTypeShape::Reference { .. }),
                |_| false,
            )
        }

        pub fn structural_type_has_borrowed_storage_by<'types>(
            lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
            root: StructuralTypeId,
        ) -> bool {
            structural_type_contains_by(
                lookup,
                root,
                StructuralTypeShape::is_borrowed_descriptor,
                |field| {
                    matches!(
                        field.byte_sequence_carrier(),
                        Some(
                            ByteSequenceCarrier::BorrowedView { .. }
                                | ByteSequenceCarrier::BoundedView { .. }
                        )
                    )
                },
            )
        }

        fn structural_type_contains_by<'types>(
            lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
            root: StructuralTypeId,
            contains_shape: impl Fn(&StructuralTypeShape) -> bool,
            contains_field: impl Fn(&StructuralFieldType) -> bool,
        ) -> bool {
            let mut pending = vec![root];
            let mut visited = std::collections::BTreeSet::new();
            while let Some(current) = pending.pop() {
                if !visited.insert(current) {
                    continue;
                }
                let Some(declaration) = lookup(current) else {
                    continue;
                };
                if contains_shape(&declaration.shape) {
                    return true;
                }
                let mut push_fields = |fields: &[StructuralFieldDeclaration]| {
                    pending.extend(structural_children(fields));
                    fields.iter().any(|field| {
                        !field.relevance.is_erased() && contains_field(&field.field_type)
                    })
                };
                match &declaration.shape {
                    StructuralTypeShape::Record { fields, .. } => {
                        if push_fields(fields) {
                            return true;
                        }
                    }
                    StructuralTypeShape::Sum { cases } => {
                        if cases.iter().any(|case| push_fields(&case.fields)) {
                            return true;
                        }
                    }
                    StructuralTypeShape::Mixed { fields, cases } => {
                        if push_fields(fields) || cases.iter().any(|case| push_fields(&case.fields))
                        {
                            return true;
                        }
                    }
                    StructuralTypeShape::FixedArray { element, length } => {
                        if *length != crate::ArrayExtent::Literal(0) {
                            pending.push(*element);
                        }
                    }
                    StructuralTypeShape::ElementView { element, .. } => pending.push(*element),
                    StructuralTypeShape::Opaque
                    | StructuralTypeShape::Reference { .. }
                    | StructuralTypeShape::PrimitiveScalar(_)
                    | StructuralTypeShape::AtomicCell(_)
                    | StructuralTypeShape::ByteSequence(_) => {}
                }
            }
            false
        }

        pub fn structural_case_by_identity(
            cases: &[StructuralCaseDeclaration],
            identity: TerminalIdentityIndex,
        ) -> Option<&StructuralCaseDeclaration> {
            cases.iter().find(|case| case.identity == identity)
        }

        pub fn structural_type_by_identity(
            types: &[StructuralTypeDeclaration],
            identity: TerminalIdentityIndex,
        ) -> Option<&StructuralTypeDeclaration> {
            types
                .iter()
                .find(|declaration| declaration.identity == identity)
        }

        pub fn live_field_by_id(
            fields: &[StructuralFieldDeclaration],
            id: StructuralFieldId,
        ) -> Option<&StructuralFieldDeclaration> {
            fields
                .iter()
                .find(|field| field.id == id && !field.relevance.is_erased())
        }

        pub fn live_field_entry(
            fields: &[StructuralFieldDeclaration],
            identity: TerminalIdentityIndex,
        ) -> Option<(usize, &StructuralFieldDeclaration)> {
            fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .enumerate()
                .find(|(_, field)| field.identity == identity)
        }

        pub fn find_structural_case(
            cases: &[StructuralCaseDeclaration],
            id: StructuralCaseId,
        ) -> Option<&StructuralCaseDeclaration> {
            cases.iter().find(|case| case.id == id)
        }

        pub fn structural_case_initializer_path(
            shape: &StructuralTypeShape,
            selected_case: StructuralCaseId,
            field_ordinal: usize,
            field: StructuralFieldId,
        ) -> Option<Vec<crate::StructuralPathSegment>> {
            let (common, cases) = match shape {
                StructuralTypeShape::Sum { cases } => (&[][..], cases),
                StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
                _ => return None,
            };
            let selected = find_structural_case(cases, selected_case)?;
            let (declaration, is_payload) = common
                .iter()
                .map(|declaration| (declaration, false))
                .chain(
                    selected
                        .fields
                        .iter()
                        .map(|declaration| (declaration, true)),
                )
                .filter(|(declaration, _)| !declaration.relevance.is_erased())
                .nth(field_ordinal)?;
            if declaration.id != field {
                return None;
            }
            let mut path = Vec::new();
            if is_payload {
                path.push(crate::StructuralPathSegment::Case(selected_case));
            }
            path.push(crate::StructuralPathSegment::Field(declaration.identity));
            Some(path)
        }

        pub fn structural_case_position(
            cases: &[StructuralCaseDeclaration],
            id: StructuralCaseId,
        ) -> Option<usize> {
            cases.iter().position(|case| case.id == id)
        }

        #[macro_export]
        macro_rules! record_or_mixed_shape_pattern {
            ($fields:ident) => {
                $crate::StructuralTypeShape::Record {
                    fields: $fields,
                    ..
                } | $crate::StructuralTypeShape::Mixed {
                    fields: $fields,
                    ..
                }
            };
        }

        #[macro_export]
        macro_rules! record_or_fixed_array_shape_pattern {
            () => {
                $crate::StructuralTypeShape::Record { .. }
                    | $crate::StructuralTypeShape::FixedArray { .. }
            };
        }

        #[macro_export]
        macro_rules! byte_sequence_view_carrier_pattern {
            () => {
                $crate::ByteSequenceCarrier::BorrowedView { .. }
                    | $crate::ByteSequenceCarrier::BoundedView { .. }
            };
        }

        #[macro_export]
        macro_rules! sum_or_mixed_shape_pattern {
            () => {
                $crate::StructuralTypeShape::Sum { .. } | $crate::StructuralTypeShape::Mixed { .. }
            };
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn empty_fixed_array_contains_no_reference() {
                let referent = StructuralTypeId::new(1).unwrap();
                let reference = StructuralTypeId::new(2).unwrap();
                let element = StructuralTypeId::new(3).unwrap();
                let array = StructuralTypeId::new(4).unwrap();
                let types = vec![
                    StructuralTypeDeclaration {
                        declaration_identity: Default::default(),
                        requires: Vec::new(),
                        quotient_carrier: None,
                        owns_linear_claim: false,
                        no_code_affine_cleanup: false,
                        multiplicity: crate::StructuralMultiplicity::Affine,
                        descriptor_cleanup: false,
                        nominal_cleanup: None,
                        scalar_restriction: None,
                        id: reference,
                        identity: crate::TerminalIdentityIndex(1),
                        shape: StructuralTypeShape::Reference {
                            referent,
                            access: crate::StructuralAccess::Owned,
                        },
                    },
                    StructuralTypeDeclaration {
                        declaration_identity: Default::default(),
                        requires: Vec::new(),
                        quotient_carrier: None,
                        owns_linear_claim: false,
                        no_code_affine_cleanup: false,
                        multiplicity: crate::StructuralMultiplicity::Affine,
                        descriptor_cleanup: false,
                        nominal_cleanup: None,
                        scalar_restriction: None,
                        id: referent,
                        identity: crate::TerminalIdentityIndex(2),
                        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                    },
                    StructuralTypeDeclaration {
                        declaration_identity: Default::default(),
                        requires: Vec::new(),
                        quotient_carrier: None,
                        owns_linear_claim: false,
                        no_code_affine_cleanup: false,
                        multiplicity: crate::StructuralMultiplicity::Affine,
                        descriptor_cleanup: false,
                        nominal_cleanup: None,
                        scalar_restriction: None,
                        id: element,
                        identity: crate::TerminalIdentityIndex(3),
                        shape: StructuralTypeShape::Record {
                            fields: vec![StructuralFieldDeclaration {
                                declaration_identity: Default::default(),
                                inline_carrier: None,
                                id: StructuralFieldId::new(1).unwrap(),
                                identity: crate::TerminalIdentityIndex(4),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Structural(reference),
                                storage: None,
                            }],
                            storage: crate::StructuralRecordStorage::Packed,
                        },
                    },
                    StructuralTypeDeclaration {
                        declaration_identity: Default::default(),
                        requires: Vec::new(),
                        quotient_carrier: None,
                        owns_linear_claim: false,
                        no_code_affine_cleanup: false,
                        multiplicity: crate::StructuralMultiplicity::Affine,
                        descriptor_cleanup: false,
                        nominal_cleanup: None,
                        scalar_restriction: None,
                        id: array,
                        identity: crate::TerminalIdentityIndex(5),
                        shape: StructuralTypeShape::FixedArray {
                            element,
                            length: crate::ArrayExtent::Literal(0),
                        },
                    },
                ];
                assert!(structural_type_contains_reference(&types, reference));
                assert!(!structural_type_contains_reference(&types, array));
            }
        }
    }

    pub use qualifications::{
        ResultQualificationEstablishment, StructuralContentProjection, StructuralDomainAccessPlan,
        StructuralDomainDeclaration, StructuralDomainPredicate, StructuralEstablishmentRoute,
        StructuralPathQualification, StructuralQualificationChange,
    };
    pub use scalar_qualifications::{
        ScalarDomainDeclaration, ScalarDomainEstablishmentRoute, ScalarDomainPredicate,
        ScalarFieldQualification, ScalarFloatRange, ScalarIntegerRange,
        ScalarPrimitiveQualification, ScalarQualificationCatalog, ScalarQualificationCoercion,
        ScalarQualificationSet, ScalarReturnOrigin, ScalarReturnQualification, ScalarReturnSite,
    };
    pub use structural::{
        ByteSequenceCarrier, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralReferentIdentity, StructuralTypeDeclaration,
        StructuralTypeShape, field_by_identity, find_structural_case, find_structural_type,
        live_field, live_field_by_id, live_field_by_identity, live_field_entry,
        structural_case_by_identity, structural_case_initializer_path, structural_case_position,
        structural_children, structural_type_by_identity, structural_type_contains_reference,
        structural_type_contains_reference_by, structural_type_has_borrowed_storage_by,
        unique_structural_declaration,
    };
}

pub mod values {

    pub(crate) mod places {

        use crate::terminal_module::TerminalIdentityIndex;
        use crate::{OperationKind, StructuralAccess};
        use semantic_vocabulary::{
            ObligationId, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId,
            StructuralPlaceKind, StructuralTypeId, ValueId,
        };

        /// A captured storage path keeps resolved field identities, selector
        /// values and changes of presentation together across analyses.
        /// Unlike a portable proof path, it can name machine-local selectors.
        /// Different paths may still address the same cell; equality alone
        /// does not establish disjoint storage.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum ResolvedStructuralPathSegment {
            /// ownership.md: "Record segments use the exact field identity".
            Field(StructuralFieldId),
            /// ownership.md: "A Case segment guards its payload namespace; it adds
            /// neither physical storage nor an independent claim."
            Case(StructuralCaseId),
            /// ownership.md: "Fixed-array segments use canonical zero-based literal
            /// indexes".
            FixedIndex(u64),
            /// An element selected at run time, named by the selector value captured
            /// when the place was formed. `wiki/spec/terminal-psi/loans.md`: "Loan
            /// formation captures a place, not a live selector expression." Because a
            /// value never changes, a later write to the selector's source names a
            /// different value and therefore a different place.
            RuntimeIndex { selector: ValueId },
            /// The descriptor's current extent, a scalar observation of its
            /// contents. This is metadata, not a physical field or another owner.
            /// byte_views.md: "a validated length observation of a machine or block
            /// parameter equals that exact parameter's `ViewExtent`".
            /// Replacing any ancestor descriptor replaces this observation's
            /// reaching content, just as a store replaces a scalar field's.
            Extent,
            /// Present the same storage through a checked shape. This step retains
            /// its physical scalar codec; it neither moves bytes nor adds access.
            RepresentationView {
                structural_type: StructuralTypeId,
                stored_scalar: Option<ScalarType>,
            },
            /// Cross a live permission descriptor into its separately owned backing.
            /// It is never an inline field, a claim transfer or a disjointness fact.
            Referent,
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
        pub enum StructuralPathSegment {
            AllElements,
            Case(StructuralCaseId),
            Field(TerminalIdentityIndex),
            FixedIndex(u64),
            FixedRange {
                start: u64,
                end: u64,
            },
            RuntimeIndex {
                index: ValueId,
                obligation: ObligationId,
            },
            RepresentationView {
                structural_type: StructuralTypeId,
                stored_scalar: Option<ScalarType>,
            },
            Referent,
        }

        impl Default for StructuralPathSegment {
            fn default() -> Self {
                Self::Field(TerminalIdentityIndex::ABSENT)
            }
        }

        macro_rules! call_and_scalar_constant_operations {
            () => {
                Self::Call { .. }
                    | Self::SelectImplementation { .. }
                    | Self::SelectMachine { .. }
                    | Self::PortWrite { .. }
                    | Self::RegisterAccess { .. }
                    | Self::IntegerConstant { .. }
                    | Self::BooleanConstant { .. }
                    | Self::IeeeFloatConstant { .. }
                    | Self::IeeeFloatCompare { .. }
                    | Self::ScalarRepresentationView { .. }
                    | Self::NearestIeeeFloatFusedMultiplyAdd { .. }
            };
        }

        macro_rules! scalar_predicate_operations {
            () => {
                Self::BooleanNot { .. }
                    | Self::BooleanEqual { .. }
                    | Self::IntegerEqual { .. }
                    | Self::IntegerLessThan { .. }
                    | Self::IntegerLessOrEqual { .. }
                    | Self::IntegerBitwiseNot { .. }
                    | Self::IntegerWiden { .. }
                    | Self::IntegerExactCast { .. }
                    | Self::IntegerBitwiseAnd { .. }
                    | Self::IntegerBitwiseOr { .. }
                    | Self::IntegerBitwiseXor { .. }
                    | Self::WrappingIntegerShiftLeft { .. }
                    | Self::WrappingIntegerShiftRight { .. }
                    | Self::ExactIntegerShiftLeft { .. }
                    | Self::ExactIntegerShiftRight { .. }
                    | Self::ExactIntegerAdd { .. }
                    | Self::ExactIntegerSubtract { .. }
                    | Self::ExactIntegerMultiply { .. }
                    | Self::ExactIntegerDivide { .. }
                    | Self::ExactIntegerRemainder { .. }
                    | Self::WrappingIntegerDivide { .. }
                    | Self::WrappingIntegerRemainder { .. }
                    | Self::SaturatingIntegerDivide { .. }
                    | Self::SaturatingIntegerRemainder { .. }
                    | Self::WrappingIntegerAdd { .. }
                    | Self::SaturatingIntegerAdd { .. }
                    | Self::WrappingIntegerSubtract { .. }
                    | Self::SaturatingIntegerSubtract { .. }
                    | Self::WrappingIntegerMultiply { .. }
                    | Self::SaturatingIntegerMultiply { .. }
                    | Self::TrappingInteger { .. }
                    | Self::FloatResultPolicy { .. }
            };
        }

        impl StructuralPathSegment {
            pub const fn runtime_index(&self) -> Option<(ValueId, ObligationId)> {
                match self {
                    Self::RuntimeIndex { index, obligation } => Some((*index, *obligation)),
                    Self::AllElements
                    | Self::Case(_)
                    | Self::Field(_)
                    | Self::FixedIndex(_)
                    | Self::FixedRange { .. }
                    | Self::RepresentationView { .. }
                    | Self::Referent => None,
                }
            }
        }

        pub fn is_static_structural_path(path: &[StructuralPathSegment]) -> bool {
            path.iter().all(|segment| {
                !matches!(segment, StructuralPathSegment::AllElements)
                    && segment.runtime_index().is_none()
            })
        }

        pub fn is_structural_scalar_store_path(path: &[StructuralPathSegment]) -> bool {
            path.iter().all(|segment| match segment {
                StructuralPathSegment::Field(identity) => !identity.is_absent(),
                StructuralPathSegment::Case(_) => true,
                StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. } => true,
                StructuralPathSegment::AllElements
                | StructuralPathSegment::FixedRange { .. }
                | StructuralPathSegment::RepresentationView { .. }
                | StructuralPathSegment::Referent => false,
            })
        }

        pub fn is_static_structural_scalar_store_path(path: &[StructuralPathSegment]) -> bool {
            is_static_structural_path(path) && is_structural_scalar_store_path(path)
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct StructuralPlaceDeclaration {
            pub id: PlaceId,
            pub kind: StructuralPlaceKind,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct OperationProjection<'a> {
            pub root: PlaceId,
            pub path: &'a [StructuralPathSegment],
            /// The final canonical field, when the instruction keeps it outside
            /// its authored path. Operand consumers must not lose this selector.
            pub field: Option<StructuralFieldId>,
        }

        #[derive(Debug, Clone, Copy)]
        pub enum StructuralStorageUse<'a> {
            Place(PlaceId),
            Projection(OperationProjection<'a>),
            Canonical {
                root: PlaceId,
                path: &'a [semantic_vocabulary::CanonicalStructuralPathSegment],
            },
            ObservedField {
                root: PlaceId,
                path: &'a [semantic_vocabulary::CanonicalStructuralPathSegment],
                field: StructuralFieldId,
            },
            Element {
                descriptor: OperationProjection<'a>,
                path: &'a [StructuralPathSegment],
            },
            Type(StructuralTypeId),
        }

        impl<'a> From<OperationProjection<'a>> for StructuralStorageUse<'a> {
            fn from(projection: OperationProjection<'a>) -> Self {
                Self::Projection(projection)
            }
        }

        impl OperationKind {
            pub fn map_structural_uses(&mut self, map: &mut impl FnMut(PlaceId) -> PlaceId) {
                self.map_structural_uses_with_access(&mut |place, _| map(place));
            }

            pub fn map_structural_uses_with_access(
                &mut self,
                map: &mut impl FnMut(PlaceId, StructuralAccess) -> PlaceId,
            ) {
                match self {
                    Self::ActivateTaskRegister {
                        gdt,
                        task_state,
                        scratch,
                        ..
                    } => {
                        for source in [gdt, task_state, scratch] {
                            source.place = map(source.place, source.access);
                        }
                    }
                    Self::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                        for source in [gdt, scratch] {
                            source.place = map(source.place, source.access);
                        }
                    }
                    Self::ReadTaskRegister { .. } | Self::ReadSegmentSelector { .. } => {}
                    Self::EstablishNativeCodeLayout { .. }
                    | Self::EstablishActivationClaim { .. } => {}
                    Self::TransformContent {
                        source,
                        parents,
                        derivations,
                    } => {
                        let mut bindings = Vec::new();
                        for argument in std::iter::once(source).chain(parents) {
                            let original = argument.place;
                            argument.place = map(original, argument.access);
                            bindings.push((original, argument.place));
                        }
                        for derived in derivations.iter_mut() {
                            derived.map_places(&mut |place| {
                                bindings
                                    .iter()
                                    .find(|(source, _)| *source == place)
                                    .map_or(place, |(_, target)| *target)
                            });
                        }
                        derivations.sort();
                    }
                    Self::ReleaseActivationClaim { source, .. }
                    | Self::SplitActivationClaim { source, .. }
                    | Self::StorageAddress { source }
                    | Self::GlobalDescriptorTable { source, .. }
                    | Self::MoveStructuralValue { source }
                    | Self::DecomposeStructuralValue { subject: source }
                    | Self::EstablishAccessReference { source, .. }
                    | Self::EstablishReference { source, .. }
                    | Self::EstablishElementView { source, .. }
                    | Self::EstablishMutableElementView { source, .. } => {
                        source.place = map(source.place, source.access);
                    }
                    Self::MergeActivationClaim { lower, upper, .. } => {
                        lower.place = map(lower.place, lower.access);
                        upper.place = map(upper.place, upper.access);
                    }
                    Self::ElementViewLength { source }
                    | Self::ElementViewRead { source, .. }
                    | Self::ByteSequenceLength { source } => {
                        source.place = map(source.place, StructuralAccess::SharedBorrow);
                    }
                    Self::ReleaseReference { source, .. } => {
                        *source = map(*source, StructuralAccess::Owned);
                    }
                    Self::MoveStructuralField { source, .. } => {
                        *source = map(*source, StructuralAccess::MutableBorrow);
                    }
                    Self::ByteSequenceSubslice { source, .. }
                    | Self::ElementViewSubslice { source, .. } => {
                        source.place = map(source.place, source.access);
                    }
                    Self::PrimitiveScalarRead { source, .. }
                    | Self::StructuralByteSequenceFieldLength { source, .. }
                    | Self::StructuralByteSequenceFieldRead { source, .. }
                    | Self::StructuralCaseMembership { source, .. }
                    | Self::StructuralLeafCopy { source, .. }
                    | Self::StructuralCaseLeafCopy { source, .. }
                    | Self::ByteSequenceRead { source, .. }
                    | Self::BooleanStructuralField { source, .. }
                    | Self::ScalarStructuralField { source, .. } => {
                        *source = map(*source, StructuralAccess::SharedBorrow)
                    }
                    Self::StructuralByteSequenceFieldByteStore { destination, .. }
                    | Self::WriteOnlyPrimitiveStore { destination, .. }
                    | Self::StructuralScalarFieldStore { destination, .. }
                    | Self::ByteSequenceWrite { destination, .. } => {
                        *destination = map(*destination, StructuralAccess::WriteOnlyBorrow);
                    }
                    Self::ElementViewWrite { destination, .. } => {
                        destination.place = map(destination.place, destination.access);
                    }
                    Self::ByteSequenceStore {
                        destination,
                        source,
                        ..
                    }
                    | Self::StructuralByteSequenceFieldStore {
                        destination,
                        source,
                        ..
                    }
                    | Self::StructuralByteViewFieldStore {
                        destination,
                        source,
                        ..
                    } => {
                        *destination = map(*destination, StructuralAccess::WriteOnlyBorrow);
                        *source = map(*source, StructuralAccess::SharedBorrow);
                    }
                    Self::StoreStructuralField {
                        destination, value, ..
                    } => {
                        *destination = map(*destination, StructuralAccess::WriteOnlyBorrow);
                        value.place = map(value.place, value.access);
                    }
                    Self::EstablishByteSequenceConcatenation { left, right, .. }
                    | Self::ByteSequenceEqual { left, right } => {
                        for operand in [left, right] {
                            map_byte_sequence_operand(operand, map);
                        }
                    }
                    Self::ByteSequencePredicate { operand, .. } => {
                        map_byte_sequence_operand(operand, map);
                    }
                    Self::EstablishStructuralArray { elements } => {
                        for argument in elements {
                            argument.place = map(argument.place, argument.access);
                        }
                    }
                    Self::EstablishRecord { fields }
                    | Self::EstablishStructuralCase { fields, .. } => {
                        for field in fields {
                            match &mut field.value {
                                crate::RecordFieldValue::Structural(value) => {
                                    value.place = map(value.place, value.access);
                                }
                                crate::RecordFieldValue::Scalar { .. } => {}
                            }
                        }
                    }
                    crate::call_structural_argument_variants!(structural_arguments) => {
                        for argument in structural_arguments {
                            argument.place = map(argument.place, argument.access);
                        }
                    }
                    Self::AtomicAccess { place, event, .. } => {
                        let access = match (event.observes_resident(), event.modifies_resident()) {
                            (true, true) => StructuralAccess::MutableBorrow,
                            (false, true) => StructuralAccess::WriteOnlyBorrow,
                            (_, false) => StructuralAccess::SharedBorrow,
                        };
                        *place = map(*place, access);
                    }
                    Self::EstablishScalarArray { .. }
                    | Self::EstablishZeroedStructural { .. }
                    | Self::EstablishStructuralLocal { .. }
                    | Self::EstablishPrimitiveLocal { .. }
                    | Self::EstablishScalarCase { .. }
                    | Self::EstablishByteSequenceLiteral { .. }
                    | Self::EstablishTrivialAffineLocal { .. }
                    | Self::AtomicFence { .. }
                    | call_and_scalar_constant_operations!()
                    | Self::ReadScalarApplication { .. }
                    | scalar_predicate_operations!() => {}
                }
            }

            pub fn structural_projections(&self) -> Vec<OperationProjection<'_>> {
                let mut projections = Vec::new();
                for (storage, _) in self.storage_uses() {
                    match storage {
                        StructuralStorageUse::Projection(projection) => {
                            projections.push(projection)
                        }
                        StructuralStorageUse::Element { descriptor, .. } => {
                            projections.push(descriptor);
                        }
                        StructuralStorageUse::Place(_)
                        | StructuralStorageUse::Canonical { .. }
                        | StructuralStorageUse::ObservedField { .. }
                        | StructuralStorageUse::Type(_) => {}
                    }
                }
                projections
            }

            /// Keep content effects with the same operands used for ownership
            /// and layout checks. Transporting a descriptor does not read its
            /// pointee; calls obtain delegated effects from their contracts.
            pub fn storage_uses(
                &self,
            ) -> Vec<(
                StructuralStorageUse<'_>,
                Option<access_plans::AccessOperation>,
            )> {
                use access_plans::AccessOperation::{CompoundMutation, Read, Take, Write};
                let mut uses = Vec::new();
                let mut push = |storage, operation| uses.push((storage, operation));
                match self {
                    Self::ActivateTaskRegister {
                        gdt,
                        task_state,
                        scratch,
                        ..
                    } => {
                        for source in [gdt, task_state, scratch] {
                            push(argument_projection(source).into(), Some(CompoundMutation));
                        }
                    }
                    Self::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                        for source in [gdt, scratch] {
                            push(argument_projection(source).into(), Some(CompoundMutation));
                        }
                    }
                    Self::ReadTaskRegister { .. }
                    | Self::ReadSegmentSelector { .. }
                    | Self::EstablishNativeCodeLayout { .. } => {}
                    Self::EstablishActivationClaim { backing_type, .. } => {
                        push(StructuralStorageUse::Type(*backing_type), None);
                    }
                    Self::EstablishByteSequenceConcatenation { left, right, .. }
                    | Self::ByteSequenceEqual { left, right } => {
                        for operand in [left, right] {
                            push(operand.storage_projection().into(), Some(Read));
                        }
                    }
                    Self::ByteSequencePredicate { operand, .. } => {
                        push(operand.storage_projection().into(), Some(Read));
                    }
                    Self::ReleaseActivationClaim { source, .. }
                    | Self::SplitActivationClaim { source, .. } => {
                        push(argument_projection(source).into(), Some(CompoundMutation));
                    }
                    Self::StorageAddress { source }
                    | Self::EstablishAccessReference { source, .. }
                    | Self::EstablishReference { source, .. } => {
                        push(argument_projection(source).into(), None);
                    }
                    Self::MoveStructuralValue { source }
                    | Self::DecomposeStructuralValue { subject: source } => {
                        push(
                            argument_projection(source).into(),
                            projected_transfer_operation(source),
                        );
                    }
                    Self::GlobalDescriptorTable {
                        operation, source, ..
                    } => {
                        push(
                            argument_projection(source).into(),
                            Some(if operation.writes_operand() {
                                Write
                            } else {
                                Read
                            }),
                        );
                    }
                    Self::ElementViewLength { source }
                    | Self::ByteSequenceLength { source }
                    | Self::ByteSequenceSubslice { source, .. }
                    | Self::ElementViewSubslice { source, .. } => {
                        push(argument_projection(source).into(), Some(Read));
                    }
                    Self::EstablishElementView {
                        source, element, ..
                    }
                    | Self::EstablishMutableElementView {
                        source, element, ..
                    } => {
                        push(argument_projection(source).into(), None);
                        push(StructuralStorageUse::Type(*element), None);
                    }
                    Self::MergeActivationClaim { lower, upper, .. } => {
                        for source in [lower, upper] {
                            push(argument_projection(source).into(), Some(CompoundMutation));
                        }
                    }
                    Self::TransformContent {
                        source, parents, ..
                    } => {
                        push(argument_projection(source).into(), Some(CompoundMutation));
                        for parent in parents {
                            push(argument_projection(parent).into(), Some(Read));
                        }
                    }
                    Self::ElementViewWrite {
                        destination, path, ..
                    } => {
                        push(
                            StructuralStorageUse::Element {
                                descriptor: argument_projection(destination),
                                path,
                            },
                            Some(Write),
                        );
                    }
                    Self::ElementViewRead { source, path, .. } => {
                        push(
                            StructuralStorageUse::Element {
                                descriptor: argument_projection(source),
                                path,
                            },
                            Some(Read),
                        );
                    }
                    Self::PrimitiveScalarRead { source, path }
                    | Self::StructuralCaseMembership { source, path, .. }
                    | Self::StructuralLeafCopy { source, path } => {
                        push(
                            OperationProjection {
                                root: *source,
                                path,
                                field: None,
                            }
                            .into(),
                            Some(Read),
                        );
                    }
                    Self::StructuralByteSequenceFieldLength {
                        source,
                        path,
                        field,
                    }
                    | Self::StructuralByteSequenceFieldRead {
                        source,
                        path,
                        field,
                        ..
                    } => {
                        push(
                            OperationProjection {
                                root: *source,
                                path,
                                field: Some(*field),
                            }
                            .into(),
                            Some(Read),
                        );
                    }
                    Self::ReleaseReference { source, path } => {
                        push(
                            OperationProjection {
                                root: *source,
                                path,
                                field: None,
                            }
                            .into(),
                            None,
                        );
                    }
                    Self::AtomicAccess {
                        place, path, event, ..
                    } => {
                        let operation =
                            access_plans::AccessOperation::Atomic(event.access_operation());
                        push(
                            OperationProjection {
                                root: *place,
                                path,
                                field: None,
                            }
                            .into(),
                            Some(operation),
                        );
                    }
                    Self::MoveStructuralField {
                        source,
                        path,
                        field,
                    } => {
                        push(
                            OperationProjection {
                                root: *source,
                                path,
                                field: *field,
                            }
                            .into(),
                            (field.is_some() || !path.is_empty()).then_some(Take),
                        );
                    }
                    Self::WriteOnlyPrimitiveStore {
                        destination, path, ..
                    } => {
                        push(
                            OperationProjection {
                                root: *destination,
                                path,
                                field: None,
                            }
                            .into(),
                            Some(Write),
                        );
                    }
                    Self::StructuralByteSequenceFieldByteStore {
                        destination,
                        path,
                        field,
                        ..
                    }
                    | Self::StructuralScalarFieldStore {
                        destination,
                        path,
                        field,
                        ..
                    } => {
                        push(
                            OperationProjection {
                                root: *destination,
                                path,
                                field: Some(*field),
                            }
                            .into(),
                            Some(Write),
                        );
                    }
                    Self::StructuralByteSequenceFieldStore {
                        destination,
                        path,
                        field,
                        source,
                        ..
                    } => {
                        push(
                            OperationProjection {
                                root: *destination,
                                path,
                                field: Some(*field),
                            }
                            .into(),
                            Some(Write),
                        );
                        push(StructuralStorageUse::Place(*source), Some(Read));
                    }
                    Self::StructuralByteViewFieldStore {
                        destination,
                        path,
                        field,
                        source,
                        ..
                    } => {
                        push(
                            OperationProjection {
                                root: *destination,
                                path,
                                field: *field,
                            }
                            .into(),
                            Some(Write),
                        );
                        push(StructuralStorageUse::Place(*source), None);
                    }
                    Self::StoreStructuralField {
                        destination,
                        path,
                        field,
                        value,
                        ..
                    } => {
                        push(
                            OperationProjection {
                                root: *destination,
                                path,
                                field: *field,
                            }
                            .into(),
                            Some(Write),
                        );
                        push(
                            argument_projection(value).into(),
                            projected_transfer_operation(value),
                        );
                    }
                    Self::EstablishStructuralArray { elements } => {
                        for element in elements {
                            push(
                                argument_projection(element).into(),
                                projected_transfer_operation(element),
                            );
                        }
                    }
                    Self::EstablishRecord { fields }
                    | Self::EstablishStructuralCase { fields, .. } => {
                        for field in fields {
                            if let crate::RecordFieldValue::Structural(value) = &field.value {
                                push(
                                    argument_projection(value).into(),
                                    projected_transfer_operation(value),
                                );
                            }
                        }
                    }
                    crate::call_structural_argument_variants!(structural_arguments) => {
                        for argument in structural_arguments {
                            push(
                                argument_projection(argument).into(),
                                projected_transfer_operation(argument),
                            );
                        }
                    }
                    Self::StructuralCaseLeafCopy { source, path } => {
                        push(
                            StructuralStorageUse::Canonical {
                                root: *source,
                                path,
                            },
                            Some(Read),
                        );
                    }
                    Self::BooleanStructuralField {
                        source,
                        path,
                        field,
                    }
                    | Self::ScalarStructuralField {
                        source,
                        path,
                        field,
                    } => {
                        push(
                            StructuralStorageUse::ObservedField {
                                root: *source,
                                path,
                                field: *field,
                            },
                            Some(Read),
                        );
                    }
                    Self::ByteSequenceRead { source, .. } => {
                        push(StructuralStorageUse::Place(*source), Some(Read))
                    }
                    Self::ByteSequenceWrite { destination, .. } => {
                        push(StructuralStorageUse::Place(*destination), Some(Write))
                    }
                    Self::ByteSequenceStore {
                        destination,
                        source,
                        ..
                    } => {
                        push(StructuralStorageUse::Place(*destination), Some(Write));
                        push(StructuralStorageUse::Place(*source), Some(Read));
                    }
                    Self::EstablishScalarArray { .. }
                    | Self::EstablishZeroedStructural { .. }
                    | Self::EstablishStructuralLocal { .. }
                    | Self::EstablishPrimitiveLocal { .. }
                    | Self::EstablishScalarCase { .. }
                    | Self::EstablishByteSequenceLiteral { .. }
                    | Self::EstablishTrivialAffineLocal { .. }
                    | Self::AtomicFence { .. }
                    | call_and_scalar_constant_operations!()
                    | Self::ReadScalarApplication { .. }
                    | scalar_predicate_operations!() => {}
                }
                uses
            }

            pub fn structural_projection_paths_mut(
                &mut self,
            ) -> Vec<&mut Vec<StructuralPathSegment>> {
                let mut paths = Vec::new();
                match self {
                    Self::ActivateTaskRegister {
                        gdt,
                        task_state,
                        scratch,
                        ..
                    } => {
                        for source in [gdt, task_state, scratch] {
                            paths.push(&mut source.path);
                        }
                    }
                    Self::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                        for source in [gdt, scratch] {
                            paths.push(&mut source.path);
                        }
                    }
                    Self::ReadTaskRegister { .. } | Self::ReadSegmentSelector { .. } => {}
                    Self::EstablishByteSequenceConcatenation { left, right, .. }
                    | Self::ByteSequenceEqual { left, right } => {
                        for operand in [left, right] {
                            paths.push(&mut operand.argument_mut().path);
                        }
                    }
                    Self::ByteSequencePredicate { operand, .. } => {
                        paths.push(&mut operand.argument_mut().path);
                    }
                    Self::ReleaseActivationClaim { source, .. }
                    | Self::SplitActivationClaim { source, .. }
                    | Self::StorageAddress { source }
                    | Self::GlobalDescriptorTable { source, .. }
                    | Self::MoveStructuralValue { source }
                    | Self::DecomposeStructuralValue { subject: source }
                    | Self::EstablishAccessReference { source, .. }
                    | Self::EstablishReference { source, .. }
                    | Self::EstablishElementView { source, .. }
                    | Self::EstablishMutableElementView { source, .. }
                    | Self::ElementViewLength { source }
                    | Self::ElementViewRead { source, .. }
                    | Self::ByteSequenceLength { source }
                    | Self::ByteSequenceSubslice { source, .. }
                    | Self::ElementViewSubslice { source, .. } => {
                        paths.push(&mut source.path);
                    }
                    Self::MergeActivationClaim { lower, upper, .. } => {
                        paths.push(&mut lower.path);
                        paths.push(&mut upper.path);
                    }
                    Self::TransformContent {
                        source, parents, ..
                    } => {
                        paths.push(&mut source.path);
                        paths.extend(parents.iter_mut().map(|parent| &mut parent.path));
                    }
                    Self::ElementViewWrite { destination, .. } => {
                        paths.push(&mut destination.path);
                    }
                    Self::ReleaseReference { path, .. }
                    | Self::PrimitiveScalarRead { path, .. }
                    | Self::AtomicAccess { path, .. }
                    | Self::WriteOnlyPrimitiveStore { path, .. }
                    | Self::StructuralByteSequenceFieldLength { path, .. }
                    | Self::StructuralByteSequenceFieldRead { path, .. }
                    | Self::StructuralCaseMembership { path, .. }
                    | Self::StructuralLeafCopy { path, .. }
                    | Self::MoveStructuralField { path, .. }
                    | Self::StructuralByteSequenceFieldByteStore { path, .. }
                    | Self::StructuralByteSequenceFieldStore { path, .. }
                    | Self::StructuralByteViewFieldStore { path, .. }
                    | Self::StructuralScalarFieldStore { path, .. } => paths.push(path),
                    Self::StoreStructuralField { path, value, .. } => {
                        paths.push(path);
                        paths.push(&mut value.path);
                    }
                    Self::EstablishStructuralArray { elements } => {
                        paths.extend(elements.iter_mut().map(|argument| &mut argument.path));
                    }
                    Self::EstablishRecord { fields }
                    | Self::EstablishStructuralCase { fields, .. } => {
                        paths.extend(fields.iter_mut().filter_map(
                            |field| match &mut field.value {
                                crate::RecordFieldValue::Structural(value) => Some(&mut value.path),
                                crate::RecordFieldValue::Scalar { .. } => None,
                            },
                        ));
                    }
                    crate::call_structural_argument_variants!(structural_arguments) => paths
                        .extend(
                            structural_arguments
                                .iter_mut()
                                .map(|argument| &mut argument.path),
                        ),
                    _ => {}
                }
                paths
            }

            pub fn runtime_indexes(&self) -> Vec<(ValueId, ObligationId)> {
                self.structural_projections()
                    .iter()
                    .flat_map(|projection| projection.path)
                    .filter_map(StructuralPathSegment::runtime_index)
                    .collect()
            }
        }

        fn projected_transfer_operation(
            argument: &crate::StructuralArgument,
        ) -> Option<access_plans::AccessOperation> {
            (argument.access == StructuralAccess::Owned && !argument.path.is_empty())
                .then_some(access_plans::AccessOperation::Take)
        }

        fn argument_projection(argument: &crate::StructuralArgument) -> OperationProjection<'_> {
            OperationProjection {
                field: None,
                root: argument.place,
                path: &argument.path,
            }
        }

        fn map_byte_sequence_operand(
            operand: &mut crate::control_flow::ByteSequenceOperand,
            map: &mut impl FnMut(PlaceId, StructuralAccess) -> PlaceId,
        ) {
            let source = operand.argument_mut();
            source.place = map(source.place, source.access);
        }

        #[cfg(test)]
        mod tests {
            use super::{
                StructuralPathSegment, is_static_structural_scalar_store_path,
                is_structural_scalar_store_path,
            };
            use crate::{
                OperationKind, RecordFieldInitializer, RecordFieldValue, StructuralAccess,
                StructuralArgument,
            };
            use semantic_vocabulary::{
                MachineId, ObligationId, PlaceId, ScalarTerm, StructuralFieldId, StructuralTypeId,
                ValueId,
            };

            fn place(identity: u64) -> PlaceId {
                PlaceId::new(identity).unwrap()
            }

            fn argument(identity: u64) -> StructuralArgument {
                StructuralArgument {
                    place: place(identity),
                    path: vec![runtime()],
                    access: StructuralAccess::SharedBorrow,
                }
            }

            #[test]
            fn storage_uses_keep_field_effects_separate_from_descriptor_transport() {
                use crate::{AtomicAccessEvent, MemoryOrdering, StructuralStorageUse};
                use access_plans::{
                    AccessOperation::{Atomic, Read, Take, Write},
                    AtomicAccessOperation,
                };
                let field = StructuralFieldId::new(7).unwrap();
                let read = OperationKind::StructuralByteSequenceFieldLength {
                    source: place(1),
                    path: Vec::new(),
                    field,
                };
                let uses = read.storage_uses();
                let StructuralStorageUse::Projection(projection) = uses[0].0 else {
                    panic!("the same operand roster retains the field projection")
                };
                assert_eq!(projection.root, place(1));
                assert_eq!(projection.field, Some(field));
                assert_eq!(uses[0].1, Some(Read));
                let scalar = OperationKind::ScalarStructuralField {
                    source: place(1),
                    path: Vec::new(),
                    field,
                };
                let uses = scalar.storage_uses();
                assert!(
                    matches!(uses[0].0, StructuralStorageUse::ObservedField { field: actual, .. } if actual == field)
                );
                assert_eq!(uses[0].1, Some(Read));
                let source = StructuralArgument {
                    place: place(1),
                    path: vec![StructuralPathSegment::Referent],
                    access: StructuralAccess::SharedBorrow,
                };
                let borrow = OperationKind::EstablishReference {
                    source: source.clone(),
                    storage: None,
                };
                assert_eq!(
                    borrow.storage_uses()[0].1,
                    None,
                    "forming a reference does not inspect its pointee"
                );
                let read = OperationKind::PrimitiveScalarRead {
                    source: place(1),
                    path: source.path,
                };
                assert_eq!(read.storage_uses()[0].1, Some(Read));
                for (event, expected) in [
                    (
                        AtomicAccessEvent::Load {
                            ordering: MemoryOrdering::Receive,
                        },
                        AtomicAccessOperation::Load(MemoryOrdering::Receive),
                    ),
                    (
                        AtomicAccessEvent::Store {
                            value: ValueId::new(1).unwrap(),
                            ordering: MemoryOrdering::Publish,
                        },
                        AtomicAccessOperation::Store(MemoryOrdering::Publish),
                    ),
                    (
                        AtomicAccessEvent::Swap {
                            value: ValueId::new(1).unwrap(),
                            ordering: MemoryOrdering::GlobalOrder,
                        },
                        AtomicAccessOperation::Swap(MemoryOrdering::GlobalOrder),
                    ),
                ] {
                    let access = OperationKind::AtomicAccess {
                        place: place(1),
                        path: Vec::new(),
                        event,
                        resident_obligation: None,
                    };
                    assert_eq!(access.storage_uses()[0].1, Some(Atomic(expected)));
                }
                use crate::control_flow::ByteSequenceOperand;
                let bytes = ByteSequenceOperand::Field {
                    source: StructuralArgument {
                        place: place(1),
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    },
                    field,
                    length: ValueId::new(1).unwrap(),
                };
                let equal = OperationKind::ByteSequenceEqual {
                    left: bytes.clone(),
                    right: bytes,
                };
                for (operand, operation) in equal.storage_uses() {
                    let StructuralStorageUse::Projection(projection) = operand else {
                        panic!("both byte fields retain projections")
                    };
                    assert_eq!(projection.field, Some(field));
                    assert_eq!(operation, Some(Read));
                }
                for (action, expected) in [
                    (crate::GlobalDescriptorTableOperation::Store, Write),
                    (crate::GlobalDescriptorTableOperation::Load, Read),
                ] {
                    let operation = OperationKind::GlobalDescriptorTable {
                        service: semantic_vocabulary::ServiceId::new(1).unwrap(),
                        operation: action,
                        source: StructuralArgument {
                            place: place(1),
                            path: Vec::new(),
                            access: StructuralAccess::MutableBorrow,
                        },
                    };
                    assert_eq!(operation.storage_uses()[0].1, Some(expected));
                }
                let whole = OperationKind::MoveStructuralField {
                    source: place(1),
                    path: Vec::new(),
                    field: None,
                };
                assert_eq!(
                    whole.storage_uses()[0].1,
                    None,
                    "moving a whole value retains its restrictions"
                );
                let field = OperationKind::MoveStructuralField {
                    source: place(1),
                    path: Vec::new(),
                    field: Some(field),
                };
                assert_eq!(field.storage_uses()[0].1, Some(Take));
            }

            #[test]
            fn owned_value_move_preserves_access_and_projected_source() {
                let path = vec![StructuralPathSegment::FixedIndex(3)];
                let mut operation = OperationKind::MoveStructuralValue {
                    source: StructuralArgument {
                        place: place(1),
                        path: path.clone(),
                        access: StructuralAccess::Owned,
                    },
                };
                let mut uses = Vec::new();
                operation.map_structural_uses_with_access(&mut |source, access| {
                    uses.push((source, access));
                    place(2)
                });
                assert_eq!(uses, vec![(place(1), StructuralAccess::Owned)]);
                let projections = operation.structural_projections();
                assert_eq!(projections.len(), 1);
                assert_eq!(projections[0].root, place(2));
                assert_eq!(projections[0].path, path.as_slice());
                assert_eq!(operation.structural_projection_paths_mut().len(), 1);
            }

            #[test]
            fn element_read_normalizes_only_the_descriptor_projection() {
                let mut operation = OperationKind::ElementViewRead {
                    source: argument(1),
                    index: ValueId::new(2).unwrap(),
                    length: ValueId::new(3).unwrap(),
                    obligation: ObligationId::new(2).unwrap(),
                    path: vec![StructuralPathSegment::FixedIndex(1)],
                };
                let projections = operation.structural_projections();
                assert_eq!(projections.len(), 1);
                assert_eq!(projections[0].root, place(1));
                assert_eq!(projections[0].path, [runtime()]);

                let mut paths = operation.structural_projection_paths_mut();
                assert_eq!(paths.len(), 1);
                paths[0][0] = StructuralPathSegment::FixedIndex(0);
                let OperationKind::ElementViewRead { source, path, .. } = operation else {
                    unreachable!("normalized element read");
                };
                assert_eq!(source.path, [StructuralPathSegment::FixedIndex(0)]);
                assert_eq!(path, [StructuralPathSegment::FixedIndex(1)]);
            }

            #[test]
            fn structural_use_mapping_preserves_scalar_and_proof_call_operands() {
                let scalar = ValueId::new(1).unwrap();
                let evidence =
                    ScalarTerm::boolean_field(place(1), StructuralFieldId::new(1).unwrap());
                let mut operation = OperationKind::CallStructuralScalar {
                    callee: MachineId::new(1).unwrap(),
                    arguments: vec![scalar],
                    erased_arguments: vec![evidence.clone()],
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: vec![argument(1), argument(2)],
                    claim_transfers: Vec::new(),
                    requirement_obligations: vec![ObligationId::new(1).unwrap()],
                    crash_continuations: Vec::new(),
                };
                let mut expected = operation.clone();
                if let OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } = &mut expected
                {
                    structural_arguments[0].place = place(11);
                    structural_arguments[1].place = place(12);
                }
                let mut visited = Vec::new();
                operation.map_structural_uses(&mut |source| {
                    visited.push(source);
                    place(source.get() + 10)
                });
                assert_eq!(visited, vec![place(1), place(2)]);
                assert_eq!(operation, expected);
            }

            #[test]
            fn structural_use_mapping_visits_nested_initializers_and_byte_operands() {
                use crate::control_flow::ByteSequenceOperand;

                let scalar = ValueId::new(1).unwrap();
                let mut record = OperationKind::EstablishRecord {
                    fields: vec![
                        RecordFieldInitializer {
                            field: StructuralFieldId::new(1).unwrap(),
                            value: RecordFieldValue::Scalar {
                                value: scalar,
                                range_obligation: None,
                            },
                        },
                        RecordFieldInitializer {
                            field: StructuralFieldId::new(2).unwrap(),
                            value: RecordFieldValue::Structural(argument(2)),
                        },
                    ],
                };
                let mut expected = record.clone();
                if let OperationKind::EstablishRecord { fields } = &mut expected
                    && let RecordFieldValue::Structural(source) = &mut fields[1].value
                {
                    source.place = place(12);
                }
                record.map_structural_uses(&mut |source| place(source.get() + 10));
                assert_eq!(record, expected);

                let mut concatenation = OperationKind::EstablishByteSequenceConcatenation {
                    left: ByteSequenceOperand::Whole {
                        source: argument(1),
                        length: scalar,
                    },
                    right: ByteSequenceOperand::Field {
                        source: argument(2),
                        field: StructuralFieldId::new(1).unwrap(),
                        length: scalar,
                    },
                    capacity_obligation: ObligationId::new(1).unwrap(),
                };
                let mut expected = concatenation.clone();
                if let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } =
                    &mut expected
                {
                    *left = ByteSequenceOperand::Whole {
                        source: argument(11),
                        length: scalar,
                    };
                    if let ByteSequenceOperand::Field { source, .. } = right {
                        source.place = place(12);
                    }
                }
                concatenation.map_structural_uses(&mut |source| place(source.get() + 10));
                assert_eq!(concatenation, expected);
            }

            #[test]
            fn structural_use_mapping_distinguishes_stores_from_establishments() {
                let mut store = OperationKind::StoreStructuralField {
                    disposition: crate::StructuralStoreDisposition::Vacant,
                    destination: place(1),
                    path: vec![runtime()],
                    field: StructuralFieldId::new(1),
                    value: argument(2),
                };
                let mut expected = store.clone();
                if let OperationKind::StoreStructuralField {
                    destination, value, ..
                } = &mut expected
                {
                    *destination = place(11);
                    value.place = place(12);
                }
                let mut visited = Vec::new();
                store.map_structural_uses(&mut |source| {
                    visited.push(source);
                    place(source.get() + 10)
                });
                assert_eq!(visited, vec![place(1), place(2)]);
                assert_eq!(store, expected);

                let mut view = OperationKind::EstablishMutableElementView {
                    region: crate::ElementViewRegion::WholeCollection,
                    destination: place(1),
                    source: argument(2),
                    element: StructuralTypeId::new(1).unwrap(),
                };
                let mut expected = view.clone();
                if let OperationKind::EstablishMutableElementView { source, .. } = &mut expected {
                    source.place = place(12);
                }
                view.map_structural_uses(&mut |source| place(source.get() + 10));
                assert_eq!(view, expected);
                for mut declaration in [
                    OperationKind::EstablishTrivialAffineLocal {
                        destination: place(1),
                    },
                    OperationKind::EstablishByteSequenceLiteral {
                        destination: place(1),
                        bytes: vec![7],
                        qualifications: Vec::new(),
                    },
                ] {
                    declaration.map_structural_uses(&mut |_| {
                        panic!("a destination declaration is not a use")
                    });
                }
            }

            fn field(identity: u32) -> StructuralPathSegment {
                StructuralPathSegment::Field(crate::TerminalIdentityIndex(identity))
            }

            fn runtime() -> StructuralPathSegment {
                StructuralPathSegment::RuntimeIndex {
                    index: ValueId::new(1).unwrap(),
                    obligation: ObligationId::new(1).unwrap(),
                }
            }

            #[test]
            fn field_store_carriers_compose_fields_and_elements_in_any_order() {
                for path in [
                    vec![],
                    vec![field(1), StructuralPathSegment::FixedIndex(2)],
                    vec![field(1), runtime(), field(2)],
                    vec![field(3), runtime(), StructuralPathSegment::FixedIndex(1)],
                    vec![StructuralPathSegment::FixedIndex(0), field(4)],
                ] {
                    assert!(is_structural_scalar_store_path(&path), "{path:?}");
                }
                for path in [
                    vec![StructuralPathSegment::Referent],
                    vec![field(1), StructuralPathSegment::Referent],
                    vec![StructuralPathSegment::FixedRange { start: 0, end: 1 }],
                    vec![StructuralPathSegment::Field(crate::TerminalIdentityIndex(
                        0,
                    ))],
                ] {
                    assert!(!is_structural_scalar_store_path(&path), "{path:?}");
                }
            }

            #[test]
            fn static_store_paths_compose_fields_and_literal_indices() {
                for path in [
                    vec![],
                    vec![field(1)],
                    vec![field(2), field(3)],
                    vec![StructuralPathSegment::FixedIndex(0)],
                    vec![field(4), StructuralPathSegment::FixedIndex(2)],
                    vec![
                        StructuralPathSegment::FixedIndex(0),
                        StructuralPathSegment::FixedIndex(1),
                    ],
                    vec![StructuralPathSegment::FixedIndex(0), field(5)],
                    vec![field(4), StructuralPathSegment::FixedIndex(0), field(5)],
                ] {
                    assert!(is_static_structural_scalar_store_path(&path), "{path:?}");
                }
                for path in [
                    vec![field(4), runtime()],
                    vec![StructuralPathSegment::Referent],
                    vec![field(1), StructuralPathSegment::Referent],
                    vec![StructuralPathSegment::Referent, field(1)],
                    vec![StructuralPathSegment::Field(crate::TerminalIdentityIndex(
                        0,
                    ))],
                ] {
                    assert!(!is_static_structural_scalar_store_path(&path), "{path:?}");
                }
            }
        }
    }
    pub(crate) mod reference_origins {

        use super::{StructuralArgument, StructuralPathSegment};
        use crate::StructuralAccess;
        use semantic_vocabulary::PlaceId;

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum ReferenceOriginProjection {
            Exact(StructuralPathSegment),
            Element,
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct ReferenceOriginSource {
            pub place: PlaceId,
            pub path: Vec<ReferenceOriginProjection>,
            pub extent: semantic_vocabulary::ReferenceOriginExtent,
            pub access: StructuralAccess,
        }

        impl From<StructuralArgument> for ReferenceOriginSource {
            fn from(argument: StructuralArgument) -> Self {
                Self {
                    place: argument.place,
                    extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                    path: argument
                        .path
                        .into_iter()
                        .map(ReferenceOriginProjection::Exact)
                        .collect(),
                    access: argument.access,
                }
            }
        }
    }
    pub(crate) mod signatures {
        use crate::terminal_module::TerminalIdentityIndex;
        use crate::{
            StructuralAccess, StructuralMultiplicity, StructuralPathQualification,
            StructuralPathSegment,
        };
        use semantic_vocabulary::{
            PlaceId, ScalarType, StructuralDomainId, StructuralTypeId, ValueId,
        };

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct ValueDeclaration {
            pub id: ValueId,
            pub scalar_type: ScalarType,
            pub qualifications: semantic_vocabulary::ScalarQualificationSetId,
        }

        impl ValueDeclaration {
            pub const fn value_type(self) -> semantic_vocabulary::QualifiedScalarType {
                semantic_vocabulary::QualifiedScalarType {
                    scalar_type: self.scalar_type,
                    qualifications: self.qualifications,
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ErasedProofFormal {
            pub source_position: u32,
            pub type_identity: TerminalIdentityIndex,
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum TerminalMachineResult {
            Unit,
            Scalar(ValueDeclaration),
            Structural(StructuralResultDeclaration),
        }

        impl TerminalMachineResult {
            pub const fn scalar(&self) -> Option<ValueDeclaration> {
                match self {
                    Self::Scalar(result) => Some(*result),
                    Self::Unit | Self::Structural(_) => None,
                }
            }

            pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
                match self {
                    Self::Scalar(result) => Some(result),
                    Self::Unit | Self::Structural(_) => None,
                }
            }

            optimization::structural_variant_accessor!(StructuralResultDeclaration; Self::Unit | Self::Scalar(_));
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct StructuralParameterDeclaration {
            pub place: PlaceId,
            pub position: u32,
            pub structural_type: StructuralTypeId,
            pub multiplicity: StructuralMultiplicity,
            pub access: StructuralAccess,
            pub qualifications: Vec<StructuralDomainId>,
            pub projected_qualifications: Vec<StructuralPathQualification>,
            pub additional_input_qualifications: Vec<StructuralPathQualification>,
        }

        impl StructuralParameterDeclaration {
            pub fn find_by_place(declarations: &[Self], place: PlaceId) -> Option<&Self> {
                declarations
                    .iter()
                    .find(|parameter| parameter.place == place)
            }

            pub fn is_unqualified_root(&self) -> bool {
                matches!(
                    self.multiplicity,
                    crate::nonlinear_multiplicity_pair_pattern!()
                ) && self.qualifications.is_empty()
                    && self.projected_qualifications.is_empty()
            }

            pub fn is_unqualified_owned_affine(&self) -> bool {
                self.access == StructuralAccess::Owned
                    && self.multiplicity == StructuralMultiplicity::Affine
                    && self.qualifications.is_empty()
                    && self.projected_qualifications.is_empty()
            }

            pub fn admits_scalar_read_at(&self, path_is_empty: bool) -> bool {
                (self.multiplicity == StructuralMultiplicity::Unrestricted
                    || (!path_is_empty && self.multiplicity == StructuralMultiplicity::Affine))
                    && self.qualifications.is_empty()
                    && self.projected_qualifications.is_empty()
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct StructuralResultDeclaration {
            pub place: PlaceId,
            pub structural_type: StructuralTypeId,
            pub multiplicity: StructuralMultiplicity,
            pub qualifications: Vec<StructuralDomainId>,
            pub projected_qualifications: Vec<StructuralPathQualification>,
            pub reference_sources: Vec<StructuralReferenceResultSource>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct StructuralReferenceResultSource {
            pub path: Vec<StructuralPathSegment>,
            pub sources: Vec<StructuralReferenceOrigin>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum StructuralReferenceOrigin {
            Parameter(super::ReferenceOriginSource),
            Invocation {
                carrier_type: semantic_vocabulary::StructuralTypeId,
                path: Vec<super::ReferenceOriginProjection>,
                extent: semantic_vocabulary::ReferenceOriginExtent,
                access: crate::StructuralAccess,
            },
        }

        impl StructuralReferenceOrigin {
            pub fn parameter(&self) -> Option<&super::ReferenceOriginSource> {
                match self {
                    Self::Parameter(source) => Some(source),
                    Self::Invocation { .. } => None,
                }
            }
            pub fn access(&self) -> crate::StructuralAccess {
                match self {
                    Self::Parameter(source) => source.access,
                    Self::Invocation { access, .. } => *access,
                }
            }
            pub fn path(&self) -> &[super::ReferenceOriginProjection] {
                match self {
                    Self::Parameter(source) => &source.path,
                    Self::Invocation { path, .. } => path,
                }
            }
            pub fn extent(&self) -> semantic_vocabulary::ReferenceOriginExtent {
                match self {
                    Self::Parameter(source) => source.extent,
                    Self::Invocation { extent, .. } => *extent,
                }
            }
        }

        impl From<super::ReferenceOriginSource> for StructuralReferenceOrigin {
            fn from(source: super::ReferenceOriginSource) -> Self {
                Self::Parameter(source)
            }
        }

        impl From<StructuralArgument> for StructuralReferenceOrigin {
            fn from(source: StructuralArgument) -> Self {
                Self::Parameter(source.into())
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct StructuralArgument {
            pub place: PlaceId,
            pub path: Vec<StructuralPathSegment>,
            pub access: StructuralAccess,
        }

        impl StructuralArgument {
            pub fn whole(place: PlaceId, access: StructuralAccess) -> Self {
                Self {
                    place,
                    path: Vec::new(),
                    access,
                }
            }
        }
    }

    pub use reference_origins::{ReferenceOriginProjection, ReferenceOriginSource};

    pub use places::{
        OperationProjection, ResolvedStructuralPathSegment, StructuralPathSegment,
        StructuralPlaceDeclaration, StructuralStorageUse, is_static_structural_path,
        is_static_structural_scalar_store_path, is_structural_scalar_store_path,
    };
    pub use signatures::{
        ErasedProofFormal, StructuralArgument, StructuralParameterDeclaration,
        StructuralReferenceOrigin, StructuralReferenceResultSource, StructuralResultDeclaration,
        TerminalMachineResult, ValueDeclaration,
    };
}
