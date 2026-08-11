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
| Exact cast | the source value is within the target carrier |
| Exact right shift | `0 <= count < width` |
| Exact left shift | legal count plus carrier-tight no-overflow bounds |
| Exact add | a known addend and its complementary carrier bound; unsigned `left <= MAX - right`; signed positive/negative variants with the sign fact that makes the bound operation total |
| Exact subtract | a known right operand and its complementary carrier bound; unsigned `right <= left`; signed positive/negative variants with the sign fact that makes the bound operation total |
| Exact multiply | a known factor and the carrier-tight interval; a positive runtime factor plus `MIN / factor <= value <= MAX / factor` for signed carriers, or the upper bound for unsigned carriers; runtime factor `-1` plus `MIN + 1 <= value`; a signed factor at most `-2` plus `MAX / factor <= value <= MIN / factor` |
| Exact divide/remainder | a known safe divisor; `1 <= divisor`; `divisor <= -2`; or `divisor <= -1` together with `MIN + 1 <= dividend` |
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

## Settled certificate growth

The live kernel does not yet check source-level recursive proofs or algebraic
normalization. When those enter the shared calculus, they follow one settled
shape rather than importing the legacy entailment engine as trusted code.

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

The live tree-certificate kernel records the exact rule families and cited
assumption/semantic-axiom propositions during the accepting traversal. The
terminal artifact layer fingerprints the exact accepted proof bundle and
renders its review synopsis only from a `VerifiedTerminalModule`; changing a
valid proof route therefore changes both the fingerprint and rendered trust
record. The kernel also has a total recursive-component checker: reconstruction
owns a canonical strongly connected member/edge set and selected relation; one
certificate supplies the single well-foundedness route and exact per-edge
decrease evidence, with admissions retained in the returned provenance. The
normalization checker similarly pins the selected conformance and canonical law
set, verifies each law route, and requires the conclusion certificate to cite
every law premise; an admitted law therefore remains explicit in the accepted
normalization record. The terminal vocabulary, source producer, and synopsis
still need to carry both records rather than adding a second explanation path.

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
