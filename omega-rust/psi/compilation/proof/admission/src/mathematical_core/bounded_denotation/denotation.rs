use super::*;
use crate::term_matchers::TermMatcher;

/// Proposition and scalar-term denotation state: the arena the judgment's
/// terms live in, the declaration signature every constant names, and the
/// interning tables that give each atom, carrier and scalar value exactly
/// one declaration.
pub(super) struct Denotation {
    pub(super) arena: TermArena,
    pub(super) type_zero: TermHandle,
    pub(super) two: TermHandle,
    pub(super) two_zero: TermHandle,
    pub(super) two_one: TermHandle,
    pub(super) boolean_equality: TermHandle,
    pub(super) declarations: Vec<Declaration>,
    /// Canonical atomic proposition → assumption position. The key is
    /// the proposition's lifted mathematical form when the fixed-width
    /// relation lifts, so lift/lower-equivalent citations share one atom.
    pub(super) atoms: BTreeMap<Proposition, u32>,
    /// Scalar type → `Type 0` carrier assumption position.
    pub(super) carriers: BTreeMap<ScalarType, u32>,
    /// Canonical scalar term → assumption position at its carrier. The
    /// key is the term's evaluated closed literal when it has one, so
    /// decided closed equalities denote reflexive identities.
    pub(super) terms: BTreeMap<ScalarTerm, u32>,
    /// Content algebra → `Type 0` carrier assumption position. Each
    /// algebra's conservation equations denote `Id` over their own
    /// carrier, so identities in different algebras can never compose.
    pub(super) content_carriers: BTreeMap<ContentAlgebra, u32>,
    /// `(algebra, content term)` → element assumption position at that
    /// algebra's carrier. The key is the term itself: `ContentTerm`'s
    /// constructors already normalize ordering and nesting, so
    /// syntactic identity is the algebra's own canonical form.
    pub(super) content_terms: BTreeMap<(ContentAlgebra, ContentTerm), u32>,
    /// The `Int : Type 0` carrier position — one declaration every
    /// mathematical-integer constant and relation names.
    pub(super) integer: Option<u32>,
    /// `IntLt : Π(_ : Int). Π(_ : Int). Type 0` — the strict order.
    pub(super) integer_less_than: Option<u32>,
    /// `IntLe : Π(_ : Int). Π(_ : Int). Type 0` — the nonstrict order.
    pub(super) integer_less_or_equal: Option<u32>,
    /// The fixed integer-law roster: law → assumption position.
    pub(super) integer_laws: BTreeMap<IntegerLaw, u32>,
    /// Shared fixed-width numeral definitions and their fixed arithmetic laws.
    pub(super) binary_numerals: binary_numerals::BinaryNumerals,
    pub(super) subtraction: subtraction::Subtraction,
    pub(super) addition: addition::Addition,
    /// Shared `mul` and the multiplication/division-residual law roster
    /// the correlated multiply bounds spend.
    pub(super) multiplication: multiplication::Multiplication,
    /// The correlated forbidden-root law roster: chain-inversion,
    /// quotient-squeeze and divisor-case-split assumptions.
    pub(super) forbidden_roots: forbidden_roots::ForbiddenRoots,
    /// Canonical mathematical term → `Int`-typed declaration position.
    /// Closed terms intern by exact evaluated value — so a decided
    /// `IntegerMathEqual` on closed operands denotes `refl`-provable
    /// `Id Int c c` — and open terms intern by the term itself.
    pub(super) math_terms: BTreeMap<MathTermKey, u32>,
    /// `Int` declaration position → the exact closed value it interned
    /// by — the reverse of `math_terms`' `MathTermKey::Closed` entries.
    /// The closed Truth/Falsehood carrier crossing reads the decided
    /// value behind a denoted endpoint to pick the numeral-order
    /// derivation it needs without re-evaluating the term.
    pub(super) closed_math_values: BTreeMap<u32, BigInt>,
    /// Sparse scalar terms outside the compositional Int denotation:
    /// field projections and any constructor `integer_operation_term`
    /// does not classify. Values/closed literals use math_terms; every
    /// classified operation denotes its uninterpreted function applied to
    /// the operand terms rather than a whole-term constant here.
    pub(super) scalar_integer_terms: BTreeMap<ScalarTerm, u32>,
    /// Uninterpreted fixed-integer operation → `Π` assumption position,
    /// interned per operation constructor and machine types. Sharing one
    /// function per operation keeps transport's operand rewrites inside
    /// `op l r` applications visible; one constant per applied term
    /// would hide them.
    pub(super) integer_operations: BTreeMap<integer_operations::IntegerOperation, u32>,
    /// Exact cast/widen operation → the position of its identity law
    /// `Π(x : Int). Id Int (op x) x` — the denoted content of the
    /// checked chain's "every edge preserves the mathematical integer"
    /// invariant, interned once per operation like the other fixed laws.
    pub(super) cast_identities: BTreeMap<integer_operations::IntegerOperation, u32>,
    /// `Primitive` leaf statement → decision-assumption position.
    pub(super) decisions: HashMap<TermHandle, u32>,
    /// `(operand, lower)` → assumption position of the operand's
    /// carrier-membership bound — `IntLe min' op'` or `IntLe op' max'`.
    /// A `Truth` bound over an open operand contributes exactly this
    /// fact; interning it per operand keeps the closure's named
    /// assumption the membership bound itself, not a rule implication.
    pub(super) carrier_bounds: BTreeMap<(ScalarTerm, bool), u32>,
    /// Bounded rule instance → decision-assumption position. The key is
    /// the instance's premise propositions in rule order plus its
    /// conclusion — exactly what determines the axiom's `Π` type — so
    /// two nodes deriving the same implication share one constant.
    pub(super) rule_axioms: BTreeMap<(Vec<Proposition>, Proposition), u32>,
    /// Declaration position → `Constant` node, so equal references share
    /// one handle.
    pub(super) constants: HashMap<u32, TermHandle>,
    /// Proposition → denoted term cache; denotation is deterministic, so
    /// the same proposition always denotes to the same handle.
    pub(super) denotations: BTreeMap<Proposition, TermHandle>,
}

/// The shared front of an exact-add/subtract definition-bound check: the
/// conclusion is a `LessOrEqual` over one literal and the defined output,
/// and the cited definition equates that output with the matched arithmetic
/// term. The rows differ only in the term variant, so one matcher-parameterized
/// pass extracts the literal side, the bound direction and the operands.
pub(super) struct ExactDefinitionBoundTarget<'a> {
    pub(super) literal: &'a ScalarTerm,
    pub(super) output: &'a ScalarTerm,
    pub(super) lower: bool,
    pub(super) scalar_type: &'a IntegerType,
    pub(super) expression: &'a ScalarTerm,
    pub(super) left: &'a ScalarTerm,
    pub(super) right: &'a ScalarTerm,
}

/// The conclusion of an arithmetic bound check is an `IntegerMathLessOrEqual`
/// whose operation side is the term being denoted; the checks differ only in
/// the math term variant, so one predicate-parameterized pass returns the
/// bound side, the operation side and the direction.
pub(super) fn math_less_or_equal_sides<'a>(
    conclusion: &'a Proposition,
    is_target_operation: fn(&IntegerMathTerm) -> bool,
) -> Option<(bool, &'a IntegerMathTerm, &'a IntegerMathTerm)> {
    match conclusion {
        Proposition::IntegerMathLessOrEqual(bound, operation) if is_target_operation(operation) => {
            Some((true, bound, operation))
        }
        Proposition::IntegerMathLessOrEqual(operation, bound) if is_target_operation(operation) => {
            Some((false, bound, operation))
        }
        _ => None,
    }
}

/// One operand's bound side of a direct or definition-cited arithmetic
/// bound: the operand term, its authored bound proposition and the
/// evidence naming that bound.
pub(super) struct OperandBound<'a> {
    pub(super) operand: &'a ScalarTerm,
    pub(super) bound: &'a Proposition,
    pub(super) evidence: TermHandle,
}

