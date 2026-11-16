# What each stage may trust

A compiler stage consumes its predecessor's declared output instead of rebuilding
its decisions. That does not make the compiler infallible or a digest a proof.
[Verification placement](../../../omega-rust/pipeline.md#verification-placement)
owns the current checks; this note explains the distinctions at their handoffs.

## The decision

| Boundary | Required treatment |
| --- | --- |
| A stage's output within one compilation | Consume its representation and exact retained identities. Do not reconstruct selections from source spelling or call the same producer again as an alleged independent validator. |
| A source proof obligation | The lowered-form checker establishes it on the module being checked. Proof search supplies evidence to the proof-admission kernel; an unproved obligation rejects, not a slower guarded-native fallback. Structural ownership and loan judgments keep their own checks. |
| A separately received artifact | Decode its current canonical structure under bounded resource limits. Reuse judgments only under receiving-host authorization of the exact artifact and producer. A hash or successful decoding grants no execution authority. |
| An installed provider or external entry | Establish actual calling, storage, authority, lifetime and resource obligations. A signature or provider selection does not manufacture backing or permission. |

The backend consumes accepted judgments under the selected installation contracts;
it does not repeat source proof search. Trusted-artifact reuse does not require
certificate replay merely because evidence bytes are present. See
[publication and separate consumption](../../spec/terminal-psi/product.md#publication-and-separate-consumption).

## Evidence, seals and validation

A seal commits to bytes and identities. A consistency check compares related
producer records. An independent validator derives its checked property without
using the producer's builder as its expected answer. Name the actual check and
its coverage; none of these descriptions substitutes for another.

Default compilation does not independently revalidate every stage output.
Focused tests and explicit
[validator requests](../../spec/build/command_line.md#independent-stage-validators)
exercise those validators. Required source checking, artifact admission and
installation checks are not optional stage validators.

The checker's obligations must correspond to the checked module's operations
and control flow. Proving every item in a producer-written list is insufficient
if that list omitted an obligation or assigned it to another value. Tests must
expose removed, weakened and misassociated obligations, not merely count verdicts.
The [lowered-form design](check_on_lowered_form.md#the-gate) describes these controls.

An absent result, unsupported construct or declined validation is not success.
Retain its reason and source occurrence. A generated operation without a source
position reports that absence rather than guessing a nearby declaration.

## What execution comparisons establish

`omega run --both` compares one Psi program through interpreted and native
execution. Agreement is evidence for the inputs, paths and observations that
actually ran. Both routes can share an upstream source-meaning error.

A semantic repair therefore needs its own expected-result and rejection tests,
plus the applicable independent boundary checks. Publication repairs exercise the
actual source-independent consumer, without live producer source objects as an
accidental fallback. A modified claim or broken association must fail at the
boundary responsible for it.

A missing engine, timeout, skipped comparison or unavailable validator remains
distinguishable from success. These are coverage limits, not alternative ways
to satisfy the [release gates](../reference/rust_compiler_completion.md).

## Lessons to preserve

Trust belongs to a checked judgment and its exact subject, not a label such as
`source_checked`, a fingerprint or a matching record count. Two consumers agreeing
on the same wrongly produced input do not establish that input's correctness.
Discarded identity makes later consumers guess; retain the relationship at its
owner instead.

Implementation gaps belong in [TASKS.md](../../../TASKS.md), and concrete
cross-cutting defects in the [repair queue](../../../OWNER_CODE_CONCERNS.md).
Superseded audits, traces, measurements and fallback designs live in Git history,
not alongside the current instructions.
