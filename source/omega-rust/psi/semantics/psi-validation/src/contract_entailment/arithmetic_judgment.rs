use super::*;

pub(super) enum Judgment {
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

/// A polynomial: monomials to EXACT BigInt coefficients (math roster N2:
/// coefficient arithmetic never overflows, so a provable goal never
/// downgrades to "unknown" by width). Zero coefficients are never stored,
/// so structural equality is polynomial identity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Polynomial {
    pub(super) terms: BTreeMap<Monomial, BigInt>,
}

impl Polynomial {
    pub(super) fn constant(value: BigInt) -> Self {
        let mut polynomial = Self::default();
        if !value.is_zero() {
            polynomial.terms.insert(Monomial::new(), value);
        }
        polynomial
    }

    pub(super) fn atom(name: String) -> Self {
        let mut monomial = Monomial::new();
        monomial.insert(name, 1);
        let mut polynomial = Self::default();
        polynomial.terms.insert(monomial, BigInt::from_i64(1));
        polynomial
    }

    fn constant_value(&self) -> Option<BigInt> {
        match self.terms.len() {
            0 => Some(BigInt::zero()),
            1 => self.terms.get(&Monomial::new()).cloned(),
            _ => None,
        }
    }

    pub(super) fn add(&self, other: &Self) -> Self {
        let mut terms = self.terms.clone();
        for (monomial, coefficient) in &other.terms {
            let entry = terms.entry(monomial.clone()).or_insert_with(BigInt::zero);
            *entry = entry.add(coefficient);
            if entry.is_zero() {
                terms.remove(monomial);
            }
        }
        Self { terms }
    }

    fn neg(&self) -> Self {
        let mut terms = BTreeMap::new();
        for (monomial, coefficient) in &self.terms {
            terms.insert(monomial.clone(), coefficient.negate());
        }
        Self { terms }
    }

    pub(super) fn sub(&self, other: &Self) -> Self {
        self.add(&other.neg())
    }

    /// Coefficients are exact; the only remaining failure is monomial POWER
    /// overflow (u32), which no writable program reaches.
    pub(super) fn checked_mul(&self, other: &Self) -> Option<Self> {
        let mut result = Self::default();
        for (left_monomial, left_coefficient) in &self.terms {
            for (right_monomial, right_coefficient) in &other.terms {
                let coefficient = left_coefficient.mul(right_coefficient);
                let mut monomial = left_monomial.clone();
                for (atom, power) in right_monomial {
                    let entry = monomial.entry(atom.clone()).or_insert(0);
                    *entry = entry.checked_add(*power)?;
                }
                let entry = result
                    .terms
                    .entry(monomial.clone())
                    .or_insert_with(BigInt::zero);
                *entry = entry.add(&coefficient);
                if entry.is_zero() {
                    result.terms.remove(&monomial);
                }
            }
        }
        Some(result)
    }

    /// `(difference-of-two-unit-atoms, constant)`: `a - b + c` as
    /// `Some((a, b, c))`. The shape the difference-bound matrix consumes.
    fn as_atom_difference(&self) -> Option<(String, String, BigInt)> {
        let mut positive = None;
        let mut negative = None;
        let mut constant = BigInt::zero();
        for (monomial, coefficient) in &self.terms {
            if monomial.is_empty() {
                constant = coefficient.clone();
                continue;
            }
            if monomial.len() != 1 || *monomial.values().next().unwrap() != 1 {
                return None;
            }
            let atom = monomial.keys().next().unwrap().clone();
            if *coefficient == BigInt::from_i64(1) && positive.is_none() {
                positive = Some(atom);
            } else if *coefficient == BigInt::from_i64(-1) && negative.is_none() {
                negative = Some(atom);
            } else {
                return None;
            }
        }
        Some((positive?, negative?, constant))
    }

    /// `(single-unit-atom, coefficient-sign, constant)` for bounds like
    /// `a + c >= 0` / `-a + c >= 0`.
    fn as_single_atom(&self) -> Option<(String, i64, BigInt)> {
        let mut atom = None;
        let mut coefficient_value = BigInt::zero();
        let mut constant = BigInt::zero();
        for (monomial, coefficient) in &self.terms {
            if monomial.is_empty() {
                constant = coefficient.clone();
                continue;
            }
            if monomial.len() != 1 || *monomial.values().next().unwrap() != 1 || atom.is_some() {
                return None;
            }
            atom = Some(monomial.keys().next().unwrap().clone());
            coefficient_value = coefficient.clone();
        }
        let atom = atom?;
        let sign = if coefficient_value == BigInt::from_i64(1) {
            1
        } else if coefficient_value == BigInt::from_i64(-1) {
            -1
        } else {
            return None;
        };
        Some((atom, sign, constant))
    }
}

