# `compiler/` — the bootstrap lattice

One tower, built bottom-up, aimed at hosting the **Psi/Omega** toolchain from a
small audited seed. Omega programs can ship with machine-checked evidence of
their safety and correctness, validated by a proof kernel small enough to audit.

The rung names (Alpha, Beta, Gamma, Delta) are languages in one bootstrap spine,
each built by the rung beneath it. The only thing taken on faith is the ~300-line
seed at the very bottom, and even that is hand-audited and cross-checked three independent
ways. The thesis is **trust by checking, not by pedigree**: every rung's output is
re-derived, conformance-tested, or diverse-double-checked. The whole tower verifies in one
command:

```sh
sh verify-lattice.sh        # language spine plus proof-kernel and meaning gates
```

The lattice has two connected jobs:

1. Build a compiler-host language chain: Alpha → Beta → Gamma → Delta.
2. Produce a proof kernel the product toolchain can emit certificates to.

## The ladder is a tower of languages

Each greek name is a **language**, from the simplest and most auditable up to the richest.
A tier's *tools* — its compiler, interpreter, or checker — are built by the tier below and
pinned by their own gate. Trust flows strictly upward: the hand-audited seed runs the
assembler, which lowers `bc`, which compiles the checker, which validates a proof. Every
step is re-checkable. (The tiers are emergent — they fall out of the dependency graph — so
the names are labels, not a fixed contract.)

| tier | the language | built with it | how the tier earns trust |
| --- | --- | --- | --- |
| **α Alpha** | a ~300-line, 21-opcode VM + its bytecode and assembly — the root of trust | the **seed** (kept) and the **assembler** (kept) | hand audit against a written semantics + a conformance suite over every opcode + a **triple diamond** — two independently authored seeds (x64, arm64) and a Python reference agree byte-for-byte; the assembler self-hosts and an independent Python assembler agrees |
| **β Beta** | the first structured language | **bc**, its self-hosting compiler (kept); a Rust on-ramp, `beta-lang-rs` (scaffolding), cold-starts it | `bc` self-hosts to a fixed point — **Rust leaves the lineage** — and a second independent front-end double-compiles the trust surface |
| **γ Gamma** | a safe functional language: ADTs + pattern matching | an interpreter, static type checker, and an independent proof-kernel implementation | interpreter and type-checker gates; checker diamond |
| **δ Delta** | the compiler-host systems language | the self-hosting compiler, native producer, and Delta-to-Gamma meaning path | self-host fixed point, native corpus, meaning diamond |

**On the names.** The Greek letters track *languages*. The proof kernel is a
cross-cutting assurance service under `proof-kernel/`; Psi and Omega are compiler
products hosted by Delta rather than additional rungs. The **assembler** lives in
`beta/` and the gates label
its step `beta`, but it is an **Alpha** tool (written in Alpha, it turns Alpha assembly into
bytecode). Read “the beta assembler” as “Alpha's assembler,” and reserve **Beta**
for the language `bc` compiles.

## Why a compiler project is full of math proofs

Three reasons, in order of how directly they aim at Omega.

**1. The proof kernel *decides* proofs, and these are its test cases.** It doesn't find proofs; it
decides whether a given one is valid. To trust that verdict, we exercise it: feed it real
theorems and confirm it accepts the sound and rejects the bogus. The number-theory results
— commutative semiring → divisibility → the Euclidean algorithm → the **Fundamental Theorem
of Arithmetic** — are a deliberate torture test: a checker that correctly verifies a
231-lemma proof that every integer factors into primes will handle the small obligations a
real program emits. The **soundness battery** is the other half — famous non-theorems
(excluded middle, the drinker paradox, fabricated "proofs") that it must reject.

**2. The proofs are increasingly the *actual* obligations a verified program emits.** A
memory-safe array access needs a proof its index is in bounds; a refinement type needs a
proof one range fits inside a wider one. So `proof-kernel/` banks a growing library of
**verification conditions** in exactly the form Omega's verifier generates them —
flat/2-D/3-D and strided (array-of-structs) index bounds, overflow bounds, slice-element
bounds, bounded-integer range subtyping. The trust anchor, shown discharging the precise
lemmas the language above it produces.

