# Conditional callback carry

Ratified 2026-11-07. A generic resource helper should work with both synchronous
and suspending callbacks when the selected resource permits the actual crossing.
Check its body once and retain the condition: if this call can suspend, its live
resources must permit suspension. Applications discharge or satisfy that condition.

The normative owners are [operational dependencies](../../spec/language/effects.md#conditional-suspension-carry),
[carry](../../spec/resources/carry.md#borrows-across-suspension),
[forwarding](../../spec/language/generics.md#forwarded-callback-requirements)
and [encoding](../../spec/terminal-psi/encoding.md#conditional-suspension-carry-summaries).
Track implementation under `STATIC-MACHINE-MIGRATION` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

- No new source syntax. Keep the existing `suspends` ceiling, `suspend` marker,
  carry policies and `suspends false` refinement. This extends source acceptance;
  published contract identities can change and require dependent rebuilding.
- Use the exact call site's normalized suspension dependency, including all
  callback projections and any fixed contribution. False needs no suspension
  permission; true needs it unconditionally; a symbolic condition is retained.
- Conditional rows belong to the enclosing generic machine. Retain the live
  subject or claim, scoped provenance, required permission, crossing and condition.
  Forwarding substitutes resource subjects as well as callback binders.
- Selected published contracts and valid carry evidence decide admission.
  An opaque callback's broad contract stays broad. Observed immediate completion,
  private-body inspection and the current consumer set establish no guarantee.
- Ordinary ownership, access, lifetimes, call fences, invariant windows, outcomes,
  cancellation and CPU/thread/address rules are unchanged. Blocking adds no such
  condition. A caller cannot grant permission for a helper's private acquisition
  or override an explicit suspension-forbidden type policy.
- Runtime callable types, installation-bound rows and refinement syntax keep
  their forms. Ordinary callable construction must discharge the selected
  generic's constraints or include its valid residual resource preconditions in
  the destination's complete contract; erasure cannot discard an obligation.

## Implementation

The documentation sweep from base `1287c6af5653a316d2edf90c4660e960d24e6776`
does not implement conditional carry. Complete the following in both compilers
and the existing suites before closing the board item:

1. Attribute only suspension-axis permission obligations to each canonical call's
   suspension dependency during generic body checking. Preserve exact liveness,
   projected subjects, claim origins and invocation scopes. Normalize finite OR
   conditions without enumerating callback combinations or arbitrary proof search.
2. Publish rows on the enclosing machine's operational contract. Compose through
   wrappers and anchored recursive summaries using exact argument, type and
   callback substitutions. Distinguish separate resource arguments and local
   acquisitions; a missing mapping cannot remove an obligation.
3. At applications, discharge false conditions and check active permissions using
   valid resource evidence and type policy. Forward unresolved conditions only
   through checked generic contracts. Closed execution must have no unresolved
   condition. Diagnostics identify the resource, originating crossing, forwarding
   context and callback contributions that require suspension permission.
4. Carry normalized rows into interface identity, dependency invalidation and
   exported-interface encoding. Specify wire placement/versioning with the codec
   implementation and update field tables, canonical validation and round trips
   together. This ratification assigns no new tags or serialized bytes.
5. Verify closed lowering and runtime admission retain the selected live frontier
   and carry policies. Reuse the existing call-plan format where sufficient;
   unchanged format does not establish correct plan construction. Add no runtime
   branch or proof search to implement this static selection rule.

## Acceptance

Extend existing generic/carry suites with these six cases and their minimal
reject controls. Use the same helper body and retained resource across each
pair, changing only the selected callback or valid suspension permission.

| Case | Required result |
| --- | --- |
| Strict claim, synchronous callback | Accept; the published contract guarantees no suspension and discharges only the suspension-permission obligation. |
| Strict claim, suspending callback | Reject at application, naming the claim, originating crossing and selected callback. A callback that can park but completes immediately on this run still rejects. |
| Independent suspension | With the same strict claim live, adding an independent suspending call requires permission unconditionally, regardless of a synchronous callback selection. |
| Established permission | Accept a suspending callback when the claim validly establishes `Carry::AcrossSuspend` and all other carry requirements hold. A type-wide suspension prohibition still rejects. |
| Forwarding and multiple callbacks | Compose a call's condition over both callbacks. Test different resource arguments and projected fields, scoped local acquisitions and nested wrappers so evidence for one subject cannot authorize another. |
| Ordinary invariant fence | A synchronous callback still rejects when the call itself exposes an unfinished invariant. Keep outcome and cancellation obligations intact. |

Cover both structural and nominal static callbacks, including existing checked
forwarding summaries. Check interface identity, codec round trips, stale/missing
rows and the fixed runtime-callable contract boundary. Preserve previously valid
cases and pin intentional rejection changes. Execute positive cases with
`omega run --both` across supported hosts; no new test subsystem is required.
