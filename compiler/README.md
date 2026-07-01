# `compiler/` — the bootstrap lattice

## Are we building six compilers?

**No — one tower, built bottom-up, aimed at a single summit (Omega).** The names
(alpha, beta, gamma, delta, epsilon, omega) are not six rival products; they are
**rungs of one ladder**. Each rung is built *and checked* by the rung beneath it, so the
only thing you have to take on faith is the ~300-line seed at the very bottom — and even
that is hand-audited and cross-checked. Two rungs (the `*-rs` ones) are **disposable
scaffolding**: throwaway Rust compilers that exist only to bootstrap a rung, then leave
the trust path for good.

The thesis is **trust by checking, not by pedigree**: nothing is trusted because of where
it came from (a vendor, a binary, a previous compiler); every rung's output is *re-derived,
conformance-tested, or diverse-double-checked*. The whole tower verifies in one command:

```sh
sh verify-lattice.sh        # seed → assembler → bc → checker → systems language, every gate in order
```

## The summit: what Omega is *for*

Omega is meant to be a **self-hosting, verified systems language**: every program ships
with a machine-checked **proof of its own safety and correctness** — no out-of-bounds
access, no integer overflow, loops terminate, contracts (`requires`/`ensures`) hold — and
that proof is validated by a checker small enough to **audit by hand** rather than trust by
reputation. So the lattice has two jobs:

