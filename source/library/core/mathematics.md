# Mathematical library construction

[Proof contracts](../../../wiki/spec/proofs/contracts.md) and
[quotients](../../../wiki/spec/proofs/quotients.md) define the intended language.
Core declarations below are implementation subjects, not compiler-specific
mathematical rules.

[Rat](rat.omg) currently stores an `IntPair` numerator and positive `Nat`
denominator. `mk_signed_rat` cancels the signed difference pair before gcd
reduction. `rat_gap` remains a natural-valued cross-product metric. Constructors
establish canonical representatives, but arbitrary record construction does not;
static index admission rechecks positivity, cancelled coordinates, and reduction.
The recursive Nat backing has no runtime layout. Float meaning uses this public
rational theory, not a separate private one.

[Cauchy obligations](cauchy.omg) use static generator/modulus machines in the
currently expressible pointwise proofs. The target witness-bundle construction
must also reason with abstract mathematical moduli: transitivity composes
`M3(e) = max(M1(2e), M2(2e))` and the rational triangle law without evaluating
either hidden modulus at a numeral. Existing pointwise machines do not establish
that general function/predicate binders or the full quotient are implemented.

[Real](real.omg) remains a temporary opaque axiomatic package with separately
admitted laws. It is not yet the intended Cauchy quotient, a native primitive,
or a runtime float representation. Replacing it requires the relation, witness,
quotient, and receiving-axiom contracts to survive, not merely the same names.
`PROOF-CONTRACT-MIGRATION` and `QUOTIENT-THEOREM-LIFT` on the
[execution board](../../../TASKS.md) own that work.
