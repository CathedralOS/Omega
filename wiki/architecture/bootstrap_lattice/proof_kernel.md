# Proof kernel

[Lattice overview](bootstrap_lattice.md) | [Delta language rung](rungs/delta.md)

> **Status: WORKING PROTOTYPE.** Independent Beta and Gamma implementations
> accept valid certificates, reject invalid ones, and are exercised by logic,
> equality, operational-seam, fuzz, and cross-implementation gates.

The proof kernel is an assurance service used across the bootstrap lattice. It
is deliberately not a Greek-letter rung: it does not extend the programming
language chain, and programs do not elaborate through it. Producers at several
rungs emit certificates; the kernel checks them.

```text
Alpha → Beta → Gamma → Delta       language/bootstrap spine
              ↘       ↙
                proof kernel       cross-cutting assurance
```

The source lives in [`compiler/proof-kernel/`](../../../compiler/proof-kernel/).
The principal implementations are:

- `check.beta`: logical proof checking;
- `eq.beta`: definitional equality and conversion;
- `checker.gamma`: an independent Gamma implementation used for the checker
  diamond.

The kernel accepts a proposition and a certificate and returns accept or reject.
Proof search, elaboration, SMT integration, compilers, and other certificate
producers remain untrusted: they gain no authority by producing a candidate
certificate.

## Proof checking is not artifact verification

The proof kernel answers one deliberately small question:

```text
Does this certificate derive proposition P under these explicit premises?
```

It does not decide which proposition a terminal-Psi artifact ought to prove.
That is the job of the Psi-aware artifact verifier:

```text
canonical Psi artifact
    -> validate structure, identities, and contracts
    -> reconstruct every operation, edge, and return obligation
    -> validate every authorized admission against the consuming profile
    -> require exactly one permitted discharge for every obligation
    -> invoke the proof kernel for certificate-derived facts
    -> VerifiedTerminalModule
```

The proof bundle may carry a proposition for decoding and checking, but that
proposition must exactly match an obligation independently reconstructed from
the fingerprinted artifact. A certificate cannot omit an access, weaken an
author contract, reclassify a derivable fact as admitted, or prove an unrelated
theorem and attach it to different code.

The certificate is therefore self-contained as a derivation while remaining
bound to the exact artifact and obligation identities. The small proof kernel
need only understand the canonical proposition and derivation calculus. A
component must still understand terminal Psi well enough to reconstruct the
right propositions.

### Current arithmetic reconstruction

For proof-gated arithmetic the operation carries operands, result type, and an
obligation identity. The artifact verifier derives the proposition from terminal
carriers and path-local semantic facts; producer interval metadata is never an
axiom. Native lowering is authorized only after the exact proposition is
certified. Unsupported relations fail closed.

