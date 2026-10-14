//! Rename caller-local observations before continuation evidence is published.
//! Declaration atoms, erased formal positions and structural member identities
//! stay in their original telescopes. Only instantiated values and places move.

use semantic_vocabulary::{
    ContentConservation, ContentTerm, IntegerMathTerm, ProofTerm, Proposition, ScalarTerm,
};
use terminal_psi::{CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

use crate::lowering_error::LoweringError;

impl super::IdentityMap {
    pub(super) fn scalar_term(&mut self, term: &mut ScalarTerm) {
        let mut pending = vec![term];
        while let Some(term) = pending.pop() {
            match term {
                ScalarTerm::Value { id, .. } => *id = self.value(*id),
                ScalarTerm::StructuralRead { root, path, .. } => {
                    *root = self.place(*root);
                    for projection in path.iter_mut().rev() {
                        if let semantic_vocabulary::ScalarReadProjection::Index(selector) =
                            projection
                        {
                            pending.push(selector);
                        }
                    }
                }

                ScalarTerm::BooleanField { root, .. }
                | ScalarTerm::IntegerField { root, .. }
                | ScalarTerm::IeeeFloatField { root, .. }
                | ScalarTerm::ViewExtent { root, .. }
                | ScalarTerm::ByteSequencePredicate { root, .. } => *root = self.place(*root),
                semantic_vocabulary::scalar_term_unary_operand!(operand) => {
                    pending.push(operand);
                }
                semantic_vocabulary::scalar_term_binary_operands!(left, right) => {
                    pending.push(right);
                    pending.push(left);
                }
                ScalarTerm::Boolean(_)
                | ScalarTerm::Integer { .. }
                | ScalarTerm::IeeeFloatLiteral(_) => {}
            }
        }
    }

    pub(super) fn proof_term(&mut self, term: &mut ProofTerm) {
        let mut pending = vec![term];
        while let Some(term) = pending.pop() {
            match term {
                ProofTerm::Construction { fields, .. } => {
                    pending.extend(fields.iter_mut().rev().map(|field| &mut field.term));
                }
                ProofTerm::Array { elements, .. } => pending.extend(elements.iter_mut().rev()),
                ProofTerm::Scalar(term) => self.scalar_term(term),
                ProofTerm::Formal { .. } => {}
            }
        }
    }

    fn integer_math_term(&mut self, term: &mut IntegerMathTerm) {
        let mut pending = vec![term];
        while let Some(term) = pending.pop() {
            match term {
                IntegerMathTerm::MathValue { value, .. } => *value = self.value(*value),
                IntegerMathTerm::IntegerLiteral(_) => {}
                IntegerMathTerm::Add(left, right)
                | IntegerMathTerm::Subtract(left, right)
                | IntegerMathTerm::Multiply(left, right)
                | IntegerMathTerm::ShiftLeft {
                    value: left,
                    count: right,
                } => {
                    pending.push(right);
                    pending.push(left);
                }
            }
        }
    }

    fn content_term(&mut self, term: &mut ContentTerm) {
        match term {
            ContentTerm::Projection { subject, .. } => {
                subject.root = self.place(subject.root);
            }
            ContentTerm::Separate(children) => {
                for child in children.iter_mut() {
                    self.content_term(child);
                }
                // Renaming is injective: preserve cardinality and separation,
                // restoring only the order required by the content algebra.
                children.sort();
            }
        }
    }

    pub(super) fn proposition(
        &mut self,
        proposition: &mut Proposition,
    ) -> Result<(), LoweringError> {
        match proposition {
            Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
            Proposition::Equal(left, right) => {
                self.scalar_term(left);
                self.scalar_term(right);
                let left_key = terminal_codec::canonical_scalar_term_order_key(left)
                    .map_err(LoweringError::DebugSemanticCodec)?;
                let right_key = terminal_codec::canonical_scalar_term_order_key(right)
                    .map_err(LoweringError::DebugSemanticCodec)?;
                if left_key > right_key {
                    std::mem::swap(left, right);
                }
            }
            Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
                self.scalar_term(left);
                self.scalar_term(right);
            }
            Proposition::IntegerMathEqual(left, right) => {
                self.integer_math_term(left);
                self.integer_math_term(right);
                if left > right {
                    std::mem::swap(left, right);
                }
            }
            Proposition::IntegerMathLessThan(left, right)
            | Proposition::IntegerMathLessOrEqual(left, right) => {
                self.integer_math_term(left);
                self.integer_math_term(right);
            }
            Proposition::IeeeFloatComparison {
                kind, left, right, ..
            } => {
                *left = left.rebase(self.place(left.root()), &[]);
                *right = right.rebase(self.place(right.root()), &[]);
                if kind.is_symmetric() && left > right {
                    std::mem::swap(left, right);
                }
            }
            Proposition::ScalarIeeeFloatComparison {
                kind, left, right, ..
            } => {
                self.scalar_term(left);
                self.scalar_term(right);
                if kind.is_symmetric() && left > right {
                    std::mem::swap(left, right);
                }
            }
            Proposition::ByteSequenceEqual { left, right } => {
                *left = left.rebase(self.place(left.root()), &[]);
                *right = right.rebase(self.place(right.root()), &[]);
                if left > right {
                    std::mem::swap(left, right);
                }
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                *subject = subject.rebase(self.place(subject.root()), &[]);
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children.iter_mut() {
                    self.proposition(child)?;
                }
                // Terminal conjunction/disjunction order is wire order, not
                // Rust's derived variant order. Keep the existing arity intact.
                let mut keyed = children
                    .iter()
                    .map(|child| {
                        terminal_codec::canonical_proposition_order_key(child)
                            .map(|key| (key, child.clone()))
                            .map_err(LoweringError::DebugSemanticCodec)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                keyed.sort_by(|left, right| left.0.cmp(&right.0));
                *children = keyed.into_iter().map(|(_, child)| child).collect();
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                self.proposition(premise)?;
                self.proposition(conclusion)?;
            }
            Proposition::ContentConservation(conservation) => {
                let mut left = conservation.left().clone();
                let mut right = conservation.right().clone();
                self.content_term(&mut left);
                self.content_term(&mut right);
                *conservation =
                    ContentConservation::new(conservation.algebra().clone(), left, right);
            }
        }
        Ok(())
    }

    pub(super) fn crash_routes(
        &mut self,
        routes: &mut [CrashRouteBucket],
    ) -> Result<(), LoweringError> {
        for route in routes {
            for alternative in &mut route.alternatives {
                match alternative {
                    CrashRouteGuard::Truth => {}
                    CrashRouteGuard::Predicate(predicate) => {
                        let mut proposition = predicate.proposition().clone();
                        self.proposition(&mut proposition)?;
                        *predicate = CrashPredicateTerm::new(proposition);
                    }
                }
            }
            route.alternatives.sort();
        }
        Ok(())
    }
}
