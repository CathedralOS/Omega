//! Recursive arithmetic expression analysis.
//!
//! This module owns the operand-domain walk and interval result for one
//! expression. Declaration checks, flow-state ownership, and total-proposition
//! formation stay in their dedicated modules.

use super::*;

fn integer_policy_primitive(
    operator: BinaryOperator,
) -> Option<psi_numerics::integer_policy::IntegerPolicyPrimitive> {
    use psi_numerics::integer_policy::IntegerPolicyPrimitive;

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

/// The result of analysing an expression for the domain + overflow rules.
pub(super) struct Analysis {
    /// The arithmetic domain (`None` = neutral: a literal or `bool` result).
    pub(super) domain: Option<ArithmeticDomain>,
    /// The value range, for the overflow proof obligation.
    pub(super) interval: Interval,
    /// The integer primitive type, for the overflow range bound (`None` when it
    /// cannot be determined, e.g. a bare literal).
    pub(super) primitive: Option<PrimitiveType>,
}

const NEUTRAL: Analysis = Analysis {
    domain: None,
    interval: Interval::UNBOUNDED,
    primitive: None,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn analyze(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    env: &ValueEnv,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Analysis {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let operator = binary.operator;
            let left = analyze(
                program,
                machine,
                state,
                binary.left,
                env,
                target_primitive,
                target_domain,
                owner,
                diagnostics,
            );
            let right = analyze(
                program,
                machine,
                state,
                binary.right,
                env,
                target_primitive,
                target_domain,
                owner,
                diagnostics,
            );
            if !is_arithmetic(operator) {
                // Comparison / logical `and`/`or`: a `bool` whose integer value is
                // 0 or 1. Its interval is [0, 1] (NOT unbounded) so it does not
                // poison an enclosing arithmetic op -- e.g. the match desugar
                // `d + (s == p) * (v - d)` stays bounded. No arithmetic domain.
                return Analysis {
                    domain: None,
                    interval: Interval {
                        low: Some(0),
                        high: Some(1),
                    },
                    primitive: None,
                };
            }

            // A divisor that is PROVABLY zero (a literal `0`, or a value the prover
            // has pinned to exactly 0) always traps -- the interpreter traps on
            // div/mod-by-zero in every domain, and native `idiv` faults -- so it is
            // dead-wrong code, rejected here like an out-of-range literal. This is
            // the constant case only: a divisor that MIGHT be zero (an interval that
            // merely straddles 0) stays a runtime concern, not a compile error.
            if matches!(operator, BinaryOperator::Divide | BinaryOperator::Modulo)
                && right.interval.low == Some(0)
                && right.interval.high == Some(0)
            {
                let operation = if operator == BinaryOperator::Divide {
                    "division"
                } else {
                    "remainder"
                };
                diagnostics.push(Diagnostic::error(format!(
                    "{operation} by zero in {owner}: the divisor is provably zero, which always \
                     traps at runtime. Remove the operation or use a nonzero divisor."
                )));
            }

            // The index/count/address model (design brief, SETTLED): `addr`
            // composes with COUNTS -- `addr + count` / `count + addr` /
            // `addr - count` offset an address -- and differences with
            // ITSELF (`addr - addr` is the count between two addresses).
            // Every other arithmetic pairing is meaningless: `addr + addr`
            // has no referent (and no representation on CHERI-class targets
            // where addr may be a 128-bit capability), and multiplying,
            // dividing, or shifting an address conflates the axes the model
            // exists to separate. Reject loudly.
            let left_is_addr = left.primitive == Some(PrimitiveType::Addr);
            let right_is_addr = right.primitive == Some(PrimitiveType::Addr);
            if left_is_addr || right_is_addr {
                let legal = match operator {
                    // addr - addr -> count; addr - count -> addr. (count -
                    // addr has left_is_addr == false and stays illegal.)
                    BinaryOperator::Subtract => left_is_addr,
                    // Exactly one side is the address; the other offsets it.
                    BinaryOperator::Add => left_is_addr != right_is_addr,
                    _ => false,
                };
                if !legal {
                    diagnostics.push(Diagnostic::error(format!(
                        "meaningless address arithmetic in {owner}: `addr` composes with \
                         counts (`addr + u64`, `addr - u64`) or differences with itself \
                         (`addr - addr` is the count between two addresses); `{}` over \
                         these operands conflates the address and count axes.",
                        arithmetic_operator_spelling(operator),
                    )));
                }
            }

            // A SHIFT's count operand carries no domain weight: "shift
            // overflow is defined by the domain on which the operator is
            // happening... lhs domain governs, rhs doesn't matter" (owner
            // ruling, 2026-07-13). `wrapped << self.k` takes the LHS domain
            // and the count is just a number -- exempt from the mixed-domain
            // check and from the domain merge below.
            let shift_count_rhs = matches!(
                operator,
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            );

            // S2: a binary mixing two different explicit domains is illegal.
            if !shift_count_rhs
                && let (Some(left_domain), Some(right_domain)) = (left.domain, right.domain)
                && left_domain != right_domain
            {
                diagnostics.push(Diagnostic::error(format!(
                    "mixed arithmetic domains in {owner}: one operand is `{}` and the other is \
                     `{}`. Decision 17 forbids implicit domain mixing -- cross domains with an \
                     explicit `as` cast, or declare both operands in the same domain.",
                    left_domain.name(),
                    right_domain.name(),
                )));
            }

            let domain = if shift_count_rhs {
                left.domain
            } else {
                match (left.domain, right.domain) {
                    (Some(left_domain), Some(right_domain)) => {
                        Some(if left_domain == ArithmeticDomain::Exact {
                            right_domain
                        } else {
                            left_domain
                        })
                    }
                    (Some(domain), None) | (None, Some(domain)) => Some(domain),
                    (None, None) => None,
                }
            };
            let mut interval = match operator {
                BinaryOperator::Add => refine_dependent_product(
                    program,
                    machine,
                    state,
                    binary.left,
                    binary.right,
                    left.interval.add(right.interval),
                ),
                BinaryOperator::Subtract => refine_dependent_subtract(
                    program,
                    machine,
                    state,
                    binary.left,
                    binary.right,
                    left.interval.subtract(right.interval),
                ),
                BinaryOperator::Multiply => refine_dependent_product_factor(
                    program,
                    machine,
                    state,
                    binary.left,
                    binary.right,
                    left.interval.multiply(right.interval),
                ),
                // S4: modulo is bounded by the divisor's magnitude and division
                // never grows the dividend's magnitude -- bounding both lets a
                // `(a % K)` / `(a / K)` result feed exact arithmetic instead of
                // poisoning the enclosing op with an unbounded operand. Neither is
                // overflow-flagged below; the tighter interval is purely a better
                // (still sound) over-approximation for any ENCLOSING op. Shifts
                // stay unbounded except for left shift, whose mathematical
                // product bounds feed the Exact value-overflow obligation.
                BinaryOperator::Modulo => left.interval.modulo(right.interval),
                BinaryOperator::Divide => left.interval.divide(right.interval),
                BinaryOperator::ShiftLeft => left.interval.shift_left(right.interval),
                BinaryOperator::ShiftRight => left.interval.shift_right(right.interval),
                _ => Interval::UNBOUNDED,
            };
            // Operand primitives win. The destination type is a fallback ONLY when
            // the result is a BOUNDED constant (a bare-literal computation like
            // `let c: u8 = 200 + 100`), so it is range-checked against `c`. An
            // UNbounded result (an unknown operand -- a call result, a param) keeps
            // no primitive and stays unchecked, as before -- the target fallback
            // must not turn "unknown" into a spurious overflow.
            let primitive = left.primitive.or(right.primitive).or_else(|| {
                if interval.low.is_some() && interval.high.is_some() {
                    target_primitive
                } else {
                    None
                }
            });

            // S3: an EXACT (undomained) `+`/`-`/`*`/`<<` must be provably in
            // range. Left shift separately retains F8's count obligation
            // below; proving a legal count never authorizes value overflow.
            let effective_domain = domain.unwrap_or(ArithmeticDomain::Exact);
            let policy_bridge = integer_policy_primitive(operator).map(|primitive| {
                psi_numerics::integer_policy::integer_policy_bridge(primitive, effective_domain)
            });
            if effective_domain == ArithmeticDomain::Exact
                && operator == BinaryOperator::Add
                && (env.proves_joint_add_upper_bound(program, binary.left, binary.right)
                    || env.proves_joint_add_lower_bound(program, binary.left, binary.right))
                && let Some(range) = primitive.and_then(primitive_range)
            {
                interval = range;
            }
            if effective_domain == ArithmeticDomain::Exact
                && operator == BinaryOperator::Subtract
                && (env.proves_joint_subtract_bound(program, binary.left, binary.right)
                    || env.proves_signed_joint_subtract_lower_bound(
                        program,
                        binary.left,
                        binary.right,
                    )
                    || env.proves_signed_joint_subtract_upper_bound(
                        program,
                        binary.left,
                        binary.right,
                    ))
                && let Some(range) = primitive.and_then(primitive_range)
            {
                interval = range;
            }
            if effective_domain == ArithmeticDomain::Exact
                && operator == BinaryOperator::Multiply
                && (env.proves_joint_multiply_bound(program, binary.left, binary.right)
                    || env.proves_signed_joint_multiply_bounds(program, binary.left, binary.right)
                    || env.proves_signed_joint_multiply_negation_bound(
                        program,
                        binary.left,
                        binary.right,
                    )
                    || exact_unsigned_widened_multiply_fits(
                        program,
                        machine,
                        state,
                        binary.left,
                        binary.right,
                    ))
                && let Some(range) = primitive.and_then(primitive_range)
            {
                interval = range;
            }
            if effective_domain == ArithmeticDomain::Exact
                && operator == BinaryOperator::ShiftLeft
                && primitive == Some(PrimitiveType::U64)
                && u64_exact_shift_left_fits(left.interval, right.interval)
                && let Some(range) = primitive_range(PrimitiveType::U64)
            {
                interval = range;
            }
            // Abort-as-effect follow-up (owner 2026-07-18): a TRAPPING op
            // whose result interval is provably DISJOINT from its type's
            // range ALWAYS traps at runtime -- legal (the trap is the
            // requested effect, and a trap is never dead), but almost
            // certainly not what the author meant, so it warns.
            if policy_bridge.is_some_and(|bridge| {
                bridge.trap_predicates.contains(
                    &psi_numerics::integer_policy::IntegerTrapPredicate::ResultOutsideCarrier,
                )
            }) && let Some(primitive) = primitive
                && let Some(range) = primitive_range(primitive)
            {
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
                        "trapping arithmetic in {owner} ALWAYS overflows `{}` -- this \
                         computation traps unconditionally at runtime (the trap is an \
                         effect and will fire even if the result is never used)",
                        primitive_name(primitive),
                    )));
                }
            }
            // Exact division/remainder retain their dedicated specification-
            // position definedness checker; preserve that established lane
            // while the generic interval gate consumes every other
            // representability row.
            if !matches!(operator, BinaryOperator::Divide | BinaryOperator::Modulo)
                && policy_bridge.is_some_and(|bridge| {
                    bridge.formation_conditions.contains(
                    &psi_numerics::integer_policy::IntegerFormationCondition::ResultRepresentable,
                )
                })
                && let Some(primitive) = primitive
                && let Some(range) = primitive_range(primitive)
                && !range.contains(interval)
            {
                // When an operand is a value-machine CALL, "constrain the operands'
                // range" is unactionable at the call site -- the fix is to annotate
                // the CALLEE's return type. Name it so the user knows where to look.
                let call_hint = overflow_operand_value_call_target(program, machine, binary.left)
                    .or_else(|| overflow_operand_value_call_target(program, machine, binary.right))
                    .map(|target| {
                        format!(
                            " Here the operand `{target}(..)` is a value-machine call whose \
                             return range is unproven -- annotate its return type with a range or \
                             domain (e.g. `-> {} in Wrapping`).",
                            primitive_name(primitive)
                        )
                    })
                    .unwrap_or_default();
                // A user who already declared the TARGET's domain (`let v: i32 in
                // Wrapping = t + 100`) needs to hear the operand-driven rule, not
                // "opt into a domain" -- they think they already did.
                let target_hint = if target_domain != ArithmeticDomain::Exact {
                    format!(
                        " The target's `in {domain}` does not re-domain the value expression \
                         (decision 17 is operand-driven): declare the domain on an operand or \
                         intermediate (`let t: {prim} in {domain} = ...`), or re-tag the operand \
                         inline (`x as {prim} in {domain}`).",
                        domain = target_domain.name(),
                        prim = primitive_name(primitive),
                    )
                } else {
                    String::new()
                };
                diagnostics.push(Diagnostic::error(format!(
                    "exact arithmetic in {owner} may overflow `{}`: the operands are not provably \
                     in range (decision 17 -- exact arithmetic is a proof obligation). Widen with \
                     an `as` cast to a larger type, constrain the operands' range (bound a \
                     parameter with a `requires` clause, or narrow a value with a dominating \
                     guard), or opt into a defined-overflow domain \
                     (`{} in Wrapping`/`Saturating`/`Trapping`).{}{}",
                    primitive_name(primitive),
                    primitive_name(primitive),
                    call_hint,
                    target_hint,
                )));
            }

            // F8 -- the shift-COUNT ruling (ch5, settled 2026-07-18): the count is
            // proof-or-policy. Under Exact the count must be PROVABLY in
            // [0, width); Saturating governs value overflow, not operand
            // validity, so its count obligation is Exact's. Wrapping reduces
            // the count modulo the shifted width (`k & (width - 1)` for every
            // current power-of-two source carrier) and Trapping TRAPS on an
            // out-of-range count -- both defined, no obligation. The width is the SHIFTED
            // operand's (decision 17 stays operand-driven: an anonymous lhs
            // falls back to the destination primitive, but the lhs DOMAIN
            // governs and a target `in Wrapping` never re-domains the count).
            // The ISA's silent count-masking under Exact is an invented number
            // and never adopted.
            let shift_count_law = match operator {
                BinaryOperator::ShiftLeft => Some(
                    psi_numerics::integer_policy::integer_policy_bridge(
                        psi_numerics::integer_policy::IntegerPolicyPrimitive::ShiftLeft,
                        effective_domain,
                    )
                    .shift_count_law,
                ),
                BinaryOperator::ShiftRight => Some(
                    psi_numerics::integer_policy::integer_policy_bridge(
                        psi_numerics::integer_policy::IntegerPolicyPrimitive::ShiftRight,
                        effective_domain,
                    )
                    .shift_count_law,
                ),
                _ => None,
            };
            if shift_count_law
                == Some(psi_numerics::integer_policy::ShiftCountLaw::MustBeWithinWidth)
                && let Some(shift_primitive) = left.primitive.or(target_primitive)
                && let Some(width) = integer_bit_width(shift_primitive)
            {
                let provably_in_range = matches!(right.interval.low, Some(low) if low >= 0)
                    && matches!(right.interval.high, Some(high) if high < width);
                if !provably_in_range {
                    // A count that can NEVER be legal (a spelled `1 << 40` on
                    // u32) reads differently from an unproven one.
                    let always_out = matches!(right.interval.low, Some(low) if low >= width)
                        || matches!(right.interval.high, Some(high) if high < 0);
                    let verdict = if always_out {
                        "is provably out of range and can never execute"
                    } else {
                        "is not provably below the operand width"
                    };
                    let saturating_hint = if effective_domain == ArithmeticDomain::Saturating {
                        " (`Saturating` governs value overflow, not count validity -- its count \
                         obligation is Exact's)"
                    } else {
                        ""
                    };
                    diagnostics.push(Diagnostic::error(format!(
                        "shift count in {owner} {verdict} for `{prim}`{saturating_hint}: exact \
                         shifts prove `count < {width}` (ch5 shift-count ruling -- \
                         proof-or-policy). Constrain the count's range (a ranged type, a \
                         `requires` clause, or a dominating guard), or pick a defined-count \
                         policy on the SHIFTED operand (`{prim} in Wrapping` reduces the count \
                         modulo {width}, equivalently `count & {mask}` for this carrier; \
                         `in Trapping` traps at runtime).",
                        prim = primitive_name(shift_primitive),
                        mask = width - 1,
                    )));
                }
            }

            Analysis {
                domain,
                interval,
                primitive,
            }
        }
        ExpressionNode::Cast(cast) => {
            // A recast is a byte-preserving view (`&x as &T`), not a numeric
            // conversion.  In particular, viewing an f32/f64 place through an
            // equal-width unsigned referee must not acquire F4's float-to-int
            // proof obligation: no floating value is being truncated or
            // rounded.  `expression_types::validate_cast_types` already keeps
            // this distinction; preserve it in the arithmetic-domain walk as
            // well so later integer operations see the stated referee type.
            if cast.form.is_recast() {
                let primitive = program.primitive_type_reference(cast.target_type);
                return Analysis {
                    domain: Some(ArithmeticDomain::Exact),
                    interval: primitive
                        .and_then(primitive_range)
                        .unwrap_or(Interval::UNBOUNDED),
                    primitive,
                };
            }
            // A cast re-types its operand, so the outer target does not flow in.
            let source = analyze(
                program,
                machine,
                state,
                cast.value,
                env,
                None,
                ArithmeticDomain::Exact,
                owner,
                diagnostics,
            );
            let primitive = program.primitive_type_reference(cast.target_type);
            // F4 (the float->int cast ruling): there is NO MODULAR READING
            // of a float, so `f as iN in Wrapping` is a compile error (ch5;
            // the ruling's precedent generalized to the float domain list).
            // Saturating (NaN -> 0, clamp to the target range) and Trapping
            // (trap on NaN/out-of-range) are the defined policies; a bare
            // Exact cast keeps the transitional truncation until float
            // constant tracking can carry the value obligation.
            if cast.domain == ArithmeticDomain::Wrapping
                && matches!(
                    source.primitive,
                    Some(PrimitiveType::F32 | PrimitiveType::F64)
                )
                && primitive.is_some_and(|target| integer_bit_width(target).is_some())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "float-to-int cast `in Wrapping` in {owner}: there is no modular reading \
                     of a float (ch5 cast ruling). Use `in Saturating` (NaN -> 0, clamp to \
                     the target range) or `in Trapping` (trap on NaN/out-of-range) instead.",
                )));
            }
            // F4 Exact obligation (the cast ruling's proof side): a BARE
            // float->int cast requires the value provably in the target's
            // range. What validation can prove today: a float LITERAL source
            // (through Mutable) whose truncation fits -- the two-phase law's
            // fold-visible face; runtime sources need a policy. (Mirrors the
            // F8a shift-count obligation's shape: proof where visible,
            // policy otherwise, never a silent target-defined number -- the
            // out-of-range bare cast was a pinned THREE-WAY native
            // divergence, x86 integer-indefinite vs aarch64/interp
            // saturation.)
            if cast.domain == ArithmeticDomain::Exact
                && matches!(
                    source.primitive,
                    Some(PrimitiveType::F32 | PrimitiveType::F64)
                )
                && let Some(target) = primitive
                && integer_bit_width(target).is_some()
            {
                let provable =
                    float_source_proves_int_cast(program, machine, state, env, cast.value, target);
                if !provable {
                    diagnostics.push(Diagnostic::error(format!(
                        "float-to-int cast in {owner} is not provably in `{}`'s range \
                         (ch5 cast ruling -- proof-or-policy). Prove a finite declared range \
                         or a dominating non-NaN/range guard, or use `in Saturating` (NaN \
                         -> 0, clamp to the target range) or `in Trapping` \
                         (trap on NaN/out-of-range).",
                        primitive_name(target),
                    )));
                }
            }
            // Exact integer coercion preserves the mathematical value. Width
            // narrowing and signedness changes therefore need a proof that
            // the source interval fits the complete target range; a cast is
            // not an opt-in truncation/reinterpretation surface. Widening
            // succeeds from the source carrier's ordinary full-range fact.
            if cast.domain == ArithmeticDomain::Exact
                && let Some(source_primitive) = source
                    .primitive
                    .filter(|source| primitive_range(*source).is_some())
                && let Some(target) = primitive.filter(|target| primitive_range(*target).is_some())
                && !integer_interval_fits_primitive(source.interval, source_primitive, target)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "Exact integer cast in {owner} from `{}` to `{}` is not provably \
                     representable; constrain the source with a range or dominating guard, \
                     or use a named Wrapping, Saturating, or Trapping conversion.",
                    source.primitive.map(primitive_name).unwrap_or("integer"),
                    primitive_name(target),
                )));
            }
            // Numeric conversion of a Boolean preserves its binary value. Keep
            // that stronger fact instead of widening the result to the target's
            // complete carrier: foreign bitmask construction such as
            // `(flag as i32) << 10` is therefore ordinary provable Exact
            // arithmetic, not a reason to weaken the operation to Wrapping.
            //
            // Exact integer coercion likewise preserves the mathematical
            // source value. Retain a sound source interval after the fit check
            // above so a widening conversion remains useful proof evidence for
            // enclosing arithmetic. Wrapped expressions can carry a computed
            // interval outside their carrier; in that case use the carrier's
            // full range rather than manufacturing an impossible intersection.
            // Non-Exact casts still re-range to the target carrier.
            let interval = if source.primitive == Some(PrimitiveType::Bool)
                && primitive.is_some_and(|target| integer_bit_width(target).is_some())
            {
                Interval {
                    low: Some(0),
                    high: Some(1),
                }
            } else if cast.domain == ArithmeticDomain::Exact
                && let Some(source_primitive) = source.primitive
                && integer_bit_width(source_primitive).is_some()
                && primitive.is_some_and(|target| integer_bit_width(target).is_some())
                && let Some(source_range) = primitive_range(source_primitive)
            {
                if source_range.contains(source.interval) {
                    source.interval
                } else {
                    source_range
                }
            } else {
                primitive
                    .and_then(primitive_range)
                    .unwrap_or(Interval::UNBOUNDED)
            };
            Analysis {
                domain: Some(cast.domain),
                interval,
                primitive,
            }
        }
        ExpressionNode::Call(call) => {
            // F7 named float requirements use the same operand-driven policy
            // selection as spellings. Preserve the selected domain for an
            // enclosing expression and reject two different explicit policies
            // before checked evidence is built. Classification calls return a
            // non-float and therefore carry no float result policy.
            if let Some((_operator, return_primitive)) =
                resolve_named_float_arithmetic(program, call)
            {
                let mut selected_domain: Option<ArithmeticDomain> = None;
                for argument in program.expression_table.expression_handles(call.arguments) {
                    let mut throwaway = Vec::new();
                    let argument = analyze(
                        program,
                        machine,
                        state,
                        *argument,
                        env,
                        target_primitive,
                        target_domain,
                        owner,
                        &mut throwaway,
                    );
                    let Some(domain) = argument.domain else {
                        continue;
                    };
                    if let Some(selected) = selected_domain
                        && selected != domain
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "mixed arithmetic domains in {owner}: named float operation `{}` \
                             receives both `{}` and `{}` operands. Decision 17 forbids implicit \
                             domain mixing -- cross domains with an explicit `as` cast, or \
                             declare every operand in the same domain.",
                            call.target,
                            selected.name(),
                            domain.name(),
                        )));
                    } else {
                        selected_domain = Some(domain);
                    }
                }
                return Analysis {
                    domain: selected_domain,
                    interval: Interval::UNBOUNDED,
                    primitive: Some(return_primitive),
                };
            }

            // S4: the `min`/`max` builtins bound their result by their operands'
            // intervals (`max(0, x)` is >= 0, `min(x, 100)` is <= 100), so a
            // clamped value can feed exact arithmetic instead of poisoning the
            // enclosing op. Reserved-builtin name + a free (receiverless) call +
            // exactly two arguments. Each operand is analyzed into a THROWAWAY
            // diagnostic buffer and its interval is trusted ONLY if it proves
            // clean -- the bound then rests on sound operand ranges, while an
            // unproven operand's diagnostics are dropped (call arguments are not
            // otherwise overflow-checked today, so this stays strictly permissive:
            // it can only tighten a previously-unbounded call result, never add a
            // rejection).
            if !call.receiver.is_valid()
                && matches!(call.target.as_str(), "min" | "max")
                && let [left_arg, right_arg] =
                    program.expression_table.expression_handles(call.arguments)
            {
                let mut throwaway = Vec::new();
                let left = analyze(
                    program,
                    machine,
                    state,
                    *left_arg,
                    env,
                    target_primitive,
                    target_domain,
                    owner,
                    &mut throwaway,
                );
                let right = analyze(
                    program,
                    machine,
                    state,
                    *right_arg,
                    env,
                    target_primitive,
                    target_domain,
                    owner,
                    &mut throwaway,
                );
                if throwaway.is_empty() {
                    let interval = if call.target.as_str() == "max" {
                        left.interval.max_with(right.interval)
                    } else {
                        left.interval.min_with(right.interval)
                    };
                    let domain = match (left.domain, right.domain) {
                        (Some(left_domain), Some(right_domain)) if left_domain == right_domain => {
                            Some(left_domain)
                        }
                        (Some(domain), None) | (None, Some(domain)) => Some(domain),
                        _ => None,
                    };
                    return Analysis {
                        domain,
                        interval,
                        primitive: left.primitive.or(right.primitive),
                    };
                }
            }

            // S4 return-range inference: a machine whose return type declares a
            // literal range constraint (`-> i32 [0..=10]`) is ENFORCED to
            // return within that range (validate_return_value_range, below), so a
            // caller doing exact arithmetic on the result can rely on the narrowed
            // interval instead of being forced into a domain. Narrow ONLY when the
            // return type carries a range constraint -- a plain `-> u32` stays
            // NEUTRAL (opaque/unbounded, as before); attaching a bare type's full
            // range + primitive to an otherwise-unbounded call result would turn a
            // previously-unchecked expression into a spurious overflow.
            if let Some((primitive, interval)) =
                call_return_type(program, machine, call).and_then(|return_type| {
                    range_constraint_interval(program, return_type)
                        .map(|interval| (program.primitive_type_reference(return_type), interval))
                })
            {
                return Analysis {
                    domain: None,
                    interval,
                    primitive,
                };
            }
            // ch15 stage 2 (modular return-range inference): no DECLARED range, so
            // infer the callee's return interval from its body (sound, permissive).
            match resolve_unique_self_call_state(program, machine, call).and_then(
                |(callee, state)| {
                    let primitive = program.primitive_type_reference(state.return_type);
                    infer_return_interval(program, callee, state, primitive)
                        .map(|interval| (primitive, interval))
                },
            ) {
                Some((primitive, interval)) => Analysis {
                    domain: None,
                    interval,
                    primitive,
                },
                None => NEUTRAL,
            }
        }
        ExpressionNode::Integer(value) => Analysis {
            domain: None,
            interval: literal_interval(value),
            primitive: None,
        },
        ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => NEUTRAL,
        // A place (`x`, `self.field`): its declared type gives the domain and the
        // integer primitive. The value range is the FLOW-tracked interval (S4) if
        // we have proven one on this linear path, else the primitive's full range.
        // An INDEXED read (`self.cells[rp]`) resolves through its collection's
        // ELEMENT type, so a range-refined element (`[i32 [0..=7]; N]`) feeds
        // the overflow proof exactly like a range-refined field (writes into
        // elements are range-enforced by the proof side, and ZII requires 0 in
        // the element range -- the interval is a true invariant).
        _ => match declared_place_type_raw(program, machine, state, expression).or_else(|| {
            // Only a RANGE-refined element feeds the analysis. An unranged
            // element stays NEUTRAL (unchecked) as before -- resolving it
            // would stamp `primitive + Exact` onto reads whose enclosing
            // arithmetic was historically domain-neutral (`[i32; 3] in
            // Wrapping` carries the domain on the ARRAY, so its bare-`i32`
            // element read as Exact broke Wrapping-array canaries with
            // spurious overflow + domain-mixing rejects).
            crate::places::declared_indexed_projection_type_raw(program, machine, state, expression)
                .filter(|handle| range_constraint_interval(program, *handle).is_some())
        }) {
            Some(handle) => {
                let primitive = program.primitive_type_reference(handle);
                let type_range = primitive
                    .and_then(primitive_range)
                    .unwrap_or(Interval::UNBOUNDED);
                // Narrowest sound interval: a value PROVEN on this path (flow env)
                // wins; else a declared `[min..max]` range constraint (S4); else
                // the full type width. A proven interval is INTERSECTED with the
                // type range, because a typed value is ALWAYS within its type even
                // when the proof only bounds ONE end -- a one-sided `requires x <
                // 100` gives the env `[None, 99]`, and `[None, 99] ∩ i32 = [i32::MIN,
                // 99]` keeps the type's low end so `x + 1` proves Exact (a Wrapping-
                // spilled interval is likewise clamped back to the type). The same
                // intersect-with-source-type keystone the narrowing store uses.
                let interval = place_path(program, expression)
                    .and_then(|path| env.get(&path))
                    .or_else(|| range_constraint_interval(program, handle))
                    .map(|proven| proven.intersect(type_range))
                    .unwrap_or(type_range);
                // R2 rung 3 slice 7 (READER HYPOTHESES): a domain-carrying
                // place's standing where facts refine the read -- sound
                // because the write net is TOTAL and gated reads are
                // access-gated, so the facts hold at every legal
                // observation.
                let interval = match crate::default_domains::where_fact_interval(
                    program, machine, state, expression,
                ) {
                    Some(facts) => interval.intersect(facts),
                    None => interval,
                };
                // Atomic integer types (AtomicU32, ...) have hardware wrap-around
                // semantics, so their arithmetic is Wrapping, not Exact -- a
                // `fetch_add` never raises an overflow proof obligation.
                let domain = if is_atomic_type(program, handle) {
                    ArithmeticDomain::Wrapping
                } else {
                    program.arithmetic_domain_for_type_reference(handle)
                };
                Analysis {
                    domain: Some(domain),
                    interval,
                    primitive,
                }
            }
            None => NEUTRAL,
        },
    }
}
