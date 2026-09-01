# Total Specification Arithmetic

This brief settles arithmetic inside `requires`, `ensures`, domain predicates,
and guarded `crashes` routes. Those positions inhabit the proof logic. They are
not executable expressions, and every term admitted there is total.

## The boundary

Arithmetic policy remains operand-driven in executable code:

- Exact arithmetic forms only after its representability obligations are
  discharged;
- Wrapping arithmetic reduces representable-range overflow at the selected
  machine width;
- Saturating arithmetic clamps representable-range overflow to the selected
  carrier bounds; and
- Trapping arithmetic transfers control to a `Trap` crash edge when its
  primitive-specific failure condition holds.

The same written operator is admitted in `Prop` only when its selected meaning
is total. Exact is legal because partiality is discharged when the term is
formed. Wrapping and Saturating are legal after any primitive definedness
conditions not decided by the overflow policy are discharged; their selected
overflow behavior is total. Division by zero remains an obligation, for
example. A direct Trapping arithmetic operation is illegal: its partiality is
resolved by runtime control, and `Prop` has no runtime control.

This restriction is per operation, not per binding. Comparisons, equality,
classification, and fixed-width bitwise operations remain legal on a
Trapping-qualified value when those operations are total. The qualification
does not poison the value; it selects the behavior of operations that consume
the arithmetic-policy role.

The compiler never silently reinterprets a Trapping operation as Exact or
unbounded mathematics. An author chooses one of two explicit total readings:

```omega
requires
    embed(left) + embed(right) <= embed(i32::Maximum)

requires
    embed(right) >= 0
    embed(left) <= embed(i32::Maximum) - embed(right)
ensures
    result == (left as i32) + (right as i32)
```

The first is unbounded mathematical addition. The second explicitly removes
the Trapping qualification and forms an Exact `i32` addition after the earlier
facts discharge its ordinary representability obligation. They are not
synonyms, and an Exact operation cannot use the proposition containing that
same operation to justify its own formation.

## Integer embedding

`embed(value)` is a proof-only, total projection of a fixed-width integer or
address payload into proof `Int`. It performs no runtime conversion, allocates
no bytes, does not mutate or requalify the source binding, and cannot influence
runtime data or control.

Embedding retains the source carrier identity and contributes its exact range:

- an embedded unsigned integer or address is nonnegative and no greater than
  its carrier maximum;
- an embedded signed integer lies between its carrier minimum and maximum; and
- embeddings of equal source values are equal and remain injective within the
  source carrier.

Target-relative carrier limits enter the same proof language as canonical
compile-time observations. In particular, `addr::Bound: Int` is the selected
address carrier's exclusive one-past bound, and range geometry is stated
transparently:

```omega
pub proposition no_wrap(base: addr, length: u64) =
    embed(base) + embed(length) <= addr::Bound;
```

The observation may fold after target closure, but its exact target dependency
remains in the proposition, certificate, and artifact identity. The inclusive
upper bound does not imply the sum or length fits an `addr` or same-width
unsigned carrier: the one-past value is intentionally expressible only in proof
arithmetic.

Uniform `Int` is deliberate. Proof subtraction is therefore ordinary signed
subtraction even when the source carrier is unsigned:

```omega
let distance: Int = embed(end) - embed(start);
```

Proof `Nat` remains the carrier for natural induction, counts, quantities, and
nonnegative interval coordinates. Converting an `Int` into `Nat` is an Exact
proof-only coercion and requires nonnegativity:

```omega
let length: Nat = distance as Nat;
// obligation: distance >= 0
```

The checker normally discharges conversions of embedded unsigned values from
their derived range facts. A half-open address interval therefore remains an
`IntervalSet<Nat>`; its construction explicitly converts the embedded start and
one-past end rather than changing the published content algebra to signed
coordinates.

## Natural subtraction

Ordinary `Nat - Nat` is Exact. Forming the term requires the right operand to
be no greater than the left:

