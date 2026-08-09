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

Terminal Psi v28 exercises this split for exact fixed-integer casts. The
producer carries an obligation identity but does not choose its proposition.
The Psi-aware verifier derives the representability bound or conjunction from
the source and target carriers at the operation site, reconstructs only
path-local comparison facts that reach that site, and passes those facts as
semantic axioms to the small kernel. Removing the certificate, moving the cast
off the proved edge, changing either carrier, or involving `addr` rejects
without treating producer range metadata as authority.

Terminal Psi v29 applies the same separation to Exact integer right shift. The
operation carries only its value, independently typed count, result, and
obligation identity. The verifier derives the nonnegative and/or
`value_width - 1` bounds that the count carrier does not guarantee, reconstructs
path-local comparison facts, and asks the kernel to check the dedicated
certificate. Count masking in a native instruction cannot discharge this
obligation. Terminal Psi v30 closes the left-shift side without collapsing the
facts: its operation-owned proposition conjoins those count bounds with a
distinct value no-overflow bound. The first runtime surface uses bounds safe at
the worst legal count (`value <= 1` unsigned; `-1 <= value <= 0` signed) unless
prior terminal facts determine one exact legal count or a finite legal count
ceiling. In that case the verifier reconstructs the carrier-tight shifted
minimum and maximum for the largest possible count and retains the exact ceiling
as a certificate conjunct. Both paths use terminal carriers and path-local facts
rather than producer range metadata.

Terminal Psi v31 applies the same split to Exact fixed-integer addition. The
operation carries its two same-typed addends and obligation identity, while the
verifier independently resolves terminal-known constants and reconstructs the
carrier-tight bound on the other addend. The producer's interval proof is not
an axiom. The first surface therefore accepts literal/terminal-equality addends
under matching path-local bounds and rejects two unrelated runtime addends.
Native wrapping-width addition is authorized only after that certificate has
established representability.

Terminal Psi v32 applies the split to Exact fixed-integer subtraction. The
operation carries its two same-typed operands and obligation identity, while
the verifier independently resolves a terminal-known right operand and
reconstructs the carrier-tight lower or upper bound on the left operand. The
producer's interval proof is not an axiom. The first surface accepts
literal/terminal-equality right operands under matching path-local bounds and
rejects an unknown right operand or other two-runtime relation. Native
wrapping-width subtraction is authorized only after that certificate has
established representability.

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

## Scope discipline

- The kernel contains proof rules and deterministic certificate checking.
- Automation and proof search live outside it and must materialize evidence.
- New logical capabilities require negative tests and an operational or semantic
  seam where one exists.
- The certificate format and the soundness connection are versioned compatibility
  surfaces.

## Open work

- Finish the formal soundness bridge to the canonical execution semantics.
- Stabilize the certificate vocabulary consumed by terminal Psi.
- Connect `psi-terminal-verifier` to the low-rung kernel format and decide the
  final trust placement of terminal-Psi obligation reconstruction: low reference
  verifier, checked derivation of reconstruction, or explicit trusted component.
- Reconcile future fast native checkers against the small reference route.
