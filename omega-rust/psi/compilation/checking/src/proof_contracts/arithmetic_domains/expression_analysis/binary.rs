//! Binary operators under decision 17's operand-driven domains.
//!
//! [`analyze`] reads as the rule sequence: operands first, then a declared
//! (non-builtin) operator's signature, anonymous operand landing, the Boolean
//! result of comparisons, the rejections that need no range proof (a provably
//! zero divisor, meaningless address arithmetic, S2 domain mixing), the
//! mathematical interval, the S3 Exact representability obligation, the F8
//! shift-count obligation, and finally the interval a parent observes after the
//! operation's domain policy is applied.

use super::unsigned_constants::bitwise_known_unsigned;
use super::{Analysis, BOOLEAN_INTERVAL, ExpressionWalk};
use crate::proof_contracts::arithmetic_domains::dependent_products::{
    refine_dependent_product, refine_dependent_product_factor,
};
use crate::proof_contracts::arithmetic_domains::dependent_relations::refine_dependent_subtract;
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    exact_unsigned_widened_multiply_fits, integer_bit_width, literal_interval, primitive_name,
    primitive_range, u64_exact_shift_left_fits, validate_anonymous_integer_primitive_range,
};
use crate::proof_contracts::arithmetic_domains::operand_reports::arithmetic_operator_spelling;
use crate::proof_contracts::arithmetic_domains::return_ranges::overflow_operand_value_call_target;
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, BinaryOperator, Diagnostic, ExpressionHandle, ExpressionNode, Interval,
    PrimitiveType, bitwise, division_definedness, guard_narrowing, is_arithmetic, ordered_values,
    unsigned_representability,
};
use crate::value_custody::literals;
use numerics::integer_policy::{
    IntegerFormationCondition, IntegerPolicyBridge, IntegerPolicyPrimitive, IntegerTrapPredicate,
    ShiftCountLaw, integer_policy_bridge,
};
use typed_trees::expression::TableBinaryExpression;

pub(super) fn analyze(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    binary: &TableBinaryExpression,
    diagnostics: &mut Vec<Diagnostic>,
    decisions: &mut super::ArithmeticDecisions,
) -> Analysis {
    let mut operation =
        Operation::analyze_operands(walk, expression, binary, diagnostics, decisions);
    if !crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning(
        walk.program,
        walk.machine,
        walk.state,
        expression,
    ) {
        return operation.declared_operator_result();
    }
    crate::proof_contracts::arithmetic_domains::report_mismatched_integer_operands(
        walk.program,
        walk.machine,
        walk.state,
        binary.operator,
        binary.left,
        binary.right,
        diagnostics,
    );
    if !operation.is_shift() {
        operation.land_anonymous_operands(diagnostics);
    }
    let operator = binary.operator;
    let bitwise = matches!(
        operator,
        BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor
    );
    if !is_arithmetic(operator) && !bitwise {
        // Comparison / logical `and`/`or`: a `bool` whose integer value is 0
        // or 1, like `!`. Its interval is [0, 1] (NOT unbounded) so it does not
        // poison an enclosing arithmetic op, and its `bool` primitive lets an
        // explicit `(a == b) as i32` keep that interval. No arithmetic domain.
        return Analysis {
            recipe: None,
            domain: None,
            interval: BOOLEAN_INTERVAL,
            primitive: Some(PrimitiveType::Bool),
        };
    }
    operation.reject_provably_zero_divisor(diagnostics);
    operation.reject_address_arithmetic(diagnostics);
    let domain = operation.operand_domain(diagnostics);
    // Representation operations retain operand width and policy, but have no
    // arithmetic overflow condition of their own. In particular their result
    // is not a Boolean bound for surrounding arithmetic.
    if bitwise {
        return operation.bitwise_result(domain);
    }
    let mut interval = operation.mathematical_interval();
    let primitive = operation.result_primitive(interval);
    let effective_domain = domain.unwrap_or(ArithmeticDomain::Exact);
    if let Some(primitive) = primitive
        && crate::proof_contracts::arithmetic_domains::division_definedness::validate_operands(
            walk.program,
            walk.machine,
            walk.state,
            walk.environment,
            binary,
            effective_domain,
            primitive,
            operation.left.interval,
            operation.right.interval,
            walk.owner,
            diagnostics,
        )
    {
        decisions.decided_formations.push(expression);
    }
    let mut exact_result_proven = false;
    if effective_domain == ArithmeticDomain::Exact {
        if operation.cancels_to_zero() {
            // Both children were checked above. Cancellation establishes this
            // result, never the safety of an overflowing child.
            interval = Interval {
                low: Some(0),
                high: Some(0),
            };
        }
        // A relational representability proof adds a carrier bound; it must
        // not discard tighter bounds already proved for the result.
        if let Some(primitive) = primitive
            && let Some(range) = primitive_range(primitive)
            && (operation.result_fits(primitive, range, interval, false)
                || operation.relationally_fits_carrier(Some(primitive)))
        {
            interval = interval.intersect(range);
            exact_result_proven = true;
        }
    }
    let policy_bridge = integer_policy_primitive(operator)
        .map(|primitive| integer_policy_bridge(primitive, effective_domain));
    let mut result_representable = false;
    let recipe = if effective_domain == ArithmeticDomain::Exact {
        operation
            .left
            .range_recipe(walk.program, walk.machine.symbol, binary.left)
            .zip(
                operation
                    .right
                    .range_recipe(walk.program, walk.machine.symbol, binary.right),
            )
            .and_then(|(left, right)| left.binary(operator, &right))
            .filter(checked_trees::IntegerRangeRecipe::is_target_dependent)
    } else {
        None
    };
    if let Some(bridge) = policy_bridge
        && let Some(primitive) = primitive
    {
        operation.warn_unconditional_trap(bridge, primitive, interval, diagnostics);
        let representable = operation.require_representable_result(
            bridge,
            primitive,
            interval,
            exact_result_proven,
            recipe.as_ref(),
            decisions,
            diagnostics,
        );
        result_representable = representable;
        // A shift also owes its count bound, decided below; record only the
        // sum, difference and product whose one condition this proved.
        if representable
            && matches!(
                operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            )
        {
            decisions.decided_formations.push(expression);
        }
    }
    if operation.is_shift()
        && let Some(bridge) = policy_bridge
    {
        let count_proven =
            operation.require_shift_count_within_width(bridge, effective_domain, diagnostics);
        if count_proven
            && (operator == BinaryOperator::ShiftRight
                || effective_domain == ArithmeticDomain::Saturating
                || result_representable)
        {
            decisions.decided_formations.push(expression);
        }
    }
    if effective_domain == ArithmeticDomain::Trapping
        && let Some(policy_primitive) = integer_policy_primitive(operator)
        && let Some(primitive) = primitive.filter(|primitive| primitive_range(*primitive).is_some())
    {
        let trap_predicates =
            integer_policy_bridge(policy_primitive, ArithmeticDomain::Trapping).trap_predicates;
        walk.require_trap_coverage(
            expression,
            || operation.forms_exactly(policy_primitive, primitive, interval),
            |covering| Operation::forms_exactly_under(covering, expression, binary),
            || {
                format!(
                    "Trapping `{}` of `{}` and `{}` in `{}` traps when {}",
                    arithmetic_operator_spelling(operator),
                    walk.operand_label(binary.left),
                    walk.operand_label(binary.right),
                    primitive_name(primitive),
                    trap_condition(trap_predicates, primitive),
                )
            },
            diagnostics,
        );
    }
    Analysis {
        recipe,
        domain,
        interval: operation.policy_result_interval(effective_domain, primitive, interval),
        primitive,
    }
}

