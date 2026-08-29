# Proof kernel

[Lattice overview](bootstrap_lattice.md) | [Omega product toolchain](omega_toolchain.md) | [Terminal Psi](../pipeline/terminal_psi.md)

> **Status: ROOTED CHECKER SERVICE; EDGE ADMISSION OPEN.** The authoritative
> Beta implementation and one independent diagnostic reference exercise the shared calculus, and the accepted checker
> tape is constructed directly from the Alpha-written Beta compiler source,
> independently of the persisted Beta compiler artifact. The lattice
> requires every compiler edge to relate its immediate-predecessor source
> directly to Alpha tape. Former self-host-specific proof machinery was deleted;
> retained machinery targets the canonical Alpha-written Beta compiler or a
> later source-to-tape edge. No coinductive kernel extension is selected.

The proof kernel is deliberately not a language rung. Programs do not elaborate
through it, and it adds no stage between Gamma and Delta. It is an Alpha-owned
service used by producers and artifact verifiers throughout the build lattice.

Its canonical owner is `source/alpha/checker/`. The authoritative Beta and
untrusted executable reference implementations live under `implementations/`;
the one retained deterministic generator and executable policy live under
`corpus/` and `gates/`. A theorem library, proof-search stack, conversion layer,
or collection of language-hosted checker copies is not part of the service.
The product-local Rust crate `psi-proof-admission` remains under Psi semantics;
it checks Psi judgments and admission policy and is not this generic bootstrap
derivation checker. Its role name does not move it into the bootstrap assurance
owner.

```text
Alpha source ───────────────▶ Beta compiler tape
Beta source  ───────────────▶ Gamma compiler tape
Gamma source ───────────────▶ Delta compiler tape
Delta source D ─────────────▶ omega₀ compiler tape
Omega source C ─────────────▶ omega compiler tape
                 all targets are exact Alpha tape

Alpha-owned proof checker ──▶ checks edge propositions reconstructed by owners
```

Using one target machine is a material simplification: target semantics,
observations, decoding, VM realization, and most simulation infrastructure are
shared by every row. Source stepping and source-to-Alpha relations remain
language-specific; this common target does not justify transpiling one source
language through another.

## Judgment

The kernel answers one small question:

```text
Does certificate C derive proposition P under explicit premises Γ?
```

It accepts or rejects. It does not search for proofs, optimize programs, compile
source, reconstruct artifact obligations, or decide which claims are sufficient
for deployment. Those larger components may be arbitrarily sophisticated, but
they gain no authority by producing a candidate certificate.

## Implementations

The retained implementations are:

- `source/alpha/checker/implementations/beta/check.beta` — logical proof checking in Beta;
- `source/alpha/checker/implementations/beta/eq.beta` — fuel-bounded definitional equality;
- `source/alpha/checker/implementations/reference/check_ref.py` — one
  independently written, untrusted complete diagnostic checker.

The compact discriminator and adversarial suites exercise the authoritative
checker. One deterministic differential generator checks the complete retained
rule set against the independent reference, and one bounded operational seam
compares definitional equality with Gamma evaluation. Agreement is evidence
while the formal soundness bridge matures; it does not grant the reference
authority over artifact-specific obligation reconstruction. The accepted
checker artifact is reconstructed independently below `bc`. Checker acceptance
can authorize a compiler edge only after an artifact-aware producer
reconstructs the exact proposition and supplies a derivation in the kernel's
supported calculus.

## Proof checking is not artifact verification

A proof bundle can honestly prove the wrong proposition. Therefore the kernel
cannot, by itself, validate a terminal-Psi artifact.

The artifact-aware route is:

```text
canonical terminal-Psi bytes
    → validate canonical structure and identities
    → reconstruct the complete ordered semantic ledger
    → derive the exact obligation set for this artifact
    → require exactly the permitted evidence for every obligation
    → invoke the proof kernel on certificate-derived facts
    → VerifiedTerminalModule
```