/// An interval with optional (= unbounded) ends; end arithmetic is exact.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Interval {
    low: Option<BigInt>,
    high: Option<BigInt>,
}

impl Interval {
    fn unbounded() -> Self {
        Self {
            low: None,
            high: None,
        }
    }

    pub(super) fn constant(value: BigInt) -> Self {
        Self {
            low: Some(value.clone()),
            high: Some(value),
        }
    }

    pub(super) fn add(&self, other: &Self) -> Self {
        Self {
            low: match (&self.low, &other.low) {
                (Some(a), Some(b)) => Some(a.add(b)),
                _ => None,
            },
            high: match (&self.high, &other.high) {
                (Some(a), Some(b)) => Some(a.add(b)),
                _ => None,
            },
        }
    }

    fn scale(&self, factor: &BigInt) -> Self {
        let scaled_low = self.low.as_ref().map(|value| value.mul(factor));
        let scaled_high = self.high.as_ref().map(|value| value.mul(factor));
        if factor.is_negative() {
            Self {
                low: scaled_high,
                high: scaled_low,
            }
        } else {
            Self {
                low: scaled_low,
                high: scaled_high,
            }
        }
    }

    fn multiply(&self, other: &Self) -> Self {
        // An unbounded end makes the product unbounded on the side it could
        // extend; with all four ends finite the corner products are exact.
        let (Some(self_low), Some(self_high), Some(other_low), Some(other_high)) =
            (&self.low, &self.high, &other.low, &other.high)
        else {
            return Interval::unbounded();
        };
        let candidates = [
            self_low.mul(other_low),
            self_low.mul(other_high),
            self_high.mul(other_low),
            self_high.mul(other_high),
        ];
        Self {
            low: candidates.iter().min().cloned(),
            high: candidates.iter().max().cloned(),
        }
    }

    /// `self` raised to `power`, treating repeated factors as CORRELATED:
    /// an even power of any interval is non-negative, and the square of
    /// `[lo, hi]` is exact rather than the independent product.
    fn correlated_power(&self, power: u32) -> Self {
        if power == 0 {
            return Self::constant(BigInt::from_i64(1));
        }
        if power == 1 {
            return self.clone();
        }
        let (Some(low), Some(high)) = (&self.low, &self.high) else {
            // Unbounded base: an even power is still known non-negative.
            return if power.is_multiple_of(2) {
                Self {
                    low: Some(BigInt::zero()),
                    high: None,
                }
            } else {
                Interval::unbounded()
            };
        };
        let corner_low = pow(low, power);
        let corner_high = pow(high, power);
        if power % 2 == 1 {
            return Self {
                low: Some(corner_low),
                high: Some(corner_high),
            };
        }
        let max_corner = corner_low.clone().max(corner_high.clone());
        let min_corner = if !low.is_negative() || high.is_negative() {
            corner_low.min(corner_high)
        } else {
            // The base interval straddles zero: the even power bottoms at 0.
            BigInt::zero()
        };
        Self {
            low: Some(min_corner),
            high: Some(max_corner),
        }
    }
}

fn pow(base: &BigInt, power: u32) -> BigInt {
    let mut result = BigInt::from_i64(1);
    for _ in 0..power {
        result = result.mul(base);
    }
    result
}