/// Everything the shared binary-bound chain computes for its caller: the
/// two endpoint shapes (for numeral extraction), the denoted endpoint,
/// operand and bound-operation terms.
pub(super) struct BinaryBoundChain {
    pub(super) left_endpoint: ScalarTerm,
    pub(super) right_endpoint: ScalarTerm,
    pub(super) left_endpoint_term: TermHandle,
    pub(super) right_endpoint_term: TermHandle,
    pub(super) left_term: TermHandle,
    pub(super) right_term: TermHandle,
    pub(super) bound_operation: TermHandle,
    pub(super) operand_operation: TermHandle,
    pub(super) left_order: TermHandle,
    pub(super) right_order: TermHandle,
}

/// The shared endpoint-through-terms chain of every arithmetic bound
/// check: each operand's endpoint re-shapes its own bound evidence and
/// the operation builds the applicative bound and operand terms.
/// `right_antitone` flips the right operand's direction for subtraction's
/// contravariant right side. The law application stays with the caller:
/// the direct checks run a defensive shape check first, so a shared
/// `binary_bound_order` applies it where the caller asks.
pub(super) fn binary_bound_chain(
    denotation: &mut Denotation,
    left: OperandBound<'_>,
    right: OperandBound<'_>,
    lower: bool,
    right_antitone: bool,
    scalar_type: &IntegerType,
    terms: fn(
        &mut Denotation,
        TermHandle,
        TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError>,
) -> Result<Option<BinaryBoundChain>, BoundedDenotationError> {
    let right_lower = if right_antitone { !lower } else { lower };
    let Some((left_endpoint, left_order)) = denotation.add_bound_endpoint(
        left.operand,
        left.bound,
        left.evidence,
        lower,
        scalar_type,
    )?
    else {
        return Ok(None);
    };
    let Some((right_endpoint, right_order)) = denotation.add_bound_endpoint(
        right.operand,
        right.bound,
        right.evidence,
        right_lower,
        scalar_type,
    )?
    else {
        return Ok(None);
    };
    let left_endpoint_term = denotation.fixed_scalar_term(&left_endpoint)?;
    let right_endpoint_term = denotation.fixed_scalar_term(&right_endpoint)?;
    let left_term = denotation.fixed_scalar_term(left.operand)?;
    let right_term = denotation.fixed_scalar_term(right.operand)?;
    let bound_operation = terms(denotation, left_endpoint_term, right_endpoint_term)?;
    let operand_operation = terms(denotation, left_term, right_term)?;
    Ok(Some(BinaryBoundChain {
        left_endpoint,
        right_endpoint,
        left_endpoint_term,
        right_endpoint_term,
        left_term,
        right_term,
        bound_operation,
        operand_operation,
        left_order,
        right_order,
    }))
}

/// The two-sided monotonicity application every arithmetic bound check
/// ends its endpoint chain with: lower spends each operand's endpoint on
/// the same side, upper flips both pairs.
pub(super) fn binary_bound_order(
    denotation: &mut Denotation,
    lower: bool,
    chain: &BinaryBoundChain,
    ordered_law: impl Fn(&mut Denotation, &[TermHandle]) -> Result<TermHandle, BoundedDenotationError>,
) -> Result<TermHandle, BoundedDenotationError> {
    if lower {
        ordered_law(
            denotation,
            &[
                chain.left_endpoint_term,
                chain.left_term,
                chain.right_endpoint_term,
                chain.right_term,
                chain.left_order,
                chain.right_order,
            ],
        )
    } else {
        ordered_law(
            denotation,
            &[
                chain.left_term,
                chain.left_endpoint_term,
                chain.right_term,
                chain.right_endpoint_term,
                chain.left_order,
                chain.right_order,
            ],
        )
    }
}

pub(super) fn exact_definition_bound_target<'a>(
    conclusion: &'a Proposition,
    definition: &'a Proposition,
    matcher: TermMatcher,
) -> Option<ExactDefinitionBoundTarget<'a>> {
    let Proposition::LessOrEqual(conclusion_left, conclusion_right) = conclusion else {
        return None;
    };
    let (literal, output, lower) = if conclusion_left.integer_value().is_some() {
        (conclusion_left, conclusion_right, true)
    } else if conclusion_right.integer_value().is_some() {
        (conclusion_right, conclusion_left, false)
    } else {
        return None;
    };
    let (defined_output, expression) = match definition {
        Proposition::Equal(first, second) => match (first, second) {
            (output @ ScalarTerm::Value { .. }, term) if matcher(term).is_some() => {
                (output, second)
            }
            (term, output @ ScalarTerm::Value { .. }) if matcher(term).is_some() => (output, first),
            _ => return None,
        },
        _ => return None,
    };
    let (scalar_type, left, right) = matcher(expression)?;
    if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
        || defined_output != output
        || output.scalar_type() != ScalarType::Integer(*scalar_type)
    {
        return None;
    }
    Some(ExactDefinitionBoundTarget {
        literal,
        output,
        lower,
        scalar_type,
        expression,
        left,
        right,
    })
}

/// The operation-specific plugs of the two binary arithmetic bound
/// chains: which `ScalarTerm::ExactInteger*` target the chain accepts,
/// which math term the conclusion's operation side must be, whether the
/// right operand is antitone — subtraction's `l − r` spends `r`'s upper
/// endpoint on its lower bound — the denoted application, the roster
/// monotonicity law, and the checked numeral-operation equation.
pub(super) struct BinaryBoundOperation {
    pub(super) target: TermMatcher,
    pub(super) is_target_operation: fn(&IntegerMathTerm) -> bool,
    pub(super) right_antitone: bool,
    pub(super) terms:
        fn(&mut Denotation, TermHandle, TermHandle) -> Result<TermHandle, BoundedDenotationError>,
    pub(super) monotone:
        fn(&mut Denotation, &[TermHandle]) -> Result<TermHandle, BoundedDenotationError>,
    pub(super) numeral_equation: fn(
        &mut Denotation,
        IntegerValue,
        IntegerValue,
        IntegerValue,
        TermHandle,
        TermHandle,
        TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError>,
}

/// The `get → record` surface one interned-position roster needs —
/// shared by the `BTreeMap` tables and the `HashMap` decision table.
pub(super) trait PositionRoster<K> {
    fn roster_position(&self, key: &K) -> Option<u32>;
    fn record_position(&mut self, key: K, position: u32);
}

impl<K: Ord> PositionRoster<K> for BTreeMap<K, u32> {
    fn roster_position(&self, key: &K) -> Option<u32> {
        self.get(key).copied()
    }

    fn record_position(&mut self, key: K, position: u32) {
        self.insert(key, position);
    }
}

impl<K: Eq + std::hash::Hash> PositionRoster<K> for HashMap<K, u32> {
    fn roster_position(&self, key: &K) -> Option<u32> {
        self.get(key).copied()
    }

