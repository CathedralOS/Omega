# Bounded growth

A fixed-capacity value is valid only when every length-affecting construction
and write proves its resulting live length fits capacity. Missing evidence
cannot be replaced with a guessed finite bound, silent truncation, or a private
default size. The author must strengthen the input/cycle invariant, select a
larger proved bound, or use explicit [allocation](allocation.md) with its own
authority, reach, resource, and failure contract.

[Bounded input](bounded_input.md) need not prove that an entire external stream
or line fits. It writes within an existing supplied range and returns a full
outcome when that range is exhausted. Reporting a partial prefix is not silent
truncation; subsequent logical reads retain the remaining input. The reader
changes neither the owner's live length nor its allocation. A caller that
assembles prefixes still owes the ordinary growth and owner-update obligations.

## Derivation

| Shape | Required bound |
| --- | --- |
| Literal or fixed append chain | Sum of known lengths. |
| Finite cycle, fixed append length | Initial length plus checked iteration bound times append length. |
| Finite cycle, bounded element length | Initial length plus checked iteration bound times maximum element length. |
| Unconstrained symbolic product | No general finite inference from the intended linear fragment. |
| Unbounded/data-dependent growth without a finite maximum | No fixed-storage admission. |

The rule applies to text, vectors, encoded output, and other capacity-bearing
values. A cycle needs a relation between progress and accumulated length;
independent intervals alone can lose it. Derivation may use a checked relational
summary or a selected kernel-checkable length law, retaining exact premises and
trust dependencies. A finite established bound may replace a symbolic coefficient
with a constant for that obligation. An unconstrained nonlinear product cannot
be treated as if that premise existed.

Length laws describe the selected operation. Normalizing a concat to the sum
of operand lengths, for example, must cite that operation's exact law, not a
same-spelled builtin assumption. Failure of a bounded proof procedure is not
evidence that the requested finite bound holds.

## Carrier invariants

An owned variable-fill bounded carrier has exact capacity and live length. A
zero-initialized carrier has live length zero. A borrowed slice exposes only the
live initialized range, not unused capacity. Every length-affecting mutation,
assignment, call, return, case payload, and control-flow edge must preserve:

- Length within the same capacity and matching the initialized prefix.
- Required value qualifications, such as UTF-8.
- No borrowed exposure beyond the live range.
- Capacity and live-length meaning through copies and transfers.

An inline `{length, bytes}` representation does not discharge those obligations
by itself. Checking only construction while allowing unchecked later writes is
insufficient.

## Allocation and optimization

A proved fixed-storage path requires no allocation-service reach. Inference
removes an allocation operation only when checked normalized control flow proves
it unreachable; it neither creates backing nor hides reachable provider use.
Failure to infer a bound does not select an ambient heap.
Fixed-storage and allocation-backed variants remain distinct in public type and
effect identity.

A local growth bound proves only the stated carrier/request fits. It does not
prove global peak storage, fragmentation, reuse, or allocator selection. Cycle
summaries and their Terminal evidence must obey these same rules even where
the current implementation supports only a narrower inference fragment.
