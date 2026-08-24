# The Bootstrap Lattice

> **Status: DIRECTION + a working vertical slice.** The audited bootstrap spine is
> `Alpha → Beta → Gamma → Delta`; the complete build lattice continues
> `→ omega-bootstrap (accepts Ωself) → omega (implements full Ω)`. Alpha through Gamma exist on the audited
> lineage; the Delta rung's native corpus, self-hosting compiler, and meaning
> diamond exist while the full Rust-free hosting path remains under construction.
> The proof kernel is a cross-cutting assurance service, independently
> implemented in Beta and Gamma, rather than a language rung. One command checks
> the current construction: `sh bootstrap/verify-lattice.sh`.
>
> **Live build status + onboarding for a fresh agent:**
> [TASKS_BOOTSTRAP.md](../../../TASKS_BOOTSTRAP.md). Target ownership and paths:
> [Bootstrap repository structure](repository_structure.md).

This is the architecture for how the Psi/Omega toolchain is rebuilt from a tiny
hand-audited seed through increasingly capable languages. Delta builds a
profile-limited bridge compiler; that compiler builds the full optimizing Omega
compiler from deliberately constrained Omega source. Only the optional final
Omega-to-Omega rebuild is strict self-hosting. The construction is separate
from two things it is easy to confuse it with:

- **What Omega means** — the language semantics. Owned by the
  [language guide](../../language_guide/language_guide.md) and the
  [design briefs](../../design_briefs/). The lattice must *preserve* that
  meaning; it does not define it.