**3. Convergence — programs that prove themselves.** The Omega idea in miniature, running
today (`omega/convergence-reference.sh`, `delta-rs/convergence.sh`): a program computes
something — a sum, a sort, a gcd, a bounds check — **and emits a proof certificate that
its own result is correct**; the trust anchor then checks that certificate independently.
The anchor validates the *result*; how the certificate was produced, and whether its author
is honest, is irrelevant. The two ends of the ladder meet: a program at the top is verified
by the checker at the bottom.

The library stands at **238 machine-checked theorems** today (`proof-kernel/README.md`,
`ORDER.md`, `INTEGERS.md`, `FUNCTIONS.md`, `FTA-UNIQUENESS.md`): pure logic, algebra,
number theory through the FTA and the infinitude of primes (a 244-lemma proof), a full ℤ
built as difference pairs, user-function fold laws, and the VC library above.

## The proof kernel — why you can believe the trust anchor

The rung the architecture exists to produce: a small, hand-auditable checker with sole
authority over what is true. An arbitrarily clever proof-*search* engine may propose
certificates; this checker decides — a false proposition cannot pass, however the
certificate was found.

`check.beta` decides first-order intuitionistic predicate logic with induction:
propositional logic (by Curry–Howard, a simply-typed λ-calculus type checker); equality
with the conversion rule (`refl` discharged by computation); ∀/∃ with capture-avoiding
instantiation; induction over the built-in types and over user `data` types; user recursive
functions whose equations reduce under conversion (so theorems *about* user functions prove
by induction); named lemmas so big proofs factor; and two inductive predicates — list
membership and the relational product — needed even to *state* facts like "every element of
this list is prime."

It is defended several independent ways, all under `verify-lattice.sh`:

- **acceptance + soundness** — valid certificates accepted, invalid rejected, plus a battery
  of famous non-theorems (including classically-valid but non-constructive tautologies) that
  must *all* be rejected. Rejecting those is what makes the logic intuitionistic.
- **the checker diamond** (83 cases) — *diversity is security* applied to the checker
  itself: `check.beta` (Beta) and `checker.gamma` (a fully independent Gamma implementation)
  must return identical verdicts on every proof. It has caught real bugs.
- **type-safety** — the checker's own code, fully type-annotated, is accepted by gamma's
  type checker: the trust anchor is statically safe.
- **soundness seams** — definitional `=` cross-checked against the interpreter's operational
  eval, and inductively-proved universals confirmed against the interpreter at concrete
  instances: evidence for the open bridge *provable ⟹ true-about-the-reference-interpreter*.

## The product toolchain — Psi/Omega

`omega-rs/` is the full Rust compiler for Omega: the untrusted fast producer and today's
executable reference. `omega/` holds the **Rust-free** artifacts — the meaning route and its
gates:

- **`omega2gamma.beta`** elaborates Omega source to a gamma meaning term (decision D2), which
  `interp.beta` runs — the same alpha→beta→bc lineage as the checker. It covers a growing
  kernel subset (state machines, self fields/arrays, cross-machine calls and recursion,
  stdin/stdout, self-methods) plus omega surface syntax.
- **`omega-meaning.sh`** runs real samples down this route; each must exit with the value its
  header documents.
- **`kernel-diamond.sh`** — a triple diamond over kernel-subset programs: native execution
  matches the Rust-free `omega2gamma → interp` matches the Rust cross-check, across
  arithmetic, machines, fields, arrays, calls, and I/O.
- **`convergence-reference.sh`** — the proof-carrying loop with no Rust anywhere: certifiers,
  including Omega's own safety obligations, are translated, *run*, and their proof
  certificates checked — all on the trusted lineage.

The subset grows slice by slice. The deep remaining arc is validating omega-rs's native
output per-compilation against this route — translation validation, decision D3.

## Honest frontiers

- The **soundness theorem** — *provable ⟹ true* — is the deep open problem; the seams above
  are strong evidence for it, and the proof of it remains ahead.
- The proof kernel's `fun` rules pattern-match user constructors only, and function application is
  binary-max.
- The Omega subset given Rust-free meaning is still a fragment; slices, strings, floats (a
  native-producer concern by design), and full dependent types lie ahead — the subset grows
  slice by slice, as it has since slice 0.