| Operation | Independently reconstructed sufficient forms |
| --- | --- |
| Exact cast | the source value is within the target carrier. When one partial fixed-native cast consumes a retained finite left-associated same-carrier exact-add/subtract literal-offset chain rooted at a direct parameter, the verifier follows ordered shrinking-prefix definitions, accumulates the checked mathematical offset, shifts the target interval back by that offset, intersects it with the source carrier, and reconstructs the surviving direct-root bounds independently of every prefix operation's own evidence. A retained finite left-associated same-source-carrier exact-multiply chain may likewise feed one partial fixed-native cast: the verifier accumulates independently landed nonnegative factors with checked arithmetic, maps the target interval back through the cumulative product, intersects it with the source carrier, and keeps every multiply-prefix proof independent. Product zero makes only the cast true, product one uses the ordinary target/source intersection, and larger products use the signed or unsigned inverse target bounds. A unified same-source-carrier affine chain containing both offset and multiply operations may also feed the cast: the verifier replays checked `A * root + B`, maps the target interval back through positive `A`, or decides a zero-coefficient cast solely from target representability of `B`, without importing any prefix proof. Finite same-source-carrier exact-left- or exact-right-shift chains may also feed the cast with independently landed heterogeneous legal counts. Left shift uses inverse target bounds or a zero result at source width; right shift uses the arithmetic/zero-fill target preimage, saturating to zero for unsigned sources and `-1` or zero for signed sources. A mixed-only finite left-associated chain containing both shift kinds may also feed the cast: the verifier starts from the target/source carrier intersection and maps it backward through every ordered inverse-left or inverse-right definition. Mathematical emptiness is falsehood; checked transfer failure is no admission. A finite exact-divide/remainder chain may feed the cast only when verifier-replayed toward-zero quotient and dividend-sign remainder interval hulls map the full source carrier wholly inside the target; guard-sensitive nonconvex preimages remain outside this family. Every prefix proof remains independently mandatory. |
| Exact right shift | `0 <= count < width`. In a retained finite left-associated chain, every link reconstructs independently from its own landed in-range count; the prior shifted-value definition is an operand, not proof authority. One direct partial fixed-native cast may root the same finite chain: the cast retains independent representability evidence, while each shift prefix remains independently true from only its own landed legal count. This independent count proof is unchanged when the post-cast chain contains both shift kinds. |
| Exact left shift | legal count plus carrier-tight no-overflow bounds. In a retained finite left-associated chain, the verifier follows only ordered shrinking-prefix definitions to a direct same-carrier parameter, checks every independently landed in-range count, accumulates counts with checked arithmetic, and reconstructs each link's cumulative carrier-tight root bound; a cumulative count at least the value width admits only the zero root. A direct partial fixed-native cast may instead root such a finite nonempty same-value-carrier chain: the cast proves representability separately, while every shift prefix shifts the target interval right by its checked cumulative count and intersects it with the source carrier; heterogeneous fixed-native count carriers remain legal. One direct-root mixed family admits any finite left-associated chain containing both exact-left and exact-right shifts. Each left prefix independently maps its safe input interval backward through every prior canonical definition: inverse left shift uses ceiling/floor division by the power of two, inverse arithmetic or zero-fill right shift uses `[a*2^k, (b+1)*2^k-1]`, and every step intersects the carrier. The same mixed-only chain may be rooted at one direct partial fixed-native cast: each left prefix walks the ordered definitions back to the cast and intersects the resulting target interval with the source carrier before emitting canonical source-root bounds. No cast or earlier shift proof is imported, so later shifts cannot erase unsafe prefixes. Mathematical emptiness is falsehood; checked transfer failure is no admission. |
| Exact add | a known addend and its complementary carrier bound; unsigned `left <= MAX - right`; signed positive/negative variants with the sign fact that makes the bound operation total. In a retained left-associated mixed exact-add/subtract chain, the verifier follows ordered shrinking-prefix definitions to a direct same-carrier parameter, combines landed same-carrier right literals as additions or mathematical negations of subtrahends, and reconstructs every prefix from the checked cumulative offset. A direct partial fixed-native cast may instead root such a finite nonempty same-target-carrier chain; the cast proves direct representability independently, while every arithmetic prefix walks back to the canonical cast definition and reconstructs its own target interval shifted by the checked cumulative offset and intersected with the source carrier. The post-cast chain may widen to the unified mixed affine family only when offset and multiply operations both occur: each prefix replays checked `A * source + B` against the target/source intersection, retaining every earlier proof across zero or cancellation. |
| Exact subtract | a known right operand and its complementary carrier bound; unsigned `right <= left`; signed positive/negative variants with the sign fact that makes the bound operation total. The same finite post-cast mixed-chain rule reconstructs every subtract prefix independently, using the mathematical negation of each landed subtrahend; cancellation never replaces an earlier prefix obligation. |
| Exact multiply | a known factor and the carrier-tight interval; a positive runtime factor plus `MIN / factor <= value <= MAX / factor` for signed carriers, or the upper bound for unsigned carriers; runtime factor `-1` plus `MIN + 1 <= value`; a signed factor at most `-2` plus `MAX / factor <= value <= MIN / factor`. In a retained finite left-associated chain, the verifier follows only ordered shrinking-prefix definitions to a direct same-carrier parameter, accumulates explicitly landed nonnegative right factors with checked arithmetic, and reconstructs each link's cumulative carrier-tight root bound independently. A direct partial fixed-native cast may instead root such a finite nonempty same-target-carrier chain: the cast proves representability separately, while every multiply prefix divides the target interval by its checked cumulative product and intersects it with the source carrier; product zero or one makes only that prefix true, and a later zero cannot erase earlier evidence. A wider retained affine chain may interleave exact add/subtract with exact multiply only when both families occur, whether rooted directly or at one direct partial cast: the verifier replays every ordered prefix as checked `A * root + B`, maps the carrier back through positive `A`, or decides a zero-coefficient prefix solely from `B`, while preserving every earlier obligation. |
| Exact divide/remainder | a known safe divisor; `1 <= divisor`; `divisor <= -2`; or `divisor <= -1` together with `MIN + 1 <= dividend`. In a retained finite mixed divide/remainder chain, every link reconstructs independently from its own safe divisor; an earlier result definition is an operand, not proof authority. One direct partial fixed-native cast may root the same finite chain: the cast retains independent representability evidence and every prefix remains independently true from only its own divisor proof. Either root form may contain direct same-carrier runtime divisors when at least one occurs: each runtime divisor independently supplies its positive or at-most-`-2` proposition, while the joint `-1`/dividend exception is available only to the first direct-root operation whose dividend bound is independently reconstructed. Computed and post-cast dividends import no such authority. |
| Wrapping/Saturating divide/remainder | a known nonzero divisor, `1 <= divisor`, `divisor <= -2`, or `divisor <= -1`; policy defines the signed `MIN`/`-1` result |