- **How Omega compiles today** — the current Rust implementation under
  `bootstrap/onramps/omega-rust/psi/` and `bootstrap/onramps/omega-rust/omega/`, exposed by `bootstrap/onramps/omega-rust/apps/omega-cli/`. Owned by
  [Repository Layout](../repository_layout.md) and
  [Pipeline Architecture](../pipeline/pipeline.md). In this architecture
  the product compiler is a *role*, not a rival (see
  [How today's work fits](#how-todays-work-fits)).

## The one idea: trust by checking, not by pedigree

There are exactly two ways anything becomes trustworthy.

- **Trust by pedigree** — "this is good because a good thing made it." Layer 0 is
  audited; layer 0 built layer 1; layer 1 built layer 2. This is what ordinary
  bootstrapping (and our current rung-climbing) does. Its fatal flaw for a
  *security* language: a good pedigree does not make a thing **correct**. A
  backdoored compiler that compiles itself has a perfect pedigree and reproduces
  its backdoor forever — that is the Thompson attack exactly. Pedigree buys "I
  know where this came from," never "this is right."
- **Trust by checking** — "this is good because I checked it, and I do not care
  who made it." The producer can be a vast LLM-grown compiler, or malware, or
  Rust — irrelevant *to whether the output is correct* — because it must hand over
  **evidence** that its output is correct, and a deliberately small verification
  base validates both the required claims and their derivations. Bad evidence is
  rejected. The producer is not trusted; the artifact verifier, proof kernel,
  canonical semantics, and disclosed admissions are. ("Irrelevant to correctness"
  does not make Rust a required dependency — see
  [Two roles for Rust](#two-roles-for-rust-and-where-it-exits).)

The asymmetry is the whole game:

- Pedigree: you must trust *everything that ever touched the artifact*.
- Checking: you must trust *one small verification stack plus the meaning it
  checks against*. The compiler, the build host, the supply chain, and the
  producer's implementation language may be hostile and the result is still
  sound.

Trust-by-checking is a single answer to every threat we care about: **Thompson**
(the backdoored self-hoster's output fails the check), **supply-chain** (a
poisoned dependency yields artifacts that do not check), **layer-2-depending-on-
layer-6** (layer 2 is trusted because the checker validated it, not because
layer 6 produced it, so layer 6 can be garbage), and **the Rust lineage** (Rust
becomes a mere producer whose output is checked — outside the *soundness* base,
though [it still exits the required build](#two-roles-for-rust-and-where-it-exits)).

The historical reason almost nobody does this is that it demands writing the
meaning down formally and making producers emit proofs — brutal human labor (a
verified C compiler was ~6 person-years). **That labor is exactly what LLMs and
unlimited time dissolve.** This project is positioned to make trust-by-checking
the default rather than a heroic stunt.

## Five roles that masquerade as "the bottom"

Most confusion in this space comes from one word ("the trust root", "rung 0")
standing for five different jobs. Keep them apart:

1. **Executor** — physically runs instructions. A CPU; our tape VM. Dumb muscle.
2. **Meaning** — a precise *written* description of what programs do. Not code; a
   spec. It never runs; it is the truth running is measured against.
3. **Checker** — given an artifact and a claim, answers yes/no. Produces nothing;
   it judges.
4. **Translator** (compiler) — turns a higher representation into a lower one.
5. **Axioms** — the handful of things you cannot check and must accept: the chip
   obeys its manual, logic holds, the human who read the smallest seed understood
   it.

`alpha` is only role #1, an executor. It is **not** the meaning, the checker, or
the axioms. "Is alpha the trust root?" is malformed in the way "is the engine the
car?" is malformed. The checker (role #3) exists in this architecture — but it
is a cross-cutting program rather than a rung (see the
[proof kernel](proof_kernel.md)).

## Two kinds of minimality

These are different budgets and must be tracked separately.

- **Native minimality** — how much *architecture-specific* code must exist? Ideal:
  one `alpha` interpreter per ISA, a loader, a minimal I/O boundary. This is the
  hand-inspected native seed. Keep it tiny.
- **Semantic minimality** — how much code ultimately *participates in deciding
  correctness*? This is larger: the alpha VM, the beta/gamma interpreters, the
  proof kernel, the written semantics, and the certificates. But it is **not**
  hand-written assembly — it accumulates gradually in increasingly readable,
  safer languages, and **freezes** as each rung is finished.

The trustworthy core grows, but its lower layers stay frozen and comprehensible.

## Two stacks, joined by artifact verification

Build two parallel stacks.

- The **meaning stack** is what programs *mean* — defined operationally by
  **reference interpreters** (see next section). Slow, canonical, the spine of
  truth.
- The **machine stack** is the actual stuff: source text, the real (untrusted)
  compiler, real bytes, real silicon.

They touch through two deliberately separate checks. An **artifact verifier**
reads the canonical artifact and independently reconstructs the exact claims
that artifact must establish. The **proof kernel** then checks the supplied
derivations of those claims. A producer emits output plus evidence, but it does
not choose which claims count as sufficient; only fully verified output ships.

```text
   MEANING (reference route, slow)        MACHINE (fast, untrusted)
   ------------------------------         -------------------------
   what a program means     <—— verify artifact + check proofs —— output + evidence
        |                                          |
   ...defined by interpreters                 ...produced by any tooling
        |                                          |
   alpha small-step semantics                 native bytes
        |                                          |
   formal ISA model            <—— gap ——     real silicon
```

At the very bottom the meaning stack ends at *a formal model of the chip*; the
machine stack ends at *the actual chip*. The gap between them — does silicon
truly obey its own manual — is the one thing software can never close. That is
the irreducible axiom.

## Meaning is a reference interpreter; the compiler is an acceleration

For each language, the **definition of meaning is a canonical interpreter**:

```text
meaning of a Gamma program  =  what the Gamma reference interpreter does
```

A compiler is then *merely an accelerator* — "Alpha code that should behave like
the interpreter" — which is far easier to reason about than letting an optimizing
compiler *be* the definition. The reference route nests all the way down and is
absurdly slow, and that is fine; it is the semantic spine, not the production
path:

```text
omega-bootstrap hosted in Delta
  full Omega compiler built by omega-bootstrap from Ωself source
  Delta meaning elaborated to Gamma
  Gamma interpreted by a Beta program
  Beta compiled to Alpha
  Alpha executed by the native seed
```

**Semantic authority is not native code.** A new capability arrives as a program
the lower spine already understands, not as more native kernel machinery. To
Alpha, a proof object is just a byte sequence processed by another Alpha program;
Alpha has no idea what “proof” means.

## Why this is a lattice rather than a pedigree chain

The build spine (`alpha → beta → … → omega`) gives staged comprehensibility. It
does not become trustworthy merely by extending that pedigree. The **lattice**
comes from joining each build edge to canonical meaning and artifact checking:

```text
source ──────────────── canonical meaning
  │                            │
  │ untrusted compiler         │ reconstructed refinement obligation
  ▼                            ▼
artifact ─────────────── proof/semantic checker
```

- **Vertical build edges add expressiveness and self-sufficiency.**
- **Meaning and checking edges add authority.**
- **Reference comparisons and independent implementations expose bugs, but do
  not replace a checked refinement claim.**

Cross-implementation comparisons remain useful engineering tests where cheap,
but they are not trust edges and do not define the repository structure.
[D5](decisions.md#d5--direct-checked-refinement-closes-compiler-provenance)
records why direct checked refinement is the provenance rule.

## The irreducible trust ledger

You cannot reach zero trust. The honest, finite list:

1. **Logic itself.**
2. **The hardware assumption** — "these alpha bytes, on this physical ISA, execute
   per the alpha specification." Shrink the risk with direct inspection,
   conformance testing, a formal ISA model, reproducible builds, and additional
   platform realizations where useful. Alpha does **not** authenticate itself
   merely by being small or multiply implemented.
3. **A short, frozen sequence of audited programs** — the Alpha VM, the Beta and
   Gamma meaning path, and the proof kernel. Its Beta and Gamma implementations
   are cross-checked against shared corpora and semantic seams while the formal
   soundness bridges mature. The audit burden shrinks going up, as lower tooling
   helps check each next artifact.

The craft is making this set as small, as explicit, and as independently
re-verified as possible — then deriving or checking *everything else*.

## The fixed language spine, bridge, and one hosted production build

The language names and order are fixed by [D6](decisions.md). Each small
bootstrap rung adds **one coherent idea** and is implemented in the rung below.

| Rung | Adds (one idea) | Implemented in | Meaning defined by | Status |
| --- | --- | --- | --- | --- |
| [alpha](rungs/alpha.md) | raw computation: bytes, fixed-width arithmetic, bounded memory, load/store, branch, byte I/O, trap | native (hand-written per ISA); Alpha assembler written in Alpha | the VM's own small-step semantics ([`SEMANTICS.md`](../../../bootstrap/rungs/alpha/SEMANTICS.md)) | **EXISTS** — 21-opcode tape VM, audited x64/arm64 realizations, written semantics, conformance suite, and self-hosting Alpha assembler |
| [beta](rungs/beta.md) | names + structure: a small structured systems language (procedures, locals, control flow, memory) | alpha | the Beta-language compiler, **written in Beta** (`bc.beta`), lowers to Alpha assembly | **EXISTS + SELF-HOSTS + REFINES** — the Alpha-rooted artifact is used downstream and its complete `B_bc1` maximal observable is checked below `bc` |
| [gamma](rungs/gamma.md) | safe definitional computation: algebraic data, pattern matching, pure functions, fuel-bounded evaluation, a simple type system | beta | a Gamma reference interpreter written in Beta ([`interp.beta`](../../../bootstrap/rungs/gamma/interp.beta)) | **EXISTS** — fuel-bounded functional core, ADTs, pattern matching, and a static type checker; also hosts an independent proof-kernel implementation ([`checker.gamma`](../../../bootstrap/assurance/proof-kernel/implementations/gamma/checker.gamma)) |
| [delta](rungs/delta.md) | a robust deterministic compiler-host surface justified by the complete `omega-bootstrap` source closure | gamma | Delta-to-Gamma elaboration plus the Gamma reference interpreter | **WORKING RUNG** — native corpus, self-hosting compiler, and meaning diamond exist; the v1 feature inventory and full Rust-free toolchain hosting remain open |

The [proof kernel](proof_kernel.md) and the [Psi/Omega toolchain](omega_toolchain.md)
are connected nodes in the architecture, not additional rungs in this table.

The build continues through a bridge compiler and the production compiler:

```text
Alpha → Beta → Gamma → Delta
Delta bridge source ──[lattice-built Delta compiler]──▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (full Ω; conservative binary)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
```

Delta is independent rather than an Omega subset. `Ωself` is not another
language rung: it is a mechanically enforced, compositional subset of ordinary
Omega under which the exact production compiler source closure must fit. The
source manifest proves closure; it is not a whitelist or a set of hard-coded
AST shapes. `omega-bootstrap` may reject every Omega program outside that
profile, but accepted programs keep exact Omega semantics. The production
compiler source uses `Ωself` while implementing the complete language for users.

The bridge binary may itself be slow and may lower the production compiler
conservatively. It compiles the `Ωself` source that implements the product
optimizer and advanced lowering rather than duplicating those passes. A further
production `omega` → `omega` self-rebuild can optimize the compiler binary and
is optional. The required bridge → product edge is a cross-language hosted
build, not that self-rebuild. See
[`compiler_source_profile.md`](compiler_source_profile.md).

## How today's work fits

This architecture does not demote the existing docs; it assigns roles.

- **Language docs** (`language_guide/`, `design_briefs/`) own **meaning**. The
  lattice preserves it. Authoritative, unchanged.
- **`bootstrap/onramps/omega-rust/{psi,omega}/`** (`pipeline/`, `repository_layout`) forms
  the **current fast, untrusted producer** and today's executable reference for the language. In this
  architecture it sits on the *machine* side; it is progressively replaced by
  lattice-built rungs and, in the end-state, its output is *checked* rather than
  trusted. Its pipeline docs stay valid — they describe a real working artifact.
- [`alpha_language.md`](../../design_briefs/alpha_language.md) records the
  tape-VM execution substrate and explicitly retires the former “Alpha as an
  Omega subset/compiler” design.
- [`proof_engine_north_star.md`](../../design_briefs/proof_engine_north_star.md)
  and [`cathedral_alignment.md`](../../cathedral_alignment.md) already name the
  endpoint (a tiny kernel + automation + a verified translation; trust bottoms
  out at {seed, checker, specs, hardware}). This doc is the *construction* that
  reaches it.

## Two roles for Rust (and where it exits)

Trust-by-checking says the producer's implementation language is irrelevant *to
the soundness of a checked artifact*. That is true and liberating — but it is
**not** "Rust may remain required forever." Two different guarantees are in
play, and a required Rust dependency threatens one of them regardless of
checking:

- **Semantic correctness of an artifact** — given a sound artifact verifier and
  proof kernel, independent of the producer. A Rust-built Omega compiler whose
  output is checked can at worst produce output that *fails* verification; it
  cannot choose an easier obligation or forge a derivation the kernel accepts.
  For this, Rust-as-producer is genuinely irrelevant.
- **Bootstrappability / self-sufficiency** — that the whole toolchain traces to an
  audited seed with no external unaudited dependency. Checking does nothing for
  this. A checked artifact built by an unaudited Rust+LLVM blob is sound but not
  self-sufficient — you still *need* that blob. For an OS aiming at bare-metal
  reproducibility, that dependency is exactly the thing being killed.

So Rust exits every required trust, bootstrap, and release role; that does not
require deleting a useful parallel implementation from the repository.
Checking sets the **order**, because Rust plays two different roles:

- **Rust as the artifact verifier, proof kernel, or meaning implementation** is
  part of the trusted base. The generic proof kernel already has Beta and Gamma
  implementations on the audited-seed lineage. Terminal-Psi obligation
  reconstruction is defined by a total low-rung semantic-ledger generator over
  canonical bytes; direct low evaluation or a checked derivation of that same
  result is authoritative. Until that route lands, every Rust verifier,
  reduction family, and denotation rule it supplies is named explicitly as a
  versioned trusted dependency.
- **Rust as the producer** (`bootstrap/onramps/omega-rust/{psi,omega}/`) is, once a
  complete verifier-plus-kernel route exists, *outside the soundness base*. It
  must become omittable for self-sufficiency, but its removal from the repository
  is unnecessary. It may remain a maintained, non-authoritative differential
  comparator while its bug-finding value justifies the cost; a verified,
  Rust-built compiler output is also a fine interim state.

**Where the repo is today:** the low proof kernel exists in Beta and Gamma. The
current terminal-Psi artifact verifier, terminal interpreter, source proof
engine, and production compiler are still Rust. Rust therefore remains in the
explicit trusted path until canonical semantic-ledger reconstruction and
execution have audited closures. Its current status is recorded rather than
being implied by successful kernel checks. Removing Rust as a required producer
remains necessary for self-sufficiency; deleting the parallel comparator is not.

## Honest edges

The places this architecture glides over real cost. Build with eyes open.

1. **A reference interpreter gives operational meaning, not logical meaning.**
   "What the interpreter does" tells you what programs *do* (perfect for checking
   a compiler preserves behavior). It does not give a theory to *prove things
   about* programs. Connecting the proof kernel's logic to what programs actually
   do per the reference interpreter" is a **soundness theorem**
   (`kernel-accepted ⟹ true-about-execution`) at the proof/meaning seam. That
   bridge is the hard core of the proof ambition; the reference interpreter is
   only half of "meaning."
2. **Cross-implementation agreement is evidence, not authority.** It can catch
   bugs but cannot establish source-to-artifact correctness. Every compiler edge
   still needs the checked refinement shape described above.
3. **The trusted base is a sequence of audited programs, not just alpha.** See
   the [trust ledger](#the-irreducible-trust-ledger). Say it out loud rather than
   implying only alpha is inspected.
4. **Totality is not free; it reshapes every interpreter.** A *total* gamma cannot
   contain a plain interpreter for a Turing-complete language (it would loop on
   looping input). Reference interpreters become fuel-bounded
   (`interp(program, fuel) -> Result | OutOfFuel`) — which makes the slow route's
   slowness *bounded and explicit*, and is exactly
   [`totality_and_bounded_computation.md`](../../design_briefs/totality_and_bounded_computation.md).
   Thread fuel through the whole spine deliberately.
5. **"Alpha never grows" is true for computation, false for hardware.** Static
   features (ownership, regions, effects, types) are checked then erased and never
   touch alpha. But an OS needs a runtime and hardware access — allocator,
   atomics, memory fences, MMIO, interrupt entry. Those either reduce to alpha's
   existing ops or form a **second native boundary** that grows with hardware
   targets (Cathedral already needs atomics-as-real-LOCK). Freeze the
   computational core; deliberately manage a separate hardware-interface surface.

## Implementation and research frontiers

These are execution work under the standing decisions, not unresolved owner
architecture questions:

- **Operational vs written semantics** — when (if ever) do we add a separately-
  written mathematical semantics alongside the reference interpreters, and prove
  the interpreters refine it? (Honest edge #1.)
- **Alpha conformance depth** — strengthen written semantics, boundary tests,
  and formal ISA correspondence; add platform realizations when portability or
  concrete fault isolation justifies their audit and maintenance cost.
- **Terminal-Psi semantic-ledger realization** — the placement is settled: one
  total low-rung definition consumes canonical bytes and produces the complete
  ordered semantic ledger; deployment establishes its result by direct
  evaluation or a checked reconstruction derivation. The open engineering
  question is whether current Gamma expresses that definition readably and at
  acceptable reference-route cost. The spike must measure schema/audit size,
  decoding, denotation, control-flow availability, and execution cost rather
  than merely demonstrate that a toy traversal compiles.
- **Certificate coverage** — continue extending the shared, versioned
  proposition and derivation shape without changing the kernel/artifact-verifier
  responsibility split.
- **Delta sufficiency** — complete the bridge, prune Delta to the lowest-total-
  cost robust compiler-host language justified by its complete source closure,
  then freeze and prove that closure against the resulting v1 contract. Fewer
  features are not a win when their absence makes the bridge brittle or much
  larger.
- **Omega product-compiler source profile** — derive and enforce `Ωself` from
  the exact production compiler dependency manifest, with explicit exclusions
  and negative gates.
- **Hosted product build** — use `omega-bootstrap` once to build and validate
  the full optimizing compiler from `Ωself`-constrained Omega source. Its own
  binary may remain conservatively lowered until an optional rebuild.

## Rung Questions

Every rung document answers:

- **Adds** — the one coherent idea this rung introduces.
- **Written in** — the rung below (so its implementation is a program in that
  rung), and what its reference interpreter is written in.
- **Meaning** — what defines a program of this rung (its reference interpreter, or
  for alpha its small-step semantics).
- **Must not contain** — what belongs higher, kept out to keep the rung small and
  the trust argument clean.
- **Current repo reality** — where the actual repository is versus this target.
- **Implementation frontiers.**