pub(super) struct Engine<'program> {
    pub(super) program: &'program TypedTrees,
    /// The machine this engine judges (entry-range hypotheses resolve
    /// through it).
    machine_symbol: SymbolHandle,
    /// Canonical atom names for the machine's parameters.
    pub(super) parameter_atoms: Vec<String>,
    /// Authority-bearing adapters bind resolved symbols directly. When this
    /// is `Some`, an unbound name is outside the language rather than falling
    /// back to its display spelling.
    strict_symbol_bindings: Option<Vec<(SymbolHandle, Polynomial)>>,
    strict_symbol_bindings_valid: bool,
    /// Parameters whose primitive type is unsigned carry an implicit `>= 0`.
    unsigned_atoms: Vec<String>,
    /// Directed substitutions from requires equations (`atom := polynomial`),
    /// applied to fixpoint during normalization.
    pub(super) substitutions: BTreeMap<String, Polynomial>,
    /// Lower bounds: each entry means `polynomial >= bound`.
    bounds: Vec<(Polynomial, BigInt)>,
    /// Mod-term atoms with their euclidean intervals (`t % k` in `0 ..= k-1`).
    mod_intervals: BTreeMap<String, Interval>,
    /// Difference-bound matrix over atoms + the virtual ZERO atom:
    /// `matrix[a][b]` = best known lower bound of `a - b`.
    matrix: BTreeMap<String, BTreeMap<String, BigInt>>,
    pub(super) requires_unsatisfiable: bool,
}

const ZERO_ATOM: &str = "\u{0}zero";
const SUBSTITUTION_ROUNDS: usize = 8;

