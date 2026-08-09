# Design Brief: Static Growth-Bound Inference

Status: the semantic boundary is settled. Straight-line bounded-carrier checks
are live; general cycle/recursion growth summaries remain implementation work.
Allocation strategy is owned separately by
[`allocator_story.md`](allocator_story.md).

This brief owns one question: when may Psi prove that a growing value fits
fixed storage, so no allocation or exhaustion outcome exists?

## Rule

A bounded carrier is legal only when every length-affecting construction and
write proves that the resulting live length is within capacity. A compiler may
infer the bound where the transfer is within its checked arithmetic fragment.
It must never guess a finite bound, silently truncate, or size from an
implementation default.

If no finite bound is proved, the program must do one of the following:

- provide a stronger input or cycle invariant;
- use an explicitly larger bounded carrier; or
- use an ordinary allocation-strategy package with its declared authority,
  resource contract, reach, and failure behavior.

Static inference removes an allocation operation only when stable normalized
control flow proves that operation unreachable. It does not mint storage
authority or hide reachable provider use.

## Decidable frontier

| Shape | Bound | Disposition |
| --- | --- | --- |
| literals and a fixed append chain | sum of known lengths | infer directly |
| bounded cycle with constant append size | `initial + iterations * constant` | require a checked relational cycle summary or cited length law |
| bounded cycle with a declared element bound | `initial + max_iterations * max_element_length` | infer after both finite bounds are established |
| two unconstrained symbolic factors | nonlinear product | do not infer a general finite bound |
| unbounded cycle or data-dependent growth | no proved finite maximum | reject fixed-storage construction or use explicit fallible allocation |

The important split is not “string versus collection.” It is whether Psi can
derive a finite arithmetic upper bound. The same rule applies to byte text,
vectors, encoded output, and other capacity-bearing values.

## Proof shape

Straight-line growth composes exact or upper-bound length transfers. A cycle
needs a relation between progress and accumulated length; independent intervals
lose that correlation. The implementation may discharge the relation through a
small checked relational domain or through a selected, kernel-checkable law for
the exact operation. Either route must retain its premises and trust closure.

One free symbolic coefficient is acceptable only after a finite bound turns it
into a constant for the obligation. An unconstrained product such as
`iterations * element_length` is outside the intended linear fragment. Psi
stands down explicitly rather than appealing to a general string solver or an
unbounded optimization heuristic.

Length laws are semantic facts about operations, not compiler folklore. For
example, concat may provide `len(result) = len(left) + len(right)` and a bounded
push may require `len(before) < capacity` while ensuring a one-element increase.
The proof-certificate bridge must cite the exact selected law used by each
normalization.

## Carrier and write enforcement

An owned variable-fill bounded carrier has an exact capacity and a live length.
Its physical representation may be inline `{length, bytes}`; a borrowed slice
projects the live byte range, not the entire capacity. A zero-initialized carrier
has live length zero.

Every mutation that can change length must preserve all of the following:

- the resulting length is within the same carrier capacity;
- the stored live length matches the initialized prefix;
- domain facts such as UTF-8 are established or preserved by the operation;
- a borrow exposes no bytes beyond the live range; and
- copying or returning the carrier retains the same capacity and live length.

Proving a narrow construction while allowing unchecked later writes is unsound.
Construction, assignment, call boundaries, case payloads, and loop/cycle edges
therefore consume the same capacity obligation family.

## Interaction with allocation

A proved fixed bound selects fixed storage and has no allocation-service reach.
Failure to prove a fixed bound does not imply a language-owned heap or `Arena`.
The caller chooses an ordinary strategy package over a qualified `Extent`, as
specified by `allocator_story.md`. Fresh backing requires the selected provider;
already-owned backing does not. Exhaustion is an explicit outcome unless the
caller proves the strategy has adequate capacity.

Global peak accounting, fragmentation, reuse guarantees, and allocator
selection are separate problems. A local growth theorem proves only that this
carrier or request fits its stated capacity.

## Remaining work and acceptance

- Generalize checked growth summaries across cycles and recursion without
  introducing unchecked built-in axioms.
- Connect inferred bounds to source-visible bounded collection construction and
  all length-changing operations.
- Preserve the derivation through terminal Psi and the proof-certificate bridge.
- Keep fixed-storage and allocation-backed variants distinct in public type and
  effect identity.

Acceptance requires positive canaries for literal chains, refined inputs, and
bounded cycles; negative canaries for one-byte overflow, missing input bounds,
unconstrained symbolic products, and unbounded growth; and artifact tests proving
that a certified fixed path has neither allocation authority nor allocation
service reach.