1. **Produce a trustworthy proof-checker** you can actually believe (that's `delta`), and
2. **Build a language whose programs emit proofs to it** (that's `epsilon` → `omega`).

Everything below is in service of those two jobs.

## The rungs (the ladder)

The lower you go, the smaller and more auditable; the higher you go, the more expressive.
Each rung is built by the one below it and pinned by its own automated gate.

| rung | what it is | kept or thrown away | how it earns trust |
| --- | --- | --- | --- |
| **alpha** | a ~300-line seed VM (21 opcodes) — the root of trust | **kept** (the seed) | hand audit + a 25-case conformance suite + a **diamond**: two independent seeds (x64, arm64) emit byte-identical bytecode |
| **beta** | the assembler, written in Alpha | **kept** | **self-hosts** — reproduces its own bytecode byte-for-byte |
| `beta-lang-rs` | a Rust on-ramp for the Beta language | **thrown away** | exists only to bootstrap `bc`, then leaves the lineage |
| **bc** (`beta-lang`) | the Beta compiler **written in Beta** | **kept** | self-hosts byte-for-byte — Rust is now out of the trust path |
| **gamma** | a safe functional language (ADTs + pattern matching): an interpreter and a static **type checker** | **kept** | 22-case interpreter gate + 23-case type-checker gate. Its job: make the proof-checker *safe to write* (and provide a second, independent implementation of it) |
| **delta** | the **certificate checker** — the trust anchor | **kept** (the point of it all) | defended five independent ways — see below |
| `epsilon-rs` | a Rust on-ramp for **epsilon**, the systems language | **thrown away** (eventually) | doesn't just compile — it **runs** its corpus on real hardware under a 75-case gate |
| **epsilon** | the systems language (machines, transitions, `data`, arrays, host I/O, trap-on-overflow) — and an epsilon compiler **being written in epsilon** | **kept** (in progress) | the self-hosting passes are cross-checked **byte-for-byte against the trusted backend's own output** |
| **omega** | the summit — full dependent types | design-stage | — |

Trust flows strictly bottom-up: the hand-audited seed runs the assembler, which lowers
`bc`, which compiles the checker, which validates a proof. No rung trusts its builder
blindly — every step is re-checkable.

## Why is a compiler project full of math proofs?

This is the question that surprises people, so here it is head-on. Three answers, in order
of how directly they aim at Omega:

**1. `delta` is a proof *checker*, and these are its test cases.** `delta` doesn't *find*
proofs — it *decides* whether a given proof is valid. To trust it, we exercise it: feed it
real theorems and confirm it accepts the sound ones and rejects the bogus ones. The
number-theory results (commutative semiring → divisibility → the Euclidean algorithm → the
**Fundamental Theorem of Arithmetic**) are a deliberate **torture test**: a checker that can
correctly verify a 231-lemma proof that every integer factors into primes can certainly
verify the small, dull obligations a real program emits. (And the **soundness battery** is
the other half of the test: famous *non*-theorems — excluded middle, the drinker paradox,
fabricated "proofs" — that the checker must *reject*.)

**2. The proofs are increasingly the *actual* obligations a verified program emits.**
A memory-safe array access needs a proof that the index is in bounds; a refinement type
needs a proof that one bounded range fits inside a wider one. So `delta` now also banks a
growing **library of verification conditions (VCs)** in exactly the form Omega's verifier
will generate them — flat/2-D/3-D and **strided (array-of-structs)** index bounds,
overflow bounds, slice-element bounds, bounded-integer **range subtyping**. These aren't
abstract math; they're the trust anchor shown discharging the precise lemmas the language
above it produces.

**3. The convergence — programs that prove themselves.** This is the Omega idea in
miniature, and it already runs (`epsilon-rs/convergence.sh`): an **epsilon program**
computes something (a sum, a sort, a gcd, a bounds check) **and emits a `delta`
certificate that its own result is correct**; the trust anchor then independently checks
that certificate. A *wrong* computation would emit a certificate `delta` **rejects** — the
anchor validates the computation, not the compiler. The two top rungs meet: a systems
program at the top is verified by the proof checker at the bottom. 76 such checks run today.

## `delta` — the trust anchor (`delta/`)

The rung the whole architecture exists to produce: a small, hand-auditable checker with
**sole authority over what is true**. An untrusted, arbitrarily clever proof-*search*
engine may propose certificates; this checker decides — a false proposition cannot get
past it, however the certificate was found.

`check.beta` decides **first-order intuitionistic predicate logic with induction**:
propositional logic (`→ & ∨ ⊥`, so `¬A = A→⊥`, by Curry–Howard a simply-typed λ-calculus
type checker); **equality** with the conversion rule (`refl` discharged by computation);
`∀`/`∃` with capture-avoiding instantiation; **induction** over the built-in types (Peano
naturals, lists) and over **user-declared `data` types**, plus user-defined **recursive
functions** whose equations reduce under conversion (so theorems *about* user functions
prove by induction); **named lemmas** (`def`/`use`) so big proofs factor; and two inductive
predicates added to the core — list membership `Mem(x,L)` and the relational product
`ProdIs(L,n)` — needed to even *state* facts like "every element of this list is prime."

### What it has actually proved (the catalog — skim on a first read)

The point of the list below is its *range*: pure logic, algebra, deep number theory, the
integers, and the safety obligations of real programs all pass through the same tiny checker.

- **Algebra.** The built-in naturals — and a number type with *user-defined* `+`/`*` — are
  each a full **commutative semiring** (both commutativities, both distributive laws, both
  associativities, the `0`/`1` identities), proved by induction + the lemma layer.
- **Logic.** Non-contradiction, the contrapositive, constructive de Morgan, the
  `¬∃ ↔ ∀¬` duality — with their *classical* converses (excluded middle, `¬¬A→A`) pinned as
  **rejected**, which is what makes the logic intuitionistic.
- **Order & number theory** (`delta/ORDER.md`). `≤` and `<` *defined in the logic*
  (`a≤b := ∃c. a+c=b`) and proved a **total order** / **strict order** (incl. trichotomy and
  `+`/`·` monotonicity); even-or-odd; **divisibility** proved a partial order; the
  **Euclidean division algorithm** (`∀b>0 ∀a. ∃q,r. a=bq+r ∧ r<b`) — both **existence** and
  **uniqueness**; **GCD existence** (93 lemmas in one proof); **decidable divisibility**;
  **prime-divisor existence** (175 lemmas); the **Fundamental Theorem of Arithmetic**,
  existence half (231 lemmas — every positive integer is a product of primes); **Euclid's
  lemma** (`p prime ∧ p∣ab → p∣a ∨ p∣b`, the Bézout-free way) and the **uniqueness** pieces
  of the FTA; and the **infinitude of primes** (244 lemmas). A complete **ℤ** is built as
  difference pairs and proved a linearly ordered commutative ring (`delta/INTEGERS.md`).
- **Functions & folds** (`delta/FUNCTIONS.md`). User list folds (`len`, `sum`, `product`,
  `map`) with their append/reverse laws; the structural **constructor triad** for user
  lists — injective, disjoint, exhaustive; `product` is a **monoid homomorphism**.
- **The VC library** (the bridge to Omega, per reason #2 above). Memory-safety index bounds
  — flat, 2-D, 3-D, **strided / array-of-structs**, slice-element — overflow bounds, and
  **bounded-range refinement subtyping**, each framed as the obligation the language emits.

## `epsilon` — the systems language, and self-hosting

`epsilon-rs` is the throwaway on-ramp, but it is unusual for a bootstrap stub: it doesn't
just compile, it **runs** its corpus on real hardware. Because its native x86-64 Windows-PE
output can't execute on this macOS/arm64 machine, there is a second backend (`src/aarch64.rs`)
that emits ARM64 assembly → `clang` → ad-hoc `codesign`; a 75-case gate then *executes* the
samples — value programs, bounds/overflow **traps**, and real stdin/stdout **filters**.

On top of that runs the genuinely Omega-aligned work: **a compiler for epsilon, written in
epsilon.** The self-hosting passes (each a single-responsibility `.alp` program) already
cover a real compiler front-to-mid:

- **front end** — a lexer, a bracket validator, and complete **name resolution** across all
  three namespaces (transition labels, `self.field` reads, `self.method()` calls), plus
  scope-aware **duplicate-definition** and **dead-code** detection;
- **representations** — it computes the program's **data layout** (field byte-offsets, frame
  size) and **control-flow graph**, reproducing the trusted backend's own numbers *exactly*;
- **code generation** — it assigns each state its assembly label, lowers transitions to the
  correct branch instructions, and lowers expressions (shunting-yard to stack-machine order;
  constant materialization; the binary-operator snippets) — **cross-checked byte-for-byte
  against the real backend's generated assembly**.

That cross-check is the discipline that matters: the language being built verifies *itself*
against the trusted reference, the same way every other rung does.

## Why you can believe the trust anchor

`delta` is defended five independent ways, all under `verify-lattice.sh`:

- **377-case gate** (`test.sh`) — valid certificates accepted, invalid rejected.
- **43-case soundness battery** (`soundness.sh`) — non-theorems that must *all* be rejected,
  including classically-valid-but-non-constructive tautologies and fabricated memberships.
- **83-case checker diamond** (`checker-diamond.sh`) — *diversity = security* applied to the
  checker itself: `check.beta` (Beta) and `checker.gamma` (Gamma, a totally independent
  implementation) must return identical verdicts on every proof. It has caught real bugs.
- **type-safety** — `checker_typed.gamma` is the checker fully type-annotated, and gamma's
  own type checker accepts it: the trust anchor's *code* is statically safe.
- **soundness seams** — `semantics-diamond.sh` (definitional `=` vs the interpreter's
  operational eval) and `induction-soundness.sh` (inductively-proved universals confirmed
  against the interpreter at concrete instances): *evidence* for the open soundness theorem
  `provable ⟹ true-about-the-reference-interpreter`.

## Honest frontiers

- The **soundness theorem itself** — `provable ⟹ true` — is the deep open problem; what we
  have is strong evidence (the seams above), not a proof of it.
- **delta**: `fun` rules pattern-match only *user* constructors (no Bool test over built-in
  naturals); N-ary (3+) function arguments aren't supported.
- **epsilon**: the self-hosting compiler still needs the operand half of expression lowering
  (field/local loads, which consult the layout table) and statement codegen before it
  closes the loop and compiles itself.
- **omega** (full dependent types) is design-stage — the summit the rest of the ladder is
  built to reach.
