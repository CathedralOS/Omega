# Lattice — ratified decisions

This file records architectural decisions. Current implementation order lives
only in [`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## D1 — Rust exits by role, not by rung

The Rust implementation under `source/omega-rust/` may remain as a comparator,
migration aid, and working product while the lattice closes. It supplies no
semantic authority. Meaning/checking dependencies leave the trusted path before
convenient producers need to disappear.

## D2 — Superseded: the Delta-to-Gamma meaning route

The former decision made a Beta-written program parse Delta and emit Gamma for
evaluation. D11 supersedes it. That route crossed the Gamma ownership boundary,
made Beta an undeclared Delta implementation, and hid a bridge compiler behind
the phrase “meaning route.” Its implementation and receipts are deleted. Git
history is sufficient; no migration diagnostic may preserve a second compiler
architecture.

## D3 — Trust flows through proofs, not native pedigree

Canonical bootstrap artifacts are accepted Alpha tapes checked by direct
source-to-tape refinement under pinned source and Alpha semantics and an exact
observation profile. Native VM seeds have their own Alpha-realization
obligation. Reproducibility, producer identity, native realization, and
independent compiler agreement are useful evidence for operations and debugging
but are not compiler-correctness proofs.

## D4 — Every proof capability lands with operational seams

A calculus feature is not complete merely because the kernel parses it. Each
feature has positive seams to a real compiler obligation and negative controls
that perturb the claim and must reject.

## D5 — Compiler provenance closes by direct checked refinement

The required judgment is:

```text
source subject + produced Alpha tape
  → tape refines canonical source meaning
```

The obligation is reconstructed from both subjects. An artifact cannot define
the question against which it is accepted. Subject identity, source and Alpha
semantics, observation profile, resource profile, schema identity,
certificates, and admissions all participate in replay compatibility.

## D6 — Each rung is implemented by its immediate predecessor

The permanent chain is:

```text
audited Alpha seed
  → Alpha-written Beta compiler       → beta tape
  → Beta-written Gamma compiler       → gamma tape
  → Gamma-written Delta compiler      → delta tape
  → Delta-written Omega source D      → omega₀ tape
  → Omega-written Omega source C      → omega tape
```

Every arrow consumes a genuine compiler for exactly the language on its right.
No lower rung parses past its immediate successor. A compiler may use private
internal representations, but no older compiler, interpreter, or source
transpiler remains an external semantic dependency of a later edge.

`D` is the exact Delta-written source closure of the first full Omega compiler.
`C` is the exact Omega-written source closure of the self-hosting compiler.
They are different implementations of the same complete Omega language.
`omega₀` may be slow and poorly optimized, but it must compile `C` with exact
Omega semantics. The resulting `omega` may apply the optimizer implemented by
the compiler source.

The three source facts remain distinct:

| Contract | Meaning |
| --- | --- |
| Delta v1 | independent compiler-host language used to author `D` |
| features used by `C` | incidental ordinary-Omega subset used to author the self-hosting compiler |
| full Omega | language implemented for users by both Omega compiler implementations |

The Delta compiler accepts Delta, not Omega. The Omega compiler built from `D`
accepts the ordinary-Omega surface exercised by `C` and implements full Omega.
The compiler built from `C` is the optimized self-hosted implementation. Neither
source closure defines the Omega language it implements.

## D7 — Scripts coordinate but never become stages

Shell, Python, or other host scaffolding may invoke the chain, stamp an exact
tape into a selected Alpha seed, compare artifacts, and report diagnostics. It
may not discover a compiler closure, parse or lower accepted source, manufacture
evidence, or decide trust. A required semantic transformation must be
implemented in the named compiler/checker stage, not hidden in a runner.

## D8 — Evidence and admissions compose transitively

Verification records are re-derivable and subject-indexed. Admissions remain
visible through dependency closure and are re-evaluated under each consumer's
policy. A dependency cannot launder an unresolved obligation into a clean
“verified” result.

Human-facing output keeps verified facts, admitted claims, and provenance
metadata visually and semantically distinct. There is no unqualified “artifact
proven correct” verdict; the honest statement names the source, Alpha semantics,
observation and resource profiles, and admissions.

## D9 — Cyclic compiler refinement starts inside the existing calculus

A compiler edge does not add a coinductive or labelled-transition-system
judgment to the accepted checker merely because the subject contains loops. The
source language and Alpha have canonical deterministic small-step semantics.
The admission owner presents them as constructive total step functions with
explicit terminal self-loops and defines traces by primitive recursion over
`Nat`.

The proof relates source and Alpha states at nondecreasing synchronization
points. A source step may correspond to zero or more Alpha steps. Every
unmatched step must be observationally silent and decrease one well-founded
rank over the related state pair, so neither side can hide infinite internal
work. The existing first-order calculus and induction rules check the resulting
determinism, progress, observation, synchronization, and rank obligations.

The producer may elaborate and DAG-share the proof, but the artifact-aware owner
reconstructs both machines and the exact input, resource, and observation
profiles. Executable agreement remains diagnostic. A new trusted kernel
primitive is considered only after a concrete attempt proves an expressiveness
failure; certificate verbosity or producer inconvenience alone is insufficient.

## D10 — Delta meaning is independent; spelling and artifact names are explicit

Delta v1 is an independent compiler-host language with separately fixed syntax,
static judgments, small-step execution, resources, and observations. Delta may
share familiar spelling with Omega, but its language contract is self-contained:
neither the contract nor a checker may consult Omega documentation, a compiler
implementation, or the accepted corpus to decide what Delta source means. A
file accepted by both languages acquires only the meaning of the route under
which it is checked.

Delta refinement quantifies over a verifier-selected observation profile. The
profile fixes sealed input bytes, exact artifact and diagnostic bytes, and
terminal exit, rejection, trap, exhaustion, and incomplete outcomes. Ambient
filesystem state, environment, time, network, and process spawning are absent.

Each apparent fixed bound becomes exactly one of:

- a source-visible Delta semantic bound;
- an explicit resource-profile parameter; or
- a private producer/checker budget whose exhaustion yields `Incomplete` and
  grants no semantic verdict or publishable tape.

The Gamma-written Delta compiler implements Delta-to-Alpha compilation; it does
not define Delta. Coverage may land before authority. Authority additionally
requires the checked refinement route selected by D5 and D9.

Bootstrap file names identify format and role. `.alpha` is Alpha assembly
source, `.proof` is proof-source input to untrusted elaboration, `.beta`,
`.gamma`, `.delta`, `.omg`, and `.psi` identify their respective source
languages, and `.tape` identifies canonical Alpha VM bytecode. Artifact base
names remain descriptive. Native realizations are optional target-qualified
containers, never the canonical compiler identity.

## D11 — Alpha tape is the canonical bootstrap artifact

Every required compiler artifact from Beta through `omega` is one
platform-independent Alpha tape. Each compiler is standalone: it consumes its
own language and emits the next exact tape without invoking an older compiler,
interpreter, assembler, or host script to perform a semantic transformation.
The Alpha-written first compiler may use the Alpha assembler during its audited
construction; later compilers emit the canonical tape format directly.

A target-specific Alpha VM seed executes any admitted tape. Stamping a tape
into a seed or placing it in an equally transparent container is packaging, not
another compiler edge. Canonical identity and refinement belong to the tape,
not the Mach-O, ELF, PE, signature, or installation wrapper.

The product Omega compiler may emit native user artifacts. That does not make
native code the canonical bootstrap representation of the compiler itself. A
general Alpha-to-native realizer may accelerate any Alpha tape only when its
relation to Alpha semantics is independently checked. Source-, function-,
hash-, or workload-specific native substitutions (“jets”) are forbidden.

Common Alpha targeting reduces the trusted executable vocabulary but can
increase generated-code and certificate size. Internal CFGs, typed blocks, or
other proof vocabulary may structure a derivation, provided they are checked
lemmas rather than external historical compiler stages.

## D12 — Owner escalation protects the lattice from local workarounds

Implementation stops for an owner ruling rather than silently changing the
architecture when any of these occurs:

- representative `delta → omega₀` or `omega₀ → omega` work has terrible wall
  time, memory, or tape-size behavior after ordinary algorithmic and diagnostic
  cleanup;
- Alpha's instruction set appears too weak or excessively verbose, including
  pressure to add an opcode, widen an encoding, or smuggle a higher-level
  operation into the VM;
- source-to-Alpha certificates or checking time grow prohibitively despite DAG
  sharing, compositional lemmas, and removal of duplicate evidence;
- a special native accelerator, source-pattern substitution, tape-hash shortcut,
  or other jet appears necessary;
- target ABI, object-format, runtime, or hardware behavior leaks into Beta,
  Gamma, or Delta instead of remaining at the Alpha realization or Omega
  product boundary;
- a compiler cannot emit the next runnable tape without invoking an older rung,
  host parser/lowerer, or semantic script;
- a realistic compiler closure exhausts a private bound, requires undefined
  Alpha behavior, or cannot receive an explicit fail-closed resource profile;
- proving an edge appears to require a new trusted axiom or kernel rule rather
  than better untrusted proof production;
- conforming Alpha realizations disagree on the same exact tape and input;
- retaining a legacy component requires a second accepted chain, duplicated
  semantic owner, or permanent compatibility adapter; or
- implementation pressure encourages weakening a language contract,
  observation profile, subject identity, or fail-closed behavior.

These are escalation criteria, not advance permission to add complexity. Until
the owner rules, the existing language semantics and Alpha instruction set stay
fixed and the affected edge remains open.

## Dependency order

1. finish the Alpha-written Beta compiler edge and common tape boundary;
2. publish the Beta-written Gamma compiler tape;
3. implement and publish the Gamma-written Delta compiler tape;
4. compile the Delta-written Omega source closure `D` into `omega₀`;
5. compile the Omega-written source closure `C` with `omega₀` into `omega`; and
6. optimize or natively realize tapes without changing the semantic chain.