Path facts must reach the operation through verified terminal control. Count
masking, machine overflow behavior, or a producer claim cannot discharge an
obligation. The verifier passes only the reconstructed proposition and exact
semantic axioms to the small kernel.

## Trust and meaning

Kernel acceptance defines certificate validity, not program behavior. A separate
soundness bridge must connect each checked proposition to the execution semantics
it claims to describe:

```text
kernel-accepted certificate  ⇒  claim true about the specified execution model
```

That bridge is the central open proof obligation. Current seam tests compare
kernel derivations with independent operational decisions and reject perturbed
claims; they are evidence and regression gates, not the final metatheorem.

The kernel remains on the audited bootstrap lineage. The Beta and Gamma versions
are independently implemented and compared, so moving the directory or removing
its former rung name does not weaken its assurance role.

A later Psi implementation may provide a faster third implementation of the
same kernel. It joins the checker diamond only over the exact shared calculus
and certificate semantics, and only when it is independently implemented; a
port of an existing checker tests the compilation path but is not implementation
diversity. The Beta and Gamma implementations remain the low-rung reference
route unless a separately justified trust transition replaces them.

## Recursive and normalization certificates

The Rust kernel checks recursive components and algebraic normalization using
the settled shapes below. Source automation does not yet emit those records,
and terminal Psi does not yet reconstruct or retain them; that bridge remains
open without making the source entailment engine trusted.

A recursive certificate is organized by the strongly connected component of
the proof-call graph. The component cites its ranking relation and one proof of
that relation's well-foundedness. Each application edge within the component
then proves only its local obligation:

```text
ranking_relation(callee_measure, caller_measure)
```

The component rule makes every member's contract available only at strictly
smaller measures. A call outside the component is an ordinary contract
application. Thus self induction and mutual induction use the same kernel rule,
while an unmeasured proof cycle cannot disguise itself as a sequence of
ordinary citations. The well-foundedness proof is shared component evidence;
the decrease proof is edge evidence. Both participate in provenance.

A normalization certificate identifies the selected conformance and exact law
evidence used to justify the canonicalization. Replaying a total normalizer may
compress primitive inference steps, but it does not erase premises. The trust
closure of every cited law is inherited by the normalized conclusion, including
admitted laws.

The review synopsis is a deterministic projection of the checked certificate,
not a source-side explanation pass. It names the certificate fingerprint and
renders its recursive components, closure rules, cited laws, and trust closure.
Source attribution may decorate certificate nodes, but cannot substitute for a
certified derivation.

The tree-certificate checker records exact rule families and cited
assumption/semantic-axiom propositions. The terminal artifact layer
fingerprints the accepted proof bundle and renders its current review synopsis
only from a `VerifiedTerminalModule`, so changing an accepted proof route
changes both its fingerprint and trust record. The recursive-component checker
requires a canonical member/edge set, one selected relation and
well-foundedness route, and exact per-edge decrease evidence. The normalization
checker pins the selected conformance and law set, verifies every law route,
and requires the conclusion to cite every law premise. Both retain admissions
in provenance. Terminal vocabulary, source production, verification, and the
synopsis still need to carry these two record families end to end.

## Scope discipline

- The kernel contains proof rules and deterministic certificate checking.
- Automation and proof search live outside it and must materialize evidence.
- New logical capabilities require negative tests and an operational or semantic
  seam where one exists.
- During pre-release development the certificate producer and every checker move
  together. Stale certificates reject; this page describes only the current
  calculus.

The remaining certification bridge, terminal reconstruction closure, and
soundness work is tracked once under P3 in
[`TASKS.md`](../../../TASKS.md).