```omega
machine remaining(total: Nat, used: Nat) -> Nat
requires
    used <= total
{
    transition { _ -> total - used }
}
```

The entry fact discharges the formation obligation, just as a carrier bound
discharges an Exact machine-integer subtraction. Without that fact the term
rejects.

Clamping at zero remains useful but is explicit:

```omega
Nat::saturating_sub(left, right)
```

It denotes `max(left - right, 0)` and is total. Bare `-` never silently selects
this operation. The bootstrap proof library's current `Nat::sub`/"monus"
spelling is transitional; migrate it and its order lemmas to the explicit
`Nat::saturating_sub` name as the Exact operator surface lands.

## Denotation bridges

Let `M` be the unbounded mathematical result of one primitive over the embedded
operands, and let `[MIN, MAX]` be the selected result carrier. The compiler-owned
operation catalog first checks any primitive definedness conditions not decided
by the arithmetic policy, then publishes the following bridges:

| Policy | Formation or result law |
|---|---|
| Exact | formation requires `MIN <= M <= MAX`; `embed(result) == M` |
| Wrapping | `embed(result) == wrap(M, MIN, MAX)` |
| Saturating | `embed(result) == clamp(M, MIN, MAX)` |
| Trapping | on normal return, `embed(result) == M`; the executable operation traps exactly when its catalogued trap predicate holds |

The Trapping predicate is per primitive. For fixed-integer addition,
subtraction, and multiplication, trapping is equivalent to `M` lying outside
the result carrier. Division additionally names division by zero and the signed
`MIN / -1` case. Shifts name their invalid-count and overflow conditions.
Float adapters use their separately defined finite/special-value and policy
rules. The verifier must not replace this catalog with one convenient generic
"outside the range" approximation.

Shift-count definedness follows the settled primitive catalog from chapter 5:
Wrapping reduces every signed or unsigned count by Euclidean modulo of the
shifted value's width, so it has no invalid-count input; Exact and Saturating
require the count in `[0, width)` before the term forms; Trapping retains an
out-of-range count as an executable trap condition. Thus the invalid-count
condition named above is policy-sensitive, not a common partiality of all four
shift denotations.

The first proof-ledger migration slice now projects that exact count law
directly for fixed integer carriers. Literal counts normalize to `Truth` or
`Falsehood`; symbolic counts retain only the lower or upper bounds not already
implied by their carrier, in canonical order. Exact right shift may use a
kernel-checked prior-fact certificate for this unchanged goal, while a missing
proof retains the explicitly versioned trusted reduction dependency. Exact
left-shift result representability remains a separate obligation.

Cast, left-shift, addition, subtraction, and multiplication representability
share one canonical construction. A proof-only total mathematical term denotes
the source value or unbounded operation result without first forming a partial
machine operation. Carrier membership expands to its lower and upper
mathematical order bounds; left shift keeps its shift-count obligation
independent. Completely closed terms, bare source-carrier inclusion, and
vacuous bounds normalize deterministically. Symbolic interval propagation,
affine reduction, aliases, and every other fact-dependent simplification remain
producer proof steps checked against the unchanged canonical bounds. The
exact-cast projection and certificate route now implement this construction:
the verifier reconstructs the mathematical carrier bounds and checks the
producer-selected fixed-carrier derivation through canonical carrier
normalization in the existing proof calculus. Existing checked affine and cast
bound witnesses replay ordered exact arithmetic, landed shift counts, partial
casts, and widening identity edges; carrier-determined total images and exact
cast-intersection endpoints may start from a checked `Truth` child. This adds no
proof rule and gives the verifier no search authority. Exact shift-left and
exact add/subtract/multiply remain behind the optional projection and legacy
reducer; those fences are implementation status, not alternate semantics.
The current untrusted certificate producer searches direct signed affine words
through exactly twelve source-ordered definitions. Thirteen or more definitions
remain outside that fixed producer frontier; the verifier still replays only
the supplied witness against the unchanged canonical goal and proof calculus.

