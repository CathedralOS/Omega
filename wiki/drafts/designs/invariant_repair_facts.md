# Invariant repair facts

Repair helpers should compose through existing `requires` and `ensures`, and
fresh evidence should re-establish named predicates after mutation. This
owner-ratified clarification and call-frame extension (2026-11-05) keeps the
original lender's restoration obligation while avoiding unnecessary loss of
usable facts. It introduces no repair syntax or runtime repair operation.

The normative rules are [contract-based repair](../../spec/language/dependent_values.md#contract-based-repair),
[completion contracts](../../spec/language/state_contracts.md), and
[casts on mutable views](../../spec/language/domains.md#mutable-views).
Implementation work belongs to **DOMAIN-ACCESS-AUTHORITY** in
[TASKS.md](../../../TASKS.md).

## Remaining implementation and acceptance

- Re-establish predicate-only membership from current defining conditions after
  a write, using the shared proof machinery and checked helper guarantees.
  A named domain and the same predicate written explicitly must preserve the
  same logical obligation. Retain routed establishment, access and cleanup
  restrictions rather than inferring authority from predicate truth.
- Qualify repair composition for both unqualified owners and owners/fields
  declared `T in D`: entry facts, intermediate weaker contracts, successor
  guarantees and restored handback. A successor guarantee proves completion,
  never its own entry requirements. Preserve exact argument substitution,
  backing storage, loans and value versions through the entire pipeline.
- Allow unrelated calls during a domain window only when complete access,
  effect and outcome evidence excludes observation, interference, exposure
  and custody release. Reuse existing frame analysis; an empty write set alone
  is insufficient. Account for argument evaluation, callbacks, capability state,
  suspension and cancellation. Unknown effects must not acquire an exemption.
- Apply existing predicate-only weakening to compatible borrowed views without
  a redundant explicit cast. Preserve reference mode/lifetime and lender duties;
  no policy, routed provenance or default-domain obligation may disappear.
- Preserve invalid-entry rejection for default-domain values and unfinished
  construction. Reusing postconditions for completion does not authorize
  passing such a value to a receiver that assumes it valid on arrival.

## Regression evidence

The exploratory `p1`/`p2` pair describes this minimal obligation: after setting
`right = left + 1` from equal, bounded fields, prove a named predicate equivalent
to `left <= right + 1 && right <= left + 1`, as well as the same arithmetic
written directly. Use bounded inputs so overflow is not an unrelated refusal.

The `a5` family describes an exclusively borrowed qualified owner whose first
step breaks a named balance predicate and whose terminal successor repairs it
and promises membership back. Require actual full acceptance, then execution;
one remaining diagnostic is not evidence that every other check succeeded.
The unqualified-owner counterpart must also promise the invariant back, not
merely check a single runtime result. These probe names describe regression
intent, not portable fixture paths or fresh compiler validation.

Pin missing-guarantee, unsatisfied-entry, wrong-subject, stale-fact, unproved
cycle, conflicting-loan and invalid-default-domain controls. Pair unrelated
calls with read-only observers and overlapping/capability effects that reject.
Check reference weakening against both pure predicate and retained-policy cases.

Reuse existing fixtures and samples across the repository's nine value places;
add only a focused non-`facts/` fixture pair where needed. Verify both Terminal
and native execution on matching hosted targets and inspect evidence transport.
Keep golden outcomes tied to clean committed sources and review all movements.
Measure checker/entailment cost on simple and composed repairs against a fixed
base; reuse canonical facts and avoid repeatedly expanding whole predicates.
The ratification promises sound composition, not a measured cost improvement.