The proposition carried for a certificate must exactly match an obligation
reconstructed independently from the fingerprinted artifact. A producer cannot
omit an access, weaken a contract, relabel a derived fact as admitted, or attach
a valid theorem to unrelated code.

This is why Gamma is not “the Psi proof checker.” Gamma's evaluator participates
only in one bounded semantic diagnostic; it does not own checker authority. A
low-rung Psi-aware semantic-ledger generator supplies the artifact-specific
reconstruction assurance. A future Psi- or Omega-hosted kernel
may accelerate or independently cross-check validation, but it cannot replace
that reconstruction step merely by understanding the proof calculus.

## Canonical semantic ledger

The deployment endpoint consumes canonical terminal-Psi bytes and produces one
ordered semantic ledger containing every required goal, permitted premise,
identity, validity scope, establishment point, invalidation, and justification
dependency. Direct low-rung evaluation or a checked derivation of that same
ledger is authoritative; agreement with the current optimized Rust verifier is
diagnostic only.

Local operation denotations and goal shapes come from closed, typed declarative
schemas. Multi-operation algebraic reduction remains untrusted and must emit a
kernel-checked derivation from exact ledger premises to the unchanged canonical
goal. Adding a proof-bearing operation therefore does not add an opaque trusted
verification function.

The detailed operation inventories, arithmetic reconstruction families,
composition rules, canonical byte formats, and current Rust migration closure
belong to [Terminal Psi](../pipeline/terminal_psi.md) and the implementation
READMEs. They are intentionally not duplicated in this kernel-boundary document.

## Trust and soundness

Kernel acceptance is authoritative only for the kernel judgment. Connecting
that judgment to meaning requires the certificate to identify its exact
semantic subject. Artifact acceptance has one terminal subject: the produced
artifact refines the canonical source operational semantics under a
verifier-reconstructed observation profile. Supporting derivations may instead
state a judgment in one exact intended mathematical model, or a global
consequence over every model of one exact theory. They become usable by the
artifact claim only through explicit checked bridges.

The complete assurance graph distinguishes every node:

```text
global theory consequence: T entails P
    + exact intended model M satisfies T
        -> P holds in M

intended mathematical model M
    <-> domain- and operation-indexed representation bridge
canonical source operational system S
    <- refinement under required observation profile O
formal target operational system T

physical deployment H
    -- disclosed realization admission --> formal target T
```

The hardware edge belongs to deployment assurance, not the reusable artifact
seal. Identical bytes may be verified once against `T`; each deployment reports
the irreducible admission that its silicon realizes that formal target.

Source already marks several bridge applications. `satisfies` names an
implementation-to-trait-theory join, discharged by checked conformance evidence
or a disclosed admission. `embed` denotes an injection from a fixed-width
machine carrier into a bounded subset of proof `Int`; exact `as Nat` separately
consumes nonnegativity. `terminates by` maps operational states to a
well-founded mathematical rank and owes a decrease proof on every cycle edge.
These forms identify where a bridge applies; none authenticates its own bridge.

Representation is indexed by carrier, mathematical model, arithmetic domain,
operation, and semantics version. Exact arithmetic owes a no-overflow proof and
otherwise rejects; it does not acquire a trap branch. Wrapping commutes with
modular arithmetic, Saturating with clamping, and Trapping relates both its
successful and trapping outcomes.

The observation profile is part of the verifier-reconstructed obligation, not
producer configuration. Canonical source semantics, formal target semantics,
boundary/component contracts, and consumer deployment policy determine it. A
producer may neither choose nor weaken the profile. Exact profile identity is a
sound conservative replay gate. Normatively, reuse across profiles requires a
canonical checked forgetting projection from the proved profile to the
requested profile; profiles may be incomparable, so names or field inclusion
never imply strength.

Every certificate and bridge records the exact subject/model/theory identity,
semantics version, observation profile, target capsule, admitted premises, and
bridge dependencies. No transitive join is inferred merely because two roots
mention similar propositions. Consequently, an honest verdict is always
qualified: artifact `A` refines source `S` under profile `O` and semantics
versions `V`, subject to admissions `D`. Human-facing reports must not collapse
that into an unqualified `verified` label.

