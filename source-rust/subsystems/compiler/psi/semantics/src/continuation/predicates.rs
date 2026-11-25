//! A field or case inside a proposition belongs to the declaration reached
//! from its root. Rejoin that declaration before comparing the two clauses.
use super::CheckedPrefix;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, IntegerMathTerm, PlaceId, Proposition,
    ScalarReadProjection, ScalarTerm, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{StructuralFieldType, StructuralTypeShape, TerminalMachine};

#[derive(Clone, Copy)]
pub(super) enum Roots<'a> {
    Machine(&'a TerminalMachine, &'a TerminalMachine),
    Record(StructuralTypeId),
}

impl CheckedPrefix<'_> {
    pub(super) fn same_predicate(
        &self,
        roots: Roots<'_>,
        old: &Proposition,
        new: &Proposition,
    ) -> bool {
        use Proposition::*;
        let mut pending = vec![(old, new)];
        while let Some((left, right)) = pending.pop() {
            match (left, right) {
                (Conjunction(l), Conjunction(r)) | (Disjunction(l), Disjunction(r))
                    if l.len() == r.len() =>
                {
                    pending.extend(l.iter().zip(r))
                }
                (
                    Implication {
                        premise: lp,
                        conclusion: lc,
                    },
                    Implication {
                        premise: rp,
                        conclusion: rc,
                    },
                ) => {
                    pending.push((lp, rp));
                    pending.push((lc, rc));
                }
                (Conjunction(_) | Disjunction(_) | Implication { .. }, _) => return false,
                _ => {
                    let mut translated = left.clone();
                    if self
                        .translate_atomic_predicate(roots, &mut translated)
                        .is_none()
                        || translated != *right
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub(super) fn same_predicates<'a, 'b>(
        &self,
        roots: Roots<'_>,
        old: impl IntoIterator<Item = &'a Proposition>,
        new: impl IntoIterator<Item = &'b Proposition>,
    ) -> bool {
        let mut new = new.into_iter();
        old.into_iter().all(|left| {
            new.next()
                .is_some_and(|right| self.same_predicate(roots, left, right))
        }) && new.next().is_none()
    }

    fn root_type(&self, roots: Roots<'_>, place: PlaceId) -> Option<StructuralTypeId> {
        match roots {
            Roots::Machine(a, b) => {
                let ty = a.structural_place_type(place)?;
                (self.structural_type(ty)? == b.structural_place_type(place)?).then_some(ty)
            }
            Roots::Record(ty) => (place.get() == 1).then_some(ty),
        }
    }

    fn case_for_clause(
        &self,
        roots: Roots<'_>,
        parent: StructuralTypeId,
        case: StructuralCaseId,
    ) -> Option<StructuralCaseId> {
        match roots {
            Roots::Machine(..) => self.case_id(parent, case),
            Roots::Record(_) => self.nominal_case_id(parent, case),
        }
    }

    fn field_for_clause(
        &self,
        roots: Roots<'_>,
        parent: StructuralTypeId,
        field: StructuralFieldId,
    ) -> Option<StructuralFieldId> {
        match roots {
            Roots::Machine(..) => self.field_id(parent, field),
            Roots::Record(_) => self.nominal_field_id(parent, field),
        }
    }

    fn element_type(&self, parent: Option<StructuralTypeId>) -> Option<StructuralTypeId> {
        match &self.old.structural_type(parent?)?.shape {
            StructuralTypeShape::FixedArray { element, .. }
            | StructuralTypeShape::ElementView { element, .. } => Some(*element),
            _ => None,
        }
    }

    fn clause_step(
        &self,
        roots: Roots<'_>,
        parent: &mut Option<StructuralTypeId>,
        step: &mut Segment,
    ) -> Option<()> {
        let ty = (*parent)?;
        let shape = &self.old.structural_type(ty)?.shape;
        match step {
            Segment::Field(id) => {
                let old = *id;
                *id = self.field_for_clause(roots, ty, old)?;
                let field = shape.fields().find(|field| field.id == old)?;
                *parent = match field.field_type {
                    StructuralFieldType::Structural(child) => Some(child),
                    _ => field.inline_carrier,
                };
            }
            Segment::Case(id) => *id = self.case_for_clause(roots, ty, *id)?,
            Segment::FixedIndex(_) => *parent = Some(self.element_type(*parent)?),
            Segment::Referent => {
                *parent = match shape {
                    StructuralTypeShape::Reference { referent, .. } => Some(*referent),
                    _ => return None,
                }
            }
        }
        Some(())
    }

    fn clause_path(
        &self,
        roots: Roots<'_>,
        place: PlaceId,
        path: &mut [Segment],
    ) -> Option<Option<StructuralTypeId>> {
        let mut parent = Some(self.root_type(roots, place)?);
        for step in path {
            self.clause_step(roots, &mut parent, step)?;
        }
        Some(parent)
    }

    fn clause_scalar(&self, roots: Roots<'_>, term: &mut ScalarTerm) -> Option<()> {
        use ScalarTerm::*;
        let mut pending = vec![term];
        while let Some(term) = pending.pop() {
            match term {
                BooleanField { root, path, .. }
                | IntegerField { root, path, .. }
                | IeeeFloatField { root, path, .. }
                | ViewExtent { root, path, .. }
                | ByteSequencePredicate { root, path, .. } => {
                    self.clause_path(roots, *root, path)?;
                }
                StructuralRead { root, path, .. } => {
                    let mut parent = Some(self.root_type(roots, *root)?);
                    for step in path {
                        match step {
                            ScalarReadProjection::Static(segment) => {
                                self.clause_step(roots, &mut parent, segment)?
                            }
                            ScalarReadProjection::Referent => {
                                self.clause_step(roots, &mut parent, &mut Segment::Referent)?
                            }
                            ScalarReadProjection::Index(_) => {
                                parent = Some(self.element_type(parent)?)
                            }
                            ScalarReadProjection::Extent => parent = None,
                        }
                    }
                }
                Application { .. } => return None,
                _ => {}
            }
            pending.extend(term.operands_mut());
        }
        Some(())
    }

    fn clause_math(&self, roots: Roots<'_>, term: &mut IntegerMathTerm) -> Option<()> {
        let mut pending = vec![term];
        while let Some(term) = pending.pop() {
            if let IntegerMathTerm::Embed(scalar) = term {
                self.clause_scalar(roots, scalar)?;
            }
            pending.extend(term.operands_mut());
        }
        Some(())
    }

    fn translate_atomic_predicate(
        &self,
        roots: Roots<'_>,
        predicate: &mut Proposition,
    ) -> Option<()> {
        use Proposition::*;
        match predicate {
            Truth | Falsehood => {}
            // These indirections require a correspondence for their proof or content table.
            Atom(_) | ContentConservation(_) => return None,
            Equal(l, r) | LessThan(l, r) | LessOrEqual(l, r) => {
                self.clause_scalar(roots, l)?;
                self.clause_scalar(roots, r)?;
            }
            ScalarIeeeFloatComparison {
                kind, left, right, ..
            } => {
                self.clause_scalar(roots, left)?;
                self.clause_scalar(roots, right)?;
                if kind.is_symmetric() && left > right {
                    std::mem::swap(left, right);
                }
            }
            IntegerMathEqual(left, right) => {
                self.clause_math(roots, left)?;
                self.clause_math(roots, right)?;
                if left > right {
                    std::mem::swap(left, right);
                }
            }
            IntegerMathLessThan(l, r) | IntegerMathLessOrEqual(l, r) => {
                self.clause_math(roots, l)?;
                self.clause_math(roots, r)?;
            }
            IeeeFloatComparison {
                kind, left, right, ..
            } => {
                for field in [&mut *left, &mut *right] {
                    let mut path = field.path().to_vec();
                    self.clause_path(roots, field.root(), &mut path)?;
                    *field = semantic_vocabulary::IeeeFloatStructuralField::new(field.root(), path)
                        .ok()?;
                }
                if kind.is_symmetric() && left > right {
                    std::mem::swap(left, right);
                }
            }
            StructuralCaseMembership { subject, case } => {
                let mut path = subject.path().to_vec();
                let parent = self.clause_path(roots, subject.root(), &mut path)??;
                *case = self.case_for_clause(roots, parent, *case)?;
                *subject = semantic_vocabulary::StructuralCaseSubject::new(subject.root(), path);
            }
            // Only a published Identity crosses: an input at its data type
            // and a scalar term at its scalar type.
            DeclarationIdentity(clause) => {
                use semantic_vocabulary::{ClauseCarrier, ClauseOperand};
                if !clause.is_portable() {
                    return None;
                }
                for value in &mut clause.values {
                    match (&mut value.carrier, &mut value.operand) {
                        (ClauseCarrier::Structural(carrier), ClauseOperand::Input(subject)) => {
                            let mut path = subject.path().to_vec();
                            let ty = self.clause_path(roots, subject.root(), &mut path)??;
                            if *carrier != ty {
                                return None;
                            }
                            *carrier = match roots {
                                Roots::Machine(..) => self.structural_type(ty)?,
                                Roots::Record(_) => self.nominal_type(ty)?.0,
                            };
                            *subject = semantic_vocabulary::StructuralCaseSubject::new(
                                subject.root(),
                                path,
                            );
                        }
                        (ClauseCarrier::Scalar(_), ClauseOperand::Scalar(term)) => {
                            self.clause_scalar(roots, term)?
                        }
                        _ => return None,
                    }
                }
            }
            Conjunction(_) | Disjunction(_) | Implication { .. } => {
                unreachable!("logical predicates are traversed in pairs")
            }
            FloatMeaningEqual(clause) => {
                *clause = clause
                    .map_sources::<&'static str>(|term| {
                        let mut term = term.clone();
                        self.clause_scalar(roots, &mut term)
                            .ok_or("unmatched float operand")?;
                        Ok(term)
                    })
                    .ok()?;
            }
            FloatIntegerConversion(clause) => {
                self.clause_scalar(roots, &mut clause.source)?;
                self.clause_scalar(roots, &mut clause.result)?;
            }
            IeeeFloatIntegerExact { source, .. } | IntegerIeeeFloatExact { source, .. } => {
                self.clause_scalar(roots, source)?;
            }
        }
        Some(())
    }
}
