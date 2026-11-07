//! Checked algebra licenses permit reassociation and established-hypothesis
//! exchange. Callers supply the hypotheses of the relation they are proving:
//! scalar equations and core Id remain separate. Atomic comparison retains
//! complete resolution payloads, including IEEE bits, reference observations,
//! selected applications and opaque canonical content values.

use super::licenses::{SemiringLicense, term_uses_application};
use super::{StructuralJudge, StructuralTerm};

/// An algebra atom keeps all value and observation metadata. Debug provides
/// deterministic grouping only; equality always checks the complete payload.
#[derive(Clone)]
struct AlgebraAtom(StructuralTerm);

impl PartialEq for AlgebraAtom {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_resolution_payload(&other.0)
    }
}

impl Eq for AlgebraAtom {}

impl std::fmt::Debug for AlgebraAtom {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, formatter)
    }
}

impl StructuralJudge<'_> {
    /// Scalar comparison premises do not include arbitrary data Id rows.
    pub(super) fn ring_rearranged_equal(
        &self,
        left: &StructuralTerm,
        right: &StructuralTerm,
    ) -> bool {
        self.licensed_algebra_equal(
            left,
            right,
            self.rewrites.iter().map(|(left, right)| (left, right)),
        )
    }

    /// The rearrange tier's comparison: for each ring license whose op
    /// appears in the equation, flatten both sides into addend multisets
    /// (nested applications of the licensed op associate away; everything
    /// else is a structural atom) and compare. At least two
    /// addends must appear -- a single atom has nothing to rearrange.
    pub(super) fn licensed_algebra_equal<'a>(
        &self,
        left: &StructuralTerm,
        right: &StructuralTerm,
        hypotheses: impl Iterator<Item = (&'a StructuralTerm, &'a StructuralTerm)> + Clone,
    ) -> bool {
        for license in &self.ring_licenses {
            let op = &license.add_machine;
            if !term_uses_application(left, op) && !term_uses_application(right, op) {
                continue;
            }
            let mut left_addends = additive_addends(self.program, left, op);
            let mut right_addends = additive_addends(self.program, right, op);
            if left_addends.len() < 2 {
                continue;
            }
            left_addends.sort_by_key(|term| format!("{term:?}"));
            right_addends.sort_by_key(|term| format!("{term:?}"));
            if left_addends.len() == right_addends.len() && left_addends == right_addends {
                return true;
            }
            // HYPOTHESIS EXCHANGE (bounded, depth 2): a requires / citation /
            // IH equation whose sides flatten over this SAME licensed op
            // licenses swapping that sub-multiset of addends -- sum(left) ==
            // sum(left - from + to) because sum(from) == sum(to) is the
            // hypothesis and the op's comm+assoc closure is exactly what the
            // license's conformance proved. This is what makes QUOTIENT
            // lemmas provable: congruence needs ONE exchange (a.pos + a2.neg
            // exchanges inside a.pos + b.pos + a2.neg + b.neg), transitivity
            // needs TWO (h1 then h2 inside the cancellation citation's
            // requires). Whole-term matches were already rewritten during
            // resolve; this reaches the sub-multisets the rewriter cannot
            // see. Frontier-capped BFS -- over-refusal past the cap, never
            // unsound.
            let mut rewrite_addends: Vec<(Vec<AlgebraAtom>, Vec<AlgebraAtom>)> = Vec::new();
            for (pattern, replacement) in hypotheses.clone() {
                for (from, to) in [(pattern, replacement), (replacement, pattern)] {
                    let mut from_addends = additive_addends(self.program, from, op);
                    let to_addends = additive_addends(self.program, to, op);
                    from_addends.sort_by_key(|term| format!("{term:?}"));
                    rewrite_addends.push((from_addends, to_addends));
                }
            }
            let mut frontier: Vec<Vec<AlgebraAtom>> = vec![left_addends.clone()];
            for _depth in 0..2 {
                let mut next: Vec<Vec<AlgebraAtom>> = Vec::new();
                for current in &frontier {
                    for (from_addends, to_addends) in &rewrite_addends {
                        let Some(mut candidate) = sorted_multiset_subtract(current, from_addends)
                        else {
                            continue;
                        };
                        candidate.extend(to_addends.iter().cloned());
                        candidate.sort_by_key(|term| format!("{term:?}"));
                        if candidate == right_addends {
                            return true;
                        }
                        if next.len() < 64 {
                            next.push(candidate);
                        }
                    }
                }
                frontier = next;
                if frontier.is_empty() {
                    break;
                }
            }
        }
        // Tier-2 FULL POLYNOMIAL: under a paired license, both sides
        // normalize by distributing the licensed mul through the licensed
        // add into a multiset of monomials (each a sorted factor multiset).
        // The distributivity law is CONFORMED (machine-checked), so the
        // normal form is exactly what the carrier proved.
        for license in &self.semiring_licenses {
            if !term_uses_application(left, &license.mul_machine)
                && !term_uses_application(right, &license.mul_machine)
            {
                continue;
            }
            let (Some(mut left_poly), Some(mut right_poly)) = (
                polynomial_normal_form(self.program, left, license),
                polynomial_normal_form(self.program, right, license),
            ) else {
                continue;
            };
            left_poly.sort_by_key(|term| format!("{term:?}"));
            right_poly.sort_by_key(|term| format!("{term:?}"));
            if left_poly == right_poly {
                return true;
            }
            // SCALED-HYPOTHESIS EXCHANGE (tier-2 twin of the addend
            // exchange): a hypothesis equation polynomial-normalizes to a
            // monomial-multiset pair (hl, hr), and multiplying BOTH sides by
            // any monomial factor m keeps it an equation (the semiring's
            // conformed distributivity is exactly that license), so hl*m
            // exchanges for hr*m inside the goal's monomials. Factors are
            // drawn from the goal's own atoms (plus unscaled) -- this is
            // what proves mul-CONGRUENCE over a quotient: the cross-sum
            // hypothesis scaled by b.pos and by b.neg equalizes the product
            // components in two exchanges. Depth-2 frontier-capped BFS.
            let mut atoms: Vec<AlgebraAtom> = left_poly
                .iter()
                .chain(right_poly.iter())
                .flatten()
                .cloned()
                .collect();
            atoms.sort_by_key(|term| format!("{term:?}"));
            atoms.dedup();
            let mut scales: Vec<Vec<AlgebraAtom>> = vec![Vec::new()];
            scales.extend(atoms.into_iter().map(|atom| vec![atom]));
            let mut hypothesis_polys: Vec<(Vec<Vec<AlgebraAtom>>, Vec<Vec<AlgebraAtom>>)> =
                Vec::new();
            for (pattern, replacement) in hypotheses.clone() {
                if let (Some(mut hl), Some(mut hr)) = (
                    polynomial_normal_form(self.program, pattern, license),
                    polynomial_normal_form(self.program, replacement, license),
                ) {
                    hl.sort_by_key(|term| format!("{term:?}"));
                    hr.sort_by_key(|term| format!("{term:?}"));
                    hypothesis_polys.push((hl, hr));
                }
            }
            let scaled =
                |poly: &[Vec<AlgebraAtom>], scale: &[AlgebraAtom]| -> Vec<Vec<AlgebraAtom>> {
                    poly.iter()
                        .map(|monomial| {
                            let mut product = monomial.clone();
                            product.extend(scale.iter().cloned());
                            product.sort_by_key(|term| format!("{term:?}"));
                            product
                        })
                        .collect()
                };
            let mut frontier: Vec<Vec<Vec<AlgebraAtom>>> = vec![left_poly.clone()];
            for _depth in 0..2 {
                let mut next: Vec<Vec<Vec<AlgebraAtom>>> = Vec::new();
                for current in &frontier {
                    for (hypothesis_left, hypothesis_right) in &hypothesis_polys {
                        for (from, to) in [
                            (hypothesis_left, hypothesis_right),
                            (hypothesis_right, hypothesis_left),
                        ] {
                            for scale in &scales {
                                let from_scaled = scaled(from, scale);
                                let Some(mut candidate) =
                                    sorted_multiset_subtract(current, &from_scaled)
                                else {
                                    continue;
                                };
                                candidate.extend(scaled(to, scale));
                                candidate.sort_by_key(|term| format!("{term:?}"));
                                if candidate == right_poly {
                                    return true;
                                }
                                if next.len() < 64 {
                                    next.push(candidate);
                                }
                            }
                        }
                    }
                }
                frontier = next;
                if frontier.is_empty() {
                    break;
                }
            }
        }
        false
    }
}

