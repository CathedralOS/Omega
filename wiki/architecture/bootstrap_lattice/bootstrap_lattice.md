# The Bootstrap Lattice

> **Status: DIRECTION + a working vertical slice.** The *principles* here are
> decided. The *rung breakdown* (alpha…omega) is a working decomposition, not a
> frozen contract — per the project stance that rung names are version labels and
> the count is emergent (it falls out of the machinery dependency DAG, it is not
> chosen). **As of now the lattice exists, in working form, from the seed up to a
> first certificate checker** — `alpha` (seed) → `beta` (self-hosting assembler) →
> a self-hosting **Beta compiler written in Beta** (Rust out of the lineage) →
> `delta` (a checker: full intuitionistic propositional logic + equality with the
> conversion rule) and `gamma` (an interpreter-first functional language with ADTs
> + pattern matching, which runs the same checker cleanly). One command verifies it
> all: `sh compiler/verify-lattice.sh`. Above delta (`epsilon`, `omega`) is still
> design; the deep open problem — a *soundness bridge* from `provable` to
> `true-about-execution` — is untouched. This document exists so the construction
> is **designed on paper rather than drifted into**.
>
> **Live build status + onboarding for a fresh agent:**
> [TASKS_BOOTSTRAP.md](../../../TASKS_BOOTSTRAP.md).

This is the architecture for how Omega builds *itself* — a tower of small
languages rising from a tiny hand-audited seed, ending at the full language. It
is a separate concern from two things it is easy to confuse it with:

- **What Omega means** — the language semantics. Owned by the
  [language guide](../../language_guide/language_guide.md) and the
  [design briefs](../../design_briefs/). The lattice must *preserve* that
  meaning; it does not define it.
