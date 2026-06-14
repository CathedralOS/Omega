# Architecture Smell: Scalar Type/Width Re-Derived, Not Threaded

Written 2026-06-14, after ~11 native miscompiles this session clustered in
instruction selection. This is the "is this emergent from bad architecture?"
assessment — and yes, a large fraction of them are one structural smell.

## The smell

A value's **scalar type and byte width** (i32 vs i64, f32 vs f64) is the input
to nearly every codegen decision: which move width, `addss` vs `addsd`,
`cvttss2si` vs `cvttsd2si`, `movss` vs `movsd`, signed vs unsigned. In the
current backend that information is **re-derived independently at each site
that needs it**, from the expression tree + storage descriptors, rather than
**computed once and threaded** on the value representation. The re-derivations
do not agree, and when they disagree the result is a silent miscompile (wrong
width move / wrong convert), not an error.

Concrete sites that each re-derive float width/type, found chasing the f32
canary family (all in omega-instruction-selection):

1. **Storage descriptors** — `resolve_runtime_storage_primitive_type_in_table`
   → a leaf descriptor's `primitive_type`. Authoritative for a place, but only
   for places.
2. **`classify_scalar_value_type_in_table`** (storage_places.rs) — for
   non-place expressions: a float literal is *assumed f64*; a binary is
   classified by recursing into operands and taking the "narrower". A separate
   re-derivation with its own defaults.
3. **Nested binary op emission** (isa-x86_64 `append_runtime_value_operand`)
   — hardcoded `append_runtime_float_binary_operation(.., 8)`; the code comment
   literally says "f32 value-operand arithmetic is a further gap." A *third*
   width source that ignores the operands entirely.
4. **Convert source width** (the `as` cast) — re-derives the source's width
   from `classify_scalar_value_type` again, independently of how the source
   was actually computed in (3).

## Why it produces miscompiles (the f32 family)

`let c: f32 = self.a + self.b; let n: i32 = c as i32` — `c` is **folded** (no
slot; verified in slots.txt — only `n` gets a slot). So the cast's source is
the binary `self.a + self.b`, and:

- the binary is **computed** by site (3) at hardcoded width 8 (`addsd` over
  f32 bit patterns → garbage in the high bits), while
- the convert **reads** it by site (4) at width 4 (`cvttss2si`, low 4 bytes).

Producer and consumer disagree on the width of the same value. Even patching
(3) to derive width from operands (tried this session, reverted — it did not
fix the canary alone) leaves (2)/(4) as independent derivations that can still
diverge: the fix has to make all sites read ONE answer, which a local patch
can't guarantee.

The deep-chain canary (`a+b+c+d`, depth ≥3 fails, depth 2 works) is the same
family seen from the operand-nesting angle: the re-derivation that "works" at
depth 2 stops agreeing at depth 3.

## The broader pattern (most of the session's 11 miscompiles)

Nearly all shared a shape: **instruction selection independently re-derives a
property that an earlier phase already knew (slot, width, scalar type, the
source place of a folded local), and the re-derivation either disagrees or
returns `None` and the write is silently dropped.** Examples fixed this
session: by-value struct arg/return (re-derived materialization missing a
StructLiteral case → no write), bare-local return (storage planner re-derived
"needs a slot" and said no), String `!=` (re-derived operand shape dropped the
text term). Each was patched at its site; the *class* keeps recurring because
the re-derivation architecture invites it.

## Remediation (how to make it less miserable)

Thread the answer instead of re-deriving it. Two levels, increasing payoff:

1. **Carry scalar type+width on `RuntimeValueOperand`.** Today `Binary` carries
   `is_float: bool` but no width; `Convert` carries widths but the source's
   actual computed width is a separate guess. Add the resolved
   `PrimitiveType` (or at least `{is_float, byte_width}`) to the operand at
   BUILD time (value_operands.rs, where the full expression context is
   available), and make every ISA emission READ it instead of re-deriving.
   One authoritative source, set once. This collapses the f32 family
   (nested-binary width, convert source width, and the classify default all
   become the same stored value).

2. **Make "no write strategy / unresolved width" a hard error, not a silent
   fallthrough — where safe.** The structure review found a blanket guard is
   unsafe (text-guard-through-ref legitimately falls through), but a *typed*
   "this float op has no width" or "this binary write resolved no target"
   assertion in the narrow paths that should always resolve would convert the
   silent-miscompile class into compile errors. Scope per-path, not blanket.

Neither is a one-turn change (operand-representation change spans the builder,
the trait, and both ISAs), but it is the difference between fixing f32 bugs
one canary at a time forever and removing the soil they grow in.

## A second face of the same smell: stale static-value fold across calls

Found 2026-06-14 (`calls/sequential_self_field_rmw_stale_fold`): a sub-machine
that does `self.s.total = self.s.total + 1`, called N times sequentially,
re-reads the entry/ZII value of `total` each call instead of the prior call's
write (accumulator stuck at 1). The within-one-body version of this was fixed
earlier (`invalidate_runtime_static_value` on a binary write — the
`v=v+5; v=v-3` case), but the per-call-context static-value state RESETS to
entry/ZII across a call boundary, so the fold re-derives a field's value from
entry state rather than reading live storage. Same root as above: a value is
RE-DERIVED (here from entry static state) instead of THREADED from the
authoritative live location. The static-value-fold optimization is itself a
re-derivation that must be invalidated wherever live storage diverges from the
entry snapshot — and call boundaries are one such place it currently misses.

This is why one-off patches don't hold for this class: each site that
re-derives (width, type, slot, static-fold value) is a separate place the
same mistake can recur. The remediation below addresses the width/type axis;
the static-fold axis wants the analogous treatment (a single "is this place's
entry snapshot still valid here?" authority, invalidated at every divergence
point including call boundaries) rather than per-site invalidation patches.

## Status

Not yet implemented. Acceptance tests for remediation:
- width/type axis: the f32 canaries (`expressions/f32_*`,
  `float_array_binary_op_zero`) pass together when scalar width is threaded.
- static-fold axis: `calls/sequential_self_field_rmw_stale_fold` passes when
  the entry-snapshot fold is invalidated across call boundaries.
See session_review_2026_06_12.md §1c for the per-canary roots.