/// Peel owned admission shells only to inspect a licensed application. A
/// reference remains its observed descriptor and content stays an opaque atom.
fn owned_application<'a>(
    program: &dyn crate::CheckingInput,
    mut term: &'a StructuralTerm,
) -> Option<(symbols::SymbolHandle, &'a [StructuralTerm])> {
    while let StructuralTerm::ObservedValue {
        observed_type,
        value,
        ..
    } = term
    {
        if crate::reference_shell(program, *observed_type).is_some() {
            return None;
        }
        term = value;
    }
    let StructuralTerm::Application {
        target,
        selections,
        arguments,
        ..
    } = term
    else {
        return None;
    };
    (selections.is_empty() && arguments.len() == 2).then_some((*target, arguments.as_slice()))
}

fn additive_addends(
    program: &dyn crate::CheckingInput,
    term: &StructuralTerm,
    op: &symbols::SymbolHandle,
) -> Vec<AlgebraAtom> {
    fn append(
        program: &dyn crate::CheckingInput,
        term: &StructuralTerm,
        op: &symbols::SymbolHandle,
        out: &mut Vec<AlgebraAtom>,
    ) {
        if let Some((target, arguments)) = owned_application(program, term)
            && target == *op
        {
            append(program, &arguments[0], op, out);
            append(program, &arguments[1], op, out);
        } else {
            out.push(AlgebraAtom(term.clone()));
        }
    }
    let mut out = Vec::new();
    append(program, term, op, &mut out);
    out
}

