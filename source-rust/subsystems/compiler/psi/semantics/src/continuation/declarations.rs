//! Compare declarations through their own identity tables. The numbers in two
//! independent publications are coordinates, even when they happen to agree.

use super::CheckedPrefix;
use super::predicates::Roots;
use semantic_vocabulary::{BoundaryMachineId, StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    NominalDeclarationIdentity, OperationResult, StructuralFieldDeclaration, StructuralFieldType,
    StructuralParameterDeclaration, StructuralTypeShape, TerminalIdentityIndex, TerminalMachine,
};

type NominalType = (TerminalIdentityIndex, TerminalIdentityIndex);

#[derive(Default)]
pub(super) struct ReachedTypes {
    candidates: std::cell::OnceCell<Vec<(NominalType, StructuralTypeId)>>,
    verified: std::cell::RefCell<Vec<(StructuralTypeId, StructuralTypeId)>>,
    pub(super) members: super::coordinates::ReachedMembers,
}

impl CheckedPrefix<'_> {
    pub(super) fn identity(&self, a: TerminalIdentityIndex, b: TerminalIdentityIndex) -> bool {
        if a.is_absent() || b.is_absent() {
            return a.is_absent() && b.is_absent();
        }
        NominalDeclarationIdentity::new(a).same_identity_as(
            &self.old.identities,
            &NominalDeclarationIdentity::new(b),
            &self.new.identities,
        )
    }

    /// Translate the nominal field selector carried by a stored projection.
    pub fn field_identity(&self, field: TerminalIdentityIndex) -> Option<TerminalIdentityIndex> {
        if field.is_absent() {
            return None;
        }
        let other = self
            .new
            .identities
            .index_of(self.old.identities.text(field)?)?;
        self.identity(field, other).then_some(other)
    }
    /// Translate an actual operand or stored value. The selected declaration
    /// and its nested field, case and reference types must agree in both programs.
    /// Additional types used only by the successor do not constrain the frame.
    pub fn structural_type(&self, old: StructuralTypeId) -> Option<StructuralTypeId> {
        let (to, _) = self.nominal_type(old)?;
        let mut verified = self.types.verified.borrow_mut();
        let start = verified.len();
        if self.types_match(old, to, &mut verified) {
            Some(to)
        } else {
            // A failed walk has proved none of the pairs it visited.
            verified.truncate(start);
            None
        }
    }

    pub(super) fn nominal_type(&self, old: StructuralTypeId) -> Option<(StructuralTypeId, bool)> {
        match self
            .old
            .identities
            .rejoined_record(old, &self.new.identities)
            .ok()?
        {
            Some(to) => Some((to, true)),
            None => Some((self.named_type(old)?, false)),
        }
    }

    fn named_type(&self, old: StructuralTypeId) -> Option<StructuralTypeId> {
        let from = self.old.structural_type(old)?;
        let identity = self.field_identity(from.identity)?;
        let declaration = if from.declaration_identity.is_absent() {
            TerminalIdentityIndex::ABSENT
        } else {
            self.field_identity(from.declaration_identity)?
        };
        let candidates = self.types.candidates.get_or_init(|| {
            let mut rows: Vec<_> = self
                .new
                .types
                .structural_types
                .iter()
                .map(|ty| ((ty.identity, ty.declaration_identity), ty.id))
                .collect();
            rows.sort_unstable();
            rows
        });
        let key = (identity, declaration);
        let first = candidates.partition_point(|(k, _)| *k < key);
        let &(candidate, to) = candidates.get(first)?;
        if candidate != key || candidates.get(first + 1).is_some_and(|(k, _)| *k == key) {
            return None;
        }
        Some(to)
    }

    fn scalar_set(&self, id: semantic_vocabulary::ScalarQualificationSetId) -> bool {
        if id == semantic_vocabulary::ScalarQualificationSetId::ZERO {
            return true;
        }
        let old = &self.old.types.scalar_qualifications;
        let new = &self.new.types.scalar_qualifications;
        let (Some(a), Some(b)) = (
            old.sets.iter().find(|s| s.id == id),
            new.sets.iter().find(|s| s.id == id),
        ) else {
            return false;
        };
        a.domains == b.domains
            && a.domains.iter().all(|d| {
                let (Some(l), Some(r)) = (
                    old.domains.iter().find(|s| s.id == *d),
                    new.domains.iter().find(|s| s.id == *d),
                ) else {
                    return false;
                };
                l.id == r.id
                    && l.semantic_domain == r.semantic_domain
                    && l.carrier == r.carrier
                    && l.required_memberships == r.required_memberships
                    && l.predicate == r.predicate
                    && l.establishment_routes == r.establishment_routes
                    && self.identity(l.identity, r.identity)
                    && l.required_memberships.as_ref().is_none_or(Vec::is_empty)
                    && l.predicate.as_ref().is_none_or(|p| {
                        matches!(
                            p.proposition,
                            semantic_vocabulary::Proposition::Truth
                                | semantic_vocabulary::Proposition::Falsehood
                        )
                    })
                    && self.issuer_routes(&l.establishment_routes)
                    && !matches!(l.carrier, semantic_vocabulary::ScalarType::Callable(_))
            })
    }

    fn issuer_routes(
        &self,
        routes: &Option<Vec<terminal_psi::StructuralEstablishmentRoute>>,
    ) -> bool {
        routes.iter().flatten().all(|route| {
            self.identity(route.identity(), route.identity())
                && self.identity(
                    route.declaration().identity(),
                    route.declaration().identity(),
                )
        })
    }

    pub(super) fn same_scalar_storage(
        &self,
        a: Option<semantic_vocabulary::QualifiedScalarType>,
        b: Option<semantic_vocabulary::QualifiedScalarType>,
    ) -> bool {
        a == b
            && a.is_none_or(|v| {
                self.scalar_set(v.qualifications)
                    && !matches!(v.scalar_type, semantic_vocabulary::ScalarType::Callable(_))
            })
    }

    /// Every stored domain tag also identifies the domain's carrier, checked
    /// membership and issuer. Refuse access plans whose executable selections
    /// need a separate correspondence.
    pub fn structural_domain(&self, id: StructuralDomainId) -> Option<StructuralDomainId> {
        let a = self.old.structural_domain(id)?;
        let identity = self.field_identity(a.identity)?;
        let mut candidates = self
            .new
            .types
            .structural_domains
            .iter()
            .filter(|d| d.identity == identity);
        let b = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        (self.structural_type(a.carrier) == Some(b.carrier)
            && self.identity(a.subject_identity, b.subject_identity)
            && a.required_memberships == b.required_memberships
            && a.required_memberships.as_ref().is_none_or(Vec::is_empty)
            && a.complete_predicate_theory == b.complete_predicate_theory
            && a.predicate == b.predicate
            && a.byte_predicate == b.byte_predicate
            && a.establishment_routes == b.establishment_routes
            && self.issuer_routes(&a.establishment_routes)
            && a.access_policy.is_none()
            && b.access_policy.is_none()
            && a.content_projection.is_none()
            && b.content_projection.is_none()
            && a.predicate.as_ref().is_none_or(|p| {
                matches!(
                    p.proposition,
                    semantic_vocabulary::Proposition::Truth
                        | semantic_vocabulary::Proposition::Falsehood
                )
            }))
        .then_some(b.id)
    }

    pub(super) fn storage_domain_unchanged(&self, id: StructuralDomainId) -> bool {
        self.structural_domain(id) == Some(id)
    }

    fn types_match(
        &self,
        old: StructuralTypeId,
        new: StructuralTypeId,
        visited: &mut Vec<(StructuralTypeId, StructuralTypeId)>,
    ) -> bool {
        use StructuralTypeShape::*;
        let mut pending = vec![(old, new)];
        while let Some((a, b)) = pending.pop() {
            if visited.contains(&(a, b)) {
                continue;
            }
            if visited.iter().any(|&(x, y)| x == a || y == b) {
                return false;
            }
            visited.push((a, b));
            let (Some(l), Some(r)) = (self.old.structural_type(a), self.new.structural_type(b))
            else {
                return false;
            };
            let primitive = |module: &terminal_psi::TerminalModule, id| {
                module
                    .types
                    .scalar_qualifications
                    .primitives
                    .iter()
                    .find(|p| p.structural_type == id)
                    .map(|p| p.value_type)
            };
            if !self.same_scalar_storage(primitive(self.old, a), primitive(self.new, b)) {
                return false;
            }
            let opaque = |module: &terminal_psi::TerminalModule, ty| {
                module
                    .types
                    .opaque_representations
                    .iter()
                    .any(|r| r.opaque == ty)
            };
            if opaque(self.old, a) || opaque(self.new, b) {
                return false;
            }
            let rejoined = match self.old.identities.rejoined_record(a, &self.new.identities) {
                Ok(Some(found)) if found == b => true,
                Ok(None) => false,
                _ => return false,
            };
            if (!rejoined
                && (!self.identity(l.declaration_identity, r.declaration_identity)
                    || l.identity.is_absent()
                    || !self.identity(l.identity, r.identity)))
                || l.owns_linear_claim != r.owns_linear_claim
                || l.multiplicity != r.multiplicity
                || l.no_code_affine_cleanup != r.no_code_affine_cleanup
                || l.descriptor_cleanup != r.descriptor_cleanup
                || l.nominal_cleanup.is_some()
                || r.nominal_cleanup.is_some()
                || !self.same_predicates(Roots::Record(a), &l.requires, &r.requires)
                || l.scalar_restriction != r.scalar_restriction
            {
                return false;
            }
            match (l.quotient_carrier, r.quotient_carrier) {
                (Some(x), Some(y)) => pending.push((x, y)),
                (None, None) => {}
                _ => return false,
            }
            if matches!(
                l.scalar_restriction
                    .as_ref()
                    .and_then(StructuralFieldType::scalar_type),
                Some(semantic_vocabulary::ScalarType::Callable(_))
            ) {
                return false;
            }
            match (&l.shape, &r.shape) {
                (PrimitiveScalar(semantic_vocabulary::ScalarType::Callable(_)), _) => return false,
                (
                    Reference {
                        referent: a,
                        access: x,
                    },
                    Reference {
                        referent: b,
                        access: y,
                    },
                ) if x == y => pending.push((*a, *b)),
                (
                    ElementView {
                        element: a,
                        access: x,
                    },
                    ElementView {
                        element: b,
                        access: y,
                    },
                ) if x == y => pending.push((*a, *b)),
                (
                    FixedArray {
                        element: a,
                        length: x,
                    },
                    FixedArray {
                        element: b,
                        length: y,
                    },
                ) if x.literal().is_some() && x.literal() == y.literal() => pending.push((*a, *b)),
                (Record { storage: a, .. }, Record { storage: b, .. })
                    if matches!(
                        (a, b),
                        (
                            terminal_psi::StructuralRecordStorage::Packed,
                            terminal_psi::StructuralRecordStorage::Packed
                        )
                    ) => {}
                (Mixed { .. }, Mixed { .. }) | (Sum { .. }, Sum { .. }) => {}
                (PrimitiveScalar(_) | ByteSequence(_) | Opaque | AtomicCell(_), _)
                    if l.shape == r.shape => {}
                _ => return false,
            }
            if l.shape.cases().len() != r.shape.cases().len() {
                return false;
            }
            for (x, y) in l.shape.cases().iter().zip(r.shape.cases()) {
                if (x.id != y.id && x.identity.is_absent() && !rejoined)
                    || !self.identity(x.identity, y.identity)
                    || (!rejoined && !self.identity(x.declaration_identity, y.declaration_identity))
                    || x.fields.len() != y.fields.len()
                {
                    return false;
                }
            }
            if !l
                .shape
                .fields()
                .zip(r.shape.fields())
                .all(|(x, y)| self.field(x, y, rejoined, &mut pending))
                || l.shape.fields().count() != r.shape.fields().count()
            {
                return false;
            }
        }
        true
    }
    fn field(
        &self,
        x: &StructuralFieldDeclaration,
        y: &StructuralFieldDeclaration,
        rejoined: bool,
        pending: &mut Vec<(StructuralTypeId, StructuralTypeId)>,
    ) -> bool {
        let qualification = |module: &terminal_psi::TerminalModule, id| {
            module
                .types
                .scalar_qualifications
                .fields
                .iter()
                .find(|f| f.field == id)
                .map(|p| p.value_type)
        };
        if !self.same_scalar_storage(qualification(self.old, x.id), qualification(self.new, y.id)) {
            return false;
        }
        if (x.id != y.id && x.identity.is_absent() && !rejoined)
            || !self.identity(x.identity, y.identity)
            || (!rejoined && !self.identity(x.declaration_identity, y.declaration_identity))
            || x.storage != y.storage
            || x.relevance != y.relevance
        {
            return false;
        }
        match (x.inline_carrier, y.inline_carrier) {
            (Some(a), Some(b)) => pending.push((a, b)),
            (None, None) => {}
            _ => return false,
        }
        if matches!(
            x.field_type.scalar_type(),
            Some(semantic_vocabulary::ScalarType::Callable(_))
        ) {
            return false;
        }
        match (&x.field_type, &y.field_type) {
            (StructuralFieldType::Structural(a), StructuralFieldType::Structural(b)) => {
                pending.push((*a, *b));
                true
            }
            (
                StructuralFieldType::Erased {
                    type_identity: a,
                    scalar: sa,
                },
                StructuralFieldType::Erased {
                    type_identity: b,
                    scalar: sb,
                },
            ) => sa == sb && self.identity(*a, *b),
            (a, b) => a == b,
        }
    }

    pub(super) fn scalars(
        &self,
        a: &[terminal_psi::ValueDeclaration],
        b: &[terminal_psi::ValueDeclaration],
    ) -> bool {
        a == b
            && a.iter().all(|v| {
                self.scalar_set(v.qualifications)
                    && !matches!(v.scalar_type, semantic_vocabulary::ScalarType::Callable(_))
            })
    }

    fn qualification_paths(
        &self,
        old: &[terminal_psi::StructuralPathQualification],
        new: &mut [terminal_psi::StructuralPathQualification],
    ) -> bool {
        old.len() == new.len()
            && old.iter().zip(new).all(|(a, b)| {
                a.domain == b.domain
                    && self.storage_domain_unchanged(a.domain)
                    && self.normalize_path(&a.path, &mut b.path).is_ok()
            })
    }

    pub(super) fn parameters(
        &self,
        l: &[StructuralParameterDeclaration],
        r: &[StructuralParameterDeclaration],
    ) -> bool {
        l.len() == r.len()
            && l.iter().zip(r).all(|(a, b)| {
                let mut adjusted = b.clone();
                if self.structural_type(a.structural_type) != Some(b.structural_type)
                    || !a
                        .qualifications
                        .iter()
                        .all(|d| self.storage_domain_unchanged(*d))
                    || !self.qualification_paths(
                        &a.projected_qualifications,
                        &mut adjusted.projected_qualifications,
                    )
                    || !self.qualification_paths(
                        &a.additional_input_qualifications,
                        &mut adjusted.additional_input_qualifications,
                    )
                {
                    return false;
                }
                adjusted.structural_type = a.structural_type;
                *a == adjusted
            })
    }
    pub(super) fn result(&self, l: &OperationResult, r: &OperationResult) -> bool {
        if let (OperationResult::Structural(a), OperationResult::Structural(b)) = (l, r) {
            let mut adjusted = b.clone();
            if self.structural_type(a.structural_type) != Some(b.structural_type)
                || !a
                    .qualifications
                    .iter()
                    .all(|d| self.storage_domain_unchanged(*d))
                || !self.qualification_paths(
                    &a.projected_qualifications,
                    &mut adjusted.projected_qualifications,
                )
                || !self.qualification_paths(
                    &a.additional_input_qualifications,
                    &mut adjusted.additional_input_qualifications,
                )
                || !self.result_claims(&a.claims, &adjusted.claims)
                || a.qualification_establishments
                    .iter()
                    .any(|row| !self.storage_domain_unchanged(row.domain))
            {
                return false;
            }
            for (a, b) in a.claims.iter().zip(&mut adjusted.claims) {
                b.path.clone_from_slice(&a.path);
            }
            adjusted.structural_type = a.structural_type;
            *a == adjusted
        } else {
            l == r
                && l.scalar().is_none_or(|v| {
                    self.scalar_set(v.qualifications)
                        && !matches!(v.scalar_type, semantic_vocabulary::ScalarType::Callable(_))
                })
        }
    }

    /// Only a parked root may gain inferred effects from its newly admitted
    /// suffix. Its old services and their parent declarations still agree.
    fn preserves_services(
        &self,
        old: &[semantic_vocabulary::ServiceId],
        new: &[semantic_vocabulary::ServiceId],
        open: bool,
    ) -> bool {
        use semantic_vocabulary::ServiceId;
        let translate = |id: ServiceId| {
            let source = self.old.service(id)?;
            let identity = self.field_identity(source.identity)?;
            let mut selected = self
                .new
                .interfaces
                .services
                .iter()
                .filter(|s| s.identity == identity);
            let target = selected.next()?;
            selected.next().is_none().then_some((source, target))
        };
        if (!open && old.len() != new.len())
            || old
                .iter()
                .any(|&id| !translate(id).is_some_and(|(_, s)| new.contains(&s.id)))
        {
            return false;
        }
        let mut pending = old.to_vec();
        let mut visited = Vec::new();
        while let Some(id) = pending.pop() {
            if visited.contains(&id) {
                continue;
            }
            visited.push(id);
            let Some((a, b)) = translate(id) else {
                return false;
            };
            if a.parents.len() != b.parents.len()
                || a.parents.iter().any(|&p| {
                    !translate(p).is_some_and(|(_, mapped)| b.parents.contains(&mapped.id))
                })
            {
                return false;
            }
            pending.extend(&a.parents);
        }
        true
    }

    fn same_invocation_declaration(&self, old: &TerminalMachine, new: &TerminalMachine) -> bool {
        old.declaration
            .as_ref()
            .zip(new.declaration.as_ref())
            .is_some_and(|(a, b)| {
                self.old
                    .identities
                    .rejoins_machine(a, &self.new.identities, b)
            })
    }

    pub(super) fn same_entry_requirements(
        &self,
        old: &TerminalMachine,
        new: &TerminalMachine,
    ) -> bool {
        if self.same_predicates(Roots::Machine(old, new), &old.requires, &new.requires) {
            return true;
        }
        // External entry requirements are promises to the caller. Only the
        // checked arrivals at a private helper can supply additional premises.
        if self.old.entry == Some(terminal_psi::TargetMachineCallable::Machine(old.id))
            || self.new.entry == Some(terminal_psi::TargetMachineCallable::Machine(new.id))
            || !self.same_invocation_declaration(old, new)
        {
            return false;
        }
        fn base<'a>(
            module: &'a terminal_psi::TerminalModule,
            machine: &'a TerminalMachine,
        ) -> impl Iterator<Item = &'a semantic_vocabulary::Proposition> {
            machine
                .requires
                .iter()
                .enumerate()
                .filter_map(move |(index, condition)| {
                    (!module
                        .identities
                        .matches_inferred_input_clause(machine.id, index, condition))
                    .then_some(condition)
                })
        }
        self.same_predicates(
            Roots::Machine(old, new),
            base(self.old, old),
            base(self.new, new),
        )
    }

    fn same_normal_guarantees(&self, old: &TerminalMachine, new: &TerminalMachine) -> bool {
        let roots = Roots::Machine(old, new);
        if old.ensures.len() == new.ensures.len()
            && old.ensures.iter().zip(&new.ensures).all(|(l, r)| {
                l.obligation == r.obligation
                    && self.same_predicate(roots, &l.proposition, &r.proposition)
            })
        {
            return true;
        }
        if !self.same_invocation_declaration(old, new) {
            return false;
        }
        // The body can establish more than the author promised. Each such
        // fact has to have been proved at every real return in its own program.
        fn written<'a>(
            module: &'a terminal_psi::TerminalModule,
            machine: &'a TerminalMachine,
        ) -> impl Iterator<Item = &'a semantic_vocabulary::Proposition> {
            machine
                .ensures
                .iter()
                .enumerate()
                .filter_map(move |(ordinal, clause)| {
                    (!module.identities.proved_private_result(machine, ordinal))
                        .then_some(&clause.proposition)
                })
        }
        self.same_predicates(roots, written(self.old, old), written(self.new, new))
    }

    fn same_guarded_guarantees(&self, old: &TerminalMachine, new: &TerminalMachine) -> bool {
        old.outcome_specific_ensures.len() == new.outcome_specific_ensures.len()
            && old
                .outcome_specific_ensures
                .iter()
                .zip(&new.outcome_specific_ensures)
                .all(|(l, r)| {
                    self.structural_type(l.guard.result_type) == Some(r.guard.result_type)
                        && self.case_id(l.guard.result_type, l.guard.result_case)
                            == Some(r.guard.result_case)
                        && l.position == r.position
                        && l.obligation == r.obligation
                        && self.same_predicate(
                            Roots::Machine(old, new),
                            &l.proposition,
                            &r.proposition,
                        )
                        && match (&l.evidence, &r.evidence) {
                            (None, None) => true,
                            (Some(x), Some(y)) => {
                                x.term == y.term
                                    && x.output_field == y.output_field
                                    && x.argument_subjects.len() == y.argument_subjects.len()
                                    && x.argument_subjects.iter().zip(&y.argument_subjects).all(
                                        |(a, b)| {
                                            a.argument_position == b.argument_position
                                                && a.parameter_position == b.parameter_position
                                                && match (&a.structural_path, &b.structural_path) {
                                                    (None, None) => true,
                                                    (Some(l), Some(r)) => self.paths(l, r),
                                                    _ => false,
                                                }
                                        },
                                    )
                            }
                            _ => false,
                        }
                })
    }

    pub(super) fn signature(
        &self,
        a: &TerminalMachine,
        b: &TerminalMachine,
        active_root: bool,
    ) -> Result<(), &'static str> {
        if !self.parameters(&a.structural_parameters, &b.structural_parameters)
            || !self.scalars(&a.parameters, &b.parameters)
            || a.parameter_order != b.parameter_order
            || !self.same_entry_requirements(a, b)
            || !self.same_predicates(
                Roots::Machine(a, b),
                &a.source_checked_requires,
                &b.source_checked_requires,
            )
            || !super::same_rows(a.entry_claims.iter(), b.entry_claims.iter(), |a, b| {
                self.entry_claim(a, b)
            })
            || a.content_entry_claims != b.content_entry_claims
            || a.erased_proof_formals != b.erased_proof_formals
            || a.erased_scalar_formals != b.erased_scalar_formals
        {
            return Err("successor changed a reached machine's entry contract");
        }
        if !self.machine_result(&a.result, &b.result)
            || !self.result_lifetimes(a.result_lifetimes.as_deref(), b.result_lifetimes.as_deref())
        {
            return Err("successor changed a reached machine's return contract");
        }
        if !self.same_normal_guarantees(a, b) || !self.same_guarded_guarantees(a, b) {
            return Err("successor changed a reached machine's normal guarantees");
        }
        let l = a
            .declaration
            .as_ref()
            .ok_or("old machine lacks a checked declaration")?;
        let r = b
            .declaration
            .as_ref()
            .ok_or("successor machine lacks a checked declaration")?;
        if !l.same_identity_as(&self.old.identities, r, &self.new.identities) {
            if self
                .old
                .identities
                .rejoins_machine(l, &self.new.identities, r)
                && l.specialization.is_none()
                && r.specialization.is_none()
                // The source producer's private commitment above binds every
                // parameter and result, including any anonymous record that
                // acquired its package name. Its lowered types were checked above.
                && l.published_may_suspend == r.published_may_suspend
                && l.published_may_block == r.published_may_block
            {
                let mut signature = b.contract.clone();
                signature.id = a.contract.id;
                signature.declaration = a.declaration.clone();
                // These typed rows have already been compared in their own
                // publications. Their local type or field numbers can differ.
                signature
                    .structural_parameters
                    .clone_from(&a.structural_parameters);
                signature.result.clone_from(&a.result);
                signature.requires.clone_from(&a.requires);
                signature
                    .source_checked_requires
                    .clone_from(&a.source_checked_requires);
                signature
                    .outcome_specific_ensures
                    .clone_from(&a.outcome_specific_ensures);
                signature.ensures.clone_from(&a.ensures);
                signature.result_lifetimes.clone_from(&a.result_lifetimes);
                if !self.preserves_services(
                    &a.published_service_ceiling,
                    &b.published_service_ceiling,
                    active_root
                        && a.declared_service_reach.is_none()
                        && b.declared_service_reach.is_none(),
                ) || !self.preserves_services(
                    &a.published_synchronous_entries,
                    &b.published_synchronous_entries,
                    active_root
                        && a.declared_synchronous_entries.is_none()
                        && b.declared_synchronous_entries.is_none(),
                ) {
                    return Err("successor changed a reached machine's service contract");
                }
                signature
                    .published_service_ceiling
                    .clone_from(&a.published_service_ceiling);
                signature
                    .published_synchronous_entries
                    .clone_from(&a.published_synchronous_entries);
                if signature == a.contract {
                    return Ok(());
                }
            }
            return Err("successor selected a different machine declaration");
        }
        Ok(())
    }

    pub(super) fn boundary(
        &self,
        a: BoundaryMachineId,
        b: BoundaryMachineId,
    ) -> Result<(), &'static str> {
        let l = self
            .old
            .boundary_machine(a)
            .ok_or("missing original boundary")?;
        let r = self
            .new
            .boundary_machine(b)
            .ok_or("missing successor boundary")?;
        let x = l
            .declaration
            .as_ref()
            .ok_or("original boundary lacks its checked declaration")?;
        let y = r
            .declaration
            .as_ref()
            .ok_or("successor boundary lacks its checked declaration")?;
        if !x.same_identity_as(self.old, y, self.new)
            || !self.identity(l.identity, r.identity)
            || !l
                .owner
                .same_identity_as(&self.old.identities, &r.owner, &self.new.identities)
            || l.parameter_order != r.parameter_order
            || l.scalar_parameters != r.scalar_parameters
            || l.scalar_parameters
                .iter()
                .any(|ty| matches!(ty, semantic_vocabulary::ScalarType::Callable(_)))
            || !self.boundary_returns(l, r)
            || !self.result_lifetimes(l.result_lifetimes.as_deref(), r.result_lifetimes.as_deref())
            || l.numeric_intrinsic != r.numeric_intrinsic
            || l.role != r.role
            || l.kind != r.kind
            || l.completion != r.completion
            || !self.parameters(&l.structural_parameters, &r.structural_parameters)
        {
            return Err("successor changed an invoked boundary or its declaration");
        }
        Ok(())
    }
}
