//! Source equations and facts over already completed values.
//!
//! Their shared decomposition retains selected meanings and sequential bindings.
//! Only the completed entrance can retain authenticated integer observations;
//! source proof terms still owe the ordinary general normalization.

use super::super::{BinaryOperator, term_contains};
use super::{ResolutionPurpose, StructuralJudge, StructuralTerm, value_predicates};
use std::sync::Arc;

impl StructuralJudge<'_> {
    /// One structural equation: constructor pairs DECOMPOSE (injectivity --
    /// `Succ(a) == Succ(b)` yields `a == b`), distinct cases of one data
    /// make the hypotheses contradictory (disjointness), and a variable side
    /// becomes a directed substitution (first binding wins).
    pub(in crate::proof_contracts::contract_entailment) fn intake_equation(
        &mut self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) {
        self.intake_equation_with_purpose(left, right, depth, ResolutionPurpose::General);
    }

    /// Completed operands already returned under their own formation checks.
    /// Consume their current integer observations without reopening a retained
    /// call description. Non-integer, closed and recording queries retain the
    /// ordinary resolver's existing eligibility and meaning checks.
    pub(super) fn intake_completed_equation(
        &mut self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) {
        self.intake_equation_with_purpose(left, right, depth, ResolutionPurpose::IntegerProjection);
    }

    fn intake_equation_with_purpose(
        &mut self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
        purpose: ResolutionPurpose,
    ) {
        if self.case_facts_only || depth >= 32 {
            return;
        }
        let resolution_depth = self.application_resolution_depth();
        let left = self.resolve_with_at(left, resolution_depth, purpose);
        let right = self.resolve_with_at(right, resolution_depth, purpose);
        self.intake_equation_fact_with_purpose(left, right, depth, purpose);
    }

    /// Shared fact storage after the producer has selected its value inputs.
    /// Ordinary equations normalize above. Independently established scalar
    /// observations retain their occurrence instead of evaluating a description.
    /// This operation supplies no observation, formation or domain authority.
    pub(super) fn intake_equation_fact(
        &mut self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) {
        self.intake_equation_fact_with_purpose(left, right, depth, ResolutionPurpose::General);
    }

    fn equation_inputs_are_observed(
        &self,
        left: &StructuralTerm,
        right: &StructuralTerm,
        depth: usize,
    ) -> bool {
        !self.case_facts_only
            && depth < 32
            && !super::super::structural_terms::term_is_unobserved_descriptor(self.program, left)
            && !super::super::structural_terms::term_is_unobserved_descriptor(self.program, right)
    }

    fn intake_equation_fact_with_purpose(
        &mut self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
        purpose: ResolutionPurpose,
    ) {
        if !self.equation_inputs_are_observed(&left, &right, depth) {
            return;
        }
        let left = left.into_semantic_value();
        let right = right.into_semantic_value();
        if right == value_predicates::boolean_term(true)
            && let StructuralTerm::MathematicalBinary {
                operator: BinaryOperator::And,
                left,
                right,
            } = &left
            && left.is_checked_predicate()
            && right.is_checked_predicate()
        {
            self.intake_equation_with_purpose(
                *left.clone(),
                value_predicates::boolean_term(true),
                depth + 1,
                purpose,
            );
            self.intake_equation_with_purpose(
                *right.clone(),
                value_predicates::boolean_term(true),
                depth + 1,
                purpose,
            );
            return;
        }
        if let Some((operand, truth)) = self
            .boolean_equality_operand(&left, &right)
            .or_else(|| self.boolean_equality_operand(&right, &left))
        {
            self.intake_equation_with_purpose(operand, truth, depth + 1, purpose);
            return;
        }
        if let Some((left, right)) = self
            .identity_equality_operands(&left, &right)
            .or_else(|| self.identity_equality_operands(&right, &left))
        {
            self.intake_equation_with_purpose(left, right, depth + 1, purpose);
            return;
        }
        if purpose == ResolutionPurpose::IntegerProjection {
            let established =
                [&left, &right].map(|value| self.established_completed_capture_identity(value));
            if established.iter().any(Option::is_some) {
                self.register_completed_value_equation(left, right, established, depth);
                return;
            }
        }
        match (&left, &right) {
            (
                StructuralTerm::Constructor {
                    data: data_l,
                    case: case_l,
                    fields: fields_l,
                },
                StructuralTerm::Constructor {
                    data: data_r,
                    case: case_r,
                    fields: fields_r,
                },
            ) if data_l == data_r => {
                if case_l != case_r {
                    self.hypotheses_contradictory = true;
                    return;
                }
                for (name_l, value_l) in fields_l {
                    if let Some((_, value_r)) = fields_r.iter().find(|(name_r, _)| name_r == name_l)
                    {
                        self.intake_equation_with_purpose(
                            value_l.clone(),
                            value_r.clone(),
                            depth + 1,
                            purpose,
                        );
                    }
                }
            }
            (
                StructuralTerm::Application { .. }
                | StructuralTerm::CapturedValue { .. }
                | StructuralTerm::CollectionMeasure { .. }
                | StructuralTerm::Projection { .. }
                | StructuralTerm::IndexedValue { .. }
                | StructuralTerm::MathematicalBinary { .. }
                | StructuralTerm::IntegerEmbedding { .. }
                | StructuralTerm::ScalarRepresentation { .. }
                | StructuralTerm::ScalarBinary { .. },
                _,
            ) => {
                if !term_contains(&right, &left) {
                    Arc::make_mut(&mut self.rewrites).push((left, right));
                }
            }
            (
                _,
                StructuralTerm::Application { .. }
                | StructuralTerm::CapturedValue { .. }
                | StructuralTerm::CollectionMeasure { .. }
                | StructuralTerm::Projection { .. }
                | StructuralTerm::IndexedValue { .. }
                | StructuralTerm::MathematicalBinary { .. }
                | StructuralTerm::IntegerEmbedding { .. }
                | StructuralTerm::ScalarRepresentation { .. }
                | StructuralTerm::ScalarBinary { .. },
            ) => {
                if !term_contains(&left, &right) {
                    Arc::make_mut(&mut self.rewrites).push((right, left));
                }
            }
            (StructuralTerm::Variable(name), _) => {
                if left != right {
                    Arc::make_mut(&mut self.substitutions).push((name.clone(), right.clone()));
                    self.note_type_atom(name, &right);
                }
            }
            (_, StructuralTerm::Variable(name)) => {
                Arc::make_mut(&mut self.substitutions).push((name.clone(), left.clone()));
                self.note_type_atom(name, &left);
            }
            // A parameter-rooted member place resolves to its Opaque display
            // (`left.code`), so it carries the same rewrite as the unresolved
            // `Projection` above. Variables keep their substitution priority.
            (StructuralTerm::Opaque(_), _) => {
                if !term_contains(&right, &left) {
                    Arc::make_mut(&mut self.rewrites).push((left, right));
                }
            }
            (_, StructuralTerm::Opaque(_)) => {
                if !term_contains(&left, &right) {
                    Arc::make_mut(&mut self.rewrites).push((right, left));
                }
            }
            // Distinct literals are disjoint, as distinct cases are above:
            // a rewrite can resolve both sides of a later premise to them.
            (StructuralTerm::Integer(_), StructuralTerm::Integer(_)) if left != right => {
                self.hypotheses_contradictory = true;
            }
            _ => {}
        }
    }

    /// Static type equality is exact (generics.md, "Static type equality"): a
    /// type binder equated to one closed type is not equal to any other. This
    /// judge sees `T == i32` and `T == bool` as unrelated atoms, so binding the
    /// second one true while the first holds is the contradiction that lets a
    /// contract's equation refute a case whose default domain states another.
    fn note_type_atom(&mut self, name: &str, value: &StructuralTerm) {
        let Some((open, closed)) = super::super::structural_terms::open_closed_type_atom(name)
        else {
            return;
        };
        if value_predicates::boolean_value(value.semantic_value()) != Some(true) {
            return;
        }
        let conflicts = self.substitutions.iter().any(|(other, bound)| {
            other != name
                && super::super::structural_terms::open_closed_type_atom(other).is_some_and(
                    |(other_open, other_closed)| other_open == open && other_closed != closed,
                )
                && value_predicates::boolean_value(self.resolve(bound.clone()).semantic_value())
                    == Some(true)
        });
        if conflicts {
            self.hypotheses_contradictory = true;
        }
    }

    /// Find an exact previously registered rewrite, never an alias selected
    /// from a live place or a denotation peer. Call publication owns admission
    /// of a new observation; later completed equations use the retained roster.
    pub(super) fn exact_capture_rewrite(&self, value: &StructuralTerm) -> Option<StructuralTerm> {
        if !matches!(value, StructuralTerm::CapturedValue { .. }) {
            return None;
        }
        self.rewrites
            .find(value)
            .filter(|(pattern, _)| pattern.same_resolution_payload(value))
            .map(|(_, replacement)| replacement.clone())
    }

    fn established_completed_capture_identity(
        &self,
        value: &StructuralTerm,
    ) -> Option<StructuralTerm> {
        let replacement = self.exact_capture_rewrite(value)?;
        self.captured_result_equations
            .iter()
            .any(|equation| equation.retains_completed_identity(value, &replacement))
            .then_some(replacement)
    }

    /// Record the full equation and the relation between both already admitted
    /// presentations. First-match rewrites otherwise hide a later capture fact
    /// behind an earlier call description. This reads no declaration body and
    /// grants no new observation, access, formation or denotation authority.
    pub(super) fn register_completed_value_equation(
        &mut self,
        left: StructuralTerm,
        right: StructuralTerm,
        [established_left, established_right]: [Option<StructuralTerm>; 2],
        depth: usize,
    ) {
        if !self.equation_inputs_are_observed(&left, &right, depth) {
            return;
        }
        let has_established = established_left.is_some() || established_right.is_some();
        self.intake_equation_fact(left.clone(), right.clone(), depth);
        if has_established {
            // Preserve the established presentation's priority, just as normal
            // call identity publication does. Both sides keep their full payload.
            self.intake_equation_fact(
                established_right.unwrap_or(right),
                established_left.unwrap_or(left),
                depth,
            );
        }
    }
}