fn polynomial_normal_form(
    program: &dyn crate::CheckingInput,
    term: &StructuralTerm,
    license: &SemiringLicense,
) -> Option<Vec<Vec<AlgebraAtom>>> {
    const MONOMIAL_CAP: usize = 64;
    if let Some((target, arguments)) = owned_application(program, term) {
        if target == license.add_machine {
            let mut left = polynomial_normal_form(program, &arguments[0], license)?;
            let right = polynomial_normal_form(program, &arguments[1], license)?;
            left.extend(right);
            return (left.len() <= MONOMIAL_CAP).then_some(left);
        }
        if target == license.mul_machine {
            let left = polynomial_normal_form(program, &arguments[0], license)?;
            let right = polynomial_normal_form(program, &arguments[1], license)?;
            let mut product = Vec::new();
            for left_monomial in &left {
                for right_monomial in &right {
                    let mut monomial = left_monomial.clone();
                    monomial.extend(right_monomial.iter().cloned());
                    monomial.sort_by_key(|term| format!("{term:?}"));
                    product.push(monomial);
                }
            }
            return (product.len() <= MONOMIAL_CAP).then_some(product);
        }
    }
    Some(vec![vec![AlgebraAtom(term.clone())]])
}

/// Subtract exact atoms/monomials; a missing member leaves the goal unknown.
fn sorted_multiset_subtract<T: Clone + PartialEq>(left: &[T], from: &[T]) -> Option<Vec<T>> {
    let mut remaining = left.to_vec();
    for item in from {
        let index = remaining.iter().position(|candidate| candidate == item)?;
        remaining.remove(index);
    }
    Some(remaining)
}
