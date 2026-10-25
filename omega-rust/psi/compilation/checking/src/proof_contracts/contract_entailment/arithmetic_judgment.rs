use super::{
    BTreeMap, BigInt, BinaryOperator, ExpressionHandle, ExpressionNode, Machine, RESULT_BINDER,
    SymbolHandle, proof_integer_expression, proof_nat_cast,
};
mod difference_matrix;
mod integer_operations;
mod intervals;
mod polynomial_bounds;
use crate::proof_contracts::arithmetic_domains::BigIntInterval as Interval;
mod captured_calls;
mod mathematical_terms;
mod strict_bindings;
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

    pub(super) fn constant_value(&self) -> Option<BigInt> {
        match self.terms.len() {
            0 => Some(BigInt::zero()),
            1 => self.terms.get(&Monomial::new()).cloned(),
            _ => None,
        }
    }

    /// Every nonconstant monomial is integral in an integer-only scope. Its
    /// coefficient gcd divides their sum, so an incompatible constant residue
    /// excludes zero without choosing a sign or enumerating atom values.
    fn excludes_integer_zero(&self) -> bool {
        let mut divisor = BigInt::zero();
        let mut constant = BigInt::zero();
        for (monomial, coefficient) in &self.terms {
            if monomial.is_empty() {
                constant = coefficient.clone();
            } else {
                divisor = divisor.gcd(coefficient);
            }
        }
        if divisor.is_zero() {
            return !constant.is_zero();
        }
        constant
            .div_rem(&divisor)
            .is_some_and(|(_, remainder)| !remainder.is_zero())
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

/// The operands one opaque atom was minted from, retained so a later
/// simultaneous substitution can re-mint the same shape under the
/// transported operands rather than dropping the term.
#[derive(Clone)]
pub(super) enum OpaqueTerm {
    /// A completed finite-carrier value, distinct from its unbounded
    /// mathematical calculation. Wrapping retains its residue polynomial;
    /// saturation retains the calculation to clamp after substitution.
    IntegerPolicy {
        primitive: typed_trees::types::PrimitiveType,
        domain: numerics::arithmetic::ArithmeticDomain,
        value: Polynomial,
    },
    /// A truncating remainder keeps both inputs, including a runtime modulus;
    /// equal result intervals do not establish conserved endpoint identity.
    Remainder {
        operand: Polynomial,
        modulus: Polynomial,
        /// The mint site's provenance gate for the single-quotient tight
        /// interval; re-mints reuse it rather than re-deriving it.
        tight_interval: bool,
    },
    /// A truncating quotient retains both inputs, not just the dividend:
    /// a runtime divisor is an independently transported value too.
    Quotient {
        dividend: Polynomial,
        divisor: Polynomial,
    },
    /// An exact left shift keeps both inputs and the shifted carrier's bit
    /// width: the count's defined range `[0, count_bits)` bounds the factor,
    /// and re-mints reuse the mint site's width rather than re-deriving it.
    ShiftLeft {
        value: Polynomial,
        count: Polynomial,
        count_bits: u32,
    },
    /// An exact right shift keeps the same inputs and width: the result is
    /// `floor(value / 2^count)`, arithmetic (sign-filling) for a signed
    /// value and logical for an unsigned one, so it never escapes the
    /// shifted carrier. The F8 count obligation is identical to `<<`'s.
    ShiftRight {
        value: Polynomial,
        count: Polynomial,
        count_bits: u32,
    },
    /// An integer bitwise operation keeps its operator, both inputs and the
    /// shared carrier's shape (`bits` wide, two's complement when `signed`). The operation
    /// is total -- every operand pairing lands inside that carrier -- so
    /// the term carries no divisor/count-style side condition, only the
    /// shape its interval reads sign corners from.
    Bitwise {
        operator: BinaryOperator,
        left: Polynomial,
        right: Polynomial,
        bits: u32,
        signed: bool,
    },
}

/// Conditional facts of a completed Boolean observation. Keeping normalized
/// operands, rather than its initializer, prevents later scope substitution
/// from replaying the comparison against different values.
pub(super) struct SavedCondition {
    pub(super) symbol: SymbolHandle,
    pub(super) holds: bool,
    pub(super) comparisons: Vec<(BinaryOperator, Polynomial, Polynomial)>,
    pub(super) fully_visible: bool,
}

pub(super) struct Engine<'program> {
    pub(super) program: &'program dyn crate::CheckingInput,
    /// The machine this engine judges (entry-range hypotheses resolve
    /// through it).
    machine_symbol: SymbolHandle,
    /// Source declaration for the currently installed scoped expression.
    /// Nested actual terms retain their own owner before formal substitution.
    source_owner: SymbolHandle,
    /// Canonical atom names for the machine's parameters.
    pub(super) parameter_atoms: Vec<String>,
    /// Authority-bearing adapters bind resolved symbols directly. When this
    /// is `Some`, an unbound name is outside the language rather than falling
    /// back to its display spelling.
    strict_symbol_bindings: Option<Vec<(SymbolHandle, Polynomial)>>,
    /// A callee telescope denotes completed caller values before any operation
    /// simplifies. This overlays symbol identity, not arithmetic capabilities.
    formal_arguments: Option<Vec<(SymbolHandle, Polynomial)>>,
    strict_symbol_bindings_valid: bool,
    /// Only the exact-meaning call adapter licenses widening while capturing
    /// actuals. Other strict readers (ranges and saved locals) have not checked
    /// operand arithmetic and must retain their existing cast refusal.
    /// The synthetic guarantee `result` has no resolved symbol. A scoped
    /// roster may bind it to the exact term an exit returns; without a
    /// binding the name stays outside a strict engine's language.
    strict_result_binding: Option<Polynomial>,
    /// Exact numeric projections and quotient occurrences installed by the
    /// owning strict query after checking builtin meaning and source custody.
    strict_occurrences: Vec<(ExpressionHandle, Polynomial)>,
    pub(super) saved_conditions: Vec<SavedCondition>,
    /// One call-observation namespace for this judgment. Reusing it prevents
    /// a shared public-clause expression from identifying different invocations
    /// merely because each operand was prepared first in a fresh namespace.
    captured_calls: Option<super::scoped_arithmetic::calls::ScopedCalls<'program>>,
    /// A formation query has exact symbol bindings and admits only total
    /// mathematical arithmetic. It must not inherit the legacy proof
    /// machine adapter's interpretation of arbitrary executable operators.
    proof_integer_formation: bool,
    /// Parameters whose primitive type is unsigned carry an implicit `>= 0`.
    unsigned_atoms: Vec<String>,
    /// Directed substitutions from requires equations (`atom := polynomial`),
    /// applied to fixpoint during normalization.
    pub(super) substitutions: BTreeMap<String, Polynomial>,
    /// Lower bounds: each entry means `polynomial >= bound`.
    bounds: Vec<(Polynomial, BigInt)>,
    /// Exact integer disequalities retained by strict adapters. A nonzero
    /// difference has no sign; it can establish disequality, not an order bound.
    nonzero_differences: Vec<Polynomial>,
    /// Derived bounds for truncating integer quotient, remainder, and shift
    /// atoms.
    arithmetic_intervals: BTreeMap<String, Interval>,
    /// Opaque quotient/remainder/shift atoms this engine minted, in mint
    /// order with each term's operands. An outer term's operand atoms were
    /// necessarily minted earlier, so one ordered pass transports nested
    /// terms innermost-first.
    opaque_terms: Vec<(String, OpaqueTerm)>,
    /// Difference-bound matrix over atoms + the virtual ZERO atom:
    /// `matrix[a][b]` = best known lower bound of `a - b`.
    matrix: BTreeMap<String, BTreeMap<String, BigInt>>,
    pub(super) requires_unsatisfiable: bool,
}