impl<'program> Engine<'program> {
    pub(super) fn new(program: &'program TypedTrees, machine: &Machine) -> Self {
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
                    if let Some(primitive) = primitive
                        && !primitive.is_signed_integer()
                    {
                        unsigned_atoms.push(name.clone());
                    }
                    parameter_atoms.push(name);
                }
            }
        }
        Self {
            program,
            machine_symbol: machine.symbol,
            parameter_atoms,
            strict_symbol_bindings: None,
            strict_symbol_bindings_valid: true,
            unsigned_atoms,
            substitutions: BTreeMap::new(),
            bounds: Vec::new(),
            mod_intervals: BTreeMap::new(),
            matrix: BTreeMap::new(),
            requires_unsatisfiable: false,
        }
    }

    pub(super) fn strict_with_symbol_bindings(
        program: &'program TypedTrees,
        machine: &Machine,
        bindings: &[StrictArithmeticSymbolBinding],
    ) -> Self {
        let mut engine = Self::new(program, machine);
        engine.parameter_atoms.clear();
        engine.unsigned_atoms.clear();
        let mut resolved = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let polynomial = match &binding.value {
                StrictArithmeticBindingValue::Atom { identity, unsigned } => {
                    if !engine.parameter_atoms.contains(identity) {
                        engine.parameter_atoms.push(identity.clone());
                    }
                    if *unsigned && !engine.unsigned_atoms.contains(identity) {
                        engine.unsigned_atoms.push(identity.clone());
                    }
                    Polynomial::atom(identity.clone())
                }
                StrictArithmeticBindingValue::Integer(value) => Polynomial::constant(value.clone()),
            };
            if let Some((_, existing)) = resolved
                .iter()
                .find(|(symbol, _)| *symbol == binding.symbol)
            {
                if existing != &polynomial {
                    // A contradictory binding set must invalidate even a
                    // constant-only goal, not degrade to an empty table.
                    engine.strict_symbol_bindings_valid = false;
                    return engine;
                }
                continue;
            }
            resolved.push((binding.symbol, polynomial));
        }
        engine.strict_symbol_bindings = Some(resolved);
        engine
    }

    pub(super) fn strict_symbol_bindings_are_valid(&self) -> bool {
        self.strict_symbol_bindings_valid
    }

    /// Like [`Engine::new`], plus the reserved `result` atom for the
    /// machine's return value (unless a real parameter shadows it, matching
    /// the call-site binder rule). Used by the inductive transition path,
    /// where each arm binds or shares `result`.
    pub(super) fn with_result_atom(
        program: &'program TypedTrees,
        machine: &Machine,
        root: &psi_typed_trees::state::State,
    ) -> Self {
        let mut engine = Self::new(program, machine);
        let shadowed = program
            .state_parameters(root)
            .iter()
            .any(|parameter| !parameter.is_self && parameter.name.as_str() == RESULT_BINDER);
        if !shadowed {
            engine.parameter_atoms.push(RESULT_BINDER.to_owned());
            if root.return_type.is_valid()
                && let Some(primitive) = program
                    .type_reference_table
                    .primitive_type(root.return_type)
                && !primitive.is_signed_integer()
            {
                engine.unsigned_atoms.push(RESULT_BINDER.to_owned());
            }
        }
        engine
    }

    /// Load the requires facts. Returns whether EVERY fact was inside the
    /// engine's language (full visibility is the precondition for rejecting
    /// unproven ensures). The ENTRY-state parameters' declared bracket
    /// ranges join as hypotheses too -- R1's bracket-as-sugar rule (ch12:
    /// `k: u64 [0..=8]` IS `requires k >= 0 && k <= 8`; the range is
    /// caller-discharged, so the callee's contract proofs may assume it).
    pub(super) fn add_requires(&mut self, facts: &[ExpressionHandle]) -> bool {
        let mut comparisons = Vec::new();
        let mut fully_visible = self.collect_comparisons(facts, &mut comparisons);
        self.collect_entry_range_hypotheses(&mut comparisons);
        fully_visible &= self.install_hypotheses(comparisons);
        fully_visible
    }

    /// The bracket-as-sugar hypotheses: for each ENTRY-state (machine
    /// signature) parameter whose type carries a LITERAL `[a..=b]` range,
    /// push `param >= a` and `param <= b`. Entry-only: sub-state params are
    /// different binders that may reuse names.
    pub(super) fn collect_entry_range_hypotheses(
        &mut self,
        comparisons: &mut Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) {
        let Some(machine) = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == self.machine_symbol)
        else {
            return;
        };
        let Some(entry) = self.program.machine_states(machine).first() else {
            return;
        };
        for parameter in self.program.state_parameters(entry) {
            if parameter.is_self {
                continue;
            }
            let Some(interval) = crate::arithmetic_domains::range_constraint_interval(
                self.program,
                parameter.type_reference,
            ) else {
                continue;
            };
            let atom = Polynomial::atom(parameter.name.as_str().to_owned());
            if let Some(low) = interval.low() {
                comparisons.push((
                    BinaryOperator::GreaterOrEqual,
                    atom.clone(),
                    Polynomial::constant(BigInt::from_i64(low)),
                ));
            }
            if let Some(high) = interval.high() {
                comparisons.push((
                    BinaryOperator::LessOrEqual,
                    atom.clone(),
                    Polynomial::constant(BigInt::from_i64(high)),
                ));
            }
        }
    }

    /// First ingestion pass: split facts into conjuncts and normalize each to
    /// a comparison triple. Range membership lowers to `&&` chains
    /// (`x in 1..=10` arrives as `(x >= 1) && (x <= 10)`), so facts split
    /// into conjuncts first. Returns whether every conjunct was readable.
    pub(super) fn collect_comparisons(
        &mut self,
        facts: &[ExpressionHandle],
        comparisons: &mut Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) -> bool {
        let mut fully_visible = true;
        for fact in facts {
            for conjunct in self.conjuncts(*fact) {
                match self.comparison_polynomials(conjunct) {
                    Some(comparison) => comparisons.push(comparison),
                    None => fully_visible = false,
                }
            }
        }
        fully_visible
    }

    /// Second ingestion pass: harvest substitutions from equations so every
    /// later normalization sees them, store lower bounds, then seed and close
    /// the difference-bound matrix. Returns whether every hypothesis
    /// installed without arithmetic overflow.
    pub(super) fn install_hypotheses(
        &mut self,
        comparisons: Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) -> bool {
        let mut fully_visible = true;
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
            let difference_rl = right.sub(&left);
            let difference_lr = left.sub(&right);
            match operator {
                BinaryOperator::Less => lower_bounds.push((difference_rl, BigInt::from_i64(1))),
                BinaryOperator::LessOrEqual => lower_bounds.push((difference_rl, BigInt::zero())),
                BinaryOperator::Greater => lower_bounds.push((difference_lr, BigInt::from_i64(1))),
                BinaryOperator::GreaterOrEqual => {
                    lower_bounds.push((difference_lr, BigInt::zero()))
                }
                BinaryOperator::Equal => {
                    lower_bounds.push((difference_rl, BigInt::zero()));
                    lower_bounds.push((difference_lr, BigInt::zero()));
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
    pub(super) fn judge(&mut self, fact: ExpressionHandle) -> Judgment {
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
        let difference_rl = right.sub(&left);
        let difference_lr = left.sub(&right);

        // Constant fold first: it gives the crispest diagnostic.
        if let Some(value) = difference_rl.constant_value() {
            let holds = match operator {
                BinaryOperator::Less => !value.is_negative() && !value.is_zero(),
                BinaryOperator::LessOrEqual => !value.is_negative(),
                BinaryOperator::Greater => value.is_negative(),
                BinaryOperator::GreaterOrEqual => value.is_negative() || value.is_zero(),
                BinaryOperator::Equal => value.is_zero(),
                BinaryOperator::NotEqual => !value.is_zero(),
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

        let zero = BigInt::zero();
        let one = BigInt::from_i64(1);
        let proved = match operator {
            BinaryOperator::Less => self.prove_at_least(&difference_rl, &one),
            BinaryOperator::LessOrEqual => self.prove_at_least(&difference_rl, &zero),
            BinaryOperator::Greater => self.prove_at_least(&difference_lr, &one),
            BinaryOperator::GreaterOrEqual => self.prove_at_least(&difference_lr, &zero),
            BinaryOperator::Equal => {
                self.prove_at_least(&difference_rl, &zero)
                    && self.prove_at_least(&difference_lr, &zero)
            }
            BinaryOperator::NotEqual => {
                self.prove_at_least(&difference_rl, &one)
                    || self.prove_at_least(&difference_lr, &one)
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
            BinaryOperator::Less => self.prove_at_least(&difference_lr, &zero),
            BinaryOperator::LessOrEqual => self.prove_at_least(&difference_lr, &one),
            BinaryOperator::Greater => self.prove_at_least(&difference_rl, &zero),
            BinaryOperator::GreaterOrEqual => self.prove_at_least(&difference_rl, &one),
            BinaryOperator::Equal => {
                self.prove_at_least(&difference_rl, &one)
                    || self.prove_at_least(&difference_lr, &one)
            }
            BinaryOperator::NotEqual => {
                self.prove_at_least(&difference_rl, &zero)
                    && self.prove_at_least(&difference_lr, &zero)
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
    pub(super) fn prove_at_least(&self, polynomial: &Polynomial, bound: &BigInt) -> bool {
        if let Some((positive, negative, constant)) = polynomial.as_atom_difference()
            && let Some(best) = self.matrix_bound(&positive, &negative)
            && best.add(&constant) >= *bound
        {
            return true;
        }
        if let Some((atom, sign, constant)) = polynomial.as_single_atom() {
            let other = if sign == 1 {
                self.matrix_bound(&atom, ZERO_ATOM)
            } else {
                self.matrix_bound(ZERO_ATOM, &atom)
            };
            if let Some(best) = other
                && best.add(&constant) >= *bound
            {
                return true;
            }
        }
        // A stored hypothesis bound whose polynomial IS the goal polynomial
        // subsumes it directly. This is the shape induction hypotheses
        // arrive in: general polynomial equations (e.g. `2*result - P >= 0`)
        // that fit neither the difference-bound matrix nor the interval
        // evaluator, but whose canonical form matches the goal exactly.
        for (stored, stored_bound) in &self.bounds {
            if stored == polynomial && stored_bound >= bound {
                return true;
            }
        }
        if let Some(low) = self.polynomial_interval(polynomial).low
            && low >= *bound
        {
            return true;
        }
        false
    }

    fn polynomial_interval(&self, polynomial: &Polynomial) -> Interval {
        let mut total = Interval::constant(BigInt::zero());
        for (monomial, coefficient) in &polynomial.terms {
            let mut product = Interval::constant(BigInt::from_i64(1));
            for (atom, power) in monomial {
                let base = self.atom_interval(atom);
                product = product.multiply(&base.correlated_power(*power));
            }
            total = total.add(&product.scale(coefficient));
        }
        total
    }

    fn atom_interval(&self, atom: &str) -> Interval {
        if let Some(interval) = self.mod_intervals.get(atom) {
            return interval.clone();
        }
        Interval {
            low: self.matrix_bound(atom, ZERO_ATOM),
            high: self
                .matrix_bound(ZERO_ATOM, atom)
                .map(|bound| bound.negate()),
        }
    }

    fn matrix_bound(&self, from: &str, to: &str) -> Option<BigInt> {
        if from == to {
            return Some(BigInt::zero());
        }
        self.matrix.get(from).and_then(|row| row.get(to)).cloned()
    }

    fn seed_matrix(&mut self) {
        for atom in self.unsigned_atoms.clone() {
            self.record_difference(&atom, ZERO_ATOM, BigInt::zero());
        }
        let mod_atoms: Vec<(String, Interval)> = self
            .mod_intervals
            .iter()
            .map(|(atom, interval)| (atom.clone(), interval.clone()))
            .collect();
        for (atom, interval) in mod_atoms {
            if let Some(low) = interval.low {
                self.record_difference(&atom, ZERO_ATOM, low);
            }
            if let Some(high) = interval.high {
                self.record_difference(ZERO_ATOM, &atom, high.negate());
            }
        }
        for (polynomial, bound) in self.bounds.clone() {
            if let Some((positive, negative, constant)) = polynomial.as_atom_difference() {
                self.record_difference(&positive, &negative, bound.sub(&constant));
            }
            if let Some((atom, sign, constant)) = polynomial.as_single_atom() {
                let edge = bound.sub(&constant);
                if sign == 1 {
                    self.record_difference(&atom, ZERO_ATOM, edge);
                } else {
                    self.record_difference(ZERO_ATOM, &atom, edge);
                }
            }
        }
    }

    fn record_difference(&mut self, from: &str, to: &str, bound: BigInt) {
        let row = self.matrix.entry(from.to_owned()).or_default();
        match row.entry(to.to_owned()) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(bound);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                if bound > *slot.get() {
                    slot.insert(bound);
                }
            }
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
                    let combined = first.add(&second);
                    if from == to {
                        if !combined.is_negative() && !combined.is_zero() {
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
            if let Some((atom, 1, constant)) = candidate.as_single_atom()
                && constant.is_zero()
            {
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

    pub(super) fn substituted(&self, polynomial: &Polynomial) -> Polynomial {
        let mut current = polynomial.clone();
        for _ in 0..SUBSTITUTION_ROUNDS {
            let mut changed = false;
            let mut next = Polynomial::default();
            let mut overflowed = false;
            for (monomial, coefficient) in &current.terms {
                let mut piece = Polynomial::constant(coefficient.clone());
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
                next = next.add(&piece);
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
    pub(super) fn conjuncts(&self, fact: ExpressionHandle) -> Vec<ExpressionHandle> {
        let node = self.program.expression_table.expression(fact).clone();
        match node {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                let mut left = self.conjuncts(binary.left);
                left.extend(self.conjuncts(binary.right));
                left
            }
            ExpressionNode::Borrow(inner) => self.conjuncts(inner.target),
            _ => vec![fact],
        }
    }

    /// Split a fact into `(comparison operator, left polynomial, right
    /// polynomial)`.
    pub(super) fn comparison_polynomials(
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
    pub(super) fn normalize(&mut self, expression: ExpressionHandle) -> Option<Polynomial> {
        let node = self.program.expression_table.expression(expression).clone();
        match node {
            ExpressionNode::Integer(value) => Some(Polynomial::constant(value.value_bignum()?)),
            ExpressionNode::Borrow(inner) => self.normalize(inner.target),
            ExpressionNode::Name(path) => {
                if let Some(bindings) = &self.strict_symbol_bindings {
                    if self
                        .program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        != 1
                    {
                        return None;
                    }
                    return bindings.iter().find_map(|(symbol, value)| {
                        (*symbol == path.symbol).then(|| value.clone())
                    });
                }
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
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
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::Add => {
                    let left = self.normalize(binary.left)?;
                    let right = self.normalize(binary.right)?;
                    Some(left.add(&right))
                }
                BinaryOperator::Subtract => {
                    let left = self.normalize(binary.left)?;
                    let right = self.normalize(binary.right)?;
                    Some(left.sub(&right))
                }
                BinaryOperator::Multiply => {
                    let left = self.normalize(binary.left)?;
                    let right = self.normalize(binary.right)?;
                    left.checked_mul(&right)
                }
                BinaryOperator::Modulo => {
                    let operand = self.normalize(binary.left)?;
                    let modulus = self.normalize(binary.right)?.constant_value()?;
                    if modulus.is_negative() || modulus.is_zero() {
                        return None;
                    }
                    let display = format!("({}) % {}", polynomial_display(&operand), modulus);
                    self.mod_intervals.insert(
                        display.clone(),
                        Interval {
                            low: Some(BigInt::zero()),
                            high: Some(modulus.sub(&BigInt::from_i64(1))),
                        },
                    );
                    Some(Polynomial::atom(display))
                }
                _ => None,
            },
            ExpressionNode::Call(call) => {
                if self.strict_symbol_bindings.is_some() {
                    return None;
                }
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
        } else if *coefficient == BigInt::from_i64(1) {
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