- **How Omega compiles today** — the current Rust compiler `omega-rs`. Owned by
  [Repository Layout](../repository_layout.md) and
  [Pipeline Architecture](../pipeline/pipeline.md). In this architecture
  `omega-rs` is a *role*, not a rival (see [How today's work fits](#how-todays-work-fits)).

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
  **evidence** that its output is correct, and one tiny thing we *do* trust
  validates that evidence. Bad evidence is rejected. The producer is never
  trusted; only the checker is. ("Irrelevant to correctness" is not "fine to keep
  forever" — see [Two roles for Rust](#two-roles-for-rust-and-why-it-still-dies).)

The asymmetry is the whole game:

- Pedigree: you must trust *everything that ever touched the artifact*.
- Checking: you must trust *one small checker plus the meaning it checks
  against*. The compiler, the build host, the supply chain, the implementation
  language — all may be hostile and the result is still sound.

Trust-by-checking is a single answer to every threat we care about: **Thompson**
(the backdoored self-hoster's output fails the check), **supply-chain** (a
poisoned dependency yields artifacts that do not check), **layer-2-depending-on-
layer-6** (layer 2 is trusted because the checker validated it, not because
layer 6 produced it, so layer 6 can be garbage), and **the Rust lineage** (Rust
becomes a mere producer whose output is checked — outside the *soundness* base,
though [it still dies for provenance](#two-roles-for-rust-and-why-it-still-dies)).

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
lives **several rungs up**, as a program, not as native machinery (see
[Delta](rungs/delta.md)).

## Two kinds of minimality

These are different budgets and must be tracked separately.

- **Native minimality** — how much *architecture-specific* code must exist? Ideal:
  one `alpha` interpreter per ISA, a loader, a minimal I/O boundary. This is the
  hand-inspected native seed. Keep it tiny.
- **Semantic minimality** — how much code ultimately *participates in deciding
  correctness*? This is larger: the alpha VM, the beta/gamma interpreters, the
  delta checker, the written semantics, the certificates. But it is **not**
  hand-written assembly — it accumulates gradually in increasingly readable,
  safer languages, and **freezes** as each rung is finished.

The trustworthy core grows, but its lower layers stay frozen and comprehensible.

## Two stacks, joined only by a checker

Build two parallel stacks.

- The **meaning stack** is what programs *mean* — defined operationally by
  **reference interpreters** (see next section). Slow, canonical, the spine of
  truth.
- The **machine stack** is the actual stuff: source text, the real (untrusted)
  compiler, real bytes, real silicon.

They touch only through the checker: a producer emits output **plus a
certificate**; the checker validates the certificate against the meaning; only
checked output ships.

```text
   MEANING (reference route, slow)        MACHINE (fast, untrusted)
   ------------------------------         -------------------------
   what a program means         <—— check —— compiler output + certificate
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
Omega program
  interpreted by an Omega interpreter written in Epsilon
  interpreted by an Epsilon interpreter written in Delta
  interpreted by a  Delta interpreter written in Gamma
  interpreted by a  Gamma interpreter written in Beta
  interpreted by a  Beta interpreter written in Alpha
  executed by the native Alpha VM
```

**Semantic authority is not native code.** A new capability (say, a logical
calculus at Delta) arrives as *a program the previous rung already understands* —
not as more native kernel machinery. To `alpha`, a proof object is just a byte
sequence being processed by another alpha program; alpha has no idea what "proof"
means.

## Ladder vs lattice: diversity is the security

A pure chain (`alpha → beta → … → omega`) gives staged *comprehensibility*, but a
bug in one compiler still poisons everything above it. The **lattice** comes from
adding independent paths and comparing them:

```text
                 Gamma program
                 /            \
        interpreter         compiler
         in Beta             in Beta
            |                   |
            └──— compare behavior ——┘   (diamond: exposes disagreement)
```

- **Vertical edges add expressiveness.** **Diagonal edges add assurance.**
  **Diamonds expose disagreement.**

One caveat that matters for *our* threat model (see
[Honest edges](#honest-edges)): a diamond between two paths that share a low
ancestor only catches *implementation* bugs. Genuine Thompson / supply-chain
resistance requires paths that diverge **as low as possible** — different native
alpha implementations, on different ISAs, ideally authored independently. That
diversity is the actual defense, and it is the expensive part. Design it in
early; do not treat it as a later garnish.

## The irreducible trust ledger

You cannot reach zero trust. The honest, finite list:

1. **Logic itself.**
2. **The hardware assumption** — "these alpha bytes, on this physical ISA, execute
   per the alpha specification." Shrink the risk with multiple independent alpha
   implementations, different host ISAs, direct inspection, a formal ISA model,
   reproducible builds, and hardware testing. Alpha does **not** authenticate
   itself merely by being small.
3. **A short, frozen sequence of audited programs** — at birth, each of {the alpha
   VM, the beta interpreter, the gamma interpreter/checker, the delta proof
   checker} is hand-audited, because nothing above it yet exists to check it. The
   delta checker is a *second trust anchor* exactly like the alpha VM, just
   written in gamma (this is how Lean works: you audit the kernel, you do not
   derive it). The audit burden shrinks going up, as lower tooling helps check
   the next rung — but the foundational artifacts are pure audit.

The craft is making this set as small, as explicit, and as independently
re-verified as possible — then deriving or checking *everything else*.

## The rungs (working decomposition)

Names are working labels; the count is emergent. Each rung adds **one coherent
idea** and is implemented in the rung below.

| Rung | Adds (one idea) | Implemented in | Meaning defined by | Status |
| --- | --- | --- | --- | --- |
| [alpha](rungs/alpha.md) | raw computation: bytes, fixed-width arithmetic, bounded memory, load/store, branch, byte I/O, trap | native (hand-written per ISA) | the VM's own small-step semantics ([`SEMANTICS.md`](../../../compiler/alpha/SEMANTICS.md)) | **EXISTS** — 21-opcode tape VM, **two independent seeds** (x64, arm64) forming a diamond; written semantics + conformance suite; beta self-hosts on it |
| [beta](rungs/beta.md) | names + structure: a small structured systems language (procedures, locals, control flow, memory) — *and* the assembler beneath it | alpha | the assembler in alpha-asm; the Beta-language compiler, **written in Beta** (`bc.beta`) | **EXISTS + SELF-HOSTS** — assembler self-hosts byte-identically; `bc` (the Beta compiler in Beta) self-hosts, so Rust is out of the lineage |
| [gamma](rungs/gamma.md) | safe definitional computation: algebraic data, pattern matching, pure/total functions, a simple type system | beta | a gamma reference interpreter written in beta ([`interp.beta`](../../../compiler/gamma/interp.beta)) | **EXISTS** (interpreter-first) — fuel-bounded functional core + **ADTs + pattern matching**; runs the Delta checker ([`checker.gamma`](../../../compiler/gamma/checker.gamma)). A simple static type system now exists too (`typeck.beta`: Int + ADTs, monomorphic, catches Int-vs-List etc.). The old compiler-first v13 is parked. |
| [delta](rungs/delta.md) | **evidence**: a small logical calculus + a certificate checker | gamma | the delta checker (a gamma program) *is* the definition of a valid proof | **FIRST PROTOTYPE EXISTS** — a checker for full intuitionistic propositional logic + equality/conversion, in both Beta ([`check.beta`](../../../compiler/delta/check.beta)) and Gamma; **no soundness bridge yet** (the deep open problem) |
| [epsilon](rungs/epsilon.md) | safe systems programming: mutable memory, ownership, regions, effects | delta / gamma | an epsilon reference interpreter | DIRECTION — note: unrelated legacy `compiler/epsilon/` is to be cleared |
| [omega](rungs/omega.md) | the full language: contracts, refinement/dependent proofs, proof automation | epsilon | omega's written semantics + reference interpreter | DIRECTION — today realized by `omega-rs` (the Rust on-ramp) |

## How today's work fits

This architecture does not demote the existing docs; it assigns roles.

- **Language docs** (`language_guide/`, `design_briefs/`) own **meaning**. The
  lattice preserves it. Authoritative, unchanged.
- **`omega-rs`** (`pipeline/`, `repository_layout`) is the **current fast,
  untrusted producer** and today's executable reference for the language. In this
  architecture it sits on the *machine* side; it is progressively replaced by
  lattice-built rungs and, in the end-state, its output is *checked* rather than
  trusted. Its pipeline docs stay valid — they describe a real working artifact.
- [`alpha_language.md`](../../design_briefs/alpha_language.md) predates the
  tape-VM re-rooting and is **partly stale**; its trust-architecture framing is
  superseded here, while its concrete constraint list (resource budgets, banned
  features, trap-everything) is salvaged into [alpha](rungs/alpha.md) and
  [gamma](rungs/gamma.md).
- [`proof_engine_north_star.md`](../../design_briefs/proof_engine_north_star.md)
  and [`cathedral_alignment.md`](../../cathedral_alignment.md) already name the
  endpoint (a tiny kernel + automation + a verified translation; trust bottoms
  out at {seed, checker, specs, hardware}). This doc is the *construction* that
  reaches it.

## Two roles for Rust (and why it still dies)

Trust-by-checking says the producer's implementation language is irrelevant *to
the soundness of a checked artifact*. That is true and liberating — but it is
**not** "Rust is fine forever." Two different guarantees are in play, and Rust
threatens one of them regardless of checking:

- **Semantic correctness of an artifact** — given a sound checker, independent of
  the producer. A Rust-built Omega compiler whose output is checked can at worst
  produce output that *fails* the check; it cannot produce wrong-but-accepted
  output (it cannot forge a certificate the checker accepts). For this,
  Rust-as-producer is genuinely irrelevant.
- **Bootstrappability / self-sufficiency** — that the whole toolchain traces to an
  audited seed with no external unaudited dependency. Checking does nothing for
  this. A checked artifact built by an unaudited Rust+LLVM blob is sound but not
  self-sufficient — you still *need* that blob. For an OS aiming at bare-metal
  reproducibility, that dependency is exactly the thing being killed.

So Rust dies completely; checking only sets the **order**, because Rust plays two
different roles:

- **Rust as the checker or the meaning** (the reference interpreters, the
  semantics, today's contract-entailment engine) is the **trusted base itself**.
  If the checker is a Rust program, Rust *is* your trusted base — Thompson and
  supply-chain risk back in full. This must die **first**, and moving it onto the
  audited-seed lineage is *the entire reason the lattice exists*. The checker is a
  [Gamma](rungs/gamma.md) program run down the reference route to the
  hand-assembled [Alpha](rungs/alpha.md) seed — depending on {Alpha, Beta, Gamma},
  none of which is Rust.
- **Rust as the producer** (`omega-rs`, the optimizing compiler) is, once a
  checker exists, *outside the soundness base*. It still dies — for
  self-sufficiency — but it is the **deferrable** kill; a checked, Rust-built
  compiler is a fine interim state.

**Where the repo is today:** the differential-oracle reference interpreter and the
entailment engine are both Rust, and there is no checker — so Rust is currently
deep in the de-facto trusted base. Removing it from the *trusted* parts (checker +
meaning) is the urgent job; removing it from the *producer* is the cosmetic, later
one. Treat any framing that calls Rust "merely a producer" as describing the
*end-state after the checker exists*, not today.

## Honest edges

The places this architecture glides over real cost. Build with eyes open.

1. **A reference interpreter gives operational meaning, not logical meaning.**
   "What the interpreter does" tells you what programs *do* (perfect for checking
   a compiler preserves behavior). It does not give a theory to *prove things
   about* programs. Connecting "the logic delta checks" to "what programs actually
   do per the reference interpreter" is a **soundness theorem**
   (`provable-in-Delta ⟹ true-about-execution`) at the gamma/delta seam. That
   bridge is the hard core of the proof ambition; the reference interpreter is
   only half of "meaning."
2. **Diamonds catch compiler bugs; they do not resist Thompson without genuine
   diversity.** See [Ladder vs lattice](#ladder-vs-lattice-diversity-is-the-security).
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

## Open questions

- **Where does meaning start?** The smallest end-to-end slice is (a) a reference
  interpreter for one tiny language that *defines* its meaning, and (b) one
  checkable property about a program in it, with the checker run down the
  reference route. Pin the language fragment and the property.
- **Operational vs written semantics** — when (if ever) do we add a separately-
  written mathematical semantics alongside the reference interpreters, and prove
  the interpreters refine it? (Honest edge #1.)
- **The diversity plan** — how many independent alpha implementations, on which
  ISAs, authored how, to make the diamonds real Thompson resistance?
- **Certificate format** — the shared shape a producer emits and the delta checker
  reads. (Tracked also as proof-engine-north-star open question #3.)
- **Reconciling current gamma** — today's compiler-first imperative gamma vs the
  interpreter-first functional/total gamma this architecture wants.
- **Rung count and names** — emergent; revisit as the dependency DAG settles.

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
- **Open questions.**
