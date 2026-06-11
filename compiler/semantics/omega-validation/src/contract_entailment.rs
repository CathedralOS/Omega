//! Requires -> ensures ENTAILMENT for empty-body proof machines.
//!
//! An empty-body machine with `requires`/`ensures` contracts is a proof
//! artifact (chapter 10): the claim is that the requirements entail the
//! guarantees. This module judges each ensures fact as PROVEN, REFUTED, or
//! UNKNOWN, and -- when the whole contract lies inside the engine's language --
//! rejects anything it cannot prove, so a false theorem can no longer pass
//! `--check` silently (see `wiki/proof_engine_roadmap.md`).
//!
//! ## The engine's language
//!
//! Terms: integer literals, the machine's own parameters, `+ - *` over those,
//! `t % k` with a positive constant `k` (with the euclidean range lemma), and
//! opaque proof-view applications (`Bag(items)`, `Seq(items)`) compared only
//! by equality. Facts: comparisons (`== != < <= > >=`) and range membership
//! (`t in lo..=hi`) over such terms. Anything else (domain membership,
//! unknown calls, non-parameter places) is OUTSIDE the language: the engine
//! still tries to prove with what it can see (extra unknown hypotheses can
//! only help, never hurt soundness of a proof), but it never REJECTS a
//! contract it cannot fully read.
//!
//! ## Mechanics
//!
//! 1. Terms normalize to canonical POLYNOMIALS over atoms (sum of monomials
//!    with i64 coefficients, atoms = parameter names / view applications /
//!    mod terms). Polynomial identity proves L0 constants, L3/L4 congruence
//!    and commutativity, and distributivity with no hypotheses at all.
//! 2. `requires` equations whose one side is a lone atom become directed
//!    SUBSTITUTIONS (occurs-checked, applied to fixpoint), so `a == b` lets
//!    `a + 1 == b + 1` normalize to `0 == 0`.
//! 3. Order and range facts become LOWER BOUNDS on difference polynomials
//!    (integer semantics: strict `<` is slack 1). Single-atom bounds and
//!    atom-minus-atom bounds feed a difference-bound matrix over the atoms
//!    plus a virtual ZERO atom, closed transitively (Floyd-Warshall), which
//!    proves L1 transitivity and L6 antisymmetry. A positive self-cycle means
//!    the requires set is UNSATISFIABLE: every ensures is vacuously true.
//! 4. Remaining goals go to an INTERVAL evaluator: atom intervals come from
//!    the closed matrix (plus `unsigned >= 0` and the mod lemma), monomials
//!    multiply intervals with CORRELATED powers (`a * a` squares one
//!    interval; it never treats the factors as independent), which proves L2
//!    range sums and L5 square ranges.
//! 5. REFUTATION is proving the goal's negation with the same machinery.
//!    Refutations gate on satisfiability of the visible requires set, exactly
//!    like the vacuity rule above.

use std::collections::BTreeMap;

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::SignatureContractKind;