const ZERO_ATOM: &str = "\u{0}zero";
const SUBSTITUTION_ROUNDS: usize = 8;

impl<'program> Engine<'program> {
    pub(super) fn new(program: &'program dyn crate::CheckingInput, machine: &Machine) -> Self {
        let mut engine = Self::for_owner(program, machine.symbol, None);
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                let name = parameter.name.as_str().to_owned();
                if engine.parameter_atoms.contains(&name) {
                    continue;
                }
                let unsigned = unsigned_parameter_carrier(program, parameter.type_reference);
                engine.declare_atom(&name, unsigned);
            }
        }
        engine
    }

    /// The one Engine field roster. `new` and `for_explicit_bindings` differ
    /// only in their owner symbols, declared atoms and whether unbound names
    /// stay outside the language, so the roster lives here once: a new engine
    /// field is initialized in one place instead of once per constructor.
    fn for_owner(
        program: &'program dyn crate::CheckingInput,
        owner: SymbolHandle,
        strict_symbol_bindings: Option<Vec<(SymbolHandle, Polynomial)>>,
    ) -> Self {
        Self {
            program,
            machine_symbol: owner,
            source_owner: owner,
            parameter_atoms: Vec::new(),
            strict_symbol_bindings,
            formal_arguments: None,
            strict_symbol_bindings_valid: true,
            strict_result_binding: None,
            strict_occurrences: Vec::new(),
            saved_conditions: Vec::new(),
            captured_calls: None,
            proof_integer_formation: false,
            unsigned_atoms: Vec::new(),
            substitutions: BTreeMap::new(),
            bounds: Vec::new(),
            nonzero_differences: Vec::new(),
            arithmetic_intervals: BTreeMap::new(),
            opaque_terms: Vec::new(),
            matrix: BTreeMap::new(),
            requires_unsatisfiable: false,
        }
    }

    /// Declare one atom identity: every declared atom is a parameter atom, and
    /// an unsigned carrier's atom additionally carries the implicit `>= 0` the
    /// matrix seeds on the next hypothesis installation. Entry parameters, the
    /// reserved `result` binder, strict bindings and scoped rosters all declare
    /// through this one rule.
    fn declare_atom(&mut self, identity: &str, unsigned: bool) {
        if !self.parameter_atoms.iter().any(|atom| atom == identity) {
            self.parameter_atoms.push(identity.to_owned());
        }
        if unsigned && !self.unsigned_atoms.iter().any(|atom| atom == identity) {
            self.unsigned_atoms.push(identity.to_owned());
        }
    }
    /// Range queries mint endpoint terms before installing their hypotheses.
    /// Recompute dependent intervals in mint order once those facts are live:
    /// an inner quotient must tighten before an outer quotient reads it.
    pub(super) fn refresh_opaque_intervals(&mut self) {
        if self.opaque_terms.is_empty() {
            return;
        }
        for position in 0..self.opaque_terms.len() {
            let (atom, term) = self.opaque_terms[position].clone();
            let operation = match &term {
                OpaqueTerm::Quotient { dividend, divisor } => {
                    Some((BinaryOperator::Divide, dividend.clone(), divisor.clone()))
                }
                OpaqueTerm::Remainder {
                    operand, modulus, ..
                } => Some((BinaryOperator::Modulo, operand.clone(), modulus.clone())),
                _ => None,
            };
            let interval = match term {
                OpaqueTerm::IntegerPolicy {
                    primitive,
                    domain,
                    value,
                } => self.integer_policy_interval(primitive, domain, &value),
                OpaqueTerm::Quotient { dividend, divisor } => {
                    self.quotient_interval(&dividend, &divisor)
                }
                OpaqueTerm::Remainder {
                    operand,
                    modulus,
                    tight_interval,
                } => self.remainder_interval(&operand, &modulus, tight_interval),
                OpaqueTerm::ShiftLeft {
                    value,
                    count,
                    count_bits,
                } => self.shift_left_interval(&value, &count, count_bits),
                OpaqueTerm::ShiftRight {
                    value,
                    count,
                    count_bits,
                } => self.shift_right_interval(&value, &count, count_bits),
                OpaqueTerm::Bitwise {
                    operator,
                    left,
                    right,
                    bits,
                    signed,
                } => self.bitwise_interval(operator, &left, &right, bits, signed),
            };
            self.arithmetic_intervals.insert(atom.clone(), interval);
            if let Some((operator, left, right)) = operation {
                self.install_integer_operation_bounds(
                    operator,
                    &left,
                    &right,
                    &Polynomial::atom(atom),
                );
            }
        }
        self.seed_matrix();
        self.close_matrix();
    }

    /// Both operands mapped through the argument map together — `None` when
    /// either substitution declines.
    fn mapped_polynomials(
        a: &Polynomial,
        b: &Polynomial,
        argument_map: &mut BTreeMap<String, Polynomial>,
    ) -> Option<(Polynomial, Polynomial)> {
        Some((
            super::inductive_judgment::apply_argument_map(a, argument_map)?,
            super::inductive_judgment::apply_argument_map(b, argument_map)?,
        ))
    }

    /// Extend a simultaneous argument map across this engine's opaque atoms:
    /// a remainder, quotient, or shift atom whose operands substitute
    /// completely is re-minted under the transported inputs and mapped to
    /// that fresh atom,
    /// its interval registered like any minted term. An operand whose leaf
    /// the map does not cover keeps its atom unmapped, so the consuming
    /// substitution still fails closed on it. Mint order transports an inner
    /// atom before every outer term that embeds it.
    pub(super) fn extend_argument_map_over_opaque_terms(
        &mut self,
        argument_map: &mut BTreeMap<String, Polynomial>,
    ) {
        for index in 0..self.opaque_terms.len() {
            let (atom, term) = self.opaque_terms[index].clone();
            let (next_atom, interval) = match term {
                OpaqueTerm::IntegerPolicy {
                    primitive,
                    domain,
                    value,
                } => {
                    let Some(value) =
                        super::inductive_judgment::apply_argument_map(&value, argument_map)
                    else {
                        continue;
                    };
                    let Some(value) = self.integer_policy_value(primitive, domain, value) else {
                        continue;
                    };
                    argument_map.insert(atom, value);
                    continue;
                }
                OpaqueTerm::Remainder {
                    operand,
                    modulus,
                    tight_interval,
                } => {
                    let Some((operand, modulus)) =
                        Self::mapped_polynomials(&operand, &modulus, argument_map)
                    else {
                        continue;
                    };
                    let next = format!("\0integer-remainder:{operand:?}%{modulus:?}");
                    (
                        next,
                        self.remainder_interval(&operand, &modulus, tight_interval),
                    )
                }
                OpaqueTerm::Quotient { dividend, divisor } => {
                    let Some((dividend, divisor)) =
                        Self::mapped_polynomials(&dividend, &divisor, argument_map)
                    else {
                        continue;
                    };
                    (
                        format!("\0integer-quotient:{dividend:?}/{divisor:?}"),
                        self.quotient_interval(&dividend, &divisor),
                    )
                }
                OpaqueTerm::ShiftLeft {
                    value,
                    count,
                    count_bits,
                } => {
                    let Some((value, count)) =
                        Self::mapped_polynomials(&value, &count, argument_map)
                    else {
                        continue;
                    };
                    (
                        format!("\0integer-shift-left:{value:?}<<{count:?}"),
                        self.shift_left_interval(&value, &count, count_bits),
                    )
                }
                OpaqueTerm::ShiftRight {
                    value,
                    count,
                    count_bits,
                } => {
                    let Some((value, count)) =
                        Self::mapped_polynomials(&value, &count, argument_map)
                    else {
                        continue;
                    };
                    (
                        format!("\0integer-shift-right:{value:?}>>{count:?}"),
                        self.shift_right_interval(&value, &count, count_bits),
                    )
                }
                OpaqueTerm::Bitwise {
                    operator,
                    left,
                    right,
                    bits,
                    signed,
                } => {
                    let Some(left) =
                        super::inductive_judgment::apply_argument_map(&left, argument_map)
                    else {
                        continue;
                    };
                    let Some(right) =
                        super::inductive_judgment::apply_argument_map(&right, argument_map)
                    else {
                        continue;
                    };
                    if let Some(value) = self.integer_bitwise(operator, left, right, bits, signed) {
                        argument_map.insert(atom, value);
                    }
                    continue;
                }
            };
            // An unchanged operand re-mints the same atom and keeps its
            // already-registered interval.
            self.arithmetic_intervals
                .entry(next_atom.clone())
                .or_insert(interval);
            argument_map.insert(atom, Polynomial::atom(next_atom));
        }
    }

    pub(super) fn for_proof_integer_formation(
        program: &'program dyn crate::CheckingInput,
        owner: SymbolHandle,
        hypotheses: &[ExpressionHandle],
        selections: &[crate::proof_contracts::arithmetic_domains::DispatchSelection],
    ) -> Self {
        let mut bindings = Vec::new();
        for (handle, expression) in program.expression_table.iter_expressions() {
            let ExpressionNode::Name(path) = expression else {
                continue;
            };
            if path.symbol.is_valid()
                && (crate::proof_contracts::proof_embeddings::integer_embedding(program, handle)
                    .is_some()
                    || proof_integer_expression(program, handle))
                && !bindings.iter().any(|(symbol, _)| *symbol == path.symbol)
            {
                bindings.push((
                    path.symbol,
                    Polynomial::atom(format!("\0proof-symbol:{:?}", path.symbol)),
                ));
            }
        }
        let mut engine = Self::strict_for_declaration(program, owner);
        engine.strict_symbol_bindings = Some(bindings);
        engine.proof_integer_formation = true;
        engine.install_source_hypotheses(hypotheses);
        for selection in selections {
            engine.install_dispatch_selection(*selection);
        }
        engine
    }

    /// Start with an empty explicit roster. Declaration-scoped judgments supply
    /// every binder themselves; scanning all program names only to discard that
    /// roster makes each local predicate proportional to the entire program.
    pub(super) fn for_explicit_bindings(program: &'program dyn crate::CheckingInput) -> Self {
        Self::for_owner(program, SymbolHandle::default(), Some(Vec::new()))
    }

    /// Like [`Engine::new`], plus the reserved `result` atom for the
    /// machine's return value (unless a real parameter shadows it, matching
    /// the call-site binder rule). Used by the inductive transition path,
    /// where each arm binds or shares `result`.
    pub(super) fn with_result_atom(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        root: &typed_trees::state::State,
    ) -> Self {
        let mut engine = Self::new(program, machine);
        let shadowed = program
            .state_parameters(root)
            .iter()
            .any(|parameter| parameter.name.as_str() == RESULT_BINDER);
        if !shadowed {
            let unsigned = root.return_type.is_valid()
                && unsigned_parameter_carrier(program, root.return_type);
            engine.declare_atom(RESULT_BINDER, unsigned);
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
        self.collect_entry_range_hypotheses(&mut comparisons);
        let mut fully_visible = self.install_hypotheses(comparisons);
        fully_visible &= self.install_source_hypotheses(facts);
        fully_visible
    }

    /// These facts have independently formed. Install their comparisons in
    /// order so a later quotient can consume an earlier nonzero premise.
    /// Legacy scalar readers also admit floats: retain unoriented differences
    /// only for the exact selected integer relation, not arbitrary disequality.
    fn install_source_hypotheses(&mut self, facts: &[ExpressionHandle]) -> bool {
        let mut fully_visible = true;
        for fact in facts {
            let mut comparisons = Vec::new();
            let mut integer_differences = Vec::new();
            for conjunct in self.conjuncts(*fact) {
                // Prepared comparisons come from retained IntegerRelation
                // meanings (and integer/address identity), never IEEE values.
                let retain_integer_differences = self.strict_symbol_bindings.is_none()
                    && (self.is_mathematical_application(conjunct)
                        || self.source_integer_disequality(conjunct));
                let first_comparison = comparisons.len();
                fully_visible &= self.collect_comparisons(&[conjunct], &mut comparisons);
                if retain_integer_differences {
                    integer_differences.extend(
                        comparisons[first_comparison..]
                            .iter()
                            .filter(|(operator, _, _)| *operator == BinaryOperator::NotEqual)
                            .map(|(_, left, right)| left.sub(right)),
                    );
                }
            }
            // Normalize the whole fact before publishing any of its premises.
            fully_visible &= self.install_hypotheses(comparisons);
            for difference in integer_differences {
                let difference = self.substituted(&difference);
                if !self.nonzero_differences.contains(&difference) {
                    self.nonzero_differences.push(difference);
                }
            }
        }
        fully_visible
    }

    fn source_integer_disequality(&self, expression: ExpressionHandle) -> bool {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return false;
        };
        if binary.operator != BinaryOperator::NotEqual {
            return false;
        }
        let operands = [binary.left, binary.right];
        let references = operands.map(|operand| self.source_result_type(operand));
        operands
            .into_iter()
            .zip(references)
            .all(|(operand, reference)| {
                proof_integer_expression(self.program, operand)
                    || match reference
                        .and_then(|reference| self.program.primitive_type_reference(reference))
                    {
                        Some(primitive) => {
                            primitive.is_integer()
                                || primitive == typed_trees::types::PrimitiveType::Addr
                        }
                        None => {
                            super::proof_integer::anonymous_integer_value(self.program, operand)
                                .is_some()
                        }
                    }
            })
            && typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.program,
                self.source_owner,
                expression,
                language_core::OperatorSpelling::NotEqual,
                &references,
            )
    }

    /// Entry parameters retain their intrinsic finite carrier bounds and
    /// caller-discharged literal ranges. Install these before normalizing
    /// arguments: a guarded wrapping predecessor can then prove no-wrap.
    /// Sub-state parameters are different binders that may reuse names.
    pub(super) fn collect_entry_range_hypotheses(
        &mut self,
        comparisons: &mut Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) {
        let Some(machine) = self.program.machine(self.machine_symbol) else {
            return;
        };
        let Some(entry) = self.program.machine_states(machine).first() else {
            return;
        };
        for parameter in self.program.state_parameters(entry) {
            let atom = Polynomial::atom(parameter.name.as_str().to_owned());
            if let Some((minimum, maximum)) = self
                .program
                .primitive_type_reference(parameter.type_reference)
                .filter(|primitive| primitive.is_fixed_integer())
                .and_then(|primitive| primitive.integer_bounds())
            {
                comparisons.push((
                    BinaryOperator::GreaterOrEqual,
                    atom.clone(),
                    Polynomial::constant(BigInt::from_i128(minimum)),
                ));
                comparisons.push((
                    BinaryOperator::LessOrEqual,
                    atom.clone(),
                    Polynomial::constant(BigInt::from_i128(maximum)),
                ));
            }
            let Some(interval) =
                crate::proof_contracts::arithmetic_domains::range_constraint_interval(
                    self.program,
                    parameter.type_reference,
                )
            else {
                continue;
            };
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
            if self.is_mathematical_application(*fact) {
                fully_visible &= self.collect_mathematical_comparisons(*fact, comparisons);
                continue;
            }
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
        // Premises arrive in formation order: intrinsic carrier bounds can
        // precede an equality fixing their symbolic ceiling. Re-normalize the
        // retained bounds too, or that equality disappears into substitution
        // while earlier inequalities keep an atom the goal no longer uses.
        let mut lower_bounds = std::mem::take(&mut self.bounds);
        for (polynomial, _) in &mut lower_bounds {
            *polynomial = self.substituted(polynomial);
        }
        let mut nonzero_differences = Vec::new();
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
                BinaryOperator::NotEqual => nonzero_differences.push(difference_lr),
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
        // Disequality alone has no direction. Once the ordinary bounds prove
        // a direction, excluding zero strengthens that integer bound by one.
        // Strict integer adapters also retain an unoriented difference as
        // itself nonzero. Legacy readers admit a wider expression language
        // (including floating operands), so they keep only the old bound rule.
        let previous_bounds = self.bounds.len();
        for difference in nonzero_differences {
            if self.strict_symbol_bindings.is_some()
                && !self.nonzero_differences.contains(&difference)
            {
                self.nonzero_differences.push(difference.clone());
            }
            let opposite = difference.neg();
            let nonnegative = self.prove_at_least(&difference, &BigInt::zero());
            let nonpositive = self.prove_at_least(&opposite, &BigInt::zero());
            match (nonnegative, nonpositive) {
                (true, true) => self.requires_unsatisfiable = true,
                (true, false) => self.bounds.push((difference, BigInt::from_i64(1))),
                (false, true) => self.bounds.push((opposite, BigInt::from_i64(1))),
                (false, false) => {}
            }
        }
        if self.bounds.len() != previous_bounds {
            self.seed_matrix();
            self.close_matrix();
        }
        fully_visible
    }

    /// A strict integer-only query may also use coefficient residues and the
    /// existing polynomial lower-bound judgment. Legacy adapters can admit
    /// noninteger expressions, so these consequences are not installed by the
    /// shared hypothesis ingestion path.
    pub(super) fn integer_comparisons_unsatisfiable(
        &mut self,
        comparisons: Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) -> bool {
        if self.strict_symbol_bindings.is_none() || !self.strict_symbol_bindings_valid {
            return false;
        }
        let equalities = comparisons
            .iter()
            .filter(|(operator, _, _)| *operator == BinaryOperator::Equal)
            .map(|(_, left, right)| left.sub(right))
            .collect::<Vec<_>>();
        if !self.install_hypotheses(comparisons) {
            return false;
        }
        // Scoped observations retain admitted integer operations. Once their
        // live premises are installed, recover operand/result correlations as
        // well as intervals before asking whether the goal's complement fits.
        self.refresh_opaque_intervals();
        if self.requires_unsatisfiable
            || equalities
                .iter()
                .any(|difference| self.substituted(difference).excludes_integer_zero())
        {
            return true;
        }
        self.bounds_are_inconsistent()
    }

    /// Judge a full ensures fact: an `&&` chain proves when every conjunct
    /// proves, and is disproved when any conjunct is.
    pub(super) fn judge(&mut self, fact: ExpressionHandle) -> Judgment {
        match self.program.expression_table.expression(fact) {
            ExpressionNode::Boolean(true) => return Judgment::Proven,
            ExpressionNode::Boolean(false) => return Judgment::ConstantFalse,
            _ => {}
        }
        if self.is_mathematical_application(fact) {
            return self.judge_mathematical_application(fact);
        }
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
        self.judge_comparison(operator, left, right)
    }

    fn judge_comparison(
        &mut self,
        operator: BinaryOperator,
        left: Polynomial,
        right: Polynomial,
    ) -> Judgment {
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
            BinaryOperator::NotEqual => self.proves_nonzero(&difference_lr),
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
            BinaryOperator::Equal => self.proves_nonzero(&difference_lr),
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
    /// Flatten nested `&&` chains into conjunct handles (a single
    /// non-conjunction fact returns itself).
    pub(super) fn conjuncts(&self, fact: ExpressionHandle) -> Vec<ExpressionHandle> {
        let node = self.program.expression_table.expression(fact).clone();
        match node {
            ExpressionNode::Binary(binary)
                if binary.operator == BinaryOperator::And
                    && self.selected_binary_meaning(fact, &binary) =>
            {
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
        if self.is_mathematical_application(fact) {
            return self.mathematical_comparison(fact);
        }
        let node = self.program.expression_table.expression(fact).clone();
        let ExpressionNode::Binary(binary) = node else {
            return None;
        };
        if !self.selected_binary_meaning(fact, &binary) {
            return None;
        }
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
        // Dummy expression storage is an integer zero. An authority-bearing
        // substitution must not turn a missing or stale operand into a constant.
        if self.strict_symbol_bindings.is_some()
            && !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return None;
        }
        // Closed anonymous arithmetic has the same rational denotation in a
        // strict binder namespace. Fold the complete integral subtree; never
        // truncate a fractional child or erase an already-landed carrier.
        if let Some(value) = super::proof_integer::anonymous_integer_value(self.program, expression)
        {
            return Some(Polynomial::constant(value));
        }
        if let Some((_, value)) = self
            .strict_occurrences
            .iter()
            .find(|(candidate, _)| *candidate == expression)
        {
            return Some(value.clone());
        }
        // Closed constants keep their declaration's carrier and selected
        // operation. This is not a runtime-value lookup or a body replay.
        if self.strict_symbol_bindings.is_some()
            && !self.proof_integer_formation
            && let Some(value) = self
                .program
                .closed_integer_value_in(expression, self.source_owner)
        {
            return Some(Polynomial::constant(value.value));
        }
        let node = self.program.expression_table.expression(expression).clone();
        match node {
            ExpressionNode::Member(_)
                if self.strict_symbol_bindings.is_some()
                    && !self.proof_integer_formation
                    && self.formal_arguments.is_none() =>
            {
                // A resolved member place names the value its storage holds
                // at this occurrence: every store enforced the declared
                // range and the owning data's `where` facts, so the standing
                // interval bounds the atom at any read, mutable or not. The
                // identity is the canonical member-chain spelling, so every
                // occurrence of one place shares the atom.
                // Only `self`-rooted members are admitted: a member rooted at
                // a formal names the callee's storage, and an unbound record
                // actual leaves the formal's declaration symbol in scope --
                // the atom would collide with the caller's same-declared
                // parameter on a recursive call and read another frame's
                // hypotheses.
                // A match arm may already have replaced this place with its
                // pattern integer. That constant is the arm's value, so the
                // parameter-root guard below does not apply to it.
                let matched = format!("\0{}", integer_source_identity(self.program, expression));
                if let Some(replacement) = self.substitutions.get(&matched) {
                    return Some(replacement.clone());
                }
                let machine = self.program.machine(self.machine_symbol)?;
                let root = member_receiver_root_symbol(self.program, expression);
                if root != Some(self.machine_symbol) && root.is_some() {
                    return None;
                }
                let state = self.program.machine_states(machine).first()?;
                // The atom exists even when the place is no narrower than
                // its carrier: relational hypotheses still name it. Bare
                // `standing_integer_interval` deliberately reports None for
                // carrier-only places, so the carrier is the floor here.
                let place_type = crate::value_custody::places::declared_place_type_raw(
                    self.program,
                    machine,
                    Some(state),
                    expression,
                )?;
                let carrier =
                    self.program
                        .primitive_type_reference(place_type)
                        .and_then(|primitive| {
                            crate::proof_contracts::arithmetic_domains::primitive_range(primitive)
                        })?;
                let interval =
                    crate::proof_contracts::arithmetic_domains::standing_integer_interval(
                        self.program,
                        machine,
                        state,
                        expression,
                    )
                    .unwrap_or(carrier);
                let atom = format!("\0{}", integer_source_identity(self.program, expression));
                self.arithmetic_intervals.insert(
                    atom.clone(),
                    Interval {
                        low: interval.low.map(BigInt::from_i64),
                        high: interval.high.map(BigInt::from_i64),
                    },
                );
                Some(Polynomial::atom(atom))
            }
            ExpressionNode::Member(_) => None,
            ExpressionNode::Integer(value) => Some(Polynomial::constant(value.value_bignum()?)),
            ExpressionNode::Borrow(inner) => self.normalize(inner.target),
            ExpressionNode::Name(path) => {
                if let Some((_, value)) = self
                    .formal_arguments
                    .iter()
                    .flatten()
                    .find(|(symbol, _)| *symbol == path.symbol)
                {
                    return Some(value.clone());
                }
                if self.program.symbols.builtin_type_member(path.symbol)
                    == Some(symbols::BuiltinTypeMember::AddressBound)
                {
                    return self.address_model_bound();
                }
                if self.formal_arguments.is_some()
                    && !crate::reserved_result_place(self.program, expression)
                        .is_some_and(|place| place.machine_symbol == self.source_owner)
                {
                    // Unbound callee names cannot borrow a same-spelled caller
                    // atom. Closed declarations keep their existing evaluator;
                    // only the recursive result is shared across these scopes.
                    return self
                        .program
                        .closed_integer_value_in(expression, self.source_owner)
                        .map(|value| Polynomial::constant(value.value));
                }
                if let Some(bindings) = &self.strict_symbol_bindings {
                    let [member] = self
                        .program
                        .expression_table
                        .name_path_members(path.members)
                    else {
                        return None;
                    };
                    if let Some(value) = bindings
                        .iter()
                        .find_map(|(symbol, value)| (*symbol == path.symbol).then(|| value.clone()))
                    {
                        return Some(value);
                    }
                    // An authored binder always resolves through its symbol
                    // above; only the synthetic guarantee result reaches here.
                    return self
                        .strict_result_binding
                        .clone()
                        .filter(|_| member.as_str() == RESULT_BINDER);
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
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::BitwiseAnd
                        | BinaryOperator::BitwiseOr
                        | BinaryOperator::BitwiseXor
                ) =>
            {
                self.selected_bitwise_value(expression, &binary)
            }
            ExpressionNode::Binary(binary)
                if !self.selected_binary_meaning(expression, &binary)
                    && matches!(
                        binary.operator,
                        BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                    )
                    && super::ranking_range::unselected_or_builtin_arithmetic(
                        self.program,
                        expression,
                    ) =>
            {
                // Catalog arithmetic records the mathematical operation on the
                // destination register. An enclosing exact cast is the fit
                // claim; hypotheses decide whether a wrapping calculation lands
                // in that carrier. No authored overload is displaced.
                let left = self.normalize(binary.left)?;
                let right = self.normalize(binary.right)?;
                let value = match binary.operator {
                    BinaryOperator::Add => left.add(&right),
                    BinaryOperator::Subtract => left.sub(&right),
                    BinaryOperator::Multiply => left.checked_mul(&right)?,
                    _ => return None,
                };
                if let Some(result) = self.selected_policy_result(expression, value.clone()) {
                    return Some(result);
                }
                // The operation records its result on the operand carrier when
                // no separate result declaration exists. Wrapping stays opaque
                // until the hypotheses already installed prove the result lands
                // in that carrier; the decreased register is that landed value.
                let primitive = [binary.left, binary.right]
                    .into_iter()
                    .find_map(|operand| {
                        crate::proof_contracts::proof_embeddings::integer_embedding(
                            self.program,
                            operand,
                        )
                        .map(|embedding| embedding.primitive)
                        .filter(|primitive| primitive.is_fixed_integer())
                    })?;
                self.integer_policy_value(
                    primitive,
                    numerics::arithmetic::ArithmeticDomain::Wrapping,
                    value,
                )
            }
            ExpressionNode::Binary(binary)
                if !self.selected_binary_meaning(expression, &binary) =>
            {
                None
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                ) =>
            {
                self.selected_exact_shift_value(expression, &binary)
            }
            ExpressionNode::Binary(binary)
                if !proof_integer_expression(self.program, expression)
                    && matches!(
                        binary.operator,
                        BinaryOperator::Add
                            | BinaryOperator::Subtract
                            | BinaryOperator::Multiply
                            | BinaryOperator::Divide
                            | BinaryOperator::Modulo
                    )
                    && self
                        .source_result_type(expression)
                        .is_some_and(|reference| {
                            matches!(
                                self.program.arithmetic_domain_for_type_reference(reference),
                                numerics::arithmetic::ArithmeticDomain::Wrapping
                                    | numerics::arithmetic::ArithmeticDomain::Saturating
                            )
                        }) =>
            {
                let left = self.normalize(binary.left)?;
                let right = self.normalize(binary.right)?;
                let value = self.integer_binary_value(binary.operator, left, right)?;
                self.selected_policy_result(expression, value)
            }
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::Add => {
                    let left = self.normalize(binary.left)?;
                    let right = self.normalize(binary.right)?;
                    self.selected_policy_result(expression, left.add(&right))
                }
                BinaryOperator::Subtract => {
                    let left = self.normalize(binary.left)?;
                    let right = self.normalize(binary.right)?;
                    self.selected_policy_result(expression, left.sub(&right))
                }
                BinaryOperator::Multiply => {
                    let left = self.normalize(binary.left)?;
                    let right = self.normalize(binary.right)?;
                    let product = left.checked_mul(&right)?;
                    self.selected_policy_result(expression, product)
                }
                BinaryOperator::Divide | BinaryOperator::Modulo => {
                    let dividend = self.normalize(binary.left)?;
                    let divisor = self.normalize(binary.right)?;
                    // Scoped runtime values retain the selected builtin Exact
                    // integer denotation, including non-integral quotients
                    // truncated toward zero. Source formation remains a
                    // separate obligation; this is not rational division or
                    // authority for a proof-only operation.
                    if self.strict_symbol_bindings.is_some()
                        && !self.proof_integer_formation
                        && self
                            .source_result_type(expression)
                            .is_some_and(|reference| {
                                self.program
                                    .primitive_type_reference(reference)
                                    .is_some_and(|primitive| primitive.is_fixed_integer())
                            })
                    {
                        return self.integer_binary_value(binary.operator, dividend, divisor);
                    }
                    let builtin_proof_integer = proof_integer_expression(self.program, expression)
                        && self
                            .program
                            .expression_table
                            .authored_selection_occurrences(expression)
                            .all(|occurrence| {
                                use language_semantics::declaration_selection::{
                                    AuthoredDeclarationSelectionIntrinsic as Intrinsic,
                                    AuthoredDeclarationSelectionLateBinding as LateBinding,
                                    AuthoredDeclarationSelectionTarget as Target,
                                };
                                self.program
                                    .authored_declaration_selections()
                                    .get(occurrence)
                                    .is_some_and(|selection| {
                                        matches!(
                                            selection.target(),
                                            Target::Intrinsic(Intrinsic::BuiltinOperator)
                                                | Target::LateBound(LateBinding::CheckedOperator)
                                        )
                                    })
                            });
                    if builtin_proof_integer
                        && let (Some(dividend), Some(divisor)) = (
                            self.substituted(&dividend).constant_value(),
                            self.substituted(&divisor).constant_value(),
                        )
                    {
                        let (quotient, remainder) = dividend.div_rem(&divisor)?;
                        return Some(Polynomial::constant(
                            if binary.operator == BinaryOperator::Divide {
                                quotient
                            } else {
                                remainder
                            },
                        ));
                    }
                    if builtin_proof_integer {
                        let divisor = self.substituted(&divisor);
                        if !self.proves_nonzero(&divisor) {
                            return None;
                        }
                        // The same admitted integer operation retains both
                        // symbolic operands. A result interval or reflexivity
                        // cannot supply its own nonzero formation premise.
                        return self.integer_binary_value(
                            binary.operator,
                            self.substituted(&dividend),
                            divisor,
                        );
                    }
                    if binary.operator == BinaryOperator::Divide {
                        return None;
                    }
                    let modulus = self.substituted(&divisor).constant_value()?;
                    if modulus.is_zero() {
                        return None;
                    }
                    self.integer_remainder(dividend, Polynomial::constant(modulus), false)
                }
                _ => None,
            },
            ExpressionNode::Call(call) => {
                if crate::mathematical_applications::selected_definition(self.program, &call)
                    .is_some()
                {
                    return self.normalize_mathematical_application(expression);
                }
                if crate::proof_contracts::proof_embeddings::is_exact_embed_call(
                    self.program,
                    &call,
                ) {
                    let (_, argument) =
                        crate::proof_contracts::proof_embeddings::integer_embedding_argument(
                            self.program,
                            expression,
                        )?;
                    return self.normalize_integer_embedding(argument);
                }
                // Ordinary proof views are source-defined data and machines.
                // Their spelling cannot introduce an arithmetic atom or stand
                // in for a selected declaration and its checked semantics.
                // A value call result is different: the callee proved its
                // `ensures` conjuncts at every exit, so inside the strict
                // judgment the produced term is an opaque atom carrying the
                // declared return carrier intersected with those conjuncts.
                if self.strict_symbol_bindings.is_some()
                    && self.formal_arguments.is_none()
                    && let Some(interval) =
                        crate::proof_contracts::arithmetic_domains::call_result_interval(
                            self.program,
                            self.machine_symbol,
                            expression,
                            &call,
                        )
                {
                    let atom = format!("\0call-result:{expression:?}");
                    self.arithmetic_intervals.insert(
                        atom.clone(),
                        Interval {
                            low: interval.low().map(BigInt::from_i64),
                            high: interval.high().map(BigInt::from_i64),
                        },
                    );
                    return Some(Polynomial::atom(atom));
                }
                None
            }
            ExpressionNode::Cast(cast)
                if proof_nat_cast(self.program, &cast)
                    && (self.strict_symbol_bindings.is_none() || self.proof_integer_formation) =>
            {
                // Formation is checked separately against prior facts. This
                // conversion preserves the already-proven mathematical value.
                self.normalize(cast.value)
            }
            ExpressionNode::Cast(cast) if self.value_preserving_integer_cast(&cast) => {
                // Formation checks independently prove carrier fit before a
                // proposition is admitted. Every authored integer coercion
                // then denotes its operand, regardless of the result policy.
                // Child normalization still owns captured value identity and
                // the selected arithmetic semantics; this does not erase the
                // wrapping or saturation of an operation beneath the cast.
                self.normalize(cast.value)
            }
            // A match join denotes one term when every arm denotes that term
            // after the bindings already installed for this occurrence. Value
            // patterns are bound by the walker that selects the arm; this join
            // does not give a later arm a pattern it failed. Differing arms
            // stay outside the engine rather than picking one of them.
            ExpressionNode::Match(dispatch) => self.normalize_matched_integer(&dispatch),
            _ => None,
        }
    }

    fn normalize_matched_integer(
        &mut self,
        dispatch: &typed_trees::expression::TableMatchExpression,
    ) -> Option<Polynomial> {
        let arms = self
            .program
            .expression_table
            .match_arms(dispatch.arms)
            .to_vec();
        if arms.is_empty() {
            return None;
        }
        let mut joined = None;
        for arm in arms {
            let normalized = self.normalize(arm.value)?;
            let value = self.substituted(&normalized);
            match &joined {
                None => joined = Some(value),
                Some(previous) if previous == &value => {}
                Some(_) => return None,
            }
        }
        joined
    }

    /// A builtin `+ - *` over a fixed integer carrier denotes its
    /// mathematical result `M` only under the operation's selected policy
    /// (`proofs/contracts.md`, total arithmetic). An Exact result is `M`: the
    /// arithmetic-domain checker discharged its representability before any
    /// proof term formed. A Wrapping result is the width-specific wrapping of
    /// `M` and a Saturating result is `M` clamped, so each equals `M` exactly
    /// when `M` lands in the result carrier; that no-wrap premise is proved
    /// here from the hypotheses already installed (a dominating guard such as
    /// `n > 0` beside the formal's carrier bounds), never assumed from the
    /// descent or contract the engine is asked to establish
    /// (`numeric_values.md`: "Wrapping loop updates need an independent
    /// no-wrap proof before monotonicity can be inferred"). Otherwise retain
    /// the completed policy value as an opaque bounded representative.
    /// Selected binary meaning already refused Trapping and noninteger results.
    fn selected_policy_result(
        &mut self,
        expression: ExpressionHandle,
        value: Polynomial,
    ) -> Option<Polynomial> {
        use numerics::arithmetic::ArithmeticDomain;
        if proof_integer_expression(self.program, expression) {
            return Some(value);
        }
        let reference = self.source_result_type(expression)?;
        match self.program.arithmetic_domain_for_type_reference(reference) {
            ArithmeticDomain::Exact => Some(value),
            ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating => {
                let primitive = self.program.primitive_type_reference(reference)?;
                self.integer_policy_value(
                    primitive,
                    self.program.arithmetic_domain_for_type_reference(reference),
                    value,
                )
            }
            ArithmeticDomain::Trapping => None,
        }
    }

    fn value_preserving_integer_cast(
        &self,
        cast: &typed_trees::expression::TableCastExpression,
    ) -> bool {
        if cast.form.is_recast() || self.proof_integer_formation {
            return false;
        }
        self.program
            .primitive_type_reference(cast.target_type)
            .is_some_and(|target| target.is_fixed_integer())
            && (matches!(
                self.program.expression_table.expression(cast.value),
                ExpressionNode::Integer(_)
            ) || crate::proof_contracts::proof_embeddings::integer_embedding(
                self.program,
                cast.value,
            )
            .is_some_and(|source| source.primitive.is_fixed_integer()))
    }

    fn is_mathematical_application(&self, expression: ExpressionHandle) -> bool {
        self.program
            .expression_table
            .call_node(expression)
            .is_some_and(|call| {
                crate::mathematical_applications::selected_definition(self.program, call).is_some()
            })
    }

    pub(in crate::proof_contracts::contract_entailment) fn address_model_bound(
        &mut self,
    ) -> Option<Polynomial> {
        let polynomial = self.address_model_bound_value()?;
        self.bounds.push((polynomial.clone(), BigInt::from_i64(1)));
        Some(polynomial)
    }

    /// The selected model's exclusive endpoint, without installing a premise.
    /// Formation checks use the same symbol as observed address carriers.
    fn address_model_bound_value(&self) -> Option<Polynomial> {
        let bound = self
            .program
            .symbols
            .builtin_type_member_symbol(symbols::BuiltinTypeMember::AddressBound)?;
        let model = self.program.symbols.get(bound).parent;
        (self.program.symbols.builtin_type_atom(model) == Some(symbols::BuiltinTypeAtom::Address))
            .then_some(())?;
        Some(Polynomial::atom(format!(
            "\0address-model-bound:{model:?}:{bound:?}"
        )))
    }

    fn normalize_integer_embedding(&mut self, expression: ExpressionHandle) -> Option<Polynomial> {
        let embedding =
            crate::proof_contracts::proof_embeddings::integer_embedding(self.program, expression)?;
        if let Some(literal) = self.program.expression_table.integer_node(expression) {
            return Some(Polynomial::constant(literal.value_bignum()?));
        }
        // Embedding projects the same selected, policy-correct value used by
        // ordinary comparisons. Unsupported denotations stay opaque; there is
        // no second arithmetic interpretation of the written computation.
        let direct = self.normalize(expression);
        // A scoped implication may read only values its owner captured. The
        // formation query separately owns opaque source denotations; importing
        // those here would equate occurrences across distinct scoped rosters.
        if direct.is_none()
            && (self.formal_arguments.is_some()
                || self.strict_symbol_bindings.is_some() && !self.proof_integer_formation)
        {
            return None;
        }
        let polynomial = direct.unwrap_or_else(|| {
            Polynomial::atom(format!(
                "\0embed:{:?}:{}",
                embedding.primitive,
                integer_source_identity(self.program, expression),
            ))
        });
        self.bounds.push((polynomial.clone(), embedding.minimum));
        if let Some(maximum) = embedding.maximum {
            self.bounds.push((polynomial.neg(), maximum.negate()));
        } else {
            // Exact address model: embed(address) < Bound. The strict one-past
            // relation grants no fixed-width count conversion or address
            // realization. The target-neutral checker keeps Bound symbolic.
            let bound = self.address_model_bound()?;
            self.bounds
                .push((bound.sub(&polynomial), BigInt::from_i64(1)));
        }
        self.seed_matrix();
        self.close_matrix();
        Some(polynomial)
    }
}

/// `is_signed_integer` is false exactly for the unsigned integer primitives
/// (floats/bool/string report true), so this marks precisely the carriers whose
/// atoms carry the implicit `>= 0`.
fn unsigned_parameter_carrier(
    program: &dyn crate::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    program
        .type_reference_table
        .primitive_type(reference)
        .is_some_and(|primitive| !primitive.is_signed_integer())
}

/// The declaration symbol a member chain bottoms out on, or None when the
/// receiver is not a name place (a literal, a call result, a projection).
fn member_receiver_root_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let mut handle = expression;
    loop {
        match program.expression_table.expression(handle) {
            ExpressionNode::Member(member) => handle = member.receiver,
            ExpressionNode::Borrow(inner) => handle = inner.target,
            ExpressionNode::Name(path) => {
                return path.symbol.is_valid().then_some(path.symbol);
            }
            _ => return None,
        }
    }
}

/// A transient normalization key, never artifact identity. Resolved source
/// symbols, selected casts and operator structure remain distinct. Unsupported
/// source forms and calls without established value identity use occurrence
/// identity rather than guessing equivalence. A declared result carrier grants
/// standing bounds on one observation, not equality between two invocations.
fn integer_source_identity(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> String {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if path.symbol.is_valid() => format!("name:{:?}", path.symbol),
        ExpressionNode::Integer(literal) => format!("literal:{literal:?}"),
        ExpressionNode::Binary(binary) => format!(
            "binary:{:?}({},{})",
            binary.operator,
            integer_source_identity(program, binary.left),
            integer_source_identity(program, binary.right)
        ),
        ExpressionNode::Cast(cast) => format!(
            "cast:{:?}:{:?}:{:?}:{}",
            cast.target_type,
            cast.domain,
            cast.semantic_domain_id,
            integer_source_identity(program, cast.value)
        ),
        ExpressionNode::Member(member) if member.member_symbol.is_valid() => format!(
            "member:{:?}:{:?}:{}",
            member.member_symbol,
            member.case_variant,
            integer_source_identity(program, member.receiver)
        ),
        ExpressionNode::Indexed(indexed) => format!(
            "index:{}:{}",
            integer_source_identity(program, indexed.collection),
            integer_source_identity(program, indexed.index)
        ),
        ExpressionNode::Call(call) if call.target_symbol.is_valid() => {
            let plans = crate::machine_calls::calls::operand_call_plans(program, None);
            if !crate::machine_calls::denotational_calls::expression_values_have_call_identity(
                program,
                expression,
                &plans.0,
                &plans.1,
                &|_| false,
            ) {
                return format!("occurrence:{expression:?}");
            }
            format!(
                "call:{:?}:{:?}:{}:{:?}",
                call.target_symbol,
                call.machine_arguments,
                integer_source_identity(program, call.receiver),
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| integer_source_identity(program, *argument))
                    .collect::<Vec<_>>(),
            )
        }
        _ => format!("occurrence:{expression:?}"),
    }
}

/// Compare already-admitted embedding sources using the same carrier and
/// exact transient identity as arithmetic normalization. No display label or
/// bare call spelling can establish equality.
pub fn integer_embedding_sources_equal(
    program: &dyn crate::CheckingInput,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let Some(left_carrier) =
        crate::proof_contracts::proof_embeddings::integer_embedding(program, left)
    else {
        return false;
    };
    let Some(right_carrier) =
        crate::proof_contracts::proof_embeddings::integer_embedding(program, right)
    else {
        return false;
    };
    left_carrier.primitive == right_carrier.primitive
        && integer_source_identity(program, left) == integer_source_identity(program, right)
}