The full metatheorems connecting kernel judgments, intended models, and pinned
operational execution remain research work. Today every logical capability must
ship with an operational seam that compares kernel-provability with independent
evaluation or decision on a positive corpus and a negative battery. These seams
are strong regression evidence, not substitutes for the theorems.

Fuel bounds ensure normalization and checking terminate operationally. Fuel
exhaustion is rejection or an explicit incomplete result; it is never evidence
that a proposition is false and never termination evidence for the program being
proved.

## Cyclic operational refinement

Loops do not by themselves require a greatest-fixed-point rule in the kernel.
For a source-language-to-Alpha compiler-correctness edge, the artifact-aware
owner reconstructs constructive total step functions for the canonical source
and Alpha machines. Terminal outcomes self-loop, so primitive recursion defines
one coherent state at every natural-number index; this avoids treating
`forall n. exists a prefix`
as though it constructively selected one infinite execution.

The systems need not advance in lockstep. A nondecreasing synchronization
function maps source indices to artifact indices. One source step may lower to
many Alpha steps, while an erased source operation may lower to none. Every
single-sided step is required to preserve the published observation and
decrease a well-founded rank over the related source/artifact state pair.
Matched progress may establish the next rank. This prevents either system from
stuttering forever while claiming correspondence.

States, steps, observations, relation schemas, synchronization, determinism,
progress, and rank decrease are represented by existing terms and predicates.
Their proofs use the current first-order rules and natural-number induction.
An untrusted elaborator may construct or compress those derivations; it cannot
add a trusted LTS judgment or assert a reconstructed premise. Reusable generic
simulation lemmas are checked once and referenced through a shared proof DAG.

The Alpha assembly source used to construct the Beta compiler is not such an
edge. Its authoritative encoder must produce the exact persisted tape; byte
equality gives identical Alpha initial programs and therefore lockstep traces.
Non-lockstep machinery begins with proving that the compiler correctly lowers
arbitrary Beta source, and with later compiler rungs.

The first implementation must measure certificate size and checking time and
exercise termination, divergence, zero-instruction source steps, multi-step
artifact lowering, infinite internal stuttering, changed observations, weakened
input profiles, and swapped subjects. Performance pressure first justifies DAG
sharing, reusable proved lemmas, and better elaboration. Only an identified
expressiveness failure can reopen the kernel-rule decision.

## Producer and self-host boundaries

The Delta-written compiler `D`, `omega₀`, and production Omega may all emit
certificates. No compiler decides whether its own evidence is valid. A compiler
bug can reproduce into production Omega, but it
cannot forge a derivation accepted by an independent sound kernel or choose a
different artifact obligation when canonical reconstruction is in place.

Self-hosting closes external compiler dependencies. Proof checking, canonical
meaning, artifact reconstruction, and translation validation establish semantic
assurance across the self-host edge.

## Scope discipline

- The kernel grows only when an actual obligation class requires a new logical
  rule.
- Every new rule lands in each independent implementation with positive,
  negative, differential, and operational-seam coverage.
- Proof search, SMT procedures, algebraic reducers, optimizers, and compilers are
  untrusted certificate producers.
- Artifact-specific semantics remain in the canonical ledger definition, not in
  generic kernel rules.
- Production optimization remains entirely outside the kernel.
- Proof size or checker time that remains prohibitive after proof-DAG sharing,
  reusable compositional lemmas, and removal of redundant evidence requires an
  owner escalation. It does not authorize a new trusted rule.
- Existing proof code that cannot be adapted to a canonical compiler edge or a
  focused kernel semantic test is deleted; historical effort is not a trust
  premise or a maintenance justification.

The theoretical
[matching-logic research lane](../../design_briefs/matching_logic_proof_research.md)
does not alter this boundary. It evaluates matching logic as a producer,
independent diamond, or import format for the same verifier-reconstructed,
subject-qualified obligations before any kernel role is considered.
