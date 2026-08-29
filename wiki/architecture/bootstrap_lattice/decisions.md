# Lattice — ratified decisions

This file records architectural decisions. Current implementation order lives
only in [`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## D1 — Rust exits by role, not by rung

The Rust implementation under `source/omega-rust/` may remain as a comparator,
migration aid, and working product while the lattice closes. It supplies no
semantic authority. Meaning/checking dependencies leave the trusted path before
convenient producers need to disappear.

## D2 — Delta meaning elaborates nonoptimizingly to Gamma

Delta has an independent language contract. Its canonical lower-rung meaning
route elaborates Delta to Gamma and evaluates through Gamma's Beta-written
semantics. Optimizations are not part of that meaning edge.

## D3 — Trust flows through proofs, not native pedigree

Native artifacts are accepted by checked source-to-artifact refinement under a
pinned semantics and observation profile. Reproducibility, producer identity,
and independent compiler agreement are useful evidence for operations and
debugging but are not correctness proofs.

## D4 — Every proof capability lands with operational seams

A calculus feature is not complete merely because the kernel parses it. Each
feature has positive seams to a real compiler obligation and negative controls
that perturb the claim and must reject.

## D5 — Compiler provenance closes by direct checked refinement

The required judgment is:

```text
source subject + produced artifact
  → artifact refines canonical source meaning
```

The obligation is reconstructed from both subjects. An artifact cannot define
the question against which it is accepted. Subject identity, semantics,
observation profile, target dependencies, schema identity, certificates, and
admissions all participate in replay compatibility.

## D6 — Delta directly builds the first Omega compiler

The permanent chain is:

```text
Alpha → Beta → Gamma → Delta-produced compiler
Delta-produced compiler + C → omega₀
omega₀ + the same C            → omega
```

`C` is the exact Omega compiler source closure. It deliberately uses only a
compositional subset of ordinary Omega.
There is no separately owned bridge compiler, bridge source tree, or bridge
refinement layer. `omega₀` is already the product compiler, though its generated
code may be conservative.

Delta v1 and the source actually used by `C` are distinct facts:

| Contract | Meaning |
| --- | --- |
| Delta v1 | independent compiler-host language used to implement the direct first product edge |
| features used by `C` | incidental ordinary-Omega subset used to author the product compiler |
| full Omega | language implemented for users by the product compiler |

The Delta compiler may reject Omega source outside the surface exercised by
`C`, but accepted source keeps normal Omega semantics. The product compiler built from `C` implements
full Omega. Rebuilding the same `C` changes the artifact, not the compiler's
source identity or language contract.

## D7 — Scripts coordinate but never become stages

Shell, Python, or other host scaffolding may invoke the chain during
construction. It may not discover `C`, parse or lower accepted programs,
manufacture evidence, or decide trust. A required semantic transformation must
be implemented in the named compiler/checker stage, not hidden in a runner.

## D8 — Evidence and admissions compose transitively

Verification records are re-derivable and subject-indexed. Admissions remain
visible through dependency closure and are re-evaluated under each consumer's
policy. A dependency cannot launder an unresolved obligation into a clean
“verified” result.

Human-facing output keeps verified facts, admitted claims, and provenance
metadata visually and semantically distinct. There is no unqualified
“artifact proven correct” verdict; the honest statement always names the
profile, semantics, observation profile, and admissions.

## D9 — Cyclic compiler refinement starts inside the existing calculus

The Beta compiler edge does not add a coinductive or labelled-transition-system
judgment to the accepted checker merely because the subject contains loops.
Beta already has canonical deterministic small-step semantics. The admission
owner first presents those semantics and Alpha execution as constructive total
step functions, with explicit terminal states that self-loop, and defines each
trace by primitive recursion over `Nat`.

The proof relates source and artifact states at nondecreasing synchronization
points. A source step may correspond to zero or more artifact steps. Every
unmatched step must be observationally silent and decrease one well-founded
rank over the related state pair, so neither side can hide infinite internal
work. Symbolic relation rows are ordinary predicates and their determinism,
progress, observation, synchronization, and rank obligations are ordinary
intuitionistic first-order propositions discharged using the existing
induction rules.

The producer may elaborate and DAG-share the proof, but the artifact-aware
owner reconstructs both machines, the exact input/resource profile, and the
observation profile. ROOT execution and agreement with the Gamma checker remain
diagnostic evidence. A new trusted kernel primitive is considered only after a
concrete attempt proves that the required theorem is inexpressible in the
existing calculus; certificate verbosity or producer inconvenience alone is
not such a proof.

## D10 — Delta meaning is independent; spelling and artifact names are explicit

Delta v1 is an independent compiler-host language whose canonical meaning is a
separately fixed, syntax-directed elaboration to Gamma followed by the pinned
Gamma semantics. Delta may deliberately share familiar source spelling with
Omega, but its language contract is self-contained: neither the contract nor a
checker may consult Omega documentation, the current translator, or the
accepted corpus to decide what Delta source means. A file accepted by both
languages acquires only the meaning of the route under which it is checked;
Delta acceptance is never evidence of Omega refinement.

Delta refinement quantifies over a verifier-selected observation profile. The
profile fixes the sealed input-byte domain and admissible input profile, exact
artifact and diagnostic bytes, and terminal exit, rejection, trap, and
language-level exhaustion outcomes. Ambient filesystem state, environment,
time, network, process spawning, and other host observations are absent unless
the contract adds them explicitly.

Resource classification is part of authoring that contract, not a separate
pre-pass. Each apparent fixed bound becomes exactly one of:

- a source-visible Delta semantic bound;
- an explicit resource-profile parameter; or
- a private producer/checker budget whose exhaustion yields `Incomplete` and
  grants no semantic verdict or publishable artifact.

The Beta-written `delta2gamma` program implements the declared elaboration; it
does not define it. Grammar/elaboration coverage and negative tests may land
before source-to-artifact authority. Authority additionally requires the
checked refinement route selected by D5 and D9.

Bootstrap file names identify their format and role. `.alpha` is Alpha assembly
source, `.proof` is proof-source input to untrusted elaboration, `.beta`,
`.gamma`, `.delta`, `.omg`, and `.psi` identify their respective source
languages, and `.tape` identifies canonical Alpha VM bytecode. Artifact base
names remain descriptive—for example `beta_compiler_bytecode.tape` rather than
`bc.tape`. Native realizations use target-qualified names and their native
container convention. The historical Delta `.alp` and proof-source `.elab`
suffixes are retired; path-only renames do not change path-independent content
or closure identities, but they invalidate any in-flight attempt that pinned
the changed locator or tooling files.

## Dependency order

1. close lower-rung semantics and proof checking needed by Delta publication;
2. publish the complete Delta compiler artifact;
3. census the compositional ordinary-Omega surface used by the complete source closure;
4. compile that closure into `omega₀` and check the edge;
5. compile the same closure with `omega₀` into `omega` and check the edge; and
6. optimize or reproduce artifacts without changing the semantic chain.