    fn record_position(&mut self, key: K, position: u32) {
        self.insert(key, position);
    }
}

impl Denotation {
    /// Denote the checked direct binary-operation bound: a `Conjunction`
    /// of the two operand bounds proves `k ≤ op l r` or `op l r ≤ k`,
    /// where `k` is the checked value of the two endpoint literals —
    /// the direct form names the `op` target itself with no endpoint
    /// steps or cited axioms. The mapped bound is a canonical math
    /// literal; when its magnitude leaves the representable numeral
    /// range the denoted endpoint is an opaque `Int` constant no checked
    /// `op lb rb = k` equation can name — keep the instance fallback.
    pub(super) fn direct_binary_bound_evidence(
        &mut self,
        operation: &BinaryBoundOperation,
        premise: &Proposition,
        evidence: TermHandle,
        witness: &crate::IntegerAffineWitness,
        conclusion: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Proposition::Conjunction(bounds) = premise else {
            return Ok(None);
        };
        let [left_bound, right_bound] = bounds.as_slice() else {
            return Ok(None);
        };
        let Some((scalar_type, left, right)) = (operation.target)(&witness.target) else {
            return Ok(None);
        };
        if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
            || witness.root != *left
            || !witness.definition_axioms.is_empty()
            || !witness.literal_axioms.is_empty()
        {
            return Ok(None);
        }
        let Some((lower, bound, result)) =
            math_less_or_equal_sides(conclusion, operation.is_target_operation)
        else {
            return Ok(None);
        };
        let IntegerMathTerm::IntegerLiteral(bound_literal) = bound else {
            return Ok(None);
        };
        let Some(bound_value) = math_literal_value(*bound_literal) else {
            return Ok(None);
        };
        // The conjunction denotes to a `Σ` pair: the first conjunct is
        // `fst`, the second — last — conjunct is `snd`.
        let left_evidence = self.arena.insert(Term::Fst { pair: evidence });
        let right_evidence = self.arena.insert(Term::Snd { pair: evidence });
        let Some(chain) = binary_bound_chain(
            self,
            OperandBound {
                operand: left,
                bound: left_bound,
                evidence: left_evidence,
            },
            OperandBound {
                operand: right,
                bound: right_bound,
                evidence: right_evidence,
            },
            lower,
            operation.right_antitone,
            scalar_type,
            operation.terms,
        )?
        else {
            return Ok(None);
        };
        // The conclusion's operation math term must denote the same
        // application as the target's operands — the shared relation
        // already equated them, so this is the defensive shape check.
        let result_term = self.math_term(result)?;
        if !self
            .arena
            .structurally_equal(result_term, chain.operand_operation)
        {
            return Ok(None);
        }
        let order = binary_bound_order(self, lower, &chain, operation.monotone)?;
        let (Some((_, left_value)), Some((_, right_value))) = (
            chain.left_endpoint.integer_value(),
            chain.right_endpoint.integer_value(),
        ) else {
            return Ok(None);
        };
        let bound_term = self.math_term(bound)?;
        let Some(equality) = (operation.numeral_equation)(
            self,
            left_value,
            right_value,
            bound_value,
            chain.left_endpoint_term,
            chain.right_endpoint_term,
            bound_term,
        )?
        else {
            return Ok(None);
        };
        if lower {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteLeft,
                &[
                    chain.bound_operation,
                    bound_term,
                    chain.operand_operation,
                    equality,
                    order,
                ],
            )
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[
                    chain.operand_operation,
                    chain.bound_operation,
                    bound_term,
                    equality,
                    order,
                ],
            )
        }
        .map(Some)
    }

    /// Denote the checked exact-definition binary-operation bound:
    /// `lb ∧ rb` over the operation's two operands plus the cited
    /// `output = op l r` definition prove `k ≤ output` or `output ≤ k`,
    /// where `k` is the checked value of the two endpoint literals.
    /// Two-sided monotonicity combines the operand bounds; the cited
    /// definition equality — bridged by the checked `op l r = n`
    /// numeral equation when the definition's result is closed — moves
    /// the bound onto `output`, and a second interned equation
    /// `op lb rb = k` lands the literal.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn exact_binary_definition_bound_evidence(
        &mut self,
        operation: &BinaryBoundOperation,
        left_bound: &Proposition,
        left_evidence: TermHandle,
        right_bound: &Proposition,
        right_evidence: TermHandle,
        definition: &Proposition,
        definition_evidence: TermHandle,
        conclusion: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Some(ExactDefinitionBoundTarget {
            literal,
            output,
            lower,
            scalar_type,
            expression,
            left,
            right,
        }) = exact_definition_bound_target(conclusion, definition, operation.target)
        else {
            return Ok(None);
        };
        let Some(chain) = binary_bound_chain(
            self,
            OperandBound {
                operand: left,
                bound: left_bound,
                evidence: left_evidence,
            },
            OperandBound {
                operand: right,
                bound: right_bound,
                evidence: right_evidence,
            },
            lower,
            operation.right_antitone,
            scalar_type,
            operation.terms,
        )?
        else {
            return Ok(None);
        };
        let order = binary_bound_order(self, lower, &chain, operation.monotone)?;
        // `Id Int (op l' r') output'` — directly the cited definition when
        // the expression stays applicative, or the numeral-operation
        // equation `op l r = n` composed with the definition when the
        // closed result denotes to its numeral `n`.
        let output_term = self.fixed_scalar_term(output)?;
        let expression_term = self.fixed_scalar_term(expression)?;
        let denoted = self.denote(definition)?;
        let Some((_, from, to)) = self.identity_parts(denoted) else {
            return Ok(None);
        };
        let result_equality = if self
            .arena
            .structurally_equal(expression_term, chain.operand_operation)
        {
            let Some(equality) = self.directed_equality(
                from,
                to,
                chain.operand_operation,
                output_term,
                definition_evidence,
            ) else {
                return Ok(None);
            };
            equality
        } else {
            let (Some((_, left_value)), Some((_, right_value)), Some((_, result_value))) = (
                left.integer_value(),
                right.integer_value(),
                expression.integer_value(),
            ) else {
                return Ok(None);
            };
            let Some(bridge) = (operation.numeral_equation)(
                self,
                left_value,
                right_value,
                result_value,
                chain.left_term,
                chain.right_term,
                expression_term,
            )?
            else {
                return Ok(None);
            };
            let Some(forward) =
                self.directed_equality(from, to, expression_term, output_term, definition_evidence)
            else {
                return Ok(None);
            };
            let integer = self.integer_constant()?;
            self.transitivity(
                integer,
                chain.operand_operation,
                expression_term,
                output_term,
                bridge,
                forward,
            )
        };
        // `op lb' rb' ≤ output'` (lower) or `output' ≤ op lb' rb'` (upper).
        let order = if lower {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[
                    chain.bound_operation,
                    chain.operand_operation,
                    output_term,
                    result_equality,
                    order,
                ],
            )?
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteLeft,
                &[
                    chain.operand_operation,
                    output_term,
                    chain.bound_operation,
                    result_equality,
                    order,
                ],
            )?
        };
        // The bound side is an `op` over two endpoint numerals; the
        // checked equation `op lb rb = k` lands the conclusion's literal.
        let (Some((_, left_value)), Some((_, right_value)), Some((_, bound_value))) = (
            chain.left_endpoint.integer_value(),
            chain.right_endpoint.integer_value(),
            literal.integer_value(),
        ) else {
            return Ok(None);
        };
        let literal_term = self.fixed_scalar_term(literal)?;
        let Some(equality) = (operation.numeral_equation)(
            self,
            left_value,
            right_value,
            bound_value,
            chain.left_endpoint_term,
            chain.right_endpoint_term,
            literal_term,
        )?
        else {
            return Ok(None);
        };
        if lower {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteLeft,
                &[
                    chain.bound_operation,
                    literal_term,
                    output_term,
                    equality,
                    order,
                ],
            )
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[
                    output_term,
                    chain.bound_operation,
                    literal_term,
                    equality,
                    order,
                ],
            )
        }
        .map(Some)
    }

    pub(super) fn new() -> Self {
        let mut arena = TermArena::new();
        let type_zero = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let two = arena.insert(Term::Two);
        let two_zero = arena.insert(Term::TwoZero);
        let two_one = arena.insert(Term::TwoOne);
        Self {
            arena,
            type_zero,
            two,
            two_zero,
            two_one,
            boolean_equality: TermHandle::invalid(),
            declarations: Vec::new(),
            atoms: BTreeMap::new(),
            carriers: BTreeMap::new(),
            terms: BTreeMap::new(),
            content_carriers: BTreeMap::new(),
            content_terms: BTreeMap::new(),
            integer: None,
            integer_less_than: None,
            integer_less_or_equal: None,
            integer_laws: BTreeMap::new(),
            binary_numerals: binary_numerals::BinaryNumerals::default(),
            subtraction: subtraction::Subtraction::default(),
            addition: addition::Addition::default(),
            multiplication: multiplication::Multiplication::default(),
            forbidden_roots: forbidden_roots::ForbiddenRoots::default(),
            math_terms: BTreeMap::new(),
            closed_math_values: BTreeMap::new(),
            scalar_integer_terms: BTreeMap::new(),
            integer_operations: BTreeMap::new(),
            cast_identities: BTreeMap::new(),
            decisions: HashMap::new(),
            carrier_bounds: BTreeMap::new(),
            rule_axioms: BTreeMap::new(),
            constants: HashMap::new(),
            denotations: BTreeMap::new(),
        }
    }

    pub(super) fn position(&self) -> Result<u32, BoundedDenotationError> {
        u32::try_from(self.declarations.len())
            .map_err(|_| BoundedDenotationError::DepthLimitExceeded)
    }

    pub(super) fn constant(&mut self, declaration: u32) -> TermHandle {
        if let Some(&handle) = self.constants.get(&declaration) {
            return handle;
        }
        let handle = self.arena.insert(Term::Constant {
            declaration,
            levels: Vec::new(),
        });
        self.constants.insert(declaration, handle);
        handle
    }

    /// The `Type 0` assumption constant an atomic proposition outside
    /// the integer vocabulary denotes — every atom the certificate's
    /// cited premises reach, interned once by the proposition itself.
    pub(super) fn atom(
        &mut self,
        proposition: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let key = lift_fixed_integer_relation(proposition).unwrap_or_else(|| proposition.clone());
        let position =
            self.interned_keyed_assumption(|this| &mut this.atoms, key, |this| Ok(this.type_zero))?;
        Ok(self.constant(position))
    }

    /// The `Type 0` assumption constant a scalar carrier denotes.
    pub(super) fn carrier(
        &mut self,
        scalar_type: ScalarType,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if scalar_type == ScalarType::Boolean {
            return Ok(self.two);
        }
        let position = self.interned_keyed_assumption(
            |this| &mut this.carriers,
            scalar_type,
            |this| Ok(this.type_zero),
        )?;
        Ok(self.constant(position))
    }

    /// Selected Boolean computation, an applicative value-level integer
    /// comparison, or an opaque scalar constant. A closed integer
    /// term is interned by its evaluated literal — the denotation is by
    /// value, so `2 + 0` and `2` name one constant and a decided
    /// `Equal(2 + 0, 2)` is `refl`-provable rather than admitted.
    pub(super) fn scalar_term(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if let Some(boolean) = self.boolean_term(term)? {
            return Ok(boolean);
        }
        if let Some(relation) = self.integer_relation_term(term)? {
            return Ok(relation);
        }
        let key = match term.integer_value() {
            Some((integer_type, value)) => {
                ScalarTerm::integer(integer_type, value).unwrap_or_else(|_| term.clone())
            }
            None => term.clone(),
        };
        let scalar_type = key.scalar_type();
        let position = self.interned_keyed_assumption(
            |this| &mut this.terms,
            key,
            |this| this.carrier(scalar_type),
        )?;
        Ok(self.constant(position))
    }

    /// The `Type 0` carrier assumption one closed content algebra
    /// denotes — its conservation equations are identities over this
    /// carrier, interned once per algebra.
    pub(super) fn content_carrier(
        &mut self,
        algebra: &ContentAlgebra,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.interned_keyed_assumption(
            |this| &mut this.content_carriers,
            algebra.clone(),
            |this| Ok(this.type_zero),
        )?;
        Ok(self.constant(position))
    }

    /// The assumption constant one content term denotes — an element of
    /// its algebra's carrier, interned by the canonical term. Content
    /// terms are already normalized at construction, so `a ≡ a` denotes
    /// a reflexive `Id` the kernel's `refl` proves rather than an
    /// admitted atom.
    pub(super) fn content_term(
        &mut self,
        algebra: &ContentAlgebra,
        term: &ContentTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.interned_keyed_assumption(
            |this| &mut this.content_terms,
            (algebra.clone(), term.clone()),
            |this| this.content_carrier(algebra),
        )?;
        Ok(self.constant(position))
    }

    /// `Int : Type 0` — the mathematical-integer carrier assumption
    /// every `Int` element and integer relation names, pushed once.
    pub(super) fn integer(&mut self) -> Result<u32, BoundedDenotationError> {
        self.interned_slot_assumption(|this| &mut this.integer, |this| Ok(this.type_zero))
    }

    /// The `Int` carrier as a term — the `Constant` node every integer
    /// identity types its endpoints against.
    pub(super) fn integer_constant(&mut self) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.integer()?;
        Ok(self.constant(position))
    }

    /// `IntLt`/`IntLe : Π(_ : Int). Π(_ : Int). Type 0` — one shared
    /// order-relation statement shape, two positions.
    pub(super) fn integer_order_relation(&mut self) -> Result<u32, BoundedDenotationError> {
        let integer = self.integer()?;
        let ty = scheme_dsl::build(
            &mut self.arena,
            &mut Vec::new(),
            &scheme_dsl::pi(
                "_",
                scheme_dsl::scheme_at(integer, Vec::new()),
                scheme_dsl::pi(
                    "_",
                    scheme_dsl::scheme_at(integer, Vec::new()),
                    scheme_dsl::sort(Level::Constant(0)),
                ),
            ),
        );
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        Ok(position)
    }

    /// `IntLt`'s declaration position.
    pub(super) fn integer_less_than(&mut self) -> Result<u32, BoundedDenotationError> {
        self.interned_slot(
            |this| &mut this.integer_less_than,
            |this| this.integer_order_relation(),
        )
    }

    /// `IntLe`'s declaration position.
    pub(super) fn integer_less_or_equal(&mut self) -> Result<u32, BoundedDenotationError> {
        self.interned_slot(
            |this| &mut this.integer_less_or_equal,
            |this| this.integer_order_relation(),
        )
    }

    /// One roster law's assumption position — pushed with the carrier
    /// and both relation constants before it, so `check_signature`
    /// decides its exact `Π` statement under the roster prefix.
    pub(super) fn integer_law(&mut self, law: IntegerLaw) -> Result<u32, BoundedDenotationError> {
        self.roster_law_position(
            |this| &mut this.integer_laws,
            law,
            |this| -> Result<scheme_dsl::Syntax, BoundedDenotationError> {
                let integer = this.integer()?;
                let less_than = this.integer_less_than()?;
                let less_or_equal = this.integer_less_or_equal()?;
                Ok(integer_law_statement(
                    law,
                    integer,
                    less_than,
                    less_or_equal,
                ))
            },
        )
    }

    /// `c a₁ … aₙ` — the declaration at `position` applied to each
    /// argument in order.
    pub(super) fn constant_application(
        &mut self,
        position: u32,
        arguments: &[TermHandle],
    ) -> TermHandle {
        let mut function = self.constant(position);
        for &argument in arguments {
            function = self.arena.insert(Term::Apply { function, argument });
        }
        function
    }

    /// `law a₁ … aₙ` — the roster constant applied to the denoted
    /// endpoints, then the premise evidence, in statement order.
    pub(super) fn integer_law_application(
        &mut self,
        law: IntegerLaw,
        arguments: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.integer_law(law)?;
        Ok(self.constant_application(position, arguments))
    }

    /// One `Option<u32>` slot interning its own assumption position:
    /// `statement` builds the declared type only on a miss.
    pub(super) fn interned_slot_assumption(
        &mut self,
        slot: impl Fn(&mut Self) -> &mut Option<u32>,
        statement: impl FnOnce(&mut Self) -> Result<TermHandle, BoundedDenotationError>,
    ) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = *slot(self) {
            return Ok(position);
        }
        let ty = statement(self)?;
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        *slot(self) = Some(position);
        Ok(position)
    }

    /// One `Option<u32>` slot interning a position `produce` yields on a
    /// miss — the same lookup-or-compute shape when the producer already
    /// declares its own assumption.
    pub(super) fn interned_slot(
        &mut self,
        slot: impl Fn(&mut Self) -> &mut Option<u32>,
        produce: impl FnOnce(&mut Self) -> Result<u32, BoundedDenotationError>,
    ) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = *slot(self) {
            return Ok(position);
        }
        let position = produce(self)?;
        *slot(self) = Some(position);
        Ok(position)
    }

    /// One keyed position roster interning its assumption: `statement`
    /// builds the declared type only on a miss.
    pub(super) fn interned_keyed_assumption<K, M: PositionRoster<K>>(
        &mut self,
        roster: impl Fn(&mut Self) -> &mut M,
        key: K,
        statement: impl FnOnce(&mut Self) -> Result<TermHandle, BoundedDenotationError>,
    ) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = roster(self).roster_position(&key) {
            return Ok(position);
        }
        let ty = statement(self)?;
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        roster(self).record_position(key, position);
        Ok(position)
    }

    /// One operation roster law's assumption position — interned once per
    /// law; on a miss the operation's `statement` scripts the `Π`
    /// declaration which is pushed as an assumption under the roster
    /// prefix.
    pub(super) fn roster_law_position<K>(
        &mut self,
        laws: impl Fn(&mut Self) -> &mut BTreeMap<K, u32>,
        law: K,
        statement: impl FnOnce(&mut Self) -> Result<scheme_dsl::Syntax, BoundedDenotationError>,
    ) -> Result<u32, BoundedDenotationError>
    where
        K: Ord + Copy,
    {
        self.interned_keyed_assumption(laws, law, |this| {
            let statement = statement(this)?;
            Ok(scheme_dsl::build(
                &mut this.arena,
                &mut Vec::new(),
                &statement,
            ))
        })
    }

    /// `law a₁ … aₙ` — one operation-roster constant applied to the
    /// denoted endpoints and premise evidence, in statement order.
    pub(super) fn roster_law_application<K>(
        &mut self,
        laws: impl Fn(&mut Self) -> &mut BTreeMap<K, u32>,
        law: K,
        statement: impl FnOnce(&mut Self) -> Result<scheme_dsl::Syntax, BoundedDenotationError>,
        arguments: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError>
    where
        K: Ord + Copy,
    {
        let position = self.roster_law_position(laws, law, statement)?;
        Ok(self.constant_application(position, arguments))
    }

    /// The `Int`-typed declaration a mathematical term denotes —
    /// interned by its exact closed value when the shared evaluator has
    /// one, so `add(1, 1)` and `2` name one constant and a decided
    /// `IntegerMathEqual` on them is `refl`-provable. Open addition,
    /// subtraction and multiplication compose child denotations through
    /// their shared `Int` functions; other open terms intern by source
    /// structure.
    /// Fixed scalar magnitudes are binary definitions;
    /// larger closed values retain opaque assumptions so this focused
    /// discreteness encoding adds no numeric acceptance limit.
    pub(super) fn math_term(
        &mut self,
        term: &IntegerMathTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if let IntegerMathTerm::Embed(term) = term {
            return self.fixed_scalar_term(term);
        }
        let key = match ClosedIntegerEvaluator::default().evaluate_closed(term) {
            Ok(Some(value)) => MathTermKey::Closed(value),
            Ok(None) => {
                // Open addition, subtraction and multiplication share
                // their scalar functions. Retain shallow applications,
                // not cloned source-tree keys for every nested prefix.
                // Closed values still intern above.
                if let IntegerMathTerm::Subtract(left, right)
                | IntegerMathTerm::Add(left, right)
                | IntegerMathTerm::Multiply(left, right) = term
                    && ClosedIntegerEvaluator::default()
                        .evaluate_closed(left)
                        .is_ok()
                    && ClosedIntegerEvaluator::default()
                        .evaluate_closed(right)
                        .is_ok()
                {
                    let left = self.math_term(left)?;
                    let right = self.math_term(right)?;
                    return if matches!(term, IntegerMathTerm::Add(..)) {
                        self.add_terms(left, right)
                    } else if matches!(term, IntegerMathTerm::Subtract(..)) {
                        self.subtract_terms(left, right)
                    } else {
                        self.multiply_terms(left, right)
                    };
                }
                // Evaluation short-circuits on an open left child. If an
                // unvisited child would exceed its numeric work budget, keep
                // this already-admitted expression opaque as before. This
                // does not relax the whole-term preflight or a closed refusal.
                MathTermKey::Open(term.clone())
            }
            Err(error) => {
                // The same refusal the primitive judgment reports —
                // interning shares the evaluator's budget, so a term
                // too large to evaluate is refused, never approximated.
                return Err(BoundedDenotationError::Certificate(
                    ProofError::PrimitiveJudgment(KernelError::ClosedIntegerEvaluation(error)),
                ));
            }
        };
        if let Some(&position) = self.math_terms.get(&key) {
            return Ok(self.constant(position));
        }
        if let MathTermKey::Closed(value) = &key
            && let Some((negative, magnitude)) = binary_numerals::fixed_magnitude(value)
        {
            let position = self.binary_integer(negative, magnitude)?;
            self.closed_math_values.insert(position, value.clone());
            self.math_terms.insert(key, position);
            return Ok(self.constant(position));
        }
        let ty = self.integer_constant()?;
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        if let MathTermKey::Closed(value) = &key {
            self.closed_math_values.insert(position, value.clone());
        }
        self.math_terms.insert(key, position);
        Ok(self.constant(position))
    }

    /// The exact closed value an interned `Int` constant denotes — the
    /// reverse of `math_terms`' `MathTermKey::Closed` interning, so the
    /// decided value behind a denoted endpoint is readable without
    /// re-evaluating. An open leaf, an applicative composition or any
    /// non-`Int` constant has no decided value here, so the closed
    /// relation crossing never guesses at what it denotes.
    pub(super) fn closed_math_value(&self, term: TermHandle) -> Option<BigInt> {
        let Term::Constant { declaration, .. } = self.arena.get(term) else {
            return None;
        };
        self.closed_math_values.get(&declaration).cloned()
    }

    /// Keep one carrier for fixed scalar equations and orders, including
    /// mixed symbolic/compound endpoints. Every fixed-integer operation
    /// denotes its uninterpreted function applied to the operand terms —
    /// exact addition, subtraction and multiplication additionally carry
    /// their fixed arithmetic laws — while field projections and
    /// unclassified constructors stay per-term opaque.
    pub(super) fn fixed_scalar_term(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let denoted = if let Some((_, value)) = term.integer_value() {
            self.math_term(&IntegerMathTerm::literal(value))?
        } else {
            match term {
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(source_type),
                } => self.math_term(&IntegerMathTerm::MathValue {
                    source_type: *source_type,
                    value: *id,
                })?,
                ScalarTerm::ExactIntegerSubtract { left, right, .. }
                | ScalarTerm::ExactIntegerAdd { left, right, .. }
                | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
                    let left = self.fixed_scalar_term(left)?;
                    let right = self.fixed_scalar_term(right)?;
                    if matches!(term, ScalarTerm::ExactIntegerAdd { .. }) {
                        self.add_terms(left, right)?
                    } else if matches!(term, ScalarTerm::ExactIntegerSubtract { .. }) {
                        self.subtract_terms(left, right)?
                    } else {
                        self.multiply_terms(left, right)?
                    }
                }
                _ => {
                    if let Some(applied) = self.integer_operation_term(term)? {
                        return Ok(applied);
                    }
                    let position = self.interned_keyed_assumption(
                        |this| &mut this.scalar_integer_terms,
                        term.clone(),
                        |this| this.integer_constant(),
                    )?;
                    self.constant(position)
                }
            }
        };
        Ok(denoted)
    }

    pub(super) fn fixed_scalar_relation(
        &mut self,
        proposition: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (left, right) = match proposition {
            semantic_vocabulary::ordered_comparison_operands!(left, right) => (left, right),
            _ => return Ok(None),
        };
        let fixed_integer = |term: &ScalarTerm| matches!(term.scalar_type(), ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed && !integer.is_address());
        if !fixed_integer(left) || !fixed_integer(right) {
            return Ok(None);
        }
        let left = self.fixed_scalar_term(left)?;
        let right = self.fixed_scalar_term(right)?;
        let denoted = if matches!(proposition, Proposition::Equal(..)) {
            let ty = self.integer_constant()?;
            self.arena.insert(Term::Id { ty, left, right })
        } else {
            let relation = if matches!(proposition, Proposition::LessThan(..)) {
                self.integer_less_than()?
            } else {
                self.integer_less_or_equal()?
            };
            let relation = self.constant(relation);
            let function = self.arena.insert(Term::Apply {
                function: relation,
                argument: left,
            });
            self.arena.insert(Term::Apply {
                function,
                argument: right,
            })
        };
        Ok(Some(denoted))
    }

    /// `IntLt l r` or `IntLe l r` — the chosen order constant applied to
    /// two already-interned endpoints.
    pub(super) fn order_term_application(
        &mut self,
        strict: bool,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = if strict {
            self.integer_less_than()?
        } else {
            self.integer_less_or_equal()?
        };
        let relation = self.constant(position);
        Ok(self.arena.apply_binary(relation, left, right))
    }

    /// `IntLt l r` or `IntLe l r` — the relation constant applied to the
    /// two interned `Int` endpoints.
    pub(super) fn integer_order_term(
        &mut self,
        strict: bool,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let left = self.math_term(left)?;
        let right = self.math_term(right)?;
        self.order_term_application(strict, left, right)
    }

    /// The denoted head and endpoints of a proposition in the integer
    /// vocabulary — `Id Int`, `IntLt` or `IntLe` — or `None` when
    /// `denoted` lives in another vocabulary (a scalar carrier's `Id`,
    /// an atom, a connective). Read-only: matching never creates a
    /// declaration, so a partially-`Int` instance leaves the vocabulary
    /// unused rather than pushed but uncited.
    pub(super) fn integer_relation(&self, denoted: TermHandle) -> Option<IntegerRelation> {
        match self.arena.get(denoted) {
            Term::Id { ty, left, right } => match self.arena.get(ty) {
                Term::Constant { declaration, .. } if Some(declaration) == self.integer => {
                    Some(IntegerRelation::Equality { left, right })
                }
                _ => None,
            },
            Term::Apply {
                function,
                argument: right,
            } => match self.arena.get(function) {
                Term::Apply {
                    function: relation,
                    argument: left,
                } => match self.arena.get(relation) {
                    Term::Constant { declaration, .. }
                        if Some(declaration) == self.integer_less_than =>
                    {
                        Some(IntegerRelation::LessThan { left, right })
                    }
                    Term::Constant { declaration, .. }
                        if Some(declaration) == self.integer_less_or_equal =>
                    {
                        Some(IntegerRelation::LessOrEqual { left, right })
                    }
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }

    /// The `(carrier, left, right)` of an `Id`-denoted proposition, or
    /// `None` for any other denotation shape.
    pub(super) fn identity_parts(
        &self,
        denoted: TermHandle,
    ) -> Option<(TermHandle, TermHandle, TermHandle)> {
        if let Term::Id { ty, left, right } = self.arena.get(denoted) {
            Some((ty, left, right))
        } else {
            None
        }
    }

    /// `sym p` — `J(λ(y : C). λ(_ : Id C a y). Id C y a, refl C a, b, p)`
    /// mapping `p : Id C a b` to `Id C b a`, over whichever carrier the
    /// denoted identity names.
    pub(super) fn symmetry(
        &mut self,
        ty: TermHandle,
        from: TermHandle,
        to: TermHandle,
        proof: TermHandle,
    ) -> TermHandle {
        let bound = self.arena.insert(Term::Variable(0));
        let identity = self.arena.insert(Term::Id {
            ty,
            left: from,
            right: bound,
        });
        let flipped = self.arena.insert(Term::Variable(1));
        let body = self.arena.insert(Term::Id {
            ty,
            left: flipped,
            right: from,
        });
        let inner = self.arena.insert(Term::Lambda {
            domain: identity,
            body,
        });
        let motive = self.arena.insert(Term::Lambda {
            domain: ty,
            body: inner,
        });
        let base = self.arena.insert(Term::Refl { ty, value: from });
        self.arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: to,
            proof,
        })
    }

    /// `q ∘ p` — `J(λ(y : C). λ(_ : Id C m y). Id C l y, p, r, q)`
    /// composing `p : Id C l m` and `q : Id C m r` to `Id C l r`, over
    /// whichever carrier the denoted identities share.
    pub(super) fn transitivity(
        &mut self,
        ty: TermHandle,
        left: TermHandle,
        middle: TermHandle,
        right: TermHandle,
        first: TermHandle,
        second: TermHandle,
    ) -> TermHandle {
        let bound = self.arena.insert(Term::Variable(0));
        let identity = self.arena.insert(Term::Id {
            ty,
            left: middle,
            right: bound,
        });
        let endpoint = self.arena.insert(Term::Variable(1));
        let body = self.arena.insert(Term::Id {
            ty,
            left,
            right: endpoint,
        });
        let inner = self.arena.insert(Term::Lambda {
            domain: identity,
            body,
        });
        let motive = self.arena.insert(Term::Lambda {
            domain: ty,
            body: inner,
        });
        self.arena.insert(Term::IdElim {
            motive,
            base: first,
            endpoint: right,
            proof: second,
        })
    }

    /// Denoted symmetry evidence: the licensed premise and goal share
    /// one `Id` carrier — canonical `IntegerMathEqual` order may already
    /// identify them (the premise evidence stands alone), or read them
    /// swapped (a `sym` `J`); any other denotation stays a per-instance
    /// axiom.
    pub(super) fn symmetry_evidence(
        &mut self,
        premise: TermHandle,
        goal: TermHandle,
        evidence: TermHandle,
    ) -> Option<TermHandle> {
        if self.arena.structurally_equal(premise, goal) {
            return Some(evidence);
        }
        let (carrier, left, right) = self.identity_parts(premise)?;
        let (goal_carrier, goal_left, goal_right) = self.identity_parts(goal)?;
        (self.arena.structurally_equal(carrier, goal_carrier)
            && self.arena.structurally_equal(goal_left, right)
            && self.arena.structurally_equal(goal_right, left))
        .then(|| self.symmetry(carrier, left, right, evidence))
    }

    /// `p₂ ∘ p₁` over the denoted identities: the shared transitivity
    /// check licensed the chain, so the denoted premises share a carrier
    /// and one endpoint — the canonical `IntegerMathEqual` order decides
    /// which side of each premise the shared endpoint sits on, and a
    /// `sym` `J` reorients whichever premise needs it. `None` keeps the
    /// per-instance axiom for non-`Id` denotations.
    pub(super) fn transitivity_evidence(
        &mut self,
        first_ty: TermHandle,
        first: TermHandle,
        second_ty: TermHandle,
        second: TermHandle,
        goal_ty: TermHandle,
    ) -> Option<TermHandle> {
        let (first_carrier, a, b) = self.identity_parts(first_ty)?;
        let (second_carrier, c, d) = self.identity_parts(second_ty)?;
        let (goal_carrier, e, f) = self.identity_parts(goal_ty)?;
        if !(self.arena.structurally_equal(first_carrier, second_carrier)
            && self.arena.structurally_equal(first_carrier, goal_carrier))
        {
            return None;
        }
        let (composed, left, right) = if self.arena.structurally_equal(b, c) {
            (
                self.transitivity(first_carrier, a, b, d, first, second),
                a,
                d,
            )
        } else if self.arena.structurally_equal(b, d) {
            let second = self.symmetry(second_carrier, c, d, second);
            (
                self.transitivity(first_carrier, a, b, c, first, second),
                a,
                c,
            )
        } else if self.arena.structurally_equal(a, c) {
            let first = self.symmetry(first_carrier, a, b, first);
            (
                self.transitivity(first_carrier, b, a, d, first, second),
                b,
                d,
            )
        } else if self.arena.structurally_equal(a, d) {
            let first = self.symmetry(first_carrier, a, b, first);
            let second = self.symmetry(second_carrier, c, d, second);
            (
                self.transitivity(first_carrier, b, a, c, first, second),
                b,
                c,
            )
        } else {
            return None;
        };
        if self.arena.structurally_equal(e, left) && self.arena.structurally_equal(f, right) {
            Some(composed)
        } else if self.arena.structurally_equal(e, right) && self.arena.structurally_equal(f, left)
        {
            Some(self.symmetry(goal_carrier, left, right, composed))
        } else {
            None
        }
    }

    /// `eq_le`/`lt_le` on the denoted relations for one
    /// `IntegerOrderWeakening` instance: `Id Int` evidence is sym'd into
    /// the conclusion's endpoint order when canonical order flipped it.
    /// `None` leaves the per-instance axiom.
    pub(super) fn weakening_evidence(
        &mut self,
        premise_ty: TermHandle,
        evidence: TermHandle,
        goal_ty: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Some(IntegerRelation::LessOrEqual { left, right }) = self.integer_relation(goal_ty)
        else {
            return Ok(None);
        };
        match self.integer_relation(premise_ty) {
            Some(IntegerRelation::LessThan { left: a, right: b })
                if self.arena.structurally_equal(a, left)
                    && self.arena.structurally_equal(b, right) =>
            {
                self.integer_law_application(IntegerLaw::LessThanToLessOrEqual, &[a, b, evidence])
                    .map(Some)
            }
            Some(IntegerRelation::Equality { left: a, right: b })
                if self.arena.structurally_equal(a, left)
                    && self.arena.structurally_equal(b, right) =>
            {
                self.integer_law_application(IntegerLaw::EqualityToLessOrEqual, &[a, b, evidence])
                    .map(Some)
            }
            Some(IntegerRelation::Equality { left: a, right: b })
                if self.arena.structurally_equal(a, right)
                    && self.arena.structurally_equal(b, left) =>
            {
                let integer = self.integer_constant()?;
                let evidence = self.symmetry(integer, a, b, evidence);
                self.integer_law_application(IntegerLaw::EqualityToLessOrEqual, &[b, a, evidence])
                    .map(Some)
            }
            _ => Ok(None),
        }
    }

    /// The transitivity law on the denoted `IntLt`/`IntLe` chain — the
    /// shared check already fixed the exact middle and endpoints, so
    /// matching denotations is a lookup, and `None` leaves the
    /// per-instance axiom for any instance outside the vocabulary.
    pub(super) fn order_transitivity_evidence(
        &mut self,
        first_ty: TermHandle,
        first: TermHandle,
        second_ty: TermHandle,
        second: TermHandle,
        goal_ty: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        fn strictness(relation: IntegerRelation) -> Option<(bool, TermHandle, TermHandle)> {
            match relation {
                IntegerRelation::LessThan { left, right } => Some((true, left, right)),
                IntegerRelation::LessOrEqual { left, right } => Some((false, left, right)),
                IntegerRelation::Equality { .. } => None,
            }
        }
        let Some((first_strict, a, b)) = self.integer_relation(first_ty).and_then(strictness)
        else {
            return Ok(None);
        };
        let Some((second_strict, middle, c)) =
            self.integer_relation(second_ty).and_then(strictness)
        else {
            return Ok(None);
        };
        let Some((goal_strict, goal_left, goal_right)) =
            self.integer_relation(goal_ty).and_then(strictness)
        else {
            return Ok(None);
        };
        let law = match (first_strict, second_strict, goal_strict) {
            (false, false, false) => IntegerLaw::LessOrEqualTransitivity,
            (true, true, true) => IntegerLaw::LessThanTransitivity,
            (true, false, true) => IntegerLaw::LessThanLessOrEqualTransitivity,
            (false, true, true) => IntegerLaw::LessOrEqualLessThanTransitivity,
            _ => return Ok(None),
        };
        if !(self.arena.structurally_equal(b, middle)
            && self.arena.structurally_equal(a, goal_left)
            && self.arena.structurally_equal(c, goal_right))
        {
            return Ok(None);
        }
        self.integer_law_application(law, &[a, b, c, first, second])
            .map(Some)
    }

    /// The `R`-substitution law for one `IntegerOrderSubstitution`
    /// instance: the relation and conclusion agree on strictness and the
    /// unchanged endpoint, and the `Id` evidence reads old→new or its
    /// `sym` flip. `None` leaves the per-instance axiom.
    pub(super) fn order_substitution_evidence(
        &mut self,
        relation_ty: TermHandle,
        relation_evidence: TermHandle,
        equality_ty: TermHandle,
        equality_evidence: TermHandle,
        endpoint: usize,
        goal_ty: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (strict, relation_left, relation_right, substitute_left, substitute_right) =
            match self.integer_relation(relation_ty) {
                Some(IntegerRelation::LessThan { left, right }) => (
                    true,
                    left,
                    right,
                    IntegerLaw::LessThanSubstituteLeft,
                    IntegerLaw::LessThanSubstituteRight,
                ),
                Some(IntegerRelation::LessOrEqual { left, right }) => (
                    false,
                    left,
                    right,
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    IntegerLaw::LessOrEqualSubstituteRight,
                ),
                _ => return Ok(None),
            };
        let (goal_left, goal_right) = match self.integer_relation(goal_ty) {
            Some(IntegerRelation::LessThan { left, right }) if strict => (left, right),
            Some(IntegerRelation::LessOrEqual { left, right }) if !strict => (left, right),
            _ => return Ok(None),
        };
        let Some(IntegerRelation::Equality { left: s, right: t }) =
            self.integer_relation(equality_ty)
        else {
            return Ok(None);
        };
        // The law's (x, y, z) quantifier order, the replaced endpoint's
        // old→new pair, and the endpoint arguments in statement order.
        // The other endpoint never moves — denoted equality is canonical,
        // so the `Id` evidence may read new→old and take a `sym` flip.
        let (law, old, new, endpoints) = match endpoint {
            0 if self.arena.structurally_equal(relation_right, goal_right) => (
                substitute_left,
                relation_left,
                goal_left,
                [relation_left, goal_left, relation_right],
            ),
            1 if self.arena.structurally_equal(relation_left, goal_left) => (
                substitute_right,
                relation_right,
                goal_right,
                [relation_left, relation_right, goal_right],
            ),
            _ => return Ok(None),
        };
        let equality_evidence = if self.arena.structurally_equal(s, old)
            && self.arena.structurally_equal(t, new)
        {
            equality_evidence
        } else if self.arena.structurally_equal(s, new) && self.arena.structurally_equal(t, old) {
            let integer = self.integer_constant()?;
            self.symmetry(integer, s, t, equality_evidence)
        } else {
            return Ok(None);
        };
        self.integer_law_application(
            law,
            &[
                endpoints[0],
                endpoints[1],
                endpoints[2],
                equality_evidence,
                relation_evidence,
            ],
        )
        .map(Some)
    }

    /// The equation evidence oriented as `Id Int old new`, or `None`
    /// when its denoted endpoints are not `{old, new}` in either order.
    pub(super) fn directed_equality(
        &mut self,
        s: TermHandle,
        t: TermHandle,
        old: TermHandle,
        new: TermHandle,
        evidence: TermHandle,
    ) -> Option<TermHandle> {
        if self.arena.structurally_equal(s, old) && self.arena.structurally_equal(t, new) {
            Some(evidence)
        } else if self.arena.structurally_equal(s, new) && self.arena.structurally_equal(t, old) {
            let integer = self.integer_constant().ok()?;
            Some(self.symmetry(integer, s, t, evidence))
        } else {
            None
        }
    }

    /// The named decision axiom for one bounded rule instance: an
    /// assumption constant of type `Π(_ : ⟦premise₁⟧). … . ⟦conclusion⟧`.
    /// The premise/conclusion relation itself has already been re-decided
    /// by the shared check; the constant records the rule's arithmetic or
    /// conversion content so the judgment's closure names the instance
    /// exactly. Equal instances intern to one constant.
    pub(super) fn rule_axiom(
        &mut self,
        premises: &[Proposition],
        conclusion: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.interned_keyed_assumption(
            |this| &mut this.rule_axioms,
            (premises.to_vec(), conclusion.clone()),
            |this| {
                let mut ty = this.denote(conclusion)?;
                for premise in premises.iter().rev() {
                    let domain = this.denote(premise)?;
                    ty = this.arena.insert(Term::Pi {
                        domain,
                        codomain: ty,
                    });
                }
                Ok(ty)
            },
        )?;
        Ok(self.constant(position))
    }

    /// Record a `Primitive` decision the certificate commits to as a named
    /// assumption of the leaf's denoted statement, so the judgment's
    /// assumption closure names it exactly.
    pub(super) fn decision(
        &mut self,
        ty: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position =
            self.interned_keyed_assumption(|this| &mut this.decisions, ty, |_| Ok(ty))?;
        Ok(self.constant(position))
    }

    /// The correlated-affine root-equality step: direct the cited equality
    /// at the witness root's expression and substitute it into the bound.
    /// `None` means give up; `Some(term)` is the extended evidence.
    pub(super) fn affine_root_equality_step(
        &mut self,
        lower: bool,
        witness: &crate::IntegerAffineWitness,
        left_term: TermHandle,
        expression_term: TermHandle,
        equality: Option<(TermHandle, &Proposition)>,
        evidence: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Some((equality, proposition)) = equality else {
            return Ok(Some(evidence));
        };
        let root_term = self.fixed_scalar_term(&witness.root)?;
        let denoted = self.denote(proposition)?;
        let Some((_, from, to)) = self.identity_parts(denoted) else {
            return Ok(None);
        };
        let Some(equality) = self.directed_equality(from, to, root_term, expression_term, equality)
        else {
            return Ok(None);
        };
        Ok(Some(if lower {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteLeft,
                &[root_term, expression_term, left_term, equality, evidence],
            )?
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[left_term, root_term, expression_term, equality, evidence],
            )?
        }))
    }

    /// The correlated-affine endpoint-equality tail: direct recorded
    /// equality evidence at the endpoint literal, then substitute it
    /// through `swapped_term` (evaluated lazily inside the guard).
    pub(super) fn affine_endpoint_equality_order(
        &mut self,
        lower: bool,
        endpoint_equality: Option<(TermHandle, &Proposition)>,
        endpoint_literal: &ScalarTerm,
        endpoint_term: TermHandle,
        order: TermHandle,
        swapped_term: impl Fn(&mut Self) -> Result<TermHandle, BoundedDenotationError>,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Some((equality, proposition)) = endpoint_equality else {
            return Ok(Some(order));
        };
        let literal = self.fixed_scalar_term(endpoint_literal)?;
        let swapped = swapped_term(self)?;
        let denoted = self.denote(proposition)?;
        let Some((_, from, to)) = self.identity_parts(denoted) else {
            return Ok(None);
        };
        let Some(equality) = self.directed_equality(from, to, endpoint_term, literal, equality)
        else {
            return Ok(None);
        };
        if lower {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteLeft,
                &[endpoint_term, literal, swapped, equality, order],
            )
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[swapped, endpoint_term, literal, equality, order],
            )
        }
        .map(Some)
    }

    pub(super) fn denote(
        &mut self,
        proposition: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if let Some(&cached) = self.denotations.get(proposition) {
            return Ok(cached);
        }
        let term = self.denote_uncached(proposition)?;
        self.denotations.insert(proposition.clone(), term);
        Ok(term)
    }

    pub(super) fn denote_uncached(
        &mut self,
        proposition: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        Ok(match proposition {
            Proposition::Truth => {
                let ty = self.two;
                let zero = self.two_zero;
                self.arena.insert(Term::Id {
                    ty,
                    left: zero,
                    right: zero,
                })
            }
            Proposition::Falsehood => {
                let ty = self.two;
                let left = self.two_zero;
                let right = self.two_one;
                self.arena.insert(Term::Id { ty, left, right })
            }
            Proposition::Equal(..) | Proposition::LessThan(..) | Proposition::LessOrEqual(..) => {
                // A relation whose integer operands both lift denotes
                // the `Int` vocabulary — `Id Int` for `Equal`, the
                // `IntLt`/`IntLe` application for the orders — so fixed
                // and `IntegerMath*` forms share one type. Compound fixed
                // scalar operands also use Int, with law-carrying
                // addition/subtraction and applicative uninterpreted other
                // open operations. This does
                // not broaden the bounded premise matcher's lift. Other
                // carriers retain `Id S` equality and atomic orders.
                if let Some(lifted) = lift_fixed_integer_relation(proposition) {
                    self.denote(&lifted)?
                } else if let Some(denoted) = self.fixed_scalar_relation(proposition)? {
                    denoted
                } else if let Proposition::Equal(left, right) = proposition {
                    let ty = self.carrier(left.scalar_type())?;
                    let left = self.scalar_term(left)?;
                    let right = self.scalar_term(right)?;
                    self.arena.insert(Term::Id { ty, left, right })
                } else {
                    self.atom(proposition)?
                }
            }
            Proposition::IntegerMathEqual(left, right) => {
                let left = self.math_term(left)?;
                let right = self.math_term(right)?;
                let ty = self.integer_constant()?;
                self.arena.insert(Term::Id { ty, left, right })
            }
            Proposition::IntegerMathLessThan(left, right) => {
                self.integer_order_term(true, left, right)?
            }
            Proposition::IntegerMathLessOrEqual(left, right) => {
                self.integer_order_term(false, left, right)?
            }
            Proposition::Conjunction(conjuncts) => self.conjunction(conjuncts)?,
            Proposition::Disjunction(disjuncts) => self.disjunction(disjuncts)?,
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                let domain = self.denote(premise)?;
                let codomain = self.denote(conclusion)?;
                self.arena.insert(Term::Pi { domain, codomain })
            }
            Proposition::ContentConservation(conservation) => {
                // `l ≡ r` in one algebra denotes `Id` over that
                // algebra's carrier: conservation is the algebra's own
                // equality, so its transitivity and reflexivity are the
                // kernel's `J` and `refl`, not rule-instance axioms.
                let ty = self.content_carrier(conservation.algebra())?;
                let left = self.content_term(conservation.algebra(), conservation.left())?;
                let right = self.content_term(conservation.algebra(), conservation.right())?;
                self.arena.insert(Term::Id { ty, left, right })
            }
            atomic => self.atom(atomic)?,
        })
    }

    /// `⟦c₀ ∧ … ∧ cₙ₋₁⟧` — a right-nested `Σ`, so conjunction
    /// introduction is a nested pair and elimination a projection path.
    pub(super) fn conjunction(
        &mut self,
        conjuncts: &[Proposition],
    ) -> Result<TermHandle, BoundedDenotationError> {
        if conjuncts.len() < 2 {
            // `Proposition::validate` rejects non-canonical arity before
            // denotation; this stays a refusal, never a judgment.
            return Err(BoundedDenotationError::Certificate(
                ProofError::RuleConclusionMismatch("conjunction arity"),
            ));
        }
        let domain = self.denote(&conjuncts[0])?;
        let codomain = if conjuncts.len() == 2 {
            self.denote(&conjuncts[1])?
        } else {
            self.conjunction(&conjuncts[1..])?
        };
        Ok(self.arena.insert(Term::Sigma { domain, codomain }))
    }

    /// `⟦d₀ ∨ … ∨ dₙ₋₁⟧` — the tagged sum `Σ(t : Two). caseTwo(M, d₀,
    /// rest, t)` with `rest` the nested disjunction, so a disjunction
    /// introduction is a `⟨zero, …⟩`/`⟨one, …⟩` pair and an elimination a
    /// dependent `caseTwo`.
    pub(super) fn disjunction(
        &mut self,
        disjuncts: &[Proposition],
    ) -> Result<TermHandle, BoundedDenotationError> {
        if disjuncts.len() < 2 {
            return Err(BoundedDenotationError::Certificate(
                ProofError::RuleConclusionMismatch("disjunction arity"),
            ));
        }
        let zero_case = self.denote(&disjuncts[0])?;
        let one_case = if disjuncts.len() == 2 {
            self.denote(&disjuncts[1])?
        } else {
            self.disjunction(&disjuncts[1..])?
        };
        Ok(self.tagged_sum(zero_case, one_case))
    }

    /// `Σ(t : Two). caseTwo(λ(_ : Two). Type 0, zero_case, one_case, t)`
    /// — the binary coproduct shape the disjunction denotation nests.
    pub(super) fn tagged_sum(&mut self, zero_case: TermHandle, one_case: TermHandle) -> TermHandle {
        let motive = self.arena.insert(Term::Lambda {
            domain: self.two,
            body: self.type_zero,
        });
        let bound = self.arena.insert(Term::Variable(0));
        let family = self.arena.insert(Term::CaseTwo {
            motive,
            zero_branch: zero_case,
            one_branch: one_case,
            scrutinee: bound,
        });
        self.arena.insert(Term::Sigma {
            domain: self.two,
            codomain: family,
        })
    }
}