/// What makes a Trapping integer primitive trap, for an uncovered-site
/// diagnostic. The catalog row, not the operator spelling, names the cases.
fn trap_condition(trap_predicates: &[IntegerTrapPredicate], primitive: PrimitiveType) -> String {
    trap_predicates
        .iter()
        .map(|predicate| match predicate {
            IntegerTrapPredicate::ResultOutsideCarrier => {
                format!("its result lies outside `{}`", primitive_name(primitive))
            }
            IntegerTrapPredicate::ZeroDivisor => "its divisor is zero".to_owned(),
            IntegerTrapPredicate::SignedMinimumDividedByNegativeOne => {
                format!(
                    "it divides the `{}` minimum by `-1`",
                    primitive_name(primitive)
                )
            }
            IntegerTrapPredicate::ShiftCountOutOfRange => format!(
                "its shift count is not below the `{}` width",
                primitive_name(primitive)
            ),
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

fn integer_policy_primitive(operator: BinaryOperator) -> Option<IntegerPolicyPrimitive> {
    match operator {
        BinaryOperator::Add => Some(IntegerPolicyPrimitive::Add),
        BinaryOperator::Subtract => Some(IntegerPolicyPrimitive::Subtract),
        BinaryOperator::Multiply => Some(IntegerPolicyPrimitive::Multiply),
        BinaryOperator::Divide => Some(IntegerPolicyPrimitive::Divide),
        BinaryOperator::Modulo => Some(IntegerPolicyPrimitive::Remainder),
        BinaryOperator::ShiftLeft => Some(IntegerPolicyPrimitive::ShiftLeft),
        BinaryOperator::ShiftRight => Some(IntegerPolicyPrimitive::ShiftRight),
        _ => None,
    }
}

/// One binary node with its analysed operands.
struct Operation<'w, 'a> {
    walk: &'w ExpressionWalk<'a>,
    expression: ExpressionHandle,
    binary: TableBinaryExpression,
    left: Analysis,
    right: Analysis,
}

impl<'w, 'a> Operation<'w, 'a> {
    /// Analyse both operands. The right operand of a builtin `&&`/`||` sees
    /// the facts its short-circuit evaluation establishes.
    fn analyze_operands(
        walk: &'w ExpressionWalk<'a>,
        expression: ExpressionHandle,
        binary: &TableBinaryExpression,
        diagnostics: &mut Vec<Diagnostic>,
        decisions: &mut super::ArithmeticDecisions,
    ) -> Self {
        let left = walk.analyze(binary.left, diagnostics, decisions);
        let right_environment = guard_narrowing::short_circuit_operand_environment(
            walk.program,
            walk.machine,
            walk.state,
            expression,
            walk.environment,
        );
        let right_walk = ExpressionWalk {
            environment: right_environment.as_ref().unwrap_or(walk.environment),
            ..*walk
        };
        Operation {
            walk,
            expression,
            binary: *binary,
            left,
            right: right_walk.analyze(binary.right, diagnostics, decisions),
        }
    }
}

impl Operation<'_, '_> {
    fn operator(&self) -> BinaryOperator {
        self.binary.operator
    }

    /// The carrier the operation computes in. Operand primitives win. The
    /// destination type is a fallback ONLY when the result is a BOUNDED
    /// constant (a bare-literal computation like `let c: u8 = 200 + 100`), so
    /// it is range-checked against `c`. An UNbounded result (an unknown
    /// operand -- a call result, a param) keeps no primitive and stays
    /// unchecked, as before -- the target fallback must not turn "unknown"
    /// into a spurious overflow.
    ///
    /// The difference of two addresses is a distance, and the address model
    /// calls a distance a count: it names no referent, and no width equality
    /// relates counts to addresses, so it lands `u64` rather than its
    /// operands' carrier. `addr - count` still offsets an address and keeps
    /// `addr`. `reject_address_arithmetic` admits exactly the shapes separated
    /// here.
    fn result_primitive(&self, interval: Interval) -> Option<PrimitiveType> {
        if self.operator() == BinaryOperator::Subtract
            && self.left.primitive == Some(PrimitiveType::Addr)
            && self.right.primitive == Some(PrimitiveType::Addr)
        {
            return Some(PrimitiveType::U64);
        }
        self.left.primitive.or(self.right.primitive).or_else(|| {
            if interval.low.is_some() && interval.high.is_some() {
                self.walk.target_primitive
            } else {
                None
            }
        })
    }

    /// Whether this operation's Exact counterpart forms under the walk's
    /// facts: every formation condition the Exact policy owes holds. A
    /// Trapping primitive traps exactly when one of them fails, so this is
    /// also the judgment that no trap is reachable here.
    fn forms_exactly(
        &self,
        policy_primitive: IntegerPolicyPrimitive,
        primitive: PrimitiveType,
        interval: Interval,
    ) -> bool {
        let walk = self.walk;
        let mut unproven = Vec::new();
        if matches!(
            self.operator(),
            BinaryOperator::Divide | BinaryOperator::Modulo
        ) {
            return division_definedness::validate_operands(
                walk.program,
                walk.machine,
                walk.state,
                walk.environment,
                &self.binary,
                ArithmeticDomain::Exact,
                primitive,
                self.left.interval,
                self.right.interval,
                walk.owner,
                &mut unproven,
            );
        }
        let exact = integer_policy_bridge(policy_primitive, ArithmeticDomain::Exact);
        let interval = if self.cancels_to_zero() {
            Interval {
                low: Some(0),
                high: Some(0),
            }
        } else {
            interval
        };
        self.require_representable_result(
            exact,
            primitive,
            interval,
            self.relationally_fits_carrier(Some(primitive)),
            None,
            &mut super::ArithmeticDecisions::default(),
            &mut unproven,
        );
        if self.is_shift() {
            self.require_shift_count_within_width(exact, ArithmeticDomain::Exact, &mut unproven);
        }
        !unproven.iter().any(Diagnostic::is_error)
    }

    /// Re-decide [`Self::forms_exactly`] for one occurrence under another
    /// walk's environment. Operand obligations were already reported by the
    /// walk that reached them; this pass keeps only the formation verdict.
    fn forms_exactly_under(
        walk: &ExpressionWalk,
        expression: ExpressionHandle,
        binary: &TableBinaryExpression,
    ) -> bool {
        let Some(policy_primitive) = integer_policy_primitive(binary.operator) else {
            return false;
        };
        let mut ignored = Vec::new();
        let mut operation = Operation::analyze_operands(
            walk,
            expression,
            binary,
            &mut ignored,
            &mut super::ArithmeticDecisions::default(),
        );
        if !operation.is_shift() {
            operation.land_anonymous_operands(&mut ignored);
        }
        let interval = operation.mathematical_interval();
        operation
            .result_primitive(interval)
            .is_some_and(|primitive| operation.forms_exactly(policy_primitive, primitive, interval))
    }

    fn is_shift(&self) -> bool {
        matches!(
            self.operator(),
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
        )
    }

    /// A declared operation is not the token's primitive arithmetic. Its
    /// operands still owe their own obligations; only its caller-independent
    /// result signature bounds the returned value. In particular, u8 operands
    /// do not impose a u8 overflow proof on an operator declaring u64, and
    /// subtraction need not yield zero for equal operands. Uninstantiated
    /// results remain unknown.
    fn declared_operator_result(&self) -> Analysis {
        let walk = self.walk;
        let program = walk.program;
        let result = walk.state.and_then(|state| {
            crate::value_custody::expression_types::expression_result_type_reference(
                program,
                walk.machine,
                state,
                self.expression,
            )
        });
        let primitive = result.and_then(|result| program.primitive_type_reference(result));
        Analysis {
            recipe: None,
            domain: result.map(|result| program.arithmetic_domain_for_type_reference(result)),
            interval: primitive
                .and_then(primitive_range)
                .unwrap_or(Interval::UNBOUNDED),
            primitive,
        }
    }

    /// An anonymous operand beside a typed integer operand lands in that
    /// operand's primitive; it must land exactly.
    fn land_anonymous_operands(&mut self, diagnostics: &mut Vec<Diagnostic>) {
        let is_integer = |primitive: &PrimitiveType| integer_bit_width(*primitive).is_some();
        let Some(primitive) = self
            .left
            .primitive
            .filter(is_integer)
            .or_else(|| self.right.primitive.filter(is_integer))
        else {
            return;
        };
        let program = self.walk.program;
        let owner = self.walk.owner;
        for (operand, analysis) in [
            (self.binary.left, &mut self.left),
            (self.binary.right, &mut self.right),
        ] {
            if matches!(
                program.expression_table.expression(operand),
                ExpressionNode::Match(_)
            ) {
                validate_anonymous_integer_primitive_range(
                    program,
                    primitive,
                    operand,
                    owner,
                    diagnostics,
                );
                continue;
            }
            let Some(evaluated) =
                literals::anonymous_numeric_value(program, operand, &mut |expression| {
                    literals::has_anonymous_operator_meaning(program, expression)
                })
            else {
                continue;
            };
            if let Some(literal) = evaluated
                .value
                .to_integer_exact()
                .and_then(|value| literals::land_integer_value(&value, primitive))
            {
                analysis.interval = literal_interval(&literal);
            } else {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "anonymous operand `{}` cannot land exactly in `{}` in {owner}; type an \
                         operand before division if integer division was intended",
                        evaluated.value,
                        primitive.name(),
                    ))
                    .with_source_span(program.expression_table.source_span(operand)),
                );
            }
        }
    }

    /// A divisor that is PROVABLY zero (a literal `0`, or a value the prover
    /// has pinned to exactly 0) always traps -- the interpreter traps on
    /// div/mod-by-zero in every domain, and native `idiv` faults -- so it is
    /// dead-wrong code, rejected here like an out-of-range literal. This is the
    /// constant diagnostic only. The selected policy's general formation
    /// conditions are checked separately after operand domain selection;
    /// Trapping owns a runtime crash, while other policies require nonzero.
    fn reject_provably_zero_divisor(&self, diagnostics: &mut Vec<Diagnostic>) {
        let operation = match self.operator() {
            BinaryOperator::Divide => "division",
            BinaryOperator::Modulo => "remainder",
            _ => return,
        };
        if self.right.interval.low == Some(0) && self.right.interval.high == Some(0) {
            diagnostics.push(Diagnostic::error(format!(
                "{operation} by zero in {}: the divisor is provably zero, which always traps at \
                 runtime. Remove the operation or use a nonzero divisor.",
                self.walk.owner,
            )));
        }
    }

    /// The index/count/address model (design brief, SETTLED): `addr` composes
    /// with COUNTS -- `addr + count` / `count + addr` / `addr - count` offset an
    /// address -- and differences with ITSELF (`addr - addr` is the count
    /// between two addresses). Every other arithmetic pairing is meaningless:
    /// `addr + addr` has no referent (and no representation on CHERI-class
    /// targets where addr may be a 128-bit capability), and multiplying,
    /// dividing, or shifting an address conflates the axes the model exists to
    /// separate. Reject loudly.
    fn reject_address_arithmetic(&self, diagnostics: &mut Vec<Diagnostic>) {
        let left_is_addr = self.left.primitive == Some(PrimitiveType::Addr);
        let right_is_addr = self.right.primitive == Some(PrimitiveType::Addr);
        if !left_is_addr && !right_is_addr {
            return;
        }
        let legal = match self.operator() {
            // addr - addr -> count; addr - count -> addr. (count - addr has
            // left_is_addr == false and stays illegal.)
            BinaryOperator::Subtract => left_is_addr,
            // Exactly one side is the address; the other offsets it.
            BinaryOperator::Add => left_is_addr != right_is_addr,
            _ => false,
        };
        if !legal {
            diagnostics.push(Diagnostic::error(format!(
                "meaningless address arithmetic in {}: `addr` composes with \
                 counts (`addr + u64`, `addr - u64`) or differences with itself \
                 (`addr - addr` is the count between two addresses); `{}` over \
                 these operands conflates the address and count axes.",
                self.walk.owner,
                arithmetic_operator_spelling(self.operator()),
            )));
        }
    }

    /// S2: a binary mixing two different explicit domains is illegal; a neutral
    /// operand adopts the other's domain.
    ///
    /// A SHIFT's count operand carries no domain weight: "shift overflow is
    /// defined by the domain on which the operator is happening... lhs domain
    /// governs, rhs doesn't matter" (owner ruling, 2026-07-13). `wrapped <<
    /// self.k` takes the LHS domain and the count is just a number -- exempt
    /// from the mixed-domain check and from the domain merge.
    fn operand_domain(&self, diagnostics: &mut Vec<Diagnostic>) -> Option<ArithmeticDomain> {
        if self.is_shift() {
            return self.left.domain;
        }
        match (self.left.domain, self.right.domain) {
            (Some(left_domain), Some(right_domain)) => {
                if left_domain != right_domain {
                    diagnostics.push(Diagnostic::error(format!(
                        "mixed arithmetic domains in {}: one operand is `{}` and the other is \
                         `{}`. Decision 17 forbids implicit domain mixing -- cross domains with \
                         an explicit `as` cast, or declare both operands in the same domain.",
                        self.walk.owner,
                        left_domain.name(),
                        right_domain.name(),
                    )));
                }
                Some(if left_domain == ArithmeticDomain::Exact {
                    right_domain
                } else {
                    left_domain
                })
            }
            (Some(domain), None) | (None, Some(domain)) => Some(domain),
            (None, None) => None,
        }
    }

    fn bitwise_result(&self, domain: Option<ArithmeticDomain>) -> Analysis {
        let (left, right) = (&self.left, &self.right);
        let primitive = left.primitive.or(right.primitive).or_else(|| {
            (left.interval.low.is_some()
                && left.interval.high.is_some()
                && right.interval.low.is_some()
                && right.interval.high.is_some())
            .then_some(self.walk.target_primitive)
            .flatten()
        });
        Analysis {
            recipe: None,
            domain,
            interval: primitive
                .map(|primitive| {
                    bitwise::binary(
                        self.operator(),
                        primitive,
                        left.interval,
                        right.interval,
                        self.known_unsigned(primitive, self.binary.left),
                        self.known_unsigned(primitive, self.binary.right),
                    )
                })
                .unwrap_or(Interval::UNBOUNDED),
            primitive,
        }
    }

    fn known_unsigned(&self, primitive: PrimitiveType, operand: ExpressionHandle) -> Option<u64> {
        bitwise_known_unsigned(
            self.walk.program,
            self.walk.environment,
            primitive,
            operand,
            0,
        )
    }

    /// The unbounded mathematical result, refined by relations between the
    /// operands.
    ///
    /// S4: modulo is bounded by the divisor's magnitude and division never
    /// grows the dividend's magnitude -- bounding both lets a `(a % K)` /
    /// `(a / K)` result feed exact arithmetic instead of poisoning the
    /// enclosing op with an unbounded operand. Neither is overflow-flagged; the
    /// tighter interval is purely a better (still sound) over-approximation for
    /// any ENCLOSING op. Right shifts retain their operand carrier's bounds;
    /// left-shift product bounds feed the Exact value-overflow obligation.
    fn mathematical_interval(&self) -> Interval {
        let walk = self.walk;
        let (program, machine, state) = (walk.program, walk.machine, walk.state);
        let (left, right) = (self.binary.left, self.binary.right);
        let interval = match self.operator() {
            BinaryOperator::Add => refine_dependent_product(
                program,
                machine,
                state,
                left,
                right,
                self.left.interval.add(self.right.interval),
            ),
            BinaryOperator::Subtract => refine_dependent_subtract(
                program,
                machine,
                state,
                left,
                right,
                self.left.interval.subtract(self.right.interval),
            ),
            BinaryOperator::Multiply => refine_dependent_product_factor(
                program,
                machine,
                state,
                left,
                right,
                self.left.interval.multiply(self.right.interval),
            ),
            BinaryOperator::Modulo => self.left.interval.modulo(self.right.interval),
            BinaryOperator::Divide
                if self.left.primitive.or(self.right.primitive) == Some(PrimitiveType::U64) =>
            {
                unsigned_representability::divide_interval(
                    self.left.interval,
                    self.right.interval,
                    self.known_unsigned(PrimitiveType::U64, left),
                    self.known_unsigned(PrimitiveType::U64, right),
                )
            }
            BinaryOperator::Divide => self.left.interval.divide(self.right.interval),
            BinaryOperator::ShiftLeft => self.left.interval.shift_left(self.right.interval),
            BinaryOperator::ShiftRight => self.left.primitive.map_or_else(
                || self.left.interval.shift_right(self.right.interval),
                |primitive| {
                    bitwise::shift_right(
                        primitive,
                        self.left.interval,
                        self.right.interval,
                        bitwise_known_unsigned(program, walk.environment, primitive, left, 0),
                    )
                },
            ),
            _ => Interval::UNBOUNDED,
        };
        if self.operator() == BinaryOperator::Subtract
            && guard_narrowing::has_builtin_bound_arithmetic(
                program,
                machine,
                state,
                self.expression,
            )
            && let Some(floor) = ordered_values::subtract_floor(
                program,
                machine,
                state,
                walk.environment,
                left,
                right,
            )
        {
            return interval.intersect(Interval {
                low: Some(floor),
                high: None,
            });
        }
        interval
    }

    /// `x - x` over one integer place is zero.
    fn cancels_to_zero(&self) -> bool {
        let walk = self.walk;
        self.operator() == BinaryOperator::Subtract
            && self.left.primitive == self.right.primitive
            && self
                .left
                .primitive
                .is_some_and(|primitive| integer_bit_width(primitive).is_some())
            && guard_narrowing::has_builtin_bound_arithmetic(
                walk.program,
                walk.machine,
                walk.state,
                self.expression,
            )
            && ordered_values::same_place(
                walk.program,
                walk.machine,
                walk.state,
                self.binary.left,
                self.binary.right,
            )
    }

    /// Relational facts (joint operand bounds, a widened unsigned product, a
    /// bounded u64 left shift) that prove an Exact result representable
    /// without every operand being bounded.
    fn relationally_fits_carrier(&self, primitive: Option<PrimitiveType>) -> bool {
        let walk = self.walk;
        let (program, environment) = (walk.program, walk.environment);
        let (left, right) = (self.binary.left, self.binary.right);
        match self.operator() {
            BinaryOperator::Add => {
                let Some((minimum, maximum)) = primitive.and_then(PrimitiveType::integer_bounds)
                else {
                    return false;
                };
                // The children have already discharged their own formation.
                // Query the same live relations for a composite result instead
                // of requiring both operands to be named storage places.
                walk.state.is_some_and(|state| {
                    super::super::live_integer_constraints::proves_range(
                        program,
                        walk.machine,
                        state,
                        environment,
                        self.expression,
                        Some(&numerics::bignum::BigInt::from_i128(minimum)),
                        Some(&numerics::bignum::BigInt::from_i128(maximum)),
                    )
                })
            }
            BinaryOperator::Subtract => {
                environment.proves_joint_subtract_bound(program, left, right)
                    || environment.proves_signed_joint_subtract_lower_bound(program, left, right)
                    || environment.proves_signed_joint_subtract_upper_bound(program, left, right)
            }
            BinaryOperator::Multiply => {
                environment.proves_joint_multiply_bound(program, left, right)
                    || environment.proves_signed_joint_multiply_bounds(program, left, right)
                    || environment.proves_signed_joint_multiply_negation_bound(program, left, right)
                    || exact_unsigned_widened_multiply_fits(
                        program,
                        walk.machine,
                        walk.state,
                        left,
                        right,
                    )
            }
            BinaryOperator::ShiftLeft => {
                primitive == Some(PrimitiveType::U64)
                    && u64_exact_shift_left_fits(self.left.interval, self.right.interval)
            }
            _ => false,
        }
    }

    /// Abort-as-effect follow-up (owner 2026-07-18): a TRAPPING op whose result
    /// interval is provably DISJOINT from its type's range ALWAYS traps at
    /// runtime -- legal (the trap is the requested effect, and a trap is never
    /// dead), but almost certainly not what the author meant, so it warns.
    fn warn_unconditional_trap(
        &self,
        bridge: IntegerPolicyBridge,
        primitive: PrimitiveType,
        interval: Interval,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !bridge
            .trap_predicates
            .contains(&IntegerTrapPredicate::ResultOutsideCarrier)
        {
            return;
        }
        let Some(range) = primitive_range(primitive) else {
            return;
        };
        let always_above = matches!(
            (range.high, interval.low),
            (Some(bound), Some(low)) if low > bound
        );
        let always_below = matches!(
            (range.low, interval.high),
            (Some(bound), Some(high)) if high < bound
        );
        if always_above || always_below {
            diagnostics.push(Diagnostic::warning(format!(
                "trapping arithmetic in {} ALWAYS overflows `{}` -- this computation traps \
                 unconditionally at runtime (the trap is an effect and will fire even if the \
                 result is never used)",
                self.walk.owner,
                primitive_name(primitive),
            )));
        }
    }

    /// S3: an EXACT (undomained) `+`/`-`/`*`/`<<` must be provably in range.
    /// Left shift separately retains F8's count obligation; proving a legal
    /// count never authorizes value overflow.
    ///
    /// Division/remainder use the shared definedness judgment, including the
    /// signed MIN/-1 pair, rather than this result interval gate.
    /// Require a representable result where the policy owes one. Returns
    /// whether it owed one and this occurrence proved it: a decision
    /// lowering may consume.
    fn require_representable_result(
        &self,
        bridge: IntegerPolicyBridge,
        primitive: PrimitiveType,
        interval: Interval,
        exact_result_proven: bool,
        recipe: Option<&checked_trees::IntegerRangeRecipe>,
        decisions: &mut super::ArithmeticDecisions,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        if matches!(
            self.operator(),
            BinaryOperator::Divide | BinaryOperator::Modulo
        ) || !bridge
            .formation_conditions
            .contains(&IntegerFormationCondition::ResultRepresentable)
        {
            return false;
        }
        let Some(range) = primitive_range(primitive) else {
            return false;
        };
        if self.result_fits(primitive, range, interval, exact_result_proven) {
            return true;
        }
        if let Some(recipe) = recipe {
            use numerics::bignum::BigInt;
            if let Some(minimum) = range.low {
                let maximum = range.high.map(BigInt::from_i64).or_else(|| {
                    (primitive == PrimitiveType::U64).then(|| BigInt::from_u64(u64::MAX))
                });
                if let Some(maximum) = maximum {
                    decisions.integer_range_obligations.push(
                        checked_trees::IntegerRangeObligation::retain(
                            self.expression,
                            self.walk.machine.symbol,
                            recipe.clone(),
                            BigInt::from_i64(minimum),
                            maximum,
                            self.walk.owner.to_owned(),
                        ),
                    );
                    // This is conditional, not a decided formation. Target
                    // closure alone can publish the successful occurrence.
                    return false;
                }
            }
        }
        self.report_unrepresentable(primitive, diagnostics);
        false
    }

    fn result_fits(
        &self,
        primitive: PrimitiveType,
        range: Interval,
        interval: Interval,
        exact_result_proven: bool,
    ) -> bool {
        let operator = self.operator();
        // The interval route needs every operand already bounded; the
        // relational route proves `x + k` through a composed ceiling operand
        // whose own carrier fits the result primitive (`count < cap` with
        // `cap: u32` proves `count + 1` representable).
        let increase_fits =
            || operator == BinaryOperator::Add && self.unsigned_increase_fits(range.high);
        if primitive != PrimitiveType::U64 {
            return range.contains(interval) || increase_fits();
        }
        // `None` is an unknown ceiling, not evidence of fitting u64::MAX.
        // Mathematical carrier bounds and retained relational proofs establish
        // representability separately.
        exact_result_proven
            || (operator == BinaryOperator::Subtract && interval.low.is_some_and(|low| low >= 0))
            || unsigned_representability::binary_fits(
                operator,
                self.left.interval,
                self.right.interval,
                self.known_unsigned(primitive, self.binary.left),
                self.known_unsigned(primitive, self.binary.right),
            )
            // Operand formation is already checked. Use its independent
            // carrier bounds before constructing an ordered identity, which
            // can re-enter arithmetic analysis for this entire operand tree.
            // Retained relations still prove sums that carrier bounds cannot.
            || increase_fits()
    }

    fn unsigned_increase_fits(&self, ceiling: Option<i64>) -> bool {
        let walk = self.walk;
        let fits = |base, increment: Interval| {
            ordered_values::unsigned_increase_fits(
                walk.program,
                walk.machine,
                walk.state,
                walk.environment,
                base,
                increment,
                ceiling,
            )
        };
        fits(self.binary.left, self.right.interval) || fits(self.binary.right, self.left.interval)
    }

    fn report_unrepresentable(&self, primitive: PrimitiveType, diagnostics: &mut Vec<Diagnostic>) {
        let program = self.walk.program;
        // When an operand is a value-machine CALL, "constrain the operands'
        // range" is unactionable at the call site -- the fix is to annotate the
        // CALLEE's return type. Name it so the user knows where to look.
        let call_hint = overflow_operand_value_call_target(program, self.binary.left)
            .or_else(|| overflow_operand_value_call_target(program, self.binary.right))
            .map(|target| {
                format!(
                    " Here the operand `{target}(..)` is a value-machine call whose return range \
                     is unproven -- annotate its return type with a range or domain (e.g. `-> {} \
                     in Wrapping`).",
                    primitive_name(primitive)
                )
            })
            .unwrap_or_default();
        // A user who already declared the TARGET's domain (`let v: i32 in
        // Wrapping = t + 100`) needs to hear the operand-driven rule, not "opt
        // into a domain" -- they think they already did.
        let target_domain = self.walk.target_domain;
        let target_hint = if target_domain != ArithmeticDomain::Exact {
            format!(
                " The target's `in {domain}` does not re-domain the value expression (decision \
                 17 is operand-driven): declare the domain on an operand or intermediate (`let \
                 t: {prim} in {domain} = ...`), or re-tag the operand inline (`x as {prim} in \
                 {domain}`).",
                domain = target_domain.name(),
                prim = primitive_name(primitive),
            )
        } else {
            String::new()
        };
        diagnostics.push(Diagnostic::error(format!(
            "exact arithmetic in {} may overflow `{}`: the operands are not provably in range \
             (decision 17 -- exact arithmetic is a proof obligation). Widen with an `as` cast \
             to a larger type, constrain the operands' range (bound a parameter with a \
             `requires` clause, or narrow a value with a dominating guard), or opt into a \
             defined-overflow domain (`{} in Wrapping`/`Saturating`/`Trapping`).{}{}",
            self.walk.owner,
            primitive_name(primitive),
            primitive_name(primitive),
            call_hint,
            target_hint,
        )));
    }

    /// F8 -- the shift-COUNT ruling (ch5, settled 2026-07-18): the count is
    /// proof-or-policy. Under Exact the count must be PROVABLY in [0, width);
    /// Saturating governs value overflow, not operand validity, so its count
    /// obligation is Exact's. Wrapping reduces the count modulo the shifted
    /// width (`k & (width - 1)` for every current power-of-two source carrier)
    /// and Trapping TRAPS on an out-of-range count -- both defined, no
    /// obligation. The width is the SHIFTED operand's (decision 17 stays
    /// operand-driven: an anonymous lhs falls back to the destination
    /// primitive, but the lhs DOMAIN governs and a target `in Wrapping` never
    /// re-domains the count). The ISA's silent count-masking under Exact is an
    /// invented number and never adopted.
    fn require_shift_count_within_width(
        &self,
        bridge: IntegerPolicyBridge,
        effective_domain: ArithmeticDomain,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        if bridge.shift_count_law != ShiftCountLaw::MustBeWithinWidth {
            return false;
        }
        let Some(shift_primitive) = self.left.primitive.or(self.walk.target_primitive) else {
            return false;
        };
        let Some(width) = integer_bit_width(shift_primitive) else {
            return false;
        };
        let count = self.right.interval;
        let provably_in_range = matches!(count.low, Some(low) if low >= 0)
            && matches!(count.high, Some(high) if high < width);
        if provably_in_range {
            return true;
        }
        // Saved Boolean values contribute the same live resolved relations as
        // direct guards. Ask the common integer judgment for both complements;
        // count validity never supplies left-shift result representability.
        let relationally_in_range = self.walk.state.is_some_and(|state| {
            super::super::live_integer_constraints::proves_range(
                self.walk.program,
                self.walk.machine,
                state,
                self.walk.environment,
                self.binary.right,
                Some(&numerics::bignum::BigInt::from_i64(0)),
                Some(&numerics::bignum::BigInt::from_i64(width - 1)),
            )
        });
        if relationally_in_range {
            return true;
        }
        // A count that can NEVER be legal (a spelled `1 << 40` on u32) reads
        // differently from an unproven one.
        let always_out = matches!(count.low, Some(low) if low >= width)
            || matches!(count.high, Some(high) if high < 0);
        let verdict = if always_out {
            "is provably out of range and can never execute"
        } else {
            "is not provably below the operand width"
        };
        let saturating_hint = if effective_domain == ArithmeticDomain::Saturating {
            " (`Saturating` governs value overflow, not count validity -- its count obligation \
             is Exact's)"
        } else {
            ""
        };
        diagnostics.push(Diagnostic::error(format!(
            "shift count in {owner} {verdict} for `{prim}`{saturating_hint}: exact shifts prove \
             `count < {width}` (ch5 shift-count ruling -- proof-or-policy). Constrain the \
             count's range (a ranged type, a `requires` clause, or a dominating guard), or \
             pick a defined-count policy on the SHIFTED operand (`{prim} in Wrapping` reduces \
             the count modulo {width}, equivalently `count & {mask}` for this carrier; `in \
             Trapping` traps at runtime).",
            owner = self.walk.owner,
            prim = primitive_name(shift_primitive),
            mask = width - 1,
        )));
        false
    }

    /// Parents observe this node's policy result, not its unbounded
    /// mathematical calculation. In particular saturation followed by
    /// subtraction is not saturation of the final subtraction, and a wrapped
    /// quotient consumes the already-wrapped dividend.
    fn policy_result_interval(
        &self,
        effective_domain: ArithmeticDomain,
        primitive: Option<PrimitiveType>,
        interval: Interval,
    ) -> Interval {
        if effective_domain == ArithmeticDomain::Exact {
            return interval;
        }
        let Some(carrier) = primitive.and_then(primitive_range) else {
            return interval;
        };
        let count_is_ordinary = !self.is_shift()
            || primitive.and_then(integer_bit_width).is_some_and(|width| {
                matches!(self.right.interval.low, Some(low) if low >= 0)
                    && matches!(self.right.interval.high, Some(high) if high < width)
            });
        // An unbounded unsigned high endpoint is the analysis window, not proof
        // that arbitrary mathematical overflow fits u64.
        let proven_in_carrier =
            carrier.contains(interval) && (carrier.high.is_some() || interval.high.is_some());
        if !count_is_ordinary {
            carrier
        } else if effective_domain == ArithmeticDomain::Saturating {
            let clamp = |value: i64| {
                let value = carrier.low.map_or(value, |low| value.max(low));
                carrier.high.map_or(value, |high| value.min(high))
            };
            Interval {
                low: interval.low.map(clamp).or(carrier.low),
                high: interval.high.map(clamp).or(carrier.high),
            }
        } else if !proven_in_carrier {
            // Modular reduction is not monotone across a wrap. A conservative
            // carrier also describes any normal return from a potentially
            // trapping operation without claiming that its independent
            // definedness obligations hold.
            carrier
        } else {
            interval
        }
    }
}