`Float::meaning32` and `Float::meaning64` are the corresponding explicit total
projections for floats. They produce `FloatMeaning`, retaining signed zero,
infinity, and NaN as distinct cases and representing each finite nonzero value
exactly with `Rat::NonZero`. A float is not embedded into `Int` or bare `Rat`.
Under D40 the proof kernel carries a closed FloatMeaning term and the
carrier-specific `FloatMeaningEqual` proposition only. The term key is the
verifier-reconstructed landed float source, exact binary format, projection
operation, and recognized declaration/catalog contract. Equal keys share one
canonical proof-value identity; distinct keys require an explicit theorem.
Source offsets remain diagnostics, and no proof term becomes runtime data.
The closed artifact contract retains rooted-checker tuples `(32, 1, 1, 1)` and
`(64, 2, 2, 1)` plus a commitment to the exact sealed owners, hermetic operator
identity, private contract-free ordinary signature, source carrier, nominal
result identity, and immutable catalog version. Same-format equality is
required independently during source validation, checked projection, and
Terminal replay. The checked plan now retains exact owner-machine and parameter
symbols for a direct primitive `f32`/`f64` parameter in its owning top-level
machine contract, after replaying entry-state membership and primitive format.
That provenance deliberately lowers through its fallback transitional ID:
Terminal has no general float scalar parameter carrier, so this prerequisite
does not widen execution or artifact trust. Results, nested state-contract
parameters, locals, members, casts, const parameters, non-floats,
foreign-owner sources, and other nonliteral forms remain transitional rather
than production proof-ledger evidence.

## Crash routes

A specification term never creates a crash edge. Only an executable Trapping
operation in the machine body creates a compiler-derived crash site. The author
states a public may-ceiling with total route predicates:

```omega
machine add(left: i32 in Trapping, right: i32 in Trapping) -> i32
crashes Trap
    embed(left) + embed(right) < embed(i32::Minimum)
    embed(left) + embed(right) > embed(i32::Maximum)
{
    // executable Trapping addition
}
```

For each derived, path-conditioned crash guard `D` and the authored guards
`C_1 .. C_n` for the same cause, checking requires:

```text
D implies (C_1 or ... or C_n)
```

An under-approximation rejects. A conservative over-approximation is sound but
publishes a less precise crash ceiling. Proving every authored route false at a
call removes that cause for the invocation. An `ensures` clause constrains only
normal returns and is vacuous on a crash path.

## Terminal representation

Terminal Psi retains only the total proposition expression actually written:
embedded `Int` terms and their source-carrier identities, Exact operations with
their discharged formation evidence, total Wrapping/Saturating terms, or D40
FloatMeaning projections with their reconstructed source and contract
correspondence. It has no proof-side Trapping arithmetic node and no
predicate-generated effect.

Executable Trapping operations independently retain their primitive identity,
catalogued guard, path condition, and crash edge. Independent verification
reconstructs both the denotation bridge and crash-route coverage; neither the
producer's chosen predicate nor the authored crash ceiling declares the
primitive's semantics.

## Rejected alternatives

- **Treat a trapping predicate as false on trapping inputs.** This silently
  strengthens the written proposition with a definedness condition.
- **Silently read Trapping arithmetic as mathematical.** This makes identical
  syntax select incompatible runtime and proof meanings.
- **Let a contract expression contribute a crash route.** Contracts erase and
  are never evaluated.
- **Admit partial propositions.** This would spread definedness or three-valued
  logic through the proof kernel for no additional expressive power.
- **Use proof `Nat` as the embedding target for unsigned carriers.** Ordinary
  subtraction would either be unavailable or silently clamp at zero. Explicit
  conversion into `Nat` preserves natural-number invariants without changing
  the meaning of mathematical subtraction.