pub(crate) fn validate_machine_contract_entailment(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let body_is_empty = program.machine_states(machine).iter().all(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .is_empty()
    });
    if !body_is_empty {
        return;
    }

    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    for contract in program.machine_contracts(machine) {
        let bucket = match contract.kind {
            SignatureContractKind::Requires => &mut requires,
            SignatureContractKind::Ensures => &mut ensures,
            SignatureContractKind::Boundary => continue,
        };
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                bucket.push(*expression);
            }
        }
    }
    if ensures.is_empty() {
        return;
    }

    let mut engine = Engine::new(program, machine);
    let requires_fully_visible = engine.add_requires(&requires);
    if engine.requires_unsatisfiable {
        // Contradictory hypotheses entail everything: the proof-theoretic
        // `absurd` case. Accept.
        return;
    }

    for fact in &ensures {
        if std::env::var("OMEGA_ENTAILMENT_TRACE").is_ok() {
            eprintln!(
                "ENTAILDBG machine={} fact={} visible={} params={:?}",
                machine.name,
                program.expression_table.display_name(*fact),
                requires_fully_visible,
                engine.parameter_atoms
            );
        }
        match engine.judge(*fact) {
            Judgment::Proven => {}
            Judgment::Refuted => {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` ensures contract proof fact `{}` is disproved: the requires contract entails its negation",
                    machine.name,
                    program.expression_table.display_name(*fact)
                )));
            }
            Judgment::ConstantFalse => {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` ensures contract proof fact `{}` is disproved by constant arithmetic",
                    machine.name,
                    program.expression_table.display_name(*fact)
                )));
            }
            Judgment::Unknown { goal_in_language } => {
                if goal_in_language && requires_fully_visible {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` cannot prove ensures contract proof fact `{}` from the requires contract",
                        machine.name,
                        program.expression_table.display_name(*fact)
                    )));
                }
                // Otherwise the contract leans on facts outside the engine's
                // language (domain membership, unknown calls, non-parameter
                // places): stand down rather than reject what we cannot read.
            }
        }
    }
}

enum Judgment {
    Proven,
    /// Disproved purely by folding both sides to constants.
    ConstantFalse,
    /// The visible requires facts prove the goal's negation.
    Refuted,
    Unknown {
        goal_in_language: bool,
    },
}

/// A monomial: atoms (by canonical display name) to powers. Empty = the
/// constant monomial.
type Monomial = BTreeMap<String, u32>;

/// A polynomial: monomials to i64 coefficients. Zero coefficients are never
/// stored, so structural equality is polynomial identity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Polynomial {
    terms: BTreeMap<Monomial, i64>,
}

impl Polynomial {
    fn constant(value: i64) -> Self {
        let mut polynomial = Self::default();
        if value != 0 {
            polynomial.terms.insert(Monomial::new(), value);
        }
        polynomial
    }

    fn atom(name: String) -> Self {
        let mut monomial = Monomial::new();
        monomial.insert(name, 1);
        let mut polynomial = Self::default();
        polynomial.terms.insert(monomial, 1);
        polynomial
    }

    fn constant_value(&self) -> Option<i64> {
        match self.terms.len() {
            0 => Some(0),
            1 => self.terms.get(&Monomial::new()).copied(),
            _ => None,
        }
    }

    fn checked_add(&self, other: &Self) -> Option<Self> {
        let mut terms = self.terms.clone();
        for (monomial, coefficient) in &other.terms {
            let entry = terms.entry(monomial.clone()).or_insert(0);
            *entry = entry.checked_add(*coefficient)?;
            if *entry == 0 {
                terms.remove(monomial);
            }
        }
        Some(Self { terms })
    }

    fn checked_neg(&self) -> Option<Self> {
        let mut terms = BTreeMap::new();
        for (monomial, coefficient) in &self.terms {
            terms.insert(monomial.clone(), coefficient.checked_neg()?);
        }
        Some(Self { terms })
    }

    fn checked_sub(&self, other: &Self) -> Option<Self> {
        self.checked_add(&other.checked_neg()?)
    }

    fn checked_mul(&self, other: &Self) -> Option<Self> {
        let mut result = Self::default();
        for (left_monomial, left_coefficient) in &self.terms {
            for (right_monomial, right_coefficient) in &other.terms {
                let coefficient = left_coefficient.checked_mul(*right_coefficient)?;
                let mut monomial = left_monomial.clone();
                for (atom, power) in right_monomial {
                    let entry = monomial.entry(atom.clone()).or_insert(0);
                    *entry = entry.checked_add(*power)?;
                }
                let entry = result.terms.entry(monomial.clone()).or_insert(0);
                *entry = entry.checked_add(coefficient)?;
                if *entry == 0 {
                    result.terms.remove(&monomial);
                }
            }
        }
        Some(result)
    }

    /// `(difference-of-two-unit-atoms, constant)`: `a - b + c` as
    /// `Some((a, b, c))`. The shape the difference-bound matrix consumes.
    fn as_atom_difference(&self) -> Option<(String, String, i64)> {
        let mut positive = None;
        let mut negative = None;
        let mut constant = 0i64;
        for (monomial, coefficient) in &self.terms {
            if monomial.is_empty() {
                constant = *coefficient;
                continue;
            }
            if monomial.len() != 1 || *monomial.values().next().unwrap() != 1 {
                return None;
            }
            let atom = monomial.keys().next().unwrap().clone();
            match coefficient {
                1 if positive.is_none() => positive = Some(atom),
                -1 if negative.is_none() => negative = Some(atom),
                _ => return None,
            }
        }
        Some((positive?, negative?, constant))
    }

    /// `(single-unit-atom, coefficient-sign, constant)` for bounds like
    /// `a + c >= 0` / `-a + c >= 0`.
    fn as_single_atom(&self) -> Option<(String, i64, i64)> {
        let mut atom = None;
        let mut coefficient_value = 0i64;
        let mut constant = 0i64;
        for (monomial, coefficient) in &self.terms {
            if monomial.is_empty() {
                constant = *coefficient;
                continue;
            }
            if monomial.len() != 1 || *monomial.values().next().unwrap() != 1 || atom.is_some() {
                return None;
            }
            atom = Some(monomial.keys().next().unwrap().clone());
            coefficient_value = *coefficient;
        }
        let atom = atom?;
        if coefficient_value != 1 && coefficient_value != -1 {
            return None;
        }
        Some((atom, coefficient_value, constant))
    }
}

/// An interval with optional (= unbounded) ends, all arithmetic checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Interval {
    low: Option<i64>,
    high: Option<i64>,
}

impl Interval {
    const UNBOUNDED: Interval = Interval {
        low: None,
        high: None,
    };

    fn constant(value: i64) -> Self {
        Self {
            low: Some(value),
            high: Some(value),
        }
    }

    fn add(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => a.checked_add(b),
                _ => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => a.checked_add(b),
                _ => None,
            },
        }
    }

    fn scale(self, factor: i64) -> Self {
        let scaled_low = self.low.and_then(|value| value.checked_mul(factor));
        let scaled_high = self.high.and_then(|value| value.checked_mul(factor));
        if factor >= 0 {
            Self {
                low: scaled_low,
                high: scaled_high,
            }
        } else {
            Self {
                low: scaled_high,
                high: scaled_low,
            }
        }
    }

    fn multiply(self, other: Self) -> Self {
        let candidates = [
            checked_pair_mul(self.low, other.low),
            checked_pair_mul(self.low, other.high),
            checked_pair_mul(self.high, other.low),
            checked_pair_mul(self.high, other.high),
        ];
        // Any unbounded/overflowed corner makes the product unbounded on the
        // side it could extend.
        if candidates.iter().any(|corner| corner.is_none())
            || self.low.is_none()
            || self.high.is_none()
            || other.low.is_none()
            || other.high.is_none()
        {
            return Interval::UNBOUNDED;
        }
        let values: Vec<i64> = candidates.into_iter().map(Option::unwrap).collect();
        Self {
            low: values.iter().min().copied(),
            high: values.iter().max().copied(),
        }
    }

    /// `self` raised to `power`, treating repeated factors as CORRELATED:
    /// an even power of any interval is non-negative, and the square of
    /// `[lo, hi]` is exact rather than the independent product.
    fn correlated_power(self, power: u32) -> Self {
        if power == 0 {
            return Self::constant(1);
        }
        if power == 1 {
            return self;
        }
        let (Some(low), Some(high)) = (self.low, self.high) else {
            // Unbounded base: an even power is still known non-negative.
            return if power % 2 == 0 {
                Self {
                    low: Some(0),
                    high: None,
                }
            } else {
                Interval::UNBOUNDED
            };
        };
        let corner_low = checked_pow(low, power);
        let corner_high = checked_pow(high, power);
        let (Some(corner_low), Some(corner_high)) = (corner_low, corner_high) else {
            return Interval::UNBOUNDED;
        };
        if power % 2 == 1 {
            return Self {
                low: Some(corner_low),
                high: Some(corner_high),
            };
        }
        let max_corner = corner_low.max(corner_high);
        let min_corner = if low <= 0 && 0 <= high {
            0
        } else {
            corner_low.min(corner_high)
        };
        Self {
            low: Some(min_corner),
            high: Some(max_corner),
        }
    }
}

fn checked_pair_mul(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    left?.checked_mul(right?)
}

fn checked_pow(base: i64, power: u32) -> Option<i64> {
    let mut result = 1i64;
    for _ in 0..power {
        result = result.checked_mul(base)?;
    }
    Some(result)
}

struct Engine<'program> {
    program: &'program TypedTrees,
    /// Canonical atom names for the machine's parameters.
    parameter_atoms: Vec<String>,
    /// Parameters whose primitive type is unsigned carry an implicit `>= 0`.
    unsigned_atoms: Vec<String>,
    /// Directed substitutions from requires equations (`atom := polynomial`),
    /// applied to fixpoint during normalization.
    substitutions: BTreeMap<String, Polynomial>,
    /// Lower bounds: each entry means `polynomial >= bound`.
    bounds: Vec<(Polynomial, i64)>,
    /// Mod-term atoms with their euclidean intervals (`t % k` in `0 ..= k-1`).
    mod_intervals: BTreeMap<String, Interval>,
    /// Difference-bound matrix over atoms + the virtual ZERO atom:
    /// `matrix[a][b]` = best known lower bound of `a - b`.
    matrix: BTreeMap<String, BTreeMap<String, i64>>,
    requires_unsatisfiable: bool,
}

const ZERO_ATOM: &str = "\u{0}zero";
const SUBSTITUTION_ROUNDS: usize = 8;

impl<'program> Engine<'program> {
    fn new(program: &'program TypedTrees, machine: &Machine) -> Self {
        let mut parameter_atoms = Vec::new();
        let mut unsigned_atoms = Vec::new();
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.is_self {
                    continue;
                }
                let name = parameter.name.as_str().to_owned();
                if !parameter_atoms.contains(&name) {
                    let primitive = program
                        .type_reference_table
                        .primitive_type(parameter.type_reference);
                    // `is_signed_integer` is false exactly for the unsigned
                    // integer primitives (floats/bool/string report true), so
                    // this marks precisely the `>= 0` carriers.
                    if let Some(primitive) = primitive {
                        if !primitive.is_signed_integer() {
                            unsigned_atoms.push(name.clone());
                        }
                    }
                    parameter_atoms.push(name);
                }
            }
        }
        Self {
            program,
            parameter_atoms,
            unsigned_atoms,
            substitutions: BTreeMap::new(),
            bounds: Vec::new(),
            mod_intervals: BTreeMap::new(),
            matrix: BTreeMap::new(),
            requires_unsatisfiable: false,
        }
    }

    /// Load the requires facts. Returns whether EVERY fact was inside the
    /// engine's language (full visibility is the precondition for rejecting
    /// unproven ensures).
    fn add_requires(&mut self, facts: &[ExpressionHandle]) -> bool {
        // First pass: harvest substitutions from equations so every later
        // normalization sees them. Range membership lowers to `&&` chains
        // (`x in 1..=10` arrives as `(x >= 1) && (x <= 10)`), so facts split
        // into conjuncts first.
        let mut fully_visible = true;
        let mut comparisons = Vec::new();
        for fact in facts {
            for conjunct in self.conjuncts(*fact) {
                match self.comparison_polynomials(conjunct) {
                    Some(comparison) => comparisons.push(comparison),
                    None => fully_visible = false,
                }
            }
        }
        for (operator, left, right) in &comparisons {
            if *operator == BinaryOperator::Equal {
                self.harvest_substitution(left, right);
            }
        }
        // Second pass: re-normalize under the substitutions and store bounds.
        let mut lower_bounds = Vec::new();
        for (operator, left, right) in comparisons {
            let left = self.substituted(&left);
            let right = self.substituted(&right);
            let Some(difference_rl) = right.checked_sub(&left) else {
                fully_visible = false;
                continue;
            };
            let Some(difference_lr) = left.checked_sub(&right) else {
                fully_visible = false;
                continue;
            };
            match operator {
                BinaryOperator::Less => lower_bounds.push((difference_rl, 1)),
                BinaryOperator::LessOrEqual => lower_bounds.push((difference_rl, 0)),
                BinaryOperator::Greater => lower_bounds.push((difference_lr, 1)),
                BinaryOperator::GreaterOrEqual => lower_bounds.push((difference_lr, 0)),
                BinaryOperator::Equal => {
                    lower_bounds.push((difference_rl, 0));
                    lower_bounds.push((difference_lr, 0));
                }
                // A `!=` hypothesis carries no single lower bound; ignore it
                // (sound: dropping hypotheses only weakens proving power).
                BinaryOperator::NotEqual => {}
                _ => fully_visible = false,
            }
        }
        for (polynomial, bound) in lower_bounds {
            if let Some(value) = polynomial.constant_value() {
                if value < bound {
                    self.requires_unsatisfiable = true;
                }
                continue;
            }
            self.bounds.push((polynomial, bound));
        }

        self.seed_matrix();
        self.close_matrix();
        fully_visible
    }

    /// Judge a full ensures fact: an `&&` chain proves when every conjunct
    /// proves, and is disproved when any conjunct is.
    fn judge(&mut self, fact: ExpressionHandle) -> Judgment {
        let conjuncts = self.conjuncts(fact);
        if conjuncts.len() > 1 {
            // A disproved conjunct disproves the chain even if an earlier
            // conjunct was merely unknown, so judge all of them first.
            let mut constant_false = false;
            let mut refuted = false;
            let mut unknown = false;
            let mut all_in_language = true;
            for conjunct in conjuncts {
                match self.judge(conjunct) {
                    Judgment::Proven => {}
                    Judgment::ConstantFalse => constant_false = true,
                    Judgment::Refuted => refuted = true,
                    Judgment::Unknown { goal_in_language } => {
                        unknown = true;
                        all_in_language &= goal_in_language;
                    }
                }
            }
            return if constant_false {
                Judgment::ConstantFalse
            } else if refuted {
                Judgment::Refuted
            } else if unknown {
                Judgment::Unknown {
                    goal_in_language: all_in_language,
                }
            } else {
                Judgment::Proven
            };
        }

        let Some((operator, left, right)) = self.comparison_polynomials(fact) else {
            return Judgment::Unknown {
                goal_in_language: false,
            };
        };
        let left = self.substituted(&left);
        let right = self.substituted(&right);
        let (Some(difference_rl), Some(difference_lr)) =
            (right.checked_sub(&left), left.checked_sub(&right))
        else {
            return Judgment::Unknown {
                goal_in_language: false,
            };
        };

        // Constant fold first: it gives the crispest diagnostic.
        if let Some(value) = difference_rl.constant_value() {
            let holds = match operator {
                BinaryOperator::Less => value > 0,
                BinaryOperator::LessOrEqual => value >= 0,
                BinaryOperator::Greater => value < 0,
                BinaryOperator::GreaterOrEqual => value <= 0,
                BinaryOperator::Equal => value == 0,
                BinaryOperator::NotEqual => value != 0,
                _ => {
                    return Judgment::Unknown {
                        goal_in_language: false,
                    };
                }
            };
            return if holds {
                Judgment::Proven
            } else {
                Judgment::ConstantFalse
            };
        }

        let proved = match operator {
            BinaryOperator::Less => self.prove_at_least(&difference_rl, 1),
            BinaryOperator::LessOrEqual => self.prove_at_least(&difference_rl, 0),
            BinaryOperator::Greater => self.prove_at_least(&difference_lr, 1),
            BinaryOperator::GreaterOrEqual => self.prove_at_least(&difference_lr, 0),
            BinaryOperator::Equal => {
                self.prove_at_least(&difference_rl, 0) && self.prove_at_least(&difference_lr, 0)
            }
            BinaryOperator::NotEqual => {
                self.prove_at_least(&difference_rl, 1) || self.prove_at_least(&difference_lr, 1)
            }
            _ => {
                return Judgment::Unknown {
                    goal_in_language: false,
                };
            }
        };
        if proved {
            return Judgment::Proven;
        }

        let negation_proved = match operator {
            // not (l < r)  ==  l >= r
            BinaryOperator::Less => self.prove_at_least(&difference_lr, 0),
            BinaryOperator::LessOrEqual => self.prove_at_least(&difference_lr, 1),
            BinaryOperator::Greater => self.prove_at_least(&difference_rl, 0),
            BinaryOperator::GreaterOrEqual => self.prove_at_least(&difference_rl, 1),
            BinaryOperator::Equal => {
                self.prove_at_least(&difference_rl, 1) || self.prove_at_least(&difference_lr, 1)
            }
            BinaryOperator::NotEqual => {
                self.prove_at_least(&difference_rl, 0) && self.prove_at_least(&difference_lr, 0)
            }
            _ => false,
        };
        if negation_proved {
            return Judgment::Refuted;
        }

        Judgment::Unknown {
            goal_in_language: true,
        }
    }

    /// Prove `polynomial >= bound` via the difference-bound matrix or the
    /// interval evaluator.
    fn prove_at_least(&self, polynomial: &Polynomial, bound: i64) -> bool {
        if let Some((positive, negative, constant)) = polynomial.as_atom_difference() {
            if let Some(best) = self.matrix_bound(&positive, &negative) {
                if let Some(total) = best.checked_add(constant) {
                    if total >= bound {
                        return true;
                    }
                }
            }
        }
        if let Some((atom, sign, constant)) = polynomial.as_single_atom() {
            let other = if sign == 1 {
                self.matrix_bound(&atom, ZERO_ATOM)
            } else {
                self.matrix_bound(ZERO_ATOM, &atom)
            };
            if let Some(best) = other {
                if let Some(total) = best.checked_add(constant) {
                    if total >= bound {
                        return true;
                    }
                }
            }
        }
        if let Some(low) = self.polynomial_interval(polynomial).low {
            if low >= bound {
                return true;
            }
        }
        false
    }

    fn polynomial_interval(&self, polynomial: &Polynomial) -> Interval {
        let mut total = Interval::constant(0);
        for (monomial, coefficient) in &polynomial.terms {
            let mut product = Interval::constant(1);
            for (atom, power) in monomial {
                let base = self.atom_interval(atom);
                product = product.multiply(base.correlated_power(*power));
            }
            total = total.add(product.scale(*coefficient));
        }
        total
    }

    fn atom_interval(&self, atom: &str) -> Interval {
        if let Some(interval) = self.mod_intervals.get(atom) {
            return *interval;
        }
        Interval {
            low: self.matrix_bound(atom, ZERO_ATOM),
            high: self
                .matrix_bound(ZERO_ATOM, atom)
                .and_then(i64::checked_neg),
        }
    }

    fn matrix_bound(&self, from: &str, to: &str) -> Option<i64> {
        if from == to {
            return Some(0);
        }
        self.matrix.get(from).and_then(|row| row.get(to)).copied()
    }

    fn seed_matrix(&mut self) {
        for atom in self.unsigned_atoms.clone() {
            self.record_difference(&atom, ZERO_ATOM, 0);
        }
        let mod_atoms: Vec<(String, Interval)> = self
            .mod_intervals
            .iter()
            .map(|(atom, interval)| (atom.clone(), *interval))
            .collect();
        for (atom, interval) in mod_atoms {
            if let Some(low) = interval.low {
                self.record_difference(&atom, ZERO_ATOM, low);
            }
            if let Some(high) = interval.high {
                if let Some(negated) = high.checked_neg() {
                    self.record_difference(ZERO_ATOM, &atom, negated);
                }
            }
        }
        for (polynomial, bound) in self.bounds.clone() {
            if let Some((positive, negative, constant)) = polynomial.as_atom_difference() {
                if let Some(edge) = bound.checked_sub(constant) {
                    self.record_difference(&positive, &negative, edge);
                }
            }
            if let Some((atom, sign, constant)) = polynomial.as_single_atom() {
                if let Some(edge) = bound.checked_sub(constant) {
                    if sign == 1 {
                        self.record_difference(&atom, ZERO_ATOM, edge);
                    } else {
                        self.record_difference(ZERO_ATOM, &atom, edge);
                    }
                }
            }
        }
    }

    fn record_difference(&mut self, from: &str, to: &str, bound: i64) {
        let row = self.matrix.entry(from.to_owned()).or_default();
        let entry = row.entry(to.to_owned()).or_insert(bound);
        if bound > *entry {
            *entry = bound;
        }
        self.matrix.entry(to.to_owned()).or_default();
    }

    fn close_matrix(&mut self) {
        let atoms: Vec<String> = self.matrix.keys().cloned().collect();
        for via in &atoms {
            for from in &atoms {
                let Some(first) = self.matrix_bound(from, via) else {
                    continue;
                };
                for to in &atoms {
                    let Some(second) = self.matrix_bound(via, to) else {
                        continue;
                    };
                    let Some(combined) = first.checked_add(second) else {
                        continue;
                    };
                    if from == to {
                        if combined > 0 {
                            self.requires_unsatisfiable = true;
                        }
                        continue;
                    }
                    let current = self.matrix_bound(from, to);
                    if current.is_none() || combined > current.unwrap() {
                        self.record_difference(from, to, combined);
                    }
                }
            }
        }
    }

    fn harvest_substitution(&mut self, left: &Polynomial, right: &Polynomial) {
        for (candidate, replacement) in [(left, right), (right, left)] {
            if let Some((atom, 1, 0)) = candidate.as_single_atom() {
                let occurs = replacement
                    .terms
                    .keys()
                    .any(|monomial| monomial.contains_key(&atom));
                if !occurs && !self.substitutions.contains_key(&atom) {
                    self.substitutions.insert(atom, replacement.clone());
                    return;
                }
            }
        }
    }

    fn substituted(&self, polynomial: &Polynomial) -> Polynomial {
        let mut current = polynomial.clone();
        for _ in 0..SUBSTITUTION_ROUNDS {
            let mut changed = false;
            let mut next = Polynomial::default();
            let mut overflowed = false;
            for (monomial, coefficient) in &current.terms {
                let mut piece = Polynomial::constant(*coefficient);
                for (atom, power) in monomial {
                    let base = match self.substitutions.get(atom) {
                        Some(replacement) => {
                            changed = true;
                            replacement.clone()
                        }
                        None => Polynomial::atom(atom.clone()),
                    };
                    for _ in 0..*power {
                        match piece.checked_mul(&base) {
                            Some(product) => piece = product,
                            None => {
                                overflowed = true;
                                break;
                            }
                        }
                    }
                    if overflowed {
                        break;
                    }
                }
                if overflowed {
                    break;
                }
                match next.checked_add(&piece) {
                    Some(sum) => next = sum,
                    None => {
                        overflowed = true;
                        break;
                    }
                }
            }
            if overflowed {
                return current;
            }
            current = next;
            if !changed {
                break;
            }
        }
        current
    }

    /// Flatten nested `&&` chains into conjunct handles (a single
    /// non-conjunction fact returns itself).
    fn conjuncts(&self, fact: ExpressionHandle) -> Vec<ExpressionHandle> {
        let node = self.program.expression_table.expression(fact).clone();
        match node {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                let mut left = self.conjuncts(binary.left);
                left.extend(self.conjuncts(binary.right));
                left
            }
            ExpressionNode::Mutable(inner) => self.conjuncts(inner),
            _ => vec![fact],
        }
    }

    /// Split a fact into `(comparison operator, left polynomial, right
    /// polynomial)`.
    fn comparison_polynomials(
        &mut self,
        fact: ExpressionHandle,
    ) -> Option<(BinaryOperator, Polynomial, Polynomial)> {
        let node = self.program.expression_table.expression(fact).clone();
        let ExpressionNode::Binary(binary) = node else {
            return None;
        };
        match binary.operator {
            BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual => {
                let left = self.normalize(binary.left)?;
                let right = self.normalize(binary.right)?;
                Some((binary.operator, left, right))
            }
            _ => None,
        }
    }

    /// Normalize a TERM expression to a polynomial. `None` = outside the
    /// engine's language.
    fn normalize(&mut self, expression: ExpressionHandle) -> Option<Polynomial> {
        let node = self.program.expression_table.expression(expression).clone();
        match node {
            ExpressionNode::Integer(value) => Some(Polynomial::constant(value)),
            ExpressionNode::Mutable(inner) => self.normalize(inner),
            ExpressionNode::Name(path) => {
                let members = self.program.expression_table.name_path_members(path.members);
                if members.len() != 1 {
                    return None;
                }
                let name = members[0].as_str().to_owned();
                if !self.parameter_atoms.contains(&name) {
                    return None;
                }
                Some(Polynomial::atom(name))
            }
            // The typed-tree unary operator is logical-not only (negative
            // literals fold into Integer), so unary nodes are never terms.
            ExpressionNode::Unary(_) => None,
            ExpressionNode::Binary(binary) => {
                match binary.operator {
                    BinaryOperator::Add => {
                        let left = self.normalize(binary.left)?;
                        let right = self.normalize(binary.right)?;
                        left.checked_add(&right)
                    }
                    BinaryOperator::Subtract => {
                        let left = self.normalize(binary.left)?;
                        let right = self.normalize(binary.right)?;
                        left.checked_sub(&right)
                    }
                    BinaryOperator::Multiply => {
                        let left = self.normalize(binary.left)?;
                        let right = self.normalize(binary.right)?;
                        left.checked_mul(&right)
                    }
                    BinaryOperator::Modulo => {
                        let operand = self.normalize(binary.left)?;
                        let modulus = self.normalize(binary.right)?.constant_value()?;
                        if modulus <= 0 {
                            return None;
                        }
                        let display = format!(
                            "({}) % {}",
                            polynomial_display(&operand),
                            modulus
                        );
                        self.mod_intervals.insert(
                            display.clone(),
                            Interval {
                                low: Some(0),
                                high: Some(modulus - 1),
                            },
                        );
                        Some(Polynomial::atom(display))
                    }
                    _ => None,
                }
            }
            ExpressionNode::Call(call) => {
                // Proof-view applications are opaque atoms compared by
                // equality only. Anything else is outside the language.
                let target = call.target.as_str();
                if !matches!(target, "Bag" | "Seq" | "Range") {
                    return None;
                }
                if call.receiver.is_valid() {
                    return None;
                }
                let mut rendered = Vec::new();
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec()
                {
                    rendered.push(self.program.expression_table.display_name(argument));
                }
                Some(Polynomial::atom(format!(
                    "{}({})",
                    target,
                    rendered.join(", ")
                )))
            }
            _ => None,
        }
    }
}

fn polynomial_display(polynomial: &Polynomial) -> String {
    let mut parts = Vec::new();
    for (monomial, coefficient) in &polynomial.terms {
        let atoms: Vec<String> = monomial
            .iter()
            .map(|(atom, power)| {
                if *power == 1 {
                    atom.clone()
                } else {
                    format!("{atom}^{power}")
                }
            })
            .collect();
        if atoms.is_empty() {
            parts.push(coefficient.to_string());
        } else if *coefficient == 1 {
            parts.push(atoms.join("*"));
        } else {
            parts.push(format!("{}*{}", coefficient, atoms.join("*")));
        }
    }
    if parts.is_empty() {
        "0".to_owned()
    } else {
        parts.join(" + ")
    }
}
